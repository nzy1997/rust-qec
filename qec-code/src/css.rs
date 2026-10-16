use crate::Pauli;
use crate::code::StabilizerCode;
use crate::error::{QecError, Result};
use crate::gf2;
use serde::{Deserialize, Serialize};

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

    pub fn num_cols(&self) -> usize {
        self.num_cols
    }

    pub fn rows(&self) -> &[Vec<usize>] {
        &self.rows
    }

    pub fn to_dense_rows(&self) -> Vec<Vec<u8>> {
        self.rows
            .iter()
            .map(|row| {
                let mut dense = vec![0; self.num_cols];
                for &support in row {
                    dense[support] = 1;
                }
                dense
            })
            .collect()
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

#[derive(Debug, Deserialize)]
struct SparseRowsMatrixJson {
    format: String,
    num_cols: usize,
    rows: Vec<Vec<usize>>,
}

pub fn sparse_rows_matrix_from_json_str(input: &str) -> Result<SparseRowsMatrix> {
    let value: serde_json::Value = serde_json::from_str(input)
        .map_err(|err| QecError::InvalidCssMatrixJson(err.to_string()))?;

    let format = value
        .get("format")
        .and_then(serde_json::Value::as_str)
        .ok_or(QecError::MissingCssMatrixFormat)?;

    if format != "sparse_rows" {
        return Err(QecError::UnsupportedCssMatrixFormat {
            format: format.to_owned(),
        });
    }

    let parsed: SparseRowsMatrixJson = serde_json::from_value(value)
        .map_err(|err| QecError::InvalidCssMatrixJson(err.to_string()))?;
    let SparseRowsMatrixJson {
        format: _format,
        num_cols,
        rows,
    } = parsed;

    SparseRowsMatrix::new(num_cols, rows)
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

        let mut stabilizer_rows = Vec::with_capacity(hx.len() + hz.len());
        for row in hx {
            let mut symplectic_row = row;
            symplectic_row.extend(vec![0; n]);
            stabilizer_rows.push(symplectic_row);
        }
        for row in hz {
            let mut symplectic_row = vec![0; n];
            symplectic_row.extend(row);
            stabilizer_rows.push(symplectic_row);
        }

        let stabilizers = gf2::try_select_independent_rows(&stabilizer_rows)?
            .into_iter()
            .map(Pauli::from_symplectic_row)
            .collect::<Result<Vec<_>>>()?;

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
