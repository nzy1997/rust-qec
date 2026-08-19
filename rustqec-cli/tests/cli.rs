use std::io::Write;
use std::process::{Command, Stdio};

fn rustqec_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rustqec"))
}

fn run_with_stdin(args: &[&str], input: &str) -> std::process::Output {
    let mut child = rustqec_cmd()
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
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn circuit_stats_json_wraps_the_existing_rstim_result() {
    let output = run_with_stdin(
        &["circuit", "stats", "--format", "json"],
        "M 0\nDETECTOR rec[-1]\n",
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "schema_version": "rustqec.cli.v1",
            "status": "ok",
            "command": "circuit.stats",
            "result": {
                "instruction_count": 2,
                "repeat_blocks": 0,
                "max_repeat_depth": 0,
                "num_qubits": 1,
                "num_measurements": 1,
                "num_detectors": 1,
                "num_observables": 0,
                "num_ticks": 0,
                "num_sweep_bits": 0,
            },
            "warnings": [],
            "artifacts": [],
        })
    );
}

#[test]
fn circuit_stats_defaults_to_human_output() {
    let output = run_with_stdin(&["circuit", "stats"], "M 0\nDETECTOR rec[-1]\n");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        concat!(
            "instruction_count: 2\n",
            "repeat_blocks: 0\n",
            "max_repeat_depth: 0\n",
            "num_qubits: 1\n",
            "num_measurements: 1\n",
            "num_detectors: 1\n",
            "num_observables: 0\n",
            "num_ticks: 0\n",
            "num_sweep_bits: 0\n",
        )
    );
}

#[test]
fn circuit_stats_reads_from_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("circuit.stim");
    std::fs::write(&path, "M 0\nDETECTOR rec[-1]\n").unwrap();

    let output = rustqec_cmd()
        .args([
            "circuit",
            "stats",
            "--in",
            path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["result"]["num_measurements"], 1);
    assert_eq!(value["result"]["num_detectors"], 1);
}

#[test]
fn capabilities_describes_the_implemented_contract() {
    let output = rustqec_cmd()
        .args(["capabilities", "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema_version"], "rustqec.cli.v1");
    let commands = value["commands"].as_array().unwrap();
    let stats = commands
        .iter()
        .find(|entry| entry["name"] == "circuit.stats")
        .unwrap();
    assert_eq!(stats["input_sources"], serde_json::json!(["stdin", "file"]));
    assert_eq!(stats["formats"], serde_json::json!(["human", "json"]));
    assert_eq!(stats["output_schema"], "rustqec.cli.v1");
}

#[test]
fn invalid_circuit_uses_the_structured_json_error_channel() {
    let output = run_with_stdin(&["circuit", "stats", "--format", "json"], "NOT_A_GATE 0\n");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "schema_version": "rustqec.cli.v1",
            "status": "error",
            "command": "circuit.stats",
            "error": {
                "code": "invalid_circuit",
                "message": "unsupported instruction NOT_A_GATE",
            },
        })
    );
}
