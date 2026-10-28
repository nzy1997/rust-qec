// Other suites use the shared oracle's optional RNG helper.
#[allow(dead_code)]
mod common;

use common::{bits, dense_gate};
use rmagna::clifford::{CliffordCircuit, CliffordGate as C};
use rmagna::gf2::BitVector;
use rmagna::pauli::PauliWord;
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::{
    CompiledUnitary, CompilerStrategy, PauliInsertion, UnitaryCircuit, UnitaryError,
    UnitaryGate as G,
};

const STRATEGIES: [CompilerStrategy; 2] = [
    CompilerStrategy::ParityCopies,
    CompilerStrategy::SharedCommutingLayer,
];
const TERMS: usize = 65536;

// Expected labels and phases are evaluated from these literal integer data,
// without calling AffineKet contraction/preparation or QuadraticForm evaluation.
fn initial(
    width: usize,
    offset: usize,
    columns: &[usize],
    constant: u8,
    linear: &[u8],
    pairs: &[(usize, usize)],
    global: i64,
) -> (AffineKet, Vec<ExactScalar>) {
    let a = ScalarArithmetic::default();
    let rank = columns.len();
    let mut upper = vec![bits(rank, 0); rank];
    for &(j, k) in pairs {
        upper[j].set(k, true).unwrap();
    }
    let input = AffineKet::new(
        bits(width, offset),
        columns.iter().map(|&v| bits(width, v)).collect(),
        QuadraticForm::new(constant, linear.to_vec(), upper).unwrap(),
        a.omega(global, -(rank as i64)).unwrap(),
    )
    .unwrap();
    let mut dense = vec![ExactScalar::zero(); 1 << width];
    for x in 0..1usize << rank {
        let mut label = offset;
        let mut phase = i64::from(constant);
        for (j, &column) in columns.iter().enumerate() {
            if x >> j & 1 != 0 {
                label ^= column;
                phase += i64::from(linear[j]);
            }
        }
        for &(j, k) in pairs {
            phase += 2 * ((x >> j) & (x >> k) & 1) as i64;
        }
        let value = a.omega(global + 2 * phase, -(rank as i64)).unwrap();
        dense[label] = a.add(&dense[label], &value).unwrap();
    }
    (input, dense)
}

fn gates() -> Vec<G> {
    vec![
        G::T { qubit: 0, turns: 1 },
        G::Clifford(C::H(0)),
        G::Cs {
            a: 0,
            b: 1,
            turns: 3,
        },
        G::Clifford(C::H(1)),
        G::Ccz { a: 0, b: 1, c: 2 },
        G::Clifford(C::S { qubit: 2, turns: 3 }),
        G::Clifford(C::Cx {
            control: 2,
            target: 0,
        }),
        G::Clifford(C::H(2)),
        G::T { qubit: 1, turns: 5 },
        G::Cs {
            a: 0,
            b: 2,
            turns: 1,
        },
    ]
}

fn dense_unitary(values: &mut Vec<ExactScalar>, gate: G, a: &ScalarArithmetic) {
    if let G::Clifford(gate) = gate {
        dense_gate(gate, values, a);
        return;
    }
    for (label, value) in values.iter_mut().enumerate() {
        let bit = |j| ((label >> j) & 1) as i64;
        let phase = match gate {
            G::T { qubit, turns } => i64::from(turns) * bit(qubit),
            G::Cs { a, b, turns } => 2 * i64::from(turns) * bit(a) * bit(b),
            G::Ccz { a, b, c } => 4 * bit(a) * bit(b) * bit(c),
            G::Clifford(_) => unreachable!(),
        };
        *value = a.multiply(value, &a.omega(phase, 0).unwrap()).unwrap();
    }
}

fn dense_pauli(values: &mut Vec<ExactScalar>, x: usize, z: usize, phase: u8) {
    let a = ScalarArithmetic::default();
    let mut out = vec![ExactScalar::zero(); values.len()];
    for (label, value) in values.iter().enumerate() {
        let exponent = i64::from(phase) + 4 * i64::from((label & z).count_ones() % 2);
        out[label ^ x] = a.multiply(value, &a.omega(exponent, 0).unwrap()).unwrap();
    }
    *values = out;
}

fn probability(values: &[ExactScalar], rows: &[usize], outcomes: usize) -> ExactScalar {
    let a = ScalarArithmetic::default();
    let mut result = ExactScalar::zero();
    for (label, value) in values.iter().enumerate() {
        if rows
            .iter()
            .enumerate()
            .all(|(j, &row)| ((label & row).count_ones() as usize & 1) == (outcomes >> j & 1))
        {
            result = a.add(&result, &a.norm_squared(value).unwrap()).unwrap();
        }
    }
    result
}

