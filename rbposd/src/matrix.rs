use crate::error::DecodeError;
use crate::vector::{Correction, Syndrome};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityCheckMatrix {
    num_checks: usize,
    num_bits: usize,
    rows: Vec<Vec<usize>>,
    columns: Vec<Vec<usize>>,
}

impl ParityCheckMatrix {
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

    pub fn num_checks(&self) -> usize {
        self.num_checks
    }

    pub fn num_bits(&self) -> usize {
        self.num_bits
    }

    pub fn row_neighbors(&self, check: usize) -> &[usize] {
        &self.rows[check]
    }

    pub fn column_neighbors(&self, bit: usize) -> &[usize] {
        &self.columns[bit]
    }

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
