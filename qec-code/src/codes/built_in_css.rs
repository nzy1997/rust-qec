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
        "bb72" => Ok(BuiltInCssCodeSpec::Fixed { code_id: "bb72" }),
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

const BB72_LX: usize = 6;
const BB72_LY: usize = 6;
const BB72_A_TERMS: &[(usize, usize)] = &[(3, 0), (0, 1), (0, 2)];
const BB72_B_TERMS: &[(usize, usize)] = &[(0, 3), (1, 0), (2, 0)];

fn bb72_checks() -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    bivariate_bicycle_checks(BB72_LX, BB72_LY, BB72_A_TERMS, BB72_B_TERMS)
}

fn bivariate_bicycle_checks(
    lx: usize,
    ly: usize,
    a_terms: &[(usize, usize)],
    b_terms: &[(usize, usize)],
) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let block = lx * ly;
    let index = |x: usize, y: usize| -> usize { (x % lx) * ly + (y % ly) };
    let mut hx = Vec::with_capacity(block);
    let mut hz = Vec::with_capacity(block);

    for x in 0..lx {
        for y in 0..ly {
            let mut x_row = Vec::new();
            for &(dx, dy) in a_terms {
                x_row.push(index(x + dx, y + dy));
            }
            for &(dx, dy) in b_terms {
                x_row.push(block + index(x + dx, y + dy));
            }
            x_row.sort_unstable();
            hx.push(x_row);

            let mut z_row = Vec::new();
            for &(dx, dy) in b_terms {
                z_row.push(index((x + lx - dx % lx) % lx, (y + ly - dy % ly) % ly));
            }
            for &(dx, dy) in a_terms {
                z_row.push(block + index((x + lx - dx % lx) % lx, (y + ly - dy % ly) % ly));
            }
            z_row.sort_unstable();
            hz.push(z_row);
        }
    }

    (hx, hz)
}

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
        "bb72" => {
            let (hx, hz) = bb72_checks();

            Ok(BuiltInCssChecks {
                code_id: "bb72",
                num_cols: 2 * BB72_LX * BB72_LY,
                hx,
                hz,
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
