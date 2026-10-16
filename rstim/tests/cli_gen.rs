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
fn gen_surface_code_atom_loss_is_opt_in_from_cli() {
    use std::fs;

    let atom_loss_path = std::env::temp_dir().join(format!(
        "rstim-surface-atom-loss-{}.stim",
        std::process::id()
    ));
    let depol_only_path = std::env::temp_dir().join(format!(
        "rstim-surface-depol-only-{}.stim",
        std::process::id()
    ));

    let atom_loss = rstim_cmd()
        .args([
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_x",
            "--distance",
            "3",
            "--rounds",
            "3",
            "--after_clifford_loss_probability",
            "0.01",
            "--out",
            atom_loss_path.to_str().expect("utf8 temp path"),
        ])
        .output()
        .expect("rstim gen atom-loss command should run");
    assert!(
        atom_loss.status.success(),
        "atom-loss command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&atom_loss.stdout),
        String::from_utf8_lossy(&atom_loss.stderr)
    );
    let atom_loss_text = fs::read_to_string(&atom_loss_path).expect("atom-loss output exists");
    assert!(
        atom_loss_text.contains("LOSS(0.01)"),
        "atom-loss output should contain LOSS(0.01):\n{atom_loss_text}"
    );
    assert!(
        atom_loss_text.contains("H") && atom_loss_text.contains("CX"),
        "positive control should include Clifford layers:\n{atom_loss_text}"
    );

    let depol_only = rstim_cmd()
        .args([
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_x",
            "--distance",
            "3",
            "--rounds",
            "3",
            "--after_clifford_depolarization",
            "0.01",
            "--out",
            depol_only_path.to_str().expect("utf8 temp path"),
        ])
        .output()
        .expect("rstim gen depolarization command should run");
    assert!(
        depol_only.status.success(),
        "depolarization command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&depol_only.stdout),
        String::from_utf8_lossy(&depol_only.stderr)
    );
    let depol_text = fs::read_to_string(&depol_only_path).expect("depol output exists");
    assert!(depol_text.contains("DEPOLARIZE1(0.01)"));
    assert!(depol_text.contains("DEPOLARIZE2(0.01)"));
    assert!(
        !depol_text.contains("LOSS(0.01)"),
        "depolarization-only generation must not emit loss:\n{depol_text}"
    );

    let _ = fs::remove_file(atom_loss_path);
    let _ = fs::remove_file(depol_only_path);
}

#[test]
fn gen_common_without_distance_does_not_touch_out() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.stim");
    std::fs::write(&out, "keep me").unwrap();

    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "repetition_code",
            "--task",
            "memory",
            "--rounds",
            "1",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("distance is required for common generators"),
        "stderr: {stderr}"
    );
    assert_eq!(std::fs::read_to_string(out).unwrap(), "keep me");
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
        r#"{"format":"sparse_rows","num_cols":2,"rows":[[0]]}"#,
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
fn gen_css_memory_requires_hx_and_hz() {
    let dir = tempfile::tempdir().unwrap();
    let hx = dir.path().join("hx.json");
    let hz = dir.path().join("hz.json");
    std::fs::write(&hx, r#"{"format":"sparse_rows","num_cols":1,"rows":[]}"#).unwrap();
    std::fs::write(&hz, r#"{"format":"sparse_rows","num_cols":1,"rows":[]}"#).unwrap();

    for (omitted_arg, provided_arg, provided_path, expected) in [
        ("--hx", "--hz", hz.as_path(), "--hx is required"),
        ("--hz", "--hx", hx.as_path(), "--hz is required"),
    ] {
        let output = rstim_cmd()
            .args([
                "gen",
                "--code",
                "css",
                "--task",
                "memory",
                provided_arg,
                provided_path.to_str().unwrap(),
                "--rounds",
                "1",
            ])
            .output()
            .unwrap();

        assert!(
            !output.status.success(),
            "omitting {omitted_arg} unexpectedly succeeded"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(expected), "stderr: {stderr}");
    }
}

#[test]
fn gen_css_memory_requires_basis() {
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
            "--rounds",
            "1",
            "--observables",
            obs.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--basis is required"), "stderr: {stderr}");
}

#[test]
fn gen_css_memory_validation_error_does_not_touch_out() {
    let dir = tempfile::tempdir().unwrap();
    let hz = dir.path().join("hz.json");
    let out = dir.path().join("out.stim");
    std::fs::write(&hz, r#"{"format":"sparse_rows","num_cols":1,"rows":[]}"#).unwrap();
    std::fs::write(&out, "keep me").unwrap();

    let output = rstim_cmd()
        .args([
            "gen",
            "--code",
            "css",
            "--task",
            "memory",
            "--hz",
            hz.to_str().unwrap(),
            "--basis",
            "x",
            "--rounds",
            "1",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--hx is required"), "stderr: {stderr}");
    assert_eq!(std::fs::read_to_string(out).unwrap(), "keep me");
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

#[test]
fn gen_css_memory_rejects_non_logical_observable_and_preserves_out() {
    let dir = tempfile::tempdir().unwrap();
    let hx = dir.path().join("hx.json");
    let hz = dir.path().join("hz.json");
    let obs = dir.path().join("obs.json");
    let out = dir.path().join("out.stim");
    let steane_h =
        r#"{"format":"sparse_rows","num_cols":7,"rows":[[0,3,5,6],[1,3,4,6],[2,4,5,6]]}"#;
    std::fs::write(&hx, steane_h).unwrap();
    std::fs::write(&hz, steane_h).unwrap();
    std::fs::write(
        &obs,
        r#"{"format":"sparse_rows","num_cols":7,"rows":[[0]]}"#,
    )
    .unwrap();
    std::fs::write(&out, "keep me").unwrap();

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
            "--observables",
            obs.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("observable 0 is not an X logical"),
        "stderr: {stderr}"
    );
    assert_eq!(std::fs::read_to_string(out).unwrap(), "keep me");
}
