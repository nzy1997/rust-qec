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
fn sample_dem_01_format() {
    let output = run_with_stdin(
        &["sample_dem", "--shots", "3"],
        "error(1) D0 L0",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let s = String::from_utf8(output.stdout).unwrap();
    for line in s.trim().split('\n') {
        assert_eq!(line, "1");
    }
}

#[test]
fn sample_dem_dets_format() {
    let output = run_with_stdin(
        &["sample_dem", "--shots", "1", "--out_format", "dets"],
        "error(1) D0 D1",
    );
    assert!(output.status.success());
    let s = String::from_utf8(output.stdout).unwrap();
    assert_eq!(s.trim(), "shot D0 D1");
}

#[test]
fn sample_dem_seed_deterministic() {
    let dem = "error(0.5) D0";
    let out1 = run_with_stdin(&["sample_dem", "--shots", "10", "--seed", "42"], dem);
    let out2 = run_with_stdin(&["sample_dem", "--shots", "10", "--seed", "42"], dem);
    assert_eq!(out1.stdout, out2.stdout);
}

#[test]
fn sample_dem_obs_out() {
    let dir = tempfile::tempdir().unwrap();
    let obs_path = dir.path().join("obs.txt");
    let output = run_with_stdin(
        &["sample_dem", "--shots", "1", "--obs_out", obs_path.to_str().unwrap()],
        "error(1) D0 L0",
    );
    assert!(output.status.success());
    let det_out = String::from_utf8(output.stdout).unwrap();
    assert_eq!(det_out.trim(), "1");
    let obs_out = std::fs::read_to_string(&obs_path).unwrap();
    assert_eq!(obs_out.trim(), "1");
}
