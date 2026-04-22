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
