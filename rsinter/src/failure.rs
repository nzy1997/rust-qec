use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    Ok,
    LogicalFailure,
    Timeout,
    SolverFailure,
    Unsupported,
    SamplerError,
}

impl Default for FailureKind {
    fn default() -> Self {
        Self::Ok
    }
}

impl FailureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::LogicalFailure => "logical_failure",
            Self::Timeout => "timeout",
            Self::SolverFailure => "solver_failure",
            Self::Unsupported => "unsupported",
            Self::SamplerError => "sampler_error",
        }
    }

    pub fn status(self) -> &'static str {
        match self {
            Self::Ok | Self::LogicalFailure | Self::Timeout => "ok",
            Self::SolverFailure | Self::Unsupported | Self::SamplerError => "error",
        }
    }
}

impl std::fmt::Display for FailureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for FailureKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "ok" => Ok(Self::Ok),
            "logical_failure" => Ok(Self::LogicalFailure),
            "timeout" => Ok(Self::Timeout),
            "solver_failure" => Ok(Self::SolverFailure),
            "unsupported" => Ok(Self::Unsupported),
            "sampler_error" => Ok(Self::SamplerError),
            other => Err(format!("unknown failure_kind: {other}")),
        }
    }
}

pub fn classify_completed(logical_errors: u64, timed_out: bool) -> FailureKind {
    if timed_out {
        FailureKind::Timeout
    } else if logical_errors > 0 {
        FailureKind::LogicalFailure
    } else {
        FailureKind::Ok
    }
}

pub fn classify_error(message: &str, fallback: FailureKind) -> FailureKind {
    let lower = message.to_ascii_lowercase();
    if lower.contains("backendunavailable")
        || lower.contains("backend unavailable")
        || lower.contains("backend is unavailable")
        || lower.contains("no ilp backend is available")
        || lower.contains("unsupported")
    {
        FailureKind::Unsupported
    } else {
        fallback
    }
}

pub fn combine_failure_kind(a: FailureKind, b: FailureKind) -> FailureKind {
    if failure_priority(a) >= failure_priority(b) {
        a
    } else {
        b
    }
}

fn failure_priority(kind: FailureKind) -> u8 {
    match kind {
        FailureKind::Ok => 0,
        FailureKind::LogicalFailure => 1,
        FailureKind::Timeout => 2,
        FailureKind::SamplerError => 3,
        FailureKind::SolverFailure => 4,
        FailureKind::Unsupported => 5,
    }
}
