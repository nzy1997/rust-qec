use std::collections::HashSet;

use crate::error::{QecError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltInCssChecks {
    pub code_id: &'static str,
    pub num_cols: usize,
    pub hx: Vec<Vec<usize>>,
    pub hz: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltInCssCatalogEntry {
    pub spec: &'static str,
    pub description: &'static str,
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
    SurfaceRotated,
    Toric,
    BivariateBicycle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltInCssParams {
    Distance { distance: usize },
    BivariateBicycle(BivariateBicycleParams),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BivariateBicycleParams {
    pub lx: usize,
    pub ly: usize,
    pub a_terms: Vec<(usize, usize)>,
    pub b_terms: Vec<(usize, usize)>,
}

const BUILT_IN_CSS_CATALOG: &[BuiltInCssCatalogEntry] = &[
    BuiltInCssCatalogEntry {
        spec: "steane",
        description: "fixed [[7,1,3]] CSS code",
    },
    BuiltInCssCatalogEntry {
        spec: "bb72",
        description: "fixed [[72,12,6]] bivariate-bicycle CSS code",
    },
    BuiltInCssCatalogEntry {
        spec: "repetition_x:d=<distance>",
        description: "X-check chain, distance >= 2",
    },
    BuiltInCssCatalogEntry {
        spec: "repetition_z:d=<distance>",
        description: "Z-check chain, distance >= 2",
    },
    BuiltInCssCatalogEntry {
        spec: "surface_rotated:d=<distance>",
        description: "rotated surface CSS code, distance >= 2",
    },
    BuiltInCssCatalogEntry {
        spec: "toric:d=<distance>",
        description: "periodic square-lattice toric CSS code, distance >= 2",
    },
];

pub fn built_in_css_catalog() -> &'static [BuiltInCssCatalogEntry] {
    BUILT_IN_CSS_CATALOG
}

pub fn parse_built_in_css_code_spec(input: &str) -> Result<BuiltInCssCodeSpec> {
    if let Some((family_name, params_text)) = input.split_once(':') {
        return parse_built_in_css_family_spec(family_name, params_text);
    }

    match input {
        "steane" => Ok(BuiltInCssCodeSpec::Fixed { code_id: "steane" }),
        "bb72" => Ok(BuiltInCssCodeSpec::Fixed { code_id: "bb72" }),
        "repetition_x" | "repetition_z" | "surface_rotated" | "toric" => {
            Err(QecError::MissingBuiltInCssParameter {
                family: input.to_owned(),
                parameter: "d".to_owned(),
            })
        }
        "bb" => Err(QecError::MissingBuiltInCssParameter {
            family: input.to_owned(),
            parameter: "lx".to_owned(),
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
    match family_name {
        "repetition_x" => {
            parse_distance_family_spec(family_name, BuiltInCssFamily::RepetitionX, params_text)
        }
        "repetition_z" => {
            parse_distance_family_spec(family_name, BuiltInCssFamily::RepetitionZ, params_text)
        }
        "surface_rotated" => {
            parse_distance_family_spec(family_name, BuiltInCssFamily::SurfaceRotated, params_text)
        }
        "toric" => parse_distance_family_spec(family_name, BuiltInCssFamily::Toric, params_text),
        "bb" => {
            let params = parse_bivariate_bicycle_params(family_name, params_text)?;
            Ok(BuiltInCssCodeSpec::Family {
                family: BuiltInCssFamily::BivariateBicycle,
                params: BuiltInCssParams::BivariateBicycle(params),
            })
        }
        _ => Err(QecError::UnknownBuiltInCssFamily {
            family: family_name.to_owned(),
        }),
    }
}

fn parse_distance_family_spec(
    family_name: &str,
    family: BuiltInCssFamily,
    params_text: &str,
) -> Result<BuiltInCssCodeSpec> {
    let distance = parse_repetition_distance(family_name, params_text)?;

    Ok(BuiltInCssCodeSpec::Family {
        family,
        params: BuiltInCssParams::Distance { distance },
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

fn parse_bivariate_bicycle_params(
    family_name: &str,
    params_text: &str,
) -> Result<BivariateBicycleParams> {
    if params_text.is_empty() {
        return Err(QecError::MissingBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: "lx".to_owned(),
        });
    }

    let mut lx = None;
    let mut ly = None;
    let mut a_terms = None;
    let mut b_terms = None;

    for pair in params_text.split(',') {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(QecError::UnexpectedBuiltInCssParameter {
                family: family_name.to_owned(),
                parameter: pair.to_owned(),
            });
        };

        match key {
            "lx" => parse_unique_positive_usize_param(family_name, "lx", value, &mut lx)?,
            "ly" => parse_unique_positive_usize_param(family_name, "ly", value, &mut ly)?,
            "a" => parse_unique_bivariate_bicycle_terms(family_name, "a", value, &mut a_terms)?,
            "b" => parse_unique_bivariate_bicycle_terms(family_name, "b", value, &mut b_terms)?,
            _ => {
                return Err(QecError::UnexpectedBuiltInCssParameter {
                    family: family_name.to_owned(),
                    parameter: key.to_owned(),
                });
            }
        }
    }

    let params = BivariateBicycleParams {
        lx: lx.ok_or_else(|| QecError::MissingBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: "lx".to_owned(),
        })?,
        ly: ly.ok_or_else(|| QecError::MissingBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: "ly".to_owned(),
        })?,
        a_terms: a_terms.ok_or_else(|| QecError::MissingBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: "a".to_owned(),
        })?,
        b_terms: b_terms.ok_or_else(|| QecError::MissingBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: "b".to_owned(),
        })?,
    };

    validate_bivariate_bicycle_params(&params)?;
    Ok(params)
}

fn parse_unique_positive_usize_param(
    family_name: &str,
    parameter: &'static str,
    value: &str,
    slot: &mut Option<usize>,
) -> Result<()> {
    if slot.is_some() {
        return Err(QecError::DuplicateBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: parameter.to_owned(),
        });
    }

    let parsed =
        value
            .parse::<usize>()
            .map_err(|_| QecError::InvalidBuiltInCssIntegerParameter {
                family: family_name.to_owned(),
                parameter: parameter.to_owned(),
                value: value.to_owned(),
            })?;

    if parsed == 0 {
        return Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: family_name.to_owned(),
            parameter: parameter.to_owned(),
            value: parsed,
        });
    }

    *slot = Some(parsed);
    Ok(())
}

