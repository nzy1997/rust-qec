use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum IlpDecodeError {
    #[error("DEM probability must lie in [0, 1], got {0}")]
    InvalidProbability(f64),
    #[error("detector width mismatch: expected {expected}, got {actual}")]
    DetectorWidthMismatch { expected: usize, actual: usize },
    #[error("packed detection buffer length mismatch: expected {expected}, got {actual}")]
    PackedDetectionsLengthMismatch { expected: usize, actual: usize },
    #[error("correction width mismatch: expected {expected}, got {actual}")]
    CorrectionWidthMismatch { expected: usize, actual: usize },
    #[error("observable width mismatch: expected {expected}, got {actual}")]
    ObservableWidthMismatch { expected: usize, actual: usize },
    #[error(transparent)]
    Backend(#[from] qec_ilp_core::BinaryIlpError),
}
