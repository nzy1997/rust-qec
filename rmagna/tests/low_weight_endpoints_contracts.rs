//! Complete endpoint and invalid-budget/CLI contracts from pinned
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
//! tests/core/test_low_weight.py. CLI text is the dynamic type boundary;
//! Rust's usize cutoff cannot represent negative, Boolean, fractional or null values.
use rmagna::field::ExactField;
use rmagna::file_api::{FileEnumerationOptions, FileLoadOptions, FileTask, FileTaskOptions};
use rmagna::noise::NoiseSampling;
use rmagna::probability::Probability;
use rmagna::runtime::{CausalStrategy, ControllerRegistry};
use rmagna::unitary::CompilerStrategy;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-weight-endpoints-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        let files = Self(path);
        files.put("target.magna", "I 0");
        files.put("task.json", r#"{"ideal_state":"target.magna"}"#);
        files
    }
    fn put(&self, name: &str, source: &str) {
        std::fs::write(self.0.join(name), source).unwrap();
    }
    fn load(&self, source: &str, compiler: CompilerStrategy, causal: CausalStrategy) -> FileTask {
        self.put("c.magna", source);
        let mut options = FileTaskOptions::default();
        options.evaluation.runtime.compile.compiler_strategy = compiler;
        options.evaluation.runtime.strategy = causal;
        FileTask::load(
            &self.0.join("c.magna"),
            Some(&self.0.join("task.json")),
            &ControllerRegistry::default(),
            NoiseSampling::Physical,
            options,
            &FileLoadOptions::default(),
            BTreeMap::new(),
        )
        .unwrap()
    }
    fn cli(&self, args: &[&str], expected: i32) -> Value {
        let out = Command::new(env!("CARGO_BIN_EXE_rmagna"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(expected),
            "args={args:?}\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn options(weight: usize) -> FileEnumerationOptions {
    let mut options = FileEnumerationOptions::default();
    options.enumeration.noise.max_weight = weight;
    options
}

#[test]
fn original_zero_forced_and_empty_sectors_preserve_exact_endpoints_and_large_cutoffs() {
    let files = Files::new();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for causal in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            for source in ["I 0", "X_ERROR(0) 0"] {
                let file = files.load(source, compiler, causal);
                let prepared = file.compile().unwrap();
                let fresh = file.enumerate(&options(0)).unwrap();
                let reused = prepared.enumerate(&options(0)).unwrap();
                assert!(fresh.is_complete() && reused.is_complete());
                assert_eq!(fresh.raw_result(), reused.raw_result());
                let raw = fresh.raw_result().unwrap();
                assert_eq!(raw.patterns, 1);
                assert_eq!(raw.covered_probability, Probability::one());
                assert_eq!(raw.omitted_probability, Probability::zero());
                assert_eq!(raw.masses.error, ExactField::zero());
                assert_eq!(raw.masses.accepted, ExactField::one());
                assert_eq!(raw.masses.target_success, Some(ExactField::one()));
                assert_eq!(
                    fresh.to_json().unwrap()["result"]["masses"]["error"]["value"],
                    0.0
                );
            }
            let file = files.load("X_ERROR(1) 0", compiler, causal);
            let prepared = file.compile().unwrap();
            let empty = file.enumerate(&options(0)).unwrap();
            assert!(empty.is_complete());
            assert_eq!(
                empty.raw_result(),
                prepared.enumerate(&options(0)).unwrap().raw_result()
            );
            let raw = empty.raw_result().unwrap();
            assert_eq!(raw.patterns, 0);
            assert_eq!(raw.covered_probability, Probability::zero());
            assert_eq!(raw.omitted_probability, Probability::one());
            assert_eq!(raw.masses.accepted, ExactField::zero());
            assert_eq!(raw.masses.error, ExactField::zero());
            assert_eq!(raw.conditional_error_within_enumerated_sector, None);
            let bounds = raw.conditional_error.as_ref().unwrap();
            assert_eq!(bounds.lower, ExactField::zero());
            assert_eq!(bounds.upper, ExactField::one());
            assert!(
                empty.to_json().unwrap()["result"]["conditional_error_within_enumerated_sector"]
                    .is_null()
            );
            for weight in [1, usize::MAX] {
                let full = file.enumerate(&options(weight)).unwrap();
                assert!(full.is_complete());
                assert_eq!(
                    full.raw_result(),
                    prepared.enumerate(&options(weight)).unwrap().raw_result()
                );
                let raw = full.raw_result().unwrap();
                assert_eq!(raw.patterns, 1);
                assert_eq!(raw.covered_probability, Probability::one());
                assert_eq!(raw.omitted_probability, Probability::zero());
                assert_eq!(raw.masses.error, ExactField::one());
                assert_eq!(raw.masses.accepted, ExactField::one());
                assert_eq!(raw.masses.target_success, Some(ExactField::zero()));
            }
        }
    }
    // The literal native 1<<64 cutoff reaches the CLI unchanged even on
    // platforms whose typed API cannot represent it. No huge sector allocation.
    files.put("c.magna", "X_ERROR(1) 0");
    let maximum = usize::MAX.to_string();
    let beyond = (num_bigint::BigUint::from(usize::MAX) + 1_u8).to_string();
    let huge = "9".repeat(4096);
    let mut ceiling_fingerprint = None;
    for cutoff in [
        "1",
        "+0001",
        &maximum,
        &beyond,
        "18446744073709551616",
        &huge,
    ] {
        let value = files.cli(
            &[
                "enumerate",
                "c.magna",
                "--task",
                "task.json",
                "--max-weight",
                cutoff,
                "--json",
            ],
            0,
        );
        assert_eq!(value["status"], "complete");
        assert_eq!(value["result"]["patterns"], 1);
        assert_eq!(value["result"]["masses"]["error"]["value"], 1.0);
        assert_eq!(
            value["result"]["masses"]["error"]["exact"]["coefficients"][0],
            json!({"numerator":"1","denominator":"1"})
        );
        let effective = if cutoff == "1" || cutoff == "+0001" {
            1
        } else {
            usize::MAX
        };
        assert_eq!(
            value["options"]["enumeration"]["noise"]["max_weight"],
            effective
        );
        if effective == usize::MAX {
            let fingerprint = value["fingerprints"]["options"]
                .as_str()
                .unwrap()
                .to_owned();
            if let Some(previous) = &ceiling_fingerprint {
                assert_eq!(previous, &fingerprint);
            }
            ceiling_fingerprint = Some(fingerprint);
        }
    }
    // A full finite ceiling must still respect every relevant storage budget.
    files.put("c.magna", "X_ERROR(1/10) 0\nX_ERROR(1/10) 0");
    for (flag, limit) in [
        ("--max-patterns", "3"),
        ("--max-enumeration-sites", "1"),
        ("--max-fault-entries", "0"),
    ] {
        let failed = files.cli(
            &[
                "enumerate",
                "c.magna",
                "--task",
                "task.json",
                "--max-weight",
                &huge,
                flag,
                limit,
                "--json",
            ],
            1,
        );
        assert_eq!(failed["status"], "failed");
        assert_eq!(failed["failure_phase"], "enumeration");
        assert!(failed["result"].is_null());
    }
}

