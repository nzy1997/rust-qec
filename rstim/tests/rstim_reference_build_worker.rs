use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const PROTOCOL: &str = "reference-build-v1";

fn spawn_worker(protocol: &str) -> (Child, ChildStdin, BufReader<ChildStdout>) {
    let worker = env!("CARGO_BIN_EXE_rstim_reference_build_worker");
    let mut child = Command::new(worker)
        .arg("--protocol")
        .arg(protocol)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("worker starts");
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    (child, stdin, BufReader::new(stdout))
}

fn read_response(reader: &mut BufReader<ChildStdout>, line: &mut String) -> serde_json::Value {
    line.clear();
    reader.read_line(line).expect("read response");
    serde_json::from_str(line).expect("response json")
}

#[test]
fn rstim_reference_build_worker_parses_once_and_builds_references() {
    const TIMER_SCOPE: &str = "reference_build_only";
    const BYTE_SHA256: &str = "4bf5122f344554c53bde2ebb8cd2b7e3d1600ad631c385a5d7cce23c7785459a";

    let (mut child, mut stdin, mut reader) = spawn_worker(PROTOCOL);
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
        assert!(built.get("phase_counters").is_none());
    }
    drop(stdin);
    assert!(child.wait().expect("wait").success());
}

#[test]
fn rstim_reference_build_worker_reports_phase_counters_only_when_requested() {
    let (mut child, mut stdin, mut reader) = spawn_worker(PROTOCOL);
    let fixture = tempfile::NamedTempFile::new().expect("fixture");
    std::fs::write(fixture.path(), "H 0\nM 0\n").expect("write fixture");
    writeln!(
        stdin,
        "{}",
        json!({"protocol":PROTOCOL,"type":"load","fixture_path":fixture.path()})
    )
    .expect("send load");
    let mut line = String::new();
    let loaded = read_response(&mut reader, &mut line);
    assert_eq!(loaded["type"], "loaded");

    writeln!(
        stdin,
        "{}",
        json!({"protocol":PROTOCOL,"type":"build_reference","request_id":0,"include_phase_counters":true})
    )
    .expect("send build");
    let built = read_response(&mut reader, &mut line);
    let counters = built
        .get("phase_counters")
        .and_then(serde_json::Value::as_object)
        .expect("phase counters object");
    assert_eq!(counters["measurement_reset_batches"], json!(1));
    assert_eq!(counters["canonical_materializations"], json!(1));
    assert_eq!(counters["canonical_writebacks"], json!(1));
    assert_eq!(counters["collapse_pivots"], json!(1));
    assert_eq!(counters["expanded_repeat_iterations"], json!(0));
    assert_eq!(counters["measurement_bits"], json!(1));

    drop(stdin);
    assert!(child.wait().expect("wait").success());
}

