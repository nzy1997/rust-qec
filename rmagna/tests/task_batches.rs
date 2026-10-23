use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::noise::{NoiseSampling, SplitMix64};
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::statistics::{EfficiencyStatus, HistoryEfficiency, StatisticsOptions};
use rmagna::task::{
    EvaluationOptions, RunPhase, RunStatus, TaskEvaluation, TaskEvaluator, TaskOptions,
    TaskRunOptions, TaskRunStatistics, parse_task, run_task_batch,
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-batch-{}-{}",
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
fn evaluator(
    files: &Files,
    source: &str,
    task: &str,
    registry: &ControllerRegistry,
    evaluation: EvaluationOptions,
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
    TaskEvaluator::new_with_sampling(&source, &modules, registry, evaluation, sampling).unwrap()
}
fn r(n: u64, d: u64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(BigRational::new(n.into(), d.into()))
        .unwrap()
}
fn manual(e: &TaskEvaluator, count: u64, seed: u64) -> Vec<TaskEvaluation> {
    let mut rng = SplitMix64::new(seed);
    (0..count).map(|_| e.sample(&mut rng).unwrap()).collect()
}
fn check_full_list(rows: &[TaskEvaluation], summary: &TaskRunStatistics, has_error: bool) {
    let f = FieldArithmetic::default();
    let sum = |values: Vec<ExactField>| {
        values
            .iter()
            .try_fold(ExactField::zero(), |s, x| f.add(&s, x))
            .unwrap()
    };
    let n = r(rows.len() as u64, 1);
    let mean = |values: Vec<ExactField>| f.divide(&sum(values), &n).unwrap();
    let vectors = rows
        .iter()
        .map(|row| {
            let mut v = vec![row.accepted.clone(), row.error.clone()];
            v.extend_from_slice(&row.moments);
            v
        })
        .collect::<Vec<_>>();
    let means = (0..vectors[0].len())
        .map(|j| mean(vectors.iter().map(|v| v[j].clone()).collect()))
        .collect::<Vec<_>>();
    assert_eq!(summary.completed, rows.len() as u64);
    assert_eq!(summary.accepted, means[0]);
    assert_eq!(summary.error, has_error.then(|| means[1].clone()));
    assert_eq!(summary.moments, means[2..]);
    if rows.len() > 1 {
        let divisor = f.multiply(&n, &r(rows.len() as u64 - 1, 1)).unwrap();
        let variances = (0..means.len())
            .map(|j| {
                let centered = vectors
                    .iter()
                    .map(|v| {
                        let d = f.subtract(&v[j], &means[j]).unwrap();
                        f.multiply(&d, &d).unwrap()
                    })
                    .collect();
                f.divide(&sum(centered), &divisor).unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(summary.variances.accepted, Some(variances[0].clone()));
        assert_eq!(
            summary.variances.error,
            has_error.then(|| variances[1].clone())
        );
        assert_eq!(
            summary.variances.moments,
            variances[2..].iter().cloned().map(Some).collect::<Vec<_>>()
        );
        if has_error && !means[0].is_zero() {
            let ratio = f.divide(&means[1], &means[0]).unwrap();
            let residual = sum(vectors
                .iter()
                .map(|v| {
                    let d = f
                        .subtract(&v[1], &f.multiply(&ratio, &v[0]).unwrap())
                        .unwrap();
                    f.multiply(&d, &d).unwrap()
                })
                .collect());
            let expected = f
                .divide(
                    &residual,
                    &f.multiply(&divisor, &f.multiply(&means[0], &means[0]).unwrap())
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(summary.conditional_error, Some(ratio));
            assert_eq!(summary.variances.conditional_error, Some(expected));
        }
    } else {
        assert!(summary.variances.accepted.is_none());
        assert!(summary.variances.error.is_none());
        assert!(summary.variances.moments.iter().all(Option::is_none));
    }
    if !has_error || means[0].is_zero() {
        assert!(summary.conditional_error.is_none());
        assert!(summary.variances.conditional_error.is_none());
    }
    let weights = rows
        .iter()
        .map(|row| {
            f.multiply(
                &f.from_rational(row.noise_likelihood_ratio.clone()).unwrap(),
                &row.history_weight,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(summary.mean_weight, mean(weights.clone()));
    let w = sum(weights.clone());
    let w2 = sum(weights.iter().map(|w| f.multiply(w, w).unwrap()).collect());
    assert_eq!(
        summary.effective_sample_size,
        if w2.is_zero() {
            ExactField::zero()
        } else {
            f.divide(&f.multiply(&w, &w).unwrap(), &w2).unwrap()
        }
    );
}

#[test]
fn seeded_linear_and_causal_batches_match_independent_centered_full_list_statistics() {
    let files = Files::new();
    files.put("ideal.magna", "I 0");
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", |b: &[bool]| Ok(b.to_vec()))
        .unwrap();
    let linear = "X_ERROR(1/5) 0\nH 1\nM 1\nDETECTOR rec[-1]\nPOSTSELECT det[0]";
    let causal = "H 0\nT 0\nH 0\nM(1/7) 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nCH ctrl[0] 1\nM 1";
    for source in [linear, causal] {
        let task = if source == linear {
            r#"{"outputs":[0],"ideal_state":"ideal.magna","observables":[[0,1],[1,0]]}"#
        } else {
            r#"{"outputs":[1],"ideal_state":"ideal.magna","sample_records":[0,1],"observables":[[0,1],[1,0]]}"#
        };
        for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            for history_sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                for sampling in [
                    NoiseSampling::Physical,
                    NoiseSampling::Importance {
                        proposal_scale: "2".parse().unwrap(),
                    },
                ] {
                    let mut evaluation = EvaluationOptions::default();
                    evaluation.runtime.strategy = strategy;
                    evaluation.runtime.history_sampler = history_sampler;
                    let e = evaluator(&files, source, task, &registry, evaluation, &sampling);
                    let options = TaskRunOptions {
                        shots: 12,
                        seed: 41,
                        ..Default::default()
                    };
                    let rows = manual(&e, options.shots, options.seed);
                    let result = e.run_batch(&options).unwrap();
                    assert_eq!(result.status, RunStatus::Complete);
                    assert_eq!(
                        (
                            result.requested,
                            result.completed,
                            result.failed,
                            result.not_attempted
                        ),
                        (12, 12, 0, 0)
                    );
                    assert!(result.failures.is_empty() && result.partial_statistics.is_none());
                    check_full_list(&rows, result.statistics.as_ref().unwrap(), true);
                    assert_eq!(
                        result.completed_network_compilations,
                        rows.iter().map(|r| r.network_compilations).sum::<usize>()
                    );
                    assert_eq!(
                        result.completed_history_groups,
                        rows.iter().map(|r| r.history_queries).sum::<usize>()
                    );
                    let mut efficiency =
                        HistoryEfficiency::new(FieldArithmetic::default()).unwrap();
                    for row in &rows {
                        let f = FieldArithmetic::default();
                        let l = f.from_rational(row.noise_likelihood_ratio.clone()).unwrap();
                        let w = f.multiply(&l, &row.history_weight).unwrap();
                        efficiency
                            .push(&row.accepted, Some(&row.error), &w, &l)
                            .unwrap();
                    }
                    assert_eq!(result.efficiency, Some(efficiency.summarize().unwrap()));
                    if source == causal {
                        assert_eq!(
                            result.sampled_records,
                            Some(rows.iter().map(|r| r.sampled_records.clone()).collect())
                        );
                    } else {
                        assert!(result.sampled_records.is_none());
                    }
                    let repeat = e.run_batch(&options).unwrap();
                    assert_eq!(repeat.statistics, result.statistics);
                    assert_eq!(repeat.sampled_records, result.sampled_records);
                    assert_eq!(repeat.efficiency, result.efficiency);
                    assert!(result.timings.preparation_seconds.is_none());
                    assert!(result.timings.total_seconds >= result.timings.sampling_seconds);
                }
            }
        }
    }
}

#[test]
fn callback_failures_keep_only_the_aligned_completed_prefix() {
    let files = Files::new();
    let source = "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nCX ctrl[0] 1\nM 1\nOBSERVABLE_INCLUDE(0) rec[-1]";
    let task = r#"{"error_observables":[0],"sample_records":[0,1]}"#;
    for allowed in [0, 2] {
        let mut registry = ControllerRegistry::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        registry
            .register("echo", "1", move |b: &[bool]| {
                if counter.fetch_add(1, Ordering::Relaxed) >= allowed {
                    Err("simulated decoder disconnect".to_owned())
                } else {
                    Ok(b.to_vec())
                }
            })
            .unwrap();
        let e = evaluator(
            &files,
            source,
            task,
            &registry,
            EvaluationOptions::default(),
            &NoiseSampling::Physical,
        );
        let result = e
            .run_batch(&TaskRunOptions {
                shots: 5,
                seed: 7,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(result.status, RunStatus::Failed);
        assert!(result.statistics.is_none());
        assert_eq!(
            (result.completed, result.failed, result.not_attempted),
            (allowed as u64, 1, 4 - allowed as u64)
        );
        assert_eq!(calls.load(Ordering::Relaxed), allowed + 1);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].phase, RunPhase::Sampling);
        assert_eq!(result.failures[0].shot, Some(allowed as u64));
        assert!(
            result.failures[0]
                .message
                .contains("simulated decoder disconnect")
        );
        assert_eq!(result.sampled_records.as_ref().unwrap().len(), allowed);
        assert_eq!(
            result.efficiency.as_ref().unwrap().completed,
            allowed as u64
        );
        if allowed == 0 {
            assert!(result.partial_statistics.is_none());
        } else {
            let mut good = ControllerRegistry::default();
            good.register("echo", "1", |b: &[bool]| Ok(b.to_vec()))
                .unwrap();
            let expected = evaluator(
                &files,
                source,
                task,
                &good,
                EvaluationOptions::default(),
                &NoiseSampling::Physical,
            );
            check_full_list(
                &manual(&expected, allowed as u64, 7),
                result.partial_statistics.as_ref().unwrap(),
                true,
            );
        }
    }
}

#[test]
fn preparation_contraction_and_finalization_failures_preserve_attempt_accounting() {
    let files = Files::new();
    let registry = ControllerRegistry::default();
    let parse = ParseOptions::default();
    let source = parse_circuit("H 0\nT 0", &parse).unwrap();
    let modules = parse_task("{}", &files.0, &TaskOptions::default())
        .unwrap()
        .load_modules(&source, &parse)
        .unwrap();
    let mut evaluation = EvaluationOptions::default();
    evaluation.runtime.compile.max_gates = 0;
    let options = TaskRunOptions {
        shots: 5,
        ..Default::default()
    };
    let result = run_task_batch(
        &source,
        &modules,
        &registry,
        evaluation,
        &NoiseSampling::Physical,
        &options,
    )
    .unwrap();
    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(
        (result.completed, result.failed, result.not_attempted),
        (0, 0, 5)
    );
    assert_eq!(result.failures[0].phase, RunPhase::Preparation);
    assert_eq!(result.failures[0].shot, None);
    assert!(result.timings.preparation_seconds.is_some());
    let mut evaluation = EvaluationOptions::default();
    evaluation.runtime.sampling.max_terms = 0;
    let e = evaluator(
        &files,
        "I 0",
        "{}",
        &registry,
        evaluation,
        &NoiseSampling::Physical,
    );
    let result = e.run_batch(&options).unwrap();
    assert_eq!(
        (result.completed, result.failed, result.not_attempted),
        (0, 1, 4)
    );
    assert_eq!(result.failures[0].phase, RunPhase::Sampling);
    assert!(result.statistics.is_none());
    let e = evaluator(
        &files,
        "I 0",
        "{}",
        &registry,
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    let result = e
        .run_batch(&TaskRunOptions {
            shots: 3,
            statistics: StatisticsOptions {
                arithmetic: FieldArithmetic::new(2),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(
        (result.completed, result.failed, result.not_attempted),
        (3, 0, 0)
    );
    assert!(result.statistics.is_none());
    assert!(
        result
            .failures
            .iter()
            .all(|f| f.phase == RunPhase::Finalization && f.shot.is_none())
    );
}

#[test]
fn absent_metrics_one_draw_zero_acceptance_and_invalid_retention_are_explicit() {
    let files = Files::new();
    let registry = ControllerRegistry::default();
    let e = evaluator(
        &files,
        "I 0",
        "{}",
        &registry,
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    let result = e.run_batch(&TaskRunOptions::default()).unwrap();
    let s = result.statistics.as_ref().unwrap();
    assert_eq!(s.accepted, ExactField::one());
    assert!(s.error.is_none() && s.conditional_error.is_none());
    assert!(s.variances.accepted.is_none());
    assert_eq!(
        result.efficiency.unwrap().status,
        EfficiencyStatus::NoErrorMetric
    );
    assert_eq!(
        e.run_batch(&TaskRunOptions {
            shots: 0,
            ..Default::default()
        })
        .unwrap_err()
        .field,
        "run.shots"
    );
    let e = evaluator(
        &files,
        "M 0\nDETECTOR rec[-1]\nPOSTSELECT !det[0]\nOBSERVABLE_INCLUDE(0) rec[-1]",
        r#"{"error_observables":[0]}"#,
        &registry,
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    let result = e
        .run_batch(&TaskRunOptions {
            shots: 2,
            ..Default::default()
        })
        .unwrap();
    let s = result.statistics.unwrap();
    assert!(s.accepted.is_zero());
    assert_eq!(s.error, Some(ExactField::zero()));
    assert!(s.conditional_error.is_none());
    assert_eq!(
        result.efficiency.unwrap().status,
        EfficiencyStatus::ZeroAcceptance
    );
    let e = evaluator(
        &files,
        "H 0\nM 0",
        r#"{"sample_records":[0]}"#,
        &registry,
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    assert_eq!(
        e.run_batch(&TaskRunOptions {
            shots: 4,
            max_retained_record_values: 3,
            ..Default::default()
        })
        .unwrap_err()
        .field,
        "run.max_retained_record_values"
    );
    let result = e
        .run_batch(&TaskRunOptions {
            shots: 4,
            max_retained_record_values: 4,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(result.sampled_records.unwrap().len(), 4);
    let limited = TaskRunOptions {
        statistics: StatisticsOptions {
            max_dimensions: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(e.run_batch(&limited).unwrap_err().field, "run.statistics");
}

#[test]
fn an_accumulator_failure_discards_only_the_failed_draw_and_keeps_records_aligned() {
    let files = Files::new();
    let registry = ControllerRegistry::default();
    let mut evaluation = EvaluationOptions::default();
    evaluation.runtime.history_sampler = HistorySampler::AffineSupport;
    let e = evaluator(
        &files,
        "H 0\nCH 0 1\nM 0 1\nOBSERVABLE_INCLUDE(0) rec[-1]",
        r#"{"error_observables":[0],"sample_records":[0,1]}"#,
        &registry,
        evaluation,
        &NoiseSampling::Physical,
    );
    // Literal Born masses: P(00)=1/2 and P(10)=P(11)=1/4.
    // Uniform affine proposals give complete weights two and one respectively.
    let (seed, first) = (0..32)
        .find_map(|seed| {
            let rows = manual(&e, 2, seed);
            (rows[0].sampled_records[0] && rows[1].sampled_records == [false, false]).then(|| {
                assert_eq!(rows[0].accepted, ExactField::one());
                assert_eq!(rows[1].accepted, r(2, 1));
                (seed, rows[0].sampled_records.clone())
            })
        })
        .expect("fixed seed range must contain the chosen positive history pair");
    // The second row fits the staged statistics, but its reference-weight
    // inversion exceeds this budget. The first row must remain committed.
    let result = e
        .run_batch(&TaskRunOptions {
            shots: 3,
            seed,
            statistics: StatisticsOptions {
                arithmetic: FieldArithmetic::new(4),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(
        (result.completed, result.failed, result.not_attempted),
        (1, 1, 1)
    );
    assert!(result.statistics.is_none());
    assert_eq!(result.failures.len(), 1);
    assert_eq!(result.failures[0].phase, RunPhase::Accumulation);
    assert_eq!(result.failures[0].shot, Some(1));
    assert!(result.failures[0].message.contains("run.efficiency"));
    // Both attempted rows completed their quantum work, but only one was
    // committed. Actual costs must retain the discarded row's three queries.
    assert_eq!(result.execution_costs.query.attempted, 6);
    assert_eq!(result.execution_costs.query.failed, 0);
    assert_eq!(result.execution_costs.support.attempted, 2);
    let partial = result.partial_statistics.unwrap();
    assert_eq!(partial.completed, 1);
    assert_eq!(partial.accepted, ExactField::one());
    assert_eq!(
        partial.error,
        Some(if first[1] {
            ExactField::one()
        } else {
            ExactField::zero()
        })
    );
    assert_eq!(result.efficiency.unwrap().completed, 1);
    assert_eq!(result.sampled_records, Some(vec![first]));
}

#[test]
fn owned_preparation_matches_reused_batches_and_configuration_fails_before_preparation() {
    let files = Files::new();
    let registry = ControllerRegistry::default();
    let parse = ParseOptions::default();
    let physical =
        parse_circuit("X_ERROR(1/4) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]", &parse).unwrap();
    let modules = parse_task(
        r#"{"error_observables":[0],"sample_records":[0]}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap()
    .load_modules(&physical, &parse)
    .unwrap();
    let sampling = NoiseSampling::Importance {
        proposal_scale: "3/2".parse().unwrap(),
    };
    let options = TaskRunOptions {
        shots: 16,
        seed: 19,
        ..Default::default()
    };
    let owned = run_task_batch(
        &physical,
        &modules,
        &registry,
        EvaluationOptions::default(),
        &sampling,
        &options,
    )
    .unwrap();
    let prepared = TaskEvaluator::new_with_sampling(
        &physical,
        &modules,
        &registry,
        EvaluationOptions::default(),
        &sampling,
    )
    .unwrap();
    let reused = prepared.run_batch(&options).unwrap();
    assert_eq!(owned.status, RunStatus::Complete);
    assert_eq!(owned.statistics, reused.statistics);
    assert_eq!(owned.efficiency, reused.efficiency);
    assert_eq!(owned.sampled_records, reused.sampled_records);
    assert!(owned.timings.preparation_seconds.unwrap() >= 0.0);
    assert!(reused.timings.preparation_seconds.is_none());
    let mut impossible = EvaluationOptions::default();
    impossible.runtime.compile.max_gates = 0;
    assert_eq!(
        run_task_batch(
            &physical,
            &modules,
            &registry,
            impossible,
            &sampling,
            &TaskRunOptions {
                shots: 0,
                ..options
            }
        )
        .unwrap_err()
        .field,
        "run.shots"
    );
    assert_eq!(
        run_task_batch(
            &physical,
            &modules,
            &registry,
            impossible,
            &sampling,
            &TaskRunOptions {
                max_retained_record_values: 15,
                ..options
            }
        )
        .unwrap_err()
        .field,
        "run.max_retained_record_values"
    );
}