fn check_dense(state: &CompiledUnitary, values: &[ExactScalar]) {
    let a = ScalarArithmetic::default();
    for (label, wanted) in values.iter().enumerate() {
        assert_eq!(
            state
                .amplitude(&bits(state.width(), label), TERMS, &a)
                .unwrap()
                .value,
            *wanted
        );
    }
    for masks in [vec![], vec![3, 6], vec![3, 6, 3]] {
        let rows = masks
            .iter()
            .map(|&r| bits(state.width(), r))
            .collect::<Vec<_>>();
        let prepared = state.prepare_probability(&rows, &a).unwrap();
        for outcome in 0..1 << rows.len() {
            let wanted = probability(values, &masks, outcome);
            let outcome = bits(rows.len(), outcome);
            assert_eq!(
                state
                    .probability_z(&rows, &outcome, TERMS, &a)
                    .unwrap()
                    .value,
                wanted
            );
            assert_eq!(
                prepared.evaluate(state, &outcome, TERMS, &a).unwrap().value,
                wanted
            );
        }
    }
}

#[test]
fn entangled_inputs_with_literal_affine_quadratic_and_global_phases_match_dense_gates() {
    let a = ScalarArithmetic::default();
    let source = UnitaryCircuit::new(3, gates()).unwrap();
    for global in 0..8 {
        for columns in [[5, 6], [6, 5], [5, 3]] {
            let (input, mut dense) = initial(
                3,
                (global as usize + 3) % 8,
                &columns,
                3,
                &[1, 3],
                &[(0, 1)],
                global,
            );
            let original = input.clone();
            for gate in gates() {
                dense_unitary(&mut dense, gate, &a);
            }
            for strategy in STRATEGIES {
                let state = source
                    .compile_stabilizer_with_strategy(&input, &a, strategy)
                    .unwrap();
                check_dense(&state, &dense);
            }
            assert_eq!(input, original);
        }
    }
}

#[test]
fn immutable_preparations_reuse_fault_views_and_reject_foreign_input_owners() {
    let a = ScalarArithmetic::default();
    let (input, dense) = initial(3, 7, &[5, 3, 1], 1, &[3, 1, 2], &[(0, 1), (1, 2)], 5);
    let original = input.clone();
    let source = UnitaryCircuit::new(3, gates()).unwrap();
    for strategy in STRATEGIES {
        let state = source
            .compile_stabilizer_with_strategy(&input, &a, strategy)
            .unwrap();
        let rows = [bits(3, 3), bits(3, 6)];
        let prepared = state.prepare_probability(&rows, &a).unwrap();
        let assignments = [
            vec![],
            vec![(0, 1, 6, 5)],
            vec![(2, 3, 1, 2), (2, 4, 3, 7)],
            vec![(1, 2, 4, 6), (gates().len(), 4, 3, 2)],
        ];
        for assignment in assignments {
            let insertions = assignment
                .iter()
                .map(|&(before_gate, x, z, phase)| PauliInsertion {
                    before_gate,
                    word: PauliWord::new(bits(3, x), bits(3, z), phase).unwrap(),
                })
                .collect::<Vec<_>>();
            let mut wanted = dense.clone();
            for position in 0..=gates().len() {
                for &(_, x, z, phase) in assignment.iter().filter(|&&(p, _, _, _)| p == position) {
                    dense_pauli(&mut wanted, x, z, phase);
                }
                if let Some(&gate) = gates().get(position) {
                    dense_unitary(&mut wanted, gate, &a);
                }
            }
            let view = state.with_insertions(&insertions, &a).unwrap();
            check_dense(&view, &wanted);
            for outcome in 0..4 {
                assert_eq!(
                    prepared
                        .evaluate(&view, &bits(2, outcome), TERMS, &a)
                        .unwrap()
                        .value,
                    probability(&wanted, &[3, 6], outcome)
                );
            }
            // Replacing the assignment on a view returns the same reference.
            let reset = view.with_insertions(&[], &a).unwrap();
            let mut reference = dense.clone();
            for gate in gates() {
                dense_unitary(&mut reference, gate, &a);
            }
            check_dense(&reset, &reference);
        }
        let foreign = source
            .compile_stabilizer_with_strategy(&input, &a, strategy)
            .unwrap();
        assert!(matches!(
            prepared.evaluate(&foreign, &bits(2, 0), TERMS, &a),
            Err(UnitaryError::IncompatibleProgram)
        ));
        let invalid = PauliInsertion {
            before_gate: gates().len() + 1,
            word: PauliWord::identity(3).unwrap(),
        };
        assert!(matches!(
            state.with_insertions(&[invalid], &a),
            Err(UnitaryError::InvalidInsertion { .. })
        ));
        let mut wanted = dense.clone();
        for gate in gates() {
            dense_unitary(&mut wanted, gate, &a);
        }
        check_dense(&state, &wanted);
        for outcome in 0..4 {
            assert_eq!(
                prepared
                    .evaluate(&state, &bits(2, outcome), TERMS, &a)
                    .unwrap()
                    .value,
                probability(&wanted, &[3, 6], outcome)
            );
        }
    }
    assert_eq!(input, original);
    // A compiled program owns its boundary independently of both caller inputs.
    let state = source.compile_stabilizer(&input, &a).unwrap();
    drop(input);
    drop(source);
    let mut wanted = dense;
    for gate in gates() {
        dense_unitary(&mut wanted, gate, &a);
    }
    check_dense(&state, &wanted);
}

