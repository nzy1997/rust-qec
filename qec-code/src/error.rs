use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum QecError {
    #[error("row width mismatch: expected {expected}, got {actual}")]
    RowWidthMismatch { expected: usize, actual: usize },
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
    #[error("logical basis not found")]
    LogicalBasisNotFound,
    #[error("distance witness not found")]
    DistanceWitnessNotFound,
}

pub type Result<T> = core::result::Result<T, QecError>;
