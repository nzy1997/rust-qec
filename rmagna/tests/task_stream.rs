use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::{ExecutionCosts, ExecutionLimits};
use rmagna::field::FieldArithmetic;
use rmagna::noise::{NoiseSampling, SplitMix64};
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::statistics::StatisticsOptions;
use rmagna::task::{
    EvaluationOptions, RunPhase, RunStatus, StreamRecordRetention, TaskEvaluator, TaskOptions,
    TaskRunOptions, TaskStreamOptions, TaskStreamStatus, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rmagna-stream-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        std::fs::write(p.join("ideal.magna"), "I 0").unwrap();
        Self(p)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn evaluator(
    files: &Files,
    source: &str,
    task: &str,
    registry: &ControllerRegistry,
    options: EvaluationOptions,
    sampling: &NoiseSampling,
) -> TaskEvaluator {
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let source = parse_circuit(source, &parse).unwrap();
    let modules = parse_task(task, &files.0, &TaskOptions::default())
        .unwrap()
        .load_modules(&source, &parse)
        .unwrap();
    TaskEvaluator::new_with_sampling(&source, &modules, registry, options, sampling).unwrap()
}
fn counts(c: ExecutionCosts) -> [(u64, u64, u64); 4] {
    [c.compilation, c.query, c.decoder, c.support].map(|r| (r.attempted, r.succeeded, r.failed))
}
#[test]
fn chunk_partitions_preserve_batch_statistics_rng_records_callbacks_and_shared_costs() {
    let files = Files::new();
    let causal_task = r#"{"outputs":[2],"ideal_state":"ideal.magna","sample_records":[0,1],"observables":[[1,0],[1,1]]}"#;
    let linear_task = r#"{"outputs":[2],"ideal_state":"ideal.magna","observables":[[1,0],[1,1]]}"#;
    for causal in [
        None,
        Some(CausalStrategy::FixedTemplate),
        Some(CausalStrategy::ColdPrefix),
    ] {
        for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
            for compiler in [
                CompilerStrategy::ParityCopies,
                CompilerStrategy::SharedCommutingLayer,
            ] {
                for importance in [false, true] {
                    let trace = Arc::new(Mutex::new(Vec::new()));
                    let log = Arc::clone(&trace);
                    let mut registry = ControllerRegistry::default();
                    registry
                        .register("and", "1", move |bits: &[bool]| {
                            log.lock().unwrap().push(bits.to_vec());
                            Ok(vec![bits[0] && bits[1]])
                        })
                        .unwrap();
                    let source = if causal.is_some() {
                        "H 0 1\nT 0\nH 0\nX_ERROR(1/7) 0\nM(1/11) 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[0] det[1] -> ctrl[0]\nCH ctrl[0] 2"
                    } else {
                        "H 0 1\nT 0\nH 0\nX_ERROR(1/7) 0\nM(1/11) 0 1\nI 2"
                    };
                    let mut evaluation = EvaluationOptions::default();
                    evaluation.runtime.history_sampler = sampler;
                    evaluation.runtime.strategy = causal.unwrap_or(CausalStrategy::FixedTemplate);
                    evaluation.runtime.compile.compiler_strategy = compiler;
                    let sampling = if importance {
                        NoiseSampling::Importance {
                            proposal_scale: BigRational::from_integer(2.into()),
                        }
                    } else {
                        NoiseSampling::Physical
                    };
                    let task = if causal.is_some() {
                        causal_task
                    } else {
                        linear_task
                    };
                    let e = evaluator(&files, source, task, &registry, evaluation, &sampling);
                    assert_eq!(
                        e.execution_route(),
                        match causal {
                            None => "linear_affine",
                            Some(CausalStrategy::ColdPrefix) => "causal_cold_prefix",
                            Some(CausalStrategy::FixedTemplate) => "causal_fixed_template",
                        }
                    );
                    assert!(trace.lock().unwrap().is_empty());
                    let run = TaskRunOptions {
                        shots: 19,
                        seed: 39281,
                        max_retained_record_values: 38,
                        ..Default::default()
                    };
                    let batch = e.run_batch(&run).unwrap();
                    assert_eq!(batch.status, RunStatus::Complete);
                    let expected_trace = std::mem::take(&mut *trace.lock().unwrap());
                    let mut manual = SplitMix64::new(run.seed);
                    let expected_records = (0..run.shots)
                        .map(|_| e.sample(&mut manual).unwrap().sampled_records)
                        .collect::<Vec<_>>();
                    if causal.is_some() {
                        assert_eq!(batch.sampled_records.as_ref().unwrap(), &expected_records);
                    } else {
                        assert!(batch.sampled_records.is_none());
                        assert!(expected_records.iter().all(Vec::is_empty));
                    }
                    assert_eq!(*trace.lock().unwrap(), expected_trace);
                    trace.lock().unwrap().clear();
                    for partitions in [vec![3, 1, 4, 2, 9], vec![1; 19], vec![u64::MAX]] {
                        let mut stream = e
                            .start_stream(TaskStreamOptions {
                                run,
                                records: StreamRecordRetention::Retain,
                            })
                            .unwrap();
                        let initial = stream.snapshot();
                        assert_eq!(initial.status, TaskStreamStatus::Running);
                        assert_eq!(initial.not_attempted, 19);
                        assert_eq!(initial.random_state, run.seed);
                        assert!(initial.statistics.unwrap().is_none());
                        assert_eq!(counts(initial.execution_costs), [(0, 0, 0); 4]);
                        assert!(trace.lock().unwrap().is_empty());
                        for n in partitions {
                            let before = stream.random_state();
                            let a = stream.snapshot();
                            let b = stream.snapshot();
                            assert_eq!(a.random_state, before);
                            assert_eq!(a.random_state, b.random_state);
                            assert_eq!(counts(a.execution_costs), counts(b.execution_costs));
                            assert_eq!(a.timings.sampling_seconds, b.timings.sampling_seconds);
                            stream.advance(n).unwrap();
                        }
                        let result = stream.snapshot();
                        assert_eq!(result.status, TaskStreamStatus::DrawsComplete);
                        assert_eq!(
                            (result.completed, result.failed, result.not_attempted),
                            (19, 0, 0)
                        );
                        assert_eq!(result.statistics.unwrap(), batch.statistics);
                        assert_eq!(
                            result.efficiency.unwrap(),
                            batch.efficiency.clone().unwrap()
                        );
                        assert_eq!(result.sampled_records, batch.sampled_records);
                        assert_eq!(result.random_state, manual.state());
                        assert_eq!(
                            counts(result.execution_costs),
                            counts(batch.execution_costs)
                        );
                        assert_eq!(
                            result.completed_network_compilations,
                            batch.completed_network_compilations
                        );
                        assert_eq!(
                            result.completed_history_groups,
                            batch.completed_history_groups
                        );
                        assert_eq!(*trace.lock().unwrap(), expected_trace);
                        let again = stream.advance(99).unwrap();
                        assert_eq!(again.random_state, manual.state());
                        assert_eq!(
                            counts(again.execution_costs),
                            counts(result.execution_costs)
                        );
                        assert_eq!(*trace.lock().unwrap(), expected_trace);
                        trace.lock().unwrap().clear();
                    }
                    let mut discard = e
                        .start_stream(TaskStreamOptions {
                            run: TaskRunOptions {
                                max_retained_record_values: 0,
                                ..run
                            },
                            records: StreamRecordRetention::Discard,
                        })
                        .unwrap();
                    let result = discard.advance(19).unwrap();
                    assert!(result.sampled_records.is_none());
                    assert_eq!(result.statistics.unwrap(), batch.statistics);
                    assert_eq!(result.random_state, manual.state());
                    assert_eq!(*trace.lock().unwrap(), expected_trace);
                }
            }
        }
    }
}
#[test]
fn all_stream_validation_and_snapshots_precede_entropy_queries_and_controller_calls() {
    let files = Files::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&calls);
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", move |b: &[bool]| {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(b.to_vec())
        })
        .unwrap();
    let e = evaluator(
        &files,
        "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nCX ctrl[0] 1",
        r#"{"sample_records":[0]}"#,
        &registry,
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    for run in [
        TaskRunOptions {
            shots: 0,
            ..Default::default()
        },
        TaskRunOptions {
            shots: 4,
            max_retained_record_values: 3,
            ..Default::default()
        },
        TaskRunOptions {
            statistics: StatisticsOptions {
                max_dimensions: 1,
                ..Default::default()
            },
            ..Default::default()
        },
    ] {
        assert!(
            e.start_stream(TaskStreamOptions {
                run,
                records: StreamRecordRetention::Retain
            })
            .is_err()
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let mut s = e
        .start_stream(TaskStreamOptions {
            run: TaskRunOptions {
                shots: 4,
                seed: 57,
                max_retained_record_values: 0,
                ..Default::default()
            },
            records: StreamRecordRetention::Discard,
        })
        .unwrap();
    assert!(s.advance(0).is_err());
    let snapshot = s.snapshot();
    assert_eq!(snapshot.random_state, 57);
    assert_eq!(snapshot.completed, 0);
    assert_eq!(counts(snapshot.execution_costs), [(0, 0, 0); 4]);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
#[test]
fn cumulative_query_limits_and_expired_deadlines_retain_prefixes_and_terminal_rng_states() {
    let files = Files::new();
    let e = evaluator(
        &files,
        "H 0\nT 0\nH 0\nX_ERROR(1/7) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]",
        r#"{"error_observables":[0],"sample_records":[0]}"#,
        &ControllerRegistry::default(),
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    let q = e
        .run_batch(&TaskRunOptions::default())
        .unwrap()
        .execution_costs
        .query
        .attempted;
    assert!(q > 0);
    let mut s = e
        .start_stream(TaskStreamOptions {
            run: TaskRunOptions {
                shots: 5,
                seed: 31,
                execution_limits: ExecutionLimits {
                    max_queries: 2 * q - 1,
                    ..Default::default()
                },
                ..Default::default()
            },
            records: StreamRecordRetention::Retain,
        })
        .unwrap();
    assert_eq!(s.advance(1).unwrap().completed, 1);
    let result = s.advance(4).unwrap();
    assert_eq!(result.status, TaskStreamStatus::Failed);
    assert_eq!(
        (result.completed, result.failed, result.not_attempted),
        (1, 1, 3)
    );
    assert_eq!(result.execution_costs.query.attempted, 2 * q - 1);
    assert_eq!(result.failure.as_ref().unwrap().shot, Some(1));
    assert_eq!(result.failure.as_ref().unwrap().phase, RunPhase::Sampling);
    assert_eq!(result.sampled_records.as_ref().unwrap().len(), 1);
    let state = s.random_state();
    assert!(s.advance(1).is_err());
    assert_eq!(s.random_state(), state);
    assert_eq!(
        counts(s.snapshot().execution_costs),
        counts(result.execution_costs)
    );
    let mut expired = e
        .start_stream(TaskStreamOptions {
            run: TaskRunOptions {
                shots: 3,
                seed: 71,
                execution_limits: ExecutionLimits {
                    max_duration: Some(Duration::ZERO),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
    let result = expired.advance(3).unwrap();
    assert_eq!(
        (result.completed, result.failed, result.not_attempted),
        (0, 1, 2)
    );
    assert_eq!(result.random_state, 71);
    assert_eq!(counts(result.execution_costs), [(0, 0, 0); 4]);
    assert_eq!(result.status, TaskStreamStatus::Failed);
}
#[test]
fn diagnostic_errors_do_not_restart_streams_and_accumulation_errors_are_terminal() {
    let files = Files::new();
    let e = evaluator(
        &files,
        "I 0",
        "{}",
        &ControllerRegistry::default(),
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    let mut s = e
        .start_stream(TaskStreamOptions {
            run: TaskRunOptions {
                shots: 6,
                seed: 42,
                statistics: StatisticsOptions {
                    arithmetic: FieldArithmetic::new(2),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
    assert_eq!(s.advance(1).unwrap().completed, 1);
    let prefix = s.advance(1).unwrap();
    assert_eq!(prefix.status, TaskStreamStatus::Running);
    assert!(prefix.statistics.is_err() || prefix.efficiency.is_err());
    assert_eq!(s.advance(1).unwrap().completed, 3);
    let result = s.advance(3).unwrap();
    assert_eq!(
        (result.completed, result.failed, result.not_attempted),
        (3, 1, 2)
    );
    assert_eq!(result.status, TaskStreamStatus::Failed);
    assert_eq!(result.failure.unwrap().phase, RunPhase::Accumulation);
    assert_eq!(result.random_state, 42);
    assert!(s.advance(1).is_err());
    assert_eq!(s.snapshot().completed, 3);
}
#[test]
fn idle_pauses_spend_the_original_deadline_without_resetting_it_at_advance() {
    let files = Files::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "idle-deadline", move |bits: &[bool]| {
            observed.fetch_add(1, Ordering::SeqCst);
            Ok(bits.to_vec())
        })
        .unwrap();
    let e = evaluator(
        &files,
        "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nCX ctrl[0] 1",
        r#"{"sample_records":[0]}"#,
        &registry,
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    let mut stream = e
        .start_stream(TaskStreamOptions {
            run: TaskRunOptions {
                shots: 3,
                seed: 97,
                execution_limits: ExecutionLimits {
                    max_duration: Some(Duration::from_millis(1)),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
    std::thread::sleep(Duration::from_millis(2));
    let idle = stream.snapshot();
    assert_eq!(idle.status, TaskStreamStatus::Running);
    assert_eq!(idle.random_state, 97);
    assert_eq!(idle.timings.sampling_seconds, 0.0);
    assert_eq!(counts(idle.execution_costs), [(0, 0, 0); 4]);
    let expired = stream.advance(1).unwrap();
    assert_eq!(expired.status, TaskStreamStatus::Failed);
    assert_eq!(
        (expired.completed, expired.failed, expired.not_attempted),
        (0, 1, 2)
    );
    assert_eq!(expired.failure.as_ref().unwrap().phase, RunPhase::Sampling);
    assert_eq!(expired.random_state, 97);
    assert_eq!(counts(expired.execution_costs), [(0, 0, 0); 4]);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(stream.advance(1).is_err());
    assert_eq!(stream.random_state(), 97);
}
#[test]
fn controller_failures_and_caught_unwinds_poison_without_replaying_or_losing_the_prefix() {
    let files = Files::new();
    for panic in [false, true] {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&calls);
        let mut registry = ControllerRegistry::default();
        registry
            .register("fail", "1", move |b: &[bool]| {
                let call = c.fetch_add(1, Ordering::SeqCst);
                if call == 1 {
                    if panic {
                        panic!("deliberate stream-controller unwind");
                    }
                    return Err("deliberate stream-controller error".to_owned());
                }
                Ok(b.to_vec())
            })
            .unwrap();
        let e = evaluator(
            &files,
            "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[fail] det[0] -> ctrl[0]\nCX ctrl[0] 1",
            r#"{"sample_records":[0]}"#,
            &registry,
            EvaluationOptions::default(),
            &NoiseSampling::Physical,
        );
        let mut s = e
            .start_stream(TaskStreamOptions {
                run: TaskRunOptions {
                    shots: 4,
                    seed: 53,
                    ..Default::default()
                },
                records: StreamRecordRetention::Retain,
            })
            .unwrap();
        assert_eq!(s.advance(1).unwrap().completed, 1);
        if panic {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.advance(1))).is_err()
            );
        } else {
            assert_eq!(s.advance(1).unwrap().status, TaskStreamStatus::Failed);
        }
        let prefix = s.snapshot();
        assert_eq!(
            (prefix.completed, prefix.failed, prefix.not_attempted),
            (1, 1, 2)
        );
        assert_eq!(
            prefix.failure.as_ref().unwrap().kind,
            if panic { "Unwind" } else { "TaskError" }
        );
        assert_eq!(prefix.failure.as_ref().unwrap().phase, RunPhase::Sampling);
        assert_eq!(prefix.sampled_records.as_ref().unwrap().len(), 1);
        assert_eq!(prefix.execution_costs.decoder.failed, 1);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(s.advance(1).is_err());
        let after = s.snapshot();
        assert_eq!(after.random_state, prefix.random_state);
        assert_eq!(after.statistics, prefix.statistics);
        assert_eq!(
            after.timings.sampling_seconds,
            prefix.timings.sampling_seconds
        );
        assert_eq!(
            counts(after.execution_costs),
            counts(prefix.execution_costs)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
#[test]
fn zero_weight_affine_histories_survive_chunking_with_exact_efficiency_counts() {
    let files = Files::new();
    let mut options = EvaluationOptions::default();
    options.runtime.history_sampler = HistorySampler::AffineSupport;
    let e = evaluator(
        &files,
        "H 0 1 2 3\nCCZ 0 1 2\nCCZ 0 1 3\nCCZ 0 2 3\nH 0 1 2 3\nM 0 1 2 3\nOBSERVABLE_INCLUDE(0) rec[-1]",
        r#"{"error_observables":[0],"sample_records":[0,1,2,3]}"#,
        &ControllerRegistry::default(),
        options,
        &NoiseSampling::Physical,
    );
    let run = TaskRunOptions {
        shots: 64,
        seed: 23,
        ..Default::default()
    };
    let batch = e.run_batch(&run).unwrap();
    assert_eq!(batch.status, RunStatus::Complete);
    assert!(batch.efficiency.as_ref().unwrap().zero_weights > 0);
    let mut stream = e
        .start_stream(TaskStreamOptions {
            run,
            records: StreamRecordRetention::Retain,
        })
        .unwrap();
    for _ in 0..8 {
        stream.advance(8).unwrap();
    }
    let result = stream.snapshot();
    assert_eq!(result.status, TaskStreamStatus::DrawsComplete);
    assert_eq!(result.statistics.unwrap(), batch.statistics);
    assert_eq!(result.efficiency.unwrap(), batch.efficiency.unwrap());
    assert_eq!(result.sampled_records, batch.sampled_records);
}
