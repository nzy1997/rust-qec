// Dense literal matrices and projectors are independent of coherent lowering.
#[allow(dead_code)]
mod common;

use common::{bits, dense_gate};
use rmagna::circuit::{
    Basis, BitAtom, BitExpr, CircuitView, CircuitViewError, Gate, NoiseKind, Operation,
    OperationProgram, OperationProgramErrorKind as E, OperationProgramOptions, ParseOptions,
    PauliProduct, RegisterWrite, parse_circuit,
};
use rmagna::clifford::CliffordGate as C;
use rmagna::instrument::{
    CompileOptions, CompiledInstrument, FaultSelection, InstrumentError, compile_affine,
    compile_affine_stabilizer, compile_operation_program, compile_operation_program_stabilizer,
    compile_unitary,
};
use rmagna::pauli::PauliWord;
use rmagna::probability::{Probability, ProbabilityError};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar as S, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::{CompilerStrategy, UnitaryError};

const TERMS: usize = 65536;
const STRATEGIES: [CompilerStrategy; 2] = [
    CompilerStrategy::ParityCopies,
    CompilerStrategy::SharedCommutingLayer,
];

fn p(text: &str) -> Probability {
    text.parse().unwrap()
}
fn unitary(gate: Gate, qubits: &[usize]) -> Operation {
    Operation::Unitary {
        gate,
        qubits: qubits.to_vec(),
        control: None,
    }
}
fn expr(records: &[usize], constant: bool) -> BitExpr {
    BitExpr {
        constant,
        terms: records.iter().copied().map(BitAtom::Record).collect(),
    }
}
fn measure(factors: &[(usize, Basis)], reset: bool, record: usize) -> Operation {
    Operation::Measure {
        product: PauliProduct {
            factors: factors.to_vec(),
            inverted: false,
        },
        reset,
        record,
        report_flip: Probability::zero(),
    }
}
fn noise(kind: NoiseKind, probabilities: &[&str], qubits: &[usize]) -> Operation {
    Operation::Noise {
        kind,
        probabilities: probabilities.iter().map(|s| p(s)).collect(),
        qubits: qubits.to_vec(),
        product: None,
        guard: None,
    }
}

fn builtin(
    name: &str,
    inputs: Vec<BitExpr>,
    register: usize,
    version: usize,
    output: BitExpr,
) -> Operation {
    Operation::Control {
        name: name.into(),
        call: None,
        inputs,
        outputs: vec![RegisterWrite {
            register,
            version,
            expression: output,
        }],
    }
}
fn bad(width: usize, operations: Vec<Operation>, operation: usize, expected: E) {
    let failure = OperationProgram::new(width, operations).unwrap_err();
    assert_eq!(failure.operation, Some(operation));
    assert_eq!(failure.kind, expected);
    assert!(
        failure
            .to_string()
            .contains(&format!("operation {operation}:"))
    );
}

#[test]
fn rejects_malformed_quantum_shapes_and_noise_laws_before_compilation() {
    for (gate, targets) in [
        (Gate::H, vec![]),
        (Gate::Cx, vec![0]),
        (Gate::Ccz, vec![0, 1, 2, 3]),
    ] {
        bad(
            4,
            vec![unitary(gate, &targets)],
            0,
            E::GateArity {
                gate,
                expected: gate.arity(),
                actual: targets.len(),
            },
        );
    }
    bad(
        2,
        vec![unitary(Gate::Cx, &[1, 1])],
        0,
        E::RepeatedQubit { qubit: 1 },
    );
    bad(
        2,
        vec![unitary(Gate::H, &[2])],
        0,
        E::QubitOutOfBounds { qubit: 2, width: 2 },
    );
    bad(
        0,
        vec![Operation::Reset {
            qubit: 0,
            basis: Basis::Z,
        }],
        0,
        E::QubitOutOfBounds { qubit: 0, width: 0 },
    );
    bad(
        2,
        vec![measure(&[], false, 0)],
        0,
        E::EmptyMeasurementProduct,
    );
    bad(
        2,
        vec![measure(&[(0, Basis::X), (0, Basis::Z)], false, 0)],
        0,
        E::RepeatedQubit { qubit: 0 },
    );
    bad(
        2,
        vec![measure(&[(0, Basis::X), (1, Basis::Z)], true, 0)],
        0,
        E::ResetMeasurementArity { actual: 2 },
    );
    bad(
        2,
        vec![measure(&[(2, Basis::Y)], false, 0)],
        0,
        E::QubitOutOfBounds { qubit: 2, width: 2 },
    );
    for kind in [
        NoiseKind::X,
        NoiseKind::Y,
        NoiseKind::Z,
        NoiseKind::Depolarize1,
        NoiseKind::Depolarize2,
        NoiseKind::Pauli1,
        NoiseKind::Pauli2,
    ] {
        let count = match kind {
            NoiseKind::Pauli1 => 3,
            NoiseKind::Pauli2 => 15,
            _ => 1,
        };
        bad(
            2,
            vec![noise(kind.clone(), &[], &[0])],
            0,
            E::NoiseProbabilityCount {
                expected: count,
                actual: 0,
            },
        );
        let arity = if matches!(kind, NoiseKind::Depolarize2 | NoiseKind::Pauli2) {
            2
        } else {
            1
        };
        let zeroes = vec!["0"; count];
        bad(
            2,
            vec![noise(kind.clone(), &zeroes, &[])],
            0,
            E::NoiseTargetCount {
                expected: arity,
                actual: 0,
            },
        );
        let qubits = if arity == 2 { vec![0, 1] } else { vec![0] };
        let mut probabilities = zeroes.clone();
        probabilities[0] = "1";
        assert!(OperationProgram::new(2, vec![noise(kind, &probabilities, &qubits)]).is_ok());
    }
    bad(
        2,
        vec![noise(NoiseKind::Pauli1, &["1/2", "1/2", "1/4"], &[0])],
        0,
        E::Probability(ProbabilityError::OutOfRange),
    );
    bad(
        2,
        vec![noise(NoiseKind::Depolarize2, &["1/2"], &[1, 1])],
        0,
        E::RepeatedQubit { qubit: 1 },
    );
    bad(
        2,
        vec![noise(NoiseKind::X, &["1/2"], &[2])],
        0,
        E::QubitOutOfBounds { qubit: 2, width: 2 },
    );
    assert!(Probability::from_ratio((-1).into(), 3.into()).is_err());
    assert!(Probability::from_ratio(4.into(), 3.into()).is_err());
}

