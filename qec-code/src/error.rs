use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum QecError {
    #[error("row width mismatch: expected {expected}, got {actual}")]
    RowWidthMismatch { expected: usize, actual: usize },
    #[error("invalid symplectic row width: expected even width, got {width}")]
    InvalidSymplecticRowWidth { width: usize },
    #[error("non-binary matrix entry {value} at row {row}, column {col}")]
    InvalidBinaryEntry { row: usize, col: usize, value: u8 },
    #[error("invalid sparse-rows width: {num_cols}")]
    InvalidSparseRowsWidth { num_cols: usize },
    #[error("duplicate sparse-row support {support} in row {row}")]
    DuplicateSparseRowSupport { row: usize, support: usize },
    #[error("out-of-range sparse-row support {support} in row {row} for width {num_cols}")]
    SparseRowSupportOutOfRange {
        row: usize,
        support: usize,
        num_cols: usize,
    },
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
    #[error("logical basis not found")]
    LogicalBasisNotFound,
    #[error("distance witness not found")]
    DistanceWitnessNotFound,
    #[error("unknown built-in CSS code: {code_id}")]
    UnknownBuiltInCssCode { code_id: String },
    #[error("unknown built-in CSS family: {family}")]
    UnknownBuiltInCssFamily { family: String },
    #[error("missing built-in CSS parameter {parameter} for family {family}")]
    MissingBuiltInCssParameter { family: String, parameter: String },
    #[error("duplicate built-in CSS parameter {parameter} for family {family}")]
    DuplicateBuiltInCssParameter { family: String, parameter: String },
    #[error("invalid built-in CSS integer parameter {parameter} for family {family}: {value}")]
    InvalidBuiltInCssIntegerParameter {
        family: String,
        parameter: String,
        value: String,
    },
    #[error("unexpected built-in CSS parameter {parameter} for family {family}")]
    UnexpectedBuiltInCssParameter { family: String, parameter: String },
    #[error("out-of-range built-in CSS integer parameter {parameter} for family {family}: {value}")]
    OutOfRangeBuiltInCssIntegerParameter {
        family: String,
        parameter: String,
        value: usize,
    },
}

pub type Result<T> = core::result::Result<T, QecError>;
