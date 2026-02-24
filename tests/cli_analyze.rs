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
