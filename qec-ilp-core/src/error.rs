use thiserror::Error;

use crate::config::BackendKind;

#[derive(Debug, Error, PartialEq)]
pub enum BinaryIlpError {
    #[error("model row references an unknown binary variable index {0}")]
    UnknownBinaryVar(usize),
    #[error("model row references an unknown integer variable index {0}")]
    UnknownIntegerVar(usize),
    #[error("model references an unknown constraint row index {0}")]
    UnknownConstraintRow(usize),
    #[error("no ILP backend is available for kind {requested:?}")]
    BackendUnavailable { requested: BackendKind },
    #[error("HiGHS backend error: {0}")]
    Highs(String),
    #[cfg(feature = "gurobi")]
    #[error("Gurobi backend error: {0}")]
    Gurobi(String),
}
