// SPDX-License-Identifier: MPL-2.0
//! Complete source-bound prepared noise/weight fixtures plus exact-law admission.
//! Adapted schedules: itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f;
//! see fixtures/prepared_noise_batches.NOTICE for source scope and Rust contracts.
use num_bigint::BigUint;
use num_rational::BigRational;
use num_traits::One;
use rmagna::noise::{
    NoiseError, NoiseOptions, NoiseWeightError, NoiseWeightLimits, PreparedNoiseError,
    PreparedNoiseOptions, PreparedNoiseSchedule, PreparedNoiseWeights, RandomSource,
    RawNoiseFactor, SparseNoiseAction, SparseNoiseOptions, SplitMix64,
};
use rmagna::probability::Probability;

fn f(n: u32, d: u32) -> RawNoiseFactor {
    RawNoiseFactor::new(n.into(), d.into()).unwrap()
}
#[test]
fn all_original_group_sizes_preserve_unreduced_products_and_the_513_fallback() {
    for count in [1, 2, 17, 308, 512, 513] {
        let mut factors = vec![(f(19, 23), f(29, 31)); count];
        factors.extend(vec![(f(19, 23), f(37, 41)); 9]);
        factors.extend(vec![(f(43, 47), f(53, 59)); 6]);
        let prepared = PreparedNoiseWeights::new(&factors, Default::default()).unwrap();
        assert_eq!(
            prepared.statistics().fallback_groups,
            usize::from(count > 512)
        );
        for seed in 0..9 {
            let selected = (0..factors.len())
                .map(|j| j.wrapping_mul(13).wrapping_add(seed) % 7 < 3)
                .collect::<Vec<_>>();
            let mut n = BigUint::one();
            let mut d = BigUint::one();
            for (j, (i, e)) in factors.iter().enumerate() {
                let pair = if selected[j] { e } else { i };
                n *= pair.numerator();
                d *= pair.denominator();
            }
            let value = prepared.evaluate(&selected).unwrap();
            assert_eq!((value.numerator(), value.denominator()), (&n, &d));
        }
    }
}
#[test]
fn raw_zero_and_unreduced_unit_factors_follow_the_original_skip_rule_after_input_replacement() {
    let pattern = [
        (f(17, 19), f(29, 31)),
        (f(37, 41), f(29, 31)),
        (f(1, 1), f(0, 1)),
        (f(5, 7), f(0, 1)),
    ];
    let mut factors = (0..316).map(|j| pattern[j % 4].clone()).collect::<Vec<_>>();
    factors[3] = (f(5, 7), f(3, 3));
    let plan = PreparedNoiseWeights::new(&factors, Default::default()).unwrap();
    let captured = factors.clone();
    factors.clear();
    for seed in 0..15 {
        let selected = (0..316).map(|j| (j + seed) % 3 != 0).collect::<Vec<_>>();
        let (mut n, mut d) = (BigUint::one(), BigUint::one());
        for (j, (i, e)) in captured.iter().enumerate() {
            let pair = if selected[j] { e } else { i };
            if pair.numerator() != pair.denominator() {
                n *= pair.numerator();
                d *= pair.denominator();
            }
        }
        let actual = plan.evaluate(&selected).unwrap();
        assert_eq!((actual.numerator(), actual.denominator()), (&n, &d));
    }
    let cancelling = PreparedNoiseWeights::new(
        &[(f(2, 3), f(5, 7)), (f(3, 2), f(7, 5))],
        Default::default(),
    )
    .unwrap();
    assert_eq!(cancelling.evaluate(&[false, false]).unwrap(), f(6, 6));
    assert_eq!(cancelling.evaluate(&[true, true]).unwrap(), f(35, 35));
    let unit = PreparedNoiseWeights::new(&[(f(3, 3), f(5, 5))], Default::default()).unwrap();
    assert_eq!(unit.evaluate(&[true]).unwrap(), f(1, 1));
    assert_eq!(
        plan.evaluate(&[]).unwrap_err(),
        NoiseWeightError::Shape {
            expected: 316,
            actual: 0
        }
    );
}
#[test]
fn integer_payload_limits_refuse_before_count_table_construction_and_allow_explicit_fallback() {
    assert_eq!(
        RawNoiseFactor::new(1u32.into(), 0u32.into()),
        Err(NoiseWeightError::DenominatorZero)
    );
    let factors = vec![(f(19, 23), f(29, 31)); 512];
    assert!(matches!(
        PreparedNoiseWeights::new(
            &factors,
            NoiseWeightLimits {
                max_table_integer_payload_bytes: 1,
                ..Default::default()
            }
        ),
        Err(NoiseWeightError::TableBytes { .. })
    ));
    assert!(matches!(
        PreparedNoiseWeights::new(
            &factors,
            NoiseWeightLimits {
                max_product_bits: 1,
                ..Default::default()
            }
        ),
        Err(NoiseWeightError::ProductBits { .. })
    ));
    let fallback = PreparedNoiseWeights::new(
        &factors,
        NoiseWeightLimits {
            max_table_integer_payload_bytes: 0,
            count_table_max_sites: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(fallback.statistics().fallback_groups, 1);
    assert_eq!(fallback.statistics().table_rows, 0);
    let table = PreparedNoiseWeights::new(&factors, Default::default()).unwrap();
    let selected = vec![true; 512];
    assert_eq!(
        table.evaluate(&selected).unwrap(),
        fallback.evaluate(&selected).unwrap()
    );
}

use rmagna::circuit::Basis;
fn p(n: u32, d: u32) -> Probability {
    Probability::from_ratio(n.into(), d.into()).unwrap()
}

#[test]
fn full_139_site_sparse_actions_weights_and_seed_partitions_use_real_prepared_groups() {
    let mut laws = (0..137)
        .map(|j| {
            if j % 2 == 1 {
                vec![p(1, 12), p(1, 6)]
            } else {
                vec![p(1, 4)]
            }
        })
        .collect::<Vec<_>>();
    laws.extend([vec![p(0, 1)], vec![p(1, 1)]]);
    let mut actions = laws
        .iter()
        .enumerate()
        .map(|(j, law)| {
            law.iter()
                .map(|_| {
                    if j % 3 == 0 {
                        SparseNoiseAction::ReportFlip {
                            record: format!("m{j}"),
                        }
                    } else {
                        SparseNoiseAction::Pauli {
                            operation: format!("a{j}"),
                            factors: vec![(j, Basis::Y), (0, Basis::X), (j, Basis::Z)],
                        }
                    }
                })
                .collect()
        })
        .collect::<Vec<Vec<_>>>();
    let factors = (0..139)
        .map(|j| {
            if j < 137 {
                (f(7, 5), f(2, 3))
            } else {
                (f(1, 1), f(1, 1))
            }
        })
        .collect::<Vec<_>>();
    let plan = PreparedNoiseSchedule::new(&laws, &actions, &factors, Default::default()).unwrap();
    assert_eq!(
        (
            plan.statistics().sites,
            plan.statistics().packed_groups,
            plan.statistics().endpoint_groups,
            plan.statistics().conditional_models
        ),
        (139, 1, 2, 1)
    );
    let plain = PreparedNoiseSchedule::new(
        &laws,
        &actions,
        &vec![(f(1, 1), f(1, 1)); 139],
        Default::default(),
    )
    .unwrap();
    let original = actions.clone();
    actions.clear();
    laws.clear();
    let seeds = (19..83).collect::<Vec<_>>();
    let full = plan.draw_seeded_many(&seeds).unwrap();
    let partition = plan
        .draw_seeded_many(&seeds[..23])
        .unwrap()
        .into_iter()
        .chain(plan.draw_seeded_many(&seeds[23..]).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(full, partition);
    let unweighted = plain.draw_seeded_many(&seeds).unwrap();
    let mut conditional_zero = 0u64;
    let mut conditional_total = 0u64;
    for (row, plain) in full.iter().zip(unweighted) {
        assert_eq!(row.choices, plain.choices);
        assert_eq!(row.entropy_words, plain.entropy_words);
        assert_eq!(row.choices[137..], [None, Some(0)]);
        let mut n = num_bigint::BigUint::one();
        let mut d = num_bigint::BigUint::one();
        let mut expected = vec![];
        let mut physical = BigRational::one();
        for (j, choice) in row.choices.iter().enumerate() {
            if j < 137 && j % 2 == 1 && choice.is_some() {
                conditional_total += 1;
                conditional_zero += u64::from(*choice == Some(0));
            }
            let pair = if choice.is_some() {
                &factors[j].1
            } else {
                &factors[j].0
            };
            n *= pair.numerator();
            d *= pair.denominator();
            if let Some(k) = choice {
                expected.push(original[j][*k].clone());
            }
            let term = if j >= 137 {
                BigRational::one()
            } else {
                match choice {
                    None => p(3, 4),
                    Some(0) if j % 2 == 1 => p(1, 12),
                    Some(1) => p(1, 6),
                    Some(0) => p(1, 4),
                    _ => panic!("alternative"),
                }
                .value()
                .clone()
            };
            physical *= term;
        }
        assert_eq!(row.actions, expected);
        assert_eq!(
            (row.raw_weight.numerator(), row.raw_weight.denominator()),
            (&n, &d)
        );
        assert_eq!(row.physical_probability, physical);
    }
    let delta = 3 * i128::from(conditional_zero) - i128::from(conditional_total);
    assert!(
        conditional_total > 0 && delta * delta < 36 * i128::from(conditional_total) * 2,
        "conditional alternative law must be 1/3,2/3"
    );
    println!(
        "192 real prepared draws over 139 original interleaved sites / 64 seeds: full, 23/41 partition, and unweighted choice replay; ordered sparse actions and raw products exact"
    );
}
#[test]
fn arbitrary_precision_occurrences_use_exact_fallback_and_empty_endpoints_consume_no_words() {
    let big = num_bigint::BigInt::from(10u32).pow(100);
    let tiny = Probability::from_ratio(1.into(), big.clone()).unwrap();
    let laws = vec![vec![], vec![p(0, 1)], vec![p(1, 1), p(0, 1)], vec![tiny]];
    let actions = laws
        .iter()
        .enumerate()
        .map(|(j, law)| {
            law.iter()
                .map(|_| SparseNoiseAction::ReportFlip {
                    record: format!("m{j}"),
                })
                .collect()
        })
        .collect::<Vec<_>>();
    let plan = PreparedNoiseSchedule::new(
        &laws,
        &actions,
        &vec![(f(1, 1), f(1, 1)); 4],
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.statistics().exact_tail_groups, 1);
    struct Zero(usize);
    impl RandomSource for Zero {
        fn next_u64(&mut self) -> u64 {
            self.0 += 1;
            0
        }
    }
    // Force the 64-bit occurrence prefix to zero, then the last legal
    // residual integer to select the positive tail. Finite Never alone would
    // lose this event; the residual has probability 2^64 / 10^100.
    struct Words {
        values: std::collections::VecDeque<u64>,
        used: usize,
    }
    impl RandomSource for Words {
        fn next_u64(&mut self) -> u64 {
            self.used += 1;
            self.values.pop_front().expect("no extra entropy")
        }
    }
    let residual = BigRational::new(num_bigint::BigInt::from(1u32) << 64usize, big.clone());
    let digits = (residual.denom().magnitude() - BigUint::one()).to_u64_digits();
    let expected_words = 64 + digits.len();
    let mut values = vec![0u64; 64];
    values.extend(digits);
    let mut rng = Words {
        values: values.into(),
        used: 0,
    };
    let row = plan.draw(&mut rng).unwrap();
    assert_eq!(row.choices, [None, None, Some(0), Some(0)]);
    assert_eq!(rng.used, expected_words);
    assert_eq!(row.entropy_words, rng.used);
    assert_eq!((row.occurrence_planes, row.exact_tail_evaluations), (64, 1));
    assert!(rng.values.is_empty());
    assert_eq!(row.physical_probability, BigRational::new(1.into(), big));
    let endpoint = PreparedNoiseSchedule::new(
        &laws[..3],
        &actions[..3],
        &vec![(f(1, 1), f(1, 1)); 3],
        PreparedNoiseOptions {
            sparse: SparseNoiseOptions {
                noise: NoiseOptions {
                    max_entropy_words: 0,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    let row = endpoint.draw(&mut Zero(0)).unwrap();
    assert_eq!(row.entropy_words, 0);
    assert_eq!(row.choices, [None, None, Some(0)]);
    let limited = PreparedNoiseSchedule::new(
        &[vec![p(1, 4)]],
        &[vec![SparseNoiseAction::ReportFlip { record: "r".into() }]],
        &[(f(1, 1), f(1, 1))],
        PreparedNoiseOptions {
            sparse: SparseNoiseOptions {
                noise: NoiseOptions {
                    max_entropy_words: 0,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(
        limited.draw(&mut Zero(0)),
        Err(PreparedNoiseError::Noise(NoiseError::EntropyLimit {
            limit: 0
        }))
    ));
}
#[test]
fn full_513_group_reuses_exact_occurrence_model_and_avoids_weight_count_table() {
    let laws = vec![vec![p(1, 100)]; 513];
    let actions = (0..513)
        .map(|j| {
            vec![SparseNoiseAction::ReportFlip {
                record: format!("m{j}"),
            }]
        })
        .collect::<Vec<_>>();
    let factors = vec![(f(101, 100), f(1, 2)); 513];
    let plan = PreparedNoiseSchedule::new(&laws, &actions, &factors, Default::default()).unwrap();
    assert_eq!(
        (
            plan.statistics().exact_tail_groups,
            plan.statistics().weights.fallback_groups,
            plan.statistics().weights.table_rows
        ),
        (1, 1, 0)
    );
    for seed in 0..8 {
        let row = plan.draw_seeded_many(&[seed]).unwrap().pop().unwrap();
        let k = row.choices.iter().filter(|x| x.is_some()).count();
        assert_eq!(
            row.raw_weight.numerator(),
            &num_bigint::BigUint::from(101u32).pow((513 - k) as u32)
        );
        assert_eq!(
            row.raw_weight.denominator(),
            &(num_bigint::BigUint::from(100u32).pow((513 - k) as u32)
                * num_bigint::BigUint::from(2u32).pow(k as u32))
        );
    }
    println!(
        "8 real 513-site draws: shared bit-plane occurrence with exact residual, no count table, exact integer-power raw weights"
    );
}
#[test]
fn full_arbitrary_category_fixture_retains_128_seeds_partitions_inputs_and_exact_declared_factors()
{
    let big = num_bigint::BigInt::from(10u32).pow(100);
    let mut laws = vec![
        vec![],
        vec![p(0, 1)],
        vec![p(1, 1), p(0, 3)],
        vec![p(1, 15000); 15],
    ];
    laws.push(vec![p(1, 10), p(1, 5)]);
    laws.push(vec![
        Probability::from_ratio(&big - 1, big.clone()).unwrap(),
        Probability::from_ratio(1.into(), big.clone()).unwrap(),
    ]);
    laws.push(vec![
        Probability::from_ratio(1.into(), big).unwrap(),
        p(1, 3),
    ]);
    let actions = laws
        .iter()
        .enumerate()
        .map(|(j, law)| {
            law.iter()
                .map(|_| SparseNoiseAction::ReportFlip {
                    record: format!("m{j}"),
                })
                .collect()
        })
        .collect::<Vec<_>>();
    let factors = vec![(f(2, 3), f(4, 7)); laws.len()];
    let plan = PreparedNoiseSchedule::new(&laws, &actions, &factors, Default::default()).unwrap();
    assert_eq!(
        laws.iter().map(Vec::len).collect::<Vec<_>>(),
        [0, 1, 2, 15, 2, 2, 2]
    );
    assert_eq!(plan.statistics().sites, 7);
    assert!(plan.statistics().exact_tail_groups >= 3);
    let seeds = (0..128).collect::<Vec<_>>();
    let actual = plan.draw_seeded_many(&seeds).unwrap();
    assert_eq!(
        actual,
        plan.draw_seeded_many(&seeds[..47])
            .unwrap()
            .into_iter()
            .chain(plan.draw_seeded_many(&seeds[47..]).unwrap())
            .collect::<Vec<_>>()
    );
    for (seed, row) in seeds.iter().zip(&actual) {
        assert_eq!(
            *row,
            plan.draw_seeded_many(&[*seed]).unwrap().pop().unwrap()
        );
        assert_eq!(row.choices[..3], [None, None, Some(0)]);
        let mut n = num_bigint::BigUint::one();
        let mut d = num_bigint::BigUint::one();
        for choice in &row.choices {
            if choice.is_some() {
                n *= 4u32;
                d *= 7u32;
            } else {
                n *= 2u32;
                d *= 3u32;
            }
        }
        assert_eq!(
            (row.raw_weight.numerator(), row.raw_weight.denominator()),
            (&n, &d)
        );
    }
    // The source's rare 15-way category must also execute observably: each
    // occurrence plane has lane0 clear, while exact uniform sampling sees14.
    // This forces its last alternative without depending on the 128 seed run.
    struct Fourteen;
    impl RandomSource for Fourteen {
        fn next_u64(&mut self) -> u64 {
            14
        }
    }
    let distinct = (0..15)
        .map(|j| SparseNoiseAction::ReportFlip {
            record: format!("alternative{j}"),
        })
        .collect::<Vec<_>>();
    let conditional = PreparedNoiseSchedule::new(
        std::slice::from_ref(&laws[3]),
        std::slice::from_ref(&distinct),
        &[(f(2, 3), f(4, 7))],
        Default::default(),
    )
    .unwrap();
    let forced = conditional.draw(&mut Fourteen).unwrap();
    assert_eq!(forced.choices, [Some(14)]);
    assert_eq!(forced.actions, [distinct[14].clone()]);
    assert_eq!(
        forced.physical_probability,
        BigRational::new(1.into(), 15000.into())
    );
    assert_eq!(forced.raw_weight, f(4, 7));
    laws[3].clear();
    assert_eq!(plan.draw_seeded_many(&seeds).unwrap(), actual);
    assert_eq!(conditional.draw(&mut Fourteen).unwrap(), forced);
    assert!(Probability::from_ratio(1.into(), 0.into()).is_err());
    assert!(Probability::from_ratio((-1).into(), 2.into()).is_err());
    assert!(Probability::from_ratio(1.into(), (-2).into()).is_err());
    let a = [vec![
        SparseNoiseAction::ReportFlip { record: "a".into() },
        SparseNoiseAction::ReportFlip { record: "b".into() },
    ]];
    assert!(matches!(
        PreparedNoiseSchedule::new(
            &[vec![p(3, 4), p(1, 2)]],
            &a,
            &[(f(1, 1), f(1, 1))],
            Default::default()
        ),
        Err(PreparedNoiseError::Category(0))
    ));
    println!(
        "512 real draws over the original seven-site / 100-digit category fixture: 15 alternatives at site3, full, 47/81 partition, singles and source replacement; two forced last-alternative draws prove its conditional law and owned capture"
    );
}
#[test]
fn all_original_group_sizes_use_actual_packed_and_scalar_choices_with_literal_product_oracles() {
    for count in [1, 2, 17, 308, 512, 513] {
        let mut laws = vec![vec![p(1, 7)]; count];
        laws.extend(vec![vec![p(1, 3), p(1, 4)]; 9]);
        laws.extend(vec![vec![p(0, 1)]; 3]);
        laws.extend(vec![vec![p(1, 1)]; 3]);
        let mut factors = vec![(f(19, 23), f(29, 31)); count];
        factors.extend(vec![(f(19, 23), f(37, 41)); 9]);
        factors.extend(vec![(f(43, 47), f(53, 59)); 6]);
        let actions = laws
            .iter()
            .enumerate()
            .map(|(j, law)| {
                law.iter()
                    .map(|_| SparseNoiseAction::ReportFlip {
                        record: format!("m{j}"),
                    })
                    .collect()
            })
            .collect::<Vec<_>>();
        let plan =
            PreparedNoiseSchedule::new(&laws, &actions, &factors, Default::default()).unwrap();
        assert_eq!(
            plan.statistics().weights.fallback_groups,
            usize::from(count > 512)
        );
        for seed in 0..9 {
            for row in [
                plan.draw_seeded_many(&[seed]).unwrap().pop().unwrap(),
                plan.draw_scalar_seeded(seed).unwrap(),
            ] {
                let mut n = num_bigint::BigUint::one();
                let mut d = num_bigint::BigUint::one();
                let mut physical = BigRational::one();
                for (j, choice) in row.choices.iter().enumerate() {
                    let pair = if choice.is_some() {
                        &factors[j].1
                    } else {
                        &factors[j].0
                    };
                    n *= pair.numerator();
                    d *= pair.denominator();
                    let value = match choice {
                        Some(k) => laws[j][*k].value().clone(),
                        None => {
                            BigRational::one()
                                - laws[j].iter().map(|p| p.value()).sum::<BigRational>()
                        }
                    };
                    physical *= value;
                }
                assert_eq!(
                    (row.raw_weight.numerator(), row.raw_weight.denominator()),
                    (&n, &d)
                );
                assert_eq!(row.physical_probability, physical);
                assert!(
                    row.choices[count + 9..count + 12]
                        .iter()
                        .all(Option::is_none)
                );
                assert!(row.choices[count + 12..].iter().all(|x| *x == Some(0)));
            }
        }
    }
    println!(
        "108 real packed/scalar draws: original group sizes 1/2/17/308/512/513, nine seeds, mixed alternatives and six endpoints; literal raw products and physical probabilities"
    );
}
#[test]
fn output_cells_and_complete_payload_are_admitted_before_entropy_or_batch_allocation() {
    let laws = vec![vec![p(1, 2)]; 4];
    let actions = vec![vec![SparseNoiseAction::ReportFlip { record: "r".into() }]; 4];
    let factors = vec![(f(2, 3), f(4, 7)); 4];
    let plan = PreparedNoiseSchedule::new(
        &laws,
        &actions,
        &factors,
        PreparedNoiseOptions {
            max_batch_cells: 3,
            ..Default::default()
        },
    )
    .unwrap();
    struct NoEntropy;
    impl RandomSource for NoEntropy {
        fn next_u64(&mut self) -> u64 {
            panic!("output admission precedes entropy")
        }
    }
    assert!(matches!(
        plan.draw(&mut NoEntropy),
        Err(PreparedNoiseError::RequestLimit {
            option: "max_batch_cells",
            ..
        })
    ));
    let normal = PreparedNoiseSchedule::new(&laws, &actions, &factors, Default::default()).unwrap();
    let one = usize::try_from(normal.draw_payload_bound()).unwrap();
    let bounded = PreparedNoiseSchedule::new(
        &laws,
        &actions,
        &factors,
        PreparedNoiseOptions {
            max_batch_payload_bytes: one,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(bounded.draw_seeded_many(&[1]).is_ok());
    assert!(matches!(
        bounded.draw_seeded_many(&[1, 2]),
        Err(PreparedNoiseError::RequestLimit {
            option: "max_batch_payload_bytes",
            ..
        })
    ));
    assert!(bounded.draw_seeded_many(&[]).unwrap().is_empty());
    assert!(matches!(
        PreparedNoiseSchedule::new(
            &laws,
            &actions,
            &factors,
            PreparedNoiseOptions {
                max_batch_payload_bytes: one - 1,
                ..Default::default()
            }
        ),
        Err(PreparedNoiseError::RequestLimit {
            option: "max_batch_payload_bytes",
            ..
        })
    ));
}
#[test]
fn original_316_site_grouped_factors_keep_actual_plain_choices_and_skip_unreduced_units() {
    let pattern = [
        vec![p(1, 17)],
        vec![p(2, 11), p(3, 11)],
        vec![p(0, 1)],
        vec![p(1, 1)],
    ];
    let laws = (0..316).map(|j| pattern[j % 4].clone()).collect::<Vec<_>>();
    let pattern_f = [
        (f(17, 19), f(29, 31)),
        (f(37, 41), f(29, 31)),
        (f(1, 1), f(0, 1)),
        (f(5, 7), f(0, 1)),
    ];
    let mut factors = (0..316)
        .map(|j| pattern_f[j % 4].clone())
        .collect::<Vec<_>>();
    factors[3] = (f(5, 7), f(3, 3));
    let actions = laws
        .iter()
        .enumerate()
        .map(|(j, law)| {
            law.iter()
                .map(|_| SparseNoiseAction::ReportFlip {
                    record: format!("m{j}"),
                })
                .collect()
        })
        .collect::<Vec<_>>();
    let weighted =
        PreparedNoiseSchedule::new(&laws, &actions, &factors, Default::default()).unwrap();
    let plain = PreparedNoiseSchedule::new(
        &laws,
        &actions,
        &vec![(f(1, 1), f(1, 1)); 316],
        Default::default(),
    )
    .unwrap();
    for seed in 0..15 {
        let a = plain.draw_seeded_many(&[seed]).unwrap().pop().unwrap();
        let b = weighted.draw_seeded_many(&[seed]).unwrap().pop().unwrap();
        assert_eq!(a.choices, b.choices);
        assert_eq!(a.entropy_words, b.entropy_words);
        assert_eq!(a.actions, b.actions);
        assert_eq!(a.physical_probability, b.physical_probability);
        let mut n = num_bigint::BigUint::one();
        let mut d = num_bigint::BigUint::one();
        for (j, choice) in a.choices.iter().enumerate() {
            let pair = if choice.is_some() {
                &factors[j].1
            } else {
                &factors[j].0
            };
            if pair.numerator() != pair.denominator() {
                n *= pair.numerator();
                d *= pair.denominator();
            }
        }
        assert_eq!(
            (b.raw_weight.numerator(), b.raw_weight.denominator()),
            (&n, &d)
        );
    }
    println!(
        "30 real weighted/plain 316-site draws over 15 seeds: choices, entropy and sparse actions identical; unreduced unit and zero factors preserved"
    );
}
#[test]
fn actual_prepared_group_driver_preserves_joint_laws_across_all_lane_and_block_boundaries() {
    let laws = vec![vec![p(1, 4)]; 131];
    let actions = (0..131)
        .map(|j| {
            vec![SparseNoiseAction::ReportFlip {
                record: format!("m{j}"),
            }]
        })
        .collect::<Vec<_>>();
    let plan = PreparedNoiseSchedule::new(
        &laws,
        &actions,
        &vec![(f(1, 1), f(1, 1)); 131],
        Default::default(),
    )
    .unwrap();
    let sites = [0, 1, 63, 64, 127, 130];
    let pairs = [(0, 1), (0, 63), (63, 64), (64, 130)];
    let mut marginal = [0u64; 6];
    let mut joint = [0u64; 4];
    let mut seeds = SplitMix64::new(928);
    for _ in 0..32 {
        let batch = (0..128).map(|_| seeds.next_u64()).collect::<Vec<_>>();
        for row in plan.draw_seeded_many(&batch).unwrap() {
            assert!(
                row.entropy_words <= 6,
                "three dyadic blocks use at most two planes each"
            );
            for (k, &j) in sites.iter().enumerate() {
                marginal[k] += u64::from(row.choices[j].is_some());
            }
            for (k, &(a, b)) in pairs.iter().enumerate() {
                joint[k] += u64::from(row.choices[a].is_some() && row.choices[b].is_some());
            }
        }
    }
    for (count, denom) in marginal
        .into_iter()
        .map(|n| (n, 4u64))
        .chain(joint.into_iter().map(|n| (n, 16u64)))
    {
        let delta = i128::from(denom) * i128::from(count) - 4096;
        assert!(delta * delta < 36 * 4096 * i128::from(denom - 1));
    }
    assert_eq!(plan.draw_scalar_seeded(928).unwrap().entropy_words, 131);
    assert_eq!(plan.statistics().packed_groups, 1);
    assert_eq!(plan.statistics().exact_tail_groups, 0);
    println!(
        "4096 real packed draws over 131 sites: marginal/joint six-sigma checks at 0/1/63/64/127/130; at most six planes per draw versus 131 scalar words"
    );
}

#[test]
fn exact_rational_tail_runs_only_at_the_cutoff_boundary_including_the_high_lane() {
    use std::collections::VecDeque;
    struct Words(VecDeque<u64>);
    impl RandomSource for Words {
        fn next_u64(&mut self) -> u64 {
            self.0.pop_front().expect("no extra random word")
        }
    }
    let cutoff = u64::MAX / 3; // floor(2^64/3), independently fixed integer boundary.
    let planes = |value: u64| {
        (0..64)
            .map(|j| {
                if value >> (63 - j) & 1 != 0 {
                    u64::MAX
                } else {
                    0
                }
            })
            .collect::<Vec<_>>()
    };
    let plan = PreparedNoiseSchedule::unweighted(
        &[vec![p(1, 3)]],
        &[vec![SparseNoiseAction::ReportFlip {
            record: "m0".into(),
        }]],
        Default::default(),
    )
    .unwrap();
    for (tail_word, choice) in [(0, None), (2, Some(0))] {
        let mut words = planes(cutoff);
        words.push(tail_word);
        let mut rng = Words(words.into());
        let row = plan.draw(&mut rng).unwrap();
        assert_eq!(row.choices, [choice]);
        assert_eq!(
            (
                row.occurrence_planes,
                row.exact_tail_evaluations,
                row.entropy_words
            ),
            (64, 1, 65)
        );
        assert!(rng.0.is_empty());
    }
    for (value, choice) in [(cutoff - 1, Some(0)), (cutoff + 1, None)] {
        let row = plan.draw(&mut Words(planes(value).into())).unwrap();
        assert_eq!(row.choices, [choice]);
        assert_eq!(row.exact_tail_evaluations, 0);
        assert!(row.occurrence_planes <= 64);
    }
    let denominator = num_bigint::BigInt::from(10u32).pow(100);
    let almost = Probability::from_ratio(&denominator - 1, denominator.clone()).unwrap();
    let near_one = PreparedNoiseSchedule::unweighted(
        &[vec![almost]],
        &[vec![SparseNoiseAction::ReportFlip {
            record: "near-one".into(),
        }]],
        Default::default(),
    )
    .unwrap();
    let residual = BigRational::new(
        &denominator - (num_bigint::BigInt::from(1u32) << 64usize),
        denominator.clone(),
    );
    let last = (residual.denom().magnitude() - BigUint::one()).to_u64_digits();
    for chosen in [false, true] {
        let mut words = vec![u64::MAX; 64];
        words.extend(if chosen {
            last.clone()
        } else {
            vec![0; last.len()]
        });
        let mut rng = Words(words.into());
        let row = near_one.draw(&mut rng).unwrap();
        assert_eq!(row.choices, [chosen.then_some(0)]);
        assert!(rng.0.is_empty());
        assert_eq!((row.occurrence_planes, row.exact_tail_evaluations), (64, 1));
        assert_eq!(
            row.physical_probability,
            BigRational::new(
                if chosen { &denominator - 1 } else { 1.into() },
                denominator.clone()
            )
        );
    }
    let actions = (0..64)
        .map(|j| {
            vec![SparseNoiseAction::ReportFlip {
                record: format!("m{j}"),
            }]
        })
        .collect::<Vec<_>>();
    let block =
        PreparedNoiseSchedule::unweighted(&vec![vec![p(1, 3)]; 64], &actions, Default::default())
            .unwrap();
    let mut words = planes(cutoff);
    words[63] ^= 1; // lane0: U=K-1, accepted without a residual.
    words[62] ^= 1u64 << 63; // lane63: U=K+2, rejected without a residual.
    for lane in 1..63 {
        words.push(if lane % 2 == 0 { 2 } else { 0 });
    }
    let mut rng = Words(words.into());
    let row = block.draw(&mut rng).unwrap();
    assert_eq!(
        (
            row.occurrence_planes,
            row.exact_tail_evaluations,
            row.entropy_words
        ),
        (64, 62, 126)
    );
    assert!(rng.0.is_empty());
    for (lane, choice) in row.choices.iter().enumerate() {
        assert_eq!(*choice, if lane % 2 == 0 { Some(0) } else { None });
    }
    assert_eq!(
        row.physical_probability,
        BigRational::new(
            num_bigint::BigInt::from(1u32) << 32usize,
            num_bigint::BigInt::from(3u32).pow(64)
        )
    );
    println!(
        "exact rational cutoff: six single-lane boundary cases and one 64-lane case; 62 real residual evaluations, high-lane rejection, exact 32/32 physical joint law"
    );
}

#[test]
fn numeric_model_factor_and_shape_limits_report_the_actual_site_and_setting() {
    let laws = [vec![p(1, 3), p(1, 5)]];
    let actions = [vec![
        SparseNoiseAction::ReportFlip { record: "a".into() },
        SparseNoiseAction::ReportFlip { record: "b".into() },
    ]];
    let factors = [(f(2, 3), f(4, 7))];
    let normal = PreparedNoiseSchedule::new(&laws, &actions, &factors, Default::default()).unwrap();
    let need = normal.statistics().conservative_model_payload_bytes;
    assert!(
        PreparedNoiseSchedule::new(
            &laws,
            &actions,
            &factors,
            PreparedNoiseOptions {
                max_model_payload_bytes: need,
                ..Default::default()
            }
        )
        .is_ok()
    );
    let error = PreparedNoiseSchedule::new(
        &laws,
        &actions,
        &factors,
        PreparedNoiseOptions {
            max_model_payload_bytes: need - 1,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(
        matches!(error,PreparedNoiseError::RequestLimit{option:"max_model_payload_bytes",required,..}if required==need as u128)
    );
    assert!(error.to_string().contains("max_model_payload_bytes"));
    assert!(matches!(
        PreparedNoiseSchedule::new(
            &laws,
            &actions,
            &factors,
            PreparedNoiseOptions {
                sparse: SparseNoiseOptions {
                    noise: NoiseOptions {
                        max_denominator_bits: 3,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            }
        ),
        Err(PreparedNoiseError::Noise(NoiseError::DenominatorLimit {
            limit: 3,
            required: 4
        }))
    ));
    assert!(matches!(
        PreparedNoiseSchedule::new(&laws, &[vec![]], &factors, Default::default()),
        Err(PreparedNoiseError::Shape {
            field: "actions",
            site: Some(0),
            expected: 2,
            actual: 0
        })
    ));
    for (n, d) in [(-1, 2), (1, -2), (-1, -2)] {
        assert_eq!(
            RawNoiseFactor::from_ratio(n.into(), d.into()),
            Err(NoiseWeightError::NegativeFactor)
        );
    }
    assert_eq!(
        RawNoiseFactor::from_ratio(1.into(), 0.into()),
        Err(NoiseWeightError::DenominatorZero)
    );
    let raw = RawNoiseFactor::from_ratio(3.into(), 3.into()).unwrap();
    assert_eq!(
        (raw.numerator(), raw.denominator()),
        (&3u32.into(), &3u32.into())
    );
    let error = PreparedNoiseWeights::new(
        &[(f(19, 23), f(29, 31))],
        NoiseWeightLimits {
            max_factor_bits: 4,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        NoiseWeightError::FactorBits {
            site: 0,
            factor: "identity",
            required: 5,
            limit: 4
        }
    ));
    assert!(error.to_string().contains("site 0 identity"));
    assert!(matches!(
        PreparedNoiseWeights::new(
            &[],
            NoiseWeightLimits {
                max_product_bits: 0,
                ..Default::default()
            }
        ),
        Err(NoiseWeightError::InvalidOptions {
            option: "max_product_bits"
        })
    ));
    println!(
        "model payload exact-bound/refusal, LCM 15 versus three-bit cap, per-site shape, signed factor and factor-bit diagnostics checked before draws"
    );
}

#[test]
fn returned_actions_and_parallel_seeded_batches_keep_one_immutable_owner() {
    use std::sync::Arc;
    let laws = (0..17)
        .map(|j| {
            if j % 2 == 0 {
                vec![p(1, 4)]
            } else {
                vec![p(1, 5), p(1, 10)]
            }
        })
        .collect::<Vec<_>>();
    let actions = laws
        .iter()
        .enumerate()
        .map(|(j, row)| {
            row.iter()
                .map(|_| SparseNoiseAction::Pauli {
                    operation: format!("a{j}"),
                    factors: vec![(j, Basis::Y), (0, Basis::X), (j, Basis::Z)],
                })
                .collect()
        })
        .collect::<Vec<_>>();
    let plan =
        Arc::new(PreparedNoiseSchedule::unweighted(&laws, &actions, Default::default()).unwrap());
    let seeds = (0..17).collect::<Vec<_>>();
    let expected = plan.draw_seeded_many(&seeds).unwrap();
    let mut changed = expected.clone();
    let row = changed.iter_mut().find(|r| !r.actions.is_empty()).unwrap();
    if let SparseNoiseAction::Pauli { operation, factors } = &mut row.actions[0] {
        operation.clear();
        factors.reverse();
    } else {
        panic!("Pauli fixture");
    }
    row.raw_weight = f(0, 1);
    row.choices.clear();
    assert_eq!(plan.draw_seeded_many(&seeds).unwrap(), expected);
    std::thread::scope(|scope| {
        let mut jobs = vec![];
        for _ in 0..8 {
            let plan = Arc::clone(&plan);
            let seeds = &seeds;
            jobs.push(scope.spawn(move || plan.draw_seeded_many(seeds).unwrap()));
        }
        for job in jobs {
            assert_eq!(job.join().unwrap(), expected);
        }
    });
    assert_eq!(
        (
            plan.statistics().occurrence_groups,
            plan.statistics().packed_groups,
            plan.statistics().exact_tail_groups,
            plan.statistics().conditional_models
        ),
        (2, 2, 1, 1)
    );
    let bounded = PreparedNoiseSchedule::unweighted(
        &laws,
        &actions,
        PreparedNoiseOptions {
            max_batch: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(
        bounded.draw_seeded_many(&seeds),
        Err(PreparedNoiseError::BatchLimit {
            required: 17,
            limit: 1
        })
    ));
    struct OneWord;
    impl RandomSource for OneWord {
        fn next_u64(&mut self) -> u64 {
            1
        }
    }
    let conditional = PreparedNoiseSchedule::unweighted(
        &[vec![p(1, 3), p(2, 3)], vec![p(2, 3), p(1, 3)]],
        &[
            vec![
                SparseNoiseAction::ReportFlip {
                    record: "a0".into(),
                },
                SparseNoiseAction::ReportFlip {
                    record: "a1".into(),
                },
            ],
            vec![
                SparseNoiseAction::ReportFlip {
                    record: "b0".into(),
                },
                SparseNoiseAction::ReportFlip {
                    record: "b1".into(),
                },
            ],
        ],
        Default::default(),
    )
    .unwrap();
    let row = conditional.draw(&mut OneWord).unwrap();
    assert_eq!(row.choices, [Some(1), Some(0)]);
    assert_eq!(
        row.actions,
        [
            SparseNoiseAction::ReportFlip {
                record: "a1".into()
            },
            SparseNoiseAction::ReportFlip {
                record: "b0".into()
            }
        ]
    );
    assert_eq!(
        row.physical_probability,
        BigRational::new(4.into(), 9.into())
    );
    assert_eq!(
        (
            row.occurrence_planes,
            row.exact_tail_evaluations,
            row.entropy_words
        ),
        (0, 0, 2)
    );
    assert_eq!(conditional.statistics().conditional_models, 2);
    println!(
        "one immutable 17-site owner: returned snapshot mutation isolated; 136 parallel seed draws matched, metadata and batch count admission preserved"
    );
}
