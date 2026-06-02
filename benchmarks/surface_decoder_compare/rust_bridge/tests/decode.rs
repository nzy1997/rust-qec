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

#[test]
fn rmatching_request_decodes_a_known_single_shot() {
    let (_dir, dem_path, dets_path, obs_path) = write_case_files();
    let response = handle_request(BridgeRequest {
        decoder: "rmatching".to_string(),
        dem_path,
        dets_b8_path: dets_path,
        obs_b8_path: obs_path,
        num_shots: 1,
        num_dets: 2,
        num_obs: 1,
        max_errors: 1,
        batch_size: 1,
    });

    assert_eq!(response.status, "ok");
    assert_eq!(response.decoder, "rmatching");
    assert_eq!(response.backend, "native");
    assert_eq!(response.shots_used, 1);
    assert_eq!(response.logical_errors, 0);
}

#[test]
fn rbposd_request_decodes_a_known_single_shot() {
    let (_dir, dem_path, dets_path, obs_path) = write_case_files();
    let response = handle_request(BridgeRequest {
        decoder: "rbposd".to_string(),
        dem_path,
        dets_b8_path: dets_path,
        obs_b8_path: obs_path,
        num_shots: 1,
        num_dets: 2,
        num_obs: 1,
        max_errors: 1,
        batch_size: 1,
    });

    assert_eq!(response.status, "ok");
    assert_eq!(response.decoder, "rbposd");
    assert_eq!(response.backend, "native");
    assert_eq!(response.logical_errors, 0);
}

#[test]
fn rilpqec_request_reports_the_backend_it_used() {
    let (_dir, dem_path, dets_path, obs_path) = write_case_files();
    let response = handle_request(BridgeRequest {
        decoder: "rilpqec".to_string(),
        dem_path,
        dets_b8_path: dets_path,
        obs_b8_path: obs_path,
        num_shots: 1,
        num_dets: 2,
        num_obs: 1,
        max_errors: 1,
        batch_size: 1,
    });

    assert_eq!(response.status, "ok");
    assert!(response.backend == "gurobi" || response.backend == "highs");
}
