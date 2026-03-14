use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use rstim::dem::DetectorErrorModel;
use rstim::parser::parse_lines;
use rstim::showcase::{
    dem_semantic_summary, showcase_cases, strip_comment_preamble, structural_circuit_summary,
};

fn stim_cmd() -> String {
    std::env::var("RSTIM_TEST_STIM").unwrap_or_else(|_| "stim".to_string())
}

fn run_capture(cmd: &str, args: &[String]) -> String {
    let output = Command::new(cmd).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "command failed: {cmd} {args:?}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn run_with_stdin(cmd: &str, args: &[String], stdin_data: &str) -> String {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin_data.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "command failed: {cmd} {args:?}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn panic_text(payload: Box<dyn std::any::Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_string(),
            Err(_) => "non-string panic payload".to_string(),
        },
    }
}

#[test]
fn showcase_gen_parity_matches_structurally() {
    for case in showcase_cases() {
        let args = vec![
            "gen".to_string(),
            "--code".to_string(),
            case.code.to_string(),
            "--task".to_string(),
            case.task.to_string(),
            "--distance".to_string(),
            case.distance.to_string(),
            "--rounds".to_string(),
            case.rounds.to_string(),
        ];
        let stim_text = run_capture(&stim_cmd(), &args);
        let rstim_text = run_capture(env!("CARGO_BIN_EXE_rstim"), &args);

        let stim_norm = strip_comment_preamble(&stim_text);
        let stim_instrs = parse_lines(stim_norm).unwrap();
        let rstim_instrs = parse_lines(&rstim_text).unwrap();
        assert_eq!(structural_circuit_summary(&stim_instrs), structural_circuit_summary(&rstim_instrs), "gen mismatch for {}", case.label());
    }
}

#[test]
fn showcase_dem_parity_matches_semantically() {
    for case in showcase_cases() {
        let noisy_gen_args = vec![
            "gen".to_string(),
            "--code".to_string(),
            case.code.to_string(),
            "--task".to_string(),
            case.task.to_string(),
            "--distance".to_string(),
            case.distance.to_string(),
            "--rounds".to_string(),
            case.rounds.to_string(),
            "--after_clifford_depolarization".to_string(),
            "0.001".to_string(),
        ];
        let noisy_circuit = run_capture(&stim_cmd(), &noisy_gen_args);

        let analyze_args = vec!["analyze_errors".to_string()];
        let stim_dem = run_with_stdin(&stim_cmd(), &analyze_args, &noisy_circuit);
        let rstim_dem = run_with_stdin(env!("CARGO_BIN_EXE_rstim"), &analyze_args, &noisy_circuit);

        let stim_summary = dem_semantic_summary(&DetectorErrorModel::parse(&stim_dem).unwrap());
        let rstim_summary = dem_semantic_summary(&DetectorErrorModel::parse(&rstim_dem).unwrap());
        assert_eq!(stim_summary.annotation_lines, rstim_summary.annotation_lines, "annotation mismatch for {}", case.label());
        assert_eq!(stim_summary.error_probabilities.keys().collect::<Vec<_>>(), rstim_summary.error_probabilities.keys().collect::<Vec<_>>(), "target mismatch for {}", case.label());
        for (targets, stim_p) in &stim_summary.error_probabilities {
            let rstim_p = rstim_summary.error_probabilities[targets];
            let rel = (stim_p - rstim_p).abs() / stim_p.max(1e-20);
            assert!(rel <= 1e-12, "probability mismatch for {} in {}: stim={} rstim={} rel={}", targets, case.label(), stim_p, rstim_p, rel);
        }
    }
}

#[test]
fn stim_cmd_respects_override_command() {
    let dir = tempfile::tempdir().unwrap();
    let fake_stim = dir.path().join("fake-stim");
    unsafe {
        std::env::set_var("RSTIM_TEST_STIM", &fake_stim);
    }
    assert_eq!(stim_cmd(), fake_stim.to_string_lossy());
    unsafe {
        std::env::remove_var("RSTIM_TEST_STIM");
    }
}

#[test]
fn run_with_stdin_forwards_input_to_child_process() {
    let output =
        run_with_stdin("/bin/sh", &["-c".to_string(), "cat".to_string()], "alpha\nbeta\n");
    assert_eq!(output, "alpha\nbeta\n");
}

#[test]
fn run_capture_includes_stderr_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fail");
    fs::write(&script, "#!/bin/sh\necho 'boom' >&2\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).unwrap();
    }

    let panic = std::panic::catch_unwind(|| run_capture(script.to_str().unwrap(), &[])).unwrap_err();
    let text = panic_text(panic);
    assert!(text.contains("command failed"));
    assert!(text.contains("boom"));
}

#[test]
fn panic_text_handles_static_str_and_non_string_payloads() {
    let literal = std::panic::catch_unwind(|| panic!("literal panic")).unwrap_err();
    assert_eq!(panic_text(literal), "literal panic");

    let non_string = std::panic::catch_unwind(|| std::panic::panic_any(7usize)).unwrap_err();
    assert_eq!(panic_text(non_string), "non-string panic payload");
}

#[test]
fn run_with_stdin_includes_stderr_on_failure() {
    let panic = std::panic::catch_unwind(|| {
        run_with_stdin("/bin/sh", &["-c".to_string(), "cat >/dev/null; echo nope >&2; exit 1".to_string()], "ignored\n")
    }).unwrap_err();
    let text = panic_text(panic);
    assert!(text.contains("command failed"));
    assert!(text.contains("nope"));
}
