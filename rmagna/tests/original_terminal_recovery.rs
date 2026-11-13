// SPDX-License-Identifier: MPL-2.0
// Complete repetition-recovery and joint-syndrome TerminalEvaluationTests from
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_terminal_evaluation.py.
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::ExactField;
use rmagna::noise::SplitMix64;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{
    EvaluationOptions, RunStatus, TaskEvaluator, TaskOptions, TaskRunOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use serde_json::json;
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rmagna-original-terminal-recovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn evaluator(
        &self,
        source: &str,
        target: &str,
        recovery: Option<&str>,
        registry: &ControllerRegistry,
        options: EvaluationOptions,
        output: Option<usize>,
    ) -> TaskEvaluator {
        std::fs::write(self.0.join("target.magna"), target).unwrap();
        let mut task = json!({"ideal_state":"target.magna"});
        if let Some(recovery) = recovery {
            std::fs::write(self.0.join("recovery.magna"), recovery).unwrap();
            task["recovery"] = json!("recovery.magna");
        }
        if let Some(output) = output {
            task["outputs"] = json!([output]);
        }
        let parse = ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        };
        let physical = parse_circuit(source, &parse).unwrap();
        let modules = parse_task(&task.to_string(), &self.0, &TaskOptions::default())
            .unwrap()
            .load_modules(&physical, &parse)
            .unwrap();
        TaskEvaluator::new(&physical, &modules, registry, options).unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn configurations() -> Vec<EvaluationOptions> {
    let mut result = vec![];
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
            for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
                let mut options = EvaluationOptions::default();
                options.runtime.compile.compiler_strategy = compiler;
                options.runtime.history_sampler = sampler;
                options.runtime.strategy = strategy;
                result.push(options);
            }
        }
    }
    result
}
#[test]
fn original_repetition_recovery_preserves_encoded_magic_and_original_faults() {
    let files = Files::new();
    let target = "H 0\nT 0\nCX 0 1\nCX 0 2";
    let recovery = "MPP Z0*Z1 Z1*Z2\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[decode] det[-2] det[-1] -> ctrl[0] ctrl[1] ctrl[2]\nCX ctrl[0] 0\nCX ctrl[1] 1\nCX ctrl[2] 2";
    let cases = [
        ("", [false, false], [false, false, false], false),
        ("X_ERROR(1) 0", [true, false], [true, false, false], false),
        ("X_ERROR(1) 1", [true, true], [false, true, false], false),
        ("X_ERROR(1) 2", [false, true], [false, false, true], false),
        ("X_ERROR(1) 0 1", [false, true], [false, false, true], true),
    ];
    let mut scenarios = 0;
    for options in configurations() {
        for (faults, syndrome, correction, logical) in cases {
            let seen = Arc::new(Mutex::new(Vec::new()));
            let captured = seen.clone();
            let mut registry = ControllerRegistry::default();
            registry
                .register("decode", "repetition-v1", move |bits: &[bool]| {
                    let result = match bits {
                        [false, false] => vec![false, false, false],
                        [true, false] => vec![true, false, false],
                        [true, true] => vec![false, true, false],
                        [false, true] => vec![false, false, true],
                        _ => return Err("expected two syndrome bits".into()),
                    };
                    captured
                        .lock()
                        .unwrap()
                        .push((bits.to_vec(), result.clone()));
                    Ok(result)
                })
                .unwrap();
            let physical = format!("{target}\n{faults}");
            let e = files.evaluator(&physical, target, Some(recovery), &registry, options, None);
            let draw = e.sample(&mut SplitMix64::new(1)).unwrap();
            let expected = if logical {
                ScalarArithmetic::default()
                    .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
                    .unwrap()
            } else {
                ExactScalar::zero()
            };
            let field = rmagna::field::FieldArithmetic::default()
                .from_scalar(&expected)
                .unwrap();
            assert_eq!(draw.raw_error, expected);
            assert_eq!(draw.error, field);
            assert_eq!(draw.accepted, ExactField::one());
            assert_eq!(draw.controller_calls.len(), 1);
            assert_eq!(draw.controller_calls[0].inputs, syndrome);
            assert_eq!(draw.controller_calls[0].outputs, correction);
            assert_eq!(
                seen.lock().unwrap().as_slice(),
                &[(syndrome.to_vec(), correction.to_vec())]
            );
            let batch = e
                .run_batch(&TaskRunOptions {
                    shots: 2,
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(batch.status, RunStatus::Complete);
            assert_eq!(
                (batch.completed, batch.failed, batch.not_attempted),
                (2, 0, 0)
            );
            let summary = batch.statistics.unwrap();
            assert_eq!(summary.accepted, ExactField::one());
            assert_eq!(summary.error, Some(field.clone()));
            assert_eq!(summary.conditional_error, Some(field));
            assert_eq!(
                seen.lock().unwrap().as_slice(),
                &vec![(syndrome.to_vec(), correction.to_vec()); 3]
            );
            if !faults.is_empty() {
                let bare = files
                    .evaluator(&physical, target, None, &registry, options, None)
                    .sample(&mut SplitMix64::new(1))
                    .unwrap();
                assert_eq!(bare.error, ExactField::one());
                assert_eq!(bare.raw_error, ExactScalar::one());
                assert!(bare.controller_calls.is_empty());
                assert_eq!(seen.lock().unwrap().len(), 3);
            }
            scenarios += 1;
        }
    }
    assert_eq!(scenarios, 40);
}
#[test]
fn original_recovery_samples_joint_syndromes_without_acceptance_or_target_bias() {
    let files = Files::new();
    let physical = "H 0\nCX 0 1\nM 1\nDETECTOR rec[-1]\nPOSTSELECT !det[0]";
    let recovery = "M 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCX ctrl[0] 0";
    let mut pairs = 0;
    for options in configurations() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let captured = seen.clone();
        let mut registry = ControllerRegistry::default();
        registry
            .register("echo", "1", move |bits: &[bool]| {
                captured.lock().unwrap().push(bits.to_vec());
                Ok(bits.to_vec())
            })
            .unwrap();
        let left = files.evaluator(physical, "I 0", Some(recovery), &registry, options, Some(0));
        let right = files.evaluator(physical, "X 0", Some(recovery), &registry, options, Some(0));
        let mut outcomes = BTreeSet::new();
        for seed in 0..12 {
            let before = seen.lock().unwrap().len();
            let mut a = SplitMix64::new(seed);
            let mut b = SplitMix64::new(seed);
            let da = left.sample(&mut a).unwrap();
            let db = right.sample(&mut b).unwrap();
            assert_eq!(a, b);
            let history = seen.lock().unwrap();
            assert_eq!(history.len(), before + 2);
            assert_eq!(history[before].len(), 1);
            assert_eq!(history[before], history[before + 1]);
            let bit = history[before][0];
            outcomes.insert(bit);
            let acceptance = if bit {
                ExactField::one()
            } else {
                ExactField::zero()
            };
            assert_eq!(da.accepted, acceptance);
            assert_eq!(db.accepted, acceptance);
            assert_eq!(da.error, ExactField::zero());
            assert_eq!(db.error, acceptance);
            for draw in [&da, &db] {
                assert_eq!(draw.controller_calls.len(), 1);
                assert_eq!(draw.controller_calls[0].inputs, [bit]);
                assert_eq!(draw.controller_calls[0].outputs, [bit]);
                assert_eq!(draw.history_weight, ExactField::one());
                assert_eq!(
                    draw.history_mass,
                    ScalarArithmetic::default()
                        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
                        .unwrap()
                );
            }
            pairs += 1;
        }
        assert_eq!(outcomes, BTreeSet::from([false, true]));
        assert_eq!(seen.lock().unwrap().len(), 24);
    }
    assert_eq!(pairs, 96);
}
