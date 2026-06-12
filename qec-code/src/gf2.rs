use crate::error::{QecError, Result};

pub(crate) type BinaryRow = Vec<u8>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReducedRows {
    pub rows: Vec<BinaryRow>,
    pub pivot_cols: Vec<usize>,
    pub width: usize,
}

pub(crate) fn validate_rows(matrix: &[BinaryRow]) -> Result<usize> {
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

pub(crate) fn validate_target(target: &[u8]) -> Result<()> {
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

pub(crate) fn try_row_echelon(matrix: &[BinaryRow]) -> Result<Vec<BinaryRow>> {
    let width = validate_rows(matrix)?;
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

    Ok(rows)
}

pub(crate) fn try_rref(matrix: &[BinaryRow]) -> Result<ReducedRows> {
    let width = validate_rows(matrix)?;
    let mut rows = matrix.to_vec();
    let mut pivot_cols = Vec::new();
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

        pivot_cols.push(col);
        pivot_row += 1;
        if pivot_row == rows.len() {
            break;
        }
    }

    Ok(ReducedRows {
        rows,
        pivot_cols,
        width,
    })
}

pub(crate) fn try_rank(matrix: &[BinaryRow]) -> Result<usize> {
    Ok(try_rref(matrix)?.pivot_cols.len())
}

pub(crate) fn try_in_row_span(matrix: &[BinaryRow], target: &[u8]) -> Result<bool> {
    let width = validate_rows(matrix)?;
    validate_target(target)?;

    if matrix.is_empty() {
        return Ok(!target.iter().any(|bit| *bit != 0));
    }

    if target.len() != width {
        return Err(QecError::RowWidthMismatch {
            expected: width,
            actual: target.len(),
        });
    }

    let rank = try_rank(matrix)?;
    let mut augmented = matrix.to_vec();
    augmented.push(target.to_vec());
    Ok(try_rank(&augmented)? == rank)
}

pub(crate) fn try_select_independent_rows(matrix: &[BinaryRow]) -> Result<Vec<BinaryRow>> {
    validate_rows(matrix)?;
    let mut basis = Vec::new();

    for row in matrix {
        if !try_in_row_span(&basis, row)? {
            basis.push(row.clone());
        }
    }

    Ok(basis)
}

pub(crate) fn try_nullspace_basis(matrix: &[BinaryRow]) -> Result<Vec<BinaryRow>> {
    let reduced = try_rref(matrix)?;
    let width = reduced.width;
    let pivot_cols = reduced.pivot_cols;
    let free_cols = (0..width)
        .filter(|col| !pivot_cols.contains(col))
        .collect::<Vec<_>>();

    if free_cols.is_empty() {
        return Ok(Vec::new());
    }

    let mut basis = Vec::with_capacity(free_cols.len());
    for free_col in free_cols {
        let mut vector = vec![0; width];
        vector[free_col] = 1;
        for (pivot_row, pivot_col) in pivot_cols.iter().copied().enumerate() {
            vector[pivot_col] = reduced.rows[pivot_row][free_col];
        }
        basis.push(vector);
    }

    Ok(basis)
}

#[cfg(test)]
mod tests {
    use super::{try_nullspace_basis, try_select_independent_rows};

    fn dot(lhs: &[u8], rhs: &[u8]) -> u8 {
        lhs.iter()
            .zip(rhs)
            .fold(0, |parity, (left, right)| parity ^ (*left & *right))
    }

    #[test]
    fn nullspace_basis_annihilates_every_constraint_row() {
        let matrix = vec![vec![1, 0, 1, 0], vec![0, 1, 1, 0]];
        let basis = try_nullspace_basis(&matrix).unwrap();

        assert_eq!(basis.len(), 2);
        assert!(basis.iter().all(|row| row.len() == 4));
        for row in &matrix {
            for vector in &basis {
                assert_eq!(dot(row, vector), 0);
            }
        }
    }

    #[test]
    fn select_independent_rows_drops_dependent_generators() {
        let rows = vec![vec![1, 0, 1], vec![0, 1, 1], vec![1, 1, 0]];

        assert_eq!(
            try_select_independent_rows(&rows).unwrap(),
            vec![vec![1, 0, 1], vec![0, 1, 1]]
        );
    }
}
