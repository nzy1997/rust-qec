//! Owned side expansion, literal matrix oracles, and the complete original
//! SingleTraceTests retained_labels_preserve_inherited_rank_squared_bound case.
use rmagna::clifford::CliffordGate;
use rmagna::fibers::FiberOptions;
use rmagna::gf2::BitVector;
use rmagna::pauli::PauliWord;
use rmagna::phase::PhaseError;
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::{
    CompilerStrategy, ExpansionLimits, OwnedStabilizerExpansion, PauliInsertion, UnitaryCircuit,
    UnitaryError, UnitaryGate,
};

const STRATEGIES: [CompilerStrategy; 2] = [
    CompilerStrategy::ParityCopies,
    CompilerStrategy::SharedCommutingLayer,
];
fn bits(width: usize, mask: usize) -> BitVector {
    BitVector::from_indices(
        width,
        (0..width.min(usize::BITS as usize)).filter(|j| mask >> j & 1 != 0),
    )
    .unwrap()
}
fn zero_phase(rank: usize) -> QuadraticForm {
    QuadraticForm::new(
        0,
        vec![0; rank],
        vec![BitVector::zeros(rank).unwrap(); rank],
    )
    .unwrap()
}
fn plus(width: usize, a: &ScalarArithmetic) -> AffineKet {
    AffineKet::new(
        BitVector::zeros(width).unwrap(),
        (0..width)
            .map(|q| BitVector::from_indices(width, [q]).unwrap())
            .collect(),
        zero_phase(width),
        a.omega(0, -(width as i64)).unwrap(),
    )
    .unwrap()
}
fn amplitude(
    side: &OwnedStabilizerExpansion,
    word: &BitVector,
    a: &ScalarArithmetic,
) -> ExactScalar {
    side.entries()
        .iter()
        .fold(ExactScalar::zero(), |sum, term| {
            a.add(&sum, &term.prepare().unwrap().amplitude(word, a).unwrap())
                .unwrap()
        })
}

#[test]
fn original_retained_labels_preserve_inherited_rank_squared_bound() {
    let a = ScalarArithmetic::default();
    let circuit = UnitaryCircuit::new(
        3,
        [
            UnitaryGate::Ccz { a: 0, b: 1, c: 2 },
            UnitaryGate::Clifford(CliffordGate::H(0)),
            UnitaryGate::Clifford(CliffordGate::H(1)),
            UnitaryGate::Clifford(CliffordGate::H(2)),
        ],
    )
    .unwrap();
    let rows = [bits(3, 1), bits(3, 2), bits(3, 4)];
    // Rust bounds raw lowered paths (including phase-coordinate H gates),
    // whereas the original max_quotient_rank=6 bounds a reduced signature.
    let limits = ExpansionLimits {
        max_path_bits: 24,
        max_side_terms: 2,
        max_pairs: 4,
        ..Default::default()
    };
    for strategy in STRATEGIES {
        let side = circuit
            .compile_stabilizer_with_strategy(&plus(3, &a), &a, strategy)
            .unwrap()
            .stabilizer_expansion(limits, &a)
            .unwrap();
        // The compiled view is already dropped. Only the owned side kets remain.
        assert_eq!(side.statistics().certified_terms, 2);
        assert_eq!(side.statistics().side_entries, 2);
        for flip in [0, 2] {
            let records = side
                .retained_records(&rows, &bits(3, flip), limits, &a)
                .unwrap();
            assert_eq!(records.resource_expansion(), "inherited-product");
            assert_eq!(records.statistics().inherited_side_entries, 2);
            assert_eq!(records.statistics().pair_work, 4);
            assert!(records.statistics().constructive_terms <= 4);
            assert_eq!(
                records.statistics().constructive_terms,
                records.contributions().len()
            );
            assert_eq!(records.total(&a).unwrap(), ExactScalar::one());
            for word in 0..8 {
                let numerator = if word ^ flip == 0 { 9 } else { 1 };
                let expected = a.scalar([numerator, 0, 0, 0].map(Into::into), -4).unwrap();
                assert_eq!(
                    records.mass(&rows, &bits(3, word), &a).unwrap(),
                    expected,
                    "{strategy:?}, flip={flip}, word={word}"
                );
            }
        }
    }
}