fn parse_unique_bivariate_bicycle_terms(
    family_name: &str,
    parameter: &'static str,
    value: &str,
    slot: &mut Option<Vec<(usize, usize)>>,
) -> Result<()> {
    if slot.is_some() {
        return Err(QecError::DuplicateBuiltInCssParameter {
            family: family_name.to_owned(),
            parameter: parameter.to_owned(),
        });
    }

    let mut terms = Vec::new();
    for term in value.split('|') {
        let Some((dx_text, dy_text)) = term.split_once(':') else {
            return Err(QecError::InvalidBuiltInCssIntegerParameter {
                family: family_name.to_owned(),
                parameter: parameter.to_owned(),
                value: term.to_owned(),
            });
        };

        let dx =
            dx_text
                .parse::<usize>()
                .map_err(|_| QecError::InvalidBuiltInCssIntegerParameter {
                    family: family_name.to_owned(),
                    parameter: parameter.to_owned(),
                    value: dx_text.to_owned(),
                })?;
        let dy =
            dy_text
                .parse::<usize>()
                .map_err(|_| QecError::InvalidBuiltInCssIntegerParameter {
                    family: family_name.to_owned(),
                    parameter: parameter.to_owned(),
                    value: dy_text.to_owned(),
                })?;

        terms.push((dx, dy));
    }

    *slot = Some(terms);
    Ok(())
}

const STEANE_ROW_SUPPORTS: &[&[usize]] = &[&[0, 3, 5, 6], &[1, 3, 4, 6], &[2, 4, 5, 6]];

const BB72_LX: usize = 6;
const BB72_LY: usize = 6;
const BB72_A_TERMS: &[(usize, usize)] = &[(3, 0), (0, 1), (0, 2)];
const BB72_B_TERMS: &[(usize, usize)] = &[(0, 3), (1, 0), (2, 0)];

fn bb72_checks() -> Result<BuiltInCssChecks> {
    let mut checks = bivariate_bicycle_css_checks(BivariateBicycleParams {
        lx: BB72_LX,
        ly: BB72_LY,
        a_terms: BB72_A_TERMS.to_vec(),
        b_terms: BB72_B_TERMS.to_vec(),
    })?;
    checks.code_id = "bb72";
    Ok(checks)
}

