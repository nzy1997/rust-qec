#[cfg(not(feature = "shot-viewer"))]
#[test]
fn cli_without_viewer_omits_viewer_and_still_runs_library_commands() {
    use std::process::{Command, Stdio};

    let help = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .arg("--help")
        .output()
        .expect("run rstim help");
    assert!(help.status.success());
    assert!(!String::from_utf8(help.stdout).unwrap().contains("shot_viewer"));

    let mut child = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .args(["sample", "--shots", "1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run rstim sample");
    use std::io::Write;
    child.stdin.take().unwrap().write_all(b"M 0\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"0\n");
}

#[cfg(not(feature = "codegen-css"))]
#[test]
fn cli_without_css_reports_the_feature_requirement() {
    use std::process::Command;

    let output = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .args(["gen", "--code", "css", "--task", "memory", "--rounds", "1"])
        .output()
        .expect("run rstim CSS generation");
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr).unwrap().contains("codegen-css feature"));
}

#[test]
fn cli_dispatch_preserves_shared_sample_error_behavior() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let expected = rstim::operations::run_sample("M 0\n", 1, "dets", None, false, &mut Vec::new())
        .unwrap_err();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .args(["sample", "--shots", "1", "--out_format", "dets"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run rstim sample");
    child.stdin.take().unwrap().write_all(b"M 0\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr).unwrap().contains(&expected));
}

#[test]
fn sample_reports_malformed_circuit_before_dets_format_rejection() {
    let error = rstim::cli::run_sample("NOT_A_GATE 0\n", 1, "dets", None, false, &mut Vec::new())
        .unwrap_err();
    assert!(error.contains("NOT_A_GATE"), "{error}");
    assert!(!error.contains("dets format not applicable"), "{error}");
}
