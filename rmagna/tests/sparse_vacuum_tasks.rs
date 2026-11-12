#[allow(dead_code)]
mod common;

use common::{bits, dense_gate};
use num_rational::BigRational;
use rmagna::circuit::{Basis, Operation, ParseOptions, parse_circuit};
use rmagna::clifford::{CliffordCircuit, CliffordGate as C};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::instrument::{
    CompileOptions, InstrumentError, compile_affine, compile_affine_stabilizer,
};
use rmagna::noise::RandomSource;
use rmagna::pauli::PauliWord;
use rmagna::runtime::{CausalProgram, CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{
    EvaluationOptions, SuppliedActionLimits, SuppliedNoiseActions, SuppliedPauliAction,
    TaskEvaluator, TaskOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use serde_json::json;
use std::path::Path;

struct NoEntropy;
impl RandomSource for NoEntropy {
    fn next_u64(&mut self) -> u64 {
        panic!("this prescribed deterministic task must not draw entropy")
    }
}

fn configurations() -> Vec<EvaluationOptions> {
    let mut result = vec![];
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let mut options = EvaluationOptions::default();
                options.runtime.compile.compiler_strategy = compiler;
                options.runtime.strategy = strategy;
                options.runtime.history_sampler = sampler;
                result.push(options);
            }
        }
    }
    result
}

fn modules(
    source: &rmagna::circuit::Circuit,
    task: serde_json::Value,
    parse: &ParseOptions,
) -> rmagna::task::LoadedModules {
    parse_task(&task.to_string(), Path::new("."), &TaskOptions::default())
        .unwrap()
        .load_modules(source, parse)
        .unwrap()
}

fn word(width: usize, x: usize, z: usize) -> PauliWord {
    PauliWord::new(
        bits(width, x),
        bits(width, z),
        2 * (x & z).count_ones() as u8 % 8,
    )
    .unwrap()
}

#[test]
fn selected_idle_vacua_retain_independent_xy_and_z_outputs_and_width_limits() {
    let parse = ParseOptions::default();
    for (text, records, required) in [("X 9999\n", vec![], 3), ("X 9999\nM 9999\n", vec![0], 4)] {
        let source = parse_circuit(text, &parse).unwrap();
        let captured = modules(
            &source,
            json!({"outputs":[19,7],"sample_records":records,"observables":[[1,0],[0,1],[2,0],[0,2],[3,0],[3,3],[0,3]]}),
            &parse,
        );
        for mut options in configurations() {
            options.runtime.compile.max_coherent_qubits = required;
            let task =
                TaskEvaluator::new(&source, &captured, &ControllerRegistry::default(), options)
                    .unwrap();
            assert_eq!(task.compilation().coherent_width, required);
            assert_eq!(task.sampled_history(), !records.is_empty());
            let result = task.sample_prescribed(&[], &mut NoEntropy).unwrap();
            assert_eq!(result.accepted, ExactField::one());
            assert_eq!(result.sampled_records, vec![true; records.len()]);
            assert_eq!(
                result.moments,
                [0, 1, 0, 1, 0, 0, 1].map(|v| if v == 0 {
                    ExactField::zero()
                } else {
                    ExactField::one()
                })
            );
            options.runtime.compile.max_coherent_qubits = required - 1;
            let err =
                TaskEvaluator::new(&source, &captured, &ControllerRegistry::default(), options)
                    .err()
                    .unwrap();
            assert!(err.message.contains(&format!("need {required}")));
            // Omitted outputs still mean every physical wire, including idle ones.
            let all = modules(&source, json!({}), &parse);
            let err = TaskEvaluator::new(&source, &all, &ControllerRegistry::default(), options)
                .err()
                .unwrap();
            assert!(err.message.contains("need 10000"));
        }
        assert_eq!(source.num_qubits(), 10000);
        assert_eq!(source.to_text(), text);
    }
}

