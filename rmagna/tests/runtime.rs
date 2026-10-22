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
        let mut groups = expected_calls
            .iter()
            .map(|call| {
                call["inputs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() != 0)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        if name == "guarded" {
            groups.push(vec![expected_calls[0]["outputs"][0].as_u64().unwrap() != 0]);
        }
        let fixed = program
            .resolve_prescribed_history(&[], &faults, &groups, &a)
            .unwrap();
        assert_eq!(fixed.entropy_words(), (0, 0));
        assert_eq!(fixed.network_compilations(), 0);
        assert_eq!(program.reference_compilations(), 1);
        let cold_program = CausalProgram::new(
            &source,
            &registry,
            RuntimeOptions {
                strategy: rmagna::runtime::CausalStrategy::ColdPrefix,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
        let cold = cold_program
            .resolve_prescribed_history(&[], &faults, &groups, &a)
            .unwrap();
        assert_eq!(cold.history_mass(), fixed.history_mass());
        assert_eq!(cold.controller_calls(), fixed.controller_calls());
        assert_eq!(cold.noise_probability(), fixed.noise_probability());
        assert_eq!(cold.network_compilations(), groups.len() + 1);
        let cold_masses = cold
            .instrument()
            .masses(&outputs, &zero(&a), cold.faults(), 256, &a)
            .unwrap();
        assert_eq!(cold_masses.error, draw.masses.error);
        assert_eq!(cold_masses.accepted.value, draw.masses.accepted.value);
        assert_eq!(
            cold_masses.target_success.value,
            draw.masses.target_success.value
        );
        assert_eq!(fixed.history_mass(), &draw.history_mass);
        assert_eq!(fixed.controller_calls(), draw.controller_calls);
        let fixed_masses = fixed
            .instrument()
            .masses(&outputs, &zero(&a), fixed.faults(), 256, &a)
            .unwrap();
        assert_eq!(fixed_masses.accepted.value, draw.masses.accepted.value);
        assert_eq!(
            fixed_masses.target_success.value,
            draw.masses.target_success.value
        );
        assert_eq!(fixed_masses.error, draw.masses.error);
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
            strategy: rmagna::runtime::CausalStrategy::ColdPrefix,
            max_prefix_compiles: 0,
            ..Default::default()
        },
        RuntimeOptions {
            max_history_queries: 0,
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

#[test]
fn prescribed_history_shape_errors_and_impossible_prefixes_do_not_invoke_callbacks() {
    let a = ScalarArithmetic::default();
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = calls.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("decision", "1", move |bits: &[bool]| {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(vec![bits[0]])
        })
        .unwrap();
    let source = parse(
        "M 0\nDETECTOR rec[-1]\nCONTROL[decision] det[0] -> ctrl[0]",
        &registry,
    );
    let program = CausalProgram::new(&source, &registry, RuntimeOptions::default(), &a).unwrap();
    for groups in [
        vec![],
        vec![vec![]],
        vec![vec![false], vec![]],
        vec![vec![true]],
    ] {
        assert!(matches!(
            program.resolve_prescribed_history(&[], &[], &groups, &a),
            Err(RuntimeError::InvalidHistory { .. })
        ));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
    let history = program
        .resolve_prescribed_history(&[], &[], &[vec![false]], &a)
        .unwrap();
    assert_eq!(history.history_mass(), &ExactScalar::one());
    assert_eq!(history.entropy_words(), (0, 0));
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}
#[test]
fn prescribed_guard_groups_are_shared_by_correlated_chains_and_keep_physical_weights() {
    let a = ScalarArithmetic::default();
    let source = parse(
        "H 0\nM 0\nE(1/10) rec[-1] X1\nELSE_CORRELATED_ERROR(2/9) rec[-1] Z1",
        &ControllerRegistry::default(),
    );
    let program = CausalProgram::new(
        &source,
        &ControllerRegistry::default(),
        RuntimeOptions::default(),
        &a,
    )
    .unwrap();
    let site = program
        .potential_site(
            source
                .operations()
                .iter()
                .find(|op| matches!(op.operation, Operation::Noise { .. }))
                .unwrap()
                .id,
        )
        .unwrap();
    for active in [false, true] {
        let fixed = program
            .resolve_prescribed_history(
                &[],
                &[FaultSelection {
                    site,
                    alternative: 1,
                }],
                &[vec![active]],
                &a,
            )
            .unwrap();
        assert_eq!(fixed.conditions().len(), 1);
        assert_eq!(fixed.history_mass(), &a.omega(0, -2).unwrap());
        assert_eq!(
            fixed.noise_probability().to_string(),
            if active { "1/5" } else { "1" }
        );
        assert_eq!(fixed.entropy_words(), (0, 0));
    }
}
#[test]
fn prescribed_requested_records_preserve_correlations_and_ignore_final_postselection() {
    let a = ScalarArithmetic::default();
    let source = parse(
        "H 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]",
        &ControllerRegistry::default(),
    );
    let program = CausalProgram::new(
        &source,
        &ControllerRegistry::default(),
        RuntimeOptions::default(),
        &a,
    )
    .unwrap();
    let query = vec![rmagna::circuit::BitExpr::atom(rmagna::circuit::BitAtom::Record(0)); 2];
    for value in [false, true] {
        let history = program
            .resolve_prescribed_history(&query, &[], &[vec![value, value]], &a)
            .unwrap();
        assert_eq!(history.requested_values(), [value, value]);
        assert_eq!(history.history_mass(), &a.omega(0, -2).unwrap());
        let accepted = history
            .instrument()
            .acceptance(history.faults(), 256, &a)
            .unwrap()
            .value;
        assert_eq!(
            accepted,
            if value {
                ExactScalar::zero()
            } else {
                a.omega(0, -2).unwrap()
            }
        );
    }
    assert!(matches!(
        program.resolve_prescribed_history(&query, &[], &[vec![false, true]], &a),
        Err(RuntimeError::InvalidHistory { .. })
    ));
}

#[test]
fn fixed_template_matches_cold_prefixes_and_analytic_histories_despite_future_faults_and_selection()
{
    use rmagna::circuit::{BitAtom, BitExpr};
    use rmagna::runtime::CausalStrategy;
    let a = ScalarArithmetic::default();
    let mut registry = ControllerRegistry::default();
    registry
        .register("copy", "1", |bits: &[bool]| Ok(vec![bits[0]]))
        .unwrap();
    registry
        .register("parity", "1", |bits: &[bool]| Ok(vec![bits[0] ^ bits[1]]))
        .unwrap();
    let source = parse(
        "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCH ctrl[0] 1\nX_ERROR(1/3) ctrl[0] 1\nM 1\nDETECTOR rec[-1]\nCONTROL[parity] det[1] ctrl[0] -> ctrl[1]\nH 2\nCS ctrl[1] 2\nT 2\nH 2\nM 2\nDETECTOR rec[-1]\nPOSTSELECT ctrl[1] det[2]",
        &registry,
    );
    let fixed = CausalProgram::new(
        &source,
        &registry,
        RuntimeOptions {
            max_prefix_compiles: 0,
            ..Default::default()
        },
        &a,
    )
    .unwrap();
    let cold = CausalProgram::new(
        &source,
        &registry,
        RuntimeOptions {
            strategy: CausalStrategy::ColdPrefix,
            ..Default::default()
        },
        &a,
    )
    .unwrap();
    let operation = source
        .operations()
        .iter()
        .find(|op| matches!(op.operation, Operation::Noise { .. }))
        .unwrap()
        .id;
    let site = fixed.potential_site(operation).unwrap();
    let request = [BitExpr::atom(BitAtom::Record(2))];
    for faulted in [false, true] {
        let faults = if faulted {
            vec![FaultSelection {
                site,
                alternative: 0,
            }]
        } else {
            vec![]
        };
        for m0 in [false, true] {
            for m1 in [false, true] {
                for m2 in [false, true] {
                    let groups = vec![vec![m0], vec![m0], vec![m1, m0], vec![m2]];
                    let f = fixed.resolve_prescribed_history(&request, &faults, &groups, &a);
                    let c = cold.resolve_prescribed_history(&request, &faults, &groups, &a);
                    if !m0 && m1 {
                        assert!(f.is_err() && c.is_err());
                        continue;
                    }
                    let f = f.unwrap();
                    let c = c.unwrap();
                    assert_eq!(f.history_mass(), c.history_mass());
                    assert_eq!(f.controller_calls(), c.controller_calls());
                    assert_eq!(f.noise_probability(), c.noise_probability());
                    assert_eq!(f.network_compilations(), 0);
                    assert_eq!(c.network_compilations(), 5);
                    assert_eq!(f.history_queries(), 4);
                    let sign = if m0 ^ m1 { -1 } else { 1 };
                    let zero_mass = a
                        .scalar([2.into(), sign.into(), 0.into(), (-sign).into()], -2)
                        .unwrap();
                    let last = if m2 {
                        a.subtract(&ExactScalar::one(), &zero_mass).unwrap()
                    } else {
                        zero_mass
                    };
                    let history = a
                        .multiply(&last, &a.omega(0, if m0 { -4 } else { -2 }).unwrap())
                        .unwrap();
                    assert_eq!(f.history_mass(), &history);
                    let accepted = f
                        .instrument()
                        .acceptance(f.faults(), 256, &a)
                        .unwrap()
                        .value;
                    assert_eq!(
                        accepted,
                        if m0 == m1 && !m2 {
                            history
                        } else {
                            ExactScalar::zero()
                        }
                    );
                    assert_eq!(
                        f.instrument().fault_probability(f.faults()).unwrap(),
                        *f.noise_probability()
                    );
                }
            }
        }
    }
}

#[test]
fn seeded_fixed_and_cold_histories_preserve_entropy_and_full_physical_draws() {
    use rmagna::circuit::{BitAtom, BitExpr};
    use rmagna::noise::SplitMix64;
    use rmagna::runtime::CausalStrategy;
    let a = ScalarArithmetic::default();
    let mut registry = ControllerRegistry::default();
    registry
        .register("copy", "1", |bits: &[bool]| Ok(vec![bits[0]]))
        .unwrap();
    let source = parse(
        "H 0\nT 0\nH 0\nM(1/7) 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCH ctrl[0] 1\nX_ERROR(1/3) ctrl[0] 1\nM 1\nDETECTOR rec[-1]\nPOSTSELECT det[1]",
        &registry,
    );
    let fixed = CausalProgram::new(&source, &registry, RuntimeOptions::default(), &a).unwrap();
    let cold = CausalProgram::new(
        &source,
        &registry,
        RuntimeOptions {
            strategy: CausalStrategy::ColdPrefix,
            ..Default::default()
        },
        &a,
    )
    .unwrap();
    let requested = [BitExpr::atom(BitAtom::Record(1))];
    for seed in 0..48 {
        let mut f = SplitMix64::new(seed);
        let mut c = SplitMix64::new(seed);
        let first = fixed.resolve(&requested, &mut f, &a).unwrap();
        let second = cold.resolve(&requested, &mut c, &a).unwrap();
        assert_eq!(f.state(), c.state());
        assert_eq!(first.entropy_words(), second.entropy_words());
        assert_eq!(first.history_mass(), second.history_mass());
        assert_eq!(first.noise_probability(), second.noise_probability());
        assert_eq!(first.faults(), second.faults());
        assert_eq!(first.controller_calls(), second.controller_calls());
        assert_eq!(first.requested_values(), second.requested_values());
        assert_eq!(
            first
                .instrument()
                .acceptance(first.faults(), 256, &a)
                .unwrap()
                .value,
            second
                .instrument()
                .acceptance(second.faults(), 256, &a)
                .unwrap()
                .value
        );
        assert_eq!(first.network_compilations(), 0);
        assert!(second.network_compilations() > 0);
    }
}

#[test]
fn cold_compilation_metadata_describes_the_initial_potential_instrument() {
    use rmagna::runtime::CausalStrategy;
    use rmagna::unitary::CompilerStrategy;
    let mut registry = ControllerRegistry::default();
    registry
        .register("one", "1", |_: &[bool]| Ok(vec![true]))
        .unwrap();
    let circuit = parse_circuit(
        "T 0\nCONTROL[one] -> ctrl[0]\nCH ctrl[0] 0\nT 0",
        &ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        },
    )
    .unwrap();
    let arithmetic = ScalarArithmetic::default();
    let program = CausalProgram::new(
        &circuit,
        &registry,
        RuntimeOptions {
            strategy: CausalStrategy::ColdPrefix,
            compile: CompileOptions {
                compiler_strategy: CompilerStrategy::SharedCommutingLayer,
                ..Default::default()
            },
            ..Default::default()
        },
        &arithmetic,
    )
    .unwrap();
    let history = program
        .resolve_prescribed_history(&[], &[], &[vec![]], &arithmetic)
        .unwrap();
    assert_eq!(program.compilation().reference_coherent_width, 2);
    assert_eq!(program.compilation().coherent_width, 1);
    assert_eq!(program.compilation().layer_slots, [2]);
    assert_eq!(program.compilation().path_variables, 0);
    assert_eq!(
        history.instrument().compilation().reference_coherent_width,
        3
    );
    assert_eq!(history.instrument().compilation().coherent_width, 2);
    assert_eq!(history.instrument().compilation().layer_slots, [1, 1]);
    assert_eq!(history.instrument().compilation().path_variables, 1);
}

#[test]
fn forced_and_zero_guarded_categories_keep_active_path_probabilities() {
    use rmagna::runtime::CausalStrategy;
    let a = ScalarArithmetic::default();
    let registry = ControllerRegistry::default();
    for rate in [0, 1] {
        let source = parse(&format!("H 0\nM 0\nX_ERROR({rate}) rec[-1] 1"), &registry);
        for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            let program = CausalProgram::new(
                &source,
                &registry,
                RuntimeOptions {
                    strategy,
                    ..Default::default()
                },
                &a,
            )
            .unwrap();
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
                    let history = program
                        .resolve_prescribed(&[], &faults, &mut Words::bits(&[active]), &a)
                        .unwrap();
                    let expected = if !active || fault == (rate == 1) {
                        1
                    } else {
                        0
                    };
                    assert_eq!(
                        history.noise_probability(),
                        &num_rational::BigRational::from_integer(expected.into())
                    );
                    assert_eq!(history.history_mass(), &a.omega(0, -2).unwrap());
                    assert_eq!(history.faults().len(), usize::from(active && fault));
                }
            }
        }
    }
}

#[test]
fn causal_draws_expose_noise_likelihood_separately_from_joint_quantum_masses() {
    use num_rational::BigRational;
    use rmagna::noise::NoiseSampling;
    use rmagna::runtime::{CausalStrategy, HistorySampler};
    let a = ScalarArithmetic::default();
    let registry = ControllerRegistry::default();
    let source = parse(
        "X_ERROR(1/10) 0\nH 1\nM 1\nX_ERROR(3/4) rec[-1] 2",
        &registry,
    );
    let sampling = NoiseSampling::Importance {
        proposal_scale: BigRational::from_integer(2.into()),
    };
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        for history_sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
            let options = RuntimeOptions {
                strategy,
                history_sampler,
                ..Default::default()
            };
            let program =
                CausalProgram::new_with_sampling(&source, &registry, options, &a, &sampling)
                    .unwrap();
            assert_eq!(
                program
                    .scaled_proposal_summary()
                    .unwrap()
                    .excluded_categories,
                1
            );
            let draw = program
                .sample(
                    &[2],
                    &zero(&a),
                    &mut Words {
                        values: vec![4, 1, u64::MAX],
                        at: 0,
                    },
                    &a,
                )
                .unwrap();
            assert_eq!(
                draw.noise_likelihood_ratio,
                BigRational::new(1.into(), 2.into())
            );
            assert_eq!(
                draw.noise_probability,
                BigRational::new(3.into(), 40.into())
            );
            assert_eq!(draw.history_mass, a.omega(0, -2).unwrap());
            assert_eq!(draw.masses.accepted.value, draw.history_mass);
            assert_eq!(draw.masses.error, draw.history_mass);
            let prescribed = program
                .resolve_prescribed_history(&[], &[], &[vec![true]], &a)
                .unwrap();
            assert_eq!(
                prescribed.noise_likelihood_ratio(),
                &BigRational::from_integer(1.into())
            );
        }
    }
}
