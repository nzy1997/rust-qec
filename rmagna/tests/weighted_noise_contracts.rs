// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//! Complete original WeightedNoiseTests.test_deterministic_nonuniform_and_importance
//! from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_weighted_noise.py.
//! Canonical XYZ channels retain zero X/Y masses. Typed source addresses and
//! SplitMix64 replace native string anchors and Python Random trajectories.
use num_rational::BigRational;
use rmagna::circuit::{Basis, ParseOptions, SourceLocation, parse_circuit};
use rmagna::instrument::{CompileOptions, FaultSelection, compile_affine};
use rmagna::noise::{
    NoiseModel, NoiseOptions, NoiseSampler, NoiseSampling, OriginalNoiseAction,
    OriginalNoiseAnchor, OriginalNoiseOptions, SplitMix64, original_noise_categories,
};
use rmagna::probability::Probability;
use rmagna::scalar::ScalarArithmetic;
use rmagna::unitary::CompilerStrategy;
use std::collections::BTreeSet;

const TEXT: &str = "PAULI_CHANNEL_1(0,0,1/3) 4";

fn p(text: &str) -> Probability {
    text.parse().unwrap()
}
fn q(numerator: i64, denominator: i64) -> BigRational {
    BigRational::new(numerator.into(), denominator.into())
}

#[test]
fn deterministic_nonuniform_and_importance() {
    let circuit = parse_circuit(
        TEXT,
        &ParseOptions {
            source: Some("original".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!((circuit.num_qubits(), circuit.num_records()), (5, 0));
    let categories = original_noise_categories(&circuit, &OriginalNoiseOptions::default()).unwrap();
    assert_eq!(categories.len(), 1);
    let category = &categories[0];
    let original = OriginalNoiseAnchor {
        operation: 0,
        location: SourceLocation {
            source: Some("original".into()),
            line: 1,
            column: 1,
            repeat: vec![],
        },
    };
    assert_eq!(category.internal_site, 0);
    assert_eq!(
        category.source_operations.as_slice(),
        std::slice::from_ref(&original)
    );
    assert_eq!(category.guard, None);
    assert_eq!(category.identity_probability, p("2/3"));
    assert_eq!(category.channels.len(), 3);
    for ((channel, basis), mass) in category
        .channels
        .iter()
        .zip([Basis::X, Basis::Y, Basis::Z])
        .zip(["0", "0", "1/3"])
    {
        assert_eq!(channel.source, original);
        assert_eq!(channel.probability, p(mass));
        assert_eq!(
            channel.action,
            OriginalNoiseAction::Pauli {
                factors: vec![(4, basis)]
            }
        );
    }
    // Native X/Z conditional weights (0,1) become XYZ masses (0,0,1/3).
    // There are no measurement sites or report-flip actions in this model.
    assert!(
        category
            .channels
            .iter()
            .all(|channel| matches!(channel.action, OriginalNoiseAction::Pauli { .. }))
    );
    let arithmetic = ScalarArithmetic::default();
    for compiler_strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let compiled = compile_affine(
            &circuit,
            &CompileOptions {
                compiler_strategy,
                ..Default::default()
            },
            &arithmetic,
        )
        .unwrap();
        assert_eq!(compiled.noise_sites().len(), 1);
        let site = &compiled.noise_sites()[0];
        assert_eq!(site.source_operations(), &[0]);
        assert_eq!(site.identity_probability(), &p("2/3"));
        assert_eq!(
            site.alternatives()
                .iter()
                .map(|alt| alt.probability().clone())
                .collect::<Vec<_>>(),
            [p("0"), p("0"), p("1/3")]
        );
        let word = site.alternatives()[2].word();
        assert_eq!((word.width(), word.phase()), (5, 0));
        for qubit in 0..5 {
            assert!(!word.x().get(qubit).unwrap());
            assert_eq!(word.z().get(qubit).unwrap(), qubit == 4);
        }

        let options = NoiseOptions::default();
        let physical = NoiseModel::physical(&compiled, &options).unwrap();
        assert_eq!(physical.num_sites(), 1);
        assert_eq!(
            physical.probabilities(0).unwrap(),
            [p("2/3"), p("0"), p("0"), p("1/3")]
        );
        let proposal = physical
            .scaled_proposal(&q(2, 1), &BTreeSet::new(), &options)
            .unwrap();
        assert_eq!(
            proposal.model().probabilities(0).unwrap(),
            [p("1/3"), p("0"), p("0"), p("2/3")]
        );
        // Zero channels remain addressable but have zero physical and proposal mass.
        for alternative in [0, 1] {
            let zero = [FaultSelection {
                site: 0,
                alternative,
            }];
            assert_eq!(physical.probability(&zero).unwrap(), q(0, 1));
            assert_eq!(proposal.model().probability(&zero).unwrap(), q(0, 1));
        }
        let sampler = NoiseSampler::configured(
            physical.clone(),
            &NoiseSampling::Importance {
                proposal_scale: q(2, 1),
            },
            &BTreeSet::new(),
            options,
        )
        .unwrap();
        assert_eq!(sampler.scaled_proposal_summary(), Some(proposal.summary()));
        let mut rng = SplitMix64::new(14);
        let mut seen = BTreeSet::new();
        for _ in 0..40 {
            let draw = sampler.draw(&mut rng).unwrap();
            let fault = !draw.faults.is_empty();
            let (mass, proposed, ratio) = if fault {
                assert_eq!(
                    draw.faults,
                    [FaultSelection {
                        site: 0,
                        alternative: 2
                    }]
                );
                let chosen = &category.channels[draw.faults[0].alternative];
                assert_eq!(chosen.source, original);
                assert_eq!(
                    chosen.action,
                    OriginalNoiseAction::Pauli {
                        factors: vec![(4, Basis::Z)]
                    }
                );
                (q(1, 3), q(2, 3), q(1, 2))
            } else {
                assert!(draw.faults.is_empty());
                (q(2, 3), q(1, 3), q(2, 1))
            };
            assert_eq!(draw.physical_probability, mass);
            assert_eq!(draw.proposal_probability, proposed);
            assert_eq!(draw.likelihood_ratio, ratio);
            assert_eq!(physical.probability(&draw.faults).unwrap(), mass);
            assert_eq!(
                proposal.model().probability(&draw.faults).unwrap(),
                proposed
            );
            assert_eq!(&draw.likelihood_ratio * &draw.proposal_probability, mass);
            seen.insert(fault);
        }
        assert_eq!(seen, BTreeSet::from([false, true]));
    }
}
