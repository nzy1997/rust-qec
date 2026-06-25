use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::distance::{DistanceResult, LogicalClass};
use crate::distance_bound::DistanceBoundWitness;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExactCssDistanceMethod {
    RstimIlpExact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExactDistanceBoundType {
    Exact,
    Upper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExactCssDistanceStatus {
    Completed,
    Timeout,
    Incomplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExactCssDistanceBackend {
    #[default]
    Auto,
    Highs,
    Gurobi,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct ExactCssDistanceSolverOptions {
    #[serde(default)]
    pub backend: ExactCssDistanceBackend,
    pub time_limit_seconds: Option<f64>,
    pub mip_gap: Option<f64>,
    pub threads: Option<u32>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub verbose_solver: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExactCssDistanceSolverStatus {
    Optimal,
    TimeLimit,
    SolutionLimit,
    SubOptimal,
}

impl ExactCssDistanceSolverStatus {
    pub fn is_exact(self) -> bool {
        matches!(self, Self::Optimal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExactCssDistanceSolverReport {
    pub backend: ExactCssDistanceBackend,
    pub status: ExactCssDistanceSolverStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "input", rename_all = "snake_case")]
pub enum ExactCssDistanceInput {
    CodeId { code_id: String },
    Files { hx: String, hz: String },
    QuantumTannerSpec { quantum_tanner_spec: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExactCssDistanceOptions {
    #[serde(flatten)]
    pub input: ExactCssDistanceInput,
    #[serde(flatten)]
    pub solver: ExactCssDistanceSolverOptions,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExactCssDistanceResult {
    pub status: ExactCssDistanceStatus,
    pub distance: usize,
    pub method: ExactCssDistanceMethod,
    pub bound_type: ExactDistanceBoundType,
    pub logical_class: LogicalClass,
    pub witness: DistanceBoundWitness,
    #[serde(default)]
    pub requested_backend: ExactCssDistanceBackend,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<ExactCssDistanceBackend>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solver_status: Option<ExactCssDistanceSolverStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_limit_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mip_gap: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threads: Option<u32>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub verbose_solver: bool,
    pub options: ExactCssDistanceOptions,
    pub provenance: ExactCssDistanceProvenance,
}

impl ExactCssDistanceResult {
    pub fn completed(distance: DistanceResult, options: ExactCssDistanceOptions) -> Self {
        Self::completed_with_solver_report(distance, options, None)
    }

    pub fn completed_with_solver_report(
        distance: DistanceResult,
        options: ExactCssDistanceOptions,
        solver_report: Option<ExactCssDistanceSolverReport>,
    ) -> Self {
        let solver_status = solver_report.map(|report| report.status);
        let backend = solver_report.map(|report| report.backend);
        let solver_certifies_exact = solver_status
            .map(ExactCssDistanceSolverStatus::is_exact)
            .unwrap_or(true);
        let options_allow_exact = !has_positive_mip_gap(&options.solver);
        let is_exact = solver_certifies_exact && options_allow_exact;
        let status = match solver_status {
            Some(ExactCssDistanceSolverStatus::TimeLimit) => ExactCssDistanceStatus::Timeout,
            _ if !is_exact => ExactCssDistanceStatus::Incomplete,
            _ => ExactCssDistanceStatus::Completed,
        };
        let bound_type = if is_exact {
            ExactDistanceBoundType::Exact
        } else {
            ExactDistanceBoundType::Upper
        };

        Self {
            status,
            distance: distance.distance,
            method: ExactCssDistanceMethod::RstimIlpExact,
            bound_type,
            logical_class: distance.logical_class,
            witness: DistanceBoundWitness::from_pauli(&distance.witness),
            requested_backend: options.solver.backend,
            backend,
            solver_status,
            time_limit_seconds: options.solver.time_limit_seconds,
            mip_gap: options.solver.mip_gap,
            threads: options.solver.threads,
            verbose_solver: options.solver.verbose_solver,
            options,
            provenance: ExactCssDistanceProvenance::current(),
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn has_positive_mip_gap(options: &ExactCssDistanceSolverOptions) -> bool {
    options.mip_gap.map(|gap| gap > 0.0).unwrap_or(false)
}
