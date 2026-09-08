use crate::error::DecodeError;
use crate::vector::{Correction, Syndrome};

/// Sparse binary parity-check matrix with row and column adjacency indexes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityCheckMatrix {
    num_checks: usize,
    num_bits: usize,
    rows: Vec<Vec<usize>>,
    columns: Vec<Vec<usize>>,
}

impl ParityCheckMatrix {
    /// Build a matrix from the bit indexes present in each check row.
    pub fn from_sparse_rows(
        num_checks: usize,
        num_bits: usize,
        rows: Vec<Vec<usize>>,
    ) -> Result<Self, DecodeError> {
        if num_checks == 0 || num_bits == 0 {
            return Err(DecodeError::EmptyMatrix);
        }
        if rows.len() != num_checks {
            return Err(DecodeError::DimensionMismatch {
                what: "row count",
                expected: num_checks,
                actual: rows.len(),
            });
        }
        let mut columns = vec![Vec::new(); num_bits];
        for (row_index, cols) in rows.iter().enumerate() {
            for &column in cols {
                if column >= num_bits {
                    return Err(DecodeError::InvalidColumnIndex { column, num_bits });
                }
                columns[column].push(row_index);
            }
        }
        Ok(Self {
            num_checks,
            num_bits,
            rows,
            columns,
        })
    }

    /// Build a matrix from the check indexes adjacent to each bit column.
    pub fn from_sparse_columns(
        num_checks: usize,
        num_bits: usize,
        columns: Vec<Vec<usize>>,
    ) -> Result<Self, DecodeError> {
        if num_checks == 0 || num_bits == 0 {
            return Err(DecodeError::EmptyMatrix);
        }
        if columns.len() != num_bits {
            return Err(DecodeError::DimensionMismatch {
                what: "column count",
                expected: num_bits,
                actual: columns.len(),
            });
        }
        let mut rows = vec![Vec::new(); num_checks];
        for (column_index, checks) in columns.iter().enumerate() {
            for &row in checks {
                if row >= num_checks {
                    return Err(DecodeError::InvalidRowIndex { row, num_checks });
                }
                rows[row].push(column_index);
            }
        }
        Ok(Self {
            num_checks,
            num_bits,
            rows,
            columns,
        })
    }

    /// Number of check rows.
    pub fn num_checks(&self) -> usize {
        self.num_checks
    }

    /// Number of bit columns.
    pub fn num_bits(&self) -> usize {
        self.num_bits
    }

    /// Return the bit indexes in a check row.
    ///
    /// # Panics
    ///
    /// Panics when `check >= self.num_checks()`.
    pub fn row_neighbors(&self, check: usize) -> &[usize] {
        &self.rows[check]
    }

    /// Return the check indexes adjacent to a bit column.
    ///
    /// # Panics
    ///
    /// Panics when `bit >= self.num_bits()`.
    pub fn column_neighbors(&self, bit: usize) -> &[usize] {
        &self.columns[bit]
    }

    /// Multiply this matrix by a correction over GF(2).
    ///
    /// # Panics
    ///
    /// Panics when the correction contains fewer than `self.num_bits()` bits.
    pub fn multiply(&self, correction: &Correction) -> Syndrome {
        let mut syndrome = vec![false; self.num_checks];
        for (row_index, cols) in self.rows.iter().enumerate() {
            let mut parity = false;
            for &column in cols {
                parity ^= correction.as_slice()[column];
            }
            syndrome[row_index] = parity;
        }
        Syndrome::from(syndrome)
    }

    pub(crate) fn dense_rows(&self) -> Vec<Vec<bool>> {
        let mut dense = vec![vec![false; self.num_bits]; self.num_checks];
        for (row_index, cols) in self.rows.iter().enumerate() {
            for &column in cols {
                dense[row_index][column] = true;
            }
        }
        dense
    }
}
