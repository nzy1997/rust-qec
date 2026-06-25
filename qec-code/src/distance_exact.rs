use crate::distance::{DistanceResult, LogicalClass};
use crate::distance_bound::DistanceBoundWitness;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExactCssDistanceMethod {
    RstimIlpExact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExactDistanceBoundType {
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExactCssDistanceStatus {
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "input", rename_all = "snake_case")]
pub enum ExactCssDistanceInput {
    CodeId { code_id: String },
    Files { hx: String, hz: String },
    QuantumTannerSpec { quantum_tanner_spec: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExactCssDistanceOptions {
    #[serde(flatten)]
    pub input: ExactCssDistanceInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExactCssDistanceProvenance {
    pub tool: String,
    pub tool_version: String,
    pub method_revision: u32,
}

impl ExactCssDistanceProvenance {
    pub fn current() -> Self {
        Self {
            tool: "qec-code".to_owned(),
            tool_version: env!("CARGO_PKG_VERSION").to_owned(),
            method_revision: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExactCssDistanceResult {
    pub status: ExactCssDistanceStatus,
    pub distance: usize,
    pub method: ExactCssDistanceMethod,
    pub bound_type: ExactDistanceBoundType,
    pub logical_class: LogicalClass,
    pub witness: DistanceBoundWitness,
    pub options: ExactCssDistanceOptions,
    pub provenance: ExactCssDistanceProvenance,
}

impl ExactCssDistanceResult {
    pub fn completed(distance: DistanceResult, options: ExactCssDistanceOptions) -> Self {
        Self {
            status: ExactCssDistanceStatus::Completed,
            distance: distance.distance,
            method: ExactCssDistanceMethod::RstimIlpExact,
            bound_type: ExactDistanceBoundType::Exact,
            logical_class: distance.logical_class,
            witness: DistanceBoundWitness::from_pauli(&distance.witness),
            options,
            provenance: ExactCssDistanceProvenance::current(),
        }
    }
}
