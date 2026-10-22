use num_bigint::BigInt;
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::noise::NoiseSampling;
use rmagna::numeric::NumericOptions;
use rmagna::runtime::ControllerRegistry;
use rmagna::statistics::StatisticsOptions;
use rmagna::task::{
    BatchExportError, EvaluationOptions, RunFailure, RunPhase, RunStatus, TaskEvaluator,
    TaskOptions, TaskRunOptions, TaskRunResult, export_batch_metrics, parse_task,
};
use serde_json::{Value, json};
use std::path::PathBuf;

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rmagna-export-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn put(&self, name: &str, content: &str) {
        std::fs::write(self.0.join(name), content).unwrap();
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn run(source: &str, task: &str, options: TaskRunOptions) -> TaskRunResult {
    let files = Files::new();
    files.put("ideal.magna", "I 0");
    let parse = ParseOptions::default();
    let source = parse_circuit(source, &parse).unwrap();
    let modules = parse_task(task, &files.0, &TaskOptions::default())
        .unwrap()
        .load_modules(&source, &parse)
        .unwrap();
    let e = TaskEvaluator::new_with_sampling(
        &source,
        &modules,
        &ControllerRegistry::default(),
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    )
    .unwrap();
    e.run_batch(&options).unwrap()
}
fn base() -> TaskRunResult {
    run(
        "X_ERROR(1/2) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]",
        r#"{"error_observables":[0]}"#,
        TaskRunOptions {
            shots: 2,
            seed: 6,
            ..Default::default()
        },
    )
}
fn export(result: &TaskRunResult) -> Value {
    export_batch_metrics(result, NumericOptions::default()).unwrap()
}
fn contains(values: &Value, name: &str) -> bool {
    values.as_array().unwrap().iter().any(|v| v == name)
}

#[test]
fn literal_binary_statistics_and_standard_errors_use_native_scientific_fields() {
    let result = base();
    let value = export(&result);
    assert_eq!(value["status"], "complete");
    assert_eq!(value["completed"], 2);
    assert_eq!(
        value["estimates"],
        json!({"acceptance":1.0,"error":0.5,"conditional_error":0.5,"moments":[]})
    );
    assert_eq!(
        value["standard_errors"],
        json!({"acceptance":0.0,"error":0.5,"conditional_error":0.5,"moments":[]})
    );
    assert_eq!(
        value["estimates_scientific"]["acceptance"],
        json!({"mantissa":0.5,"exponent2":1})
    );
    assert_eq!(
        value["standard_errors_scientific"]["error"],
        json!({"mantissa":0.5,"exponent2":0})
    );
    assert_eq!(value["efficiency"]["kappa"], 1.0);
    assert_eq!(value["efficiency"]["status"], "available");
    assert_eq!(value["weights"]["ess"], 2.0);
    assert_eq!(value["weights"]["mean"], 1.0);
    assert_eq!(value["numerics"]["underflow_fields"], json!([]));
    assert_eq!(value["numerics"]["overflow_fields"], json!([]));
    assert!(value.get("partial_statistics").is_none());
    assert!(value.get("schema_version").is_none());
    let text = serde_json::to_string(&value).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), value);
    let negative = export(&run(
        "X 0",
        r#"{"outputs":[0],"ideal_state":"ideal.magna","observables":[[0,1]]}"#,
        TaskRunOptions {
            shots: 2,
            ..Default::default()
        },
    ));
    assert_eq!(negative["estimates"]["moments"], json!([-1.0]));
    assert_eq!(
        negative["estimates_scientific"]["moments"][0],
        json!({"mantissa":-0.5,"exponent2":1})
    );
}

