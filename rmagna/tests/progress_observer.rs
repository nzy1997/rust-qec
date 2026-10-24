use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::ExecutionLimits;
use rmagna::noise::NoiseSampling;
use rmagna::runtime::{ControllerRegistry, HistorySampler};
use rmagna::statistics::EfficiencyStatus;
use rmagna::task::{
    BatchProgress, EvaluationOptions, ProgressObserver, RunPhase, RunStatus, TaskError,
    TaskEvaluator, TaskOptions, TaskRunOptions, TaskRunResult, export_batch_metrics, parse_task,
};
use std::{
    num::NonZeroU64,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
fn evaluator(
    source: &str,
    task: &str,
    registry: &ControllerRegistry,
    sampler: HistorySampler,
    sampling: NoiseSampling,
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
    options.runtime.history_sampler = sampler;
    TaskEvaluator::new_with_sampling(&circuit, &modules, registry, options, &sampling).unwrap()
}
fn same(a: &TaskRunResult, b: &TaskRunResult) {
    assert_eq!(
        (
            a.status,
            a.requested,
            a.completed,
            a.failed,
            a.not_attempted,
            a.seed
        ),
        (
            b.status,
            b.requested,
            b.completed,
            b.failed,
            b.not_attempted,
            b.seed
        )
    );
    assert_eq!(a.statistics, b.statistics);
    assert_eq!(a.partial_statistics, b.partial_statistics);
    assert_eq!(a.efficiency, b.efficiency);
    assert_eq!(a.sampled_records, b.sampled_records);
    assert_eq!(a.failures, b.failures);
    assert_eq!(
        (
            a.execution_costs.compilation.attempted,
            a.execution_costs.query.attempted,
            a.execution_costs.decoder.attempted,
            a.execution_costs.support.attempted
        ),
        (
            b.execution_costs.compilation.attempted,
            b.execution_costs.query.attempted,
            b.execution_costs.decoder.attempted,
            b.execution_costs.support.attempted
        )
    );
}
#[test]
fn periodic_observations_preserve_noisy_nonlinear_stream_statistics_and_work() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("majority", "1", move |b: &[bool]| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(vec![b.iter().filter(|v| **v).count() >= 2])
        })
        .unwrap();
    let source = "H 0\nT 0\nH 0\nH 1 2\nM(1/7) 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[majority] det[0] det[1] det[2] -> ctrl[0]\nCX ctrl[0] 3\nX_ERROR(1/5) ctrl[0] 3\nM 3\nOBSERVABLE_INCLUDE(0) rec[-1]";
    for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
        let e = evaluator(
            source,
            r#"{"error_observables":[0],"sample_records":[0,3]}"#,
            &registry,
            sampler,
            NoiseSampling::Importance {
                proposal_scale: BigRational::new(3.into(), 2.into()),
            },
        );
        let run = TaskRunOptions {
            shots: 17,
            seed: 731,
            ..Default::default()
        };
        let baseline = e.run_batch(&run).unwrap();
        for interval in [1, 4, 17, 18, u64::MAX] {
            let before = count.load(Ordering::Relaxed);
            let mut events = Vec::new();
            let mut callback = |event: &BatchProgress| {
                assert_eq!(event.history_sampler, sampler);
                assert_eq!(event.requested, 17);
                assert!(event.sampling_seconds.is_finite() && event.sampling_seconds >= 0.0);
                assert!(event.efficiency.is_ok());
                events.push(event.clone());
                Ok(())
            };
            let mut observer =
                ProgressObserver::new(NonZeroU64::new(interval).unwrap(), &mut callback);
            let observed = e.run_batch_with_progress(&run, &mut observer).unwrap();
            let receipt = observer.receipt();
            same(&observed, &baseline);
            assert_eq!(count.load(Ordering::Relaxed) - before, 17);
            let expected = (1..=17)
                .filter(|n| n % interval == 0 || *n == 17)
                .collect::<Vec<_>>();
            assert_eq!(
                events.iter().map(|e| e.completed).collect::<Vec<_>>(),
                expected
            );
            assert_eq!(receipt.attempted, expected.len() as u64);
            assert_eq!(receipt.succeeded, receipt.attempted);
            assert_eq!(receipt.failed, 0);
            assert_eq!(receipt.diagnostic_failures, 0);
            assert_eq!(receipt.last_completed, 17);
            let final_event = events.last().unwrap();
            assert_eq!(
                final_event.efficiency.as_ref().unwrap(),
                observed.efficiency.as_ref().unwrap()
            );
            assert_eq!(final_event.execution_costs, observed.execution_costs);
            let logical_costs = |costs: &rmagna::costs::ExecutionCosts| {
                (
                    (
                        costs.compilation.attempted,
                        costs.compilation.succeeded,
                        costs.compilation.failed,
                    ),
                    (
                        costs.query.attempted,
                        costs.query.succeeded,
                        costs.query.failed,
                    ),
                    (
                        costs.decoder.attempted,
                        costs.decoder.succeeded,
                        costs.decoder.failed,
                    ),
                    (
                        costs.support.attempted,
                        costs.support.succeeded,
                        costs.support.failed,
                    ),
                )
            };
            for event in &events {
                let prefix = e
                    .run_batch(&TaskRunOptions {
                        shots: event.completed,
                        ..run
                    })
                    .unwrap();
                assert_eq!(prefix.status, RunStatus::Complete);
                assert_eq!(
                    event.efficiency.as_ref().unwrap(),
                    prefix.efficiency.as_ref().unwrap()
                );
                assert_eq!(
                    logical_costs(&event.execution_costs),
                    logical_costs(&prefix.execution_costs)
                );
            }
            for pair in events.windows(2) {
                assert!(pair[1].sampling_seconds >= pair[0].sampling_seconds);
                assert!(
                    pair[1].execution_costs.query.attempted
                        >= pair[0].execution_costs.query.attempted
                );
            }
            assert!(
                observed.timings.total_seconds
                    >= receipt.seconds + observed.timings.sampling_seconds
            );
        }
    }
}
#[test]
fn observer_errors_preserve_the_completed_prefix_without_inventing_a_failed_draw() {
    let e = evaluator(
        "H 0\nM 0\nI 1 2\nOBSERVABLE_INCLUDE(0) rec[-1]",
        r#"{"error_observables":[0],"sample_records":[0]}"#,
        &ControllerRegistry::default(),
        HistorySampler::AffineSupport,
        NoiseSampling::Physical,
    );
    for (shots, interval, stop) in [(9, 3, 3), (9, 3, 6), (6, 3, 6)] {
        let run = TaskRunOptions {
            shots,
            seed: 83,
            ..Default::default()
        };
        let mut callback = |event: &BatchProgress| {
            if event.completed == stop {
                Err(TaskError {
                    field: "observer".into(),
                    message: "injected notification failure".into(),
                })
            } else {
                Ok(())
            }
        };
        let mut observer = ProgressObserver::new(NonZeroU64::new(interval).unwrap(), &mut callback);
        let result = e.run_batch_with_progress(&run, &mut observer).unwrap();
        let receipt = observer.receipt();
        assert_eq!(result.status, RunStatus::Failed);
        assert_eq!(result.completed, stop);
        assert_eq!(result.failed, 0);
        assert_eq!(result.not_attempted, shots - stop);
        assert!(result.statistics.is_none());
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].phase, RunPhase::Progress);
        assert_eq!(result.failures[0].shot, None);
        assert!(
            result.failures[0]
                .message
                .contains("injected notification failure")
        );
        let prefix = e.run_batch(&TaskRunOptions { shots: stop, ..run }).unwrap();
        assert_eq!(result.partial_statistics, prefix.statistics);
        assert_eq!(result.efficiency, prefix.efficiency);
        assert_eq!(result.sampled_records, prefix.sampled_records);
        assert_eq!(
            result.execution_costs.query.attempted,
            prefix.execution_costs.query.attempted
        );
        assert_eq!(receipt.attempted, stop / interval);
        assert_eq!(receipt.failed, 1);
        assert_eq!(receipt.succeeded, receipt.attempted - 1);
        assert_eq!(receipt.last_completed, stop);
        let json = export_batch_metrics(&result, Default::default()).unwrap();
        assert_eq!(json["failures"][0]["phase"], "progress");
        assert_eq!(json["failed"], 0);
        assert_eq!(json["completed"], stop);
        assert!(json["estimates"].is_null());
        assert!(json["partial_statistics"].is_object());
        let mut malformed = result.clone();
        malformed.failures[0].shot = Some(stop);
        assert!(export_batch_metrics(&malformed, Default::default()).is_err());
    }
}
#[test]
fn zero_and_no_error_diagnostics_are_explicit_and_observer_reuse_resets_the_receipt() {
    for (source, task, status) in [
        (
            "H 0\nM 0\nI 1 2",
            r#"{"sample_records":[0]}"#,
            EfficiencyStatus::NoErrorMetric,
        ),
        (
            "I 0 1 2\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]",
            r#"{"error_observables":[0]}"#,
            EfficiencyStatus::ZeroResidualVariance,
        ),
        (
            "X 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]\nOBSERVABLE_INCLUDE(0) rec[-1]",
            r#"{"error_observables":[0]}"#,
            EfficiencyStatus::ZeroAcceptance,
        ),
    ] {
        let e = evaluator(
            source,
            task,
            &ControllerRegistry::default(),
            HistorySampler::AffineSupport,
            NoiseSampling::Physical,
        );
        let mut seen = 0;
        let mut callback = |event: &BatchProgress| {
            seen += 1;
            let d = event.efficiency.as_ref().unwrap();
            assert_eq!(d.status, status);
            assert!(d.kappa.is_none());
            Ok(())
        };
        let mut observer = ProgressObserver::new(NonZeroU64::new(2).unwrap(), &mut callback);
        for shots in [5, 3] {
            let result = e
                .run_batch_with_progress(
                    &TaskRunOptions {
                        shots,
                        ..Default::default()
                    },
                    &mut observer,
                )
                .unwrap();
            assert_eq!(result.status, RunStatus::Complete);
            let r = observer.receipt();
            assert_eq!(r.attempted, shots.div_ceil(2));
            assert_eq!(r.last_completed, shots);
            assert_eq!(r.failed, 0);
        }
        let invalid = e.run_batch_with_progress(
            &TaskRunOptions {
                shots: 0,
                ..Default::default()
            },
            &mut observer,
        );
        assert!(invalid.is_err());
        assert_eq!(observer.receipt().attempted, 0);
        assert_eq!(observer.receipt().last_completed, 0);

        assert_eq!(seen, 5);
    }
}
#[test]
fn failed_first_draw_has_no_notifications_and_progress_time_is_not_sampling_time() {
    let e = evaluator(
        "H 0\nM 0\nI 1 2",
        r#"{"sample_records":[0]}"#,
        &ControllerRegistry::default(),
        HistorySampler::ExactMarginal,
        NoiseSampling::Physical,
    );
    let mut count = 0;
    let mut callback = |_: &BatchProgress| {
        count += 1;
        Ok(())
    };
    let mut observer = ProgressObserver::new(NonZeroU64::new(1).unwrap(), &mut callback);
    let run = TaskRunOptions {
        shots: 3,
        execution_limits: ExecutionLimits {
            max_queries: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let failed = e.run_batch_with_progress(&run, &mut observer).unwrap();
    assert_eq!(failed.completed, 0);
    assert_eq!(failed.failed, 1);
    assert_eq!(failed.failures[0].phase, RunPhase::Sampling);
    assert_eq!(observer.receipt().attempted, 0);

    assert_eq!(count, 0);
    let mut callback = |_: &BatchProgress| {
        std::thread::sleep(Duration::from_millis(15));
        Ok(())
    };
    let mut observer = ProgressObserver::new(NonZeroU64::new(2).unwrap(), &mut callback);
    let result = e
        .run_batch_with_progress(
            &TaskRunOptions {
                shots: 3,
                ..Default::default()
            },
            &mut observer,
        )
        .unwrap();
    let r = observer.receipt();
    assert_eq!(result.status, RunStatus::Complete);
    assert_eq!(r.attempted, 2);
    assert!(r.seconds >= 0.030);
    assert!(result.timings.total_seconds - result.timings.sampling_seconds >= r.seconds);
    // A generous initial deadline avoids timing-dependent work before the
    // callback; wait until it has definitely expired at the final notification.
    let mut callback = |_: &BatchProgress| {
        std::thread::sleep(Duration::from_millis(550));
        Ok(())
    };
    let mut observer = ProgressObserver::new(NonZeroU64::new(1).unwrap(), &mut callback);
    let result = e
        .run_batch_with_progress(
            &TaskRunOptions {
                shots: 1,
                execution_limits: ExecutionLimits {
                    max_duration: Some(Duration::from_millis(500)),
                    ..Default::default()
                },
                ..Default::default()
            },
            &mut observer,
        )
        .unwrap();
    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(result.completed, 1);
    assert_eq!(result.failed, 0);
    assert_eq!(result.not_attempted, 0);
    assert_eq!(result.failures[0].phase, RunPhase::Progress);
    assert!(result.failures[0].message.contains("max_seconds"));
    assert_eq!(observer.receipt().succeeded, 1);
    assert_eq!(observer.receipt().failed, 0);
}

#[test]
fn a_failed_atomic_accumulation_does_not_emit_a_notification_for_the_discarded_draw() {
    let e = evaluator(
        "H 0\nCH 0 1\nM 0 1\nI 2\nOBSERVABLE_INCLUDE(0) rec[-1]",
        r#"{"error_observables":[0],"sample_records":[0,1]}"#,
        &ControllerRegistry::default(),
        HistorySampler::AffineSupport,
        NoiseSampling::Physical,
    );
    let seed = (0..32)
        .find(|&seed| {
            let mut random = rmagna::noise::SplitMix64::new(seed);
            let first = e.sample(&mut random).unwrap();
            let second = e.sample(&mut random).unwrap();
            first.sampled_records[0] && second.sampled_records == [false, false]
        })
        .unwrap();
    let run = TaskRunOptions {
        shots: 3,
        seed,
        statistics: rmagna::statistics::StatisticsOptions {
            arithmetic: rmagna::field::FieldArithmetic::new(4),
            ..Default::default()
        },
        ..Default::default()
    };
    let baseline = e.run_batch(&run).unwrap();
    assert_eq!(baseline.failures[0].phase, RunPhase::Accumulation);
    let mut completed = Vec::new();
    let mut callback = |progress: &BatchProgress| {
        completed.push(progress.completed);
        Ok(())
    };
    let mut observer = ProgressObserver::new(NonZeroU64::new(1).unwrap(), &mut callback);
    let result = e.run_batch_with_progress(&run, &mut observer).unwrap();
    same(&result, &baseline);
    assert_eq!(
        (result.completed, result.failed, result.not_attempted),
        (1, 1, 1)
    );
    assert_eq!(observer.receipt().attempted, 1);
    assert_eq!(observer.receipt().last_completed, 1);
    assert_eq!(result.execution_costs.query.attempted, 6);
    assert_eq!(completed, [1]);
}
