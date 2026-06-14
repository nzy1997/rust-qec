use crate::error::{QecError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltInCssChecks {
    pub code_id: &'static str,
    pub num_cols: usize,
    pub hx: Vec<Vec<usize>>,
    pub hz: Vec<Vec<usize>>,
}

const STEANE_ROW_SUPPORTS: &[&[usize]] = &[
    &[0, 3, 5, 6],
    &[1, 3, 4, 6],
    &[2, 4, 5, 6],
];

pub fn built_in_css_checks(code_id: &str) -> Result<BuiltInCssChecks> {
    match code_id {
        "steane" => {
            let hx = STEANE_ROW_SUPPORTS
                .iter()
                .map(|row| row.to_vec())
                .collect::<Vec<_>>();

            Ok(BuiltInCssChecks {
                code_id: "steane",
                num_cols: 7,
                hx: hx.clone(),
                hz: hx,
            })
        }
        _ => Err(QecError::UnknownBuiltInCssCode {
            code_id: code_id.to_owned(),
        }),
    }
}
