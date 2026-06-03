use std::fs;

use surface_decoder_compare_bridge::protocol::BridgeRequest;
use surface_decoder_compare_bridge::run::handle_request;
use tempfile::tempdir;

fn write_case_files() -> (tempfile::TempDir, String, String, String) {
    let dir = tempdir().unwrap();
    let dem_path = dir.path().join("model.dem");
    let dets_path = dir.path().join("detections.b8");
    let obs_path = dir.path().join("observables.b8");

    fs::write(&dem_path, "error(0.125) D0 L0\nerror(0.25) D1\n").unwrap();
    fs::write(&dets_path, [0b0000_0001]).unwrap();
    fs::write(&obs_path, [0b0000_0001]).unwrap();

    (
        dir,
        dem_path.display().to_string(),
        dets_path.display().to_string(),
        obs_path.display().to_string(),
    )
}

fn write_num_obs8_case_files(obs_byte: u8) -> (tempfile::TempDir, String, String, String) {
    let dir = tempdir().unwrap();
    let dem_path = dir.path().join("model.dem");
    let dets_path = dir.path().join("detections.b8");
    let obs_path = dir.path().join("observables.b8");

    fs::write(&dem_path, "error(0.125) D0 L7\nerror(0.25) D1\n").unwrap();
    fs::write(&dets_path, [0b0000_0001]).unwrap();
    fs::write(&obs_path, [obs_byte]).unwrap();

    (
        dir,
        dem_path.display().to_string(),
        dets_path.display().to_string(),
        obs_path.display().to_string(),
    )
}

fn make_request(
    decoder: &str,
    dem_path: String,
    dets_b8_path: String,
    obs_b8_path: String,
) -> BridgeRequest {
    BridgeRequest {
        decoder: decoder.to_string(),
        dem_path,
        dets_b8_path,
        obs_b8_path,
        num_shots: 1,
        num_dets: 2,
        num_obs: 1,
        max_errors: 1,
        batch_size: 1,
    }
}

#[test]
fn rmatching_request_decodes_a_known_single_shot() {
    let (_dir, dem_path, dets_path, obs_path) = write_case_files();
    let response = handle_request(make_request("rmatching", dem_path, dets_path, obs_path));

    assert_eq!(response.status, "ok");
    assert_eq!(response.decoder, "rmatching");
    assert_eq!(response.backend, "native");
    assert_eq!(response.shots_used, 1);
    assert_eq!(response.logical_errors, 0);
}

#[test]
fn rbposd_request_decodes_a_known_single_shot() {
    let (_dir, dem_path, dets_path, obs_path) = write_case_files();
    let response = handle_request(make_request("rbposd", dem_path, dets_path, obs_path));

    assert_eq!(response.status, "ok");
    assert_eq!(response.decoder, "rbposd");
    assert_eq!(response.backend, "native");
    assert_eq!(response.logical_errors, 0);
}

#[test]
fn rilpqec_request_reports_the_backend_it_used() {
    let (_dir, dem_path, dets_path, obs_path) = write_case_files();
    let response = handle_request(make_request("rilpqec", dem_path, dets_path, obs_path));

    assert_eq!(response.status, "ok");
    assert!(response.backend == "gurobi" || response.backend == "highs");
}

#[test]
fn rmatching_request_counts_logical_errors_when_predictions_disagree() {
    let dir = tempdir().unwrap();
    let dem_path = dir.path().join("model.dem");
    let dets_path = dir.path().join("detections.b8");
    let obs_path = dir.path().join("observables.b8");
    fs::write(&dem_path, "error(0.125) D0 L0\nerror(0.25) D1\n").unwrap();
    fs::write(&dets_path, [0b0000_0001]).unwrap();
    fs::write(&obs_path, [0b0000_0000]).unwrap();

    let response = handle_request(make_request(
        "rmatching",
        dem_path.display().to_string(),
        dets_path.display().to_string(),
        obs_path.display().to_string(),
    ));

    assert_eq!(response.status, "ok");
    assert_eq!(response.logical_errors, 1);
}

#[test]
fn rmatching_request_counts_logical_errors_for_full_observable_bytes() {
    let (_dir, dem_path, dets_path, obs_path) = write_num_obs8_case_files(0b0000_0000);
    let response = handle_request(BridgeRequest {
        num_obs: 8,
        ..make_request("rmatching", dem_path, dets_path, obs_path)
    });

    assert_eq!(response.status, "ok");
    assert_eq!(response.logical_errors, 1);
}

