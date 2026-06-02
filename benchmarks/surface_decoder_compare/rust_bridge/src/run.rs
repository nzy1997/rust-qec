use crate::protocol::{BridgeRequest, BridgeResponse};

pub fn handle_request(request: BridgeRequest) -> BridgeResponse {
    match request.decoder.as_str() {
        "rmatching" | "rbposd" | "rilpqec" => BridgeResponse {
            status: "error".to_string(),
            decoder: request.decoder,
            backend: String::new(),
            shots_used: 0,
            logical_errors: 0,
            compile_us: 0.0,
            total_decode_us: 0.0,
            error: "decoder implementation not added yet".to_string(),
        },
        other => BridgeResponse::error(format!("unknown decoder: {other}")),
    }
}
