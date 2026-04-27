use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn run_parity_driver(crate_root: &PathBuf, args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--example")
        .arg("parity_driver")
        .arg("--")
        .args(args)
        .current_dir(crate_root)
        .output()
        .unwrap()
}

#[test]
fn parity_driver_runs_case_and_prints_json_report() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let case_path = std::env::temp_dir().join(format!("rbposd-parity-case-{nanos}.json"));
    let case_json = r#"{
  "name": "temp_osd_case",
  "matrix": {
    "num_checks": 2,
    "num_bits": 3,
    "rows": [[0, 1], [1, 2]]
  },
  "channel": {
    "kind": "bit_flip_probabilities",
    "probabilities": [0.1, 0.2, 0.3]
  },
  "syndrome": [true, false],
  "config": {
    "max_bp_iterations": 0,
    "early_stop": true,
    "bp_variant": "minimum_sum",
    "schedule": "parallel",
    "osd_variant": "osd0"
  },
  "tags": ["temp", "smoke"]
}"#;
    fs::write(&case_path, case_json).unwrap();

    let case_arg = case_path.to_str().unwrap().to_string();
    let output = run_parity_driver(&crate_root, &[case_arg.as_str()]);

    let _ = fs::remove_file(&case_path);

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    let status_code = output.status.code();
    assert!(
        output.status.success(),
        "parity_driver failed\nstatus={:?}\nstdout=\n{}\nstderr=\n{}",
        status_code,
        stdout,
        stderr
    );
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["name"], "temp_osd_case");
    assert_eq!(report["actual"]["status"], "success");
    assert_eq!(
        report["actual"]["correction"],
        serde_json::json!([true, false, false])
    );
    let matches_expected = report["matches_expected"].clone();
    assert!(
        matches_expected.is_null() || matches_expected == true,
        "unexpected matches_expected value: {}",
        matches_expected
    );
}

#[test]
fn parity_driver_fails_with_usage_when_case_path_missing() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = run_parity_driver(&crate_root, &[]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(
        !output.status.success(),
        "expected failure for missing arg\nstdout=\n{}\nstderr=\n{}",
        stdout,
        stderr
    );
    assert!(stdout.trim().is_empty(), "expected empty stdout, got:\n{}", stdout);
    assert!(stderr.contains("usage:"), "stderr missing usage:\n{}", stderr);
    assert!(
        stderr.contains("<parity-case.json>"),
        "stderr missing arg hint:\n{}",
        stderr
    );
}

#[test]
fn parity_driver_fails_cleanly_on_invalid_json_case() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let case_path = std::env::temp_dir().join(format!("rbposd-parity-case-bad-{nanos}.json"));
    fs::write(&case_path, "{ this is not valid json").unwrap();

    let case_arg = case_path.to_str().unwrap().to_string();
    let output = run_parity_driver(&crate_root, &[case_arg.as_str()]);
    let _ = fs::remove_file(&case_path);

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        !output.status.success(),
        "expected failure for invalid json case\nstdout=\n{}\nstderr=\n{}",
        stdout,
        stderr
    );
    assert!(stdout.trim().is_empty(), "expected empty stdout, got:\n{}", stdout);
    assert!(
        stderr.contains("failed to parse"),
        "stderr should explain parse failure:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("panicked"),
        "invalid json should not panic; stderr=\n{}",
        stderr
    );
}
