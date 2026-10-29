//! Pinned itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
//! test_low_weight.py: report noise and postselection contribute jointly.
use num_bigint::BigInt;
use num_rational::BigRational;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::file_api::{FileEnumerationOptions, FileLoadOptions, FileTask, FileTaskOptions};
use rmagna::noise::NoiseSampling;
use rmagna::runtime::ControllerRegistry;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

const POINT: &str = "conditional_error_within_enumerated_sector";
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-sector-conditional-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("target.magna"), "I 0\n").unwrap();
        Self(path)
    }
    fn load(&self, source: &str, task: &str) -> FileTask {
        std::fs::write(self.0.join("physical.magna"), source).unwrap();
        std::fs::write(self.0.join("task.json"), task).unwrap();
        FileTask::load(
            &self.0.join("physical.magna"),
            Some(&self.0.join("task.json")),
            &ControllerRegistry::default(),
            NoiseSampling::Physical,
            FileTaskOptions::default(),
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
fn field(n: i64, d: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(BigRational::new(n.into(), d.into()))
        .unwrap()
}
fn exact(value: &Value, n: &str, d: &str) {
    assert_eq!(
        value["exact"]["coefficients"],
        json!([
            {"numerator":n,"denominator":d},
            {"numerator":"0","denominator":"1"},
            {"numerator":"0","denominator":"1"},
            {"numerator":"0","denominator":"1"}
        ])
    );
}
fn full() -> FileEnumerationOptions {
    let mut options = FileEnumerationOptions::default();
    options.enumeration.noise.max_weight = 2;
    options
}
fn original_json(value: &Value) {
    assert_eq!(value["status"], "complete");
    let result = &value["result"];
    exact(&result["masses"]["accepted"], "37", "50");
    exact(&result["masses"]["error"], "1", "50");
    exact(&result["masses"]["moments"][0], "7", "10");
    exact(&result[POINT], "1", "37");
    assert_eq!(result[POINT]["value"], json!(1.0 / 37.0));
    assert_eq!(
        result[POINT]["scientific"],
        json!({"mantissa":32.0/37.0,"exponent2":-5})
    );
    exact(&result["conditional_error"]["lower"], "1", "37");
    exact(&result["conditional_error"]["upper"], "1", "37");
}

#[test]
fn original_report_noise_postselection_retains_exact_covered_conditional_point() {
    let files = Files::new();
    let file = files.load(
        "X_ERROR(1/10) 0\nM(1/5) 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]",
        r#"{"observables":[[0,1]],"ideal_state":"target.magna"}"#,
    );
    // Accepted branches: no physical/readout error (36/50), or both (1/50).
    // Only the latter is a target error; their signed Z moment is 35/50.
    let accepted = BigRational::new(36.into(), 50.into()) + BigRational::new(1.into(), 50.into());
    assert_eq!(accepted, BigRational::new(37.into(), 50.into()));
    let compiled = file.compile().unwrap();
    let fresh = file.enumerate(&full()).unwrap();
    let first = compiled.enumerate(&full()).unwrap();
    let reused = compiled.enumerate(&full()).unwrap();
    assert_eq!(fresh.raw_result(), first.raw_result());
    assert_eq!(first.raw_result(), reused.raw_result());
    for report in [&fresh, &first, &reused] {
        let raw = report.raw_result().unwrap();
        assert_eq!(raw.covered_probability.to_string(), "1");
        assert_eq!(raw.omitted_probability.to_string(), "0");
        assert_eq!(raw.patterns, 4);
        assert_eq!(raw.masses.accepted, field(37, 50));
        assert_eq!(raw.masses.error, field(1, 50));
        assert_eq!(raw.masses.target_success, Some(field(18, 25)));
        assert_eq!(raw.masses.moments, [field(7, 10)]);
        assert_eq!(
            raw.conditional_error_within_enumerated_sector,
            Some(field(1, 37))
        );
        original_json(&report.to_json().unwrap());
    }
    let out = Command::new(env!("CARGO_BIN_EXE_rmagna"))
        .current_dir(&files.0)
        .args([
            "enumerate",
            "physical.magna",
            "--task",
            "task.json",
            "--max-weight",
            "2",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    original_json(&serde_json::from_slice::<Value>(&out.stdout).unwrap());
}

#[test]
fn truncated_sector_point_preserves_masses_and_full_distribution_bounds() {
    let files = Files::new();
    let file = files.load(
        "X_ERROR(1/10) 0\nX_ERROR(1/5) 0",
        r#"{"ideal_state":"target.magna"}"#,
    );
    let report = file.enumerate(&FileEnumerationOptions::default()).unwrap();
    let raw = report.raw_result().unwrap();
    // The omitted double-X has probability 1/50; the two single errors sum
    // to 4/50 + 9/50. Conditioning within the covered 49/50 gives 13/49.
    assert_eq!(raw.covered_probability.to_string(), "49/50");
    assert_eq!(raw.omitted_probability.to_string(), "1/50");
    assert_eq!(raw.masses.accepted, field(49, 50));
    assert_eq!(raw.masses.error, field(13, 50));
    assert_eq!(
        raw.conditional_error_within_enumerated_sector,
        Some(field(13, 49))
    );
    let bounds = raw.conditional_error.as_ref().unwrap();
    assert_eq!(bounds.lower, field(13, 50));
    assert_eq!(bounds.upper, field(7, 25));
    assert_ne!(
        raw.conditional_error_within_enumerated_sector.as_ref(),
        Some(&bounds.lower)
    );
    assert_ne!(
        raw.conditional_error_within_enumerated_sector.as_ref(),
        Some(&bounds.upper)
    );
    let value = report.to_json().unwrap();
    exact(&value["result"][POINT], "13", "49");
    exact(&value["result"]["masses"]["error"], "13", "50");
    exact(&value["result"]["conditional_error"]["lower"], "13", "50");
    exact(&value["result"]["conditional_error"]["upper"], "7", "25");
}

#[test]
fn undefined_zero_acceptance_and_defined_zero_error_are_distinct() {
    for (source, task, weight, expected) in [
        ("I 0", "{}", 0, None),
        (
            "I 0",
            r#"{"ideal_state":"target.magna"}"#,
            0,
            Some(field(0, 1)),
        ),
        (
            "X 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]",
            r#"{"ideal_state":"target.magna"}"#,
            0,
            None,
        ),
        ("X_ERROR(1) 0", r#"{"ideal_state":"target.magna"}"#, 0, None),
    ] {
        let files = Files::new();
        let file = files.load(source, task);
        let mut options = full();
        options.enumeration.noise.max_weight = weight;
        let report = file.enumerate(&options).unwrap();
        let raw = report.raw_result().unwrap();
        assert_eq!(raw.conditional_error_within_enumerated_sector, expected);
        let value = report.to_json().unwrap();
        if expected.is_some() {
            assert_eq!(raw.masses.accepted, ExactField::one());
            exact(&value["result"][POINT], "0", "1");
        } else {
            assert!(value["result"][POINT].is_null());
        }
        if source == "X_ERROR(1) 0" {
            assert_eq!(raw.masses.accepted, ExactField::zero());
            let bounds = raw.conditional_error.as_ref().unwrap();
            assert_eq!(bounds.lower, ExactField::zero());
            assert_eq!(bounds.upper, ExactField::one());
        }
    }
}

#[test]
fn observable_only_error_metric_retains_tiny_nonzero_exact_and_scientific_point() {
    for denominator in [BigInt::from(3), BigInt::from(1) << 1200] {
        let files = Files::new();
        let file = files.load(
            &format!("X_ERROR(1/{denominator}) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]"),
            r#"{"error_observables":[0]}"#,
        );
        let report = file.enumerate(&full()).unwrap();
        let expected = FieldArithmetic::default()
            .from_rational(BigRational::new(1.into(), denominator.clone()))
            .unwrap();
        let raw = report.raw_result().unwrap();
        assert_eq!(raw.masses.accepted, ExactField::one());
        assert_eq!(raw.masses.error, expected);
        assert_eq!(raw.masses.target_success, None);
        assert_eq!(
            raw.conditional_error_within_enumerated_sector,
            Some(expected.clone())
        );
        let value = report.to_json().unwrap();
        let point = &value["result"][POINT];
        exact(point, "1", &denominator.to_string());
        if denominator == BigInt::from(3) {
            assert_eq!(point["value"], json!(1.0 / 3.0));
        } else {
            assert!(point["value"].is_null());
            assert_eq!(
                point["scientific"],
                json!({"mantissa":0.5,"exponent2":-1199})
            );
            assert_ne!(expected, ExactField::zero());
        }
        assert_eq!(
            report
                .raw_result()
                .unwrap()
                .conditional_error_within_enumerated_sector,
            Some(expected)
        );
    }
}
