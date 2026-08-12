use std::io::{Cursor, ErrorKind, Read, Write};
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::NamedTempFile;

const READY: u8 = b'R';
const SAMPLE: u8 = b'S';
const RESULT: u8 = b'T';
const STOP: u8 = b'P';
const FINAL: u8 = b'F';
const ERROR: u8 = b'E';

fn worker_bin() -> &'static str {
    env!("CARGO_BIN_EXE_rstim_compiled_steady_worker")
}

fn frame(frame_type: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = vec![frame_type];
    frame.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    frame.extend_from_slice(payload);
    frame
}

fn sample_frame(request_id: u64, shots: usize) -> Vec<u8> {
    frame(
        SAMPLE,
        format!(r#"{{"request_id":{request_id},"shots":{shots}}}"#).as_bytes(),
    )
}

fn write_fixture(bytes: &[u8]) -> NamedTempFile {
    let mut fixture = NamedTempFile::new().expect("create fixture");
    fixture.write_all(bytes).expect("write fixture");
    fixture
}

fn run_worker_with_path(path: &Path, seed: u64, stdin_frames: &[u8]) -> Output {
    let mut child = Command::new(worker_bin())
        .arg("--variant")
        .arg("rstim-precompiled")
        .arg("--input")
        .arg(path)
        .arg("--seed")
        .arg(seed.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn worker");

    {
        let mut stdin = child.stdin.take().expect("worker stdin");
        stdin.write_all(stdin_frames).expect("write worker stdin");
    }

    child.wait_with_output().expect("wait for worker")
}

fn run_worker_with_fixture(fixture_bytes: &[u8], stdin_frames: &[u8]) -> Output {
    let fixture = write_fixture(fixture_bytes);
    run_worker_with_path(fixture.path(), 0, stdin_frames)
}

fn read_frame(cursor: &mut Cursor<&[u8]>) -> Option<(u8, Vec<u8>)> {
    let mut header = [0_u8; 9];
    match cursor.read_exact(&mut header) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
            assert_eq!(
                cursor.position() as usize,
                cursor.get_ref().len(),
                "truncated frame header"
            );
            return None;
        }
        Err(error) => panic!("read frame header: {error}"),
    }

    let payload_len = u64::from_le_bytes(header[1..].try_into().unwrap()) as usize;
    let mut payload = vec![0_u8; payload_len];
    cursor
        .read_exact(&mut payload)
        .expect("read complete frame payload");
    Some((header[0], payload))
}

fn read_frames(stdout: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let mut cursor = Cursor::new(stdout);
    let mut frames = Vec::new();
    while let Some(frame) = read_frame(&mut cursor) {
        frames.push(frame);
    }
    frames
}

fn frame_text(payload: &[u8]) -> String {
    String::from_utf8(payload.to_vec()).expect("utf8 frame payload")
}

fn telemetry(payload: &[u8]) -> Value {
    serde_json::from_slice(payload).expect("telemetry json")
}

fn assert_worker_telemetry(payload: &[u8], sample_calls: u64) {
    let value = telemetry(payload);
    assert_eq!(value["variant"], "rstim-precompiled");
    assert_eq!(value["compile_count"].as_u64(), Some(1));
    assert_eq!(value["reference_build_count"].as_u64(), Some(1));
    assert_eq!(value["sample_call_count"].as_u64(), Some(sample_calls));
    assert_eq!(value["measurement_count"].as_u64(), Some(1));
    assert_eq!(value["bytes_per_shot"].as_u64(), Some(1));
    assert_eq!(
        value["fixture_sha256"].as_str().expect("fixture sha").len(),
        64
    );
}

fn read_le_u64(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().expect("u64 bytes"))
}

