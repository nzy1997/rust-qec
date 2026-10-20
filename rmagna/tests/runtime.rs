use rmagna::circuit::{Operation, ParseOptions, parse_circuit};
use rmagna::instrument::{CompileOptions, FaultSelection, compile_unitary};
use rmagna::noise::RandomSource;
use rmagna::runtime::{CausalProgram, ControllerRegistry, RuntimeError, RuntimeOptions};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use std::sync::{Arc, Mutex};
struct Words {
    values: Vec<u64>,
    at: usize,
}
impl Words {
    fn bits(bits: &[bool]) -> Self {
        Self {
            values: bits.iter().map(|&b| if b { u64::MAX } else { 0 }).collect(),
            at: 0,
        }
    }
}
impl RandomSource for Words {
    fn next_u64(&mut self) -> u64 {
        let value = self.values[self.at];
        self.at += 1;
        value
    }
}
fn zero(a: &ScalarArithmetic) -> rmagna::unitary::CompiledUnitary {
    compile_unitary(
        &parse_circuit("I 0", &ParseOptions::default()).unwrap(),
        &CompileOptions::default(),
        a,
    )
    .unwrap()
}
fn parse(text: &str, registry: &ControllerRegistry) -> rmagna::circuit::Circuit {
    parse_circuit(
        text,
        &ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        },
    )
    .unwrap()
}
#[test]
fn majority_histories_match_enumeration_without_conditioning_on_postselection() {
    let a = ScalarArithmetic::default();
    let mut registry = ControllerRegistry::default();
    registry
        .register("majority", "1", |bits: &[bool]| {
            Ok(vec![bits.iter().filter(|&&b| b).count() >= 2])
        })
        .unwrap();
    let source = parse(
        "H 0 1 2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[majority] det[0] det[1] det[2] -> ctrl[0]\nCX ctrl[0] 3\nPOSTSELECT det[0]",
        &registry,
    );
    let program = CausalProgram::new(&source, &registry, RuntimeOptions::default(), &a).unwrap();
    assert_eq!(program.controller_versions()["majority"], "1");
    let mut accepted = ExactScalar::zero();
    let mut error = ExactScalar::zero();
    for mask in 0_usize..8 {
        let bits = (0..3).map(|j| mask >> j & 1 != 0).collect::<Vec<_>>();
        let draw = program
            .sample_prescribed(&[3], &zero(&a), &[], &mut Words::bits(&bits), &a)
            .unwrap();
        assert_eq!(draw.controller_calls.len(), 1);
        assert_eq!(draw.controller_calls[0].inputs, bits);
        assert_eq!(draw.controller_calls[0].outputs, [mask.count_ones() >= 2]);
        assert_eq!(draw.history_mass, a.omega(0, -6).unwrap());
        let mass = if bits[0] {
            ExactScalar::zero()
        } else {
            draw.history_mass.clone()
        };
        assert_eq!(draw.masses.accepted.value, mass);
        let expected_error = if mask.count_ones() >= 2 {
            mass
        } else {
            ExactScalar::zero()
        };
        assert_eq!(draw.masses.error, expected_error);
        assert_eq!(draw.history_entropy_words, 3);
        accepted = a.add(&accepted, &draw.masses.accepted.value).unwrap();
        error = a.add(&error, &draw.masses.error).unwrap();
    }
    assert_eq!(accepted, a.omega(0, -2).unwrap());
    assert_eq!(error, a.omega(0, -6).unwrap());
}
#[test]
fn sequential_calls_preserve_inputs_and_atomic_register_updates() {
    let a = ScalarArithmetic::default();
    let mut registry = ControllerRegistry::default();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let capture = seen.clone();
    registry
        .register("parity", "1", move |bits: &[bool]| {
            capture.lock().unwrap().push(bits.to_vec());
            Ok(vec![bits.iter().fold(false, |p, b| p ^ b)])
        })
        .unwrap();
    let source = parse(
        "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[parity] det[0] -> ctrl[0]\nCX ctrl[0] 1\nM 1\nDETECTOR rec[-1]\nCONTROL[parity] det[1] ctrl[0] -> ctrl[0]\nCX ctrl[0] 2",
        &registry,
    );
    let program = CausalProgram::new(&source, &registry, RuntimeOptions::default(), &a).unwrap();
    for bit in [false, true] {
        let draw = program
            .sample_prescribed(&[2], &zero(&a), &[], &mut Words::bits(&[bit]), &a)
            .unwrap();
        assert_eq!(draw.controller_calls[0].inputs, [bit]);
        assert_eq!(draw.controller_calls[1].inputs, [bit, bit]);
        assert_eq!(draw.controller_calls[1].outputs, [false]);
        assert_eq!(draw.history_mass, a.omega(0, -2).unwrap());
        assert_eq!(draw.masses.target_success.value, draw.history_mass);
        assert_eq!(draw.masses.error, ExactScalar::zero());
        assert_eq!(draw.history_entropy_words, 1);
    }
    assert_eq!(
        *seen.lock().unwrap(),
        [
            vec![false],
            vec![false, false],
            vec![true],
            vec![true, true]
        ]
    );
}
#[test]
fn guarded_faults_have_causal_actions_and_active_path_probabilities_only() {
    let a = ScalarArithmetic::default();
    let registry = ControllerRegistry::default();
    let source = parse("H 0\nM 0\nX_ERROR(1/3) rec[-1] 1", &registry);
    let program = CausalProgram::new(&source, &registry, RuntimeOptions::default(), &a).unwrap();
    let operation = source
        .operations()
        .iter()
        .find(|op| matches!(op.operation, Operation::Noise { .. }))
        .unwrap()
        .id;
    let site = program.potential_site(operation).unwrap();
    for active in [false, true] {
        for fault in [false, true] {
            let faults = if fault {
                vec![FaultSelection {
                    site,
                    alternative: 0,
                }]
            } else {
                vec![]
            };
            let draw = program
                .sample_prescribed(&[1], &zero(&a), &faults, &mut Words::bits(&[active]), &a)
                .unwrap();
            assert_eq!(draw.history_mass, a.omega(0, -2).unwrap());
            assert_eq!(
                draw.masses.error,
                if active && fault {
                    draw.history_mass.clone()
                } else {
                    ExactScalar::zero()
                }
            );
            assert_eq!(
                draw.noise_probability.to_string(),
                if active {
                    if fault { "1/3" } else { "2/3" }
                } else {
                    "1"
                }
            );
            assert_eq!(draw.conditions.len(), 1);
        }
    }
}
#[test]
fn controller_failures_shapes_and_missing_bindings_fail_explicitly() {
    let a = ScalarArithmetic::default();
    let mut registry = ControllerRegistry::default();
    assert!(matches!(
        registry.register("", "1", |_: &[bool]| Ok(vec![])),
        Err(RuntimeError::InvalidBinding { .. })
    ));
    registry
        .register("bad", "1", |_: &[bool]| Ok(vec![]))
        .unwrap();
    assert!(matches!(
        registry.register("bad", "2", |_: &[bool]| Ok(vec![])),
        Err(RuntimeError::DuplicateBinding { .. })
    ));
    let source = parse("CONTROL[bad] -> ctrl[0]\nI 0", &registry);
    assert!(matches!(
        CausalProgram::new(
            &source,
            &ControllerRegistry::default(),
            RuntimeOptions::default(),
            &a
        ),
        Err(RuntimeError::MissingBinding { .. })
    ));
    let program = CausalProgram::new(&source, &registry, RuntimeOptions::default(), &a).unwrap();
    assert!(matches!(
        program.sample_prescribed(&[0], &zero(&a), &[], &mut Words::bits(&[]), &a),
        Err(RuntimeError::OutputShape {
            expected: 1,
            actual: 0,
            ..
        })
    ));
    let mut registry = ControllerRegistry::default();
    registry
        .register("bad", "1", |_: &[bool]| {
            Err("decoder rejected configuration".to_owned())
        })
        .unwrap();
    let program = CausalProgram::new(&source, &registry, RuntimeOptions::default(), &a).unwrap();
    assert!(matches!(
        program.sample_prescribed(&[0], &zero(&a), &[], &mut Words::bits(&[]), &a),
        Err(RuntimeError::Controller { .. })
    ));
}