#[test]
fn records_are_sequential_and_all_readers_require_strictly_earlier_affine_records() {
    bad(
        1,
        vec![measure(&[(0, Basis::Z)], false, 1)],
        0,
        E::RecordSequence {
            expected: 0,
            actual: 1,
        },
    );
    let first = measure(&[(0, Basis::Z)], false, 0);
    bad(
        1,
        vec![first.clone(), first.clone()],
        1,
        E::RecordSequence {
            expected: 1,
            actual: 0,
        },
    );
    for expression in [expr(&[0], false), expr(&[usize::MAX], true)] {
        let unknown = if expression.terms.contains(&BitAtom::Record(0)) {
            0
        } else {
            usize::MAX
        };
        for reader in [
            Operation::Unitary {
                gate: Gate::X,
                qubits: vec![0],
                control: Some(expression.clone()),
            },
            Operation::Detector {
                index: 0,
                expression: expression.clone(),
                coordinates: vec![],
            },
            Operation::Observable {
                index: 77,
                expression: expression.clone(),
            },
            Operation::Postselect {
                expression: expression.clone(),
            },
        ] {
            bad(
                1,
                vec![reader],
                0,
                E::UnknownRecord {
                    record: unknown,
                    available: 0,
                },
            );
        }
    }
    let decision = BitExpr::atom(BitAtom::Decision { call: 2, bit: 65 });
    for reader in [
        Operation::Unitary {
            gate: Gate::X,
            qubits: vec![0],
            control: Some(decision.clone()),
        },
        Operation::Detector {
            index: 0,
            expression: decision.clone(),
            coordinates: vec![],
        },
        Operation::Observable {
            index: 0,
            expression: decision.clone(),
        },
        Operation::Postselect {
            expression: decision,
        },
    ] {
        bad(1, vec![reader], 0, E::ExternalDecision { call: 2, bit: 65 });
    }
    bad(
        1,
        vec![
            first.clone(),
            Operation::Unitary {
                gate: Gate::T,
                qubits: vec![0],
                control: Some(expr(&[0], false)),
            },
        ],
        1,
        E::UnsupportedControlGate { gate: Gate::T },
    );
    for value in [false, true] {
        assert!(
            OperationProgram::new(
                1,
                vec![Operation::Unitary {
                    gate: Gate::T,
                    qubits: vec![0],
                    control: Some(expr(&[], value)),
                }]
            )
            .is_ok()
        );
    }
    bad(
        1,
        vec![
            first.clone(),
            Operation::Detector {
                index: 1,
                expression: expr(&[0], false),
                coordinates: vec![],
            },
        ],
        1,
        E::DetectorSequence {
            expected: 0,
            actual: 1,
        },
    );
    for coordinate in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        bad(
            1,
            vec![Operation::Detector {
                index: 0,
                expression: expr(&[], false),
                coordinates: vec![coordinate],
            }],
            0,
            E::NonFiniteCoordinates,
        );
    }
    let program = OperationProgram::new(
        1,
        vec![
            first,
            Operation::Detector {
                index: 0,
                expression: expr(&[0], true),
                coordinates: vec![-1.5, 3.0],
            },
            Operation::Observable {
                index: usize::MAX,
                expression: expr(&[0], true),
            },
            Operation::Observable {
                index: usize::MAX,
                expression: expr(&[0], false),
            },
            Operation::Postselect {
                expression: expr(&[], false),
            },
        ],
    )
    .unwrap();
    assert_eq!(program.num_records(), 1);
    assert_eq!(program.detectors(), &[expr(&[0], true)]);
    assert_eq!(program.observables()[&usize::MAX], expr(&[], true));
}

