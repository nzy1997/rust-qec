// SPDX-License-Identifier: MPL-2.0
// All five NoiseLawCacheTests methods, itpyi/magna
// 56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_noise_law_cache.py.
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};
use rmagna::circuit::{Basis, BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::noise::{
    NoiseModel, NoiseOptions, NoiseSampler, OriginalNoiseAction as Action, OriginalNoiseOptions,
    RandomSource, SplitMix64, original_noise_categories,
};
use rmagna::probability::Probability;
use std::collections::BTreeSet;
fn r(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}
fn law(raw: &[BigRational]) -> Vec<BigRational> {
    let mut survival = BigRational::one();
    raw.iter()
        .map(|p| {
            let value = &survival * p;
            survival *= BigRational::one() - p;
            value
        })
        .collect()
}
fn pauli(factors: &[(usize, Basis)]) -> Action {
    // Typed Rust products canonically order commuting distinct-wire factors;
    // Native's textual factor order is not a private-payload ABI requirement.
    // Channel, source-location and temporal action order remain exact below.
    let mut factors = factors.to_vec();
    factors.sort_by_key(|(wire, _)| *wire);
    Action::Pauli { factors }
}
struct Row {
    law: Vec<BigRational>,
    guarded: bool,
    anchors: Vec<usize>,
    actions: Vec<Action>,
}
#[derive(Default)]
struct Case {
    text: String,
    rows: Vec<Row>,
    operations: usize,
}
impl Case {
    fn chain(&mut self, raw: &[BigRational], guarded: bool) {
        let first = self.operations;
        let bases = [Basis::X, Basis::Y, Basis::Z];
        for (j, p) in raw.iter().enumerate() {
            self.text.push_str(&format!(
                "{}({p}) {}{}1\n",
                if j == 0 {
                    "CORRELATED_ERROR"
                } else {
                    "ELSE_CORRELATED_ERROR"
                },
                if guarded { "rec[-1] " } else { "" },
                ["X", "Y", "Z"][j % 3]
            ));
        }
        self.rows.push(Row {
            law: law(raw),
            guarded,
            anchors: (first..first + raw.len()).collect(),
            actions: (0..raw.len())
                .map(|j| pauli(&[(1, bases[j % 3])]))
                .collect(),
        });
        self.operations += raw.len();
    }
    fn exclusive(&mut self, raw: &[BigRational], guarded: bool) {
        let values = raw
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        self.text.push_str(&format!(
            "PAULI_CHANNEL_1({values}) {}1\n",
            if guarded { "rec[-1] " } else { "" }
        ));
        self.rows.push(Row {
            law: raw.to_vec(),
            guarded,
            anchors: vec![self.operations; 3],
            actions: [Basis::X, Basis::Y, Basis::Z]
                .into_iter()
                .map(|b| pauli(&[(1, b)]))
                .collect(),
        });
        self.operations += 1;
    }
    fn readouts(&mut self, wires: &[usize], probability: BigRational, first_record: usize) {
        self.text.push_str(&format!(
            "M({probability}) {}\n",
            wires
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        ));
        for j in 0..wires.len() {
            self.rows.push(Row {
                law: vec![probability.clone()],
                guarded: false,
                anchors: vec![self.operations],
                actions: vec![Action::ReportFlip {
                    record: first_record + j,
                }],
            });
            self.operations += 1;
        }
    }
    fn special_chain(&mut self, first: &[(usize, Basis)], second: &[(usize, Basis)]) {
        let text = |f: &[(usize, Basis)]| {
            f.iter()
                .map(|(q, b)| {
                    format!(
                        "{}{q}",
                        match b {
                            Basis::X => "X",
                            Basis::Y => "Y",
                            Basis::Z => "Z",
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join(" ")
        };
        self.text.push_str(&format!(
            "CORRELATED_ERROR(1/3) {}\nELSE_CORRELATED_ERROR(1/2) {}\n",
            text(first),
            text(second)
        ));
        self.rows.push(Row {
            law: vec![r(1, 3), r(1, 3)],
            guarded: false,
            anchors: vec![self.operations, self.operations + 1],
            actions: vec![pauli(first), pauli(second)],
        });
        self.operations += 2;
    }
}
fn complete(errors: &[BigRational]) -> Vec<Probability> {
    std::iter::once(BigRational::one() - errors.iter().cloned().sum::<BigRational>())
        .chain(errors.iter().cloned())
        .map(|p| Probability::from_rational(p).unwrap())
        .collect()
}
fn model(rows: Vec<Vec<Probability>>) -> NoiseModel {
    NoiseModel::new(rows, &NoiseOptions::default()).unwrap()
}
fn check(case: &Case, scale: Option<BigRational>) {
    let source = parse_circuit(&case.text, &ParseOptions::default()).unwrap();
    let source_before = source.to_text().to_string();
    let operations = source.operations().to_vec();
    let categories = original_noise_categories(&source, &OriginalNoiseOptions::default()).unwrap();
    let metadata = categories.clone();
    assert_eq!(categories.len(), case.rows.len());
    let mut guards = BTreeSet::new();
    let mut expected_proposed = vec![];
    let mut actual_rows = vec![];
    let mut physical_rows = vec![];
    let mut tilted = 0;
    let mut endpoints = 0;
    let mut channels = 0;
    for (site, (category, row)) in categories.iter().zip(&case.rows).enumerate() {
        assert_eq!(category.internal_site, site);
        assert_eq!(
            category.guard,
            row.guarded.then(|| BitExpr::atom(BitAtom::Record(0)))
        );
        if row.guarded {
            guards.insert(site);
        }
        assert_eq!(
            category
                .channels
                .iter()
                .map(|c| c.probability.value().clone())
                .collect::<Vec<_>>(),
            row.law
        );
        let mass = row.law.iter().cloned().sum::<BigRational>();
        assert_eq!(
            category.identity_probability.value(),
            &(BigRational::one() - &mass)
        );
        assert_eq!(
            category
                .channels
                .iter()
                .map(|c| c.action.clone())
                .collect::<Vec<_>>(),
            row.actions
        );
        assert_eq!(
            category
                .channels
                .iter()
                .map(|c| c.source.operation)
                .collect::<Vec<_>>(),
            row.anchors
        );
        let anchors = row.anchors.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(
            category
                .source_operations
                .iter()
                .map(|c| c.operation)
                .collect::<BTreeSet<_>>(),
            anchors
        );
        for channel in &category.channels {
            assert_eq!(
                channel.source.location,
                source.operations()[channel.source.operation].location
            );
        }
        channels += row.law.len();
        let endpoint = mass.is_zero() || mass.is_one();
        endpoints += usize::from(endpoint);
        let proposal = if let Some(scale) = &scale {
            if !row.guarded && !endpoint {
                tilted += usize::from(!scale.is_one());
                row.law.iter().map(|p| p * scale).collect::<Vec<_>>()
            } else {
                row.law.clone()
            }
        } else {
            row.law.clone()
        };
        expected_proposed.push(complete(&proposal));
        physical_rows.push(complete(&row.law));
        actual_rows.push(
            std::iter::once(category.identity_probability.clone())
                .chain(category.channels.iter().map(|c| c.probability.clone()))
                .collect(),
        );
    }
    assert_eq!(
        channels,
        categories.iter().map(|c| c.channels.len()).sum::<usize>()
    );
    let actual_physical = model(actual_rows);
    let reference_physical = model(physical_rows.clone());
    let sampler = if let Some(scale) = &scale {
        let proposal = actual_physical
            .scaled_proposal(scale, &guards, &NoiseOptions::default())
            .unwrap();
        let summary = proposal.summary();
        assert_eq!(summary.proposal_scale, *scale);
        assert_eq!(summary.categories, case.rows.len());
        assert_eq!(summary.excluded_categories, guards.len());
        assert_eq!(summary.endpoint_categories, endpoints);
        assert_eq!(summary.scaled_categories, tilted);
        for (j, row) in expected_proposed.iter().enumerate() {
            assert_eq!(proposal.model().probabilities(j).unwrap(), row);
        }
        NoiseSampler::importance(
            actual_physical,
            proposal.into_model(),
            NoiseOptions::default(),
        )
        .unwrap()
    } else {
        NoiseSampler::physical(actual_physical, NoiseOptions::default())
    };
    let reference = if scale.is_some() {
        NoiseSampler::importance(
            reference_physical,
            model(expected_proposed.clone()),
            NoiseOptions::default(),
        )
        .unwrap()
    } else {
        NoiseSampler::physical(reference_physical, NoiseOptions::default())
    };
    let inspect = |draw: &rmagna::noise::NoiseDraw| {
        let mut physical = BigRational::one();
        let mut proposal = BigRational::one();
        let mut last = None;
        for fault in &draw.faults {
            assert!(last.is_none_or(|x| x < fault.site));
            last = Some(fault.site);
            assert!(fault.site < case.rows.len());
            assert!(fault.alternative < case.rows[fault.site].law.len());
        }
        for (site, (p, q)) in physical_rows.iter().zip(&expected_proposed).enumerate() {
            let choice = draw
                .faults
                .iter()
                .find(|f| f.site == site)
                .map_or(0, |f| f.alternative + 1);
            physical *= p[choice].value();
            proposal *= q[choice].value();
        }
        assert_eq!(draw.physical_probability, physical);
        assert_eq!(draw.proposal_probability, proposal);
        assert_eq!(draw.likelihood_ratio, &physical / &proposal);
    };
    let mut seed_rng = SplitMix64::new(90210);
    let mut seeds = vec![0, 1, u64::MAX];
    seeds.extend((0..61).map(|_| seed_rng.next_u64()));
    assert_eq!(seeds.len(), 64);
    let draws = |sampler: &NoiseSampler, seeds: &[u64]| {
        seeds
            .iter()
            .map(|&seed| {
                let mut rng = SplitMix64::new(seed);
                let draw = sampler.draw(&mut rng).unwrap();
                inspect(&draw);
                (draw, rng.state())
            })
            .collect::<Vec<_>>()
    };
    let actual = draws(&sampler, &seeds);
    assert_eq!(actual, draws(&reference, &seeds));
    let mut split = draws(&sampler, &seeds[..19]);
    split.extend(draws(&sampler, &seeds[19..]));
    assert_eq!(actual, split);
    let mut rng = SplitMix64::new(85);
    let mut all = vec![];
    for _ in 0..37 {
        let draw = sampler.draw(&mut rng).unwrap();
        inspect(&draw);
        all.push(draw);
    }
    let final_state = rng.state();
    let mut rng = SplitMix64::new(85);
    let mut resumed = vec![];
    for _ in 0..19 {
        resumed.push(sampler.draw(&mut rng).unwrap());
    }
    let checkpoint = rng.state();
    let mut restored = SplitMix64::new(checkpoint);
    for _ in 0..18 {
        resumed.push(sampler.draw(&mut restored).unwrap());
    }
    assert_eq!(resumed, all);
    assert_eq!(restored.state(), final_state);
    assert_eq!(
        original_noise_categories(&source, &OriginalNoiseOptions::default()).unwrap(),
        metadata
    );
    assert_eq!(source.to_text(), source_before);
    assert_eq!(source.operations(), operations);
}
#[test]
fn original_chain_and_exclusive_laws_remain_distinct_on_repeated_reuse() {
    let raw = vec![r(1, 10), r(2, 9), r(0, 1)];
    for first in [false, true] {
        let mut case = Case::default();
        for j in 0..6 {
            if (j % 2 == 0) == first {
                case.chain(&raw, false)
            } else {
                case.exclusive(&raw, false)
            }
        }
        for scale in [None, Some(r(2, 1))] {
            check(&case, scale);
        }
    }
}
#[test]
fn original_guarded_laws_remain_physical_under_importance_scaling() {
    let raw = vec![r(1, 10), r(2, 9), r(0, 1)];
    let mut case = Case::default();
    case.readouts(&[0], r(1, 10), 0);
    case.chain(&raw, true);
    case.chain(&raw, false);
    case.exclusive(&raw, false);
    case.exclusive(&raw, true);
    case.chain(&raw, true);
    case.chain(&raw, false);
    assert_eq!(case.rows.len(), 7);
    for scale in [None, Some(r(1, 2)), Some(r(2, 1))] {
        check(&case, scale);
    }
}
#[test]
fn original_long_chains_retain_large_rationals_endpoints_and_unreachable_tails() {
    let denominator: BigInt = (BigInt::one() << 80usize) + 7;
    let chains = vec![
        vec![r(0, 1); 3],
        vec![r(0, 1), r(1, 1), r(1, 3), r(0, 1), r(1, 1)],
        vec![r(1, 1), r(0, 1), r(1, 1)],
        vec![
            BigRational::new(1.into(), denominator.clone()),
            BigRational::new((BigInt::one() << 79) + 9, denominator),
            r(1, 3),
            r(0, 1),
        ],
        (0..65).map(|j| r(1, 128 - j)).collect(),
    ];
    let mut case = Case::default();
    for _ in 0..2 {
        for raw in &chains {
            case.chain(raw, false);
        }
    }
    assert_eq!(case.rows.len(), 10);
    assert_eq!(case.rows[4].law.len(), 65);
    for scale in [None, Some(r(1, 2))] {
        check(&case, scale);
    }
}
#[test]
fn original_more_than_32_laws_keep_39_location_order_and_earlier_reuse() {
    let unique = (200..235)
        .map(|n| vec![r(1, n), r(1, n + 1)])
        .collect::<Vec<_>>();
    let mut case = Case::default();
    case.chain(&unique[0], false);
    for raw in &unique {
        case.chain(raw, false);
    }
    for index in [0, 34, 0] {
        case.chain(&unique[index], false);
    }
    assert_eq!(unique.len(), 35);
    assert_eq!(case.rows.len(), 39);
    check(&case, Some(r(3, 1)));
}
#[test]
fn original_repeated_laws_keep_ordered_pauli_factors_and_separate_report_records() {
    let mut case = Case::default();
    case.special_chain(&[(2, Basis::X), (0, Basis::Y)], &[(1, Basis::Z)]);
    case.readouts(&[0, 2], r(1, 4), 0);
    case.special_chain(
        &[(1, Basis::Y), (2, Basis::X)],
        &[(0, Basis::X), (2, Basis::Z)],
    );
    case.readouts(&[1], r(1, 4), 2);
    assert_eq!(case.rows.len(), 5);
    assert_eq!(case.operations, 7);
    for scale in [None, Some(r(1, 2))] {
        check(&case, scale);
    }
}