#[test]
fn tiny_and_huge_numeric_witnesses_keep_scientific_values_and_exact_zero_distinct() {
    let f = FieldArithmetic::default();
    let tiny = f
        .from_rational(BigRational::new(1.into(), BigInt::from(10).pow(400)))
        .unwrap();
    let huge = f
        .from_rational(BigRational::from_integer(BigInt::from(10).pow(400)))
        .unwrap();
    let mut result = base();
    let s = result.statistics.as_mut().unwrap();
    s.accepted = tiny.clone();
    s.error = Some(tiny.clone());
    s.conditional_error = Some(ExactField::one());
    s.variances.accepted = Some(f.multiply(&tiny, &tiny).unwrap());
    s.variances.error = s.variances.accepted.clone();
    s.variances.conditional_error = Some(ExactField::zero());
    s.moments = vec![tiny.negated(), huge.clone()];
    s.variances.moments = vec![
        Some(f.multiply(&tiny, &tiny).unwrap()),
        Some(f.multiply(&huge, &huge).unwrap()),
    ];
    let value = export(&result);
    for path in ["acceptance", "error"] {
        assert!(value["estimates"][path].is_null());
        assert!(value["standard_errors"][path].is_null());
        assert_eq!(value["estimates_scientific"][path]["exponent2"], -1328);
    }
    assert_eq!(value["standard_errors"]["conditional_error"], 0.0);
    assert_eq!(
        value["standard_errors_scientific"]["conditional_error"],
        json!({"mantissa":0.0,"exponent2":0})
    );
    assert!(contains(
        &value["numerics"]["underflow_fields"],
        "estimates.moments[0]"
    ));
    assert!(contains(
        &value["numerics"]["overflow_fields"],
        "estimates.moments[1]"
    ));
    assert!(contains(
        &value["numerics"]["overflow_fields"],
        "standard_errors.moments[1]"
    ));
    assert!(
        value["estimates_scientific"]["moments"][0]["mantissa"]
            .as_f64()
            .unwrap()
            < 0.0
    );
    assert_eq!(
        value["estimates_scientific"]["moments"][1]["exponent2"],
        1329
    );
    serde_json::to_string(&value).unwrap();
    let error = export_batch_metrics(&result, NumericOptions { max_work_bits: 32 }).unwrap_err();
    assert!(matches!(error,BatchExportError::Numeric {field,..} if field=="estimates.acceptance"));
}

#[test]
fn direct_rounding_and_extreme_kappa_preserve_certified_range_diagnostics() {
    let f = FieldArithmetic::default();
    let d = |p: usize| {
        f.from_rational(BigRational::new(1.into(), BigInt::from(1) << p))
            .unwrap()
    };
    let mut result = base();
    let s = result.statistics.as_mut().unwrap();
    s.accepted = f.add(&d(1075), &d(1200)).unwrap();
    let value = export(&result);
    assert_eq!(
        value["estimates"]["acceptance"].as_f64().unwrap().to_bits(),
        1
    );
    assert_eq!(
        value["estimates_scientific"]["acceptance"],
        json!({"mantissa":0.5,"exponent2":-1074})
    );
    for (power, list) in [(2000, "underflow_fields"), (0, "overflow_fields")] {
        let mut result = base();
        result.efficiency.as_mut().unwrap().kappa = Some(if power == 0 {
            f.from_rational(BigRational::from_integer(BigInt::from(1) << 2000))
                .unwrap()
        } else {
            d(power)
        });
        let value = export(&result);
        assert_eq!(value["efficiency"]["status"], "diagnostic_range_exceeded");
        assert!(value["efficiency"]["kappa"].is_null());
        assert!(value["efficiency"]["kappa_scientific"].is_object());
        assert!(contains(&value["numerics"][list], "efficiency.kappa"));
    }
}

#[test]
fn missing_metrics_and_one_sample_errors_stay_null_without_underflow_flags() {
    let value = export(&run("I 0", "{}", TaskRunOptions::default()));
    assert_eq!(value["estimates"]["acceptance"], 1.0);
    assert!(value["estimates"]["error"].is_null());
    assert!(value["estimates_scientific"]["error"].is_null());
    assert!(value["standard_errors"]["acceptance"].is_null());
    assert_eq!(value["efficiency"]["status"], "no_error_metric");
    assert_eq!(value["numerics"]["underflow_fields"], json!([]));
    let value = export(&run(
        "M 0\nDETECTOR rec[-1]\nPOSTSELECT !det[0]\nOBSERVABLE_INCLUDE(0) rec[-1]",
        r#"{"error_observables":[0]}"#,
        TaskRunOptions {
            shots: 2,
            ..Default::default()
        },
    ));
    assert_eq!(value["estimates"]["acceptance"], 0.0);
    assert_eq!(value["estimates"]["error"], 0.0);
    assert!(value["estimates"]["conditional_error"].is_null());
    assert_eq!(value["efficiency"]["status"], "zero_acceptance");
}