#[test]
fn rejects_external_nodes_guards_correlations_and_annotations_without_ignoring_them() {
    for call in [Some(0), Some(65)] {
        bad(
            0,
            vec![Operation::Control {
                name: "NOT".into(),
                call,
                inputs: vec![],
                outputs: vec![],
            }],
            0,
            E::Unsupported {
                feature: "external control nodes",
            },
        );
    }
    let mut guarded = noise(NoiseKind::X, &["0"], &[0]);
    if let Operation::Noise { guard, .. } = &mut guarded {
        *guard = Some(expr(&[], false));
    }
    bad(
        1,
        vec![guarded],
        0,
        E::Unsupported {
            feature: "guarded noise",
        },
    );
    bad(
        1,
        vec![noise(
            NoiseKind::Correlated {
                chain: 0,
                alternative: 0,
            },
            &["0"],
            &[0],
        )],
        0,
        E::Unsupported {
            feature: "correlated noise",
        },
    );
    let mut extra = noise(NoiseKind::X, &["0"], &[0]);
    if let Operation::Noise { product, .. } = &mut extra {
        *product = Some(PauliProduct {
            factors: vec![(0, Basis::X)],
            inverted: false,
        });
    }
    bad(
        1,
        vec![extra],
        0,
        E::Unsupported {
            feature: "Pauli product on noncorrelated noise",
        },
    );
    bad(
        1,
        vec![Operation::Annotation {
            name: "TICK".into(),
            coordinates: vec![],
            qubits: vec![],
        }],
        0,
        E::Unsupported {
            feature: "annotations",
        },
    );
}

#[test]
fn builtin_controls_validate_resolved_outputs_causality_and_register_versions() {
    bad(
        0,
        vec![builtin("OPAQUE", vec![], 0, 0, expr(&[], false))],
        0,
        E::Unsupported {
            feature: "control names other than builtin XOR/NOT",
        },
    );
    bad(
        0,
        vec![Operation::Control {
            name: "XOR".into(),
            call: None,
            inputs: vec![],
            outputs: vec![],
        }],
        0,
        E::ControlOutputCount { actual: 0 },
    );
    bad(
        0,
        vec![Operation::Control {
            name: "XOR".into(),
            call: None,
            inputs: vec![],
            outputs: vec![
                RegisterWrite {
                    register: 0,
                    version: 0,
                    expression: expr(&[], false),
                },
                RegisterWrite {
                    register: 1,
                    version: 0,
                    expression: expr(&[], false),
                },
            ],
        }],
        0,
        E::ControlOutputCount { actual: 2 },
    );
    for count in [0, 2] {
        bad(
            0,
            vec![builtin(
                "NOT",
                vec![expr(&[], false); count],
                0,
                0,
                expr(&[], true),
            )],
            0,
            E::ControlInputCount {
                name: "NOT",
                expected: 1,
                actual: count,
            },
        );
    }
    bad(
        0,
        vec![builtin(
            "NOT",
            vec![expr(&[], false)],
            0,
            0,
            expr(&[], false),
        )],
        0,
        E::ControlOutputExpression { name: "NOT" },
    );
    bad(
        1,
        vec![
            measure(&[(0, Basis::Z)], false, 0),
            builtin("XOR", vec![expr(&[0], false)], 65, 0, expr(&[], false)),
        ],
        1,
        E::ControlOutputExpression { name: "XOR" },
    );
    // Declared reads remain causal even when two invalid reads would cancel.
    bad(
        0,
        vec![builtin(
            "XOR",
            vec![expr(&[1], false), expr(&[1], false)],
            65,
            0,
            expr(&[], false),
        )],
        0,
        E::UnknownRecord {
            record: 1,
            available: 0,
        },
    );
    bad(
        0,
        vec![builtin("XOR", vec![], 65, 0, expr(&[65], false))],
        0,
        E::UnknownRecord {
            record: 65,
            available: 0,
        },
    );
    let decision = BitExpr::atom(BitAtom::Decision { call: 2, bit: 5 });
    for (inputs, output) in [
        (vec![decision.clone()], expr(&[], false)),
        (vec![], decision),
    ] {
        bad(
            0,
            vec![builtin("XOR", inputs, 0, 0, output)],
            0,
            E::ExternalDecision { call: 2, bit: 5 },
        );
    }
    for version in [1, usize::MAX] {
        bad(
            0,
            vec![builtin("XOR", vec![], 65, version, expr(&[], true))],
            0,
            E::RegisterVersion {
                register: 65,
                expected: 0,
                actual: version,
            },
        );
    }
    bad(
        0,
        vec![
            builtin("XOR", vec![], 65, 0, expr(&[], true)),
            builtin("NOT", vec![expr(&[], true)], 65, 2, expr(&[], false)),
        ],
        1,
        E::RegisterVersion {
            register: 65,
            expected: 1,
            actual: 2,
        },
    );
    let valid = OperationProgram::new(
        0,
        vec![
            builtin("XOR", vec![], 65, 0, expr(&[], true)),
            builtin("NOT", vec![expr(&[], true)], 65, 1, expr(&[], false)),
            builtin(
                "XOR",
                vec![expr(&[], false)],
                usize::MAX,
                0,
                expr(&[], false),
            ),
        ],
    )
    .unwrap();
    let a = ScalarArithmetic::default();
    assert_eq!(
        compile_operation_program(&valid, &CompileOptions::default(), &a)
            .unwrap()
            .acceptance(&[], TERMS, &a)
            .unwrap()
            .value,
        S::one()
    );
}

