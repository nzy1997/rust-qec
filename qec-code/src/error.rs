use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{0}")]
pub struct CssMatrixReadSource(pub String);

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum QecError {
    #[error("unsupported CSS construction schema version: {version}")]
    UnsupportedCssConstructionSchemaVersion { version: u64 },
    #[error("invalid CSS construction JSON: {0}")]
    InvalidCssConstructionJson(String),
    #[error("unknown CSS construction: {construction}")]
    UnknownCssConstruction { construction: String },
    #[error("invalid CSS construction {construction}: {reason}")]
    InvalidCssConstruction {
        construction: String,
        reason: String,
    },
    #[error("row width mismatch: expected {expected}, got {actual}")]
    RowWidthMismatch { expected: usize, actual: usize },
    #[error("invalid symplectic row width: expected even width, got {width}")]
    InvalidSymplecticRowWidth { width: usize },
    #[error("non-binary matrix entry {value} at row {row}, column {col}")]
    InvalidBinaryEntry { row: usize, col: usize, value: u8 },
    #[error("invalid column permutation: {reason}")]
    InvalidColumnPermutation { reason: String },
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
    #[error("sparse GF(2) row count mismatch: expected {expected}, got {actual}")]
    SparseGf2RowCountMismatch { expected: usize, actual: usize },
    #[error("out-of-range sparse GF(2) support {support} in row {row} for width {num_cols}")]
    SparseGf2SupportOutOfRange {
        row: usize,
        support: usize,
        num_cols: usize,
    },
    #[error(
        "sparse GF(2) horizontal concatenation row mismatch: left has {left_rows}, right has {right_rows}"
    )]
    SparseGf2HorizontalRowMismatch { left_rows: usize, right_rows: usize },
    #[error("sparse GF(2) dimension overflow during {operation}")]
    SparseGf2DimensionOverflow { operation: &'static str },
    #[error(
        "invalid boundary map dimensions: domain dimension {domain_dimension}, codomain dimension {codomain_dimension}"
    )]
    InvalidBoundaryMapDimensions {
        domain_dimension: usize,
        codomain_dimension: usize,
    },
    #[error("duplicate boundary map for domain dimension {domain_dimension}")]
    DuplicateBoundaryMapDimension { domain_dimension: usize },
    #[error("missing boundary map for domain dimension {domain_dimension}")]
    MissingBoundaryMap { domain_dimension: usize },
    #[error(
        "boundary composition dimension mismatch between dimensions {lower_dimension} and {upper_dimension}: lower domain has {lower_domain_cells} cells, upper codomain has {upper_codomain_cells} cells"
    )]
    BoundaryCompositionDimensionMismatch {
        lower_dimension: usize,
        upper_dimension: usize,
        lower_domain_cells: usize,
        upper_codomain_cells: usize,
    },
    #[error(
        "nonzero boundary composition between dimensions {lower_dimension} and {upper_dimension}: row {row} has support {support:?}"
    )]
    NonzeroBoundaryComposition {
        lower_dimension: usize,
        upper_dimension: usize,
        row: usize,
        support: Vec<usize>,
    },
    #[error("invalid finite group table: {reason}")]
    InvalidFiniteGroupTable { reason: String },
    #[error("finite group order {order} exceeds maximum supported order {max_order}")]
    GroupOrderLimitExceeded { order: usize, max_order: usize },
    #[error("invalid finite group element {element}: expected < {order}")]
    InvalidFiniteGroupElement { element: usize, order: usize },
    #[error("invalid group-algebra support {support}: expected < {order}")]
    InvalidGroupAlgebraElementSupport { support: usize, order: usize },
    #[error("group-algebra element order mismatch: expected {expected}, got {actual}")]
    GroupAlgebraOrderMismatch { expected: usize, actual: usize },
    #[error("group-algebra matrix row width mismatch: expected {expected}, got {actual}")]
    GroupAlgebraMatrixRowWidthMismatch { expected: usize, actual: usize },
    #[error("group-algebra dimension overflow during {operation}")]
    GroupAlgebraDimensionOverflow { operation: &'static str },
    #[error("invalid regular classical matrix option {option}: {reason}")]
    InvalidRegularClassicalMatrixConfig {
        option: &'static str,
        reason: String,
    },
    #[error("unsupported regular classical matrix algorithm version {algorithm_version}")]
    UnsupportedRegularClassicalMatrixAlgorithm { algorithm_version: u32 },
    #[error("regular classical matrix stub-count overflow for {side}")]
    RegularClassicalMatrixStubCountOverflow { side: &'static str },
    #[error(
        "regular classical matrix stub-count mismatch: column stubs {column_stubs}, row stubs {row_stubs}"
    )]
    RegularClassicalMatrixStubCountMismatch {
        column_stubs: usize,
        row_stubs: usize,
    },
    #[error(
        "regular classical matrix generation exhausted retry limit {retry_limit} after {attempts} attempts for algorithm version {algorithm_version} seed {seed}"
    )]
    RegularClassicalMatrixGenerationExhausted {
        retry_limit: usize,
        attempts: usize,
        algorithm_version: u32,
        seed: u64,
    },
    #[error("missing CSS matrix format")]
    MissingCssMatrixFormat,
    #[error("unsupported CSS matrix format: {format}")]
    UnsupportedCssMatrixFormat { format: String },
    #[error("invalid CSS matrix JSON: {0}")]
    InvalidCssMatrixJson(String),
    #[error("invalid quantum Tanner spec JSON: {0}")]
    InvalidQuantumTannerSpecJson(String),
    #[error("invalid quantum Tanner group table: {reason}")]
    InvalidQuantumTannerGroupTable { reason: String },
    #[error("unsupported quantum Tanner construction mode: {mode}")]
    UnsupportedQuantumTannerConstructionMode { mode: String },
    #[error("invalid quantum Tanner local code matrix {matrix}: {reason}")]
    InvalidQuantumTannerLocalCodeMatrix {
        matrix: &'static str,
        reason: String,
    },
    #[error(
        "invalid quantum Tanner generator {set}[{index}]: element {element} is out of range for group order {order}"
    )]
    InvalidQuantumTannerGeneratorIndex {
        set: &'static str,
        index: usize,
        element: usize,
        order: usize,
    },
    #[error("invalid quantum Tanner generator set {set}: {reason}")]
    InvalidQuantumTannerGeneratorSet { set: &'static str, reason: String },
    #[error(
        "degenerate quantum Tanner face at root {root} with a={a}, b={b}: vertices {vertices:?}"
    )]
    DegenerateQuantumTannerFace {
        root: usize,
        a: usize,
        b: usize,
        vertices: Vec<usize>,
    },
    #[error("invalid quantum Tanner group element {element}: expected < {order}")]
    InvalidQuantumTannerGroupElement { element: usize, order: usize },
    #[error("invalid quantum Tanner CSS construction: {reason}")]
    InvalidQuantumTannerCssConstruction { reason: String },
    #[error("JSON output is required for {command}")]
    JsonOutputRequired { command: &'static str },
    #[error("invalid CSS distance input: {0}")]
    InvalidCssDistanceInput(String),
    #[error("failed to read CSS matrix {path}: {source}")]
    CssMatrixReadFailed {
        path: String,
        source: CssMatrixReadSource,
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
    #[error(
        "distance computation is unsupported for {n} qubits in the current configuration: {reason}"
    )]
    DistanceComputationUnsupported { n: usize, reason: String },
    #[error("logical basis not found")]
    LogicalBasisNotFound,
    #[error("distance witness not found")]
    DistanceWitnessNotFound,
    #[error("invalid distance bound option {option}: {reason}")]
    InvalidDistanceBoundOption {
        option: &'static str,
        reason: String,
    },
    #[error("randomized upper-bound witness not found")]
    RandomizedUpperBoundWitnessNotFound,
    #[error("distance bound validation failed: {0}")]
    DistanceBoundValidationFailed(String),
    #[error("ILP backend is unavailable: {0}")]
    IlpBackendUnavailable(String),
    #[error("ILP solve failed: {0}")]
    IlpSolveFailed(String),
    #[error("ILP model is infeasible for a code with logical qubits")]
    IlpInfeasible,
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
    #[error(
        "out-of-range built-in CSS integer parameter {parameter} for family {family}: {value}"
    )]
    OutOfRangeBuiltInCssIntegerParameter {
        family: String,
        parameter: String,
        value: usize,
    },
    #[error(
        "unsupported built-in CSS integer parameter {parameter} for family {family}: {value} (supported: {supported}; {note})"
    )]
    UnsupportedBuiltInCssIntegerParameter {
        family: String,
        parameter: String,
        value: usize,
        supported: String,
        note: String,
    },
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
