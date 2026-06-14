use crate::code::StabilizerCode;
use crate::codes::built_in_css::built_in_css_checks;
use crate::css::CssCode;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Steane {
    code: StabilizerCode,
}

impl Steane {
    pub fn new() -> Result<Self> {
        let checks = built_in_css_checks("steane")?;
        let hx = row_supports_to_dense(checks.num_cols, &checks.hx);
        let hz = row_supports_to_dense(checks.num_cols, &checks.hz);
        let css = CssCode::from_hx_hz(hx, hz)?;

        Ok(Self {
            code: css.code().clone(),
        })
    }

    pub fn code(&self) -> &StabilizerCode {
        &self.code
    }
}

fn row_supports_to_dense(num_cols: usize, rows: &[Vec<usize>]) -> Vec<Vec<u8>> {
    let mut matrix = vec![vec![0; num_cols]; rows.len()];

    for (row_idx, row) in rows.iter().enumerate() {
        for &col in row {
            matrix[row_idx][col] = 1;
        }
    }

    matrix
}