#[test]
fn computational_methods_agree_with_the_stabilizer_route_including_zero_width() {
    let a = ScalarArithmetic::default();
    for width in 0..=3 {
        let source = UnitaryCircuit::new(
            width,
            if width == 3 {
                gates()
            } else if width == 0 {
                vec![]
            } else {
                vec![G::Clifford(C::H(0)), G::T { qubit: 0, turns: 7 }]
            },
        )
        .unwrap();
        for label in 0..1 << width {
            let basis = bits(width, label);
            let input = CliffordCircuit::computational(basis.clone()).unwrap();
            let old_default = source.compile(&basis, &a).unwrap();
            let new_default = source.compile_stabilizer(&input, &a).unwrap();
            assert_eq!(old_default.compilation(), new_default.compilation());
            for strategy in STRATEGIES {
                let old = source.compile_with_strategy(&basis, &a, strategy).unwrap();
                let new = source
                    .compile_stabilizer_with_strategy(&input, &a, strategy)
                    .unwrap();
                assert_eq!(old.compilation(), new.compilation());
                let limited = source
                    .compile_stabilizer_with_path_limit(
                        &input,
                        &a,
                        strategy,
                        old.compilation().raw_path_variables,
                    )
                    .unwrap();
                assert_eq!(limited.compilation(), old.compilation());
                for output in 0..1 << width {
                    let output = bits(width, output);
                    let wanted = old.amplitude(&output, TERMS, &a).unwrap().value;
                    assert_eq!(new.amplitude(&output, TERMS, &a).unwrap().value, wanted);
                    assert_eq!(limited.amplitude(&output, TERMS, &a).unwrap().value, wanted);
                    if strategy == CompilerStrategy::ParityCopies {
                        assert_eq!(
                            old_default.amplitude(&output, TERMS, &a).unwrap().value,
                            wanted
                        );
                        assert_eq!(
                            new_default.amplitude(&output, TERMS, &a).unwrap().value,
                            wanted
                        );
                    }
                }
            }
        }
    }
    let (input, dense) = initial(0, 0, &[], 3, &[], &[], 7);
    for strategy in STRATEGIES {
        let state = UnitaryCircuit::new(0, [])
            .unwrap()
            .compile_stabilizer_with_path_limit(&input, &a, strategy, 0)
            .unwrap();
        assert_eq!(state.amplitude(&bits(0, 0), 1, &a).unwrap().value, dense[0]);
        assert_eq!(
            state.probability_z(&[], &bits(0, 0), 1, &a).unwrap().value,
            ExactScalar::one()
        );
    }
}