fn bivariate_bicycle_checks(
    lx: usize,
    ly: usize,
    a_terms: &[(usize, usize)],
    b_terms: &[(usize, usize)],
) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let block = lx * ly;
    let index = |x: usize, y: usize| -> usize { (x % lx) * ly + (y % ly) };
    let periodic_add = |coord: usize, shift: usize, period: usize| -> usize {
        if shift == 0 {
            coord
        } else if coord >= period - shift {
            coord - (period - shift)
        } else {
            coord + shift
        }
    };
    let mut hx = Vec::with_capacity(block);
    let mut hz = Vec::with_capacity(block);

    for x in 0..lx {
        for y in 0..ly {
            let mut x_row = Vec::new();
            for &(dx, dy) in a_terms {
                let dx = dx % lx;
                let dy = dy % ly;
                x_row.push(index(periodic_add(x, dx, lx), periodic_add(y, dy, ly)));
            }
            for &(dx, dy) in b_terms {
                let dx = dx % lx;
                let dy = dy % ly;
                x_row.push(block + index(periodic_add(x, dx, lx), periodic_add(y, dy, ly)));
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

pub fn bivariate_bicycle_css_checks(params: BivariateBicycleParams) -> Result<BuiltInCssChecks> {
    validate_bivariate_bicycle_params(&params)?;

    let (hx, hz) = bivariate_bicycle_checks(params.lx, params.ly, &params.a_terms, &params.b_terms);

    Ok(BuiltInCssChecks {
        code_id: "bb",
        num_cols: 2 * params.lx * params.ly,
        hx,
        hz,
    })
}

fn validate_bivariate_bicycle_params(params: &BivariateBicycleParams) -> Result<()> {
    if params.lx == 0 {
        return Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "bb".to_owned(),
            parameter: "lx".to_owned(),
            value: 0,
        });
    }

    if params.ly == 0 {
        return Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "bb".to_owned(),
            parameter: "ly".to_owned(),
            value: 0,
        });
    }

    validate_bivariate_bicycle_terms("bb", "a_terms", params.lx, params.ly, &params.a_terms)?;
    validate_bivariate_bicycle_terms("bb", "b_terms", params.lx, params.ly, &params.b_terms)?;
    Ok(())
}

fn validate_bivariate_bicycle_terms(
    family: &'static str,
    parameter: &'static str,
    lx: usize,
    ly: usize,
    terms: &[(usize, usize)],
) -> Result<()> {
    if terms.is_empty() {
        return Err(QecError::MissingBuiltInCssParameter {
            family: family.to_owned(),
            parameter: parameter.to_owned(),
        });
    }

    let mut seen = HashSet::new();
    for &(dx, dy) in terms {
        let normalized = (dx % lx, dy % ly);
        if !seen.insert(normalized) {
            return Err(QecError::DuplicateBuiltInCssParameter {
                family: family.to_owned(),
                parameter: parameter.to_owned(),
            });
        }
    }

    Ok(())
}

pub fn built_in_css_checks(code_id: &str) -> Result<BuiltInCssChecks> {
    match parse_built_in_css_code_spec(code_id)? {
        BuiltInCssCodeSpec::Fixed { code_id } => fixed_built_in_css_checks(code_id),
        BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::BivariateBicycle,
            ..
        } => Err(QecError::UnknownBuiltInCssCode {
            code_id: code_id.to_owned(),
        }),
        BuiltInCssCodeSpec::Family { family, params } => family_css_checks(family, params),
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
        "bb72" => bb72_checks(),
        _ => Err(QecError::UnknownBuiltInCssCode {
            code_id: code_id.to_owned(),
        }),
    }
}

fn family_css_checks(
    family: BuiltInCssFamily,
    params: BuiltInCssParams,
) -> Result<BuiltInCssChecks> {
    match family {
        BuiltInCssFamily::RepetitionX => {
            let BuiltInCssParams::Distance { distance } = params else {
                unreachable!("repetition_x only uses distance params");
            };
            let hx = chain_supports("repetition_x", distance)?;
            Ok(BuiltInCssChecks {
                code_id: "repetition_x",
                num_cols: distance,
                hx,
                hz: vec![],
            })
        }
        BuiltInCssFamily::RepetitionZ => {
            let BuiltInCssParams::Distance { distance } = params else {
                unreachable!("repetition_z only uses distance params");
            };
            let hz = chain_supports("repetition_z", distance)?;
            Ok(BuiltInCssChecks {
                code_id: "repetition_z",
                num_cols: distance,
                hx: vec![],
                hz,
            })
        }
        BuiltInCssFamily::SurfaceRotated => {
            let BuiltInCssParams::Distance { distance } = params else {
                unreachable!("surface_rotated only uses distance params");
            };
            surface_rotated_css_checks(distance)
        }
        BuiltInCssFamily::Toric => {
            let BuiltInCssParams::Distance { distance } = params else {
                unreachable!("toric only uses distance params");
            };
            toric_css_checks(distance)
        }
        BuiltInCssFamily::BivariateBicycle => unreachable!("bb specs are parser-only here"),
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

fn surface_rotated_css_checks(distance: usize) -> Result<BuiltInCssChecks> {
    if distance < 2 {
        return Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "surface_rotated".to_owned(),
            parameter: "d".to_owned(),
            value: distance,
        });
    }

    let (hx, hz) = rotated_surface_supports(distance);

    Ok(BuiltInCssChecks {
        code_id: "surface_rotated",
        num_cols: distance * distance,
        hx,
        hz,
    })
}