#[test]
fn original_invalid_weight_types_reject_before_loading_and_literal_cli_has_tenth_error() {
    let files = Files::new();
    files.put("decoder.py", "from pathlib import Path\nPath('imported').touch()\ndef decode(bits,config): return bits\n");
    files.put("controllers.json", r#"{"schema_version":1,"bindings":{"copy":{"import":"decoder:decode","version":"endpoint-v1","config":{}}}}"#);
    let huge_suffix = format!("{}x", "9".repeat(4096));
    for cutoff in [
        "-1",
        "true",
        "false",
        "1.5",
        "null",
        "None",
        "",
        "+",
        " 1",
        "1 ",
        "1e2",
        "١",
        &huge_suffix,
    ] {
        let value = files.cli(
            &[
                "enumerate",
                "missing.magna",
                "--task",
                "missing-task.json",
                "--controllers",
                "controllers.json",
                "--max-weight",
                cutoff,
                "--json",
                "--output",
                "invalid.json",
            ],
            2,
        );
        assert!(
            value["failures"][0]["message"]
                .as_str()
                .unwrap()
                .contains("max-weight")
        );
        assert!(!files.0.join("imported").exists());
        assert!(!files.0.join("invalid.json").exists());
    }
    let missing = files.cli(
        &[
            "enumerate",
            "missing.magna",
            "--controllers",
            "controllers.json",
            "--max-weight",
            "--json",
            "--output",
            "invalid.json",
        ],
        2,
    );
    assert!(
        missing["failures"][0]["message"]
            .as_str()
            .unwrap()
            .contains("max-weight")
    );
    assert!(!files.0.join("imported").exists());
    assert!(!files.0.join("invalid.json").exists());
    // Native ZERO_TARGET is represented by an owned I0 target, and outputs[0]
    // is literal: the final measurement must not erase physical target error.
    files.put("c.magna", "X_ERROR(1/10) 0\nM 0");
    files.put(
        "task.json",
        r#"{"outputs":[0],"ideal_state":"target.magna"}"#,
    );
    let value = files.cli(
        &[
            "enumerate",
            "c.magna",
            "--task",
            "task.json",
            "--max-weight",
            "1",
            "--json",
        ],
        0,
    );
    assert_eq!(value["status"], "complete");
    assert_eq!(value["result"]["masses"]["error"]["value"], 0.1);
    assert_eq!(
        value["result"]["masses"]["error"]["exact"]["coefficients"][0],
        json!({"numerator":"1","denominator":"10"})
    );
    assert_eq!(value["result"]["masses"]["accepted"]["value"], 1.0);
    assert_eq!(value["options"]["enumeration"]["noise"]["max_weight"], 1);
    assert!(!files.0.join("imported").exists());
}