#[test]
fn builtin_owned_programs_retain_feedback_and_survive_dropping_all_inputs() {
    let a = ScalarArithmetic::default();
    let text = "H 0\nM 0\nCONTROL[NOT] rec[-1] -> ctrl[65]\nCX ctrl[65] 0\nM 0\nCONTROL[XOR](1) rec[-2] rec[-1] -> ctrl[65]\nCZ ctrl[65] 0";
    let parsed = parse_circuit(text, &ParseOptions::default()).unwrap();
    let program = OperationProgram::new(
        1,
        parsed
            .operations()
            .iter()
            .map(|v| v.operation.clone())
            .collect(),
    )
    .unwrap();
    let clone = program.clone();
    let mut compiled = Vec::new();
    for strategy in STRATEGIES {
        let options = CompileOptions {
            compiler_strategy: strategy,
            ..Default::default()
        };
        compiled.push(compile_operation_program(&clone, &options, &a).unwrap());
        compiled.push(compile_affine(&parsed, &options, &a).unwrap());
    }
    drop(clone);
    drop(program);
    drop(parsed);
    let target = compile_unitary(
        &parse_circuit("X 0", &ParseOptions::default()).unwrap(),
        &CompileOptions::default(),
        &a,
    )
    .unwrap();
    for instrument in compiled {
        for outcome in 0..4 {
            // The first result is uniform; NOT feedback always prepares |1>,
            // so the second result is one. The final Z changes only a phase.
            assert_eq!(
                instrument
                    .reported_probability(
                        &[expr(&[0], false), expr(&[1], false)],
                        &bits(2, outcome),
                        &[],
                        TERMS,
                        &a
                    )
                    .unwrap()
                    .value,
                if outcome & 2 != 0 {
                    a.omega(0, -2).unwrap()
                } else {
                    S::zero()
                }
            );
        }
        assert_eq!(
            instrument
                .masses(&[0], &target, &[], TERMS, &a)
                .unwrap()
                .target_success
                .value,
            S::one()
        );
        assert_eq!(
            instrument
                .pauli_moment(
                    &[0],
                    &PauliWord::new(bits(1, 0), bits(1, 1), 0).unwrap(),
                    &[],
                    TERMS,
                    &a
                )
                .unwrap()
                .value,
            S::one().negated()
        );
    }
}

#[test]
fn construction_limits_and_ownership_allow_valid_retry_without_parsed_provenance() {
    let operations = vec![unitary(Gate::H, &[0]), measure(&[(0, Basis::Z)], false, 0)];
    let options = OperationProgramOptions {
        max_qubits: 0,
        ..Default::default()
    };
    let e = OperationProgram::with_options(1, operations.clone(), &options).unwrap_err();
    assert_eq!(e.operation, None);
    assert_eq!(
        e.kind,
        E::ResourceLimit {
            option: "max_qubits",
            limit: 0,
            required: 1
        }
    );
    let options = OperationProgramOptions {
        max_operations: 1,
        ..Default::default()
    };
    assert_eq!(
        OperationProgram::with_options(1, operations.clone(), &options)
            .unwrap_err()
            .kind,
        E::ResourceLimit {
            option: "max_operations",
            limit: 1,
            required: 2
        }
    );
    let options = OperationProgramOptions {
        max_records: 0,
        ..Default::default()
    };
    let e = OperationProgram::with_options(1, operations.clone(), &options).unwrap_err();
    assert_eq!(e.operation, Some(1));
    assert_eq!(
        e.kind,
        E::ResourceLimit {
            option: "max_records",
            limit: 0,
            required: 1
        }
    );
    let mut caller_owned = operations.clone();
    let program = OperationProgram::new(1, caller_owned.clone()).unwrap();
    caller_owned.clear();
    assert_eq!(program.operations(), operations);
    assert_eq!(program.clone().operations(), operations);
    let parsed = parse_circuit("H 0\nM 0", &ParseOptions::default()).unwrap();
    let view = CircuitView::from(&parsed).with_operations(parsed.operations().to_vec());
    assert!(matches!(
        view.parsed_circuit(),
        Err(CircuitViewError::NoParsedForm)
    ));
    assert_eq!(view.to_text(), Err(CircuitViewError::NoParsedForm));
    assert_eq!(view.source_snapshot(), "H 0\nM 0");
    assert_eq!(parsed.to_text(), "H 0\nM 0");
}