fn rotated_surface_supports(distance: usize) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let mut hx = Vec::new();
    let mut hz = Vec::new();

    for ax in 0..=distance {
        for ay in 0..=distance {
            let on_boundary_1 = ax == 0 || ax == distance;
            let on_boundary_2 = ay == 0 || ay == distance;
            let parity = (ax % 2) != (ay % 2);
            if on_boundary_1 && parity {
                continue;
            }
            if on_boundary_2 && !parity {
                continue;
            }

            let support = rotated_surface_measure_support(distance, ax, ay);
            if support.is_empty() {
                continue;
            }

            if parity {
                hx.push(support);
            } else {
                hz.push(support);
            }
        }
    }

    (hx, hz)
}

fn rotated_surface_measure_support(distance: usize, ax: usize, ay: usize) -> Vec<usize> {
    let mut support = Vec::new();
    let mx = (2 * ax) as isize;
    let my = (2 * ay) as isize;

    for (dx, dy) in [(1isize, 1isize), (1, -1), (-1, 1), (-1, -1)] {
        let x = mx + dx;
        let y = my + dy;
        if x >= 1
            && x <= (2 * distance - 1) as isize
            && y >= 1
            && y <= (2 * distance - 1) as isize
            && x % 2 == 1
            && y % 2 == 1
        {
            let qx = ((x - 1) / 2) as usize;
            let qy = ((y - 1) / 2) as usize;
            if qx < distance && qy < distance {
                support.push(rotated_surface_data_index(distance, qx, qy));
            }
        }
    }

    support.sort_unstable();
    support.dedup();
    support
}

fn rotated_surface_data_index(distance: usize, x: usize, y: usize) -> usize {
    x * distance + y
}

fn toric_css_checks(distance: usize) -> Result<BuiltInCssChecks> {
    if distance < 2 {
        return Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "toric".to_owned(),
            parameter: "d".to_owned(),
            value: distance,
        });
    }

    let (hx, hz) = toric_supports(distance);

    Ok(BuiltInCssChecks {
        code_id: "toric",
        num_cols: 2 * distance * distance,
        hx,
        hz,
    })
}

fn toric_supports(distance: usize) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let mut hx = Vec::with_capacity(distance * distance);
    let mut hz = Vec::with_capacity(distance * distance);

    for x in 0..distance {
        for y in 0..distance {
            hx.push(toric_x_check_support(distance, x, y));
            hz.push(toric_z_check_support(distance, x, y));
        }
    }

    (hx, hz)
}

fn toric_x_check_support(distance: usize, x: usize, y: usize) -> Vec<usize> {
    sorted_toric_row([
        toric_horizontal_index(distance, x, y),
        toric_horizontal_index(distance, x, wrap_prev(y, distance)),
        toric_vertical_index(distance, x, y),
        toric_vertical_index(distance, wrap_prev(x, distance), y),
    ])
}

fn toric_z_check_support(distance: usize, x: usize, y: usize) -> Vec<usize> {
    sorted_toric_row([
        toric_horizontal_index(distance, x, y),
        toric_horizontal_index(distance, wrap_next(x, distance), y),
        toric_vertical_index(distance, x, y),
        toric_vertical_index(distance, x, wrap_next(y, distance)),
    ])
}

fn sorted_toric_row(mut row: [usize; 4]) -> Vec<usize> {
    row.sort_unstable();
    row.to_vec()
}

fn toric_horizontal_index(distance: usize, x: usize, y: usize) -> usize {
    x * distance + y
}

fn toric_vertical_index(distance: usize, x: usize, y: usize) -> usize {
    distance * distance + x * distance + y
}

fn wrap_prev(value: usize, distance: usize) -> usize {
    (value + distance - 1) % distance
}

fn wrap_next(value: usize, distance: usize) -> usize {
    (value + 1) % distance
}
