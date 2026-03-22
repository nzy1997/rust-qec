use std::process::Command;

fn run_export_json(input: &std::path::Path, output: &std::path::Path, format: Option<&str>) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rstim"));
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

#[test]
fn export_json_writes_qstd101_document() {
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
    assert_eq!(value["standard"], "QSTD101-ZY");
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

    let text = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(!text.contains('\n') || text.ends_with('\n'));
    let text = text.trim_end_matches('\n');
    assert!(!text.contains('\n'));
    let value: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(value["standard"], "QSTD101-ZY");
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
