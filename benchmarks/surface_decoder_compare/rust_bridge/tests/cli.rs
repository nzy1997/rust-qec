use std::io::Write;
use std::process::{Command, Output, Stdio};

use surface_decoder_compare_bridge::protocol::{BridgeRequest, BridgeResponse};

fn run_bridge(stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_surface_decoder_compare_bridge"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(stdin.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn bridge_binary_returns_json_error_for_invalid_request_json() {
    let output = run_bridge("{invalid json");
    let response: BridgeResponse = serde_json::from_slice(&output.stdout).unwrap();

    assert!(output.status.success());
    assert_eq!(response.status, "error");
    assert!(response.error.contains("invalid request"));
}

#[test]
fn bridge_binary_handles_valid_json_requests_from_stdin() {
    let request = BridgeRequest {
        decoder: "rmatching".to_string(),
        dem_path: "model.dem".to_string(),
        dets_b8_path: "detections.b8".to_string(),
        obs_b8_path: "observables.b8".to_string(),
        num_shots: 0,
        num_dets: 2,
        num_obs: 1,
        max_errors: 1,
        batch_size: 1,
    };
    let output = run_bridge(&serde_json::to_string(&request).unwrap());
    let response: BridgeResponse = serde_json::from_slice(&output.stdout).unwrap();

    assert!(output.status.success());
    assert_eq!(response.status, "error");
    assert!(response.error.contains("num_shots must be positive"));
}
