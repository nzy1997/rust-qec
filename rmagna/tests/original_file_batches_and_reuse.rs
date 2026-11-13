// SPDX-License-Identifier: MPL-2.0
// Complete two NativeBatchTests and two FileReuseTests methods from
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/.
use num_rational::BigRational;
use num_traits::ToPrimitive;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::{ExecutionCosts, ExecutionLimits, ExecutionMeter};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::file_api::{FileLoadOptions, FileTask, FileTaskOptions};
use rmagna::noise::{NoiseSampling, SplitMix64};
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::statistics::{EfficiencyStatus, EfficiencySummary};
use rmagna::task::{
    EvaluationOptions, RunStatus, TaskEvaluation, TaskEvaluator, TaskOptions, TaskRunOptions,
    TaskRunStatistics, TaskVariances, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use serde_json::Value;
use std::{
    collections::BTreeMap,
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
            "rmagna-original-batch-reuse-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        std::fs::write(p.join("zero.magna"), "I 0\n").unwrap();
        std::fs::write(p.join("plus.magna"), "H 0\n").unwrap();
        Self(p)
    }
    fn load(
        &self,
        source: &str,
        task: &str,
        registry: &ControllerRegistry,
        options: EvaluationOptions,
        sampling: NoiseSampling,
    ) -> FileTask {
        let physical = self.0.join("physical.magna");
        let spec = self.0.join("task.json");
        std::fs::write(&physical, source).unwrap();
        std::fs::write(&spec, task).unwrap();
        FileTask::load(
            &physical,
            Some(&spec),
            registry,
            sampling,
            FileTaskOptions {
                evaluation: options,
                ..Default::default()
            },
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
        .unwrap()
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
        for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let mut e = EvaluationOptions::default();
                e.runtime.compile.compiler_strategy = compiler;
                e.runtime.strategy = strategy;
                e.runtime.history_sampler = sampler;
                result.push(e);
            }
        }
    }
    result
}
fn number(n: i64, d: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(BigRational::new(n.into(), d.into()))
        .unwrap()
}
fn sum(values: impl IntoIterator<Item = ExactField>) -> ExactField {
    values.into_iter().fold(ExactField::zero(), |a, b| {
        FieldArithmetic::default().add(&a, &b).unwrap()
    })
}
fn mul(a: &ExactField, b: &ExactField) -> ExactField {
    FieldArithmetic::default().multiply(a, b).unwrap()
}
fn div(a: &ExactField, b: &ExactField) -> ExactField {
    FieldArithmetic::default().divide(a, b).unwrap()
}
fn sub(a: &ExactField, b: &ExactField) -> ExactField {
    FieldArithmetic::default().subtract(a, b).unwrap()
}
fn square(a: &ExactField) -> ExactField {
    mul(a, a)
}
fn weight(row: &TaskEvaluation) -> ExactField {
    mul(
        &FieldArithmetic::default()
            .from_rational(row.noise_likelihood_ratio.clone())
            .unwrap(),
        &row.history_weight,
    )
}

