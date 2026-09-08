/// Errors returned while constructing or running a decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The parity-check matrix has no rows or no columns.
    EmptyMatrix,
    /// A channel probability is non-finite or outside the open interval `(0, 1)`.
    InvalidProbability,
    /// A sparse row contains a bit index outside the matrix.
    InvalidColumnIndex {
        /// Invalid bit index.
        column: usize,
        /// Matrix bit count.
        num_bits: usize,
    },
    /// A sparse column contains a check index outside the matrix.
    InvalidRowIndex {
        /// Invalid check index.
        row: usize,
        /// Matrix check count.
        num_checks: usize,
    },
    /// An input vector or sparse representation has the wrong length.
    DimensionMismatch {
        /// Name of the invalid input.
        what: &'static str,
        /// Required length.
        expected: usize,
        /// Supplied length.
        actual: usize,
    },
    /// A reduced GF(2) system cannot satisfy the target syndrome.
    SingularSystem,
    /// Belief propagation failed to converge where convergence was required.
    BpDidNotConverge,
    /// Ordered-statistics post-processing found no solution.
    NoOsdSolution,
    /// Localized-statistics post-processing found no solution.
    NoLsdSolution,
    /// A string did not name a supported OSD planner.
    UnsupportedOsdMethod {
        /// Unsupported method name.
        method: String,
    },
    /// The requested LSD order is not implemented.
    UnsupportedLsdOrder {
        /// Unsupported order.
        order: usize,
    },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyMatrix => write!(f, "parity-check matrix is empty"),
            Self::InvalidProbability => write!(f, "invalid probability value"),
            Self::InvalidColumnIndex { column, num_bits } => {
                write!(
                    f,
                    "column index {column} is out of bounds for {num_bits} bits"
                )
            }
            Self::InvalidRowIndex { row, num_checks } => {
                write!(
                    f,
                    "row index {row} is out of bounds for matrix with {num_checks} checks"
                )
            }
            Self::DimensionMismatch {
                what,
                expected,
                actual,
            } => write!(
                f,
                "dimension mismatch for {what}: expected {expected}, got {actual}"
            ),
            Self::SingularSystem => write!(f, "singular system cannot satisfy the target syndrome"),
            Self::BpDidNotConverge => write!(f, "belief propagation did not converge"),
            Self::NoOsdSolution => write!(f, "no OSD solution found"),
            Self::NoLsdSolution => write!(f, "no LSD solution found"),
            Self::UnsupportedOsdMethod { method } => write!(
                f,
                "unsupported OSD method \"{method}\"; supported methods are combination_sweep, legacy_combination_sweep, ldpc_osd_cs, osd_cs"
            ),
            Self::UnsupportedLsdOrder { order } => {
                write!(
                    f,
                    "unsupported LSD order {order}; only orders 0 and 1 are supported"
                )
            }
        }
    }
}

impl std::error::Error for DecodeError {}
