use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn parity_driver_runs_case_and_prints_json_report() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let case_path = std::env::temp_dir().join(format!("rbposd-parity-case-{nanos}.json"));
    let case_json = r#"{
  "name": "bp_repetition_single_flip",
  "matrix": {
    "num_checks": 4,
    "num_bits": 5,
    "rows": [[0, 1], [1, 2], [2, 3], [3, 4]]
  },
  "channel": {
    "kind": "bsc",
    "error_rate": 0.05
  },
  "syndrome": [true, false, false, false],
  "config": {
    "max_bp_iterations": 30,
    "early_stop": true,
    "bp_variant": "minimum_sum",
    "schedule": "parallel",
    "osd_variant": "osd0"
  },
  "expected": {
    "status": "success",
    "correction": [true, false, false, false, false],
    "diagnostics": {
      "converged": true,
      "bp_iterations": 2,
      "used_osd": false,
      "residual_syndrome_weight": 0
    }
  },
  "tags": ["static-baseline", "bp-only"]
}"#;
    fs::write(&case_path, case_json).unwrap();

    let output = Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--example")
        .arg("parity_driver")
        .arg("--")
        .arg(&case_path)
        .current_dir(&crate_root)
        .output()
        .unwrap();

    let _ = fs::remove_file(&case_path);

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        output.status.success(),
        "parity_driver failed\nstatus={:?}\nstdout=\n{}\nstderr=\n{}",
        output.status.code(),
        stdout,
        stderr
    );
    assert!(stdout.contains("\"name\": \"bp_repetition_single_flip\""));
    assert!(stdout.contains("\"status\": \"success\""));
    assert!(
        stdout.contains(
            "\"correction\": [\n      true,\n      false,\n      false,\n      false,\n      false\n    ]"
        ),
        "stdout did not include expected correction array:\n{}",
        stdout
    );
}