#[test]
fn partial_and_finalization_failures_do_not_publish_complete_estimates() {
    let files = Files::new();
    let mut registry = ControllerRegistry::default();
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    registry
        .register("once", "1", move |b: &[bool]| {
            if counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0 {
                Ok(b.to_vec())
            } else {
                Err("offline".to_owned())
            }
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let source = parse_circuit(
        "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[once] det[0] -> ctrl[0]",
        &parse,
    )
    .unwrap();
    let modules = parse_task(
        r#"{"sample_records":[0]}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap()
    .load_modules(&source, &parse)
    .unwrap();
    let e = TaskEvaluator::new(&source, &modules, &registry, EvaluationOptions::default()).unwrap();
    let mut result = e
        .run_batch(&TaskRunOptions {
            shots: 4,
            ..Default::default()
        })
        .unwrap();
    let f = FieldArithmetic::default();
    result.partial_statistics.as_mut().unwrap().accepted = f
        .from_rational(BigRational::new(1.into(), BigInt::from(10).pow(400)))
        .unwrap();
    let value = export(&result);
    assert_eq!(value["status"], "failed");
    assert_eq!(
        (
            value["completed"].as_u64(),
            value["failed"].as_u64(),
            value["not_attempted"].as_u64()
        ),
        (Some(1), Some(1), Some(2))
    );
    assert!(value["estimates"].is_null() && value["standard_errors"].is_null());
    assert!(value["partial_statistics"]["estimates_scientific"]["acceptance"].is_object());
    assert!(contains(
        &value["numerics"]["underflow_fields"],
        "partial_statistics.estimates.acceptance"
    ));
    assert_eq!(value["sampled_records"].as_array().unwrap().len(), 1);
    assert_eq!(value["failures"][0]["phase"], "sampling");
    let failed = run(
        "I 0",
        "{}",
        TaskRunOptions {
            shots: 3,
            statistics: StatisticsOptions {
                arithmetic: FieldArithmetic::new(2),
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(
        failed
            .failures
            .iter()
            .all(|f| f.phase == RunPhase::Finalization)
    );
    let value = export(&failed);
    assert_eq!(value["completed"], 3);
    assert_eq!(value["failed"], 0);
    assert!(value["partial_statistics"].is_null());
    assert_eq!(value["failures"][0]["phase"], "finalization");
}

#[test]
fn inconsistent_public_results_and_negative_variances_are_explicit_errors() {
    let original = base();
    let mut bad = original.clone();
    bad.completed = 1;
    assert!(matches!(
        export_batch_metrics(&bad, NumericOptions::default()),
        Err(BatchExportError::InvalidState(_))
    ));
    let mut bad = original.clone();
    bad.timings.total_seconds = f64::NAN;
    assert!(matches!(
        export_batch_metrics(&bad, NumericOptions::default()),
        Err(BatchExportError::InvalidState(_))
    ));
    let mut bad = original.clone();
    bad.statistics
        .as_mut()
        .unwrap()
        .moments
        .push(ExactField::one());
    assert!(matches!(
        export_batch_metrics(&bad, NumericOptions::default()),
        Err(BatchExportError::InvalidState(_))
    ));
    let mut bad = original.clone();
    bad.efficiency.as_mut().unwrap().completed = 1;
    assert!(matches!(
        export_batch_metrics(&bad, NumericOptions::default()),
        Err(BatchExportError::InvalidState(_))
    ));
    let mut bad = original.clone();
    bad.status = RunStatus::Failed;
    bad.requested = 3;
    bad.failed = 1;
    bad.partial_statistics = bad.statistics.take();
    bad.failures = vec![RunFailure {
        phase: RunPhase::Sampling,
        shot: Some(0),
        kind: "ProbeError",
        message: "offline".to_owned(),
    }];
    assert!(matches!(
        export_batch_metrics(&bad, NumericOptions::default()),
        Err(BatchExportError::InvalidState(_))
    ));
    bad.failures[0].shot = Some(2);
    assert_eq!(
        export(&bad)["partial_statistics"]["estimates"]["error"],
        0.5
    );
    bad.failed = 0;
    bad.not_attempted = 1;
    bad.failures[0].phase = RunPhase::Preparation;
    bad.failures[0].shot = None;
    assert!(matches!(
        export_batch_metrics(&bad, NumericOptions::default()),
        Err(BatchExportError::InvalidState(_))
    ));
    let mut bad = original;
    bad.statistics.as_mut().unwrap().variances.error = Some(ExactField::one().negated());
    assert!(
        matches!(export_batch_metrics(&bad,NumericOptions::default()),Err(BatchExportError::Numeric {field,..}) if field=="standard_errors.error")
    );
}
