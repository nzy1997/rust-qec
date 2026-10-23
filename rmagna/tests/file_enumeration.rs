use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::{ExecutionLimits, OperationCost};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::file_api::{FileEnumerationOptions, FileLoadOptions, FileTask, FileTaskOptions};
use rmagna::noise::NoiseSampling;
use rmagna::runtime::{CausalStrategy, ControllerRegistry};
use rmagna::task::{TaskOptions, TaskRunOptions, parse_task};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-file-enumeration-{}-{}",
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
fn loaded(
    source: &str,
    task: &str,
    base: &Path,
    registry: &ControllerRegistry,
    options: FileTaskOptions,
    sampling: NoiseSampling,
) -> FileTask {
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let physical = parse_circuit(source, &parse).unwrap();
    let modules = parse_task(task, base, &TaskOptions::default())
        .unwrap()
        .load_modules(&physical, &parse)
        .unwrap();
    FileTask::from_loaded(
        physical,
        modules,
        registry,
        sampling,
        options,
        BTreeMap::new(),
    )
    .unwrap()
}
fn simple(options: FileTaskOptions) -> FileTask {
    loaded(
        "X_ERROR(1/3) 0\nX_ERROR(1/5) 1\nM 0 1\nOBSERVABLE_INCLUDE(0) rec[-2] rec[-1]",
        r#"{"error_observables":[0]}"#,
        Path::new("/tmp"),
        &ControllerRegistry::default(),
        options,
        NoiseSampling::Physical,
    )
}
fn field(n: i64, d: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(BigRational::new(n.into(), d.into()))
        .unwrap()
}
fn count(c: OperationCost, expected: (u64, u64, u64)) {
    assert_eq!((c.attempted, c.succeeded, c.failed), expected);
    assert!(c.seconds.is_finite() && c.seconds >= 0.0);
}
fn exact(v: &Value, n: i64, d: i64) {
    assert_eq!(
        v["exact"]["coefficients"],
        json!([{"numerator":n.to_string(),"denominator":d.to_string()},{"numerator":"0","denominator":"1"},{"numerator":"0","denominator":"1"},{"numerator":"0","denominator":"1"}])
    );
}
#[test]
fn fresh_and_reused_sector_reports_preserve_unconditional_masses_bounds_and_identities() {
    let file = simple(FileTaskOptions::default());
    let options = FileEnumerationOptions::default();
    let compiled = file.compile().unwrap();
    let fresh = file.enumerate(&options).unwrap();
    let first = compiled.enumerate(&options).unwrap();
    let second = compiled.enumerate(&options).unwrap();
    assert_eq!(fresh.raw_result(), first.raw_result());
    assert_eq!(first.raw_result(), second.raw_result());
    let raw = fresh.raw_result().unwrap();
    assert_eq!(raw.masses.accepted, field(14, 15));
    assert_eq!(raw.masses.error, field(2, 5));
    assert_eq!(raw.omitted_probability.to_string(), "1/15");
    assert_eq!((raw.patterns, raw.histories), (3, 3));
    count(fresh.execution_costs().compilation, (1, 1, 0));
    count(first.execution_costs().compilation, (0, 0, 0));
    count(fresh.execution_costs().query, (6, 6, 0));
    count(first.execution_costs().query, (6, 6, 0));
    let a = fresh.to_json().unwrap();
    let b = first.to_json().unwrap();
    let c = second.to_json().unwrap();
    assert_eq!(a["schema_version"], 2);
    assert_eq!(a["kind"], "enumeration");
    assert_eq!(a["status"], "complete");
    assert_eq!(a["result"], b["result"]);
    assert_eq!(b["result"], c["result"]);
    assert_eq!(a["fingerprints"], b["fingerprints"]);
    assert_eq!(b["fingerprints"], c["fingerprints"]);
    assert_eq!(
        a["result"]["covered_probability"],
        json!({"numerator":"14","denominator":"15"})
    );
    exact(&a["result"]["masses"]["accepted"], 14, 15);
    exact(&a["result"]["masses"]["error"], 2, 5);
    exact(&a["result"]["full_acceptance"]["upper"], 1, 1);
    exact(&a["result"]["full_error"]["upper"], 7, 15);
    exact(&a["result"]["conditional_error"]["lower"], 2, 5);
    exact(&a["result"]["conditional_error"]["upper"], 7, 15);
    exact(&a["result"]["by_weight"][0]["masses"]["accepted"], 8, 15);
    exact(&a["result"]["by_weight"][1]["masses"]["error"], 2, 5);
    assert_eq!(a["compilation"]["preparation_included_in_this_call"], true);
    assert_eq!(b["compilation"]["preparation_included_in_this_call"], false);
    assert!(a["options"].get("run").is_none());
    assert!(a.get("seed").is_none());
    assert!(a.get("shots").is_none());
    assert_eq!(a["sampling"]["entropy"], "none");
    let inspection = file.inspect(&TaskRunOptions::default()).unwrap();
    assert_eq!(
        a["fingerprints"]["circuit"],
        inspection["fingerprints"]["circuit"]
    );
    assert_ne!(
        a["fingerprints"]["options"],
        inspection["fingerprints"]["options"]
    );
    assert_ne!(
        a["fingerprints"]["sampling"],
        inspection["fingerprints"]["sampling"]
    );
    let mut full = options;
    full.enumeration.noise.max_weight = 2;
    let full = compiled.enumerate(&full).unwrap().to_json().unwrap();
    assert_ne!(
        full["fingerprints"]["options"],
        a["fingerprints"]["options"]
    );
    assert_eq!(
        full["result"]["omitted_probability"],
        json!({"numerator":"0","denominator":"1"})
    );
    exact(&full["result"]["conditional_error"]["lower"], 2, 5);
    exact(&full["result"]["conditional_error"]["upper"], 2, 5);
}
#[test]
fn captured_projected_target_and_recovery_survive_source_deletion() {
    let files = Files::new();
    files.put("physical.magna", "X_ERROR(1/4) 2");
    files.put("target.magna", "H 0\nCX 0 1");
    files.put("recovery.magna", "X 0");
    files.put("task.json",r#"{"outputs":[2],"ideal_state":{"circuit":"target.magna","outputs":[0],"postselection":{"1":0}},"recovery":"recovery.magna","observables":[[0,1]]}"#);
    let file = FileTask::load(
        &files.0.join("physical.magna"),
        Some(&files.0.join("task.json")),
        &ControllerRegistry::default(),
        NoiseSampling::Physical,
        FileTaskOptions::default(),
        &FileLoadOptions::default(),
        BTreeMap::new(),
    )
    .unwrap();
    std::fs::remove_dir_all(&files.0).unwrap();
    let compiled = file.compile().unwrap();
    let options = FileEnumerationOptions::default();
    let fresh = file.enumerate(&options).unwrap();
    let reused = compiled.enumerate(&options).unwrap();
    assert_eq!(fresh.raw_result(), reused.raw_result());
    let raw = fresh.raw_result().unwrap();
    assert_eq!(raw.masses.accepted, ExactField::one());
    assert_eq!(raw.masses.error, field(3, 4));
    assert_eq!(raw.masses.target_success, Some(field(1, 4)));
    assert_eq!(raw.masses.moments, [field(-1, 2)]);
    count(fresh.execution_costs().query, (7, 7, 0));
    count(reused.execution_costs().query, (6, 6, 0));
    count(fresh.execution_costs().compilation, (2, 2, 0));
    count(reused.execution_costs().compilation, (0, 0, 0));
    let a = fresh.to_json().unwrap();
    let b = reused.to_json().unwrap();
    assert_eq!(a["fingerprints"], b["fingerprints"]);
    exact(&a["result"]["masses"]["target_success"], 1, 4);
    exact(&a["result"]["masses"]["moments"][0], -1, 2);
    let quota = FileEnumerationOptions {
        execution_limits: ExecutionLimits {
            max_queries: 1,
            max_duration: None,
        },
        ..Default::default()
    };
    let failed = file.enumerate(&quota).unwrap();
    assert!(
        failed
            .raw_result()
            .unwrap_err()
            .to_string()
            .contains("max_queries")
    );
    count(failed.execution_costs().query, (1, 1, 0));
    count(failed.execution_costs().compilation, (2, 2, 0));
    assert_eq!(
        failed.to_json().unwrap()["compilation"]["preparation_costs"]["query"]["succeeded"],
        1
    );
}
fn nonlinear(strategy: CausalStrategy, calls: &Arc<AtomicUsize>, fail: bool) -> FileTask {
    let mut registry = ControllerRegistry::default();
    let calls = calls.clone();
    registry
        .register("majority", "version-1", move |b: &[bool]| {
            calls.fetch_add(1, Ordering::SeqCst);
            if fail {
                Err("test decoder failure".to_owned())
            } else {
                Ok(vec![b.iter().filter(|&&x| x).count() >= 2])
            }
        })
        .unwrap();
    let mut options = FileTaskOptions::default();
    options.evaluation.runtime.strategy = strategy;
    loaded(
        "X_ERROR(1/3) 3\nH 0 1 2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[majority] det[0] det[1] det[2] -> ctrl[0]\nCX ctrl[0] 3\nM 3\nOBSERVABLE_INCLUDE(0) rec[-1]\nPOSTSELECT det[0]",
        r#"{"error_observables":[0]}"#,
        Path::new("/tmp"),
        &registry,
        options,
        NoiseSampling::Physical,
    )
}
#[test]
fn nonlinear_file_integration_preserves_all_histories_and_actual_cold_replays() {
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        let calls = Arc::new(AtomicUsize::new(0));
        let file = nonlinear(strategy, &calls, false);
        let report = file.enumerate(&FileEnumerationOptions::default()).unwrap();
        let raw = report.raw_result().unwrap();
        assert_eq!(raw.masses.accepted, field(1, 2));
        assert_eq!(raw.masses.error, field(5, 24));
        assert_eq!(raw.histories, 16);
        assert_eq!(calls.load(Ordering::SeqCst), 16);
        count(report.execution_costs().decoder, (16, 16, 0));
        count(report.execution_costs().query, (80, 80, 0));
        let compilations = if strategy == CausalStrategy::FixedTemplate {
            1
        } else {
            35
        };
        count(
            report.execution_costs().compilation,
            (compilations, compilations, 0),
        );
        let json = report.to_json().unwrap();
        assert_eq!(json["status"], "complete");
        exact(&json["result"]["masses"]["error"], 5, 24);
    }
}
#[test]
fn failures_preserve_costs_and_omit_partial_exact_estimates() {
    let file = simple(FileTaskOptions::default());
    let compiled = file.compile().unwrap();
    let options = FileEnumerationOptions {
        execution_limits: ExecutionLimits {
            max_queries: 1,
            max_duration: None,
        },
        ..Default::default()
    };
    for report in [
        file.enumerate(&options).unwrap(),
        compiled.enumerate(&options).unwrap(),
    ] {
        assert!(!report.is_complete());
        assert!(
            report
                .raw_result()
                .unwrap_err()
                .to_string()
                .contains("max_queries")
        );
        count(report.execution_costs().query, (1, 1, 0));
        let json = report.to_json().unwrap();
        assert_eq!(json["status"], "failed");
        assert_eq!(json["failure_phase"], "enumeration");
        assert!(json["result"].is_null());
        assert!(!json["fingerprints"]["circuit"].is_null());
    }
    let expired = FileEnumerationOptions {
        execution_limits: ExecutionLimits {
            max_queries: u64::MAX,
            max_duration: Some(Duration::ZERO),
        },
        ..Default::default()
    };
    let report = file.enumerate(&expired).unwrap();
    assert_eq!(report.execution_costs(), Default::default());
    let json = report.to_json().unwrap();
    assert_eq!(json["failure_phase"], "preparation");
    assert!(json["result"].is_null());
    let again = compiled
        .enumerate(&FileEnumerationOptions::default())
        .unwrap();
    assert!(again.raw_result().is_ok());
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        let calls = Arc::new(AtomicUsize::new(0));
        let file = nonlinear(strategy, &calls, true);
        let report = file.enumerate(&FileEnumerationOptions::default()).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        count(report.execution_costs().decoder, (1, 0, 1));
        let json = report.to_json().unwrap();
        assert_eq!(json["failure_phase"], "enumeration");
        assert!(json["result"].is_null());
        assert!(
            json["failure"]["message"]
                .as_str()
                .unwrap()
                .contains("test decoder failure")
        );
    }
}
#[test]
fn undefined_metrics_zero_acceptance_and_numeric_failure_are_explicit() {
    let registry = ControllerRegistry::default();
    for (source, task, defined) in [
        ("I 0", "{}", false),
        (
            "X 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]\nOBSERVABLE_INCLUDE(0) rec[-1]",
            r#"{"error_observables":[0]}"#,
            true,
        ),
    ] {
        let file = loaded(
            source,
            task,
            Path::new("/tmp"),
            &registry,
            FileTaskOptions::default(),
            NoiseSampling::Physical,
        );
        let report = file.enumerate(&FileEnumerationOptions::default()).unwrap();
        let json = report.to_json().unwrap();
        assert_eq!(json["result"]["error_defined"], defined);
        assert!(json["result"]["conditional_error"].is_null());
        if defined {
            exact(&json["result"]["masses"]["accepted"], 0, 1);
            exact(&json["result"]["masses"]["error"], 0, 1);
        } else {
            assert!(json["result"]["masses"]["error"].is_null());
            assert!(json["result"]["full_error"].is_null());
        }
    }
    let mut options = FileTaskOptions::default();
    options.numeric.max_work_bits = 0;
    let report = simple(options)
        .enumerate(&FileEnumerationOptions::default())
        .unwrap();
    assert!(report.to_json().is_err());
    assert!(report.is_complete());
    assert!(report.total_seconds().is_finite() && report.total_seconds() >= 0.0);
    assert_eq!(report.raw_result().unwrap().masses.error, field(2, 5));
    assert_eq!(report.metadata()["kind"], "enumeration");
    count(report.execution_costs().query, (6, 6, 0));
}
#[test]
fn incompatible_requests_fail_before_quantum_work_or_callbacks() {
    let mut registry = ControllerRegistry::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    registry
        .register("copy", "1", move |bits: &[bool]| {
            observed.fetch_add(1, Ordering::SeqCst);
            Ok(bits.to_vec())
        })
        .unwrap();
    for (task, sampling, expected) in [
        (
            "{}",
            NoiseSampling::Importance {
                proposal_scale: BigRational::from_integer(1.into()),
            },
            "enumeration.sampling",
        ),
        (
            r#"{"sample_records":[0]}"#,
            NoiseSampling::Physical,
            "enumeration.sample_records",
        ),
    ] {
        let file = loaded(
            "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCX ctrl[0] 1",
            task,
            Path::new("/tmp"),
            &registry,
            FileTaskOptions::default(),
            sampling,
        );
        let error = file
            .enumerate(&FileEnumerationOptions::default())
            .unwrap_err();
        assert!(error.to_string().contains(expected));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn scientific_exports_preserve_irrational_coefficients_and_tiny_positive_error() {
    let registry = ControllerRegistry::default();
    let file = loaded(
        "H 0\nT 0\nH 0",
        r#"{"observables":[[0,1]]}"#,
        Path::new("/tmp"),
        &registry,
        FileTaskOptions::default(),
        NoiseSampling::Physical,
    );
    let value = file
        .enumerate(&FileEnumerationOptions::default())
        .unwrap()
        .to_json()
        .unwrap();
    let moment = &value["result"]["masses"]["moments"][0];
    assert!((moment["value"].as_f64().unwrap() - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-15);
    assert_eq!(
        moment["exact"]["coefficients"],
        json!([
            {"numerator":"0","denominator":"1"}, {"numerator":"1","denominator":"2"},
            {"numerator":"0","denominator":"1"}, {"numerator":"-1","denominator":"2"}
        ])
    );
    let denominator = (num_bigint::BigUint::from(1_u8) << 1200_usize).to_string();
    let file = loaded(
        &format!("X_ERROR(1/{denominator}) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]"),
        r#"{"error_observables":[0]}"#,
        Path::new("/tmp"),
        &registry,
        FileTaskOptions::default(),
        NoiseSampling::Physical,
    );
    let report = file.enumerate(&FileEnumerationOptions::default()).unwrap();
    assert!(!report.raw_result().unwrap().masses.error.is_zero());
    let value = report.to_json().unwrap();
    let error = &value["result"]["masses"]["error"];
    assert!(error["value"].is_null());
    assert_eq!(
        error["scientific"],
        json!({"mantissa":0.5,"exponent2":-1199})
    );
    assert_eq!(
        error["exact"]["coefficients"][0],
        json!({"numerator":"1","denominator":denominator})
    );
    assert!(
        value["numerics"]["underflow_fields"]
            .as_array()
            .unwrap()
            .contains(&json!("result.masses.error"))
    );
    assert_eq!(value["status"], "complete");
}
