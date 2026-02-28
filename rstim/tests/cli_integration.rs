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
fn version_prints() {
    let output = rstim_cmd().output().unwrap();
    assert!(output.status.success());
    let s = String::from_utf8(output.stdout).unwrap();
    assert!(s.contains("rstim"));
}

#[test]
fn sample_r8_format() {
    let output = run_with_stdin(
        &["sample", "--shots", "1", "--out_format", "r8"],
        "R 0\nX 0\nM 0",
    );
    assert!(output.status.success());
    assert_eq!(output.stdout, vec![0, 0]);
}

#[test]
fn detect_r8_format() {
    let output = run_with_stdin(
        &["detect", "--shots", "1", "--out_format", "r8"],
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(output.status.success());
    assert_eq!(output.stdout, vec![0, 0]);
}

#[test]
fn pipeline_analyze_then_sample_dem() {
    let analyze_out = run_with_stdin(
        &["analyze_errors"],
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]",
    );
    assert!(analyze_out.status.success());

    let dem_text = String::from_utf8(analyze_out.stdout).unwrap();
    let sample_out = run_with_stdin(
        &["sample_dem", "--shots", "1", "--out_format", "dets"],
        &dem_text,
    );
    assert!(sample_out.status.success());
    let s = String::from_utf8(sample_out.stdout).unwrap();
    assert!(s.contains("D0"));
    assert!(s.contains("L0"));
}

#[test]
fn invalid_subcommand_fails() {
    let output = rstim_cmd()
        .args(["nonexistent"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn sample_invalid_format_fails() {
    let output = run_with_stdin(
        &["sample", "--shots", "1", "--out_format", "unknown"],
        "R 0\nM 0",
    );
    assert!(!output.status.success());
}
