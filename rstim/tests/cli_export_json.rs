use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value;

fn rstim_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

fn run_export_json(
    input: &std::path::Path,
    output: &std::path::Path,
    format: Option<&str>,
) -> std::process::Output {
    let mut cmd = rstim_cmd();
    cmd.arg("export_json")
        .arg("--in")
        .arg(input)
        .arg("--out")
        .arg(output);
    if let Some(fmt) = format {
        cmd.arg("--format").arg(fmt);
    }
    cmd.output().unwrap()
}

fn run_export_json_with_stdin(args: &[&str], stdin_data: &str) -> std::process::Output {
    let mut child = rstim_cmd()
        .arg("export_json")
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
fn export_json_writes_qp101_document() {
    let input = tempfile::NamedTempFile::new().unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let circuit = "QUBIT_COORDS(0,0) 0\nH 0\nTICK\nM 0\nDETECTOR rec[-1]\n";
    std::fs::write(input.path(), circuit).unwrap();
    let output = run_export_json(input.path(), tmp.path(), None);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let text = std::fs::read_to_string(tmp.path()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["standard"], "QP101-ZY");
    let operations = value["operations"].as_array().unwrap();
    assert!(operations.iter().any(|op| op["type"] == "tick"));
    assert!(operations.iter().any(|op| op["type"] == "detector"));
}

#[test]
fn export_json_compact_format_writes_single_line_json() {
    let input = tempfile::NamedTempFile::new().unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let circuit = "QUBIT_COORDS(0,0) 0\nH 0\nTICK\nM 0\nDETECTOR rec[-1]\n";
    std::fs::write(input.path(), circuit).unwrap();

    let output = run_export_json(input.path(), tmp.path(), Some("compact"));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());

    let text = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(!text.contains('\n') || text.ends_with('\n'));
    let text = text.trim_end_matches('\n');
    assert!(!text.contains('\n'));
    let value: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(value["standard"], "QP101-ZY");
}

#[test]
fn export_json_invalid_format_does_not_modify_existing_output_file() {
    let input = tempfile::NamedTempFile::new().unwrap();
    let out = tempfile::NamedTempFile::new().unwrap();
    let circuit = "QUBIT_COORDS(0,0) 0\nH 0\nTICK\nM 0\nDETECTOR rec[-1]\n";
    std::fs::write(input.path(), circuit).unwrap();
    std::fs::write(out.path(), "existing output should remain\n").unwrap();

    let output = run_export_json(input.path(), out.path(), Some("invalid"));
    assert!(
        !output.status.success(),
        "expected failure for invalid format, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown json format: invalid"), "stderr: {stderr}");

    let out_text = std::fs::read_to_string(out.path()).unwrap();
    assert_eq!(out_text, "existing output should remain\n");
}

#[test]
fn export_json_stdout_pretty_format_from_stdin() {
    let output = run_export_json_with_stdin(
        &[],
        "QUBIT_COORDS(0,0) 0\nM 0\nOBSERVABLE_INCLUDE(2) rec[-1]\n",
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.ends_with('\n'));
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["standard"], "QP101-ZY");
    assert_eq!(value["operations"][1]["type"], "gate");
    assert_eq!(value["operations"][2]["type"], "observable_include");
}

#[test]
fn export_json_invalid_circuit_fails_cleanly() {
    let output = run_export_json_with_stdin(&[], "REPEAT nope {\n  M 0\n}\n");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("REPEAT") || stderr.contains("repeat"));
    assert!(!stderr.contains("panicked"));
}

#[test]
fn export_json_can_highlight_dem_error_origins() {
    let output = run_export_json_with_stdin(
        &["--highlight_dem_error", "0"],
        "REPEAT 2 {\n  DEPOLARIZE1(0.3) 5 7\n}\nM 5 7\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["operations"][0]["body"][0]["annotations"][0]["context"]["dem_error_index"],
        0
    );
    assert_eq!(value["operations"][2]["annotations"][0]["label"], "D0");
    assert_eq!(
        value["operations"][2]["annotations"][0]["context"]["detector_index"],
        0
    );
}

