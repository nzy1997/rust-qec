use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum QecError {
    #[error("row width mismatch: expected {expected}, got {actual}")]
    RowWidthMismatch { expected: usize, actual: usize },
    #[error("invalid symplectic row width: expected even width, got {width}")]
    InvalidSymplecticRowWidth { width: usize },
    #[error("non-binary matrix entry {value} at row {row}, column {col}")]
    InvalidBinaryEntry { row: usize, col: usize, value: u8 },
    #[error("invalid Pauli width: x has {x_width} bits, z has {z_width}")]
    InvalidPauliWidth { x_width: usize, z_width: usize },
    #[error("non-binary Pauli bit {value} in {which} support at index {index}")]
    InvalidPauliBit {
        which: &'static str,
        index: usize,
        value: u8,
    },
    #[error("stabilizers do not mutually commute")]
    NonCommutingStabilizers,
    #[error("stabilizers are linearly dependent")]
    DependentStabilizers,
    #[error("CSS X/Z checks are not orthogonal")]
    InvalidCssOrthogonality,
    #[error("logical basis extraction is unsupported for {k} logical qubits")]
    UnsupportedLogicalBasis { k: usize },
    #[error("exhaustive Pauli enumeration is unsupported for {n} qubits on this target")]
    UnsupportedExhaustiveEnumeration { n: usize },
    #[error("distance computation is unsupported for {n} qubits in the current configuration: {reason}")]
    DistanceComputationUnsupported { n: usize, reason: String },
    #[error("logical basis not found")]
    LogicalBasisNotFound,
    #[error("distance witness not found")]
    DistanceWitnessNotFound,
    #[error("ILP backend is unavailable: {0}")]
    IlpBackendUnavailable(String),
    #[error("ILP solve failed: {0}")]
    IlpSolveFailed(String),
    #[error("ILP model is infeasible for a code with logical qubits")]
    IlpInfeasible,
    #[error("unknown built-in CSS code: {code_id}")]
    UnknownBuiltInCssCode { code_id: String },
}

pub type Result<T> = core::result::Result<T, QecError>;

#[cfg(feature = "distance-ilp-highs")]
impl From<qec_ilp_core::BinaryIlpError> for QecError {
    fn from(value: qec_ilp_core::BinaryIlpError) -> Self {
        match value {
            qec_ilp_core::BinaryIlpError::BackendUnavailable { requested } => {
                Self::IlpBackendUnavailable(format!("{requested:?}"))
            }
            other => Self::IlpSolveFailed(other.to_string()),
        }
    }
}
