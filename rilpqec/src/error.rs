use thiserror::Error;

#[derive(Debug, Error)]
pub enum IlpDecodeError {
    #[error("DEM probability must lie in [0, 1], got {0}")]
    InvalidProbability(f64),
    #[error("detector width mismatch: expected {expected}, got {actual}")]
    DetectorWidthMismatch { expected: usize, actual: usize },
    #[error("observable width mismatch: expected {expected}, got {actual}")]
    ObservableWidthMismatch { expected: usize, actual: usize },
    #[error("no ILP backend is available for kind {requested:?}")]
    BackendUnavailable {
        requested: crate::config::BackendKind,
    },
    #[error("HiGHS backend error: {0}")]
    Highs(String),
    #[cfg(feature = "gurobi")]
    #[error("Gurobi backend error: {0}")]
    Gurobi(String),
}
