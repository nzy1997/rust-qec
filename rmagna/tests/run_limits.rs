use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::{ExecutionCosts, ExecutionLimits, ExecutionMeter};
use rmagna::noise::{NoiseSampling, SplitMix64};
use rmagna::numeric::NumericOptions;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::task::{
    EvaluationOptions, RunPhase, RunStatus, TaskEvaluator, TaskOptions, TaskRunOptions,
    export_batch_metrics, parse_task, run_task_batch,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
fn prepared(
    source: &str,
    task: &str,
    registry: &ControllerRegistry,
    options: EvaluationOptions,
) -> TaskEvaluator {
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit(source, &parse).unwrap();
    let modules = parse_task(task, Path::new("/tmp"), &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &parse)
        .unwrap();
    TaskEvaluator::new_with_sampling(
        &circuit,
        &modules,
        registry,
        options,
        &NoiseSampling::Physical,
    )
    .unwrap()
}
fn options(shots: u64, queries: u64) -> TaskRunOptions {
    TaskRunOptions {
        shots,
        seed: 23,
        execution_limits: ExecutionLimits {
            max_queries: queries,
            ..Default::default()
        },
        ..Default::default()
    }
}
#[test]
fn logical_error_query_limit_keeps_completed_prefix_and_no_fictitious_query_failure() {
    let e = prepared(
        "X_ERROR(1/4) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]",
        r#"{"error_observables":[0]}"#,
        &ControllerRegistry::default(),
        EvaluationOptions::default(),
    );
    let full = e.run_batch(&options(4, 8)).unwrap();
    assert_eq!(full.status, RunStatus::Complete);
    assert_eq!(full.execution_costs.query.attempted, 8);
    for quota in 0..8 {
        let result = e.run_batch(&options(4, quota)).unwrap();
        assert_eq!(result.status, RunStatus::Failed);
        assert_eq!(
            (result.completed, result.failed, result.not_attempted),
            (quota / 2, 1, 3 - quota / 2)
        );
        assert_eq!(result.execution_costs.query.attempted, quota);
        assert_eq!(result.execution_costs.query.succeeded, quota);
        assert_eq!(result.execution_costs.query.failed, 0);
        assert!(
            result.failures[0]
                .message
                .contains(&format!("max_queries = {quota}"))
        );
        assert_eq!(result.failures[0].phase, RunPhase::Sampling);
        assert!(result.statistics.is_none());
        if quota >= 2 {
            let expected = e.run_batch(&options(quota / 2, quota)).unwrap();
            assert_eq!(result.partial_statistics, expected.statistics);
        } else {
            assert!(result.partial_statistics.is_none());
        }
        let json = export_batch_metrics(&result, NumericOptions::default()).unwrap();
        assert_eq!(json["execution_limits"]["max_queries"], quota);
        assert!(json["execution_limits"]["max_duration"].is_null());
    }
    let mut malformed = full;
    malformed.execution_limits.max_queries = 7;
    assert!(export_batch_metrics(&malformed, NumericOptions::default()).is_err());
}
#[test]
fn owned_target_normalization_shares_quota_and_reused_preparation_does_not() {
    let files = Files::new();
    std::fs::write(files.0.join("target.magna"), "I 0").unwrap();
    let parse = ParseOptions::default();
    let circuit = parse_circuit("I 0", &parse).unwrap();
    let modules = parse_task(
        r#"{"ideal_state":"target.magna"}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap()
    .load_modules(&circuit, &parse)
    .unwrap();
    let registry = ControllerRegistry::default();
    let evaluation = EvaluationOptions::default();
    let run = |quota| {
        run_task_batch(
            &circuit,
            &modules,
            &registry,
            evaluation,
            &NoiseSampling::Physical,
            &options(3, quota),
        )
        .unwrap()
    };
    let full = run(7);
    assert_eq!(full.status, RunStatus::Complete);
    assert_eq!(full.execution_costs.query.attempted, 7);
    let short = run(6);
    assert_eq!(
        (short.completed, short.failed, short.not_attempted),
        (2, 1, 0)
    );
    assert_eq!(short.execution_costs.query.succeeded, 6);
    assert_eq!(short.execution_costs.query.failed, 0);
    let blocked = run(0);
    assert_eq!(blocked.failures[0].phase, RunPhase::Preparation);
    assert_eq!(
        (blocked.completed, blocked.failed, blocked.not_attempted),
        (0, 0, 3)
    );
    assert_eq!(blocked.execution_costs.compilation.succeeded, 1);
    assert_eq!(blocked.execution_costs.query.attempted, 0);
    let e = TaskEvaluator::new(&circuit, &modules, &registry, evaluation).unwrap();
    let reused = e.run_batch(&options(3, 6)).unwrap();
    assert_eq!(reused.status, RunStatus::Complete);
    assert_eq!(reused.statistics, full.statistics);
    assert_eq!(reused.execution_costs.compilation.attempted, 0);
}
#[test]
fn causal_marginals_and_profiles_respect_shared_exact_query_boundaries() {
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", |b: &[bool]| Ok(b.to_vec()))
        .unwrap();
    let source = "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nCX ctrl[0] 1";
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
            let mut config = EvaluationOptions::default();
            config.runtime.strategy = strategy;
            config.runtime.history_sampler = sampler;
            let e = prepared(source, "{}", &registry, config);
            let per = if sampler == HistorySampler::ExactMarginal {
                4
            } else {
                2
            };
            let full = e.run_batch(&options(3, 3 * per)).unwrap();
            assert_eq!(full.status, RunStatus::Complete);
            let short = e.run_batch(&options(3, 3 * per - 1)).unwrap();
            assert_eq!(short.status, RunStatus::Failed);
            assert_eq!(
                (short.completed, short.failed, short.not_attempted),
                (2, 1, 0)
            );
            assert_eq!(short.execution_costs.query.succeeded, 3 * per - 1);
            assert_eq!(short.execution_costs.query.failed, 0);
            assert_eq!(short.execution_costs.decoder.succeeded, 3);
            for seed in 0..8 {
                let meter = ExecutionMeter::new(ExecutionLimits {
                    max_queries: 3 * per,
                    ..Default::default()
                });
                let mut first = SplitMix64::new(seed);
                let mut second = first;
                for _ in 0..3 {
                    assert!(
                        e.sample(&mut first).unwrap()
                            == e.sample_with_costs(&mut second, &meter).unwrap()
                    );
                }
                assert_eq!(first.state(), second.state());
                assert_eq!(meter.snapshot().query.attempted, 3 * per);
            }
        }
    }
}
#[test]
fn independent_repeated_and_concurrent_batches_do_not_spend_each_others_quota() {
    let e = prepared(
        "I 0",
        "{}",
        &ControllerRegistry::default(),
        EvaluationOptions::default(),
    );
    let config = options(3, 3);
    let expected = e.run_batch(&config).unwrap();
    assert_eq!(expected.status, RunStatus::Complete);
    for _ in 0..3 {
        let result = e.run_batch(&config).unwrap();
        assert_eq!(result.status, RunStatus::Complete);
        assert_eq!(result.execution_costs.query.attempted, 3);
    }
    std::thread::scope(|scope| {
        let handles = (0..4)
            .map(|_| scope.spawn(|| e.run_batch(&config).unwrap()))
            .collect::<Vec<_>>();
        for h in handles {
            let result = h.join().unwrap();
            assert_eq!(result.status, RunStatus::Complete);
            assert_eq!(result.statistics, expected.statistics);
            assert_eq!(result.execution_costs.query.attempted, 3);
        }
    });
}
#[test]
fn zero_duration_stops_before_preparation_or_draw_entropy_and_callbacks() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("call", "1", move |_: &[bool]| {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(vec![true])
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit("CONTROL[call] -> ctrl[0]\nCX ctrl[0] 0", &parse).unwrap();
    let modules = parse_task("{}", Path::new("/tmp"), &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &parse)
        .unwrap();
    let config = TaskRunOptions {
        execution_limits: ExecutionLimits {
            max_duration: Some(Duration::ZERO),
            ..Default::default()
        },
        ..options(3, u64::MAX)
    };
    let owned = run_task_batch(
        &circuit,
        &modules,
        &registry,
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
        &config,
    )
    .unwrap();
    assert_eq!(owned.failures[0].phase, RunPhase::Preparation);
    assert_eq!(
        (owned.completed, owned.failed, owned.not_attempted),
        (0, 0, 3)
    );
    assert_eq!(owned.execution_costs, ExecutionCosts::default());
    let evaluator =
        TaskEvaluator::new(&circuit, &modules, &registry, EvaluationOptions::default()).unwrap();
    let reused = evaluator.run_batch(&config).unwrap();
    assert_eq!(reused.failures[0].phase, RunPhase::Sampling);
    assert_eq!(
        (reused.completed, reused.failed, reused.not_attempted),
        (0, 1, 2)
    );
    assert_eq!(reused.execution_costs, ExecutionCosts::default());
    assert!(reused.failures[0].message.contains("max_seconds = 0"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let meter = ExecutionMeter::new(config.execution_limits);
    let mut random = SplitMix64::new(7);
    let before = random.state();
    // Batch preflight checks deadlines before any draw; individual profiling
    // checks operation boundaries, so use a no-noise callback task here.
    assert!(evaluator.sample_with_costs(&mut random, &meter).is_err());
    assert_eq!(random.state(), before);
    let json = export_batch_metrics(&reused, NumericOptions::default()).unwrap();
    assert_eq!(
        json["execution_limits"]["max_duration"],
        serde_json::json!({"seconds":0,"nanoseconds":0})
    );
    assert_eq!(
        ExecutionLimits {
            max_queries: 5,
            max_duration: Some(Duration::new(u64::MAX, 999_999_999))
        }
        .to_json()["max_duration"]["seconds"],
        u64::MAX
    );
}
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rmagna-limits-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn target_and_moment_query_boundaries_keep_literal_values_and_every_completed_prefix() {
    use rmagna::field::ExactField;
    use rmagna::instrument::QueryPreparation;
    use rmagna::unitary::CompilerStrategy;
    let files = Files::new();
    std::fs::write(files.0.join("target.magna"), "I 0").unwrap();
    let parse = ParseOptions::default();
    let circuit = parse_circuit("I 0", &parse).unwrap();
    let modules = parse_task(
        r#"{"ideal_state":"target.magna","observables":[[0,1],[1,0]]}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap()
    .load_modules(&circuit, &parse)
    .unwrap();
    let registry = ControllerRegistry::default();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for preparation in [
            QueryPreparation::Trace,
            QueryPreparation::FixedFibers(rmagna::fibers::FiberOptions::default()),
        ] {
            let mut evaluation = EvaluationOptions::default();
            evaluation.runtime.compile.compiler_strategy = compiler;
            evaluation.runtime.compile.probability_preparation = preparation;
            evaluation.runtime.compile.density_preparation = preparation;
            let evaluator = TaskEvaluator::new(&circuit, &modules, &registry, evaluation).unwrap();
            let run = |shots, quota| {
                run_task_batch(
                    &circuit,
                    &modules,
                    &registry,
                    evaluation,
                    &NoiseSampling::Physical,
                    &options(shots, quota),
                )
                .unwrap()
            };
            // One target normalization, then acceptance, target, Z and X per draw.
            let full = run(2, 9);
            assert_eq!(full.status, RunStatus::Complete);
            assert_eq!(full.execution_costs.query.attempted, 9);
            let exact = full.statistics.as_ref().unwrap();
            assert_eq!(exact.accepted, ExactField::one());
            assert_eq!(exact.error, Some(ExactField::zero()));
            assert_eq!(exact.moments, vec![ExactField::one(), ExactField::zero()]);
            for quota in 1..9 {
                let short = run(2, quota);
                let completed = (quota - 1) / 4;
                assert_eq!(short.status, RunStatus::Failed);
                assert_eq!(
                    (short.completed, short.failed, short.not_attempted),
                    (completed, 1, 1 - completed)
                );
                assert_eq!(short.execution_costs.query.attempted, quota);
                assert_eq!(short.execution_costs.query.succeeded, quota);
                assert_eq!(short.execution_costs.query.failed, 0);
                assert_eq!(short.failures[0].phase, RunPhase::Sampling);
                assert!(
                    short.failures[0]
                        .message
                        .contains(&format!("max_queries = {quota}"))
                );
                assert!(short.statistics.is_none());
                if completed == 1 {
                    assert_eq!(short.partial_statistics, run(1, 5).statistics);
                } else {
                    assert!(short.partial_statistics.is_none());
                }
                export_batch_metrics(&short, NumericOptions::default()).unwrap();
            }
            let reused = evaluator.run_batch(&options(2, 8)).unwrap();
            assert_eq!(reused.status, RunStatus::Complete);
            assert_eq!(reused.execution_costs.query.attempted, 8);
            assert_eq!(reused.statistics, full.statistics);
            let short = evaluator.run_batch(&options(2, 7)).unwrap();
            assert_eq!(
                (short.completed, short.failed, short.not_attempted),
                (1, 1, 0)
            );
            assert_eq!(
                short.partial_statistics,
                evaluator.run_batch(&options(1, 4)).unwrap().statistics
            );
        }
    }
}
