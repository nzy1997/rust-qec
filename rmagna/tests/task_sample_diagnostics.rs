// SPDX-License-Identifier: MPL-2.0
// Three complete TerminalEvaluationTests methods from itpyi/magna commit
// 56847478c840395735146ef1e1ccdc3d8aedc10f, tests/core/test_terminal_evaluation.py.
// Optional diagnostics observe the same Rust draw; native RNG ABI is distinct.
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::noise::{RandomSource, SplitMix64};
use rmagna::runtime::{ControllerRegistry, HistorySampler};
use rmagna::task::{
    EvaluationOptions, TaskEvaluation, TaskEvaluator, TaskOptions, TaskRunOptions,
    TaskSampleDiagnosticOptions, TaskSampleDiagnostics, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
struct Harness {
    path: PathBuf,
    calls: Arc<Mutex<Vec<Vec<bool>>>>,
}
impl Harness {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-task-diagnostics-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self {
            path,
            calls: Arc::new(Mutex::new(vec![])),
        }
    }
    fn evaluator(
        &self,
        physical: &str,
        ideal: Option<&str>,
        recovery: Option<&str>,
        mut task: Value,
        config: (CompilerStrategy, HistorySampler),
    ) -> TaskEvaluator {
        let mut registry = ControllerRegistry::default();
        let observed = self.calls.clone();
        registry
            .register("echo", "1", move |bits: &[bool]| {
                observed.lock().unwrap().push(bits.to_vec());
                Ok(bits.to_vec())
            })
            .unwrap();
        if let Some(text) = ideal {
            std::fs::write(self.path.join("ideal.magna"), text).unwrap();
            task["ideal_state"] = json!("ideal.magna");
        }
        if let Some(text) = recovery {
            std::fs::write(self.path.join("recovery.magna"), text).unwrap();
            task["recovery"] = json!("recovery.magna");
        }
        let parse = ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        };
        let source = parse_circuit(physical, &parse).unwrap();
        let modules = parse_task(&task.to_string(), &self.path, &TaskOptions::default())
            .unwrap()
            .load_modules(&source, &parse)
            .unwrap();
        let mut options = EvaluationOptions::default();
        options.runtime.compile.compiler_strategy = config.0;
        options.runtime.history_sampler = config.1;
        TaskEvaluator::new(&source, &modules, &registry, options).unwrap()
    }
    fn pair(
        &self,
        evaluator: &TaskEvaluator,
        seed: u64,
        options: TaskSampleDiagnosticOptions,
    ) -> (TaskEvaluation, TaskSampleDiagnostics) {
        let before = self.calls.lock().unwrap().len();
        let mut left = Counted::new(seed);
        let ordinary = evaluator.sample(&mut left).unwrap();
        let middle = self.calls.lock().unwrap().len();
        let mut right = Counted::new(seed);
        let (observed, diagnostics) = evaluator
            .sample_with_diagnostics(&mut right, &options)
            .unwrap();
        let log = self.calls.lock().unwrap();
        assert!(
            ordinary == observed,
            "diagnostics changed the ordinary exact result"
        );
        assert_eq!(left.source, right.source);
        assert_eq!(left.words, right.words);
        assert_eq!(
            diagnostics.noise_entropy_words + diagnostics.history_entropy_words,
            right.words
        );
        assert_eq!(&log[before..middle], &log[middle..]);
        assert_eq!(middle - before, ordinary.controller_calls.len());
        assert_eq!(log.len() - middle, observed.controller_calls.len());
        assert_eq!(
            diagnostics.protocol_controller_calls + diagnostics.recovery_controller_calls,
            observed.controller_calls.len()
        );
        (observed, diagnostics)
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
struct Counted {
    source: SplitMix64,
    words: usize,
}
impl Counted {
    fn new(seed: u64) -> Self {
        Self {
            source: SplitMix64::new(seed),
            words: 0,
        }
    }
}
impl RandomSource for Counted {
    fn next_u64(&mut self) -> u64 {
        self.words += 1;
        self.source.next_u64()
    }
}
fn configurations() -> impl Iterator<Item = (CompilerStrategy, HistorySampler)> {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
    .into_iter()
    .flat_map(|compiler| {
        [HistorySampler::ExactMarginal, HistorySampler::AffineSupport]
            .into_iter()
            .map(move |sampler| (compiler, sampler))
    })
}
#[test]
fn original_terminal_namespace_report_errors_and_external_decoders() {
    let h = Harness::new();
    for config in configurations() {
        let evaluator = h.evaluator(
            "H 0\nM(1) 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCX ctrl[0] 1",
            Some("I 0"),
            Some("M 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCX ctrl[0] 0"),
            json!({"outputs":[1]}),
            config,
        );
        for seed in 0..8 {
            let (draw, diagnostics) = h.pair(&evaluator, seed, Default::default());
            assert_eq!(diagnostics.learned_record_rank, 2);
            assert_eq!(diagnostics.protocol_controller_calls, 1);
            assert_eq!(diagnostics.recovery_controller_calls, 1);
            assert_eq!(draw.error, ExactField::zero());
            assert_eq!(draw.accepted, ExactField::one());
            assert_eq!(draw.controller_calls.len(), 2);
            assert_eq!(draw.controller_calls[0].name, "echo");
            assert_eq!(draw.controller_calls[1].name, "echo");
            assert_eq!(draw.controller_calls[0].call, 0);
            assert_eq!(draw.controller_calls[1].call, 1);
            assert_eq!(
                draw.controller_calls[0].inputs,
                draw.controller_calls[1].inputs
            );
            assert_eq!(
                draw.controller_calls[0].inputs,
                draw.controller_calls[0].outputs
            );
            assert_eq!(
                draw.controller_calls[1].inputs,
                draw.controller_calls[1].outputs
            );
        }
    }
}
#[test]
fn original_affine_recovery_integrates_records_and_moments_before_target() {
    let h = Harness::new();
    for config in configurations() {
        let evaluator = h.evaluator("H 0", Some("I 0"), Some("M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\nCONTROL[XOR] rec[-1] -> ctrl[0]\nCX ctrl[0] 0"), json!({"outputs":[0],"observables":[[0,1]]}), config);
        assert!(!evaluator.sampled_history());
        let (draw, diagnostics) = h.pair(&evaluator, 1, Default::default());
        assert_eq!(diagnostics, TaskSampleDiagnostics::default());
        assert_eq!(draw.error, ExactField::zero());
        assert_eq!(draw.moments, [ExactField::one()]);
        let reset = h.evaluator(
            "H 0",
            Some("X 0"),
            Some("R 0"),
            json!({"outputs":[0]}),
            config,
        );
        let (draw, diagnostics) = h.pair(&reset, 1, Default::default());
        assert_eq!(diagnostics.learned_record_rank, 0);
        assert_eq!(draw.error, ExactField::one());
        let batch = reset
            .run_batch(&TaskRunOptions {
                shots: 2,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(batch.completed, 2);
        assert_eq!(batch.statistics.unwrap().error, Some(ExactField::one()));
    }
}
#[test]
fn original_nonclifford_terminal_feedback_and_multiple_calls() {
    let h = Harness::new();
    let three_quarters = FieldArithmetic::default()
        .from_rational(BigRational::new(3.into(), 4.into()))
        .unwrap();
    for config in configurations() {
        for first_gate in ["H", "I"] {
            let recovery = format!(
                "{first_gate} 1\nM 1\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCS ctrl[0] 0\nCONTROL[echo] ctrl[0] -> ctrl[1]\nCH ctrl[1] 0"
            );
            let evaluator = h.evaluator(
                "H 0\nT 0",
                Some("H 0\nT 0"),
                Some(&recovery),
                json!({"outputs":[0]}),
                config,
            );
            for seed in 0..8 {
                let (draw, diagnostics) = h.pair(&evaluator, seed, Default::default());
                assert_eq!(diagnostics.learned_record_rank, 1);
                assert_eq!(diagnostics.protocol_controller_calls, 0);
                assert_eq!(diagnostics.recovery_controller_calls, 2);
                assert_eq!(draw.history_queries, 2);
                assert_eq!(draw.controller_calls.len(), 2);
                let first = &draw.controller_calls[0];
                let second = &draw.controller_calls[1];
                assert_eq!(first.inputs.len(), 1);
                assert_eq!(first.inputs, second.inputs);
                assert_eq!(first.outputs, second.inputs);
                assert_eq!(second.inputs, second.outputs);
                assert_eq!(
                    draw.error,
                    if first.inputs[0] {
                        three_quarters.clone()
                    } else {
                        ExactField::zero()
                    }
                );
                if first_gate == "I" {
                    assert_eq!(diagnostics.history_entropy_words, 0);
                    assert!(!first.inputs[0]);
                }
            }
        }
    }
}
#[test]
fn literal_rank_rows_entropy_and_resource_failure_scope() {
    let h = Harness::new();
    for config in configurations() {
        for (physical, rank, calls, deterministic) in [
            ("I 0", 0, 0, true),
            ("X_ERROR(1/3) 0", 0, 0, true),
            (
                "X_ERROR(1/3) 0\nH 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]",
                1,
                1,
                false,
            ),
            (
                "I 0\nCONTROL[XOR](0) -> ctrl[0]\nCONTROL[echo] ctrl[0] -> ctrl[1]",
                0,
                1,
                true,
            ),
            (
                "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] det[0] -> ctrl[0] ctrl[1]",
                1,
                1,
                false,
            ),
            (
                "H 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[echo] det[0] det[1] -> ctrl[0] ctrl[1]",
                2,
                1,
                false,
            ),
            (
                "I 0\nM 0\nM 0\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[echo] det[0] det[1] -> ctrl[0] ctrl[1]",
                2,
                1,
                true,
            ),
        ] {
            let evaluator = h.evaluator(physical, None, None, json!({}), config);
            let (draw, diagnostics) = h.pair(&evaluator, 13, Default::default());
            assert_eq!(diagnostics.learned_record_rank, rank);
            assert_eq!(diagnostics.protocol_controller_calls, calls);
            assert_eq!(diagnostics.recovery_controller_calls, 0);
            if deterministic {
                assert_eq!(diagnostics.history_entropy_words, 0);
            } else {
                assert!(diagnostics.history_entropy_words > 0);
            }
            if physical.starts_with("X_ERROR") {
                assert!(diagnostics.noise_entropy_words > 0);
            } else {
                assert_eq!(diagnostics.noise_entropy_words, 0);
            }
            assert_eq!(draw.history_queries, calls);
        }
        let mut source = String::from("I 0\n");
        for _ in 0..66 {
            source.push_str("M 0\nDETECTOR rec[-1]\n");
        }
        source.push_str("CONTROL[echo]");
        for i in 0..66 {
            source.push_str(&format!(" det[{i}]"));
        }
        source.push_str(" ->");
        for i in 0..66 {
            source.push_str(&format!(" ctrl[{i}]"));
        }
        let evaluator = h.evaluator(&source, None, None, json!({}), config);
        let options = TaskSampleDiagnosticOptions {
            max_record_bits: 66,
            max_conditions: 66,
            max_basis_words: 132,
        };
        let (draw, diagnostics) = h.pair(&evaluator, 11, options);
        assert_eq!(diagnostics.learned_record_rank, 66);
        assert_eq!(diagnostics.history_entropy_words, 0);
        assert_eq!(draw.history_queries, 1);
        assert_eq!(draw.controller_calls[0].inputs, vec![false; 66]);
        for (limits, before_resolution) in [
            (
                TaskSampleDiagnosticOptions {
                    max_record_bits: 65,
                    ..options
                },
                true,
            ),
            (
                TaskSampleDiagnosticOptions {
                    max_conditions: 65,
                    ..options
                },
                false,
            ),
            (
                TaskSampleDiagnosticOptions {
                    max_basis_words: 131,
                    ..options
                },
                false,
            ),
        ] {
            let before = h.calls.lock().unwrap().len();
            let mut rng = Counted::new(11);
            let Err(error) = evaluator.sample_with_diagnostics(&mut rng, &limits) else {
                panic!("diagnostic limit accepted");
            };
            assert_eq!(error.field, "diagnostics");
            assert!(error.message.contains("limit"));
            assert_eq!(rng.words, 0);
            assert_eq!(
                h.calls.lock().unwrap().len() - before,
                usize::from(!before_resolution)
            );
        }
        let empty = h.evaluator("I 0", None, None, json!({}), config);
        let (_, diagnostics) = h.pair(
            &empty,
            0,
            TaskSampleDiagnosticOptions {
                max_record_bits: 0,
                max_conditions: 0,
                max_basis_words: 0,
            },
        );
        assert_eq!(diagnostics, TaskSampleDiagnostics::default());
    }
}
