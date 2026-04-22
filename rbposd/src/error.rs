#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    EmptyMatrix,
    InvalidProbability,
    InvalidColumnIndex { column: usize, num_bits: usize },
    InvalidRowIndex { row: usize, num_checks: usize },
    DimensionMismatch {
        what: &'static str,
        expected: usize,
        actual: usize,
    },
    BpDidNotConverge,
    NoOsdSolution,
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyMatrix => write!(f, "parity-check matrix is empty"),
            Self::InvalidProbability => write!(f, "invalid probability value"),
            Self::InvalidColumnIndex { column, num_bits } => {
                write!(
                    f,
                    "column index {column} is out of bounds for code with {num_bits} bits"
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
            Self::BpDidNotConverge => write!(f, "belief propagation did not converge"),
            Self::NoOsdSolution => write!(f, "no OSD solution found"),
        }
    }
}

impl std::error::Error for DecodeError {}
