use rstim::ir::{StimInstr, circuit_to_string};
use rstim::dem::DetectorErrorModel;
use sha2::{Sha256, Digest};

#[derive(Clone, Debug, Default)]
pub struct CollectionOptions {
    pub max_shots: Option<u64>,
    pub max_errors: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct Task {
    pub circuit: Vec<StimInstr>,
    pub decoder: String,
    pub dem: DetectorErrorModel,
    pub metadata: serde_json::Value,
    pub collection_options: CollectionOptions,
}

impl Task {
    pub fn strong_id(&self) -> String {
        let obj = serde_json::json!({
            "circuit": circuit_to_string(&self.circuit),
            "decoder": self.decoder,
            "dem": self.dem.to_string(),
            "metadata": self.metadata,
        });
        let text = serde_json::to_string(&obj).unwrap();
        let hash = Sha256::digest(text.as_bytes());
        format!("{:x}", hash)
    }
}
