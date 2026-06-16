use std::path::PathBuf;
use std::process::{Command, Output};

fn qec_code_bin() -> &'static str {
    env!("CARGO_BIN_EXE_qec-code")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read_fixture(rel_path: &str) -> String {
    std::fs::read_to_string(workspace_root().join(rel_path))
        .unwrap_or_else(|err| panic!("failed to read fixture {rel_path}: {err}"))
}

fn run_qec_code(args: &[&str]) -> Output {
    Command::new(qec_code_bin())
        .args(args)
        .output()
        .expect("qec-code binary should run")
}

#[test]
fn steane_summary_reports_basic_code_parameters() {
    let output = Command::new(qec_code_bin())
        .args(["code", "steane", "summary"])
        .output()
        .expect("qec-code binary should run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");

    assert!(stdout.contains("n: 7"), "stdout was: {stdout}");
    assert!(
        stdout.contains("stabilizer_rank: 6"),
        "stdout was: {stdout}"
    );
    assert!(stdout.contains("k: 1"), "stdout was: {stdout}");
}

#[test]
fn steane_distance_reports_distance_and_logical_class() {
    let output = Command::new(qec_code_bin())
        .args(["code", "steane", "distance"])
        .output()
        .expect("qec-code binary should run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");

    assert!(stdout.contains("distance: 3"), "stdout was: {stdout}");
    assert!(stdout.contains("logical_class:"), "stdout was: {stdout}");
}

#[test]
fn steane_stabilizers_reports_generator_lines() {
    let output = Command::new(qec_code_bin())
        .args(["code", "steane", "stabilizers"])
        .output()
        .expect("qec-code binary should run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");

    assert!(stdout.contains("g1:"), "stdout was: {stdout}");
    assert!(stdout.contains("g6:"), "stdout was: {stdout}");
}

#[test]
fn steane_logicals_reports_logical_sections() {
    let output = Command::new(qec_code_bin())
        .args(["code", "steane", "logicals"])
        .output()
        .expect("qec-code binary should run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");

    assert!(stdout.contains("logical_x:"), "stdout was: {stdout}");
    assert!(stdout.contains("logical_z:"), "stdout was: {stdout}");
    assert!(stdout.contains("  1:"), "stdout was: {stdout}");
    assert!(stdout.contains("weight="), "stdout was: {stdout}");
}

#[test]
fn code_css_steane_hx_prints_workspace_fixture() {
    let output = run_qec_code(&["code", "css", "steane", "hx"]);

    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("rsinter/tests/fixtures/css/steane_hx.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_steane_hz_prints_workspace_fixture() {
    let output = run_qec_code(&["code", "css", "steane", "hz"]);

    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let expected = read_fixture("rsinter/tests/fixtures/css/steane_hz.json");

    assert_eq!(stdout, expected);
}

#[test]
fn code_css_unknown_id_fails() {
    let output = run_qec_code(&["code", "css", "unknown", "hx"]);

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "stdout was: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid utf-8");

    assert!(
        stderr.contains("unknown built-in CSS code: unknown"),
        "stderr was: {stderr}"
    );
}