fn dense_preparation() -> Vec<ExactScalar> {
    let a = ScalarArithmetic::default();
    let mut state = vec![ExactScalar::zero(); 16];
    state[0] = ExactScalar::one();
    dense_gate(C::H(0), &mut state, &a);
    for (label, value) in state.iter_mut().enumerate() {
        if label & 1 != 0 {
            *value = a.multiply(value, &a.omega(1, 0).unwrap()).unwrap();
        }
    }
    dense_gate(
        C::Cx {
            control: 0,
            target: 3,
        },
        &mut state,
        &a,
    );
    dense_gate(C::S { qubit: 3, turns: 1 }, &mut state, &a);
    dense_gate(C::H(2), &mut state, &a);
    for (label, value) in state.iter_mut().enumerate() {
        if label & 12 == 12 {
            *value = a.multiply(value, &a.omega(2, 0).unwrap()).unwrap();
        }
    }
    state
}

// Literal dense-vector Pauli action and inner product; no instrument, hidden
// paths, affine substitution, or compiled moment machinery enters the oracle.
fn dense_moment(state: &[ExactScalar], outputs: &[usize], x: usize, z: usize) -> ExactField {
    let a = ScalarArithmetic::default();
    let physical = |mask: usize| {
        outputs
            .iter()
            .enumerate()
            .filter(|(j, _)| mask >> j & 1 != 0)
            .fold(0, |v, (_, q)| v | 1 << q)
    };
    let px = physical(x);
    let pz = physical(z);
    let phase = 2 * (x & z).count_ones() as i64;
    let mut sum = ExactScalar::zero();
    for (label, value) in state.iter().enumerate() {
        let exponent = phase + 4 * ((label & pz).count_ones() % 2) as i64;
        let term = a.multiply(&state[label ^ px].conjugate(), value).unwrap();
        let term = a.multiply(&term, &a.omega(exponent, 0).unwrap()).unwrap();
        sum = a.add(&sum, &term).unwrap();
    }
    FieldArithmetic::default().from_scalar(&sum).unwrap()
}

#[test]
fn dense_four_wire_oracle_checks_every_ordered_three_output_pauli_with_sparse_gaps() {
    let parse = ParseOptions::default();
    let dense = dense_preparation();
    let paulis: Vec<_> = (0..8)
        .flat_map(|x| (0..8).filter_map(move |z| (x != 0 || z != 0).then_some([x, z])))
        .collect();
    let expected: Vec<_> = paulis
        .iter()
        .map(|&[x, z]| dense_moment(&dense, &[3, 1, 0], x, z))
        .collect();
    assert!(
        expected
            .iter()
            .any(|x| x.real_sign().unwrap() == std::cmp::Ordering::Less)
    );
    for (source, outputs, width) in [
        ("H 0\nT 0\nCX 0 3\nS 3\nH 2\nCS 2 3\n", vec![3, 1, 0], 4),
        (
            "H 7\nT 7\nCX 7 9999\nS 9999\nH 5000\nCS 5000 9999\n",
            vec![9999, 19, 7],
            10000,
        ),
    ] {
        let circuit = parse_circuit(source, &parse).unwrap();
        let operations = circuit.operations().to_vec();
        let captured = modules(
            &circuit,
            json!({"outputs":outputs,"observables":paulis}),
            &parse,
        );
        for mut options in configurations() {
            options.runtime.compile.max_coherent_qubits = 8;
            let task =
                TaskEvaluator::new(&circuit, &captured, &ControllerRegistry::default(), options)
                    .unwrap();
            let value = task.sample_prescribed(&[], &mut NoEntropy).unwrap();
            assert_eq!(value.accepted, ExactField::one());
            assert_eq!(value.moments, expected);
            assert!(task.compilation().coherent_width <= 8);
        }
        assert_eq!(circuit.num_qubits(), width);
        assert_eq!(circuit.operations(), operations);
        assert_eq!(circuit.to_text(), source);
        assert_eq!(captured.task().outputs(), outputs);
    }
}