#[test]
fn export_json_sample_shot_exports_fixed_seed_sample_visualization_contract() {
    let output = run_export_json_with_stdin(
        &["--sample_shot", "--seed", "7"],
        "DEPOLARIZE1(1) 0\nLOSS(1) 1\nLOSS(1) 2\nM 1\nMRL 2\nDETECTOR rec[-3]\n",
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let operations = value["operations"].as_array().unwrap();
    let find_gate = |op_type: &str, gate: &str| {
        operations
            .iter()
            .find(|op| op["type"] == op_type && op["gate"] == gate)
            .unwrap()
    };

    let depolarize = find_gate("noise", "DEPOLARIZE1");
    let depolarize_annotation = &depolarize["annotations"][0];
    assert_eq!(depolarize_annotation["label"], "X");
    assert_eq!(depolarize_annotation["context"]["branch_label"], "X");

    let loss_annotations = operations
        .iter()
        .filter(|op| op["type"] == "noise" && op["gate"] == "LOSS")
        .map(|op| &op["annotations"][0])
        .collect::<Vec<_>>();
    assert_eq!(loss_annotations.len(), 2);
    assert_eq!(loss_annotations[0]["label"], "L");
    assert_eq!(loss_annotations[0]["context"]["target_qubits"], serde_json::json!([1]));
    assert_eq!(loss_annotations[1]["label"], "L");
    assert_eq!(loss_annotations[1]["context"]["target_qubits"], serde_json::json!([2]));

    let measurement = find_gate("gate", "M");
    let measurement_annotation = &measurement["annotations"][0];
    assert_eq!(measurement_annotation["label"], "1[L]");
    assert_eq!(measurement_annotation["context"]["target_qubit"], 1);
    assert_eq!(measurement_annotation["context"]["measurement_index"], 1);
    assert_eq!(measurement_annotation["context"]["loss_cause"], true);

    let loss_visible_measurement = find_gate("gate", "MRL");
    let loss_visible_annotation = &loss_visible_measurement["annotations"][0];
    assert_eq!(loss_visible_annotation["label"], "L=1 | M=1[L]");
    assert_eq!(loss_visible_annotation["context"]["target_qubit"], 2);
    assert_eq!(loss_visible_annotation["context"]["loss_visible"], true);
    assert_eq!(
        loss_visible_annotation["context"]["components"]["loss_flag"],
        serde_json::json!({
            "bit": true,
            "measurement_index": 2,
        })
    );
    assert_eq!(
        loss_visible_annotation["context"]["components"]["value"],
        serde_json::json!({
            "bit": true,
            "loss_cause": true,
            "measurement_index": 3,
        })
    );

    let detector = operations.iter().find(|op| op["type"] == "detector").unwrap();
    let detector_annotation = &detector["annotations"][0];
    assert_eq!(detector_annotation["label"], "D0");
    assert_eq!(detector_annotation["context"]["detector_index"], 0);
    assert_eq!(detector_annotation["context"]["flipped"], true);
}

#[test]
fn export_json_can_highlight_dem_error_to_compact_output_file() {
    let input = tempfile::NamedTempFile::new().unwrap();
    let out = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        input.path(),
        "REPEAT 2 {\n  DEPOLARIZE1(0.3) 5 7\n}\nM 5 7\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    )
    .unwrap();

    let output = rstim_cmd()
        .arg("export_json")
        .arg("--in")
        .arg(input.path())
        .arg("--out")
        .arg(out.path())
        .arg("--format")
        .arg("compact")
        .arg("--highlight_dem_error")
        .arg("0")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());

    let text = std::fs::read_to_string(out.path()).unwrap();
    let text = text.trim_end_matches('\n');
    assert!(!text.contains('\n'));

    let value: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(
        value["operations"][0]["body"][0]["annotations"][0]["context"]["dem_error_index"],
        0
    );
}

#[test]
fn export_json_rejects_invalid_highlight_dem_error_index() {
    let output =
        run_export_json_with_stdin(&["--highlight_dem_error", "99"], "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("DEM error index out of range"));
}

#[test]
fn export_json_rejects_seed_without_sample_shot() {
    let output = run_export_json_with_stdin(&["--seed", "1"], "M 0\n");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--seed is only supported with --sample_shot"));
}

#[test]
fn export_json_rejects_sample_shot_with_highlight_dem_error() {
    let output = run_export_json_with_stdin(
        &["--sample_shot", "--highlight_dem_error", "0"],
        "LOSS(1) 0\nM 0\nDETECTOR rec[-1]\n",
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--sample_shot cannot be combined with --highlight_dem_error"));
}

#[test]
fn export_json_reports_unsupported_highlight_instruction_clearly() {
    let output = run_export_json_with_stdin(
        &["--highlight_dem_error", "0"],
        "R 0 1\nDEPOLARIZE2(0.1) 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(
        "--highlight_dem_error currently supports a subset of noise instructions"
    ));
    assert!(stderr.contains("DEPOLARIZE2"));
}

#[test]
fn export_json_sample_visualization_supports_all_measurement_instruction_families() {
    for (gate, circuit) in [
        ("MXX", "MXX 0 1\nDETECTOR rec[-1]\n"),
        ("MYY", "MYY 0 1\nDETECTOR rec[-1]\n"),
        ("MZZ", "MZZ 0 1\nDETECTOR rec[-1]\n"),
        ("MPP", "MPP X0*Z1\nDETECTOR rec[-1]\n"),
        ("MPAD", "MPAD(0) 0\nDETECTOR rec[-1]\n"),
        ("HERALDED_ERASE", "HERALDED_ERASE(1) 0\nDETECTOR rec[-1]\n"),
        (
            "HERALDED_PAULI_CHANNEL_1",
            "HERALDED_PAULI_CHANNEL_1(0,1,0,0) 0\nDETECTOR rec[-1]\n",
        ),
    ] {
        let output = run_export_json_with_stdin(&["--sample_shot", "--seed", "7"], circuit);
        assert!(
            output.status.success(),
            "sample export failed for {gate}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            document["operations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|op| op["gate"] == gate),
            "sample export omitted {gate}: {document}"
        );
    }
}

#[test]
fn export_json_preserves_non_support_tracking_errors_when_highlighting() {
    let output = run_export_json_with_stdin(
        &["--highlight_dem_error", "0"],
        "R 0\nDEPOLARIZE1(0.9) 0\nM 0\nDETECTOR rec[-1]\n",
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("DEPOLARIZE1(0.9) exceeds exact-analysis limit of 3/4"));
    assert!(!stderr.contains("subset of noise instructions"));
}

#[test]
fn export_json_preserves_non_range_export_errors_when_highlighting() {
    let output = run_export_json_with_stdin(
        &["--highlight_dem_error", "0"],
        "QUBIT_COORDS(1,2) !0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n",
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("QUBIT_COORDS expects qubit targets"));
    assert!(!stderr.contains("DEM error index out of range"));
}