#[test]
fn worker_samples_once_then_reports_final_telemetry() {
    let mut stdin_frames = Vec::new();
    stdin_frames.extend_from_slice(&sample_frame(7, 1));
    stdin_frames.extend_from_slice(&frame(STOP, &[]));

    let output = run_worker_with_fixture(b"X 0\nM 0\n", &stdin_frames);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let frames = read_frames(&output.stdout);
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0].0, READY);
    assert_worker_telemetry(&frames[0].1, 0);

    assert_eq!(frames[1].0, RESULT);
    assert_eq!(frames[1].1.len(), 33);
    assert_eq!(read_le_u64(&frames[1].1[0..8]), 7);
    assert_eq!(read_le_u64(&frames[1].1[8..16]), 1);
    assert_eq!(&frames[1].1[32..], &[0x01]);

    assert_eq!(frames[2].0, FINAL);
    assert_worker_telemetry(&frames[2].1, 1);
}

#[test]
fn worker_reports_invalid_sample_json_and_continues_to_final() {
    let mut stdin_frames = Vec::new();
    stdin_frames.extend_from_slice(&frame(SAMPLE, b"{not json"));
    stdin_frames.extend_from_slice(&frame(STOP, &[]));

    let output = run_worker_with_fixture(b"X 0\nM 0\n", &stdin_frames);

    assert!(output.status.success());
    let frames = read_frames(&output.stdout);
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0].0, READY);
    assert_eq!(frames[1].0, ERROR);
    assert!(
        frame_text(&frames[1].1).contains("invalid SAMPLE JSON"),
        "{}",
        frame_text(&frames[1].1)
    );
    assert_eq!(frames[2].0, FINAL);
    assert_worker_telemetry(&frames[2].1, 0);
}

#[test]
fn worker_reports_unexpected_frame_and_continues_to_final() {
    let mut stdin_frames = Vec::new();
    stdin_frames.extend_from_slice(&frame(b'?', &[]));
    stdin_frames.extend_from_slice(&frame(STOP, &[]));

    let output = run_worker_with_fixture(b"X 0\nM 0\n", &stdin_frames);

    assert!(output.status.success());
    let frames = read_frames(&output.stdout);
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0].0, READY);
    assert_eq!(frames[1].0, ERROR);
    assert!(
        frame_text(&frames[1].1).contains("unexpected frame"),
        "{}",
        frame_text(&frames[1].1)
    );
    assert_eq!(frames[2].0, FINAL);
    assert_worker_telemetry(&frames[2].1, 0);
}

#[test]
fn worker_reports_fixture_read_errors_as_error_frames() {
    let missing_dir = tempfile::tempdir().expect("temp dir");
    let missing_path = missing_dir.path().join("missing.stim");

    let output = run_worker_with_path(&missing_path, 0, &[]);

    assert!(!output.status.success());
    let frames = read_frames(&output.stdout);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].0, ERROR);
    assert!(
        frame_text(&frames[0].1).contains("failed to read"),
        "{}",
        frame_text(&frames[0].1)
    );
}

#[test]
fn worker_reports_invalid_utf8_fixtures_as_error_frames() {
    let output = run_worker_with_fixture(&[0xff], &[]);

    assert!(!output.status.success());
    let frames = read_frames(&output.stdout);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].0, ERROR);
    assert!(
        frame_text(&frames[0].1).contains("invalid utf-8"),
        "{}",
        frame_text(&frames[0].1)
    );
}

#[test]
fn worker_reports_missing_required_args_as_error_frames() {
    let output = Command::new(worker_bin())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run worker without args");

    assert!(!output.status.success());
    let frames = read_frames(&output.stdout);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].0, ERROR);
    assert!(
        frame_text(&frames[0].1).contains("required"),
        "{}",
        frame_text(&frames[0].1)
    );
}

#[test]
fn worker_help_exits_successfully() {
    let output = Command::new(worker_bin())
        .arg("--help")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run worker help");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help utf8");
    assert!(stdout.contains("--variant"));
    assert!(stdout.contains("--input"));
    assert!(stdout.contains("--seed"));
}