#[test]
fn generic_explicit_entangled_inputs_and_public_physical_bitvectors_are_unchanged() {
    let a = ScalarArithmetic::default();
    let source = parse_circuit("I 5\n", &ParseOptions::default()).unwrap();
    let zero = CliffordCircuit::computational(bits(6, 0)).unwrap();
    let input = CliffordCircuit::new(
        6,
        [
            C::H(0),
            C::Cx {
                control: 0,
                target: 5,
            },
        ],
    )
    .unwrap()
    .evolve(&zero, &a)
    .unwrap()
    .unwrap();
    let original = input.clone();
    for options in configurations() {
        let compile = options.runtime.compile;
        let instrument = compile_affine_stabilizer(&source, &input, &compile, &a).unwrap();
        assert_eq!(instrument.num_qubits(), 6);
        assert_eq!(instrument.coherent_qubits(), 6);
        let program = CausalProgram::new_stabilizer(
            &source,
            &input,
            &ControllerRegistry::default(),
            options.runtime,
            &a,
        )
        .unwrap();
        let history = program.resolve(&[], &mut NoEntropy, &a).unwrap();
        for p in [&instrument, history.instrument()] {
            for (x, z, want) in [
                (3, 0, ExactScalar::one()),
                (0, 3, ExactScalar::one()),
                (3, 3, ExactScalar::one().negated()),
                (0, 1, ExactScalar::zero()),
            ] {
                assert_eq!(
                    p.pauli_moment(&[0, 5], &word(2, x, z), &[], 256, &a)
                        .unwrap()
                        .value,
                    want
                );
            }
            assert!(p.pauli_moment(&[0], &word(2, 0, 1), &[], 256, &a).is_err());
            assert!(matches!(
                p.pauli_moment(&[6], &word(1, 0, 1), &[], 256, &a),
                Err(InstrumentError::QubitOutOfBounds { qubit: 6, width: 6 })
            ));
        }
        let low = CompileOptions {
            max_coherent_qubits: 5,
            ..compile
        };
        assert!(matches!(
            compile_affine_stabilizer(&source, &input, &low, &a),
            Err(InstrumentError::ResourceLimit { required: 6, .. })
        ));
        assert!(compile_affine(&source, &low, &a).is_err());
        let short = CliffordCircuit::computational(bits(5, 0)).unwrap();
        assert!(compile_affine_stabilizer(&source, &short, &compile, &a).is_err());
    }
    assert_eq!(input, original);
}

