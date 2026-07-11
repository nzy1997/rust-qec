use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn rstim_reference_build_worker_parses_once_and_builds_references() {
    const PROTOCOL: &str = "reference-build-v1";
    const TIMER_SCOPE: &str = "reference_build_only";
    const BYTE_SHA256: &str = "4bf5122f344554c53bde2ebb8cd2b7e3d1600ad631c385a5d7cce23c7785459a";

    let worker = env!("CARGO_BIN_EXE_rstim_reference_build_worker");
    let mut child = Command::new(worker)
        .arg("--protocol")
        .arg(PROTOCOL)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("worker starts");
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);
    let fixture = tempfile::NamedTempFile::new().expect("fixture");
    std::fs::write(fixture.path(), "X 0\nM 0\n").expect("write fixture");
    writeln!(
        stdin,
        "{}",
        json!({"protocol":PROTOCOL,"type":"load","fixture_path":fixture.path()})
    )
    .expect("send load");
    let mut line = String::new();
    reader.read_line(&mut line).expect("read load");
    let loaded: serde_json::Value = serde_json::from_str(&line).expect("load json");
    assert_eq!(loaded["protocol"], PROTOCOL);
    assert_eq!(loaded["type"], "loaded");
    assert_eq!(loaded["parse_count"], json!(1));
    assert_eq!(loaded["measurement_bits"], json!(1));
    for request_id in 0..9 {
        writeln!(
            stdin,
            "{}",
            json!({"protocol":PROTOCOL,"type":"build_reference","request_id":request_id})
        )
        .expect("send build");
        line.clear();
        reader.read_line(&mut line).expect("read build");
        let built: serde_json::Value = serde_json::from_str(&line).expect("build json");
        assert_eq!(built["protocol"], PROTOCOL);
        assert_eq!(built["type"], "reference_built");
        assert_eq!(built["request_id"], json!(request_id));
        assert_eq!(built["backend"], "packed_inverse");
        assert_eq!(built["parse_count"], json!(1));
        assert_eq!(built["reference_build_count"], json!(request_id + 1));
        assert_eq!(built["measurement_bits"], json!(1));
        assert_eq!(built["packed_bytes"], json!(1));
        assert_eq!(built["packed_base64"], "AQ==");
        assert_eq!(built["byte_sha256"], BYTE_SHA256);
        assert_eq!(built["timer_scope"], TIMER_SCOPE);
        let elapsed_ns = built["elapsed_ns"]
            .as_u64()
            .expect("elapsed_ns is unsigned");
        assert!(elapsed_ns > 0, "elapsed_ns must be positive");
    }
    drop(stdin);
    assert!(child.wait().expect("wait").success());
}

#[test]
fn rstim_reference_build_worker_errors_when_reference_requires_legacy_fallback() {
    const PROTOCOL: &str = "reference-build-v1";

    let worker = env!("CARGO_BIN_EXE_rstim_reference_build_worker");
    let mut child = Command::new(worker)
        .arg("--protocol")
        .arg(PROTOCOL)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("worker starts");
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);
    let fixture = tempfile::NamedTempFile::new().expect("fixture");
    std::fs::write(fixture.path(), "H 1\nX 0\nCZ 0 1\nH 1\nM 0 1\n").expect("write fixture");
    writeln!(
        stdin,
        "{}",
        json!({"protocol":PROTOCOL,"type":"load","fixture_path":fixture.path()})
    )
    .expect("send load");
    let mut line = String::new();
    reader.read_line(&mut line).expect("read load");
    let loaded: serde_json::Value = serde_json::from_str(&line).expect("load json");
    assert_eq!(loaded["protocol"], PROTOCOL);
    assert_eq!(loaded["type"], "loaded");
    assert_eq!(loaded["parse_count"], json!(1));
    assert_eq!(loaded["measurement_bits"], json!(2));

    writeln!(
        stdin,
        "{}",
        json!({"protocol":PROTOCOL,"type":"build_reference","request_id":7})
    )
    .expect("send build");
    line.clear();
    reader.read_line(&mut line).expect("read build");
    let error: serde_json::Value = serde_json::from_str(&line).expect("error json");
    assert_eq!(error["protocol"], PROTOCOL);
    assert_eq!(error["type"], "error");
    assert_ne!(error["type"], "reference_built");
    let message = error["message"].as_str().expect("error message");
    assert!(
        message.contains("unsupported reference sample decision"),
        "unexpected message: {message}"
    );

    drop(stdin);
    assert!(child.wait().expect("wait").success());
}
