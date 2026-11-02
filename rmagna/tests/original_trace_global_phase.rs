//! Finite translations of all three pinned original trace-global-phase methods.
#[allow(dead_code)]
mod common;

use common::{bits, dense_gate};
use rmagna::circuit::{
    Basis, BitAtom, BitExpr, Gate, NoiseKind, Operation, OperationProgram, PauliProduct,
};
use rmagna::clifford::CliffordGate;
use rmagna::instrument::{
    CompileOptions, FaultSelection, InstrumentQueryLayout, QueryPreparation,
    QueryPreparationOutcome, compile_operation_program_stabilizer,
};
use rmagna::pauli::PauliWord;
use rmagna::probability::Probability;
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::{CompilerStrategy, UnitaryCircuit, UnitaryGate};
use serde_json::Value;

fn fixture() -> Value {
    let value: Value =
        serde_json::from_str(include_str!("fixtures/original_trace_global_phase.json")).unwrap();
    assert_eq!(
        value["schema"],
        "rmagna.original-trace-global-phase-witness.v1"
    );
    assert_eq!(
        value["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(
        value["reference_manifest_sha256"],
        "c49203a586f5a451e3183b435793df693dd486857356aed8826df5dc723e5454"
    );
    assert_eq!(
        value["source"]["path"],
        "tests/core/test_trace_global_phase.py"
    );
    assert_eq!(value["trials"].as_array().unwrap().len(), 8);
    value
}

fn scalar(value: &Value, a: &ScalarArithmetic) -> ExactScalar {
    assert_eq!(value["coefficients"].as_array().unwrap().len(), 4);
    let coefficients =
        std::array::from_fn(|j| value["coefficients"][j].as_str().unwrap().parse().unwrap());
    a.scalar(coefficients, value["power"].as_i64().unwrap())
        .unwrap()
}

fn phased_basis(
    width: usize,
    label: usize,
    phase: i64,
    power: i64,
    a: &ScalarArithmetic,
) -> AffineKet {
    AffineKet::new(
        bits(width, label),
        vec![],
        QuadraticForm::new(0, vec![], vec![]).unwrap(),
        a.omega(phase, power).unwrap(),
    )
    .unwrap()
}

fn strategies() -> [CompilerStrategy; 2] {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
}

fn unitary(gate: Gate, qubits: &[usize]) -> Operation {
    Operation::Unitary {
        gate,
        qubits: qubits.to_vec(),
        control: None,
    }
}

fn instrument() -> OperationProgram {
    OperationProgram::new(
        2,
        vec![
            unitary(Gate::H, &[0]),
            unitary(Gate::T, &[0]),
            unitary(Gate::Cx, &[0, 1]),
            Operation::Noise {
                kind: NoiseKind::Y,
                probabilities: vec![Probability::one()],
                qubits: vec![0],
                product: None,
                guard: None,
            },
            unitary(Gate::Y, &[1]),
            Operation::Measure {
                product: PauliProduct {
                    factors: vec![(1, Basis::X)],
                    inverted: false,
                },
                reset: false,
                report_flip: Probability::one(),
                record: 0,
            },
            Operation::Reset {
                qubit: 1,
                basis: Basis::X,
            },
            unitary(Gate::Cz, &[0, 1]),
        ],
    )
    .unwrap()
}

fn literal_y(qubit: usize, state: &mut Vec<ExactScalar>, a: &ScalarArithmetic) {
    let mut output = vec![ExactScalar::zero(); state.len()];
    for (label, amplitude) in state.iter().enumerate() {
        let phase = if label >> qubit & 1 == 0 { 2 } else { 6 };
        output[label ^ (1 << qubit)] = a.multiply(amplitude, &a.omega(phase, 0).unwrap()).unwrap();
    }
    *state = output;
}

// Literal isometries retain the reported label and the reset environment.
// MX uses the two 2x2 projectors; reset uses |+>|old>_env, not affine lowering.
fn literal_environment(phase: i64, a: &ScalarArithmetic) -> Vec<ExactScalar> {
    let mut state = vec![ExactScalar::zero(); 4];
    state[0] = a.omega(phase, 0).unwrap();
    dense_gate(CliffordGate::H(0), &mut state, a);
    for (label, amplitude) in state.iter_mut().enumerate() {
        if label & 1 != 0 {
            *amplitude = a.multiply(amplitude, &a.omega(1, 0).unwrap()).unwrap();
        }
    }
    dense_gate(
        CliffordGate::Cx {
            control: 0,
            target: 1,
        },
        &mut state,
        a,
    );
    literal_y(0, &mut state, a); // Fault immediately after CX.
    literal_y(1, &mut state, a);
    let mut measured = vec![ExactScalar::zero(); 8];
    let projectors = [[[1, 1], [1, 1]], [[1, -1], [-1, 1]]];
    let half = a.omega(0, -2).unwrap();
    for (label, amplitude) in state.iter().enumerate() {
        let old = label >> 1 & 1;
        for (truth, matrix) in projectors.iter().enumerate() {
            for (new, row) in matrix.iter().enumerate() {
                let mut term = a.multiply(amplitude, &half).unwrap();
                if row[old] < 0 {
                    term = term.negated();
                }
                let reported = truth ^ 1;
                let target = (label & !2) | (new << 1) | (reported << 2);
                measured[target] = a.add(&measured[target], &term).unwrap();
            }
        }
    }
    let mut retained = vec![ExactScalar::zero(); 16];
    let root_half = a.omega(0, -1).unwrap();
    for (label, amplitude) in measured.iter().enumerate() {
        let old = label >> 1 & 1;
        let term = a.multiply(amplitude, &root_half).unwrap();
        for new in 0..2 {
            let target = (label & !2) | (new << 1) | (old << 3);
            retained[target] = a.add(&retained[target], &term).unwrap();
        }
    }
    dense_gate(CliffordGate::Cz { a: 0, b: 1 }, &mut retained, a);
    retained
}

fn literal_moment(
    state: &[ExactScalar],
    x: usize,
    z: usize,
    event: Option<bool>,
    a: &ScalarArithmetic,
) -> ExactScalar {
    let canonical_phase = 2 * (x & z).count_ones();
    state
        .iter()
        .enumerate()
        .filter(|(label, _)| event.is_none_or(|bit| (*label >> 2 & 1 != 0) == bit))
        .fold(ExactScalar::zero(), |sum, (label, amplitude)| {
            let phase = i64::from(canonical_phase) + 4 * i64::from((label & z).count_ones() & 1);
            let term = a
                .multiply(
                    &state[label ^ x].conjugate(),
                    &a.multiply(amplitude, &a.omega(phase, 0).unwrap()).unwrap(),
                )
                .unwrap();
            a.add(&sum, &term).unwrap()
        })
}

#[test]
fn original_common_phase_with_faults_events_reset_and_retained_labels() {
    let a = ScalarArithmetic::default();
    let data = fixture();
    let program = instrument();
    let masks = [(0, 0), (1, 0), (0, 1), (1, 1), (1, 2), (2, 1)];
    let event = BitExpr::atom(BitAtom::Record(0));
    let faults = [
        FaultSelection {
            site: 0,
            alternative: 0,
        },
        FaultSelection {
            site: 1,
            alternative: 0,
        },
    ];
    let reference = literal_environment(0, &a);
    for (phase, trial) in data["trials"].as_array().unwrap().iter().enumerate() {
        assert_eq!(trial["phase"], phase);
        assert_eq!(trial["moments"].as_array().unwrap().len(), masks.len());
        let dense = literal_environment(phase as i64, &a);
        for strategy in strategies() {
            for mode in [
                QueryPreparation::Trace,
                QueryPreparation::FixedFibers(Default::default()),
            ] {
                let input = phased_basis(2, 0, phase as i64, 0, &a);
                let compiled = compile_operation_program_stabilizer(
                    &program,
                    &input,
                    &CompileOptions {
                        compiler_strategy: strategy,
                        moment_preparation: mode,
                        max_prepared_moments: 2 * masks.len(),
                        ..Default::default()
                    },
                    &a,
                )
                .unwrap();
                assert_eq!(compiled.num_records(), 1);
                assert_eq!(compiled.noise_sites().len(), 2);
                assert_eq!(compiled.noise_sites()[0].source_operations(), [3]);
                assert_eq!(compiled.noise_sites()[1].source_operations(), [5]);
                assert_eq!(
                    compiled.fault_probability(&faults).unwrap(),
                    Probability::one().value().clone()
                );
                // A single immutable retained-label family covers both outcomes.
                // Its two views share the original moment cache, including plans.
                let family = [
                    compiled
                        .with_reported_conditions(&[(event.clone(), false)])
                        .unwrap(),
                    compiled
                        .with_reported_conditions(&[(event.clone(), true)])
                        .unwrap(),
                ];
                for (index, &(x, z)) in masks.iter().enumerate() {
                    let native = &trial["moments"][index];
                    assert_eq!(native["x"], x);
                    assert_eq!(native["z"], z);
                    let word =
                        PauliWord::new(bits(2, x), bits(2, z), (2 * (x & z).count_ones()) as u8)
                            .unwrap();
                    let unselected = literal_moment(&dense, x, z, None, &a);
                    assert_eq!(unselected, literal_moment(&reference, x, z, None, &a));
                    assert_eq!(unselected, scalar(&native["unconditional"], &a));
                    assert_eq!(
                        compiled
                            .pauli_moment(&[0, 1], &word, &faults, 256, &a)
                            .unwrap()
                            .value,
                        unselected
                    );
                    assert_eq!(
                        compiled
                            .prepared_moment_statistics()
                            .unwrap()
                            .cached_queries,
                        2 * index + 1
                    );
                    assert_eq!(
                        family[0]
                            .prepare_query(
                                InstrumentQueryLayout::PauliMoment {
                                    outputs: &[0, 1],
                                    word: &word
                                },
                                256,
                                &a
                            )
                            .unwrap(),
                        QueryPreparationOutcome::Created
                    );
                    assert_eq!(
                        family[1]
                            .prepare_query(
                                InstrumentQueryLayout::PauliMoment {
                                    outputs: &[0, 1],
                                    word: &word
                                },
                                256,
                                &a
                            )
                            .unwrap(),
                        QueryPreparationOutcome::Reused
                    );
                    let prepared = compiled.prepared_moment_statistics().unwrap();
                    assert_eq!(prepared.cached_queries, 2 * index + 2);
                    let selected: [ExactScalar; 2] = std::array::from_fn(|bit| {
                        let expected = literal_moment(&dense, x, z, Some(bit != 0), &a);
                        assert_eq!(
                            expected,
                            literal_moment(&reference, x, z, Some(bit != 0), &a)
                        );
                        assert_eq!(expected, scalar(&native["selected"][bit], &a));
                        let value = family[bit]
                            .pauli_moment(&[0, 1], &word, &faults, 256, &a)
                            .unwrap()
                            .value;
                        assert_eq!(value, expected);
                        assert_eq!(compiled.prepared_moment_statistics().unwrap(), prepared);
                        value
                    });
                    assert_eq!(a.add(&selected[0], &selected[1]).unwrap(), unselected);
                    for bit in [1, 0, 1, 0] {
                        let retained = family[bit]
                            .pauli_moment(&[0, 1], &word, &faults, 256, &a)
                            .unwrap()
                            .value;
                        assert_eq!(retained, scalar(&native["retained"][bit], &a));
                        assert_eq!(retained, selected[bit]);
                        assert_eq!(compiled.prepared_moment_statistics().unwrap(), prepared);
                    }
                }
                assert_eq!(
                    compiled
                        .prepared_moment_statistics()
                        .unwrap()
                        .cached_queries,
                    12
                );
            }
        }
        assert_eq!(literal_moment(&dense, 0, 0, None, &a), ExactScalar::one());
        assert_eq!(
            literal_moment(&dense, 0, 0, Some(false), &a),
            a.omega(0, -2).unwrap()
        );
        // Nonzero joint controls prevent an all-zero moment oracle from passing.
        assert_ne!(
            literal_moment(&dense, 1, 2, Some(false), &a),
            ExactScalar::zero()
        );
        assert_eq!(literal_moment(&dense, 2, 1, None, &a), ExactScalar::one());
    }
}

#[test]
fn original_paired_sign_and_subnormalization_survive() {
    let a = ScalarArithmetic::default();
    let data = fixture();
    for (phase, trial) in data["trials"].as_array().unwrap().iter().enumerate() {
        let ket = phased_basis(1, 1, phase as i64, 0, &a);
        let acted = ket.pauli(&bits(1, 0), &bits(1, 1)).unwrap();
        let original_constant = trial["paired_constant_z8"].as_u64().unwrap();
        assert_eq!(original_constant, 4);
        // No hidden variables: this Z-action constant is the paired sign.
        // Native Z8 turn4 is i^2 in Rust's Z4 quadratic form; omega cancels.
        assert_eq!(ket.phase().constant(), 0);
        assert_eq!(acted.phase().constant(), (original_constant / 2) as u8);
        assert_eq!(acted.phase().constant(), 2);
        assert_eq!(
            ket.overlap(&acted, &a).unwrap(),
            ExactScalar::one().negated()
        );
        assert_eq!(
            ket.overlap(&acted, &a).unwrap(),
            scalar(&trial["negative"], &a)
        );
        let subnormalized = phased_basis(1, 1, phase as i64, -2, &a);
        let quarter = a.omega(0, -4).unwrap();
        assert_eq!(subnormalized.norm_squared(&a).unwrap(), quarter);
        assert_eq!(
            subnormalized.norm_squared(&a).unwrap(),
            scalar(&trial["subnormalized_mass"], &a)
        );
        assert_eq!(
            subnormalized
                .overlap(&subnormalized.pauli(&bits(1, 0), &bits(1, 1)).unwrap(), &a)
                .unwrap(),
            quarter.negated()
        );
    }
}

#[test]
fn original_amplitude_compiler_preserves_all_requested_phases() {
    let a = ScalarArithmetic::default();
    let data = fixture();
    // S X Sdag is the exact Y matrix, applied from right to left.
    let circuit = UnitaryCircuit::new(
        1,
        [
            UnitaryGate::Clifford(CliffordGate::S { qubit: 0, turns: 3 }),
            UnitaryGate::Clifford(CliffordGate::X(0)),
            UnitaryGate::Clifford(CliffordGate::S { qubit: 0, turns: 1 }),
        ],
    )
    .unwrap();
    for (phase, trial) in data["trials"].as_array().unwrap().iter().enumerate() {
        let input = phased_basis(1, 0, phase as i64, 0, &a);
        let mut dense = vec![a.omega(phase as i64, 0).unwrap(), ExactScalar::zero()];
        literal_y(0, &mut dense, &a);
        assert_eq!(dense[0], ExactScalar::zero());
        assert_eq!(dense[1], a.omega(phase as i64 + 2, 0).unwrap());
        for strategy in strategies() {
            let compiled = circuit
                .compile_stabilizer_with_strategy(&input, &a, strategy)
                .unwrap();
            for (label, expected) in dense.iter().enumerate() {
                assert_eq!(*expected, scalar(&trial["amplitudes"][label], &a));
                assert_eq!(
                    compiled.amplitude(&bits(1, label), 256, &a).unwrap().value,
                    *expected
                );
            }
        }
    }
}