// Independent eight-component state-vector oracle: only exact scalar arithmetic
// is shared. No phase polynomial, Clifford lowering, affine ket or query kernel
// computes these literal input values or gate matrix entries.
fn literal_state(
    global: u8,
    y: bool,
    gates: &[UnitaryGate],
    a: &ScalarArithmetic,
) -> Vec<ExactScalar> {
    let mut state = vec![ExactScalar::zero(); 8];
    for (word, phase) in [(2, 2), (7, 4), (1, 0), (4, 6)] {
        state[word] = a.omega(i64::from(global + phase), -2).unwrap();
    }
    for (position, gate) in gates.iter().enumerate() {
        if y && position == 3 {
            let mut changed = vec![ExactScalar::zero(); 8];
            for word in 0..8 {
                // Literal Y = [[0,-i],[i,0]] on qubit one.
                changed[word ^ 2] = a
                    .multiply(
                        &state[word],
                        &a.omega(if word & 2 == 0 { 2 } else { 6 }, 0).unwrap(),
                    )
                    .unwrap();
            }
            state = changed;
        }
        match *gate {
            UnitaryGate::Clifford(CliffordGate::H(q)) => {
                let mut changed = vec![ExactScalar::zero(); 8];
                for word in 0..8 {
                    let other = &state[word ^ (1 << q)];
                    let own = if word & (1 << q) == 0 {
                        state[word].clone()
                    } else {
                        state[word].negated()
                    };
                    changed[word] = a
                        .multiply(&a.add(&own, other).unwrap(), &a.omega(0, -1).unwrap())
                        .unwrap();
                }
                state = changed;
            }
            UnitaryGate::T { qubit, turns } => {
                for (word, value) in state.iter_mut().enumerate() {
                    if word >> qubit & 1 != 0 {
                        *value = a
                            .multiply(value, &a.omega(i64::from(turns), 0).unwrap())
                            .unwrap();
                    }
                }
            }
            UnitaryGate::Cs { a: q, b, turns } => {
                for (word, value) in state.iter_mut().enumerate() {
                    if word >> q & word >> b & 1 != 0 {
                        *value = a
                            .multiply(value, &a.omega(i64::from(2 * turns), 0).unwrap())
                            .unwrap();
                    }
                }
            }
            UnitaryGate::Ccz { a: q, b, c } => {
                for (word, value) in state.iter_mut().enumerate() {
                    if word >> q & word >> b & word >> c & 1 != 0 {
                        *value = value.negated();
                    }
                }
            }
            _ => panic!("the literal oracle only admits its explicitly defined gates"),
        }
    }
    state
}
#[test]
fn actual_views_match_literal_noncommuting_matrices_with_all_global_phases_and_y() {
    let a = ScalarArithmetic::default();
    let gates = [
        UnitaryGate::Clifford(CliffordGate::H(0)),
        UnitaryGate::T { qubit: 0, turns: 1 },
        UnitaryGate::Cs {
            a: 0,
            b: 1,
            turns: 1,
        },
        UnitaryGate::Clifford(CliffordGate::H(1)),
        UnitaryGate::Ccz { a: 0, b: 1, c: 2 },
        UnitaryGate::Clifford(CliffordGate::H(2)),
        UnitaryGate::T { qubit: 2, turns: 3 },
        UnitaryGate::Cs {
            a: 1,
            b: 2,
            turns: 3,
        },
        UnitaryGate::Clifford(CliffordGate::H(0)),
    ];
    let circuit = UnitaryCircuit::new(3, gates).unwrap();
    let labels = [bits(3, 5), bits(3, 3), bits(3, 6), bits(3, 5), bits(3, 0)];
    let events = (0..5).map(|q| bits(5, 1 << q)).collect::<Vec<_>>();
    let limits = ExpansionLimits::default();
    for strategy in STRATEGIES {
        for global in 0..8 {
            let input = AffineKet::new(
                bits(3, 2),
                vec![bits(3, 5), bits(3, 3)],
                QuadraticForm::new(1, vec![1, 3], vec![bits(2, 2), bits(2, 0)]).unwrap(),
                a.omega(i64::from(global), -2).unwrap(),
            )
            .unwrap();
            let compiled = circuit
                .compile_stabilizer_with_strategy(&input, &a, strategy)
                .unwrap();
            for y in [false, true] {
                let insertions = if y {
                    vec![PauliInsertion {
                        before_gate: 3,
                        word: PauliWord::new(bits(3, 2), bits(3, 2), 2).unwrap(),
                    }]
                } else {
                    vec![]
                };
                let view = compiled.with_insertions(&insertions, &a).unwrap();
                let side = view.stabilizer_expansion(limits, &a).unwrap();
                let dense = literal_state(global, y, &gates, &a);
                for (word, expected) in dense.iter().enumerate() {
                    assert_eq!(
                        amplitude(&side, &bits(3, word), &a),
                        *expected,
                        "{strategy:?} global={global}, Y={y}, word={word}"
                    );
                }
                let records = side
                    .retained_records(&labels, &bits(5, 18), limits, &a)
                    .unwrap();
                assert_eq!(records.total(&a).unwrap(), ExactScalar::one());
                assert_eq!(records.statistics().pair_work, side.entries().len().pow(2));
                // All labels, including dependent/zero-row inconsistencies;
                // common global phases have already been checked as amplitudes.
                if global == 0 {
                    for label in 0..32 {
                        let mut expected = ExactScalar::zero();
                        for (word, value) in dense.iter().enumerate() {
                            let observed = [5_usize, 3, 6, 5, 0]
                                .iter()
                                .enumerate()
                                .fold(18, |v, (j, mask)| {
                                    v ^ (((word & mask).count_ones() as usize & 1) << j)
                                });
                            if observed == label {
                                expected =
                                    a.add(&expected, &a.norm_squared(value).unwrap()).unwrap();
                            }
                        }
                        assert_eq!(
                            records.mass(&events, &bits(5, label), &a).unwrap(),
                            expected
                        );
                    }
                    let repeated = [bits(5, 1), bits(5, 1)];
                    assert_eq!(
                        records.mass(&repeated, &bits(2, 1), &a).unwrap(),
                        ExactScalar::zero()
                    );
                    for outcome in [0, 1] {
                        assert_eq!(
                            records.mass(&repeated, &bits(2, outcome * 3), &a).unwrap(),
                            records.mass(&repeated[..1], &bits(1, outcome), &a).unwrap()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn contributions_keep_complex_cross_terms_and_physical_trace_removes_coherent_marginals() {
    let a = ScalarArithmetic::default();
    let circuit = UnitaryCircuit::new(
        1,
        [
            UnitaryGate::Clifford(CliffordGate::H(0)),
            UnitaryGate::T { qubit: 0, turns: 1 },
            UnitaryGate::Clifford(CliffordGate::H(0)),
        ],
    )
    .unwrap();
    for strategy in STRATEGIES {
        let side = circuit
            .compile_with_strategy(&bits(1, 0), &a, strategy)
            .unwrap()
            .stabilizer_expansion(ExpansionLimits::default(), &a)
            .unwrap();
        let records = side
            .retained_records(&[bits(1, 1)], &bits(1, 0), ExpansionLimits::default(), &a)
            .unwrap();
        let mut raw = records
            .contributions()
            .iter()
            .map(|t| t.prepare().unwrap().amplitude(&bits(1, 0), &a).unwrap())
            .collect::<Vec<_>>();
        let mut expected = vec![
            a.omega(0, -4).unwrap(),
            a.omega(0, -4).unwrap(),
            a.omega(1, -4).unwrap(),
            a.omega(7, -4).unwrap(),
        ];
        // Exact multiset comparison without relying on a fiber order.
        for value in expected.drain(..) {
            let at = raw.iter().position(|v| *v == value).unwrap();
            raw.remove(at);
        }
        assert!(raw.is_empty());
        let no_records = side
            .retained_records(&[], &bits(0, 0), ExpansionLimits::default(), &a)
            .unwrap();
        assert_eq!(no_records.total(&a).unwrap(), ExactScalar::one());
    }
    let bell = UnitaryCircuit::new(
        2,
        [
            UnitaryGate::Clifford(CliffordGate::H(0)),
            UnitaryGate::Clifford(CliffordGate::Cx {
                control: 0,
                target: 1,
            }),
        ],
    )
    .unwrap()
    .compile(&bits(2, 0), &a)
    .unwrap()
    .stabilizer_expansion(ExpansionLimits::default(), &a)
    .unwrap();
    let marginal = bell
        .retained_records(&[bits(2, 1)], &bits(1, 0), ExpansionLimits::default(), &a)
        .unwrap();
    for word in 0..2 {
        assert_eq!(
            marginal.mass(&[bits(1, 1)], &bits(1, word), &a).unwrap(),
            a.omega(0, -2).unwrap()
        );
    }
}

#[test]
fn vacuum_and_cross_word_physical_and_record_labels_retain_order_and_flips() {
    let a = ScalarArithmetic::default();
    let limits = ExpansionLimits::default();
    let vacuum = UnitaryCircuit::new(0, [])
        .unwrap()
        .compile(&bits(0, 0), &a)
        .unwrap()
        .stabilizer_expansion(limits, &a)
        .unwrap();
    assert_eq!(amplitude(&vacuum, &bits(0, 0), &a), ExactScalar::one());
    assert_eq!(
        vacuum
            .retained_records(&[], &bits(0, 0), limits, &a)
            .unwrap()
            .total(&a)
            .unwrap(),
        ExactScalar::one()
    );
    let rows = (0..129)
        .map(|j| BitVector::from_indices(129, [if j % 2 == 0 { 128 } else { 64 }]).unwrap())
        .collect::<Vec<_>>();
    let flips = BitVector::from_indices(129, [0, 64, 128]).unwrap();
    for strategy in STRATEGIES {
        let circuit = UnitaryCircuit::new(
            129,
            [
                UnitaryGate::Clifford(CliffordGate::H(64)),
                UnitaryGate::Clifford(CliffordGate::Cx {
                    control: 64,
                    target: 128,
                }),
                UnitaryGate::T {
                    qubit: 128,
                    turns: 1,
                },
            ],
        )
        .unwrap();
        let side = circuit
            .compile_with_strategy(&BitVector::zeros(129).unwrap(), &a, strategy)
            .unwrap()
            .stabilizer_expansion(limits, &a)
            .unwrap();
        let records = side.retained_records(&rows, &flips, limits, &a).unwrap();
        let events = (0..129)
            .map(|j| BitVector::from_indices(129, [j]).unwrap())
            .collect::<Vec<_>>();
        for bit in [false, true] {
            let mut word = if bit {
                BitVector::from_indices(129, 0..129).unwrap()
            } else {
                BitVector::zeros(129).unwrap()
            };
            word.xor_assign(&flips).unwrap();
            assert_eq!(
                records.mass(&events, &word, &a).unwrap(),
                a.omega(0, -2).unwrap()
            );
            word.set(65, !word.get(65).unwrap()).unwrap();
            assert_eq!(
                records.mass(&events, &word, &a).unwrap(),
                ExactScalar::zero()
            );
        }
        assert_eq!(records.total(&a).unwrap(), ExactScalar::one());
    }
}

#[test]
fn finite_side_pair_record_event_and_geometry_limits_return_typed_errors() {
    let a = ScalarArithmetic::default();
    let compiled = UnitaryCircuit::new(3, [UnitaryGate::Ccz { a: 0, b: 1, c: 2 }])
        .unwrap()
        .compile_stabilizer_with_strategy(&plus(3, &a), &a, CompilerStrategy::ParityCopies)
        .unwrap();
    for (option, limits) in [
        (
            "max_path_bits",
            ExpansionLimits {
                max_path_bits: 2,
                ..Default::default()
            },
        ),
        (
            "max_side_terms",
            ExpansionLimits {
                max_side_terms: 0,
                ..Default::default()
            },
        ),
        (
            "max_geometry_words",
            ExpansionLimits {
                max_geometry_words: 0,
                ..Default::default()
            },
        ),
    ] {
        assert!(
            matches!(compiled.stabilizer_expansion(limits, &a), Err(UnitaryError::ExpansionResourceLimit { option: got, .. }) if got == option)
        );
    }
    assert!(matches!(
        compiled.stabilizer_expansion(
            ExpansionLimits {
                max_side_terms: 1,
                ..Default::default()
            },
            &a
        ),
        Err(UnitaryError::Phase(PhaseError::FiberTermLimit {
            limit: 1,
            ..
        }))
    ));
    // Disabling the fiber search cannot silently retry a different route.
    assert!(matches!(
        compiled.stabilizer_expansion_with_fibers(
            ExpansionLimits {
                max_side_terms: 2,
                ..Default::default()
            },
            FiberOptions {
                attempts: 0,
                ..Default::default()
            },
            &a
        ),
        Err(UnitaryError::Phase(PhaseError::FiberTermLimit {
            limit: 2,
            ..
        }))
    ));
    let side = compiled
        .stabilizer_expansion(ExpansionLimits::default(), &a)
        .unwrap();
    let rows = [bits(3, 1), bits(3, 2), bits(3, 4)];
    for (option, limits) in [
        (
            "max_pairs",
            ExpansionLimits {
                max_pairs: 3,
                ..Default::default()
            },
        ),
        (
            "max_side_terms",
            ExpansionLimits {
                max_side_terms: 1,
                ..Default::default()
            },
        ),
        (
            "max_record_rows",
            ExpansionLimits {
                max_record_rows: 2,
                ..Default::default()
            },
        ),
        (
            "max_path_bits",
            ExpansionLimits {
                max_path_bits: 0,
                ..Default::default()
            },
        ),
        (
            "max_geometry_words",
            ExpansionLimits {
                max_geometry_words: 0,
                ..Default::default()
            },
        ),
    ] {
        assert!(
            matches!(side.retained_records(&rows, &bits(3, 0), limits, &a), Err(UnitaryError::ExpansionResourceLimit { option: got, .. }) if got == option)
        );
    }
    assert!(matches!(
        side.retained_records(&[bits(2, 1)], &bits(1, 0), ExpansionLimits::default(), &a),
        Err(UnitaryError::DimensionMismatch)
    ));
    assert!(matches!(
        side.retained_records(&rows, &bits(2, 0), ExpansionLimits::default(), &a),
        Err(UnitaryError::DimensionMismatch)
    ));
    let distribution = side
        .retained_records(
            &rows,
            &bits(3, 0),
            ExpansionLimits {
                max_event_rows: 1,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
    assert!(matches!(
        distribution.mass(&rows, &bits(3, 0), &a),
        Err(UnitaryError::ExpansionResourceLimit {
            option: "max_event_rows",
            ..
        })
    ));
    assert!(matches!(
        distribution.mass(&[bits(2, 1)], &bits(1, 0), &a),
        Err(UnitaryError::DimensionMismatch)
    ));
    assert!(matches!(
        distribution.mass(&[bits(3, 1)], &bits(2, 0), &a),
        Err(UnitaryError::DimensionMismatch)
    ));
}

#[test]
fn signed_contributions_sum_without_renormalization_and_scalar_failures_propagate() {
    let a = ScalarArithmetic::default();
    let circuit = UnitaryCircuit::new(
        3,
        [
            UnitaryGate::Ccz { a: 0, b: 1, c: 2 },
            UnitaryGate::Clifford(CliffordGate::H(0)),
            UnitaryGate::Clifford(CliffordGate::H(1)),
            UnitaryGate::Clifford(CliffordGate::H(2)),
        ],
    )
    .unwrap();
    let rows = [bits(3, 1), bits(3, 2), bits(3, 4)];
    let side = circuit
        .compile_stabilizer_with_strategy(&plus(3, &a), &a, CompilerStrategy::ParityCopies)
        .unwrap()
        .stabilizer_expansion(ExpansionLimits::default(), &a)
        .unwrap();
    let records = side
        .retained_records(&rows, &bits(3, 0), ExpansionLimits::default(), &a)
        .unwrap();
    let mut negative = false;
    for word in 0..8 {
        let mut sum = ExactScalar::zero();
        for term in records.contributions() {
            let value = term
                .prepare()
                .unwrap()
                .amplitude(&bits(3, word), &a)
                .unwrap();
            negative |= value.real_sign().is_ok_and(|s| s.is_lt());
            sum = a.add(&sum, &value).unwrap();
        }
        assert_eq!(sum, records.mass(&rows, &bits(3, word), &a).unwrap());
    }
    assert!(
        negative,
        "destructive interference needs its signed cross contributions"
    );
    assert!(matches!(
        records.mass(&rows, &bits(3, 0), &ScalarArithmetic::new(1).unwrap()),
        Err(UnitaryError::Scalar(
            rmagna::scalar::ScalarError::CoefficientLimit { .. }
        ))
    ));
}
