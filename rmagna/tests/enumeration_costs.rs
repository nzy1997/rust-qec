use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::{ExecutionLimits, ExecutionMeter, OperationCost};
use rmagna::field::FieldArithmetic;
use rmagna::runtime::{CausalStrategy, ControllerRegistry};
use rmagna::task::{
    EvaluationOptions, TaskEnumerationOptions, TaskEvaluator, TaskOptions, parse_task,
};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn prepare(
    source: &str,
    task: &str,
    registry: &ControllerRegistry,
    strategy: CausalStrategy,
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
    let mut options = EvaluationOptions::default();
    options.runtime.strategy = strategy;
    TaskEvaluator::new(&circuit, &modules, registry, options).unwrap()
}
fn count(cost: OperationCost, expected: (u64, u64, u64)) {
    assert_eq!((cost.attempted, cost.succeeded, cost.failed), expected);
    assert!(cost.seconds.is_finite() && cost.seconds >= 0.0);
}
fn scalar(n: i64, d: i64) -> rmagna::field::ExactField {
    FieldArithmetic::default()
        .from_rational(BigRational::new(n.into(), d.into()))
        .unwrap()
}
fn causal(registry: &ControllerRegistry, strategy: CausalStrategy) -> TaskEvaluator {
    prepare(
        "X_ERROR(1/3) 3\nH 0 1 2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[majority] det[0] det[1] det[2] -> ctrl[0]\nCX ctrl[0] 3\nM 3\nOBSERVABLE_INCLUDE(0) rec[-1]\nPOSTSELECT det[0]",
        r#"{"error_observables":[0]}"#,
        registry,
        strategy,
    )
}
fn registry(calls: &Arc<AtomicUsize>, fail: bool) -> ControllerRegistry {
    let mut registry = ControllerRegistry::default();
    let calls = calls.clone();
    registry
        .register("majority", "1", move |bits: &[bool]| {
            calls.fetch_add(1, Ordering::SeqCst);
            if fail {
                Err("deliberate decoder failure".to_owned())
            } else {
                Ok(vec![bits.iter().filter(|&&b| b).count() >= 2])
            }
        })
        .unwrap();
    registry
}
#[test]
fn linear_enumeration_counts_every_response_without_changing_exact_sectors() {
    let evaluator = prepare(
        "X_ERROR(1/3) 0\nX_ERROR(1/4) 1\nX_ERROR(1/5) 2\nM 0 1 2\nOBSERVABLE_INCLUDE(0) rec[-3] rec[-2] rec[-1]",
        r#"{"observables":[[0,7]]}"#,
        &ControllerRegistry::default(),
        CausalStrategy::FixedTemplate,
    );
    let mut options = TaskEnumerationOptions::default();
    options.noise.max_weight = 3;
    let expected = evaluator.enumerate_physical(&options).unwrap();
    let meter = ExecutionMeter::default();
    let observed = evaluator
        .enumerate_physical_with_costs(&options, &meter)
        .unwrap();
    assert_eq!(observed, expected);
    assert_eq!((observed.patterns, observed.histories), (8, 8));
    assert_eq!(observed.masses.accepted, scalar(1, 1));
    assert_eq!(observed.masses.error, scalar(0, 1));
    assert_eq!(observed.masses.moments, [scalar(1, 10)]);
    let costs = meter.snapshot();
    count(costs.query, (16, 16, 0));
    count(costs.compilation, (0, 0, 0));
    count(costs.decoder, (0, 0, 0));
    count(costs.support, (0, 0, 0));
}
#[test]
fn nonlinear_sectors_count_unresolved_and_interrupted_replays_for_both_strategies() {
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        let calls = Arc::new(AtomicUsize::new(0));
        let evaluator = causal(&registry(&calls, false), strategy);
        for weight in [0, 1] {
            let mut options = TaskEnumerationOptions::default();
            options.noise.max_weight = weight;
            let expected = evaluator.enumerate_physical(&options).unwrap();
            calls.store(0, Ordering::SeqCst);
            let meter = ExecutionMeter::default();
            let actual = evaluator
                .enumerate_physical_with_costs(&options, &meter)
                .unwrap();
            assert_eq!(actual, expected);
            let patterns = weight as u64 + 1;
            assert_eq!(actual.histories, 8 * patterns as usize);
            assert_eq!(actual.replay_attempts, 9 * patterns as usize);
            assert_eq!(actual.marginal_queries, 24 * patterns as usize);
            assert_eq!(
                actual.masses.accepted,
                if weight == 0 {
                    scalar(1, 3)
                } else {
                    scalar(1, 2)
                }
            );
            assert_eq!(
                actual.masses.error,
                if weight == 0 {
                    scalar(1, 12)
                } else {
                    scalar(5, 24)
                }
            );
            let costs = meter.snapshot();
            count(costs.query, (40 * patterns, 40 * patterns, 0));
            count(costs.support, (patterns, patterns, 0));
            count(costs.decoder, (8 * patterns, 8 * patterns, 0));
            assert_eq!(calls.load(Ordering::SeqCst), 8 * patterns as usize);
            let compilations = if strategy == CausalStrategy::ColdPrefix {
                17 * patterns
            } else {
                0
            };
            count(costs.compilation, (compilations, compilations, 0));
            assert_eq!(actual.network_compilations, compilations as usize);
        }
    }
}
#[test]
fn quotas_and_expired_deadlines_stop_before_callbacks_and_leave_actual_costs() {
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        let calls = Arc::new(AtomicUsize::new(0));
        let evaluator = causal(&registry(&calls, false), strategy);
        let options = TaskEnumerationOptions::default();
        for (quota, callbacks) in [(0, 0), (8, 0), (9, 1)] {
            calls.store(0, Ordering::SeqCst);
            let meter = ExecutionMeter::new(ExecutionLimits {
                max_queries: quota,
                max_duration: None,
            });
            let error = evaluator
                .enumerate_physical_with_costs(&options, &meter)
                .unwrap_err();
            assert!(error.to_string().contains("max_queries"), "{error}");
            let costs = meter.snapshot();
            count(costs.query, (quota, quota, 0));
            count(costs.decoder, (callbacks, callbacks, 0));
            assert_eq!(calls.load(Ordering::SeqCst), callbacks as usize);
            // The unresolved traversal prepares support even when the first
            // physical marginal is denied by a zero query quota.
            count(costs.support, (1, 1, 0));
            let compilations = if strategy == CausalStrategy::ColdPrefix {
                match quota {
                    0 => 1,
                    8 => 2,
                    _ => 3,
                }
            } else {
                0
            };
            count(costs.compilation, (compilations, compilations, 0));
        }
        calls.store(0, Ordering::SeqCst);
        let expired = ExecutionMeter::new(ExecutionLimits {
            max_queries: u64::MAX,
            max_duration: Some(Duration::ZERO),
        });
        let error = evaluator
            .enumerate_physical_with_costs(&options, &expired)
            .unwrap_err();
        assert!(error.to_string().contains("max_seconds"));
        assert_eq!(expired.snapshot(), Default::default());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let fresh = ExecutionMeter::default();
        let result = evaluator
            .enumerate_physical_with_costs(&options, &fresh)
            .unwrap();
        assert_eq!(result.histories, 16);
        assert_eq!(result.masses.error, scalar(5, 24));
        count(fresh.snapshot().query, (80, 80, 0));
    }
}
#[test]
fn failing_decoder_is_counted_once_without_retry_or_partial_exact_estimates() {
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        let calls = Arc::new(AtomicUsize::new(0));
        let evaluator = causal(&registry(&calls, true), strategy);
        let meter = ExecutionMeter::default();
        let error = evaluator
            .enumerate_physical_with_costs(&TaskEnumerationOptions::default(), &meter)
            .unwrap_err();
        assert!(error.to_string().contains("deliberate decoder failure"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let costs = meter.snapshot();
        count(costs.query, (9, 9, 0));
        count(costs.decoder, (1, 0, 1));
        count(costs.support, (1, 1, 0));
        let compilations = if strategy == CausalStrategy::ColdPrefix {
            2
        } else {
            0
        };
        count(costs.compilation, (compilations, compilations, 0));
    }
}

#[test]
fn failed_physical_contraction_retains_its_attempt_in_the_meter() {
    let parse = ParseOptions::default();
    let circuit = parse_circuit("H 0 1 2\nT 0 1 2\nH 0 1 2", &parse).unwrap();
    let modules = parse_task("{}", Path::new("/tmp"), &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &parse)
        .unwrap();
    let mut options = EvaluationOptions::default();
    options.runtime.sampling.max_terms = 0;
    let evaluator =
        TaskEvaluator::new(&circuit, &modules, &ControllerRegistry::default(), options).unwrap();
    let meter = ExecutionMeter::default();
    let error = evaluator
        .enumerate_physical_with_costs(&TaskEnumerationOptions::default(), &meter)
        .unwrap_err();
    assert_eq!(error.field, "acceptance");
    assert!(error.to_string().contains("limit is 0"), "{error}");
    let costs = meter.snapshot();
    count(costs.query, (1, 0, 1));
    count(costs.compilation, (0, 0, 0));
    count(costs.decoder, (0, 0, 0));
    count(costs.support, (0, 0, 0));
}
