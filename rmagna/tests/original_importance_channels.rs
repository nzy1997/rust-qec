// SPDX-License-Identifier: MPL-2.0
// Complete finite contracts from pinned Magna commit
// 56847478c840395735146ef1e1ccdc3d8aedc10f, tests/core/test_importance_sampling.py:
// ImportanceSamplingTests.test_channel_proportions_correlations_endpoints_and_guard_policy
// and test_scaled_channel_totals_preserve_full_support_or_fail.
use num_rational::BigRational;
use rmagna::circuit::{Basis, BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::noise::{
    NoiseError, NoiseModel, NoiseOptions, OriginalNoiseAction, OriginalNoiseCategory,
    OriginalNoiseOptions, original_noise_categories,
};
use rmagna::probability::Probability;
use std::collections::BTreeSet;

fn rational(text: &str) -> BigRational {
    text.parse().unwrap()
}

fn probabilities(text: &str) -> Vec<Probability> {
    text.split_whitespace()
        .map(|p| p.parse().unwrap())
        .collect()
}

fn describe(source: &str) -> Vec<OriginalNoiseCategory> {
    let circuit = parse_circuit(source, &ParseOptions::default()).unwrap();
    original_noise_categories(&circuit, &OriginalNoiseOptions::default()).unwrap()
}

fn model(categories: &[OriginalNoiseCategory]) -> NoiseModel {
    let rows = categories
        .iter()
        .map(|category| {
            std::iter::once(category.identity_probability.clone())
                .chain(category.channels.iter().map(|c| c.probability.clone()))
                .collect()
        })
        .collect();
    NoiseModel::new(rows, &NoiseOptions::default()).unwrap()
}

fn rows(model: &NoiseModel) -> Vec<Vec<Probability>> {
    (0..model.num_sites())
        .map(|site| model.probabilities(site).unwrap().to_vec())
        .collect()
}

fn guarded_model_rows(categories: &[OriginalNoiseCategory]) -> BTreeSet<usize> {
    categories
        .iter()
        .enumerate()
        .filter(|(_, category)| category.guard.is_some())
        .map(|(row, _)| row)
        .collect()
}

fn pauli(basis: Basis) -> OriginalNoiseAction {
    OriginalNoiseAction::Pauli {
        factors: vec![(0, basis)],
    }
}

#[test]
fn original_channel_proportions_correlations_endpoints_and_guard_policy() {
    let source = concat!(
        "PAULI_CHANNEL_1(1/20,1/10,0) 0\n",
        "CORRELATED_ERROR(1/10) X0\nELSE_CORRELATED_ERROR(2/9) Z0\n",
        "M(1/100) 0\nX_ERROR(1/5) rec[-1] 0\nX_ERROR(0) 0\nZ_ERROR(1) 0"
    );
    let categories = describe(source);
    let metadata_before = categories.clone();
    assert_eq!(categories.len(), 6);
    assert_eq!(
        categories
            .iter()
            .map(|c| c.internal_site)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4, 5]
    );
    assert_eq!(
        categories
            .iter()
            .map(|c| c
                .source_operations
                .iter()
                .map(|a| a.operation)
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        vec![vec![0], vec![1, 2], vec![3], vec![4], vec![5], vec![6]]
    );
    assert_eq!(
        categories[0]
            .channels
            .iter()
            .map(|c| c.action.clone())
            .collect::<Vec<_>>(),
        vec![pauli(Basis::X), pauli(Basis::Y), pauli(Basis::Z)]
    );
    assert_eq!(
        categories[1]
            .channels
            .iter()
            .map(|c| c.action.clone())
            .collect::<Vec<_>>(),
        vec![pauli(Basis::X), pauli(Basis::Z)]
    );
    for (channel, anchor) in categories[1]
        .channels
        .iter()
        .zip(&categories[1].source_operations)
    {
        assert_eq!(&channel.source, anchor);
    }
    assert_eq!(
        categories[2].channels[0].action,
        OriginalNoiseAction::ReportFlip { record: 0 }
    );
    assert_eq!(categories[3].guard, Some(BitExpr::atom(BitAtom::Record(0))));
    for (site, category) in categories.iter().enumerate() {
        if site != 3 {
            assert_eq!(category.guard, None);
        }
    }
    assert_eq!(categories[3].channels[0].action, pauli(Basis::X));
    assert_eq!(categories[4].channels[0].action, pauli(Basis::X));
    assert_eq!(categories[5].channels[0].action, pauli(Basis::Z));

    let physical = model(&categories);
    let before = rows(&physical);
    assert_eq!(
        before,
        [
            "17/20 1/20 1/10 0",
            "7/10 1/10 1/5",
            "99/100 1/100",
            "4/5 1/5",
            "1 0",
            "0 1",
        ]
        .map(probabilities)
        .to_vec()
    );
    let excluded = guarded_model_rows(&categories);
    assert_eq!(excluded, BTreeSet::from([3]));
    let proposal = physical
        .scaled_proposal(&rational("2"), &excluded, &NoiseOptions::default())
        .unwrap();
    assert_eq!(
        rows(proposal.model()),
        [
            "7/10 1/10 1/5 0",
            "2/5 1/5 2/5",
            "49/50 1/50",
            "4/5 1/5",
            "1 0",
            "0 1",
        ]
        .map(probabilities)
        .to_vec()
    );
    let summary = proposal.summary();
    assert_eq!(summary.proposal_scale, rational("2"));
    assert_eq!(summary.categories, 6);
    assert_eq!(summary.scaled_categories, 3);
    assert_eq!(summary.unchanged_categories, 3);
    assert_eq!(summary.excluded_categories, 1);
    assert_eq!(summary.endpoint_categories, 2);

    // Literal factors are independent of the observed model probabilities.
    // In particular, the common fault factor remains 1/2 for a zero alternative;
    // computing its factor as p/q would instead attempt the undefined 0/0.
    let factors = [
        ("17/14", "1/2"),
        ("7/4", "1/2"),
        ("99/98", "1/2"),
        ("1", "1"),
        ("1", "1"),
        ("1", "1"),
    ];
    for (site, (identity_factor, fault_factor)) in factors.into_iter().enumerate() {
        let p = physical.probabilities(site).unwrap();
        let q = proposal.model().probabilities(site).unwrap();
        assert_eq!(p.len(), q.len());
        for (choice, (p, q)) in p.iter().zip(q).enumerate() {
            let factor = rational(if choice == 0 {
                identity_factor
            } else {
                fault_factor
            });
            assert_eq!(
                q.value() * factor,
                p.value().clone(),
                "site={site}, choice={choice}"
            );
            if site < 3 && choice != 0 {
                assert_eq!(*q.value(), p.value() * rational("2"));
            } else if site >= 3 {
                assert_eq!(p, q);
            }
        }
    }
    assert_eq!(rows(&physical), before);
    assert_eq!(categories, metadata_before);
}

#[test]
fn original_scaled_channel_totals_preserve_full_support_or_fail() {
    for (source, boundary, retry_row) in [
        ("X_ERROR(1/5) 0", "5", "3/5 2/5"),
        ("PAULI_CHANNEL_1(1/10,1/10,0) 0", "5", "3/5 1/5 1/5 0"),
        (
            "CORRELATED_ERROR(1/10) X0\nELSE_CORRELATED_ERROR(2/9) Z0",
            "10/3",
            "2/5 1/5 2/5",
        ),
        ("M(1/4) 0", "4", "1/2 1/2"),
    ] {
        let categories = describe(source);
        let physical = model(&categories);
        let before = rows(&physical);
        assert_eq!(physical.num_sites(), 1);
        let boundary = rational(boundary);
        for scale in [boundary.clone(), &boundary + rational("1")] {
            assert_eq!(
                physical
                    .scaled_proposal(&scale, &BTreeSet::new(), &NoiseOptions::default())
                    .unwrap_err(),
                NoiseError::ScaledProbability { site: 0 },
                "source={source}, scale={scale}"
            );
            assert_eq!(rows(&physical), before);
        }
        let retry = physical
            .scaled_proposal(&rational("2"), &BTreeSet::new(), &NoiseOptions::default())
            .unwrap();
        assert_eq!(
            retry.model().probabilities(0).unwrap(),
            probabilities(retry_row).as_slice()
        );
        assert_eq!(retry.summary().scaled_categories, 1);
        assert_eq!(rows(&physical), before);
    }

    let categories = describe("M 0\nX_ERROR(1/3) rec[-1] 0\nX_ERROR(0) 0\nZ_ERROR(1) 0");
    assert_eq!(
        categories
            .iter()
            .map(|c| c.internal_site)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(categories[0].guard, Some(BitExpr::atom(BitAtom::Record(0))));
    let physical = model(&categories);
    let before = rows(&physical);
    assert_eq!(
        before,
        ["2/3 1/3", "1 0", "0 1"].map(probabilities).to_vec()
    );
    let excluded = guarded_model_rows(&categories);
    assert_eq!(excluded, BTreeSet::from([0]));
    let original_source_sites = categories
        .iter()
        .filter(|c| c.guard.is_some())
        .map(|c| c.internal_site)
        .collect::<BTreeSet<_>>();
    assert_eq!(original_source_sites, BTreeSet::from([1]));
    // Exclusions index dense model rows. Reusing the source internal-site value
    // would leave the guarded row unprotected and must fail at model row zero.
    assert_eq!(
        physical
            .scaled_proposal(
                &rational("100"),
                &original_source_sites,
                &NoiseOptions::default()
            )
            .unwrap_err(),
        NoiseError::ScaledProbability { site: 0 }
    );
    assert_eq!(rows(&physical), before);
    let scaled = physical
        .scaled_proposal(&rational("100"), &excluded, &NoiseOptions::default())
        .unwrap();
    assert_eq!(rows(scaled.model()), before);
    assert_eq!(scaled.summary().proposal_scale, rational("100"));
    assert_eq!(scaled.summary().categories, 3);
    assert_eq!(scaled.summary().scaled_categories, 0);
    assert_eq!(scaled.summary().unchanged_categories, 3);
    assert_eq!(scaled.summary().excluded_categories, 1);
    assert_eq!(scaled.summary().endpoint_categories, 2);
    assert_eq!(rows(&physical), before);
}
