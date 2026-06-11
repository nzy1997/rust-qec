use crate::error::{QecError, Result};

fn validate_binary_rows(matrix: &[Vec<u8>]) -> Result<usize> {
    let width = matrix.first().map_or(0, Vec::len);
    for (row_index, row) in matrix.iter().enumerate() {
        if row.len() != width {
            return Err(QecError::RowWidthMismatch {
                expected: width,
                actual: row.len(),
            });
        }
        for (col_index, bit) in row.iter().enumerate() {
            if *bit > 1 {
                return Err(QecError::InvalidBinaryEntry {
                    row: row_index,
                    col: col_index,
                    value: *bit,
                });
            }
        }
    }
    Ok(width)
}

fn assert_binary_rows(matrix: &[Vec<u8>]) -> usize {
    validate_binary_rows(matrix).expect("matrix rows must be rectangular binary vectors")
}

fn validate_binary_target(target: &[u8]) -> Result<()> {
    for (index, bit) in target.iter().enumerate() {
        if *bit > 1 {
            return Err(QecError::InvalidBinaryEntry {
                row: 0,
                col: index,
                value: *bit,
            });
        }
    }
    Ok(())
}

pub fn eliminate_to_row_echelon(matrix: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let width = assert_binary_rows(matrix);
    let mut rows = matrix.to_vec();
    let mut pivot_row = 0;

    for col in 0..width {
        let Some(pivot) = (pivot_row..rows.len()).find(|&row| rows[row][col] == 1) else {
            continue;
        };
        rows.swap(pivot_row, pivot);

        for row in (pivot_row + 1)..rows.len() {
            if rows[row][col] == 1 {
                for k in col..width {
                    rows[row][k] ^= rows[pivot_row][k];
                }
            }
        }

        pivot_row += 1;
        if pivot_row == rows.len() {
            break;
        }
    }

    rows
}

fn row_echelon_rank(rows: &mut [Vec<u8>], width: usize) -> usize {
    let mut rank = 0;
    let mut pivot_row = 0;

    for col in 0..width {
        let Some(pivot) = (pivot_row..rows.len()).find(|&row| rows[row][col] == 1) else {
            continue;
        };
        rows.swap(pivot_row, pivot);

        for row in 0..rows.len() {
            if row != pivot_row && rows[row][col] == 1 {
                for k in col..width {
                    rows[row][k] ^= rows[pivot_row][k];
                }
            }
        }

        rank += 1;
        pivot_row += 1;
        if pivot_row == rows.len() {
            break;
        }
    }

    rank
}

pub fn binary_rank(matrix: &[Vec<u8>]) -> usize {
    let width = assert_binary_rows(matrix);
    let mut rows = matrix.to_vec();
    row_echelon_rank(&mut rows, width)
}

pub fn try_eliminate_to_row_echelon(matrix: &[Vec<u8>]) -> Result<Vec<Vec<u8>>> {
    validate_binary_rows(matrix)?;
    Ok(eliminate_to_row_echelon(matrix))
}

pub fn try_binary_rank(matrix: &[Vec<u8>]) -> Result<usize> {
    validate_binary_rows(matrix)?;
    Ok(binary_rank(matrix))
}

pub fn try_in_row_span(matrix: &[Vec<u8>], target: &[u8]) -> Result<bool> {
    let width = validate_binary_rows(matrix)?;
    validate_binary_target(target)?;

    if matrix.is_empty() {
        return Ok(!target.iter().any(|bit| *bit != 0));
    }

    if target.len() != width {
        return Err(QecError::RowWidthMismatch {
            expected: width,
            actual: target.len(),
        });
    }

    Ok(in_row_span(matrix, target))
}

pub fn in_row_span(matrix: &[Vec<u8>], target: &[u8]) -> bool {
    let width = assert_binary_rows(matrix);
    validate_binary_target(target).expect("target entries must be binary");

    if matrix.is_empty() {
        return !target.iter().any(|bit| *bit != 0);
    }

    assert_eq!(target.len(), width, "target length must match matrix width");

    let rank = binary_rank(matrix);
    let mut augmented = matrix.to_vec();
    augmented.push(target.to_vec());
    binary_rank(&augmented) == rank
}