fn dense_unitary(gate: Gate, q: &[usize], state: &mut Vec<S>, a: &ScalarArithmetic) {
    match gate {
        Gate::I => {}
        Gate::X => dense_gate(C::X(q[0]), state, a),
        Gate::H => dense_gate(C::H(q[0]), state, a),
        Gate::S => dense_gate(
            C::S {
                qubit: q[0],
                turns: 1,
            },
            state,
            a,
        ),
        Gate::Cx => dense_gate(
            C::Cx {
                control: q[0],
                target: q[1],
            },
            state,
            a,
        ),
        Gate::T | Gate::Cs | Gate::CsDag | Gate::Ccz => {
            for (label, value) in state.iter_mut().enumerate() {
                let bit = |j| ((label >> q[j]) & 1) as i64;
                let phase = match gate {
                    Gate::T => bit(0),
                    Gate::Cs => 2 * bit(0) * bit(1),
                    Gate::CsDag => 6 * bit(0) * bit(1),
                    Gate::Ccz => 4 * bit(0) * bit(1) * bit(2),
                    _ => unreachable!(),
                };
                *value = a.multiply(value, &a.omega(phase, 0).unwrap()).unwrap();
            }
        }
        _ => panic!("oracle fixture uses no other gate"),
    }
}
fn dense_pauli(state: &[S], factors: &[(usize, Basis)], a: &ScalarArithmetic) -> Vec<S> {
    let mut out = vec![S::zero(); state.len()];
    for (label, value) in state.iter().enumerate() {
        let mut target = label;
        let mut phase = 0;
        for &(q, basis) in factors {
            if basis != Basis::Z {
                target ^= 1 << q;
            }
            if basis != Basis::X {
                phase += 4 * ((label >> q) & 1) as i64;
            }
            if basis == Basis::Y {
                phase += 2;
            }
        }
        out[target] = a.multiply(value, &a.omega(phase, 0).unwrap()).unwrap();
    }
    out
}
fn inner(left: &[S], right: &[S], a: &ScalarArithmetic) -> S {
    left.iter().zip(right).fold(S::zero(), |sum, (l, r)| {
        a.add(&sum, &a.multiply(&l.conjugate(), r).unwrap())
            .unwrap()
    })
}
fn norm(state: &[S], a: &ScalarArithmetic) -> S {
    state.iter().fold(S::zero(), |sum, x| {
        a.add(&sum, &a.norm_squared(x).unwrap()).unwrap()
    })
}
#[derive(Clone)]
struct Branch {
    records: Vec<bool>,
    state: Vec<S>,
}
fn evaluate(expression: &BitExpr, records: &[bool]) -> bool {
    expression
        .terms
        .iter()
        .fold(expression.constant, |v, atom| match atom {
            BitAtom::Record(r) => v ^ records[*r],
            _ => unreachable!(),
        })
}
// A reset traces the old physical Z coordinate into separate Kraus branches,
// then prepares the requested basis. No coherent ancilla or wire relabeling.
fn reset(branch: Branch, q: usize, basis: Basis, a: &ScalarArithmetic) -> Vec<Branch> {
    (0..2)
        .map(|bit| {
            let mut state = vec![S::zero(); branch.state.len()];
            for (label, v) in branch.state.iter().enumerate() {
                if label >> q & 1 == bit {
                    state[label & !(1 << q)] = v.clone();
                }
            }
            if basis != Basis::Z {
                dense_gate(C::H(q), &mut state, a);
            }
            if basis == Basis::Y {
                dense_gate(C::S { qubit: q, turns: 1 }, &mut state, a);
            }
            Branch {
                records: branch.records.clone(),
                state,
            }
        })
        .collect()
}
fn branches(
    program: &OperationProgram,
    initial: Vec<S>,
    selected_sites: usize,
    a: &ScalarArithmetic,
) -> Vec<Branch> {
    let mut branches = vec![Branch {
        records: vec![],
        state: initial,
    }];
    let mut site = 0;
    for op in program.operations() {
        match op {
            Operation::Unitary {
                gate,
                qubits,
                control,
            } => {
                for branch in &mut branches {
                    if control
                        .as_ref()
                        .is_none_or(|e| evaluate(e, &branch.records))
                    {
                        dense_unitary(*gate, qubits, &mut branch.state, a);
                    }
                }
            }
            Operation::Reset { qubit, basis } => {
                branches = branches
                    .into_iter()
                    .flat_map(|b| reset(b, *qubit, *basis, a))
                    .collect();
            }
            Operation::Measure {
                product,
                reset: do_reset,
                ..
            } => {
                let mut next = vec![];
                for b in branches {
                    let projected = dense_pauli(&b.state, &product.factors, a);
                    for result in [false, true] {
                        let state = b
                            .state
                            .iter()
                            .zip(&projected)
                            .map(|(v, pv)| {
                                let pv = if result ^ product.inverted {
                                    pv.negated()
                                } else {
                                    pv.clone()
                                };
                                a.multiply(&a.add(v, &pv).unwrap(), &a.omega(0, -2).unwrap())
                                    .unwrap()
                            })
                            .collect();
                        let mut records = b.records.clone();
                        records.push(result ^ (selected_sites >> site & 1 != 0));
                        let branch = Branch { records, state };
                        if *do_reset {
                            let (q, basis) = product.factors[0];
                            next.extend(reset(branch, q, basis, a));
                        } else {
                            next.push(branch);
                        }
                    }
                }
                branches = next;
                site += 1;
            }
            Operation::Noise {
                kind: NoiseKind::X,
                qubits,
                ..
            } => {
                if selected_sites >> site & 1 != 0 {
                    for branch in &mut branches {
                        dense_gate(C::X(qubits[0]), &mut branch.state, a);
                    }
                }
                site += 1;
            }
            Operation::Detector { .. }
            | Operation::Observable { .. }
            | Operation::Postselect { .. } => {}
            _ => panic!("oracle fixture uses no other operation"),
        }
    }
    branches
}
fn accepted(program: &OperationProgram, branch: &Branch) -> bool {
    program.operations().iter().all(|op| match op {
        Operation::Postselect { expression } => !evaluate(expression, &branch.records),
        _ => true,
    })
}
fn projection(state: &[S], outputs: &[usize], target: &[S], a: &ScalarArithmetic) -> S {
    let output_mask = outputs.iter().fold(0, |v, q| v | (1 << q));
    let mut total = S::zero();
    for env in 0..state.len() {
        if env & output_mask != 0 {
            continue;
        }
        let mut overlap = S::zero();
        for (local, value) in target.iter().enumerate() {
            let full = outputs
                .iter()
                .enumerate()
                .fold(env, |v, (j, q)| v | (((local >> j) & 1) << q));
            overlap = a
                .add(
                    &overlap,
                    &a.multiply(&value.conjugate(), &state[full]).unwrap(),
                )
                .unwrap();
        }
        total = a.add(&total, &a.norm_squared(&overlap).unwrap()).unwrap();
    }
    total
}

