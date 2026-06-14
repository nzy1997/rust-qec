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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn failure_kind_status_groups_completed_and_error_kinds() {
        assert_eq!(FailureKind::Ok.status(), "ok");
        assert_eq!(FailureKind::LogicalFailure.status(), "ok");
        assert_eq!(FailureKind::Timeout.status(), "ok");
        assert_eq!(FailureKind::SolverFailure.status(), "error");
        assert_eq!(FailureKind::Unsupported.status(), "error");
        assert_eq!(FailureKind::SamplerError.status(), "error");
    }

    #[test]
    fn failure_kind_parses_snake_case_values() {
        assert_eq!(FailureKind::from_str("ok").unwrap(), FailureKind::Ok);
        assert_eq!(
            FailureKind::from_str("logical_failure").unwrap(),
            FailureKind::LogicalFailure
        );
        assert_eq!(
            FailureKind::from_str("solver_failure").unwrap(),
            FailureKind::SolverFailure
        );
        assert!(FailureKind::from_str("logical-failure").is_err());
    }

    #[test]
    fn classify_completed_prioritizes_timeout_then_logical_failure() {
        assert_eq!(classify_completed(0, false), FailureKind::Ok);
        assert_eq!(classify_completed(1, false), FailureKind::LogicalFailure);
        assert_eq!(classify_completed(0, true), FailureKind::Timeout);
        assert_eq!(classify_completed(1, true), FailureKind::Timeout);
    }

    #[test]
    fn combine_failure_kind_uses_failure_priority_order() {
        assert_eq!(
            combine_failure_kind(FailureKind::Ok, FailureKind::LogicalFailure),
            FailureKind::LogicalFailure
        );
        assert_eq!(
            combine_failure_kind(FailureKind::Timeout, FailureKind::SamplerError),
            FailureKind::SamplerError
        );
        assert_eq!(
            combine_failure_kind(FailureKind::SolverFailure, FailureKind::Unsupported),
            FailureKind::Unsupported
        );
    }
}
