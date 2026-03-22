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
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin_data.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn stats_text_output_from_stdin() {
    let output = run_with_stdin(
        &["stats"],
        "H 0\nREPEAT 2 {\n  M 0\n  DETECTOR rec[-1]\n  TICK\n}\n",
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("instruction_count: 5"));
    assert!(text.contains("repeat_blocks: 1"));
    assert!(text.contains("max_repeat_depth: 1"));
    assert!(text.contains("num_measurements: 2"));
    assert!(text.contains("num_detectors: 2"));
    assert!(text.contains("num_ticks: 2"));
}

#[test]
fn stats_json_output_from_stdin() {
    let output = run_with_stdin(
        &["stats", "--json"],
        "CX sweep[3] 0\nOBSERVABLE_INCLUDE(2) rec[-1]\n",
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["instruction_count"], 2);
    assert_eq!(value["repeat_blocks"], 0);
    assert_eq!(value["max_repeat_depth"], 0);
    assert_eq!(value["num_qubits"], 1);
    assert_eq!(value["num_measurements"], 0);
    assert_eq!(value["num_observables"], 3);
    assert_eq!(value["num_sweep_bits"], 4);
}

#[test]
fn stats_reads_from_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.stim");
    std::fs::write(&path, "M 0\nDETECTOR rec[-1]\n").unwrap();
    let output = rstim_cmd()
        .args(["stats", "--in", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("num_measurements: 1"));
    assert!(text.contains("num_detectors: 1"));
}

#[test]
fn stats_invalid_input_fails_cleanly() {
    let output = run_with_stdin(&["stats"], "REPEAT two {\n  M 0\n}\n");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("REPEAT") || stderr.contains("repeat"));
    assert!(!stderr.contains("panicked"));
}
