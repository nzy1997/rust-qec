// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//! Complete original atomic-output/nonlinear-parity method from
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_circuit_file.py.
//! Resolved inputs replace versioned target reads; distinct decision atoms
//! represent atomic external outputs. Runtime calls use the public causal API.
use rmagna::circuit::{Basis, BitAtom, BitExpr, Gate, Operation, ParseOptions, parse_circuit};
use rmagna::clifford::CliffordCircuit;
use rmagna::gf2::BitVector;
use rmagna::noise::SplitMix64;
use rmagna::probability::Probability;
use rmagna::runtime::{CausalProgram, CausalStrategy, ControllerRegistry, RuntimeOptions};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::unitary::CompilerStrategy;
use std::sync::{Arc, Mutex};

const TEXT: &str = "M 0 1 2\nCONTROL[XOR] rec[-3] rec[-2] -> ctrl[0]\nDETECTOR rec[-1]\nCONTROL[actual] ctrl[0] det[-1] -> ctrl[0] ctrl[1]\nCONTROL[XOR] ctrl[0] ctrl[1] -> ctrl[2]\nCH ctrl[2] 0\n";

fn parity(records: &[usize]) -> BitExpr {
    BitExpr {
        constant: false,
        terms: records.iter().copied().map(BitAtom::Record).collect(),
    }
}
fn decision(bit: usize) -> BitExpr {
    BitExpr::atom(BitAtom::Decision { call: 0, bit })
}

#[test]
fn atomic_outputs_and_nonlinear_parity_information() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("actual", "1", move |bits: &[bool]| {
            seen.lock().unwrap().push(bits.to_vec());
            Ok(bits.to_vec())
        })
        .unwrap();
    let circuit = parse_circuit(
        TEXT,
        &ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(circuit.to_text(), TEXT);
    assert_eq!((circuit.num_qubits(), circuit.num_records()), (3, 3));
    // This predicate selects Rust's causal nonlinear route. Required controller
    // observations are the owned input expressions of the external operation.
    assert!(circuit.needs_history_sampling());
    assert_eq!(circuit.external_calls(), 1);
    let measures = circuit
        .operations()
        .iter()
        .filter_map(|op| match &op.operation {
            Operation::Measure {
                product,
                report_flip,
                record,
                ..
            } => {
                assert_eq!(product.factors, [(*record, Basis::Z)]);
                assert_eq!(report_flip, &Probability::zero());
                Some(*record)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(measures, [0, 1, 2]);
    assert_eq!(circuit.detectors(), &[parity(&[2])]);
    let controls = circuit
        .operations()
        .iter()
        .filter_map(|op| match &op.operation {
            Operation::Control {
                name,
                call,
                inputs,
                outputs,
            } => Some((name.as_str(), *call, inputs, outputs)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(controls.len(), 3);
    let first = controls[0];
    assert_eq!((first.0, first.1), ("XOR", None));
    assert_eq!(first.2, &[parity(&[0]), parity(&[1])]);
    assert_eq!(first.3.len(), 1);
    assert_eq!((first.3[0].register, first.3[0].version), (0, 0));
    assert_eq!(first.3[0].expression, parity(&[0, 1]));

    let external = controls[1];
    assert_eq!((external.0, external.1), ("actual", Some(0)));
    assert_eq!(external.2.len(), 2);
    assert_eq!(external.2, &[parity(&[0, 1]), parity(&[2])]);
    // The read captures register0/version0 before both outputs are installed.
    assert_eq!(external.2[0], first.3[0].expression);
    assert_eq!(external.3.len(), 2);
    assert_eq!((external.3[0].register, external.3[0].version), (0, 1));
    assert_eq!((external.3[1].register, external.3[1].version), (1, 0));
    assert_eq!(external.3[0].expression, decision(0));
    assert_eq!(external.3[1].expression, decision(1));
    assert_ne!(external.3[0].expression, external.3[1].expression);
    assert_ne!(external.2[0], external.3[0].expression);

    let final_xor = controls[2];
    assert_eq!((final_xor.0, final_xor.1), ("XOR", None));
    assert_eq!(final_xor.2, &[decision(0), decision(1)]);
    assert_eq!(final_xor.3.len(), 1);
    assert_eq!((final_xor.3[0].register, final_xor.3[0].version), (2, 0));
    let final_control = BitExpr {
        constant: false,
        terms: [
            BitAtom::Decision { call: 0, bit: 0 },
            BitAtom::Decision { call: 0, bit: 1 },
        ]
        .into(),
    };
    assert_eq!(final_xor.3[0].expression, final_control);
    assert!(!final_control.is_affine());
    assert!(matches!(
        &circuit.operations().last().unwrap().operation,
        Operation::Unitary { gate: Gate::H, qubits, control: Some(value) }
            if qubits == &[0] && value == &final_control
    ));

    // Explicit |q2 q1 q0> = |100> yields the original callback input (0,1)
    // without changing its source circuit or invoking a callback directly.
    let mut bits = BitVector::zeros(3).unwrap();
    bits.set(2, true).unwrap();
    let input = CliffordCircuit::computational(bits).unwrap();
    let arithmetic = ScalarArithmetic::default();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            calls.lock().unwrap().clear();
            let mut options = RuntimeOptions {
                strategy,
                ..Default::default()
            };
            options.compile.compiler_strategy = compiler;
            let program =
                CausalProgram::new_stabilizer(&circuit, &input, &registry, options, &arithmetic)
                    .unwrap();
            assert!(calls.lock().unwrap().is_empty());
            // Requested observations are reported-record parities. External
            // decisions are checked through calls and resolve_expression.
            let requested = [external.2[0].clone(), external.2[1].clone()];
            let history = program
                .resolve(&requested, &mut SplitMix64::new(0), &arithmetic)
                .unwrap();
            assert_eq!(*calls.lock().unwrap(), [vec![false, true]]);
            let actual = history.controller_calls();
            assert_eq!(actual.len(), 1);
            assert_eq!((actual[0].name.as_str(), actual[0].call), ("actual", 0));
            assert_eq!(actual[0].inputs, [false, true]);
            assert_eq!(actual[0].outputs, [false, true]);
            assert_eq!(history.requested_values(), [false, true]);
            assert_eq!(history.history_mass(), &ExactScalar::one());
            for (bit, value) in [(0, false), (1, true)] {
                let resolved = history.resolve_expression(&decision(bit)).unwrap();
                assert_eq!(resolved.constant, value);
                assert!(resolved.terms.is_empty());
            }
            let resolved = history.resolve_expression(&final_control).unwrap();
            assert!(resolved.constant);
            assert!(resolved.terms.is_empty());
            assert_eq!(external.2[0], parity(&[0, 1]));
            assert_eq!(first.3[0].version, 0);
        }
    }
    assert!(parse_circuit(TEXT, &ParseOptions::default()).is_err());
}
