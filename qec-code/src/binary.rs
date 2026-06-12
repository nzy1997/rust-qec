use crate::error::Result;
use crate::gf2;

pub fn eliminate_to_row_echelon(matrix: &[Vec<u8>]) -> Vec<Vec<u8>> {
    gf2::try_row_echelon(matrix).expect("matrix rows must be rectangular binary vectors")
}

pub fn binary_rank(matrix: &[Vec<u8>]) -> usize {
    gf2::try_rank(matrix).expect("matrix rows must be rectangular binary vectors")
}

pub fn try_eliminate_to_row_echelon(matrix: &[Vec<u8>]) -> Result<Vec<Vec<u8>>> {
    gf2::try_row_echelon(matrix)
}

pub fn try_binary_rank(matrix: &[Vec<u8>]) -> Result<usize> {
    gf2::try_rank(matrix)
}

pub fn try_in_row_span(matrix: &[Vec<u8>], target: &[u8]) -> Result<bool> {
    gf2::try_in_row_span(matrix, target)
}

pub fn in_row_span(matrix: &[Vec<u8>], target: &[u8]) -> bool {
    gf2::try_in_row_span(matrix, target)
        .expect("matrix rows must be rectangular binary vectors and target entries must be binary")
}