#[test]
fn rstim_reference_build_worker_errors_when_reference_requires_legacy_fallback() {
    let (mut child, mut stdin, mut reader) = spawn_worker(PROTOCOL);
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

#[test]
fn rstim_reference_build_worker_reports_cli_protocol_mismatch() {
    let worker = env!("CARGO_BIN_EXE_rstim_reference_build_worker");
    let output = Command::new(worker)
        .arg("--protocol")
        .arg("wrong-protocol")
        .output()
        .expect("worker runs");

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    let error: serde_json::Value = serde_json::from_str(&stdout).expect("error json");
    assert_eq!(error["protocol"], PROTOCOL);
    assert_eq!(error["type"], "error");
    assert_eq!(
        error["message"],
        "requires --protocol reference-build-v1, got wrong-protocol"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr utf8");
    assert!(stderr.contains("requires --protocol reference-build-v1"));
}

#[test]
fn rstim_reference_build_worker_reports_request_errors_and_continues() {
    let (mut child, mut stdin, mut reader) = spawn_worker(PROTOCOL);
    let fixture = tempfile::NamedTempFile::new().expect("fixture");
    std::fs::write(fixture.path(), "M 0\n").expect("write fixture");

    writeln!(stdin).expect("send blank line");
    writeln!(
        stdin,
        "{}",
        json!({"protocol":PROTOCOL,"type":"build_reference","request_id":0})
    )
    .expect("send build before load");
    writeln!(
        stdin,
        "{}",
        json!({"protocol":"wrong-protocol","type":"load","fixture_path":fixture.path()})
    )
    .expect("send wrong protocol");
    writeln!(
        stdin,
        "{}",
        json!({"protocol":PROTOCOL,"type":"unexpected","fixture_path":fixture.path()})
    )
    .expect("send unexpected type");
    writeln!(
        stdin,
        "{}",
        json!({"protocol":PROTOCOL,"type":"load","fixture_path":fixture.path()})
    )
    .expect("send load");

    let mut line = String::new();
    let build_before_load = read_response(&mut reader, &mut line);
    assert_eq!(build_before_load["type"], "error");
    assert_eq!(
        build_before_load["message"],
        "cannot build reference before load"
    );

    let wrong_protocol = read_response(&mut reader, &mut line);
    assert_eq!(wrong_protocol["type"], "error");
    assert_eq!(
        wrong_protocol["message"],
        "request protocol must be \"reference-build-v1\""
    );

    let unexpected_type = read_response(&mut reader, &mut line);
    assert_eq!(unexpected_type["type"], "error");
    assert_eq!(unexpected_type["message"], "unexpected request type: unexpected");

    let loaded = read_response(&mut reader, &mut line);
    assert_eq!(loaded["type"], "loaded");
    assert_eq!(loaded["parse_count"], json!(1));
    assert_eq!(loaded["measurement_bits"], json!(1));

    drop(stdin);
    assert!(child.wait().expect("wait").success());
}

#[test]
fn rstim_reference_build_worker_packs_multi_byte_base64_remainders() {
    let (mut child, mut stdin, mut reader) = spawn_worker(PROTOCOL);
    let cases = [
        (
            "M 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15\n",
            16,
            2,
            "AAA=",
            "96a296d224f285c67bee93c30f8a309157f0daa35dc5b87e410b78630a09cfc7",
        ),
        (
            "M 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23\n",
            24,
            3,
            "AAAA",
            "709e80c88487a2411e1ee4dfb9f22a861492d20c4765150c0c794abd70f8147c",
        ),
    ];

    let mut line = String::new();
    for (case_index, (fixture_text, measurement_bits, packed_bytes, packed_base64, digest)) in
        cases.into_iter().enumerate()
    {
        let fixture = tempfile::NamedTempFile::new().expect("fixture");
        std::fs::write(fixture.path(), fixture_text).expect("write fixture");
        writeln!(
            stdin,
            "{}",
            json!({"protocol":PROTOCOL,"type":"load","fixture_path":fixture.path()})
        )
        .expect("send load");
        let loaded = read_response(&mut reader, &mut line);
        assert_eq!(loaded["type"], "loaded");
        assert_eq!(loaded["parse_count"], json!(case_index + 1));
        assert_eq!(loaded["measurement_bits"], json!(measurement_bits));

        writeln!(
            stdin,
            "{}",
            json!({"protocol":PROTOCOL,"type":"build_reference","request_id":measurement_bits})
        )
        .expect("send build");
        let built = read_response(&mut reader, &mut line);
        assert_eq!(built["type"], "reference_built");
        assert_eq!(built["request_id"], json!(measurement_bits));
        assert_eq!(built["backend"], "packed_inverse");
        assert_eq!(built["parse_count"], json!(case_index + 1));
        assert_eq!(built["reference_build_count"], json!(1));
        assert_eq!(built["measurement_bits"], json!(measurement_bits));
        assert_eq!(built["packed_bytes"], json!(packed_bytes));
        assert_eq!(built["packed_base64"], packed_base64);
        assert_eq!(built["byte_sha256"], digest);
    }

    drop(stdin);
    assert!(child.wait().expect("wait").success());
}