#[test]
fn initial_support_and_lowered_hadamards_are_charged_before_local_contraction() {
    let a = ScalarArithmetic::default();
    let (input, _) = initial(3, 4, &[3, 6], 2, &[1, 2], &[(0, 1)], 3);
    for strategy in STRATEGIES {
        for (source, required) in [
            (UnitaryCircuit::new(3, []).unwrap(), 2),
            (
                UnitaryCircuit::new(3, vec![G::Clifford(C::H(0)); 8]).unwrap(),
                10,
            ),
            (
                UnitaryCircuit::new(
                    3,
                    [
                        G::Clifford(C::H(0)),
                        G::T { qubit: 0, turns: 1 },
                        G::Clifford(C::H(0)),
                    ],
                )
                .unwrap(),
                if strategy == CompilerStrategy::ParityCopies {
                    4
                } else {
                    6
                },
            ),
        ] {
            for limit in [0, required - 1] {
                assert!(
                    matches!(source.compile_stabilizer_with_path_limit(&input, &a, strategy, limit), Err(UnitaryError::RawPathLimit { limit: l, required: r }) if l == limit && r == required)
                );
            }
            let state = source
                .compile_stabilizer_with_path_limit(&input, &a, strategy, required)
                .unwrap();
            assert_eq!(state.compilation().raw_path_variables, required);
            assert_eq!(
                state
                    .probability_z(&[], &bits(0, 0), TERMS, &a)
                    .unwrap()
                    .value,
                ExactScalar::one()
            );
            if required == 10 {
                assert_eq!(state.compilation().path_variables, 2);
            }
        }
    }
}

#[test]
fn sparse_entanglement_and_full_rank_inputs_cross_the_129_bit_boundary() {
    let a = ScalarArithmetic::default();
    let positions = [0, 64, 128];
    let lift = |label: usize| {
        BitVector::from_indices(
            129,
            positions
                .iter()
                .enumerate()
                .filter_map(|(j, &p)| (label >> j & 1 != 0).then_some(p)),
        )
        .unwrap()
    };
    let (small, mut dense) = initial(3, 6, &[5, 3, 1], 3, &[1, 2, 3], &[(0, 2), (1, 2)], 5);
    let input = AffineKet::new(
        lift(6),
        [5, 3, 1].into_iter().map(lift).collect(),
        small.phase().clone(),
        small.coefficient().clone(),
    )
    .unwrap();
    let small_gates = [
        G::Clifford(C::H(0)),
        G::Cs {
            a: 0,
            b: 1,
            turns: 1,
        },
        G::Ccz { a: 0, b: 1, c: 2 },
        G::Clifford(C::H(2)),
        G::T { qubit: 1, turns: 3 },
    ];
    let wide_gates = [
        G::Clifford(C::H(0)),
        G::Cs {
            a: 0,
            b: 64,
            turns: 1,
        },
        G::Ccz {
            a: 0,
            b: 64,
            c: 128,
        },
        G::Clifford(C::H(128)),
        G::T {
            qubit: 64,
            turns: 3,
        },
    ];
    for gate in small_gates {
        dense_unitary(&mut dense, gate, &a);
    }
    for strategy in STRATEGIES {
        let state = UnitaryCircuit::new(129, wide_gates)
            .unwrap()
            .compile_stabilizer_with_strategy(&input, &a, strategy)
            .unwrap();
        for (label, wanted) in dense.iter().enumerate() {
            assert_eq!(
                state.amplitude(&lift(label), TERMS, &a).unwrap().value,
                *wanted
            );
        }
        assert_eq!(
            state
                .amplitude(&BitVector::from_indices(129, [63]).unwrap(), TERMS, &a)
                .unwrap()
                .value,
            ExactScalar::zero()
        );
        let rows = [lift(3), lift(6)];
        let prepared = state.prepare_probability(&rows, &a).unwrap();
        for outcome in 0..4 {
            let wanted = probability(&dense, &[3, 6], outcome);
            assert_eq!(
                prepared
                    .evaluate(&state, &bits(2, outcome), TERMS, &a)
                    .unwrap()
                    .value,
                wanted
            );
        }
    }
    let full = AffineKet::new(
        BitVector::from_indices(129, [64, 128]).unwrap(),
        (0..129)
            .rev()
            .map(|q| BitVector::from_indices(129, [q]).unwrap())
            .collect(),
        QuadraticForm::new(1, vec![0; 129], vec![bits(129, 0); 129]).unwrap(),
        a.omega(3, -129).unwrap(),
    )
    .unwrap();
    let expected = a.omega(5, -129).unwrap();
    for strategy in STRATEGIES {
        let source = UnitaryCircuit::new(129, []).unwrap();
        assert!(matches!(
            source.compile_stabilizer_with_path_limit(&full, &a, strategy, 128),
            Err(UnitaryError::RawPathLimit { required: 129, .. })
        ));
        let state = source
            .compile_stabilizer_with_path_limit(&full, &a, strategy, 129)
            .unwrap();
        assert_eq!(
            state.amplitude(&bits(129, 0), 1, &a).unwrap().value,
            expected
        );
        assert_eq!(
            state.probability_z(&[], &bits(0, 0), 1, &a).unwrap().value,
            ExactScalar::one()
        );
    }
}

