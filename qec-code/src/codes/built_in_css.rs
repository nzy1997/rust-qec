use crate::error::{QecError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltInCssChecks {
    pub code_id: &'static str,
    pub num_cols: usize,
    pub hx: Vec<Vec<usize>>,
    pub hz: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltInCssCodeSpec {
    Fixed {
        code_id: &'static str,
    },
    Family {
        family: BuiltInCssFamily,
        params: BuiltInCssParams,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltInCssFamily {
    RepetitionX,
    RepetitionZ,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltInCssParams {
    pub distance: usize,
}

pub fn parse_built_in_css_code_spec(input: &str) -> Result<BuiltInCssCodeSpec> {
    if let Some((family_name, params_text)) = input.split_once(':') {
        let family = match family_name {
            "repetition_x" => BuiltInCssFamily::RepetitionX,
            "repetition_z" => BuiltInCssFamily::RepetitionZ,
            _ => {
                return Err(QecError::UnknownBuiltInCssCode {
                    code_id: family_name.to_owned(),
                });
            }
        };

        let distance = params_text
            .strip_prefix("d=")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);

        return Ok(BuiltInCssCodeSpec::Family {
            family,
            params: BuiltInCssParams { distance },
        });
    }

    match input {
        "steane" => Ok(BuiltInCssCodeSpec::Fixed { code_id: "steane" }),
        _ => Err(QecError::UnknownBuiltInCssCode {
            code_id: input.to_owned(),
        }),
    }
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