#[test]
fn sparse_noise_actions_records_and_callback_addresses_keep_original_source_ids() {
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", |inputs: &[bool]| Ok(inputs.to_vec()))
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let text = "X 7\nM 7\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nX_ERROR(1/4) ctrl[0] 1000\nM(1/5) 1000\nDETECTOR rec[-1]\nCONTROL[echo] det[1] ctrl[0] -> ctrl[1] ctrl[2]\nCX ctrl[1] 7\n";
    let source = parse_circuit(text, &parse).unwrap();
    let operations = source.operations().to_vec();
    let noise = operations
        .iter()
        .find(|op| matches!(op.operation, Operation::Noise { .. }))
        .unwrap();
    assert_eq!(noise.id, 4);
    assert!(matches!(&noise.operation,Operation::Noise {qubits,..} if qubits == &[1000]));
    let captured = modules(
        &source,
        json!({"outputs":[1000,7],"observables":[[0,1],[0,2]],"sample_records":[0,1]}),
        &parse,
    );
    let unit = BigRational::from_integer(1.into());
    for mut options in configurations() {
        options.runtime.compile.max_coherent_qubits = 9;
        for flips in [vec![], vec![1]] {
            let prepare = |paulis: &[SuppliedPauliAction]| {
                TaskEvaluator::prepare_supplied_actions(
                    &source,
                    &captured,
                    &registry,
                    options,
                    SuppliedNoiseActions {
                        paulis,
                        report_flips: &flips,
                        likelihood: &unit,
                    },
                    SuppliedActionLimits::default(),
                )
            };
            let action = |basis| SuppliedPauliAction {
                anchor: noise.id,
                wire: 1000,
                basis,
            };
            for actions in [
                vec![action(Basis::X)],
                vec![action(Basis::Z), action(Basis::Y)],
            ] {
                let result = prepare(&actions).unwrap().sample(&mut NoEntropy).unwrap();
                assert_eq!(result.accepted, ExactField::one());
                assert_eq!(result.error, ExactField::zero());
                let last = if flips.is_empty() {
                    ExactField::one()
                } else {
                    ExactField::one().negated()
                };
                assert_eq!(result.moments, [ExactField::one().negated(), last]);
                assert_eq!(result.sampled_records, [true, flips.is_empty()]);
                assert_eq!(result.controller_calls.len(), 2);
                for (call, inputs) in [(0, vec![true]), (1, vec![flips.is_empty(), true])] {
                    assert_eq!(result.controller_calls[call].call, call);
                    assert_eq!(result.controller_calls[call].name, "echo");
                    assert_eq!(result.controller_calls[call].inputs, inputs);
                    assert_eq!(result.controller_calls[call].outputs, inputs);
                }
                let expected = if actions.len() == 2 {
                    BigRational::from_integer(0.into())
                } else {
                    BigRational::new(1.into(), if flips.is_empty() { 5 } else { 20 }.into())
                };
                assert_eq!(result.noise_probability, expected);
            }
            for invalid in [
                SuppliedPauliAction {
                    wire: 1,
                    ..action(Basis::X)
                },
                SuppliedPauliAction {
                    anchor: 3,
                    ..action(Basis::X)
                },
            ] {
                assert_eq!(prepare(&[invalid]).err().unwrap().field, "noise");
            }
        }
    }
    assert_eq!(source.operations(), operations);
    assert_eq!(source.to_text(), text);
    assert_eq!(source.num_qubits(), 1001);
}

#[test]
fn product_measurements_correlated_noise_and_resets_retain_every_operand() {
    let parse = ParseOptions::default();
    let source = parse_circuit(
        "RX 7\nH 31\nE(1/4) X7 Y31\nMPP X7*Z31\nDETECTOR rec[-1]\nPOSTSELECT det[0]\nRY 31\n",
        &parse,
    )
    .unwrap();
    let captured = modules(
        &source,
        json!({"outputs":[31,12,7],"observables":[[1,1],[0,2],[4,0],[5,1]]}),
        &parse,
    );
    let half = FieldArithmetic::default()
        .from_rational(BigRational::new(1.into(), 2.into()))
        .unwrap();
    for mut options in configurations() {
        options.runtime.compile.max_coherent_qubits = 7;
        let task = TaskEvaluator::new(&source, &captured, &ControllerRegistry::default(), options)
            .unwrap();
        let mut enumeration = rmagna::task::TaskEnumerationOptions::default();
        enumeration.noise.max_weight = 1;
        let result = task.enumerate_physical(&enumeration).unwrap();
        assert_eq!(result.masses.accepted, half);
        assert_eq!(result.masses.moments, vec![half.clone(); 4]);
        assert_eq!(
            result.omitted_probability.value(),
            &BigRational::from_integer(0.into())
        );
    }
    let empty = parse_circuit("TICK\n", &parse).unwrap();
    let captured = modules(&empty, json!({"outputs":[]}), &parse);
    let mut options = EvaluationOptions::default();
    options.runtime.compile.max_coherent_qubits = 0;
    let task =
        TaskEvaluator::new(&empty, &captured, &ControllerRegistry::default(), options).unwrap();
    assert_eq!(task.compilation().coherent_width, 0);
    assert_eq!(
        task.sample_prescribed(&[], &mut NoEntropy)
            .unwrap()
            .accepted,
        ExactField::one()
    );
}