#[test]
fn rmatching_request_accepts_matching_full_observable_bytes() {
    let (_dir, dem_path, dets_path, obs_path) = write_num_obs8_case_files(0b1000_0000);
    let response = handle_request(BridgeRequest {
        num_obs: 8,
        ..make_request("rmatching", dem_path, dets_path, obs_path)
    });

    assert_eq!(response.status, "ok");
    assert_eq!(response.logical_errors, 0);
}

#[test]
fn zero_num_shots_is_rejected() {
    let response = handle_request(BridgeRequest {
        num_shots: 0,
        ..make_request(
            "rmatching",
            "model.dem".to_string(),
            "detections.b8".to_string(),
            "observables.b8".to_string(),
        )
    });

    assert_eq!(response.status, "error");
    assert!(response.error.contains("num_shots must be positive"));
}

#[test]
fn zero_batch_size_is_rejected() {
    let response = handle_request(BridgeRequest {
        batch_size: 0,
        ..make_request(
            "rmatching",
            "model.dem".to_string(),
            "detections.b8".to_string(),
            "observables.b8".to_string(),
        )
    });

    assert_eq!(response.status, "error");
    assert!(response.error.contains("batch_size must be positive"));
}

#[test]
fn missing_detection_file_returns_structured_error() {
    let dir = tempdir().unwrap();
    let dem_path = dir.path().join("model.dem");
    let obs_path = dir.path().join("observables.b8");
    let missing_dets_path = dir.path().join("missing-dets.b8");
    fs::write(&dem_path, "error(0.125) D0 L0\nerror(0.25) D1\n").unwrap();
    fs::write(&obs_path, [0b0000_0001]).unwrap();

    let response = handle_request(make_request(
        "rmatching",
        dem_path.display().to_string(),
        missing_dets_path.display().to_string(),
        obs_path.display().to_string(),
    ));

    assert_eq!(response.status, "error");
    assert!(response.error.contains("failed to read detections file"));
}

#[test]
fn missing_observable_file_returns_structured_error() {
    let dir = tempdir().unwrap();
    let dem_path = dir.path().join("model.dem");
    let dets_path = dir.path().join("detections.b8");
    let missing_obs_path = dir.path().join("missing-obs.b8");
    fs::write(&dem_path, "error(0.125) D0 L0\nerror(0.25) D1\n").unwrap();
    fs::write(&dets_path, [0b0000_0001]).unwrap();

    let response = handle_request(make_request(
        "rmatching",
        dem_path.display().to_string(),
        dets_path.display().to_string(),
        missing_obs_path.display().to_string(),
    ));

    assert_eq!(response.status, "error");
    assert!(response.error.contains("failed to read observables file"));
}

#[test]
fn short_detection_buffer_returns_structured_error() {
    let (_dir, dem_path, dets_path, obs_path) = write_case_files();
    let response = handle_request(BridgeRequest {
        num_shots: 2,
        ..make_request("rmatching", dem_path, dets_path, obs_path)
    });

    assert_eq!(response.status, "error");
    assert!(response.error.contains("detection buffer too short"));
}

#[test]
fn short_observable_buffer_returns_structured_error() {
    let (_dir, dem_path, dets_path, obs_path) = write_case_files();
    let response = handle_request(BridgeRequest {
        num_obs: 9,
        ..make_request("rmatching", dem_path, dets_path, obs_path)
    });

    assert_eq!(response.status, "error");
    assert!(response.error.contains("observable buffer too short"));
}

#[test]
fn unknown_decoder_returns_structured_error() {
    let (_dir, dem_path, dets_path, obs_path) = write_case_files();
    let response = handle_request(make_request("bogus", dem_path, dets_path, obs_path));

    assert_eq!(response.status, "error");
    assert!(response.error.contains("unknown decoder: bogus"));
}

#[test]
fn rilpqec_rejects_detector_width_mismatch() {
    let (_dir, dem_path, dets_path, obs_path) = write_case_files();
    let response = handle_request(BridgeRequest {
        num_dets: 3,
        ..make_request("rilpqec", dem_path, dets_path, obs_path)
    });

    assert_eq!(response.status, "error");
    assert!(response.error.contains("detector width mismatch for rilpqec"));
}

#[test]
fn rilpqec_rejects_observable_width_mismatch() {
    let (_dir, dem_path, dets_path, obs_path) = write_case_files();
    let response = handle_request(BridgeRequest {
        num_obs: 2,
        ..make_request("rilpqec", dem_path, dets_path, obs_path)
    });

    assert_eq!(response.status, "error");
    assert!(response.error.contains("observable width mismatch for rilpqec"));
}
