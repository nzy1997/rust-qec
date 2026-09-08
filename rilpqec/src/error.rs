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
    #[error("packed {buffer} buffer is too large: {shots} shots with {bits} bits per shot")]
    PackedBufferSizeOverflow {
        buffer: &'static str,
        shots: usize,
        bits: usize,
    },
    #[error("could not allocate {bytes} bytes for observable predictions")]
    OutputAllocationFailed { bytes: usize },
    #[error("shot {shot} has a syndrome incompatible with the deterministic DEM")]
    InfeasibleSyndrome { shot: usize },
    #[error(transparent)]
    Backend(#[from] qec_ilp_core::BinaryIlpError),
}
