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
fn detect_01_noiseless() {
    let output = run_with_stdin(
        &["detect", "--shots", "3"],
        "R 0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let s = String::from_utf8(output.stdout).unwrap();
    for line in s.trim().split('\n') {
        assert_eq!(line, "0");
    }
}

#[test]
fn detect_dets_format() {
    let output = run_with_stdin(
        &["detect", "--shots", "1", "--out_format", "dets"],
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(output.status.success());
    let s = String::from_utf8(output.stdout).unwrap();
    assert_eq!(s.trim(), "shot D0");
}

#[test]
fn detect_with_observable() {
    let output = run_with_stdin(
        &["detect", "--shots", "1", "--out_format", "dets"],
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]",
    );
    assert!(output.status.success());
    let s = String::from_utf8(output.stdout).unwrap();
    assert!(s.contains("D0"));
    assert!(s.contains("L0"));
}

#[test]
fn detect_append_observables() {
    let output = run_with_stdin(
        &["detect", "--shots", "1", "--append_observables"],
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]",
    );
    assert!(output.status.success());
    let s = String::from_utf8(output.stdout).unwrap();
    assert_eq!(s.trim(), "11");
}

#[test]
fn detect_seed_deterministic() {
    let circuit = "H 0\nM 0\nDETECTOR rec[-1]";
    let out1 = run_with_stdin(&["detect", "--shots", "10", "--seed", "42"], circuit);
    let out2 = run_with_stdin(&["detect", "--shots", "10", "--seed", "42"], circuit);
    assert_eq!(out1.stdout, out2.stdout);
}

#[test]
fn detect_obs_out_writes_observables_separately() {
    let dir = tempfile::tempdir().unwrap();
    let obs_path = dir.path().join("obs.txt");
    let output = run_with_stdin(
        &["detect", "--shots", "1", "--obs_out", obs_path.to_str().unwrap()],
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "1");
    assert_eq!(std::fs::read_to_string(&obs_path).unwrap().trim(), "1");
}

#[test]
fn detect_obs_out_format_hits_writes_hits() {
    let dir = tempfile::tempdir().unwrap();
    let obs_path = dir.path().join("obs.txt");
    let output = run_with_stdin(
        &[
            "detect",
            "--shots",
            "1",
            "--obs_out",
            obs_path.to_str().unwrap(),
            "--obs_out_format",
            "hits",
        ],
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "1");
    assert_eq!(std::fs::read_to_string(&obs_path).unwrap().trim(), "0");
}

#[test]
fn detect_obs_out_and_append_observables_both_work() {
    let dir = tempfile::tempdir().unwrap();
    let obs_path = dir.path().join("obs.txt");
    let output = run_with_stdin(
        &[
            "detect",
            "--shots",
            "1",
            "--append_observables",
            "--obs_out",
            obs_path.to_str().unwrap(),
        ],
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "11");
    assert_eq!(std::fs::read_to_string(&obs_path).unwrap().trim(), "1");
}
