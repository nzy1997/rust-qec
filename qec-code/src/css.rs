use crate::Pauli;
use crate::code::StabilizerCode;
use crate::error::{QecError, Result};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseRowsMatrix {
    num_cols: usize,
    rows: Vec<Vec<usize>>,
}

impl SparseRowsMatrix {
    pub fn new(num_cols: usize, rows: Vec<Vec<usize>>) -> Result<Self> {
        validate_sparse_rows(num_cols, &rows)?;
        Ok(Self { num_cols, rows })
    }

    pub fn to_json_string(&self) -> String {
        #[derive(Serialize)]
        struct SparseRowsMatrixJson<'a> {
            format: &'static str,
            num_cols: usize,
            rows: &'a [Vec<usize>],
        }

        let json = serde_json::to_string(&SparseRowsMatrixJson {
            format: "sparse_rows",
            num_cols: self.num_cols,
            rows: &self.rows,
        })
        .expect("validated sparse rows matrix should always serialize");
        json
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssCode {
    code: StabilizerCode,
}

impl CssCode {
    pub fn from_hx_hz(hx: Vec<Vec<u8>>, hz: Vec<Vec<u8>>) -> Result<Self> {
        let n = shared_width(&hx, &hz)?;

        if !checks_are_orthogonal(&hx, &hz) {
            return Err(QecError::InvalidCssOrthogonality);
        }

        let mut stabilizers = Vec::with_capacity(hx.len() + hz.len());
        for row in hx {
            stabilizers.push(Pauli::from_xz_bits(row, vec![0; n])?);
        }
        for row in hz {
            stabilizers.push(Pauli::from_xz_bits(vec![0; n], row)?);
        }

        Ok(Self {
            code: StabilizerCode::from_stabilizers(n, stabilizers)?,
        })
    }

    pub fn code(&self) -> &StabilizerCode {
        &self.code
    }
}

fn validate_sparse_rows(num_cols: usize, rows: &[Vec<usize>]) -> Result<()> {
    if num_cols == 0 {
        return Err(QecError::InvalidSparseRowsWidth { num_cols });
    }

    for (row_index, row) in rows.iter().enumerate() {
        let mut seen = std::collections::BTreeSet::new();
        for &support in row {
            if support >= num_cols {
                return Err(QecError::SparseRowSupportOutOfRange {
                    row: row_index,
                    support,
                    num_cols,
                });
            }
            if !seen.insert(support) {
                return Err(QecError::DuplicateSparseRowSupport {
                    row: row_index,
                    support,
                });
            }
        }
    }
    Ok(())
}

fn shared_width(hx: &[Vec<u8>], hz: &[Vec<u8>]) -> Result<usize> {
    let n = hx
        .first()
        .map(Vec::len)
        .or_else(|| hz.first().map(Vec::len))
        .unwrap_or(0);

    validate_rows(hx, n)?;
    validate_rows(hz, n)?;

    Ok(n)
}

fn validate_rows(matrix: &[Vec<u8>], expected_width: usize) -> Result<()> {
    for (row_index, row) in matrix.iter().enumerate() {
        if row.len() != expected_width {
            return Err(QecError::RowWidthMismatch {
                expected: expected_width,
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

    Ok(())
}

fn checks_are_orthogonal(hx: &[Vec<u8>], hz: &[Vec<u8>]) -> bool {
    hx.iter()
        .all(|x_row| hz.iter().all(|z_row| dot_product_mod_2(x_row, z_row) == 0))
}

fn dot_product_mod_2(lhs: &[u8], rhs: &[u8]) -> u8 {
    lhs.iter()
        .zip(rhs)
        .fold(0, |parity, (left, right)| parity ^ (*left & *right))
}
