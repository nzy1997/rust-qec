// SPDX-License-Identifier: MPL-2.0
// Four complete ReferenceLayerCompilerTests methods at itpyi/magna
// 56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_reference_layer_compiler.py.
#[allow(dead_code)]
mod common;
use common::{bits, dense_gate};
use rmagna::clifford::CliffordGate as C;
use rmagna::pauli::PauliWord;
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::{
    CompiledUnitary, CompilerStrategy, PauliInsertion, UnitaryCircuit, UnitaryGate as U,
};
use serde_json::Value;

#[derive(Clone, Debug)]
struct Operation {
    kind: &'static str,
    qubits: Vec<usize>,
    turns: i64,
}
fn op(kind: &'static str, qubits: &[usize], turns: i64) -> Operation {
    Operation {
        kind,
        qubits: qubits.to_vec(),
        turns,
    }
}
type Fault = (usize, usize, char);
const COMPILERS: [CompilerStrategy; 2] = [
    CompilerStrategy::ParityCopies,
    CompilerStrategy::SharedCommutingLayer,
];
fn lower(
    width: usize,
    operations: &[Operation],
    compiler: CompilerStrategy,
) -> (CompiledUnitary, Vec<usize>) {
    let mut gates = vec![];
    let mut ends = vec![];
    for operation in operations {
        let q = &operation.qubits;
        match operation.kind {
            "H" => gates.push(U::Clifford(C::H(q[0]))),
            "X" => gates.push(U::Clifford(C::X(q[0]))),
            "Z" => gates.push(U::Clifford(C::Z(q[0]))),
            "Y" => {
                gates.extend([
                    U::Clifford(C::S {
                        qubit: q[0],
                        turns: 3,
                    }),
                    U::Clifford(C::X(q[0])),
                    U::Clifford(C::S {
                        qubit: q[0],
                        turns: 1,
                    }),
                ]);
            }
            "CX" => gates.push(U::Clifford(C::Cx {
                control: q[0],
                target: q[1],
            })),
            "CZ" => gates.push(U::Clifford(C::Cz { a: q[0], b: q[1] })),
            "CCZ" => gates.push(U::Ccz {
                a: q[0],
                b: q[1],
                c: q[2],
            }),
            "PHASE" => {
                let target = *q.last().unwrap();
                for &control in &q[..q.len() - 1] {
                    gates.push(U::Clifford(C::Cx { control, target }));
                }
                gates.push(U::T {
                    qubit: target,
                    turns: operation.turns.rem_euclid(8) as u8,
                });
                for &control in q[..q.len() - 1].iter().rev() {
                    gates.push(U::Clifford(C::Cx { control, target }));
                }
            }
            other => panic!("unknown literal operation {other}"),
        }
        // Faults are after an entire original operation, including parity
        // uncomputation and the exact S-dagger/X/S implementation of Y.
        ends.push(gates.len());
    }
    let arithmetic = ScalarArithmetic::default();
    let input = AffineKet::new(
        bits(width, 0),
        vec![],
        QuadraticForm::new(0, vec![], vec![]).unwrap(),
        arithmetic.omega(3, 0).unwrap(),
    )
    .unwrap();
    (
        UnitaryCircuit::new(width, gates)
            .unwrap()
            .compile_stabilizer_with_strategy(&input, &arithmetic, compiler)
            .unwrap(),
        ends,
    )
}
fn pauli(values: &mut Vec<ExactScalar>, q: usize, kind: char, arithmetic: &ScalarArithmetic) {
    let mut out = vec![ExactScalar::zero(); values.len()];
    for (word, value) in values.iter().enumerate() {
        let phase = if kind == 'Y' { 2 } else { 0 }
            + if kind == 'Y' || kind == 'Z' {
                4 * ((word >> q) & 1) as i64
            } else {
                0
            };
        let target = if kind == 'X' || kind == 'Y' {
            word ^ (1 << q)
        } else {
            word
        };
        out[target] = arithmetic
            .multiply(value, &arithmetic.omega(phase, 0).unwrap())
            .unwrap();
    }
    *values = out;
}
fn literal(width: usize, operations: &[Operation], faults: &[Fault]) -> Vec<ExactScalar> {
    let a = ScalarArithmetic::default();
    let mut values = vec![ExactScalar::zero(); 1 << width];
    values[0] = a.omega(3, 0).unwrap();
    for (index, operation) in operations.iter().enumerate() {
        let q = &operation.qubits;
        match operation.kind {
            "H" => dense_gate(C::H(q[0]), &mut values, &a),
            "CX" => dense_gate(
                C::Cx {
                    control: q[0],
                    target: q[1],
                },
                &mut values,
                &a,
            ),
            "CZ" => dense_gate(C::Cz { a: q[0], b: q[1] }, &mut values, &a),
            "X" | "Y" | "Z" => pauli(
                &mut values,
                q[0],
                operation.kind.chars().next().unwrap(),
                &a,
            ),
            "PHASE" | "CCZ" => {
                for (word, value) in values.iter_mut().enumerate() {
                    let phase = if operation.kind == "CCZ" {
                        4 * i64::from(q.iter().all(|q| word >> q & 1 != 0))
                    } else {
                        operation.turns
                            * (q.iter().map(|q| (word >> q) & 1).sum::<usize>() % 2) as i64
                    };
                    *value = a.multiply(value, &a.omega(phase, 0).unwrap()).unwrap();
                }
            }
            other => panic!("unknown independent matrix operation {other}"),
        }
        for &(location, wire, kind) in faults {
            if location == index {
                pauli(&mut values, wire, kind, &a);
            }
        }
    }
    values
}
fn assignment(width: usize, ends: &[usize], faults: &[Fault]) -> Vec<PauliInsertion> {
    faults
        .iter()
        .map(|&(location, wire, kind)| PauliInsertion {
            before_gate: ends[location],
            word: PauliWord::new(
                bits(
                    width,
                    if kind == 'X' || kind == 'Y' {
                        1 << wire
                    } else {
                        0
                    },
                ),
                bits(
                    width,
                    if kind == 'Z' || kind == 'Y' {
                        1 << wire
                    } else {
                        0
                    },
                ),
                if kind == 'Y' { 2 } else { 0 },
            )
            .unwrap(),
        })
        .collect()
}
fn amplitudes(compiled: &CompiledUnitary, ends: &[usize], faults: &[Fault]) -> Vec<ExactScalar> {
    let a = ScalarArithmetic::default();
    let view = compiled
        .with_insertions(&assignment(compiled.width(), ends, faults), &a)
        .unwrap();
    (0..1 << compiled.width())
        .map(|word| {
            view.amplitude(&bits(compiled.width(), word), 65536, &a)
                .unwrap()
                .value
        })
        .collect()
}
fn check_all_original_faults(
    width: usize,
    operations: &[Operation],
    compiler: CompilerStrategy,
) -> CompiledUnitary {
    let (compiled, ends) = lower(width, operations, compiler);
    assert_eq!(
        amplitudes(&compiled, &ends, &[]),
        literal(width, operations, &[])
    );
    for location in 0..operations.len() {
        for wire in 0..width {
            for kind in ['X', 'Y', 'Z'] {
                let faults = [(location, wire, kind)];
                assert_eq!(
                    amplitudes(&compiled, &ends, &faults),
                    literal(width, operations, &faults),
                    "compiler={compiler:?},location={location},wire={wire},kind={kind}"
                );
            }
        }
    }
    if operations.len() > 1 {
        let faults = [(0, 0, 'Y'), (operations.len() - 1, 0, 'X')];
        assert_eq!(
            amplitudes(&compiled, &ends, &faults),
            literal(width, operations, &faults)
        );
    }
    compiled
}
#[test]
fn original_first_middle_last_and_tie_reference_layers() {
    let h = op("H", &[0], 0);
    let t = op("PHASE", &[0], 1);
    let cases = [
        (
            0,
            2,
            2,
            1,
            vec![h.clone(), t.clone(), t.clone(), h.clone(), t.clone()],
        ),
        (
            1,
            2,
            2,
            1,
            vec![h.clone(), t.clone(), h.clone(), t.clone(), t.clone()],
        ),
        (
            1,
            3,
            3,
            2,
            vec![
                h.clone(),
                t.clone(),
                h.clone(),
                t.clone(),
                op("PHASE", &[1], -1),
                t.clone(),
                h.clone(),
                t.clone(),
            ],
        ),
        (
            0,
            3,
            1,
            2,
            vec![t.clone(), h.clone(), t.clone(), h.clone(), t.clone()],
        ),
    ];
    for compiler in COMPILERS {
        for (selected, layers, retained, ancillas, operations) in &cases {
            let compiled = check_all_original_faults(2, operations, compiler);
            if compiler == CompilerStrategy::SharedCommutingLayer {
                let diagnostics = compiled.compilation();
                assert_eq!(diagnostics.reference_layer, Some(*selected));
                assert_eq!(diagnostics.layer_slots.len(), *layers);
                assert_eq!(diagnostics.reference_layer_slots, *retained);
                assert_eq!(diagnostics.phase_ancillas, *ancillas);
            }
        }
    }
}
#[test]
fn original_dependent_constant_inverse_and_ccz_phases() {
    let cases = [
        vec![op("PHASE", &[0], 1), op("PHASE", &[0], -1)],
        vec![
            op("H", &[0], 0),
            op("PHASE", &[0], 1),
            op("PHASE", &[0], -1),
        ],
        vec![
            op("H", &[0], 0),
            op("H", &[1], 0),
            op("H", &[2], 0),
            op("CCZ", &[0, 1, 2], 0),
        ],
    ];
    for compiler in COMPILERS {
        for operations in &cases {
            let compiled = check_all_original_faults(3, operations, compiler);
            if compiler == CompilerStrategy::SharedCommutingLayer {
                assert_eq!(compiled.compilation().phase_ancillas, 0);
                assert!(compiled.compilation().shared_phase_coordinates <= 3);
            }
        }
        let compiled = check_all_original_faults(
            1,
            &[op("H", &[0], 0), op("X", &[0], 0), op("PHASE", &[0], 2)],
            compiler,
        );
        if compiler == CompilerStrategy::SharedCommutingLayer {
            assert!(compiled.compilation().layer_slots.is_empty());
        }
    }
}
#[test]
fn original_small_ordered_random_reference_circuits() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/original_reference_layer_schedule.json"
    ))
    .unwrap();
    assert_eq!(
        fixture["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(fixture["seed"], 260930);
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 24);
    for row in cases {
        assert_eq!(row["width"], 2);
        let operations = row["operations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                let kind = match entry[0].as_str().unwrap() {
                    "H" => "H",
                    "PHASE" => "PHASE",
                    "CX" => "CX",
                    "CZ" => "CZ",
                    "X" => "X",
                    "Y" => "Y",
                    "Z" => "Z",
                    _ => panic!("invalid original operation"),
                };
                op(
                    kind,
                    &entry[1]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_u64().unwrap() as usize)
                        .collect::<Vec<_>>(),
                    entry[2].as_i64().unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(operations.len(), 8);
        for compiler in COMPILERS {
            check_all_original_faults(2, &operations, compiler);
        }
    }
}
#[test]
fn original_compact_suffix_preserves_ordered_fault_phases() {
    let operations = [
        op("H", &[0], 0),
        op("PHASE", &[0, 1], 3),
        op("Y", &[1], 0),
        op("CX", &[1, 0], 0),
        op("PHASE", &[0], -2),
        op("H", &[1], 0),
        op("PHASE", &[0, 1], -1),
        op("CZ", &[0, 1], 0),
    ];
    let cases = [
        vec![(1, 0, 'X'), (1, 0, 'Y')],
        vec![(1, 0, 'Y'), (1, 0, 'X')],
        vec![(1, 1, 'Z'), (1, 1, 'Y'), (1, 1, 'X')],
        vec![(0, 0, 'Y'), (3, 1, 'X'), (6, 0, 'Z'), (7, 1, 'Y')],
        vec![(2, 1, 'Y'), (2, 1, 'Y')],
        vec![(0, 0, 'X'), (0, 1, 'Z'), (7, 0, 'Y'), (7, 1, 'X')],
    ];
    for compiler in COMPILERS {
        let (compiled, ends) = lower(2, &operations, compiler);
        for faults in &cases {
            assert_eq!(
                amplitudes(&compiled, &ends, faults),
                literal(2, &operations, faults)
            );
        }
        let forward = amplitudes(&compiled, &ends, &cases[0]);
        let reverse = amplitudes(&compiled, &ends, &cases[1]);
        assert!(forward.iter().any(|v| !v.is_zero()));
        assert_eq!(
            forward,
            reverse.iter().map(ExactScalar::negated).collect::<Vec<_>>()
        );
    }
}
