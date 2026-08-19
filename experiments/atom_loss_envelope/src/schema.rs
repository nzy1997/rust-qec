use serde::{Deserialize, Serialize};

pub const INPUT_SCHEMA_VERSION: &str = "atom-loss-envelope.v0";
pub const RESULT_SCHEMA_VERSION: &str = "atom-loss-envelope-result.v0";

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtomLossCase {
    pub schema_version: String,
    pub num_detectors: usize,
    pub num_observables: usize,
    pub observed_detectors: Vec<usize>,
    pub independent_effects: Vec<Effect>,
    pub loss_envelopes: Vec<LossEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub id: String,
    pub detectors: Vec<usize>,
    pub observables: Vec<usize>,
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LossEnvelope {
    pub loss_id: String,
    pub candidates: Vec<Effect>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OptimalResult {
    pub schema_version: &'static str,
    pub status: &'static str,
    pub backend: &'static str,
    pub selected_independent_effects: Vec<String>,
    pub selected_loss_candidates: Vec<SelectedLossCandidate>,
    pub predicted_observables: Vec<usize>,
    pub objective: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SelectedLossCandidate {
    pub loss_id: String,
    pub candidate_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InfeasibleResult {
    pub schema_version: &'static str,
    pub status: &'static str,
    pub backend: &'static str,
}
