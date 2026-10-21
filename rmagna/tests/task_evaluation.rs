use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::instrument::FaultSelection;
use rmagna::noise::RandomSource;
use rmagna::runtime::ControllerRegistry;
use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task};
use std::path::PathBuf;
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "rmagna-evaluation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Words {
    words: Vec<u64>,
    at: usize,
}
impl Words {
    fn new(words: Vec<u64>) -> Self {
        Self { words, at: 0 }
    }
}
impl RandomSource for Words {
    fn next_u64(&mut self) -> u64 {
        let value = self.words[self.at];
        self.at += 1;
        value
    }
}
fn evaluator(
    files: &Files,
    physical: &str,
    task: &str,
    registry: &ControllerRegistry,
) -> TaskEvaluator {
    evaluator_options(
        files,
        physical,
        task,
        registry,
        EvaluationOptions::default(),
    )
}
fn evaluator_options(
    files: &Files,
    physical: &str,
    task: &str,
    registry: &ControllerRegistry,
    options: EvaluationOptions,
) -> TaskEvaluator {
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let physical = parse_circuit(physical, &parse).unwrap();
    let spec = parse_task(task, &files.0, &TaskOptions::default()).unwrap();
    let modules = spec.load_modules(&physical, &parse).unwrap();
    TaskEvaluator::new(&physical, &modules, registry, options).unwrap()
}
fn rational(n: i64, d: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(BigRational::new(n.into(), d.into()))
        .unwrap()
}
#[test]
fn pure_mixed_and_projected_targets_normalize_exactly() {
    let files = Files::new();
    let registry = ControllerRegistry::default();
    files.put("bell.magna", "H 0\nCX 0 1");
    let mixed = evaluator(
        &files,
        "I 0",
        r#"{"ideal_state":{"circuit":"bell.magna","outputs":[1]}}"#,
        &registry,
    );
    let draw = mixed
        .sample_prescribed(&[], &mut Words::new(vec![]))
        .unwrap();
    assert_eq!(draw.accepted, ExactField::one());
    assert_eq!(draw.error, rational(1, 2));
    assert_eq!(draw.target_success, Some(rational(1, 2)));
    files.put("projected.magna", "H 1\nT 1\nH 1\nCX 1 0");
    let task =
        r#"{"ideal_state":{"circuit":"projected.magna","outputs":[0],"postselection":{"1":0}}}"#;
    for (physical, wrong) in [("I 0", false), ("X 0", true)] {
        let program = evaluator(&files, physical, task, &registry);
        let draw = program
            .sample_prescribed(&[], &mut Words::new(vec![]))
            .unwrap();
        assert_eq!(
            draw.error,
            if wrong {
                ExactField::one()
            } else {
                ExactField::zero()
            }
        );
        assert_eq!(
            draw.target_success,
            Some(if wrong {
                ExactField::zero()
            } else {
                ExactField::one()
            })
        );
        let expected = rmagna::scalar::ScalarArithmetic::default()
            .scalar([2.into(), 1.into(), 0.into(), (-1).into()], -2)
            .unwrap();
        assert_eq!(draw.target_normalization, Some(expected));
    }
}
#[test]
fn zero_projected_target_is_rejected_during_preparation() {
    let files = Files::new();
    files.put("zero.magna", "I 0 1");
    let physical = parse_circuit("I 0", &ParseOptions::default()).unwrap();
    let spec = parse_task(
        r#"{"ideal_state":{"circuit":"zero.magna","outputs":[0],"postselection":{"1":1}}}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    let modules = spec
        .load_modules(&physical, &ParseOptions::default())
        .unwrap();
    let result = TaskEvaluator::new(
        &physical,
        &modules,
        &ControllerRegistry::default(),
        EvaluationOptions::default(),
    );
    assert!(result.err().unwrap().message.contains("zero norm"));
}
#[test]
fn recovery_maps_selected_wires_and_keeps_its_records_file_local() {
    let files = Files::new();
    let registry = ControllerRegistry::default();
    files.put("ideal.magna", "H 0\nT 0");
    files.put("recovery.magna", "M 1\nCX rec[-1] 0\nZ 0");
    let program = evaluator(
        &files,
        "H 2\nT 2\nZ 2\nH 0\nM 0",
        r#"{"outputs":[2],"ideal_state":"ideal.magna","recovery":"recovery.magna"}"#,
        &registry,
    );
    assert!(!program.sampled_history());
    let draw = program
        .sample_prescribed(&[], &mut Words::new(vec![]))
        .unwrap();
    assert_eq!(draw.accepted, ExactField::one());
    assert_eq!(draw.error, ExactField::zero());
    assert_eq!(draw.target_success, Some(ExactField::one()));
}
#[test]
fn recovery_controller_calls_and_records_use_distinct_namespaces() {
    let files = Files::new();
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", |bits: &[bool]| Ok(bits.to_vec()))
        .unwrap();
    files.put("ideal.magna", "I 0");
    files.put(
        "recovery.magna",
        "H 1\nM 1\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nCX ctrl[0] 0",
    );
    let program = evaluator(
        &files,
        "X 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]",
        r#"{"outputs":[0],"ideal_state":"ideal.magna","recovery":"recovery.magna"}"#,
        &registry,
    );
    for bit in [false, true] {
        let draw = program
            .sample_prescribed(&[], &mut Words::new(vec![if bit { u64::MAX } else { 0 }]))
            .unwrap();
        assert_eq!(draw.controller_calls.len(), 2);
        assert_eq!(draw.controller_calls[0].call, 0);
        assert_eq!(draw.controller_calls[1].call, 1);
        assert_eq!(draw.controller_calls[0].inputs, [true]);
        assert_eq!(draw.controller_calls[1].inputs, [bit]);
        assert_eq!(
            draw.error,
            if bit {
                ExactField::zero()
            } else {
                ExactField::one()
            }
        );
        assert_eq!(
            draw.history_mass,
            rmagna::scalar::ScalarArithmetic::default()
                .omega(0, -2)
                .unwrap()
        );
    }
}
#[test]
fn logical_error_definition_matches_exact_full_noise_enumeration() {
    let files = Files::new();
    let registry = ControllerRegistry::default();
    let program = evaluator(
        &files,
        "X_ERROR(1/5) 0\nM(1/7) 0\nOBSERVABLE_INCLUDE(0) rec[-1]",
        r#"{"error_observables":[0]}"#,
        &registry,
    );
    let mut total = ExactField::zero();
    let f = FieldArithmetic::default();
    for mask in 0_usize..4 {
        let faults = (0..2)
            .filter(|&j| mask >> j & 1 != 0)
            .map(|site| FaultSelection {
                site,
                alternative: 0,
            })
            .collect::<Vec<_>>();
        let draw = program
            .sample_prescribed(&faults, &mut Words::new(vec![]))
            .unwrap();
        assert_eq!(draw.accepted, ExactField::one());
        assert_eq!(
            draw.error,
            if mask.count_ones() % 2 == 1 {
                ExactField::one()
            } else {
                ExactField::zero()
            }
        );
        total = f
            .add(
                &total,
                &f.scale(&draw.error, &draw.noise_probability).unwrap(),
            )
            .unwrap();
    }
    assert_eq!(total, rational(2, 7));
}
#[test]
fn requested_records_are_physical_and_not_conditioned_on_selection() {
    let files = Files::new();
    let registry = ControllerRegistry::default();
    let program = evaluator(
        &files,
        "H 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]",
        r#"{"outputs":[],"sample_records":[0]}"#,
        &registry,
    );
    for bit in [false, true] {
        let draw = program
            .sample_prescribed(&[], &mut Words::new(vec![if bit { u64::MAX } else { 0 }]))
            .unwrap();
        assert_eq!(draw.sampled_records, [bit]);
        assert_eq!(
            draw.accepted,
            if bit {
                ExactField::zero()
            } else {
                ExactField::one()
            }
        );
        assert_eq!(draw.error, ExactField::zero());
    }
}
#[test]
fn output_pauli_moments_are_before_target_projection_and_after_recovery() {
    let files = Files::new();
    let registry = ControllerRegistry::default();
    files.put("ideal.magna", "I 0");
    files.put("recovery.magna", "S 0");
    let program = evaluator(
        &files,
        "H 2",
        r#"{"outputs":[2],"ideal_state":"ideal.magna","recovery":"recovery.magna","observables":[[1,0],[1,1],[0,1]]}"#,
        &registry,
    );
    let draw = program
        .sample_prescribed(&[], &mut Words::new(vec![]))
        .unwrap();
    assert_eq!(draw.error, rational(1, 2));
    assert_eq!(
        draw.moments,
        [ExactField::zero(), ExactField::one(), ExactField::zero()]
    );
}

#[test]
fn bh2_tasks_match_all_prescribed_native_fault_masses_with_one_term() {
    use rmagna::circuit::Operation;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/magna-bh2");
    let physical = parse_circuit(
        &std::fs::read_to_string(root.join("physical.magna")).unwrap(),
        &ParseOptions::default(),
    )
    .unwrap();
    let spec = parse_task(
        r#"{"outputs":[14,15],"ideal_state":"ideal.magna"}"#,
        &root,
        &TaskOptions::default(),
    )
    .unwrap();
    let modules = spec
        .load_modules(&physical, &ParseOptions::default())
        .unwrap();
    let mut options = EvaluationOptions::default();
    options.runtime.sampling.max_terms = 1;
    let program =
        TaskEvaluator::new(&physical, &modules, &ControllerRegistry::default(), options).unwrap();
    let data: serde_json::Value =
        serde_json::from_str(include_str!("data/instrument-reference.json")).unwrap();
    let mut count = 0;
    for vector in data["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["case"] == "bh2-T")
    {
        let mut faults = Vec::new();
        for fault in vector["faults"].as_array().unwrap() {
            let line = fault["line"].as_u64().unwrap() as usize;
            let op = physical
                .operations()
                .iter()
                .find(|op| {
                    op.location.line == line && matches!(op.operation, Operation::Noise { .. })
                })
                .unwrap();
            faults.push(FaultSelection {
                site: program.potential_site(op.id).unwrap(),
                alternative: 0,
            });
        }
        for record in vector["report_flips"].as_array().unwrap() {
            let record = record.as_u64().unwrap() as usize;
            let op = physical
                .operations()
                .iter()
                .find(|op| matches!(op.operation,Operation::Measure {record:r,..} if r==record))
                .unwrap();
            faults.push(FaultSelection {
                site: program.potential_site(op.id).unwrap(),
                alternative: 0,
            });
        }
        let draw = program
            .sample_prescribed(&faults, &mut Words::new(vec![]))
            .unwrap();
        let a = rmagna::scalar::ScalarArithmetic::default();
        for (actual, key) in [
            (&draw.raw_acceptance, "accepted"),
            (&draw.raw_error, "error"),
            (draw.raw_target_success.as_ref().unwrap(), "target_success"),
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
                    .unwrap()
            );
        }
        assert!(draw.acceptance_resources.terms <= 1);
        assert!(draw.target_resources.unwrap().terms <= 1);
        count += 1;
    }
    assert_eq!(count, 16);
}

#[test]
fn general_task_raw_numerators_match_pinned_native_target_recovery_and_moment_witnesses() {
    use rmagna::circuit::Operation;
    let data: serde_json::Value =
        serde_json::from_str(include_str!("data/task-reference.json")).unwrap();
    assert_eq!(data["schema"], "rmagna.task-reference.v1");
    assert_eq!(
        data["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    let vectors = data["vectors"].as_array().unwrap();
    assert_eq!(vectors.len(), 10);
    let a = rmagna::scalar::ScalarArithmetic::default();
    let expected = |value: &serde_json::Value| {
        let coefficients =
            std::array::from_fn(|j| value["coefficients"][j].as_str().unwrap().parse().unwrap());
        a.scalar(coefficients, value["power"].as_i64().unwrap())
            .unwrap()
    };
    for vector in vectors {
        let files = Files::new();
        let mut task = vector["task"].clone();
        let ideal = &vector["ideal"];
        if !ideal.is_null() {
            files.put("ideal.magna", ideal["source"].as_str().unwrap());
            task["ideal_state"] = if ideal["outputs"].is_null() {
                serde_json::json!("ideal.magna")
            } else {
                serde_json::json!({"circuit":"ideal.magna","outputs":ideal["outputs"],"postselection":ideal["postselection"]})
            };
        }
        if let Some(text) = vector["recovery"].as_str() {
            files.put("recovery.magna", text);
            task["recovery"] = serde_json::json!("recovery.magna");
        }
        let source =
            parse_circuit(vector["source"].as_str().unwrap(), &ParseOptions::default()).unwrap();
        let spec = parse_task(&task.to_string(), &files.0, &TaskOptions::default()).unwrap();
        let modules = spec
            .load_modules(&source, &ParseOptions::default())
            .unwrap();
        let program = TaskEvaluator::new(
            &source,
            &modules,
            &ControllerRegistry::default(),
            EvaluationOptions::default(),
        )
        .unwrap();
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
        let draw = program
            .sample_prescribed(&faults, &mut Words::new(vec![]))
            .unwrap();
        assert_eq!(draw.history_mass, expected(&vector["history_mass"]));
        assert_eq!(draw.raw_acceptance, expected(&vector["acceptance"]));
        assert_eq!(draw.raw_error, expected(&vector["error"]));
        assert_eq!(
            draw.target_normalization
                .clone()
                .unwrap_or_else(rmagna::scalar::ExactScalar::one),
            expected(&vector["target_normalization"])
        );
        // The native file row names classical success target_success as well.
        // This Rust type reserves its optional target field for a quantum target.
        let success = draw
            .raw_target_success
            .unwrap_or_else(|| a.subtract(&draw.raw_acceptance, &draw.raw_error).unwrap());
        assert_eq!(success, expected(&vector["target_success"]));
        assert_eq!(
            draw.raw_moments,
            vector["moments"]
                .as_array()
                .unwrap()
                .iter()
                .map(&expected)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn target_metadata_without_classical_references_preserves_actual_contributions() {
    let files = Files::new();
    let registry = ControllerRegistry::default();
    files.put("target.magna", "QUBIT_COORDS(0,1) 0\nH 0\nT 0\nREPEAT 2 {\nDETECTOR(1,2)\nOBSERVABLE_INCLUDE(3)\n}\nSHIFT_COORDS(0,1)");
    for task in [
        r#"{"ideal_state":"target.magna","observables":[[1,0],[1,1],[0,1]]}"#,
        r#"{"ideal_state":{"circuit":"target.magna","outputs":[0]},"observables":[[1,0],[1,1],[0,1]]}"#,
    ] {
        let program = evaluator(&files, "H 0\nT 0", task, &registry);
        let draw = program
            .sample_prescribed(&[], &mut Words::new(vec![]))
            .unwrap();
        assert_eq!(draw.accepted, ExactField::one());
        assert_eq!(draw.error, ExactField::zero());
        assert_eq!(draw.target_success, Some(ExactField::one()));
        let f = FieldArithmetic::default();
        let inverse_sqrt_two = f
            .value([
                BigRational::from_integer(0.into()),
                BigRational::new(1.into(), 2.into()),
                BigRational::from_integer(0.into()),
                BigRational::new((-1).into(), 2.into()),
            ])
            .unwrap();
        assert_eq!(
            draw.moments,
            vec![
                inverse_sqrt_two.clone(),
                inverse_sqrt_two,
                ExactField::zero()
            ]
        );
    }
    // Syntactic classical references stay invalid even after XOR cancellation.
    let parse = ParseOptions::default();
    let physical = parse_circuit("I 0", &parse).unwrap();
    for declaration in ["DETECTOR", "OBSERVABLE_INCLUDE(0)"] {
        files.put("target.magna", &format!("I 0\n{declaration} rec[0] rec[0]"));
        let spec = parse_task(
            r#"{"ideal_state":"target.magna"}"#,
            &files.0,
            &TaskOptions::default(),
        )
        .unwrap();
        assert!(spec.load_modules(&physical, &parse).is_err());
    }
}

#[test]
fn task_prescribed_histories_are_conditional_queries_with_physical_enumeration_weights() {
    let files = Files::new();
    let task = evaluator(
        &files,
        "H 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]",
        r#"{"sample_records":[0]}"#,
        &ControllerRegistry::default(),
    );
    let fields = FieldArithmetic::default();
    let mut total = ExactField::zero();
    for value in [false, true] {
        let draw = task
            .evaluate_prescribed_history(&[], &[vec![value]])
            .unwrap();
        assert_eq!(draw.sampled_records, [value]);
        assert_eq!(
            draw.accepted,
            if value {
                ExactField::zero()
            } else {
                ExactField::one()
            }
        );
        let weighted = fields
            .multiply(
                &draw.accepted,
                &fields.from_scalar(&draw.history_mass).unwrap(),
            )
            .unwrap();
        total = fields.add(&total, &weighted).unwrap();
    }
    assert_eq!(total, rational(1, 2));
    let linear = evaluator(&files, "I 0", "{}", &ControllerRegistry::default());
    assert!(linear.evaluate_prescribed_history(&[], &[vec![]]).is_err());
    assert_eq!(
        linear
            .evaluate_prescribed_history(&[], &[])
            .unwrap()
            .accepted,
        ExactField::one()
    );
}

#[test]
fn affine_history_tasks_use_proposal_mass_and_keep_zero_weight_draws() {
    use rmagna::runtime::HistorySampler;
    use rmagna::scalar::{ExactScalar, ScalarArithmetic};
    let files = Files::new();
    files.put("ideal.magna", "I 0");
    let registry = ControllerRegistry::default();
    let a = ScalarArithmetic::default();
    let f = FieldArithmetic::default();
    let mut options = EvaluationOptions::default();
    options.runtime.history_sampler = HistorySampler::AffineSupport;
    for cancel in [false, true] {
        let source = if cancel {
            "H 0\nT 0\nT_DAG 0\nH 0\nM 0"
        } else {
            "H 0\nT 0\nH 0\nM 0"
        };
        let program = evaluator_options(
            &files,
            source,
            r#"{"outputs":[0],"ideal_state":"ideal.magna","sample_records":[0],"observables":[[0,1]]}"#,
            &registry,
            options,
        );
        let mut accepted = ExactField::zero();
        let mut error = ExactField::zero();
        let mut success = ExactField::zero();
        let mut moment = ExactField::zero();
        for bit in [false, true] {
            let draw = program
                .sample_prescribed(&[], &mut Words::new(vec![u64::from(bit)]))
                .unwrap();
            assert_eq!(draw.sampled_records, [bit]);
            assert_eq!(draw.history_proposal_mass, Some(a.omega(0, -2).unwrap()));
            let p = if cancel {
                if bit {
                    ExactScalar::zero()
                } else {
                    ExactScalar::one()
                }
            } else if bit {
                a.scalar([2.into(), (-1).into(), 0.into(), 1.into()], -2)
                    .unwrap()
            } else {
                a.scalar([2.into(), 1.into(), 0.into(), (-1).into()], -2)
                    .unwrap()
            };
            assert_eq!(draw.history_mass, p);
            let weight = f
                .multiply(&f.from_scalar(&p).unwrap(), &rational(2, 1))
                .unwrap();
            assert_eq!(draw.history_weight, weight);
            assert_eq!(draw.accepted, weight);
            assert_eq!(
                draw.error,
                if bit {
                    weight.clone()
                } else {
                    ExactField::zero()
                }
            );
            assert_eq!(
                draw.target_success,
                Some(if bit {
                    ExactField::zero()
                } else {
                    weight.clone()
                })
            );
            assert_eq!(
                draw.moments,
                [if bit {
                    f.multiply(&weight, &rational(-1, 1)).unwrap()
                } else {
                    weight
                }]
            );
            accepted = f
                .add(
                    &accepted,
                    &f.multiply(&draw.accepted, &rational(1, 2)).unwrap(),
                )
                .unwrap();
            error = f
                .add(&error, &f.multiply(&draw.error, &rational(1, 2)).unwrap())
                .unwrap();
            success = f
                .add(
                    &success,
                    &f.multiply(draw.target_success.as_ref().unwrap(), &rational(1, 2))
                        .unwrap(),
                )
                .unwrap();
            moment = f
                .add(
                    &moment,
                    &f.multiply(&draw.moments[0], &rational(1, 2)).unwrap(),
                )
                .unwrap();
        }
        let physical_error = if cancel {
            ExactScalar::zero()
        } else {
            a.scalar([2.into(), (-1).into(), 0.into(), 1.into()], -2)
                .unwrap()
        };
        assert_eq!(accepted, ExactField::one());
        assert_eq!(error, f.from_scalar(&physical_error).unwrap());
        assert_eq!(success, f.subtract(&ExactField::one(), &error).unwrap());
        assert_eq!(
            moment,
            f.subtract(
                &ExactField::one(),
                &f.multiply(&rational(2, 1), &error).unwrap()
            )
            .unwrap()
        );
        let prescribed = program
            .evaluate_prescribed_history(&[], &[vec![false]])
            .unwrap();
        assert!(prescribed.history_proposal_mass.is_none());
        assert_eq!(prescribed.history_weight, ExactField::one());
        assert_eq!(prescribed.accepted, ExactField::one());
        if cancel {
            assert!(
                program
                    .evaluate_prescribed_history(&[], &[vec![true]])
                    .is_err()
            );
        }
    }
}

#[test]
fn affine_causal_feedback_and_guarded_noise_are_unbiased_over_all_histories() {
    use rmagna::runtime::{CausalStrategy, HistorySampler};
    use rmagna::scalar::{ExactScalar, ScalarArithmetic};
    use std::collections::BTreeMap;
    let files = Files::new();
    files.put("ideal.magna", "I 0");
    let mut registry = ControllerRegistry::default();
    registry
        .register("copy", "1", |bits: &[bool]| Ok(vec![bits[0]]))
        .unwrap();
    registry
        .register("xor", "1", |bits: &[bool]| Ok(vec![bits[0] ^ bits[1]]))
        .unwrap();
    let source = "H 0\nT 0\nH 0\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCH ctrl[0] 1\nM 1\nDETECTOR rec[-1]\nCONTROL[xor] det[1] ctrl[0] -> ctrl[1]\nCX ctrl[1] 2\nX_ERROR(1/3) ctrl[0] 2\nM 2\nPOSTSELECT det[1]";
    let a = ScalarArithmetic::default();
    let f = FieldArithmetic::default();
    let p1 = a
        .scalar([2.into(), (-1).into(), 0.into(), 1.into()], -2)
        .unwrap();
    let p0 = a.subtract(&ExactScalar::one(), &p1).unwrap();
    let expected_accepted = f
        .add(
            &f.from_scalar(&p0).unwrap(),
            &f.multiply(&f.from_scalar(&p1).unwrap(), &rational(1, 2))
                .unwrap(),
        )
        .unwrap();
    let expected_error = f
        .multiply(&f.from_scalar(&p1).unwrap(), &rational(1, 3))
        .unwrap();
    for (strategy, compiler_strategy) in [
        (
            CausalStrategy::FixedTemplate,
            rmagna::unitary::CompilerStrategy::ParityCopies,
        ),
        (
            CausalStrategy::ColdPrefix,
            rmagna::unitary::CompilerStrategy::ParityCopies,
        ),
        (
            CausalStrategy::FixedTemplate,
            rmagna::unitary::CompilerStrategy::SharedCommutingLayer,
        ),
        (
            CausalStrategy::ColdPrefix,
            rmagna::unitary::CompilerStrategy::SharedCommutingLayer,
        ),
    ] {
        let mut options = EvaluationOptions::default();
        options.runtime.history_sampler = HistorySampler::AffineSupport;
        options.runtime.strategy = strategy;
        options.runtime.compile.compiler_strategy = compiler_strategy;
        let program = evaluator_options(
            &files,
            source,
            r#"{"outputs":[2],"ideal_state":"ideal.magna","sample_records":[0,1,2]}"#,
            &registry,
            options,
        );
        let mut accepted = ExactField::zero();
        let mut error = ExactField::zero();
        let mut success = ExactField::zero();
        let mut combined_proposal = ExactField::zero();
        for fault in [false, true] {
            let faults = if fault {
                vec![FaultSelection {
                    site: 2,
                    alternative: 0,
                }]
            } else {
                vec![]
            };
            let mut histories = BTreeMap::new();
            for words in 0..8_u64 {
                let draw = program
                    .sample_prescribed(
                        &faults,
                        &mut Words::new((0..3).map(|j| words >> j & 1).collect()),
                    )
                    .unwrap();
                let key = draw.sampled_records.clone();
                if let Some(previous) = histories.insert(key, draw) {
                    let current = histories.get(&previous.sampled_records).unwrap();
                    assert_eq!(
                        current.history_proposal_mass,
                        previous.history_proposal_mass
                    );
                    assert_eq!(current.raw_acceptance, previous.raw_acceptance);
                }
            }
            let mut proposal_sum = ExactField::zero();
            for (bits, draw) in histories {
                let q = f
                    .from_scalar(draw.history_proposal_mass.as_ref().unwrap())
                    .unwrap();
                proposal_sum = f.add(&proposal_sum, &q).unwrap();
                let (b0, b1, b2) = (bits[0], bits[1], bits[2]);
                assert_eq!(b2, b0 ^ b1 ^ (b0 && fault));
                assert_eq!(draw.controller_calls.len(), 2);
                assert_eq!(draw.controller_calls[0].inputs, [b0]);
                assert_eq!(draw.controller_calls[1].inputs, [b1, b0]);
                assert_eq!(draw.controller_calls[1].outputs, [b0 ^ b1]);
                let physical = if b0 {
                    a.multiply(&p1, &a.omega(0, -2).unwrap()).unwrap()
                } else if b1 {
                    ExactScalar::zero()
                } else {
                    p0.clone()
                };
                assert_eq!(draw.history_mass, physical);
                let conditional = f.divide(&f.from_scalar(&physical).unwrap(), &q).unwrap();
                assert_eq!(
                    draw.accepted,
                    if b1 {
                        ExactField::zero()
                    } else {
                        conditional.clone()
                    }
                );
                assert_eq!(
                    draw.error,
                    if !b1 && b2 {
                        conditional
                    } else {
                        ExactField::zero()
                    }
                );
                let noise = if b0 {
                    BigRational::new(if fault { 1.into() } else { 2.into() }, 3.into())
                } else {
                    BigRational::from_integer(1.into())
                };
                assert_eq!(draw.noise_probability, noise);
                if fault && !b0 {
                    continue;
                } // The inactive channel has one physical identity path.
                let probability = f.multiply(&q, &f.from_rational(noise).unwrap()).unwrap();
                combined_proposal = f.add(&combined_proposal, &probability).unwrap();
                accepted = f
                    .add(
                        &accepted,
                        &f.multiply(&probability, &draw.accepted).unwrap(),
                    )
                    .unwrap();
                error = f
                    .add(&error, &f.multiply(&probability, &draw.error).unwrap())
                    .unwrap();
                success = f
                    .add(
                        &success,
                        &f.multiply(&probability, draw.target_success.as_ref().unwrap())
                            .unwrap(),
                    )
                    .unwrap();
            }
            assert_eq!(proposal_sum, ExactField::one());
        }
        assert_eq!(combined_proposal, ExactField::one());
        assert_eq!(accepted, expected_accepted);
        assert_eq!(error, expected_error);
        assert_eq!(success, f.subtract(&accepted, &error).unwrap());
    }
}

#[test]
fn affine_majority_inputs_and_repeated_diagnostics_use_one_entropy_word_without_selection() {
    use rmagna::runtime::{CausalStrategy, HistorySampler};
    use rmagna::scalar::ScalarArithmetic;
    let files = Files::new();
    files.put("ideal.magna", "I 0");
    let mut registry = ControllerRegistry::default();
    registry
        .register("majority", "1", |bits: &[bool]| {
            if bits.len() != 4 || bits[0] != bits[3] {
                return Err("expected three inputs and a repeated first observation".into());
            }
            Ok(vec![bits[..3].iter().filter(|b| **b).count() >= 2])
        })
        .unwrap();
    let source = "H 0 1 2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[majority] det[0] det[1] det[2] det[0] -> ctrl[0]\nCX ctrl[0] 3\nM 3\nPOSTSELECT det[0]";
    let f = FieldArithmetic::default();
    let a = ScalarArithmetic::default();
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        let mut options = EvaluationOptions::default();
        options.runtime.history_sampler = HistorySampler::AffineSupport;
        options.runtime.strategy = strategy;
        options.runtime.sampling.max_entropy_words = 1;
        let program = evaluator_options(
            &files,
            source,
            r#"{"outputs":[3],"ideal_state":"ideal.magna","sample_records":[0,1,2,3,0],"observables":[[0,1]]}"#,
            &registry,
            options,
        );
        let mut accepted = ExactField::zero();
        let mut error = ExactField::zero();
        let mut success = ExactField::zero();
        let mut moment = ExactField::zero();
        for word in 0..8 {
            let mut entropy = Words::new(vec![word]);
            let draw = program.sample_prescribed(&[], &mut entropy).unwrap();
            assert_eq!(entropy.at, 1);
            let bits = [word & 1 != 0, word & 2 != 0, word & 4 != 0];
            let majority = bits.iter().filter(|b| **b).count() >= 2;
            assert_eq!(
                draw.sampled_records,
                [bits[0], bits[1], bits[2], majority, bits[0]]
            );
            assert_eq!(draw.controller_calls.len(), 1);
            assert_eq!(
                draw.controller_calls[0].inputs,
                [bits[0], bits[1], bits[2], bits[0]]
            );
            assert_eq!(draw.controller_calls[0].outputs, [majority]);
            assert_eq!(draw.history_proposal_mass, Some(a.omega(0, -6).unwrap()));
            assert_eq!(draw.history_mass, a.omega(0, -6).unwrap());
            assert_eq!(draw.history_weight, ExactField::one());
            assert_eq!(draw.noise_probability, BigRational::from_integer(1.into()));
            assert_eq!(
                draw.accepted,
                if bits[0] {
                    ExactField::zero()
                } else {
                    ExactField::one()
                }
            );
            assert_eq!(
                draw.error,
                if !bits[0] && majority {
                    ExactField::one()
                } else {
                    ExactField::zero()
                }
            );
            let probability = rational(1, 8);
            accepted = f
                .add(
                    &accepted,
                    &f.multiply(&probability, &draw.accepted).unwrap(),
                )
                .unwrap();
            error = f
                .add(&error, &f.multiply(&probability, &draw.error).unwrap())
                .unwrap();
            success = f
                .add(
                    &success,
                    &f.multiply(&probability, draw.target_success.as_ref().unwrap())
                        .unwrap(),
                )
                .unwrap();
            moment = f
                .add(
                    &moment,
                    &f.multiply(&probability, &draw.moments[0]).unwrap(),
                )
                .unwrap();
        }
        assert_eq!(accepted, rational(1, 2));
        assert_eq!(error, rational(1, 8));
        assert_eq!(success, rational(3, 8));
        assert_eq!(moment, rational(1, 4));
    }
}