#[test]
fn prescribed_causal_histories_match_pinned_native_joint_masses() {
    let a = ScalarArithmetic::default();
    let data: serde_json::Value =
        serde_json::from_str(include_str!("data/history-reference.json")).unwrap();
    assert_eq!(data["schema"], "rmagna.history-reference.v1");
    assert_eq!(
        data["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    let vectors = data["vectors"].as_array().unwrap();
    assert_eq!(vectors.len(), 18);
    for (index, vector) in vectors.iter().enumerate() {
        let name = vector["case"].as_str().unwrap().to_owned();
        let mut registry = ControllerRegistry::default();
        let majority = name == "majority";
        registry
            .register("decision", "1", move |bits: &[bool]| {
                Ok(vec![if majority {
                    bits.iter().filter(|&&v| v).count() >= 2
                } else {
                    bits.iter().fold(false, |p, b| p ^ b)
                }])
            })
            .unwrap();
        let source = parse(vector["source"].as_str().unwrap(), &registry);
        let program =
            CausalProgram::new(&source, &registry, RuntimeOptions::default(), &a).unwrap();
        let mut faults = Vec::new();
        for fault in vector["faults"].as_array().unwrap() {
            let line = fault["line"].as_u64().unwrap() as usize;
            let op = source
                .operations()
                .iter()
                .find(|op| {
                    op.location.line == line && matches!(op.operation, Operation::Noise { .. })
                })
                .unwrap();
            faults.push(FaultSelection {
                site: program.potential_site(op.id).unwrap(),
                alternative: fault["alternative"].as_u64().unwrap() as usize,
            });
        }
        for flip in vector["report_flips"].as_array().unwrap() {
            let record = flip.as_u64().unwrap() as usize;
            let op = source
                .operations()
                .iter()
                .find(|op| matches!(op.operation,Operation::Measure {record:r,..} if r==record))
                .unwrap();
            faults.push(FaultSelection {
                site: program.potential_site(op.id).unwrap(),
                alternative: 0,
            });
        }
        let expected_calls = vector["calls"].as_array().unwrap();
        let bits = expected_calls
            .iter()
            .flat_map(|call| {
                call["inputs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_u64().unwrap() != 0)
            })
            .collect::<Vec<_>>();
        let outputs = vector["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect::<Vec<_>>();
        let draw = program
            .sample_prescribed(&outputs, &zero(&a), &faults, &mut Words::bits(&bits), &a)
            .unwrap();
        assert_eq!(draw.controller_calls.len(), expected_calls.len());
        for (actual, expected) in draw.controller_calls.iter().zip(expected_calls) {
            assert_eq!(
                actual.inputs,
                expected["inputs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() != 0)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                actual.outputs,
                expected["outputs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() != 0)
                    .collect::<Vec<_>>()
            );
        }
        for (actual, key) in [
            (&draw.history_mass, "history_mass"),
            (&draw.masses.accepted.value, "acceptance"),
            (&draw.masses.error, "error"),
            (&draw.masses.target_success.value, "target_success"),
        ] {
            let expected = &vector[key];
            let coefficients = std::array::from_fn(|j| {
                expected["coefficients"][j]
                    .as_str()
                    .unwrap()
                    .parse()
                    .unwrap()
            });
            assert_eq!(
                actual,
                &a.scalar(coefficients, expected["power"].as_i64().unwrap())
                    .unwrap(),
                "{name} vector {index} {key}"
            );
        }
    }
}

#[test]
fn guarded_correlated_chains_draw_one_condition_and_keep_conditional_weights() {
    let a = ScalarArithmetic::default();
    let registry = ControllerRegistry::default();
    let source = parse(
        "H 0\nM 0\nE(1/10) rec[-1] X1\nELSE_CORRELATED_ERROR(2/9) rec[-1] Z1",
        &registry,
    );
    let program = CausalProgram::new(&source, &registry, RuntimeOptions::default(), &a).unwrap();
    let operation = source
        .operations()
        .iter()
        .find(|op| matches!(op.operation, Operation::Noise { .. }))
        .unwrap()
        .id;
    let site = program.potential_site(operation).unwrap();
    for bit in [false, true] {
        for alternative in 0..2 {
            let draw = program
                .sample_prescribed(
                    &[1],
                    &zero(&a),
                    &[FaultSelection { site, alternative }],
                    &mut Words::bits(&[bit]),
                    &a,
                )
                .unwrap();
            assert_eq!(draw.conditions.len(), 1);
            assert_eq!(draw.history_entropy_words, 1);
            assert_eq!(
                draw.noise_probability.to_string(),
                if bit {
                    if alternative == 0 { "1/10" } else { "1/5" }
                } else {
                    "1"
                }
            );
            assert_eq!(
                draw.masses.error,
                if bit && alternative == 0 {
                    a.omega(0, -2).unwrap()
                } else {
                    ExactScalar::zero()
                }
            );
        }
    }
}
#[test]
fn causal_resource_limits_and_target_validation_fail_without_silent_draws() {
    let a = ScalarArithmetic::default();
    let registry = ControllerRegistry::default();
    let source = parse("H 0\nM 0\nX_ERROR(1/3) rec[-1] 1", &registry);
    for options in [
        RuntimeOptions {
            max_history_inputs: 0,
            ..Default::default()
        },
        RuntimeOptions {
            max_prefix_compiles: 0,
            ..Default::default()
        },
    ] {
        let program = CausalProgram::new(&source, &registry, options, &a).unwrap();
        assert!(matches!(
            program.sample_prescribed(&[1], &zero(&a), &[], &mut Words::bits(&[]), &a),
            Err(RuntimeError::ResourceLimit { .. })
        ));
    }
    let program = CausalProgram::new(&source, &registry, RuntimeOptions::default(), &a).unwrap();
    let mut random = Words::bits(&[]);
    assert!(program.sample(&[100], &zero(&a), &mut random, &a).is_err());
    assert_eq!(random.at, 0);
}
