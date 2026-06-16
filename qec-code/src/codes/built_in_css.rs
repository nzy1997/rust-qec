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
        return parse_built_in_css_family_spec(family_name, params_text);
    }

    match input {
        "steane" => Ok(BuiltInCssCodeSpec::Fixed { code_id: "steane" }),
        "repetition_x" | "repetition_z" => Err(QecError::MissingBuiltInCssParameter {
            family: input.to_owned(),
            parameter: "d".to_owned(),
        }),
        _ => Err(QecError::UnknownBuiltInCssCode {
            code_id: input.to_owned(),
        }),
    }
}

fn parse_built_in_css_family_spec(
    family_name: &str,
    params_text: &str,
) -> Result<BuiltInCssCodeSpec> {
    let family = match family_name {
        "repetition_x" => BuiltInCssFamily::RepetitionX,
        "repetition_z" => BuiltInCssFamily::RepetitionZ,
        _ => {
            return Err(QecError::UnknownBuiltInCssFamily {
                family: family_name.to_owned(),
            });
        }
    };

    let distance = parse_repetition_distance(family_name, params_text)?;

    Ok(BuiltInCssCodeSpec::Family {
        family,
        params: BuiltInCssParams { distance },
    })
}

fn parse_repetition_distance(family_name: &str, params_text: &str) -> Result<usize> {
    if params_text.is_empty() {
        return Err(QecError::MissingBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: "d".to_owned(),
        });
    }

    let mut distance = None;

    for pair in params_text.split(',') {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(QecError::UnexpectedBuiltInCssParameter {
                family: family_name.to_owned(),
                parameter: pair.to_owned(),
            });
        };

        match key {
            "d" => {
                if distance.is_some() {
                    return Err(QecError::DuplicateBuiltInCssParameter {
                        family: family_name.to_owned(),
                        parameter: "d".to_owned(),
                    });
                }

                let parsed = value.parse::<usize>().map_err(|_| {
                    QecError::InvalidBuiltInCssIntegerParameter {
                        family: family_name.to_owned(),
                        parameter: "d".to_owned(),
                        value: value.to_owned(),
                    }
                })?;

                if parsed == 0 {
                    return Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
                        family: family_name.to_owned(),
                        parameter: "d".to_owned(),
                        value: parsed,
                    });
                }

                distance = Some(parsed);
            }
            _ => {
                return Err(QecError::UnexpectedBuiltInCssParameter {
                    family: family_name.to_owned(),
                    parameter: key.to_owned(),
                });
            }
        }
    }

    distance.ok_or_else(|| QecError::MissingBuiltInCssParameter {
        family: family_name.to_owned(),
        parameter: "d".to_owned(),
    })
}

const STEANE_ROW_SUPPORTS: &[&[usize]] = &[
    &[0, 3, 5, 6],
    &[1, 3, 4, 6],
    &[2, 4, 5, 6],
];

pub fn built_in_css_checks(code_id: &str) -> Result<BuiltInCssChecks> {
    match parse_built_in_css_code_spec(code_id)? {
        BuiltInCssCodeSpec::Fixed { code_id } => fixed_built_in_css_checks(code_id),
        BuiltInCssCodeSpec::Family { family, params } => {
            repetition_css_checks(family, params.distance)
        }
    }
}

fn fixed_built_in_css_checks(code_id: &'static str) -> Result<BuiltInCssChecks> {
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

fn repetition_css_checks(
    family: BuiltInCssFamily,
    distance: usize,
) -> Result<BuiltInCssChecks> {
    match family {
        BuiltInCssFamily::RepetitionX => {
            let hx = chain_supports("repetition_x", distance)?;
            Ok(BuiltInCssChecks {
                code_id: "repetition_x",
                num_cols: distance,
                hx,
                hz: vec![],
            })
        }
        BuiltInCssFamily::RepetitionZ => {
            let hz = chain_supports("repetition_z", distance)?;
            Ok(BuiltInCssChecks {
                code_id: "repetition_z",
                num_cols: distance,
                hx: vec![],
                hz,
            })
        }
    }
}

fn chain_supports(family: &'static str, distance: usize) -> Result<Vec<Vec<usize>>> {
    if distance < 2 {
        return Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: family.to_owned(),
            parameter: "d".to_owned(),
            value: distance,
        });
    }

    Ok((0..distance - 1).map(|col| vec![col, col + 1]).collect())
}