#[test]
fn noncommuting_owned_unitaries_match_literal_dense_matrices_under_both_strategies() {
    let a = ScalarArithmetic::default();
    let text = "H 0 1\nT 0\nCS 0 1\nH 0 2\nCCZ 0 1 2\nT 1\nH 1\nCS_DAG 1 2\nH 2\nCX 2 0";
    let operations = vec![
        unitary(Gate::H, &[0]),
        unitary(Gate::H, &[1]),
        unitary(Gate::T, &[0]),
        unitary(Gate::Cs, &[0, 1]),
        unitary(Gate::H, &[0]),
        unitary(Gate::H, &[2]),
        unitary(Gate::Ccz, &[0, 1, 2]),
        unitary(Gate::T, &[1]),
        unitary(Gate::H, &[1]),
        unitary(Gate::CsDag, &[1, 2]),
        unitary(Gate::H, &[2]),
        unitary(Gate::Cx, &[2, 0]),
    ];
    let program = OperationProgram::new(3, operations).unwrap();
    let mut state = vec![S::zero(); 8];
    state[0] = S::one();
    for op in program.operations() {
        if let Operation::Unitary { gate, qubits, .. } = op {
            dense_unitary(*gate, qubits, &mut state, &a);
        }
    }
    assert_eq!(norm(&state, &a), S::one());
    for strategy in STRATEGIES {
        let options = CompileOptions {
            compiler_strategy: strategy,
            ..Default::default()
        };
        let compiled = compile_operation_program(&program, &options, &a).unwrap();
        let parsed = compile_affine(
            &parse_circuit(text, &ParseOptions::default()).unwrap(),
            &options,
            &a,
        )
        .unwrap();
        for (label, amplitude) in state.iter().enumerate() {
            let target_source = (0..3)
                .map(|q| format!("{} {q}", if label >> q & 1 == 0 { "I" } else { "X" }))
                .collect::<Vec<_>>()
                .join("\n");
            let target = compile_unitary(
                &parse_circuit(&target_source, &ParseOptions::default()).unwrap(),
                &options,
                &a,
            )
            .unwrap();
            let wanted = a.norm_squared(amplitude).unwrap();
            for instrument in [&compiled, &parsed] {
                assert_eq!(
                    instrument
                        .masses(&[0, 1, 2], &target, &[], TERMS, &a)
                        .unwrap()
                        .target_success
                        .value,
                    wanted
                );
            }
        }
        for factors in [
            vec![],
            vec![(0, Basis::X)],
            vec![(1, Basis::Y)],
            vec![(0, Basis::Y), (2, Basis::X)],
            vec![(0, Basis::Z), (1, Basis::Z), (2, Basis::Z)],
        ] {
            let mut x = bits(3, 0);
            let mut z = bits(3, 0);
            let mut phase = 0;
            for &(q, b) in &factors {
                if b != Basis::Z {
                    x.set(q, true).unwrap();
                }
                if b != Basis::X {
                    z.set(q, true).unwrap();
                }
                if b == Basis::Y {
                    phase += 2;
                }
            }
            let word = PauliWord::new(x, z, phase).unwrap();
            let wanted = inner(&state, &dense_pauli(&state, &factors, &a), &a);
            for instrument in [&compiled, &parsed] {
                assert_eq!(
                    instrument
                        .pauli_moment(&[0, 1, 2], &word, &[], TERMS, &a)
                        .unwrap()
                        .value,
                    wanted
                );
            }
        }
    }
}

fn instrument_fixture() -> (OperationProgram, &'static str) {
    let operations = vec![
        noise(NoiseKind::X, &["1/7"], &[1]),
        unitary(Gate::H, &[0]),
        unitary(Gate::T, &[0]),
        unitary(Gate::H, &[0]),
        unitary(Gate::H, &[1]),
        unitary(Gate::Cx, &[1, 2]),
        unitary(Gate::Cs, &[0, 1]),
        unitary(Gate::H, &[2]),
        unitary(Gate::Ccz, &[0, 1, 2]),
        Operation::Measure {
            product: PauliProduct {
                factors: vec![(0, Basis::Y), (1, Basis::X)],
                inverted: true,
            },
            reset: false,
            record: 0,
            report_flip: p("1/3"),
        },
        Operation::Reset {
            qubit: 1,
            basis: Basis::Z,
        },
        Operation::Unitary {
            gate: Gate::S,
            qubits: vec![2],
            control: Some(expr(&[0], true)),
        },
        Operation::Measure {
            product: PauliProduct {
                factors: vec![(2, Basis::Z)],
                inverted: false,
            },
            reset: true,
            record: 1,
            report_flip: p("1/5"),
        },
        Operation::Unitary {
            gate: Gate::Cx,
            qubits: vec![0, 1],
            control: Some(expr(&[0, 1], false)),
        },
        unitary(Gate::H, &[0]),
        unitary(Gate::T, &[2]),
        Operation::Detector {
            index: 0,
            expression: expr(&[0], false),
            coordinates: vec![],
        },
        Operation::Observable {
            index: 7,
            expression: expr(&[0], false),
        },
        Operation::Observable {
            index: 7,
            expression: expr(&[0, 1], true),
        },
        Operation::Postselect {
            expression: expr(&[1], false),
        },
    ];
    (
        OperationProgram::new(3, operations).unwrap(),
        "X_ERROR(1/7) 1\nH 0\nT 0\nH 0 1\nCX 1 2\nCS 0 1\nH 2\nCCZ 0 1 2\nMPP(1/3) !Y0*X1\nR 1\nCS !rec[-1] 2\nMR(1/5) 2\nCONTROL[XOR] rec[-2] rec[-1] -> ctrl[0]\nCCX ctrl[0] 0 1\nH 0\nT 2\nDETECTOR rec[-2]\nOBSERVABLE_INCLUDE(7) rec[-2]\nCONTROL[XOR](1) rec[-2] rec[-1] -> ctrl[1]\nOBSERVABLE_INCLUDE(7) ctrl[1]\nCONTROL[XOR] rec[-1] -> ctrl[2]\nPOSTSELECT ctrl[2]",
    )
}

