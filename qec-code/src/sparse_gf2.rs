use crate::error::{QecError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseGf2Matrix {
    num_rows: usize,
    num_cols: usize,
    rows: Vec<Vec<usize>>,
}

impl SparseGf2Matrix {
    pub fn new(num_rows: usize, num_cols: usize, rows: Vec<Vec<usize>>) -> Result<Self> {
        if rows.len() != num_rows {
            return Err(QecError::SparseGf2RowCountMismatch {
                expected: num_rows,
                actual: rows.len(),
            });
        }

        let rows = rows
            .into_iter()
            .enumerate()
            .map(|(row_index, row)| canonicalize_row(num_cols, row_index, row))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            num_rows,
            num_cols,
            rows,
        })
    }

    pub fn identity(size: usize) -> Result<Self> {
        identity(size)
    }

    pub fn transpose(&self) -> Result<Self> {
        transpose(self)
    }

    pub fn hconcat(&self, rhs: &Self) -> Result<Self> {
        hconcat(self, rhs)
    }

    pub fn kron(&self, rhs: &Self) -> Result<Self> {
        kron(self, rhs)
    }

    pub fn num_rows(&self) -> usize {
        self.num_rows
    }

    pub fn num_cols(&self) -> usize {
        self.num_cols
    }

    pub fn rows(&self) -> &[Vec<usize>] {
        &self.rows
    }
}

pub fn identity(size: usize) -> Result<SparseGf2Matrix> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(size)
        .map_err(|_| QecError::SparseGf2DimensionOverflow {
            operation: "identity",
        })?;
    for index in 0..size {
        rows.push(vec![index]);
    }
    SparseGf2Matrix::new(size, size, rows)
}

pub fn transpose(matrix: &SparseGf2Matrix) -> Result<SparseGf2Matrix> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(matrix.num_cols)
        .map_err(|_| QecError::SparseGf2DimensionOverflow {
            operation: "transpose",
        })?;
    rows.resize_with(matrix.num_cols, Vec::new);

    for (row_index, row) in matrix.rows.iter().enumerate() {
        for &support in row {
            rows[support].push(row_index);
        }
    }

    SparseGf2Matrix::new(matrix.num_cols, matrix.num_rows, rows)
}

pub fn hconcat(left: &SparseGf2Matrix, right: &SparseGf2Matrix) -> Result<SparseGf2Matrix> {
    if left.num_rows != right.num_rows {
        return Err(QecError::SparseGf2HorizontalRowMismatch {
            left_rows: left.num_rows,
            right_rows: right.num_rows,
        });
    }

    let num_cols =
        left.num_cols
            .checked_add(right.num_cols)
            .ok_or(QecError::SparseGf2DimensionOverflow {
                operation: "hconcat",
            })?;

    let mut rows = Vec::new();
    rows.try_reserve_exact(left.num_rows)
        .map_err(|_| QecError::SparseGf2DimensionOverflow {
            operation: "hconcat",
        })?;

    for (left_row, right_row) in left.rows.iter().zip(&right.rows) {
        let mut row = Vec::new();
        row.try_reserve_exact(left_row.len().saturating_add(right_row.len()))
            .map_err(|_| QecError::SparseGf2DimensionOverflow {
                operation: "hconcat",
            })?;
        row.extend(left_row.iter().copied());
        for &support in right_row {
            row.push(left.num_cols.checked_add(support).ok_or(
                QecError::SparseGf2DimensionOverflow {
                    operation: "hconcat",
                },
            )?);
        }
        rows.push(row);
    }

    SparseGf2Matrix::new(left.num_rows, num_cols, rows)
}

pub fn kron(left: &SparseGf2Matrix, right: &SparseGf2Matrix) -> Result<SparseGf2Matrix> {
    let num_rows = left
        .num_rows
        .checked_mul(right.num_rows)
        .ok_or(QecError::SparseGf2DimensionOverflow { operation: "kron" })?;
    let num_cols = left
        .num_cols
        .checked_mul(right.num_cols)
        .ok_or(QecError::SparseGf2DimensionOverflow { operation: "kron" })?;

    let mut rows = Vec::new();
    rows.try_reserve_exact(num_rows)
        .map_err(|_| QecError::SparseGf2DimensionOverflow { operation: "kron" })?;

    for left_row in &left.rows {
        for right_row in &right.rows {
            let mut row = Vec::new();
            for &left_support in left_row {
                let block_start = left_support
                    .checked_mul(right.num_cols)
                    .ok_or(QecError::SparseGf2DimensionOverflow { operation: "kron" })?;
                for &right_support in right_row {
                    row.push(
                        block_start
                            .checked_add(right_support)
                            .ok_or(QecError::SparseGf2DimensionOverflow { operation: "kron" })?,
                    );
                }
            }
            rows.push(row);
        }
    }

    SparseGf2Matrix::new(num_rows, num_cols, rows)
}

fn canonicalize_row(num_cols: usize, row_index: usize, mut row: Vec<usize>) -> Result<Vec<usize>> {
    for &support in &row {
        if support >= num_cols {
            return Err(QecError::SparseGf2SupportOutOfRange {
                row: row_index,
                support,
                num_cols,
            });
        }
    }

    row.sort_unstable();

    let mut canonical = Vec::new();
    let mut index = 0;
    while index < row.len() {
        let support = row[index];
        let mut keep = false;
        while index < row.len() && row[index] == support {
            keep = !keep;
            index += 1;
        }
        if keep {
            canonical.push(support);
        }
    }

    Ok(canonical)
}