fn raw_input(columns: Vec<BitVector>, coefficient: ExactScalar) -> AffineKet {
    let n = columns.len();
    AffineKet::new(
        bits(3, 1),
        columns,
        QuadraticForm::new(0, vec![0; n], vec![bits(n, 0); n]).unwrap(),
        coefficient,
    )
    .unwrap()
}

#[test]
fn invalid_inputs_are_rejected_explicitly_without_repair_or_reference_mutation() {
    let a = ScalarArithmetic::default();
    let source = UnitaryCircuit::new(3, gates()).unwrap();
    let (valid, mut dense) = initial(3, 7, &[5, 6], 3, &[1, 3], &[(0, 1)], 1);
    for gate in gates() {
        dense_unitary(&mut dense, gate, &a);
    }
    let repaired_norm_one = raw_input(vec![bits(3, 0)], a.omega(0, -2).unwrap());
    assert_eq!(
        repaired_norm_one.norm_squared(&a).unwrap(),
        ExactScalar::one()
    );
    let duplicate_norm_one = raw_input(vec![bits(3, 1), bits(3, 1)], a.omega(0, -3).unwrap());
    assert_eq!(
        duplicate_norm_one.norm_squared(&a).unwrap(),
        ExactScalar::one()
    );
    let invalid = [
        (
            CliffordCircuit::computational(bits(2, 1)).unwrap(),
            UnitaryError::DimensionMismatch,
        ),
        (
            raw_input(vec![], ExactScalar::zero()),
            UnitaryError::ZeroStabilizerInput,
        ),
        (repaired_norm_one, UnitaryError::DependentStabilizerInput),
        (duplicate_norm_one, UnitaryError::DependentStabilizerInput),
        (
            raw_input(vec![bits(3, 1)], ExactScalar::one()),
            UnitaryError::UnnormalizedStabilizerInput,
        ),
        (
            raw_input(vec![], a.omega(0, -2).unwrap()),
            UnitaryError::UnnormalizedStabilizerInput,
        ),
    ];
    for strategy in STRATEGIES {
        let state = source
            .compile_stabilizer_with_strategy(&valid, &a, strategy)
            .unwrap();
        let prepared = state.prepare_probability(&[], &a).unwrap();
        for (input, error) in &invalid {
            let before = input.clone();
            assert_eq!(
                source
                    .compile_stabilizer_with_path_limit(input, &a, strategy, 0)
                    .err(),
                Some(error.clone())
            );
            assert_eq!(
                source
                    .compile_stabilizer_with_strategy(input, &a, strategy)
                    .err(),
                Some(error.clone())
            );
            assert_eq!(
                source.compile_stabilizer(input, &a).err(),
                Some(error.clone())
            );
            assert_eq!(*input, before);
            assert_eq!(
                prepared
                    .evaluate(&state, &bits(0, 0), TERMS, &a)
                    .unwrap()
                    .value,
                ExactScalar::one()
            );
        }
        check_dense(&state, &dense);
    }
}

#[test]
fn scalar_coefficient_and_exponent_failures_propagate_before_lowering() {
    let a = ScalarArithmetic::default();
    let tiny = ScalarArithmetic::new(1).unwrap();
    let source = UnitaryCircuit::new(3, []).unwrap();
    let oversized = raw_input(vec![], a.integer(3).unwrap());
    for strategy in STRATEGIES {
        assert!(matches!(
            source.compile_stabilizer_with_path_limit(&oversized, &tiny, strategy, 0),
            Err(UnitaryError::Scalar(ScalarError::CoefficientLimit {
                required: 2,
                limit: 1
            }))
        ));
        for exponent in [i64::MIN, i64::MAX] {
            let input = raw_input(
                vec![],
                a.scalar([1.into(), 0.into(), 0.into(), 0.into()], exponent)
                    .unwrap(),
            );
            let before = input.clone();
            assert_eq!(
                source
                    .compile_stabilizer_with_strategy(&input, &a, strategy)
                    .err(),
                Some(UnitaryError::Scalar(ScalarError::ExponentOverflow))
            );
            assert_eq!(input, before);
        }
        let (valid, _) = initial(3, 1, &[5], 3, &[1], &[], 7);
        let state = source
            .compile_stabilizer_with_path_limit(&valid, &tiny, strategy, 1)
            .unwrap();
        assert_eq!(
            state.probability_z(&[], &bits(0, 0), 1, &a).unwrap().value,
            ExactScalar::one()
        );
    }
}
