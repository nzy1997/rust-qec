use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum EnvelopeDecodeError {
    #[error("unsupported schema_version {actual:?}; expected {expected:?}")]
    UnsupportedSchema {
        expected: &'static str,
        actual: String,
    },
    #[error("{kind} ID must not be empty")]
    EmptyId { kind: &'static str },
    #[error("duplicate {kind} ID {id:?}")]
    DuplicateId { kind: &'static str, id: String },
    #[error("loss envelope {loss_id:?} must contain at least one candidate")]
    EmptyCandidates { loss_id: String },
    #[error("{owner} references detector index {index}, but num_detectors is {num_detectors}")]
    DetectorOutOfRange {
        owner: String,
        index: usize,
        num_detectors: usize,
    },
    #[error(
        "{owner} references observable index {index}, but num_observables is {num_observables}"
    )]
    ObservableOutOfRange {
        owner: String,
        index: usize,
        num_observables: usize,
    },
    #[error("{owner} weight must be finite and non-negative, got {weight}")]
    InvalidWeight { owner: String, weight: f64 },
    #[error("solver returned {0:?}; this command requires an optimal or infeasible result")]
    UnexpectedSolveStatus(qec_ilp_core::ModelSolutionStatus),
    #[error("solver returned {actual} binary values, expected {expected}")]
    SolutionWidthMismatch { expected: usize, actual: usize },
    #[error(transparent)]
    Backend(#[from] qec_ilp_core::BinaryIlpError),
}
