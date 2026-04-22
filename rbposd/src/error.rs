#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    EmptyMatrix,
    InvalidProbability,
    InvalidColumnIndex { column: usize, num_bits: usize },
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
                    "invalid column index {column} for code with {num_bits} bits"
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
