use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeRequest {
    pub decoder: String,
    pub dem_path: String,
    pub dets_b8_path: String,
    pub obs_b8_path: String,
    pub num_shots: usize,
    pub num_dets: usize,
    pub num_obs: usize,
    pub max_errors: usize,
    pub batch_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeResponse {
    pub status: String,
    pub decoder: String,
    pub backend: String,
    pub shots_used: usize,
    pub logical_errors: usize,
    pub compile_us: f64,
    pub total_decode_us: f64,
    pub error: String,
}

impl BridgeResponse {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            status: "error".to_string(),
            decoder: String::new(),
            backend: String::new(),
            shots_used: 0,
            logical_errors: 0,
            compile_us: 0.0,
            total_decode_us: 0.0,
            error: message.into(),
        }
    }
}
