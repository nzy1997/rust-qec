use std::process::Command;

fn rstim_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

#[test]
fn gen_repetition_code() {
    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "repetition_code",
            "--task",
            "memory",
            "--distance",
            "3",
            "--rounds",
            "2",
            "--after_clifford_depolarization",
            "0.001",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let s = String::from_utf8(output.stdout).unwrap();
    assert!(s.contains("R "));
    assert!(s.contains("CX "));
    assert!(s.contains("M "));
    assert!(s.contains("DETECTOR"));
    assert!(s.contains("OBSERVABLE_INCLUDE"));
}

#[test]
fn gen_noiseless() {
    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "repetition_code",
            "--task",
            "memory",
            "--distance",
            "3",
            "--rounds",
            "1",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let s = String::from_utf8(output.stdout).unwrap();
    assert!(!s.contains("DEPOLARIZE"));
}

#[test]
fn gen_unknown_code_fails() {
    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "unknown",
            "--task",
            "memory",
            "--distance",
            "3",
            "--rounds",
            "1",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn gen_css_memory_from_sparse_json_files() {
    let dir = tempfile::tempdir().unwrap();
    let hx = dir.path().join("hx.json");
    let hz = dir.path().join("hz.json");
    let obs = dir.path().join("obs.json");
    std::fs::write(
        &hx,
        r#"{"format":"sparse_rows","num_cols":2,"rows":[[0,1]]}"#,
    )
    .unwrap();
    std::fs::write(&hz, r#"{"format":"sparse_rows","num_cols":2,"rows":[]}"#).unwrap();
    std::fs::write(
        &obs,
        r#"{"format":"sparse_rows","num_cols":2,"rows":[[0,1]]}"#,
    )
    .unwrap();

    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "css",
            "--task",
            "memory",
            "--hx",
            hx.to_str().unwrap(),
            "--hz",
            hz.to_str().unwrap(),
            "--basis",
            "x",
            "--rounds",
            "2",
            "--schedule",
            "greedy",
            "--observables",
            obs.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("DETECTOR"));
    assert!(stdout.contains("OBSERVABLE_INCLUDE"));
}

#[test]
fn gen_css_memory_reports_non_orthogonal_checks() {
    let dir = tempfile::tempdir().unwrap();
    let hx = dir.path().join("hx.json");
    let hz = dir.path().join("hz.json");
    std::fs::write(&hx, r#"{"format":"dense","rows":[[1]]}"#).unwrap();
    std::fs::write(&hz, r#"{"format":"dense","rows":[[1]]}"#).unwrap();

    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "css",
            "--task",
            "memory",
            "--hx",
            hx.to_str().unwrap(),
            "--hz",
            hz.to_str().unwrap(),
            "--basis",
            "x",
            "--rounds",
            "1",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("CSS X/Z checks are not orthogonal"),
        "stderr: {stderr}"
    );
}