fn check_instrument(
    program: &OperationProgram,
    compiled: &CompiledInstrument,
    bs: &[Branch],
    fault_mask: usize,
    a: &ScalarArithmetic,
) {
    let faults = (0..3)
        .filter(|site| fault_mask >> site & 1 != 0)
        .map(|site| FaultSelection {
            site,
            alternative: 0,
        })
        .collect::<Vec<_>>();
    let wanted_acceptance = bs
        .iter()
        .filter(|b| accepted(program, b))
        .fold(S::zero(), |v, b| a.add(&v, &norm(&b.state, a)).unwrap());
    assert_eq!(
        compiled.acceptance(&faults, TERMS, a).unwrap().value,
        wanted_acceptance
    );
    for outcome in 0..4 {
        let wanted = bs
            .iter()
            .filter(|b| b.records == vec![outcome & 1 != 0, outcome & 2 != 0])
            .fold(S::zero(), |v, b| a.add(&v, &norm(&b.state, a)).unwrap());
        assert_eq!(
            compiled
                .reported_probability(
                    &[expr(&[0], false), expr(&[1], false)],
                    &bits(2, outcome),
                    &faults,
                    TERMS,
                    a
                )
                .unwrap()
                .value,
            wanted
        );
    }
    let options = CompileOptions::default();
    let target = compile_unitary(
        &parse_circuit("H 0\nCX 0 1", &ParseOptions::default()).unwrap(),
        &options,
        a,
    )
    .unwrap();
    let target_dense = vec![
        a.omega(0, -1).unwrap(),
        S::zero(),
        S::zero(),
        a.omega(0, -1).unwrap(),
    ];
    let wanted = bs
        .iter()
        .filter(|b| accepted(program, b))
        .fold(S::zero(), |v, b| {
            a.add(&v, &projection(&b.state, &[2, 0], &target_dense, a))
                .unwrap()
        });
    assert_eq!(
        compiled
            .masses(&[2, 0], &target, &faults, TERMS, a)
            .unwrap()
            .target_success
            .value,
        wanted
    );
    for (factors, word) in [
        (vec![], PauliWord::new(bits(2, 0), bits(2, 0), 0).unwrap()),
        (
            vec![(2, Basis::Y), (0, Basis::X)],
            PauliWord::new(bits(2, 3), bits(2, 1), 2).unwrap(),
        ),
        (
            vec![(0, Basis::Z)],
            PauliWord::new(bits(2, 0), bits(2, 2), 0).unwrap(),
        ),
    ] {
        let wanted = bs
            .iter()
            .filter(|b| accepted(program, b))
            .fold(S::zero(), |v, b| {
                a.add(&v, &inner(&b.state, &dense_pauli(&b.state, &factors, a), a))
                    .unwrap()
            });
        assert_eq!(
            compiled
                .pauli_moment(&[2, 0], &word, &faults, TERMS, a)
                .unwrap()
                .value,
            wanted
        );
    }
}