// Center full lists independently. This oracle never uses ExactStatistics,
// HistoryEfficiency, production encoders or their incremental accumulators.
fn oracle(rows: &[TaskEvaluation], has_error: bool) -> (TaskRunStatistics, EfficiencySummary) {
    assert!(!rows.is_empty());
    let count = rows.len() as u64;
    let n = number(count as i64, 1);
    let a: Vec<_> = rows.iter().map(|r| r.accepted.clone()).collect();
    let e: Vec<_> = rows.iter().map(|r| r.error.clone()).collect();
    let w: Vec<_> = rows.iter().map(weight).collect();
    let mean = |v: &[ExactField]| div(&sum(v.iter().cloned()), &n);
    let ma = mean(&a);
    let me = mean(&e);
    let sw = sum(w.iter().cloned());
    let sw2 = sum(w.iter().map(square));
    let ess = if sw2.is_zero() {
        ExactField::zero()
    } else {
        div(&square(&sw), &sw2)
    };
    let var = |v: &[ExactField], m: &ExactField| {
        if count < 2 {
            None
        } else {
            Some(div(
                &sum(v.iter().map(|x| square(&sub(x, m)))),
                &number((count * (count - 1)) as i64, 1),
            ))
        }
    };
    let ratio = (has_error && !ma.is_zero()).then(|| div(&me, &ma));
    let residual: Vec<_> = ratio
        .as_ref()
        .map(|r| {
            a.iter()
                .zip(&e)
                .map(|(a, e)| square(&sub(e, &mul(r, a))))
                .collect()
        })
        .unwrap_or_default();
    let ratio_var = if count > 1 {
        ratio.as_ref().map(|_| {
            div(
                &sum(residual.iter().cloned()),
                &mul(&number((count * (count - 1)) as i64, 1), &square(&ma)),
            )
        })
    } else {
        None
    };
    let moments: Vec<_> = (0..rows[0].moments.len())
        .map(|j| {
            mean(
                &rows
                    .iter()
                    .map(|r| r.moments[j].clone())
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let moment_vars = moments
        .iter()
        .enumerate()
        .map(|(j, m)| {
            var(
                &rows
                    .iter()
                    .map(|r| r.moments[j].clone())
                    .collect::<Vec<_>>(),
                m,
            )
        })
        .collect();
    let statistics = TaskRunStatistics {
        completed: count,
        accepted: ma.clone(),
        error: has_error.then_some(me.clone()),
        conditional_error: ratio.clone(),
        moments,
        variances: TaskVariances {
            accepted: var(&a, &ma),
            error: if has_error { var(&e, &me) } else { None },
            conditional_error: ratio_var,
            moments: moment_vars,
        },
        mean_weight: mean(&w),
        effective_sample_size: ess.clone(),
    };
    let actual = sum(residual.iter().cloned());
    let reference = sum(rows.iter().zip(&w).zip(&residual).map(|((row, w), r)| {
        if w.is_zero() {
            ExactField::zero()
        } else {
            let l = FieldArithmetic::default()
                .from_rational(row.noise_likelihood_ratio.clone())
                .unwrap();
            mul(&div(&l, w), r)
        }
    }));
    let status = if !has_error {
        EfficiencyStatus::NoErrorMetric
    } else if count < 2 {
        EfficiencyStatus::InsufficientSamples
    } else if ma.is_zero() {
        EfficiencyStatus::ZeroAcceptance
    } else if reference.is_zero() {
        EfficiencyStatus::ZeroResidualVariance
    } else {
        EfficiencyStatus::Available
    };
    let available = status == EfficiencyStatus::Available;
    let denom = square(&sum(a));
    let efficiency = EfficiencySummary {
        status,
        completed: count,
        zero_weights: w.iter().filter(|w| w.is_zero()).count() as u64,
        kappa: available.then(|| div(&actual, &reference)),
        variance_coefficient: available.then(|| div(&mul(&n, &actual), &denom)),
        exact_marginal_variance_coefficient: available.then(|| div(&mul(&n, &reference), &denom)),
        effective_sample_size: ess.clone(),
        rho: (!ess.is_zero()).then(|| div(&n, &ess)),
        mean_weight: Some(mean(&w)),
    };
    (statistics, efficiency)
}
fn real(value: &ExactField) -> f64 {
    let c = value.coefficients();
    c[0].to_f64().unwrap() + (c[1].to_f64().unwrap() - c[3].to_f64().unwrap()) * 0.5_f64.sqrt()
}
fn close(actual: f64, expected: f64) {
    assert!(actual.is_finite() && expected.is_finite());
    assert!(
        (actual - expected).abs() <= 1e-11 * (1.0 + expected.abs()),
        "{actual} versus {expected}"
    );
}
fn exported(ordinary: &Value, scientific: &Value, exact: Option<&ExactField>, root: bool) {
    let Some(exact) = exact else {
        assert!(ordinary.is_null() && scientific.is_null());
        return;
    };
    let mut expected = real(exact);
    if root {
        expected = expected.sqrt();
    }
    close(ordinary.as_f64().unwrap(), expected);
    let m = scientific["mantissa"].as_f64().unwrap();
    let e = scientific["exponent2"].as_i64().unwrap();
    assert!(m == 0.0 || (0.5..1.0).contains(&m.abs()));
    close(m * 2_f64.powi(e.try_into().unwrap()), expected);
}
fn check_exports(value: &Value, stats: &TaskRunStatistics, eff: &EfficiencySummary) {
    for (name, exact, var) in [
        (
            "acceptance",
            Some(&stats.accepted),
            stats.variances.accepted.as_ref(),
        ),
        (
            "error",
            stats.error.as_ref(),
            stats.variances.error.as_ref(),
        ),
        (
            "conditional_error",
            stats.conditional_error.as_ref(),
            stats.variances.conditional_error.as_ref(),
        ),
    ] {
        exported(
            &value["estimates"][name],
            &value["estimates_scientific"][name],
            exact,
            false,
        );
        exported(
            &value["standard_errors"][name],
            &value["standard_errors_scientific"][name],
            var,
            true,
        );
    }
    for key in [
        "estimates",
        "estimates_scientific",
        "standard_errors",
        "standard_errors_scientific",
    ] {
        assert!(value[key]["moments"].as_array().unwrap().is_empty());
    }
    let v = &value["efficiency"];
    assert_eq!(v["status"], eff.status.as_str());
    assert_eq!(v["completed"], eff.completed);
    assert_eq!(v["zero_weights"], eff.zero_weights);
    for (name, exact) in [
        ("kappa", eff.kappa.as_ref()),
        ("variance_coefficient", eff.variance_coefficient.as_ref()),
        (
            "exact_marginal_variance_coefficient",
            eff.exact_marginal_variance_coefficient.as_ref(),
        ),
        ("ess", Some(&eff.effective_sample_size)),
        ("rho", eff.rho.as_ref()),
        ("mean_weight", eff.mean_weight.as_ref()),
    ] {
        exported(&v[name], &v[format!("{name}_scientific")], exact, false);
    }
    let w = &value["weights"];
    assert_eq!(w["completed"], stats.completed);
    exported(
        &w["mean"],
        &w["mean_scientific"],
        Some(&stats.mean_weight),
        false,
    );
    exported(
        &w["ess"],
        &w["ess_scientific"],
        Some(&stats.effective_sample_size),
        false,
    );
}
fn costs(a: ExecutionCosts, b: ExecutionCosts) {
    for (a, b) in [
        (a.compilation, b.compilation),
        (a.query, b.query),
        (a.decoder, b.decoder),
        (a.support, b.support),
    ] {
        assert_eq!(
            (a.attempted, a.succeeded, a.failed),
            (b.attempted, b.succeeded, b.failed)
        );
    }
}
fn manual(
    files: &Files,
    source: &str,
    task: &str,
    registry: &ControllerRegistry,
    evaluation: EvaluationOptions,
    sampling: &NoiseSampling,
    run: TaskRunOptions,
) -> (Vec<TaskEvaluation>, ExecutionCosts, Option<String>) {
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit(source, &parse).unwrap();
    let modules = parse_task(task, &files.0, &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &parse)
        .unwrap();
    let meter = ExecutionMeter::new(run.execution_limits);
    let evaluator =
        TaskEvaluator::new_with_costs(&circuit, &modules, registry, evaluation, sampling, &meter)
            .unwrap();
    let mut rng = SplitMix64::new(run.seed);
    let mut rows = vec![];
    let mut failure = None;
    for _ in 0..run.shots {
        match evaluator.sample_with_costs(&mut rng, &meter) {
            Ok(row) => rows.push(row),
            Err(error) => {
                failure = Some(error.to_string());
                break;
            }
        }
    }
    (rows, meter.snapshot(), failure)
}

#[test]
fn original_aggregate_matches_single_draws() {
    const SOURCE: &str = "H 0\nT 0\nZ_ERROR(1/10) 0\nMX 0";
    const TASK: &str = r#"{"outputs":[0],"sample_records":[0],"ideal_state":"plus.magna"}"#;
    let files = Files::new();
    let registry = ControllerRegistry::default();
    for options in configurations() {
        for sampling in [
            NoiseSampling::Physical,
            NoiseSampling::Importance {
                proposal_scale: BigRational::from_integer(2.into()),
            },
        ] {
            let run = TaskRunOptions {
                shots: 79,
                seed: 391,
                ..Default::default()
            };
            let (rows, observed, failure) =
                manual(&files, SOURCE, TASK, &registry, options, &sampling, run);
            assert!(failure.is_none());
            assert_eq!(rows.len(), 79);
            for row in &rows {
                assert_eq!(row.sampled_records.len(), 1);
                let w = weight(row);
                let bit = row.sampled_records[0];
                assert_eq!(row.accepted, w);
                assert_eq!(row.error, if bit { w.clone() } else { ExactField::zero() });
                assert_eq!(
                    row.target_success,
                    Some(if bit { ExactField::zero() } else { w })
                );
            }
            let (stats, eff) = oracle(&rows, true);
            let file = files.load(SOURCE, TASK, &registry, options, sampling);
            let complete_request = run;
            let report = file.run(&complete_request).unwrap();
            let raw = report.raw_result();
            assert_eq!(raw.status, RunStatus::Complete);
            assert_eq!(raw.statistics, Some(stats.clone()));
            assert_eq!(raw.efficiency, Some(eff.clone()));
            costs(raw.execution_costs, observed);
            let records: Vec<_> = rows.iter().map(|r| r.sampled_records.clone()).collect();
            assert_eq!(raw.sampled_records, Some(records.clone()));
            let json = report.to_json().unwrap();
            check_exports(&json, &stats, &eff);
            assert_eq!(json["sampled_records"], serde_json::json!(records));
            assert_eq!(
                json["execution_costs"]["query"]["attempted"],
                observed.query.attempted
            );
            assert_eq!(
                std::fs::read_to_string(files.0.join("physical.magna")).unwrap(),
                SOURCE
            );
            assert_eq!(
                std::fs::read_to_string(files.0.join("task.json")).unwrap(),
                TASK
            );
        }
    }
}

#[test]
fn original_failure_retains_only_completed_prefix() {
    const SOURCE: &str = "X_ERROR(1/3) 0\nM 0";
    const TASK: &str = r#"{"outputs":[0],"ideal_state":"zero.magna"}"#;
    let files = Files::new();
    let registry = ControllerRegistry::default();
    for options in configurations() {
        let run = TaskRunOptions {
            shots: 20,
            seed: 99,
            execution_limits: ExecutionLimits {
                max_queries: 11,
                ..Default::default()
            },
            ..Default::default()
        };
        let (rows, observed, failure) = manual(
            &files,
            SOURCE,
            TASK,
            &registry,
            options,
            &NoiseSampling::Physical,
            run,
        );
        assert!(failure.as_ref().unwrap().contains("max_queries"));
        assert!(!rows.is_empty() && rows.len() < 20);
        let aggregate_request = run;
        let report = files
            .load(SOURCE, TASK, &registry, options, NoiseSampling::Physical)
            .run(&aggregate_request)
            .unwrap();
        let raw = report.raw_result();
        assert_eq!(raw.status, RunStatus::Failed);
        assert_eq!(raw.completed, rows.len() as u64);
        assert_eq!((raw.failed, raw.not_attempted), (1, 19 - rows.len() as u64));
        assert_eq!(raw.failures[0].shot, Some(rows.len() as u64));
        assert_eq!(raw.failures[0].message, failure.unwrap());
        assert!(raw.statistics.is_none());
        costs(raw.execution_costs, observed);
        let (stats, eff) = oracle(&rows, true);
        assert_eq!(raw.partial_statistics, Some(stats.clone()));
        assert_eq!(raw.efficiency, Some(eff.clone()));
        let json = report.to_json().unwrap();
        assert!(json["estimates"].is_null());
        let mut prefix = json["partial_statistics"].clone();
        prefix["efficiency"] = json["efficiency"].clone();
        prefix["weights"] = json["weights"].clone();
        check_exports(&prefix, &stats, &eff);
    }
}

fn failing_registry() -> (ControllerRegistry, Arc<Mutex<Vec<Vec<bool>>>>) {
    let calls = Arc::new(Mutex::new(vec![]));
    let recorded = Arc::clone(&calls);
    let mut registry = ControllerRegistry::default();
    registry
        .register("fail", "1", move |bits: &[bool]| {
            let mut rows = recorded.lock().unwrap();
            rows.push(bits.to_vec());
            if rows.len() == 4 {
                Err("declared callback failure".into())
            } else {
                Ok(bits.to_vec())
            }
        })
        .unwrap();
    (registry, calls)
}

#[test]
fn original_batch_failure_keeps_completed_prefix_and_query_budget_contract() {
    const SOURCE: &str =
        "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[fail] det[-1] -> ctrl[0]\nCX ctrl[0] 1";
    const TASK: &str = r#"{"outputs":[0,1]}"#;
    let files = Files::new();
    for options in configurations() {
        let (registry, calls) = failing_registry();
        let run = TaskRunOptions {
            shots: 70,
            seed: 8,
            ..Default::default()
        };
        let ordinary = files
            .load(SOURCE, TASK, &registry, options, NoiseSampling::Physical)
            .run(&run)
            .unwrap();
        let ordinary_calls = calls.lock().unwrap().clone();
        let (registry, calls) = failing_registry();
        let budget = TaskRunOptions {
            execution_limits: ExecutionLimits {
                max_queries: 10000,
                ..Default::default()
            },
            ..run
        };
        let direct = files
            .load(SOURCE, TASK, &registry, options, NoiseSampling::Physical)
            .run(&budget)
            .unwrap();
        let a = ordinary.raw_result();
        let b = direct.raw_result();
        assert_eq!(a.status, RunStatus::Failed);
        assert_eq!((a.completed, a.failed, a.not_attempted), (3, 1, 66));
        assert_eq!(a.failures[0].shot, Some(3));
        assert!(a.failures[0].message.contains("declared callback failure"));
        assert_eq!(a.failures, b.failures);
        assert_eq!(a.partial_statistics, b.partial_statistics);
        assert_eq!(a.efficiency, b.efficiency);
        assert!(a.statistics.is_none() && b.statistics.is_none());
        assert_eq!(ordinary_calls, *calls.lock().unwrap());
        assert_eq!(ordinary_calls.len(), 4);
        assert_eq!(a.execution_costs.decoder.attempted, 4);
        assert_eq!(a.execution_costs.decoder.failed, 1);
        let (registry, _) = failing_registry();
        let (rows, observed, failure) = manual(
            &files,
            SOURCE,
            TASK,
            &registry,
            options,
            &NoiseSampling::Physical,
            run,
        );
        assert_eq!(rows.len(), 3);
        assert_eq!(failure, Some(a.failures[0].message.clone()));
        costs(a.execution_costs, observed);
        let (stats, eff) = oracle(&rows, false);
        assert_eq!(a.partial_statistics, Some(stats));
        assert_eq!(a.efficiency, Some(eff));
        let ja = ordinary.to_json().unwrap();
        let jb = direct.to_json().unwrap();
        for key in [
            "status",
            "completed",
            "failed",
            "not_attempted",
            "failures",
            "partial_statistics",
        ] {
            assert_eq!(ja[key], jb[key]);
        }
        assert!(ja["estimates"].is_null());
        let limited = files
            .load(
                "H 0\nM 0",
                "{}",
                &ControllerRegistry::default(),
                options,
                NoiseSampling::Physical,
            )
            .run(&TaskRunOptions {
                shots: 3,
                execution_limits: ExecutionLimits {
                    max_queries: 2,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();
        let raw = limited.raw_result();
        assert_eq!((raw.completed, raw.failed, raw.not_attempted), (2, 1, 0));
        assert_eq!(raw.failures[0].shot, Some(2));
        assert!(raw.statistics.is_none());
        assert_eq!(raw.partial_statistics.as_ref().unwrap().completed, 2);
    }
}

#[test]
fn original_fixed_preparation_is_reused_for_new_original_faults() {
    const SOURCE: &str = "H 0\nT 0\nDEPOLARIZE1(1/10) 0\nM(1/10) 0\nCX rec[-1] 1";
    const TASK: &str = r#"{"outputs":[1],"ideal_state":"zero.magna"}"#;
    let files = Files::new();
    let parse = ParseOptions::default();
    let physical = parse_circuit(SOURCE, &parse).unwrap();
    let modules = parse_task(TASK, &files.0, &TaskOptions::default())
        .unwrap()
        .load_modules(&physical, &parse)
        .unwrap();
    let registry = ControllerRegistry::default();
    for options in configurations() {
        let meter = ExecutionMeter::default();
        let evaluator = TaskEvaluator::new_with_costs(
            &physical,
            &modules,
            &registry,
            options,
            &NoiseSampling::Physical,
            &meter,
        )
        .unwrap();
        assert_eq!(meter.snapshot().compilation.succeeded, 2);
        let mut rng = SplitMix64::new(4);
        for _ in 0..20 {
            let reused_draw = evaluator.sample_with_costs(&mut rng, &meter).unwrap();
            assert_eq!(reused_draw.network_compilations, 0);
            assert_eq!(meter.snapshot().compilation.succeeded, 2);
            assert_eq!(reused_draw.accepted, ExactField::one());
            assert_eq!(
                sum([
                    reused_draw.error.clone(),
                    reused_draw.target_success.unwrap()
                ]),
                ExactField::one()
            );
        }
        let compiled = files
            .load(SOURCE, TASK, &registry, options, NoiseSampling::Physical)
            .compile()
            .unwrap();
        let inspect = compiled.inspect(&TaskRunOptions::default()).unwrap();
        assert_eq!(
            inspect["compilation"]["preparation_costs"]["compilation"]["succeeded"],
            2
        );
        let report = compiled
            .run(&TaskRunOptions {
                shots: 20,
                seed: 4,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(report.raw_result().status, RunStatus::Complete);
        assert_eq!(report.raw_result().completed, 20);
        assert_eq!(report.raw_result().completed_network_compilations, 0);
        assert_eq!(report.raw_result().execution_costs.compilation.succeeded, 0);
    }
}
