use rmagna::circuit::{BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::gf2::BitVector;
use rmagna::instrument::{CompileOptions, FaultSelection, compile_affine};
use rmagna::noise::{RandomSource, SplitMix64};
use rmagna::records::{SamplingError, SamplingOptions, bernoulli_ratio, sample_reported};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
struct Words {
    values: Vec<u64>,
    at: usize,
}
impl Words {
    fn new(values: Vec<u64>) -> Self {
        Self { values, at: 0 }
    }
}
impl RandomSource for Words {
    fn next_u64(&mut self) -> u64 {
        let value = self.values[self.at];
        self.at += 1;
        value
    }
}
#[test]
fn exact_binary_intervals_handle_rational_irrational_and_tiny_probabilities() {
    let a = ScalarArithmetic::default();
    let one = ExactScalar::one();
    let half = a.omega(0, -2).unwrap();
    for word in [0, (1_u64 << 63) - 1, 1_u64 << 63, u64::MAX] {
        let draw = bernoulli_ratio(&half, &one, &mut Words::new(vec![word]), 1, &a).unwrap();
        assert_eq!(draw.selected, word < 1_u64 << 63);
        assert_eq!(draw.entropy_words, 1);
    }
    let root_half = a
        .scalar([0.into(), 1.into(), 0.into(), (-1).into()], -1)
        .unwrap();
    for (word, expected) in [
        (0, true),
        (1_u64 << 63, true),
        (3_u64 << 62, false),
        (u64::MAX, false),
    ] {
        assert_eq!(
            bernoulli_ratio(&root_half, &one, &mut Words::new(vec![word]), 1, &a)
                .unwrap()
                .selected,
            expected
        );
    }
    let three = a
        .scalar([3.into(), 0.into(), 0.into(), 0.into()], 0)
        .unwrap();
    let ambiguous = 0x5555555555555555;
    assert_eq!(
        bernoulli_ratio(&one, &three, &mut Words::new(vec![ambiguous]), 1, &a),
        Err(SamplingError::EntropyLimit { limit: 1 })
    );
    let draw = bernoulli_ratio(&one, &three, &mut Words::new(vec![ambiguous, 0]), 2, &a).unwrap();
    assert!(draw.selected);
    assert_eq!(draw.entropy_words, 2);
    let tiny = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1200)
        .unwrap();
    let draw = bernoulli_ratio(&tiny, &one, &mut Words::new(vec![0; 19]), 19, &a).unwrap();
    assert!(draw.selected);
    assert_eq!(draw.entropy_words, 19);
}
#[test]
fn invalid_masses_fail_and_deterministic_endpoints_consume_no_words() {
    let a = ScalarArithmetic::default();
    let one = ExactScalar::one();
    let zero = ExactScalar::zero();
    for (numerator, total) in [
        (&one, &zero),
        (&one, &one.negated()),
        (&one.negated(), &one),
        (&one, &a.omega(0, -2).unwrap()),
    ] {
        assert_eq!(
            bernoulli_ratio(numerator, total, &mut Words::new(vec![]), 0, &a),
            Err(SamplingError::InvalidMass)
        );
    }
    assert!(matches!(
        bernoulli_ratio(
            &a.omega(2, 0).unwrap(),
            &one,
            &mut Words::new(vec![]),
            0,
            &a
        ),
        Err(SamplingError::Scalar(_))
    ));
    for (numerator, expected) in [(&zero, false), (&one, true)] {
        let draw = bernoulli_ratio(numerator, &one, &mut Words::new(vec![]), 0, &a).unwrap();
        assert_eq!(draw.selected, expected);
        assert_eq!(draw.entropy_words, 0);
    }
}
#[test]
fn physical_prefix_sampling_preserves_correlations_report_flips_and_redundant_queries() {
    let a = ScalarArithmetic::default();
    let source = parse_circuit(
        "H 0\nCX 0 1\nM 0\nM(1/3) 1\nDETECTOR rec[-1]\nPOSTSELECT det[0]",
        &ParseOptions::default(),
    )
    .unwrap();
    let protocol = compile_affine(&source, &CompileOptions::default(), &a).unwrap();
    let r0 = BitExpr::atom(BitAtom::Record(0));
    let r1 = BitExpr::atom(BitAtom::Record(1));
    let mut xor = r0.clone();
    xor.xor_assign(&r1);
    let mut inverted = r0.clone();
    inverted.constant = true;
    let parities = [r0.clone(), r1, xor, r0, inverted];
    for flipped in [false, true] {
        let faults = if flipped {
            vec![FaultSelection {
                site: 1,
                alternative: 0,
            }]
        } else {
            vec![]
        };
        for (word, bit) in [(0, false), (u64::MAX, true)] {
            let mut random = Words::new(vec![word]);
            let draw = sample_reported(
                &protocol,
                &parities,
                &faults,
                &mut random,
                &SamplingOptions::default(),
                &a,
            )
            .unwrap();
            assert_eq!(draw.values, [bit, bit ^ flipped, flipped, bit, !bit]);
            assert_eq!(draw.mass, a.omega(0, -2).unwrap());
            assert_eq!(draw.entropy_words, 1);
            let values = BitVector::from_indices(
                parities.len(),
                draw.values
                    .iter()
                    .enumerate()
                    .filter_map(|(j, &v)| v.then_some(j)),
            )
            .unwrap();
            assert_eq!(
                draw.mass,
                protocol
                    .reported_probability(&parities, &values, &faults, 256, &a)
                    .unwrap()
                    .value
            );
        }
    }
}
#[test]
fn sampling_uses_physical_marginals_even_when_postselection_rejects_the_sample() {
    let a = ScalarArithmetic::default();
    let source = parse_circuit(
        "H 0\nT 0\nH 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]",
        &ParseOptions::default(),
    )
    .unwrap();
    let protocol = compile_affine(&source, &CompileOptions::default(), &a).unwrap();
    let parity = BitExpr::atom(BitAtom::Record(0));
    let draw = sample_reported(
        &protocol,
        &[parity],
        &[],
        &mut Words::new(vec![u64::MAX]),
        &SamplingOptions::default(),
        &a,
    )
    .unwrap();
    assert_eq!(draw.values, [true]);
    assert_eq!(
        draw.mass,
        a.scalar([2.into(), (-1).into(), 0.into(), 1.into()], -2)
            .unwrap()
    );
    assert_eq!(
        a.add(
            &draw.mass,
            &protocol.acceptance(&[], 256, &a).unwrap().value
        )
        .unwrap(),
        ExactScalar::one()
    );
}
#[test]
fn seeded_records_resume_and_invalid_queries_fail_before_entropy_is_read() {
    let a = ScalarArithmetic::default();
    let source = parse_circuit("H 0 1\nM 0 1", &ParseOptions::default()).unwrap();
    let protocol = compile_affine(&source, &CompileOptions::default(), &a).unwrap();
    let parities = [
        BitExpr::atom(BitAtom::Record(0)),
        BitExpr::atom(BitAtom::Record(1)),
    ];
    let mut first = SplitMix64::new(42);
    let mut second = first;
    let expected = (0..10)
        .map(|_| {
            sample_reported(
                &protocol,
                &parities,
                &[],
                &mut first,
                &SamplingOptions::default(),
                &a,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mut actual = Vec::new();
    for _ in 0..10 {
        actual.push(
            sample_reported(
                &protocol,
                &parities,
                &[],
                &mut second,
                &SamplingOptions::default(),
                &a,
            )
            .unwrap(),
        );
        second = SplitMix64::new(second.state());
    }
    assert_eq!(expected, actual);
    assert_eq!(first.state(), second.state());
    let mut unused = Words::new(vec![]);
    assert!(
        sample_reported(
            &protocol,
            &[BitExpr::atom(BitAtom::Record(2))],
            &[],
            &mut unused,
            &SamplingOptions::default(),
            &a
        )
        .is_err()
    );
    assert_eq!(unused.at, 0);
    let zero_budget = SamplingOptions {
        max_entropy_words: 1,
        ..Default::default()
    };
    assert_eq!(
        sample_reported(
            &protocol,
            &parities,
            &[],
            &mut Words::new(vec![0]),
            &zero_budget,
            &a
        ),
        Err(SamplingError::EntropyLimit { limit: 1 })
    );
}

#[test]
fn conditioned_sampling_keeps_prior_inputs_and_rejects_impossible_histories() {
    use rmagna::records::{RecordQuery, sample_reported_conditioned};
    let a = ScalarArithmetic::default();
    let source = parse_circuit("H 0\nCX 0 1\nM 0 1", &ParseOptions::default()).unwrap();
    let protocol = compile_affine(&source, &CompileOptions::default(), &a).unwrap();
    let r0 = BitExpr::atom(BitAtom::Record(0));
    let r1 = BitExpr::atom(BitAtom::Record(1));
    for value in [false, true] {
        let given = [(r0.clone(), value)];
        let parities = [r1.clone()];
        let draw = sample_reported_conditioned(
            &protocol,
            &RecordQuery {
                given: &given,
                parities: &parities,
            },
            &[],
            &mut Words::new(vec![]),
            &SamplingOptions {
                max_entropy_words: 0,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
        assert_eq!(draw.values, [value]);
        assert_eq!(draw.condition_mass, a.omega(0, -2).unwrap());
        assert_eq!(draw.mass, draw.condition_mass);
        assert_eq!(draw.entropy_words, 0);
    }
    let given = [(r0, false), (r1, true)];
    assert_eq!(
        sample_reported_conditioned(
            &protocol,
            &RecordQuery {
                given: &given,
                parities: &[]
            },
            &[],
            &mut Words::new(vec![]),
            &SamplingOptions::default(),
            &a
        ),
        Err(SamplingError::ImpossibleCondition)
    );
}

#[test]
fn affine_group_proposals_preserve_report_correlations_and_ignore_selection() {
    use rmagna::records::{RecordQuery, propose_affine_reported};
    let a = ScalarArithmetic::default();
    let source = parse_circuit(
        "H 0\nCX 0 1\nM 0\nM(1/3) 1\nDETECTOR rec[-1]\nPOSTSELECT det[0]",
        &ParseOptions::default(),
    )
    .unwrap();
    let protocol = compile_affine(&source, &CompileOptions::default(), &a).unwrap();
    let r0 = BitExpr::atom(BitAtom::Record(0));
    let r1 = BitExpr::atom(BitAtom::Record(1));
    let mut xor = r0.clone();
    xor.xor_assign(&r1);
    let mut inverted = r0.clone();
    inverted.constant = true;
    let parities = [r0.clone(), r1, xor, r0.clone(), inverted];
    for flipped in [false, true] {
        let faults = if flipped {
            vec![FaultSelection {
                site: 1,
                alternative: 0,
            }]
        } else {
            vec![]
        };
        for (word, bit) in [(0, false), (1, true)] {
            let mut random = Words::new(vec![word]);
            let draw = propose_affine_reported(
                &protocol,
                &RecordQuery {
                    given: &[],
                    parities: &parities,
                },
                &faults,
                &mut random,
                &SamplingOptions {
                    max_terms: 0,
                    ..Default::default()
                },
                &a,
            )
            .unwrap();
            assert_eq!(draw.values, [bit, bit ^ flipped, flipped, bit, !bit]);
            assert_eq!(draw.proposal_mass, a.omega(0, -2).unwrap());
            assert_eq!(draw.entropy_words, 1);
            assert_eq!(random.at, 1);
            let given = [(r0.clone(), bit)];
            let conditional = propose_affine_reported(
                &protocol,
                &RecordQuery {
                    given: &given,
                    parities: &parities,
                },
                &faults,
                &mut Words::new(vec![]),
                &SamplingOptions::default(),
                &a,
            )
            .unwrap();
            assert_eq!(conditional.values, draw.values);
            assert_eq!(conditional.proposal_mass, ExactScalar::one());
            assert_eq!(conditional.entropy_words, 0);
        }
    }
    let impossible = [(r0.clone(), false), (r0, true)];
    assert!(matches!(
        propose_affine_reported(
            &protocol,
            &RecordQuery {
                given: &impossible,
                parities: &parities
            },
            &[],
            &mut Words::new(vec![]),
            &SamplingOptions::default(),
            &a
        ),
        Err(SamplingError::ImpossibleSupport)
    ));
}

#[test]
fn affine_proposals_reweight_irrational_histories_and_retain_interference_zeros() {
    use rmagna::records::{RecordQuery, propose_affine_reported};
    let a = ScalarArithmetic::default();
    let parity = [BitExpr::atom(BitAtom::Record(0))];
    for cancel in [false, true] {
        let text = if cancel {
            "H 0\nT 0\nT_DAG 0\nH 0\nM 0"
        } else {
            "H 0\nT 0\nH 0\nM 0"
        };
        let source = parse_circuit(text, &ParseOptions::default()).unwrap();
        let protocol = compile_affine(&source, &CompileOptions::default(), &a).unwrap();
        let mut weighted_mean = ExactScalar::zero();
        let mut weights = ExactScalar::zero();
        for word in 0..2 {
            let draw = propose_affine_reported(
                &protocol,
                &RecordQuery {
                    given: &[],
                    parities: &parity,
                },
                &[],
                &mut Words::new(vec![word]),
                &SamplingOptions {
                    max_terms: 0,
                    ..Default::default()
                },
                &a,
            )
            .unwrap();
            assert_eq!(draw.values, [word != 0]);
            assert_eq!(draw.proposal_mass, a.omega(0, -2).unwrap());
            let outcome = BitVector::from_indices(1, (word != 0).then_some(0)).unwrap();
            let physical = protocol
                .reported_probability(&parity, &outcome, &[], 256, &a)
                .unwrap()
                .value;
            let expected = if cancel {
                if word == 0 {
                    ExactScalar::one()
                } else {
                    ExactScalar::zero()
                }
            } else if word == 0 {
                a.scalar([2.into(), 1.into(), 0.into(), (-1).into()], -2)
                    .unwrap()
            } else {
                a.scalar([2.into(), (-1).into(), 0.into(), 1.into()], -2)
                    .unwrap()
            };
            assert_eq!(physical, expected);
            let weight = a.multiply(&physical, &a.omega(0, 2).unwrap()).unwrap();
            weights = a
                .add(&weights, &a.multiply(&draw.proposal_mass, &weight).unwrap())
                .unwrap();
            if draw.values[0] {
                weighted_mean = a
                    .add(
                        &weighted_mean,
                        &a.multiply(&draw.proposal_mass, &weight).unwrap(),
                    )
                    .unwrap();
            }
        }
        assert_eq!(weights, ExactScalar::one());
        assert_eq!(
            weighted_mean,
            if cancel {
                ExactScalar::zero()
            } else {
                a.scalar([2.into(), (-1).into(), 0.into(), 1.into()], -2)
                    .unwrap()
            }
        );
    }
}

#[test]
fn affine_proposals_pack_wide_entropy_and_validate_before_drawing() {
    use rmagna::records::{RecordQuery, propose_affine_reported};
    let a = ScalarArithmetic::default();
    let targets = (0..65).map(|j| j.to_string()).collect::<Vec<_>>().join(" ");
    let source = parse_circuit(
        &format!("H {targets}\nM {targets}"),
        &ParseOptions::default(),
    )
    .unwrap();
    let protocol = compile_affine(&source, &CompileOptions::default(), &a).unwrap();
    let parities = (0..65)
        .map(|j| BitExpr::atom(BitAtom::Record(j)))
        .collect::<Vec<_>>();
    let query = RecordQuery {
        given: &[],
        parities: &parities,
    };
    let draw = propose_affine_reported(
        &protocol,
        &query,
        &[],
        &mut Words::new(vec![1, 1]),
        &SamplingOptions::default(),
        &a,
    )
    .unwrap();
    assert_eq!(
        draw.values
            .iter()
            .enumerate()
            .filter_map(|(j, &v)| v.then_some(j))
            .collect::<Vec<_>>(),
        [0, 64]
    );
    assert_eq!(draw.proposal_mass, a.omega(0, -130).unwrap());
    assert_eq!(draw.entropy_words, 2);
    assert!(matches!(
        propose_affine_reported(
            &protocol,
            &query,
            &[],
            &mut Words::new(vec![]),
            &SamplingOptions {
                max_entropy_words: 1,
                ..Default::default()
            },
            &a
        ),
        Err(SamplingError::EntropyLimit { limit: 1 })
    ));
    for invalid in [
        BitExpr::atom(BitAtom::Record(65)),
        BitExpr::atom(BitAtom::Decision { call: 0, bit: 0 }),
    ] {
        assert!(
            propose_affine_reported(
                &protocol,
                &RecordQuery {
                    given: &[],
                    parities: &[invalid]
                },
                &[],
                &mut Words::new(vec![]),
                &SamplingOptions::default(),
                &a
            )
            .is_err()
        );
    }
    assert!(
        propose_affine_reported(
            &protocol,
            &query,
            &[FaultSelection {
                site: 0,
                alternative: 999
            }],
            &mut Words::new(vec![]),
            &SamplingOptions::default(),
            &a
        )
        .is_err()
    );
}
