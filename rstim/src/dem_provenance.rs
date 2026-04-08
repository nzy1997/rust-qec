use crate::dem::{DemTarget, DetectorErrorModel};
use serde::{Deserialize, Serialize};

pub type SourceId = usize;
pub type DemErrorId = usize;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceBranch {
    X,
    Y,
    Z,
    XX,
    XY,
    XZ,
    YX,
    YY,
    YZ,
    ZX,
    ZY,
    ZZ,
    MeasurementFlip,
    CorrelatedBranch { index: usize },
    Custom { label: String },
}

impl SourceBranch {
    pub fn label(&self) -> String {
        match self {
            SourceBranch::X => "X".to_string(),
            SourceBranch::Y => "Y".to_string(),
            SourceBranch::Z => "Z".to_string(),
            SourceBranch::XX => "XX".to_string(),
            SourceBranch::XY => "XY".to_string(),
            SourceBranch::XZ => "XZ".to_string(),
            SourceBranch::YX => "YX".to_string(),
            SourceBranch::YY => "YY".to_string(),
            SourceBranch::YZ => "YZ".to_string(),
            SourceBranch::ZX => "ZX".to_string(),
            SourceBranch::ZY => "ZY".to_string(),
            SourceBranch::ZZ => "ZZ".to_string(),
            SourceBranch::MeasurementFlip => "M".to_string(),
            SourceBranch::CorrelatedBranch { index } => format!("E{index}"),
            SourceBranch::Custom { label } => label.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackedSource {
    pub source_id: SourceId,
    pub op_path: Vec<usize>,
    pub repeat_iterations: Vec<u64>,
    pub instr_name: String,
    pub target_slots: Vec<usize>,
    pub target_qubits: Vec<u32>,
    pub branch: SourceBranch,
    pub probability_fragment: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HighlightRecord {
    pub op_path: Vec<usize>,
    pub repeat_iterations: Vec<u64>,
    pub target_slots: Vec<usize>,
    pub target_qubits: Vec<u32>,
    pub branch: String,
    pub label: String,
}

impl HighlightRecord {
    pub fn from_source(source: &TrackedSource) -> Self {
        let branch = source.branch.label();
        Self {
            op_path: source.op_path.clone(),
            repeat_iterations: source.repeat_iterations.clone(),
            target_slots: source.target_slots.clone(),
            target_qubits: source.target_qubits.clone(),
            branch: branch.clone(),
            label: branch,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackedErrorTerm {
    pub probability: f64,
    pub targets: Vec<DemTarget>,
    pub source_ids: Vec<SourceId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackedDemResult {
    pub dem: DetectorErrorModel,
    pub sources: Vec<TrackedSource>,
    pub dem_error_to_sources: Vec<Vec<SourceId>>,
    pub source_to_dem_errors: Vec<Vec<DemErrorId>>,
}

impl TrackedDemResult {
    pub fn from_terms_and_sources(sources: Vec<TrackedSource>, terms: Vec<TrackedErrorTerm>) -> Self {
        let mut dem = DetectorErrorModel::new();
        let mut dem_error_to_sources = Vec::with_capacity(terms.len());
        for term in terms {
            dem.add_error(term.probability, term.targets);
            dem_error_to_sources.push(term.source_ids);
        }

        let mut source_to_dem_errors = vec![Vec::new(); sources.len()];
        for (dem_error_id, source_ids) in dem_error_to_sources.iter().enumerate() {
            for &source_id in source_ids {
                source_to_dem_errors[source_id].push(dem_error_id);
            }
        }

        Self {
            dem,
            sources,
            dem_error_to_sources,
            source_to_dem_errors,
        }
    }
}
