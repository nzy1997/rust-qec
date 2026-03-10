use std::io::Write;
use std::process::{Command, Stdio};

fn rstim_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

fn run_with_stdin(args: &[&str], stdin_data: &str) -> std::process::Output {
    let mut child = rstim_cmd()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin_data.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn analyze_errors_basic() {
    let output = run_with_stdin(
        &["analyze_errors"],
        "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let s = String::from_utf8(output.stdout).unwrap();
    assert!(s.contains("error(0.1)"));
    assert!(s.contains("D0"));
}

#[test]
fn analyze_errors_with_observable() {
    let output = run_with_stdin(
        &["analyze_errors"],
        "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]",
    );
    assert!(output.status.success());
    let s = String::from_utf8(output.stdout).unwrap();
    assert!(s.contains("D0"));
    assert!(s.contains("L0"));
}

#[test]
fn analyze_errors_from_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.stim");
    std::fs::write(&path, "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]").unwrap();
    let output = rstim_cmd()
        .args(["analyze_errors", "--in", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().contains("error"));
}

#[test]
fn analyze_errors_to_file() {
    let dir = tempfile::tempdir().unwrap();
    let out_path = dir.path().join("out.dem");
    let output = run_with_stdin(
        &["analyze_errors", "--out", out_path.to_str().unwrap()],
        "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(output.status.success());
    let dem = std::fs::read_to_string(&out_path).unwrap();
    assert!(dem.contains("error"));
}

#[test]
fn analyze_errors_invalid_rec_fails_cleanly() {
    let output = run_with_stdin(&["analyze_errors"], "DETECTOR rec[-1]");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("rec"));
    assert!(!stderr.contains("panicked"));
}

#[test]
fn analyze_errors_rejects_gauge_detector() {
    let output = run_with_stdin(&["analyze_errors"], "R 0\nH 0\nM 0\nDETECTOR rec[-1]");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("non-deterministic"));
}

#[test]
fn analyze_errors_rejects_overmixed_depolarize() {
    let output = run_with_stdin(
        &["analyze_errors"],
        "DEPOLARIZE1(0.76) 0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr).unwrap().contains("DEPOLARIZE1"));
}

#[test]
fn analyze_errors_rejects_pauli_channel_2() {
    let output = run_with_stdin(
        &["analyze_errors"],
        "PAULI_CHANNEL_2(0.01,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\nDETECTOR rec[-1]",
    );
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr).unwrap().contains("PAULI_CHANNEL_2"));
}

#[test]
fn analyze_errors_rejects_unsupported_correlated_block() {
    let output = run_with_stdin(
        &["analyze_errors"],
        "E(0.1) X0\nELSE_CORRELATED_ERROR(0.2) Z0\nELSE_CORRELATED_ERROR(0.3) Y0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr).unwrap().contains("approximation"));
}

#[test]
fn analyze_errors_allow_gauge_detectors_flag_accepts_gauge_circuit() {
    let output = run_with_stdin(
        &["analyze_errors", "--allow_gauge_detectors"],
        "R 0\nH 0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn analyze_errors_approximate_disjoint_errors_flag_accepts_pauli_channel_2() {
    let output = run_with_stdin(
        &["analyze_errors", "--approximate_disjoint_errors"],
        "PAULI_CHANNEL_2(0.01,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\nDETECTOR rec[-1]",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn analyze_errors_approximate_disjoint_errors_flag_accepts_correlated_block() {
    let output = run_with_stdin(
        &["analyze_errors", "--approximate_disjoint_errors"],
        "E(0.1) X0\nELSE_CORRELATED_ERROR(0.2) Z0\nELSE_CORRELATED_ERROR(0.3) Y0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}
