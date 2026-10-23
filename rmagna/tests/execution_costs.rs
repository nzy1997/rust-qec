use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::{ExecutionCosts, ExecutionMeter, OperationCost};
use rmagna::instrument::QueryPreparation;
use rmagna::noise::{NoiseSampling, SplitMix64};
use rmagna::numeric::NumericOptions;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::task::{
    EvaluationOptions, RunStatus, TaskEvaluator, TaskOptions, TaskRunOptions, export_batch_metrics,
    parse_task, run_task_batch,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
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
fn check(cost: OperationCost, counts: (u64, u64, u64)) {
    assert_eq!((cost.attempted, cost.succeeded, cost.failed), counts);
    assert!(cost.seconds.is_finite() && cost.seconds >= 0.0);
}
fn counts(costs: ExecutionCosts) -> [(u64, u64, u64); 4] {
    [costs.compilation, costs.query, costs.decoder, costs.support]
        .map(|c| (c.attempted, c.succeeded, c.failed))
}
#[test]
fn both_causal_strategies_and_history_samplers_count_logical_queries() {
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", |b: &[bool]| Ok(b.to_vec()))
        .unwrap();
    let source = "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nCX ctrl[0] 1";
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
            for records in [false, true] {
                let mut options = EvaluationOptions::default();
                options.runtime.strategy = strategy;
                options.runtime.history_sampler = sampler;
                let e = prepared(
                    source,
                    if records {
                        r#"{"sample_records":[0,0]}"#
                    } else {
                        "{}"
                    },
                    &registry,
                    options,
                );
                let result = e
                    .run_batch(&TaskRunOptions {
                        shots: 3,
                        seed: 19,
                        ..Default::default()
                    })
                    .unwrap();
                assert_eq!(result.status, RunStatus::Complete);
                let groups = if records { 2 } else { 1 };
                // Exact: two marginals for the controller, optionally three
                // for duplicate records, then final history and acceptance.
                let queries = if sampler == HistorySampler::ExactMarginal {
                    if records { 7 } else { 4 }
                } else {
                    2
                };
                check(result.execution_costs.query, (3 * queries, 3 * queries, 0));
                check(result.execution_costs.decoder, (3, 3, 0));
                let supports = if sampler == HistorySampler::AffineSupport {
                    3 * groups
                } else {
                    0
                };
                check(result.execution_costs.support, (supports, supports, 0));
                let compiles = if strategy == CausalStrategy::ColdPrefix {
                    3 * (groups + 1)
                } else {
                    0
                };
                check(result.execution_costs.compilation, (compiles, compiles, 0));
                assert_eq!(result.completed_history_groups, 3 * groups as usize);
                assert_eq!(result.completed_network_compilations, compiles as usize);
                for seed in 0..8 {
                    let mut ordinary = SplitMix64::new(seed);
                    let mut observed = ordinary;
                    let meter = ExecutionMeter::default();
                    assert!(
                        e.sample(&mut ordinary).unwrap()
                            == e.sample_with_costs(&mut observed, &meter).unwrap()
                    );
                    assert_eq!(ordinary.state(), observed.state());
                    assert_eq!(meter.snapshot().query.attempted, queries);
                }
            }
        }
    }
}
#[test]
fn owned_target_preparation_and_cached_concurrent_runs_have_isolated_costs() {
    let files = Files::new();
    files.put("target.magna", "I 0");
    let circuit = parse_circuit("I 0", &ParseOptions::default()).unwrap();
    let modules = parse_task(
        r#"{"ideal_state":"target.magna","observables":[[0,1],[1,0]]}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap()
    .load_modules(&circuit, &ParseOptions::default())
    .unwrap();
    let registry = ControllerRegistry::default();
    let mut evaluation = EvaluationOptions::default();
    evaluation.runtime.compile.probability_preparation = QueryPreparation::Trace;
    evaluation.runtime.compile.density_preparation = QueryPreparation::Trace;
    let run = TaskRunOptions {
        shots: 3,
        ..Default::default()
    };
    let owned = run_task_batch(
        &circuit,
        &modules,
        &registry,
        evaluation,
        &NoiseSampling::Physical,
        &run,
    )
    .unwrap();
    check(owned.execution_costs.compilation, (2, 2, 0));
    check(owned.execution_costs.query, (13, 13, 0)); // Norm + 3*(acceptance,target,2 moments).
    let preparation = ExecutionMeter::default();
    let evaluator = TaskEvaluator::new_with_costs(
        &circuit,
        &modules,
        &registry,
        evaluation,
        &NoiseSampling::Physical,
        &preparation,
    )
    .unwrap();
    check(preparation.snapshot().compilation, (2, 2, 0));
    check(preparation.snapshot().query, (1, 1, 0));
    for _ in 0..3 {
        let reused = evaluator.run_batch(&run).unwrap();
        assert_eq!(reused.statistics, owned.statistics);
        check(reused.execution_costs.compilation, (0, 0, 0));
        check(reused.execution_costs.query, (12, 12, 0));
    }
    std::thread::scope(|scope| {
        let handles = (0..4)
            .map(|_| scope.spawn(|| evaluator.run_batch(&run).unwrap()))
            .collect::<Vec<_>>();
        for handle in handles {
            let row = handle.join().unwrap();
            assert_eq!(row.statistics, owned.statistics);
            check(row.execution_costs.query, (12, 12, 0));
        }
    });
    check(preparation.snapshot().query, (1, 1, 0));
    let json = export_batch_metrics(&owned, NumericOptions::default()).unwrap();
    assert_eq!(
        json["execution_costs"]["scope"],
        "this_call_including_failed_attempts"
    );
    assert_eq!(json["execution_costs"]["query"]["attempted"], 13);
    assert_eq!(json["execution_costs"]["compilation"]["succeeded"], 2);
    let mut malformed = owned.clone();
    malformed.execution_costs.query.failed += 1;
    assert!(export_batch_metrics(&malformed, NumericOptions::default()).is_err());
    for bad in [f64::NAN, f64::INFINITY, -1.0] {
        let mut malformed = owned.clone();
        malformed.execution_costs.decoder.seconds = bad;
        assert!(export_batch_metrics(&malformed, NumericOptions::default()).is_err());
    }
}
#[test]
fn failing_empty_input_callbacks_keep_work_from_uncommitted_attempts() {
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
            for allowed in [0, 2] {
                let calls = Arc::new(AtomicUsize::new(0));
                let worker = calls.clone();
                let mut registry = ControllerRegistry::default();
                registry
                    .register("decide", "1", move |b: &[bool]| {
                        assert!(b.is_empty());
                        if worker.fetch_add(1, Ordering::SeqCst) < allowed {
                            Ok(vec![true])
                        } else {
                            Err("injected callback failure".into())
                        }
                    })
                    .unwrap();
                let mut options = EvaluationOptions::default();
                options.runtime.strategy = strategy;
                options.runtime.history_sampler = sampler;
                let e = prepared(
                    "CONTROL[decide] -> ctrl[0]\nCX ctrl[0] 0",
                    "{}",
                    &registry,
                    options,
                );
                let result = e
                    .run_batch(&TaskRunOptions {
                        shots: 4,
                        ..Default::default()
                    })
                    .unwrap();
                assert_eq!(result.status, RunStatus::Failed);
                assert_eq!(result.completed, allowed as u64);
                assert_eq!(result.failed, 1);
                let n = allowed as u64;
                check(result.execution_costs.decoder, (n + 1, n, 1));
                let queries = 2 * n
                    + if sampler == HistorySampler::ExactMarginal {
                        n + 1
                    } else {
                        0
                    };
                check(result.execution_costs.query, (queries, queries, 0));
                let support = if sampler == HistorySampler::AffineSupport {
                    n + 1
                } else {
                    0
                };
                check(result.execution_costs.support, (support, support, 0));
                let compiles = if strategy == CausalStrategy::ColdPrefix {
                    2 * n + 1
                } else {
                    0
                };
                check(result.execution_costs.compilation, (compiles, compiles, 0));
                assert_eq!(result.completed_history_groups, n as usize);
                assert_eq!(calls.load(Ordering::SeqCst), allowed + 1);
            }
        }
    }
}
#[test]
fn compiler_and_contraction_failures_are_attempts_and_noise_errors_are_not_queries() {
    let registry = ControllerRegistry::default();
    let mut options = EvaluationOptions::default();
    options.runtime.compile.max_gates = 0;
    let circuit = parse_circuit("H 0", &ParseOptions::default()).unwrap();
    let modules = parse_task("{}", Path::new("/tmp"), &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &ParseOptions::default())
        .unwrap();
    let run = TaskRunOptions {
        shots: 4,
        ..Default::default()
    };
    let result = run_task_batch(
        &circuit,
        &modules,
        &registry,
        options,
        &NoiseSampling::Physical,
        &run,
    )
    .unwrap();
    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(result.completed, 0);
    assert_eq!(result.failed, 0);
    assert_eq!(
        counts(result.execution_costs),
        [(1, 0, 1), (0, 0, 0), (0, 0, 0), (0, 0, 0)]
    );
    options = EvaluationOptions::default();
    options.runtime.sampling.max_terms = 0;
    let e = prepared("I 0", "{}", &registry, options);
    let result = e.run_batch(&run).unwrap();
    assert_eq!(result.failed, 1);
    check(result.execution_costs.query, (1, 0, 1));
    options = EvaluationOptions::default();
    options.runtime.noise.max_entropy_words = 0;
    let e = prepared("X_ERROR(1/3) 0", "{}", &registry, options);
    let result = e.run_batch(&run).unwrap();
    assert_eq!(result.failed, 1);
    assert_eq!(counts(result.execution_costs), [(0, 0, 0); 4]);
    let e = prepared(
        "M 0\nOBSERVABLE_INCLUDE(0) rec[-1]",
        r#"{"error_observables":[0]}"#,
        &registry,
        EvaluationOptions::default(),
    );
    check(e.run_batch(&run).unwrap().execution_costs.query, (8, 8, 0));
}
#[test]
fn guarded_proposals_wrong_output_shapes_and_zero_target_validation_keep_cost_scope() {
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", |b: &[bool]| Ok(b.to_vec()))
        .unwrap();
    let source = "X_ERROR(1/5) 2\nH 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nX_ERROR(1/3) ctrl[0] 1";
    for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
        let mut evaluation = EvaluationOptions::default();
        evaluation.runtime.history_sampler = sampler;
        let parse = ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        };
        let circuit = parse_circuit(source, &parse).unwrap();
        let modules = parse_task("{}", Path::new("/tmp"), &TaskOptions::default())
            .unwrap()
            .load_modules(&circuit, &parse)
            .unwrap();
        let e = TaskEvaluator::new_with_sampling(
            &circuit,
            &modules,
            &registry,
            evaluation,
            &NoiseSampling::Importance {
                proposal_scale: "2".parse().unwrap(),
            },
        )
        .unwrap();
        for seed in 0..8 {
            let mut ordinary = SplitMix64::new(seed);
            let mut observed = ordinary;
            let meter = ExecutionMeter::default();
            assert!(
                e.sample(&mut ordinary).unwrap()
                    == e.sample_with_costs(&mut observed, &meter).unwrap()
            );
            assert_eq!(ordinary.state(), observed.state());
            let queries = if sampler == HistorySampler::ExactMarginal {
                6
            } else {
                2
            };
            check(meter.snapshot().query, (queries, queries, 0));
            let supports = if sampler == HistorySampler::AffineSupport {
                2
            } else {
                0
            };
            check(meter.snapshot().support, (supports, supports, 0));
        }
    }
    let mut bad = ControllerRegistry::default();
    bad.register("bad", "1", |_: &[bool]| Ok(vec![])).unwrap();
    let e = prepared(
        "CONTROL[bad] -> ctrl[0]\nI 0",
        "{}",
        &bad,
        EvaluationOptions::default(),
    );
    let result = e.run_batch(&TaskRunOptions::default()).unwrap();
    check(result.execution_costs.decoder, (1, 0, 1));
    assert_eq!(result.failed, 1);
    let files = Files::new();
    files.put("zero.magna", "I 1");
    let physical = parse_circuit("I 0", &ParseOptions::default()).unwrap();
    let modules = parse_task(
        r#"{"ideal_state":{"circuit":"zero.magna","outputs":[0],"postselection":{"1":1}}}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap()
    .load_modules(&physical, &ParseOptions::default())
    .unwrap();
    let meter = ExecutionMeter::default();
    assert!(
        TaskEvaluator::new_with_costs(
            &physical,
            &modules,
            &ControllerRegistry::default(),
            EvaluationOptions::default(),
            &NoiseSampling::Physical,
            &meter
        )
        .is_err()
    );
    assert_eq!(
        counts(meter.snapshot()),
        [(1, 1, 0), (1, 1, 0), (0, 0, 0), (0, 0, 0)]
    );
}
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-costs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
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