#[test]
fn mid_measurements_reset_feedback_and_selection_match_dense_kraus_branches() {
    let a = ScalarArithmetic::default();
    let (program, text) = instrument_fixture();
    let parsed = parse_circuit(text, &ParseOptions::default()).unwrap();
    let from_parsed = OperationProgram::new(
        3,
        parsed
            .operations()
            .iter()
            .map(|v| v.operation.clone())
            .collect(),
    )
    .unwrap();
    let input = AffineKet::new(
        bits(3, 0),
        vec![bits(3, 7)],
        QuadraticForm::new(0, vec![1], vec![bits(1, 0)]).unwrap(),
        a.omega(0, -1).unwrap(),
    )
    .unwrap();
    let mut entangled = vec![S::zero(); 8];
    entangled[0] = a.omega(0, -1).unwrap();
    entangled[7] = a.omega(2, -1).unwrap();
    let mut zero = vec![S::zero(); 8];
    zero[0] = S::one();
    for (explicit, initial) in [(false, zero), (true, entangled)] {
        let expected = (0..8)
            .map(|mask| branches(&program, initial.clone(), mask, &a))
            .collect::<Vec<_>>();
        for bs in &expected {
            assert_eq!(
                bs.iter()
                    .fold(S::zero(), |s, b| a.add(&s, &norm(&b.state, &a)).unwrap()),
                S::one()
            );
        }
        for strategy in STRATEGIES {
            let options = CompileOptions {
                compiler_strategy: strategy,
                ..Default::default()
            };
            let owned = if explicit {
                compile_operation_program_stabilizer(&program, &input, &options, &a)
            } else {
                compile_operation_program(&program, &options, &a)
            }
            .unwrap();
            let parsed = if explicit {
                compile_affine_stabilizer(&parsed, &input, &options, &a)
            } else {
                compile_affine(&parsed, &options, &a)
            }
            .unwrap();
            assert_eq!(
                owned
                    .noise_sites()
                    .iter()
                    .map(|s| s.source_operations())
                    .collect::<Vec<_>>(),
                vec![&[0][..], &[9][..], &[12][..]]
            );
            for (mask, bs) in expected.iter().enumerate() {
                check_instrument(&program, &owned, bs, mask, &a);
                check_instrument(&program, &parsed, bs, mask, &a);
            }
            let converted = if explicit {
                compile_operation_program_stabilizer(&from_parsed, &input, &options, &a)
            } else {
                compile_operation_program(&from_parsed, &options, &a)
            }
            .unwrap();
            for (mask, bs) in expected.iter().enumerate() {
                check_instrument(&program, &converted, bs, mask, &a);
            }
        }
    }
}

#[test]
fn zero_width_constant_events_and_cross_word_labels_are_real_compiler_inputs() {
    let a = ScalarArithmetic::default();
    for constant in [false, true] {
        let program = OperationProgram::new(
            0,
            vec![Operation::Postselect {
                expression: expr(&[], constant),
            }],
        )
        .unwrap();
        let compiled = compile_operation_program(&program, &CompileOptions::default(), &a).unwrap();
        assert_eq!(compiled.num_qubits(), 0);
        assert_eq!(
            compiled.acceptance(&[], TERMS, &a).unwrap().value,
            if constant { S::zero() } else { S::one() }
        );
    }
    let mut operations = vec![unitary(Gate::X, &[64])];
    for record in 0..66 {
        operations.push(measure(&[(64, Basis::Z)], false, record));
    }
    operations.push(Operation::Unitary {
        gate: Gate::X,
        qubits: vec![0],
        control: Some(expr(&[0, 64, 65], false)),
    });
    operations.push(Operation::Postselect {
        expression: expr(&[0, 64, 65], true),
    });
    let program = OperationProgram::new(66, operations).unwrap();
    for strategy in STRATEGIES {
        let options = CompileOptions {
            compiler_strategy: strategy,
            ..Default::default()
        };
        let compiled = compile_operation_program(&program, &options, &a).unwrap();
        assert_eq!(compiled.num_qubits(), 66);
        assert_eq!(compiled.num_records(), 66);
        assert_eq!(compiled.acceptance(&[], TERMS, &a).unwrap().value, S::one());
        assert_eq!(
            compiled
                .reported_probability(&[expr(&[0, 64, 65], false)], &bits(1, 1), &[], TERMS, &a)
                .unwrap()
                .value,
            S::one()
        );
        assert_eq!(
            compiled
                .pauli_moment(
                    &[0, 64],
                    &PauliWord::new(bits(2, 0), bits(2, 3), 0).unwrap(),
                    &[],
                    TERMS,
                    &a
                )
                .unwrap()
                .value,
            S::one()
        );
    }
}

#[test]
fn explicit_input_checks_and_lowering_limits_leave_program_reusable() {
    let a = ScalarArithmetic::default();
    let program = OperationProgram::new(1, vec![unitary(Gate::H, &[0])]).unwrap();
    let original = program.operations().to_vec();
    let wrong_width = AffineKet::new(
        bits(0, 0),
        vec![],
        QuadraticForm::new(0, vec![], vec![]).unwrap(),
        S::one(),
    )
    .unwrap();
    assert!(matches!(
        compile_operation_program_stabilizer(
            &program,
            &wrong_width,
            &CompileOptions::default(),
            &a
        ),
        Err(InstrumentError::Unitary(UnitaryError::DimensionMismatch))
    ));
    let unnormalized = AffineKet::new(
        bits(1, 0),
        vec![bits(1, 1)],
        QuadraticForm::new(0, vec![0], vec![bits(1, 0)]).unwrap(),
        S::one(),
    )
    .unwrap();
    assert!(matches!(
        compile_operation_program_stabilizer(
            &program,
            &unnormalized,
            &CompileOptions::default(),
            &a
        ),
        Err(InstrumentError::Unitary(
            UnitaryError::UnnormalizedStabilizerInput
        ))
    ));
    for options in [
        CompileOptions {
            max_gates: 0,
            ..Default::default()
        },
        CompileOptions {
            max_path_bits: 0,
            ..Default::default()
        },
        CompileOptions {
            max_coherent_qubits: 0,
            ..Default::default()
        },
    ] {
        assert!(compile_operation_program(&program, &options, &a).is_err());
        assert_eq!(program.operations(), original);
    }
    assert!(compile_operation_program(&program, &CompileOptions::default(), &a).is_ok());
    assert_eq!(program.operations(), original);
}
