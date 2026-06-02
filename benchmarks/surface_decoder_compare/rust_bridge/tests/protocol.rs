use surface_decoder_compare_bridge::protocol::{BridgeRequest, BridgeResponse};

#[test]
fn request_round_trips_through_json() {
    let request = BridgeRequest {
        decoder: "rmatching".to_string(),
        dem_path: "model.dem".to_string(),
        dets_b8_path: "detections.b8".to_string(),
        obs_b8_path: "observables.b8".to_string(),
        num_shots: 64,
        num_dets: 2,
        num_obs: 1,
        max_errors: 10,
        batch_size: 8,
    };

    let text = serde_json::to_string(&request).unwrap();
    let decoded: BridgeRequest = serde_json::from_str(&text).unwrap();

    assert_eq!(decoded.decoder, "rmatching");
    assert_eq!(decoded.num_shots, 64);
}

#[test]
fn unknown_decoder_returns_structured_error() {
    let response = BridgeResponse::error("unknown decoder: bogus");

    assert_eq!(response.status, "error");
    assert!(response.error.contains("unknown decoder"));
}
