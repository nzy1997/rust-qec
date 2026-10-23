//! Independent projected-target normalization witnesses and normalized responses.
use num_rational::BigRational;
use rmagna::circuit::parse_circuit;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::file_api::{FileEnumerationOptions, FileTask, FileTaskOptions};
use rmagna::noise::NoiseSampling;
use rmagna::runtime::ControllerRegistry;
use rmagna::task::{TaskOptions, parse_task};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-enum-target-norm-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
    fn task(&self, source: &str, task: &str, options: FileTaskOptions) -> FileTask {
        let physical = parse_circuit(source, &Default::default()).unwrap();
        let modules = parse_task(task, &self.0, &TaskOptions::default())
            .unwrap()
            .load_modules(&physical, &Default::default())
            .unwrap();
        FileTask::from_loaded(
            physical,
            modules,
            &ControllerRegistry::default(),
            NoiseSampling::Physical,
            options,
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
fn check(value: &Value, coefficients: [i64; 4], power: i64) {
    assert_eq!(
        value["coefficients"],
        json!(coefficients.map(|c| c.to_string()))
    );
    assert_eq!(value["power"], power);
    assert_eq!(value["scale"], "2**power");
}
const PROJECTED: &str = r#"{"outputs":[0],"ideal_state":{"circuit":"target.magna","outputs":[1],"postselection":{"0":0}}}"#;

#[test]
fn projected_norms_survive_deleted_sources_and_do_not_add_quantum_queries_or_rescale_masses() {
    let files = Files::new();
    for (target, coefficients, power) in [
        ("H 0\nCX 0 1", [1, 0, 0, 0], -1),
        ("H 0\nT 0\nH 0\nI 1", [2, 1, 0, -1], -2),
    ] {
        // Projecting Bell's first qubit gives norm1/2. Projecting HTH's
        // first qubit gives |(1+omega)/2|^2=(2+sqrt(2))/4. In each case
        // the selected output qubit is exactly zero after normalization.
        files.put("target.magna", target);
        let file = files.task("X_ERROR(1/5) 0", PROJECTED, FileTaskOptions::default());
        let forced = files.task("X_ERROR(1) 0", PROJECTED, FileTaskOptions::default());
        let mut invalid_numeric = FileTaskOptions::default();
        invalid_numeric.numeric.max_work_bits = 0;
        let numeric_failure = files.task("X_ERROR(1/5) 0", PROJECTED, invalid_numeric);
        let compiled = file.compile().unwrap();
        std::fs::remove_file(files.0.join("target.magna")).unwrap();
        let fresh = file.enumerate(&FileEnumerationOptions::default()).unwrap();
        let reused = compiled
            .enumerate(&FileEnumerationOptions::default())
            .unwrap();
        assert_eq!(fresh.raw_result(), reused.raw_result());
        assert_eq!(fresh.execution_costs().query.attempted, 5);
        assert_eq!(reused.execution_costs().query.attempted, 4);
        for report in [&fresh, &reused] {
            assert!(report.is_complete());
            let raw = report.raw_result().unwrap();
            let norm = raw.target_normalization.as_ref().unwrap();
            assert_eq!(
                norm.coefficients().each_ref().map(ToString::to_string),
                coefficients.map(|c| c.to_string())
            );
            assert_eq!(norm.power(), power);
            assert_eq!(raw.masses.accepted, ExactField::one());
            assert_eq!(raw.masses.error, field(1, 5));
            assert_eq!(raw.masses.target_success, Some(field(4, 5)));
            check(
                &report.to_json().unwrap()["result"]["target_normalization"],
                coefficients,
                power,
            );
        }
        let mut empty = FileEnumerationOptions::default();
        empty.enumeration.noise.max_weight = 0;
        let report = forced.enumerate(&empty).unwrap();
        assert_eq!(report.raw_result().unwrap().patterns, 0);
        assert_eq!(
            report.raw_result().unwrap().target_normalization,
            fresh.raw_result().unwrap().target_normalization
        );
        check(
            &report.to_json().unwrap()["result"]["target_normalization"],
            coefficients,
            power,
        );
        let failed = numeric_failure
            .enumerate(&FileEnumerationOptions::default())
            .unwrap();
        assert!(failed.to_json().is_err());
        assert_eq!(
            failed.raw_result().unwrap().target_normalization,
            fresh.raw_result().unwrap().target_normalization
        );
        assert_eq!(failed.raw_result().unwrap().masses.error, field(1, 5));
    }
}
#[test]
fn unit_and_absent_norms_use_the_compact_binary_exponent_convention() {
    let files = Files::new();
    files.put("target.magna", "I 0");
    for (task, present) in [(r#"{"ideal_state":"target.magna"}"#, true), ("{}", false)] {
        let report = files
            .task("I 0", task, FileTaskOptions::default())
            .enumerate(&FileEnumerationOptions::default())
            .unwrap();
        let json = report.to_json().unwrap();
        if present {
            assert_eq!(
                report.raw_result().unwrap().target_normalization,
                Some(rmagna::scalar::ExactScalar::one())
            );
            check(&json["result"]["target_normalization"], [1, 0, 0, 0], 0);
        } else {
            assert_eq!(report.raw_result().unwrap().target_normalization, None);
            assert_eq!(json["result"]["target_normalization"], Value::Null);
        }
    }
}
