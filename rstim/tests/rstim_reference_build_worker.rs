use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn rstim_reference_build_worker_parses_once_and_builds_references() {
    let worker = env!("CARGO_BIN_EXE_rstim_reference_build_worker");
    let mut child = Command::new(worker)
        .arg("--protocol")
        .arg("reference-build-v1")
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
        json!({"protocol":"reference-build-v1","type":"load","fixture_path":fixture.path()})
    )
    .expect("send load");
    let mut line = String::new();
    reader.read_line(&mut line).expect("read load");
    let loaded: serde_json::Value = serde_json::from_str(&line).expect("load json");
    assert_eq!(loaded["type"], "loaded");
    assert_eq!(loaded["parse_count"], 1);
    for request_id in 0..2 {
        writeln!(
            stdin,
            "{}",
            json!({"protocol":"reference-build-v1","type":"build_reference","request_id":request_id})
        )
        .expect("send build");
        line.clear();
        reader.read_line(&mut line).expect("read build");
        let built: serde_json::Value = serde_json::from_str(&line).expect("build json");
        assert_eq!(built["type"], "reference_built");
        assert_eq!(built["backend"], "packed_inverse");
        assert_eq!(built["parse_count"], 1);
        assert_eq!(built["reference_build_count"], request_id + 1);
        assert_eq!(built["measurement_bits"], 1);
        assert_eq!(built["packed_bytes"], 1);
        assert_eq!(built["packed_base64"], "AQ==");
    }
    drop(stdin);
    assert!(child.wait().expect("wait").success());
}
