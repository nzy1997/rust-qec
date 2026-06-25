use std::path::PathBuf;
use std::process::{Command, Output};

fn qec_code_bin() -> &'static str {
    env!("CARGO_BIN_EXE_qec-code")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn run_quantum_tanner(matrix: &str, fixture: &str) -> Output {
    let spec = workspace_root().join(fixture);
    Command::new(qec_code_bin())
        .args(["code", "css", "quantum-tanner", "--spec"])
        .arg(&spec)
        .arg(matrix)
        .output()
        .expect("qec-code binary should run")
}

fn assert_quantum_tanner_sparse_rows_output(stdout: &[u8]) -> serde_json::Value {
    let stdout = String::from_utf8(stdout.to_vec()).expect("stdout should be valid utf-8");
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be sparse-row JSON");
    let rows = json["rows"]
        .as_array()
        .expect("sparse-row JSON should contain rows");

    assert_eq!(json["format"], "sparse_rows");
    assert_eq!(json["num_cols"], 16);
    assert_eq!(rows.len(), 8);
    assert!(
        rows.iter().all(|row| {
            row.as_array()
                .is_some_and(|cols| cols.is_empty() || cols.len() == 4)
        }),
        "all non-empty quantum Tanner rows should have weight 4: {rows:?}"
    );

    json
}

#[test]
fn code_css_quantum_tanner_hx_prints_sparse_rows_json() {
    let output = run_quantum_tanner("hx", "qec-code/tests/fixtures/quantum_tanner/toric_d4.json");

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");
    assert_quantum_tanner_sparse_rows_output(&output.stdout);
}

#[test]
fn code_css_quantum_tanner_hz_prints_sparse_rows_json() {
    let output = run_quantum_tanner("hz", "qec-code/tests/fixtures/quantum_tanner/toric_d4.json");

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");
    assert_quantum_tanner_sparse_rows_output(&output.stdout);
}

#[test]
fn code_css_quantum_tanner_hx_and_hz_are_exported_separately() {
    let hx = run_quantum_tanner("hx", "qec-code/tests/fixtures/quantum_tanner/toric_d4.json");
    let hz = run_quantum_tanner("hz", "qec-code/tests/fixtures/quantum_tanner/toric_d4.json");

    assert!(hx.status.success());
    assert!(hz.status.success());

    let hx_json = assert_quantum_tanner_sparse_rows_output(&hx.stdout);
    let hz_json = assert_quantum_tanner_sparse_rows_output(&hz.stdout);
    assert_ne!(hx_json, hz_json);
}

#[test]
fn code_css_quantum_tanner_invalid_spec_fails_without_stdout() {
    let output = run_quantum_tanner(
        "hx",
        "qec-code/tests/fixtures/quantum_tanner/invalid_non_symmetric_a.json",
    );

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid utf-8");
    assert!(
        stderr.contains("invalid quantum Tanner generator set A"),
        "stderr was: {stderr}"
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(&stderr).is_err(),
        "stderr should not be valid sparse-row JSON: {stderr}"
    );
}

#[test]
fn code_css_quantum_tanner_missing_spec_fails_without_stdout() {
    let output = run_quantum_tanner("hx", "qec-code/tests/fixtures/quantum_tanner/missing.json");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid utf-8");
    assert!(
        stderr.contains("failed to read CSS matrix"),
        "stderr was: {stderr}"
    );
    assert!(stderr.contains("missing.json"), "stderr was: {stderr}");
    assert!(
        serde_json::from_str::<serde_json::Value>(&stderr).is_err(),
        "stderr should not be valid sparse-row JSON: {stderr}"
    );
}
