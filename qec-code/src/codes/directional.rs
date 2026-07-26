use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::error::{QecError, Result};

pub const DIRECTIONAL_CSS_CONSTRUCTION_ID: &str = "directional";

const HEX_COMPATIBLE_NORMALIZED_ROUTES: &[&str] = &["NE3N"];

type Coordinate = (i64, i64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectionalCssSpec {
    pub torus: DirectionalTorusSpec,
    pub route: String,
    #[serde(default)]
    pub layout: DirectionalLayoutSpec,
    #[serde(default)]
    pub connectivity: DirectionalConnectivity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectionalTorusSpec {
    pub period_x: usize,
    pub period_y: usize,
    #[serde(default)]
    pub vertical_period_x_shift: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectionalLayoutSpec {
    #[serde(default = "default_x_ancilla_coset")]
    pub x_ancilla_coset: DirectionalAncillaCoset,
    #[serde(default = "default_z_ancilla_coset")]
    pub z_ancilla_coset: DirectionalAncillaCoset,
}

impl Default for DirectionalLayoutSpec {
    fn default() -> Self {
        Self {
            x_ancilla_coset: default_x_ancilla_coset(),
            z_ancilla_coset: default_z_ancilla_coset(),
        }
    }
}

fn default_x_ancilla_coset() -> DirectionalAncillaCoset {
    DirectionalAncillaCoset::OddEven
}

fn default_z_ancilla_coset() -> DirectionalAncillaCoset {
    DirectionalAncillaCoset::EvenOdd
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectionalAncillaCoset {
    OddEven,
    EvenOdd,
}

impl DirectionalAncillaCoset {
    fn contains(self, (x, y): Coordinate) -> bool {
        match self {
            Self::OddEven => x.rem_euclid(2) == 1 && y.rem_euclid(2) == 0,
            Self::EvenOdd => x.rem_euclid(2) == 0 && y.rem_euclid(2) == 1,
        }
    }

    fn translated(self, (x, y): Coordinate) -> Self {
        match (self, x.rem_euclid(2), y.rem_euclid(2)) {
            (Self::OddEven, 0, 0) | (Self::EvenOdd, 0, 0) => self,
            (Self::OddEven, 1, 1) => Self::EvenOdd,
            (Self::EvenOdd, 1, 1) => Self::OddEven,
            _ => self,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DirectionalConnectivity {
    #[default]
    Square,
    Hex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectionalCssChecks {
    pub code_id: &'static str,
    pub num_cols: usize,
    pub hx: Vec<Vec<usize>>,
    pub hz: Vec<Vec<usize>>,
    pub route_support: Vec<Coordinate>,
    pub normalized_route: String,
}

pub fn parse_directional_route_support(route: &str) -> Result<Vec<Coordinate>> {
    parse_route(route).map(|parsed| parsed.support)
}

pub fn build_directional_css_checks(spec: &DirectionalCssSpec) -> Result<DirectionalCssChecks> {
    validate_torus(&spec.torus)?;
    validate_layout(&spec.layout)?;

    let parsed_route = parse_route(&spec.route)?;
    validate_connectivity(spec.connectivity, &parsed_route.normalized)?;
    validate_infinite_support(&parsed_route.support)?;
    validate_odd_overlap(&parsed_route.support, &spec.layout)?;
    validate_finite_torus(&parsed_route.support, &spec.torus)?;

    let data_index = data_index(&spec.torus)?;
    let hx = build_check_rows(
        spec.layout.x_ancilla_coset,
        &parsed_route.support,
        &spec.torus,
        &data_index,
    )?;
    let hz = build_check_rows(
        spec.layout.z_ancilla_coset,
        &parsed_route.support,
        &spec.torus,
        &data_index,
    )?;

    Ok(DirectionalCssChecks {
        code_id: DIRECTIONAL_CSS_CONSTRUCTION_ID,
        num_cols: data_index.len(),
        hx,
        hz,
        route_support: parsed_route.support,
        normalized_route: parsed_route.normalized,
    })
}

#[derive(Debug)]
struct ParsedRoute {
    support: Vec<Coordinate>,
    normalized: String,
}

fn parse_route(route: &str) -> Result<ParsedRoute> {
    if route.is_empty() {
        return invalid_route(route, "route must contain at least one direction");
    }

    let chars: Vec<char> = route.chars().collect();
    let mut index = 0;
    let mut previous = (0_i64, 0_i64);
    let mut support = Vec::new();
    let mut normalized_runs: Vec<(char, usize)> = Vec::new();
    while index < chars.len() {
        let direction = chars[index];
        let displacement = match direction {
            'N' => (0, 1),
            'E' => (1, 0),
            'S' => (0, -1),
            'W' => (-1, 0),
            _ => return invalid_route(route, format!("unexpected symbol {direction:?}")),
        };
        index += 1;

        let digits_start = index;
        while index < chars.len() && chars[index].is_ascii_digit() {
            index += 1;
        }
        let repetitions = if digits_start == index {
            1
        } else {
            let digits: String = chars[digits_start..index].iter().collect();
            let repetitions =
                digits
                    .parse::<usize>()
                    .map_err(|_| QecError::InvalidDirectionalRoute {
                        route: route.to_owned(),
                        reason: format!("repetition suffix {digits:?} is out of range"),
                    })?;
            if repetitions == 0 {
                return invalid_route(route, "repetition suffix must be positive");
            }
            repetitions
        };
        if normalized_runs
            .last()
            .is_some_and(|(last_direction, _)| *last_direction == direction)
        {
            let (_, last_repetitions) = normalized_runs
                .last_mut()
                .expect("last normalized route run should exist");
            *last_repetitions = last_repetitions.checked_add(repetitions).ok_or_else(|| {
                QecError::InvalidDirectionalRoute {
                    route: route.to_owned(),
                    reason: "normalized route repetition overflow".to_owned(),
                }
            })?;
        } else {
            normalized_runs.push((direction, repetitions));
        }

        for _ in 0..repetitions {
            let offset = (
                previous
                    .0
                    .checked_mul(2)
                    .and_then(|x| x.checked_add(displacement.0)),
                previous
                    .1
                    .checked_mul(2)
                    .and_then(|y| y.checked_add(displacement.1)),
            );
            let (Some(x), Some(y)) = offset else {
                return invalid_route(route, "support offset overflow");
            };
            support.push((x, y));
            previous.0 = previous.0.checked_add(displacement.0).ok_or_else(|| {
                QecError::InvalidDirectionalRoute {
                    route: route.to_owned(),
                    reason: "route displacement overflow".to_owned(),
                }
            })?;
            previous.1 = previous.1.checked_add(displacement.1).ok_or_else(|| {
                QecError::InvalidDirectionalRoute {
                    route: route.to_owned(),
                    reason: "route displacement overflow".to_owned(),
                }
            })?;
        }
    }
    let mut normalized = String::new();
    for (direction, repetitions) in normalized_runs {
        normalized.push(direction);
        if repetitions > 1 {
            normalized.push_str(&repetitions.to_string());
        }
    }

    Ok(ParsedRoute {
        support,
        normalized,
    })
}

fn invalid_route<T>(route: &str, reason: impl Into<String>) -> Result<T> {
    Err(QecError::InvalidDirectionalRoute {
        route: route.to_owned(),
        reason: reason.into(),
    })
}

fn validate_torus(torus: &DirectionalTorusSpec) -> Result<()> {
    if torus.period_x == 0 || torus.period_x % 2 != 0 {
        return invalid_spec("period_x must be positive and even");
    }
    if torus.period_y == 0 || torus.period_y % 2 != 0 {
        return invalid_spec("period_y must be positive and even");
    }
    if torus.vertical_period_x_shift % 2 != 0 {
        return invalid_spec(
            "vertical_period_x_shift must be even to preserve checkerboard parity",
        );
    }
    Ok(())
}

fn validate_layout(layout: &DirectionalLayoutSpec) -> Result<()> {
    if layout.x_ancilla_coset == layout.z_ancilla_coset {
        return invalid_spec("X and Z checks must use distinct ancilla cosets");
    }
    Ok(())
}

fn validate_connectivity(
    connectivity: DirectionalConnectivity,
    normalized_route: &str,
) -> Result<()> {
    if matches!(connectivity, DirectionalConnectivity::Hex)
        && !HEX_COMPATIBLE_NORMALIZED_ROUTES.contains(&normalized_route)
    {
        return invalid_spec(format!(
            "hex connectivity does not support normalized route {normalized_route}"
        ));
    }
    Ok(())
}

fn validate_infinite_support(support: &[Coordinate]) -> Result<()> {
    let unique: BTreeSet<_> = support.iter().copied().collect();
    if unique.len() != support.len() {
        return invalid_spec("route support contains duplicate offsets");
    }
    Ok(())
}

fn validate_odd_overlap(support: &[Coordinate], layout: &DirectionalLayoutSpec) -> Result<()> {
    let mut delta_counts = BTreeMap::new();
    for &left in support {
        for &right in support {
            if left != right {
                *delta_counts.entry(subtract(left, right)).or_insert(0_usize) += 1;
            }
        }
    }

    for (delta, count) in delta_counts {
        if count % 2 == 1 && layout.x_ancilla_coset.translated(delta) == layout.z_ancilla_coset {
            return invalid_spec(format!(
                "odd route-overlap delta ({}, {}) conflicts with the selected ancilla layout",
                delta.0, delta.1
            ));
        }
    }
    Ok(())
}

fn validate_finite_torus(support: &[Coordinate], torus: &DirectionalTorusSpec) -> Result<()> {
    let reduced: BTreeSet<_> = support
        .iter()
        .map(|&coordinate| reduce_coordinate(coordinate, torus))
        .collect::<Result<_>>()?;
    if reduced.len() != support.len() {
        return invalid_spec("the finite torus identifies route support offsets");
    }

    let deltas: BTreeSet<_> = support
        .iter()
        .enumerate()
        .flat_map(|(index, &left)| {
            support[index + 1..]
                .iter()
                .map(move |&right| subtract(left, right))
        })
        .collect();
    for &delta in &deltas {
        if in_period_lattice(delta, torus)? {
            return invalid_spec("a route delta is in the torus period lattice");
        }
    }
    for &u in &deltas {
        for &w in &deltas {
            if u == w {
                continue;
            }
            for collision in [add(u, w), subtract(u, w)] {
                if collision != (0, 0) && in_period_lattice(collision, torus)? {
                    return invalid_spec("route delta vectors collide on the finite torus");
                }
            }
        }
    }
    Ok(())
}

fn data_index(torus: &DirectionalTorusSpec) -> Result<BTreeMap<Coordinate, usize>> {
    let period_x =
        i64::try_from(torus.period_x).map_err(|_| QecError::InvalidDirectionalCssSpec {
            reason: "period_x is too large".to_owned(),
        })?;
    let period_y =
        i64::try_from(torus.period_y).map_err(|_| QecError::InvalidDirectionalCssSpec {
            reason: "period_y is too large".to_owned(),
        })?;
    let mut data_index = BTreeMap::new();
    for y in 0..period_y {
        for x in 0..period_x {
            if (x + y).rem_euclid(2) == 0 {
                let next = data_index.len();
                data_index.insert((x, y), next);
            }
        }
    }
    Ok(data_index)
}

fn build_check_rows(
    selected_coset: DirectionalAncillaCoset,
    support: &[Coordinate],
    torus: &DirectionalTorusSpec,
    data_index: &BTreeMap<Coordinate, usize>,
) -> Result<Vec<Vec<usize>>> {
    let period_x =
        i64::try_from(torus.period_x).map_err(|_| QecError::InvalidDirectionalCssSpec {
            reason: "period_x is too large".to_owned(),
        })?;
    let period_y =
        i64::try_from(torus.period_y).map_err(|_| QecError::InvalidDirectionalCssSpec {
            reason: "period_y is too large".to_owned(),
        })?;
    let mut rows = Vec::new();
    for y in 0..period_y {
        for x in 0..period_x {
            let ancilla = (x, y);
            if !selected_coset.contains(ancilla) {
                continue;
            }
            let mut row = Vec::with_capacity(support.len());
            for &offset in support {
                let data = reduce_coordinate(add(ancilla, offset), torus)?;
                let column = data_index.get(&data).copied().ok_or_else(|| {
                    QecError::InvalidDirectionalCssSpec {
                        reason: format!(
                            "route support maps ancilla ({x}, {y}) to non-data coordinate ({}, {})",
                            data.0, data.1
                        ),
                    }
                })?;
                row.push(column);
            }
            row.sort_unstable();
            if row.windows(2).any(|pair| pair[0] == pair[1]) {
                return invalid_spec("a generated finite-torus check has duplicate support");
            }
            rows.push(row);
        }
    }
    Ok(rows)
}

fn reduce_coordinate((x, y): Coordinate, torus: &DirectionalTorusSpec) -> Result<Coordinate> {
    let period_x =
        i64::try_from(torus.period_x).map_err(|_| QecError::InvalidDirectionalCssSpec {
            reason: "period_x is too large".to_owned(),
        })?;
    let period_y =
        i64::try_from(torus.period_y).map_err(|_| QecError::InvalidDirectionalCssSpec {
            reason: "period_y is too large".to_owned(),
        })?;
    let shift = i64::try_from(torus.vertical_period_x_shift).map_err(|_| {
        QecError::InvalidDirectionalCssSpec {
            reason: "vertical_period_x_shift is too large".to_owned(),
        }
    })?;
    let vertical_periods = y.div_euclid(period_y);
    let reduced_x = x
        .checked_sub(vertical_periods.checked_mul(shift).ok_or_else(|| {
            QecError::InvalidDirectionalCssSpec {
                reason: "coordinate reduction overflow".to_owned(),
            }
        })?)
        .ok_or_else(|| QecError::InvalidDirectionalCssSpec {
            reason: "coordinate reduction overflow".to_owned(),
        })?
        .rem_euclid(period_x);
    Ok((reduced_x, y.rem_euclid(period_y)))
}

fn in_period_lattice((x, y): Coordinate, torus: &DirectionalTorusSpec) -> Result<bool> {
    let period_x =
        i64::try_from(torus.period_x).map_err(|_| QecError::InvalidDirectionalCssSpec {
            reason: "period_x is too large".to_owned(),
        })?;
    let period_y =
        i64::try_from(torus.period_y).map_err(|_| QecError::InvalidDirectionalCssSpec {
            reason: "period_y is too large".to_owned(),
        })?;
    let shift = i64::try_from(torus.vertical_period_x_shift).map_err(|_| {
        QecError::InvalidDirectionalCssSpec {
            reason: "vertical_period_x_shift is too large".to_owned(),
        }
    })?;
    if y.rem_euclid(period_y) != 0 {
        return Ok(false);
    }
    let vertical_periods = y.div_euclid(period_y);
    let horizontal_remainder = x
        .checked_sub(vertical_periods.checked_mul(shift).ok_or_else(|| {
            QecError::InvalidDirectionalCssSpec {
                reason: "period lattice overflow".to_owned(),
            }
        })?)
        .ok_or_else(|| QecError::InvalidDirectionalCssSpec {
            reason: "period lattice overflow".to_owned(),
        })?;
    Ok(horizontal_remainder.rem_euclid(period_x) == 0)
}

fn add(left: Coordinate, right: Coordinate) -> Coordinate {
    (left.0 + right.0, left.1 + right.1)
}

fn subtract(left: Coordinate, right: Coordinate) -> Coordinate {
    (left.0 - right.0, left.1 - right.1)
}

fn invalid_spec<T>(reason: impl Into<String>) -> Result<T> {
    Err(QecError::InvalidDirectionalCssSpec {
        reason: reason.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square_spec(route: &str) -> DirectionalCssSpec {
        DirectionalCssSpec {
            torus: DirectionalTorusSpec {
                period_x: 8,
                period_y: 6,
                vertical_period_x_shift: 4,
            },
            route: route.to_owned(),
            layout: DirectionalLayoutSpec::default(),
            connectivity: DirectionalConnectivity::Square,
        }
    }

    #[test]
    fn parses_repeated_route_with_paper_offsets() {
        assert_eq!(
            parse_directional_route_support("NE2N").unwrap(),
            vec![(0, 1), (1, 2), (3, 2), (4, 3)]
        );
        assert_eq!(
            parse_directional_route_support("NE2EN").unwrap(),
            vec![(0, 1), (1, 2), (3, 2), (5, 2), (6, 3)]
        );
        assert!(parse_directional_route_support("N0E").is_err());
        assert!(parse_directional_route_support("NX").is_err());
    }

    #[test]
    fn generates_square_ne2n_checks_in_hardware_order() {
        let checks = build_directional_css_checks(&square_spec("NE2N")).unwrap();

        assert_eq!(checks.num_cols, 24);
        assert_eq!(checks.hx[0], vec![4, 9, 10, 14]);
        assert_eq!(checks.hz[0], vec![8, 12, 13, 18]);
    }

    #[test]
    fn generates_hex_ne3n_checks_in_hardware_order() {
        let spec = DirectionalCssSpec {
            torus: DirectionalTorusSpec {
                period_x: 18,
                period_y: 4,
                vertical_period_x_shift: 0,
            },
            route: "NE3N".to_owned(),
            layout: DirectionalLayoutSpec::default(),
            connectivity: DirectionalConnectivity::Hex,
        };
        let checks = build_directional_css_checks(&spec).unwrap();

        assert_eq!(checks.num_cols, 36);
        assert_eq!(checks.hx[0], vec![9, 19, 20, 21, 30]);
        assert_eq!(checks.hz[0], vec![3, 18, 27, 28, 29]);
    }

    #[test]
    fn canonicalizes_route_spellings_before_hex_compatibility() {
        let canonical = build_directional_css_checks(&DirectionalCssSpec {
            torus: DirectionalTorusSpec {
                period_x: 18,
                period_y: 4,
                vertical_period_x_shift: 0,
            },
            route: "NE3N".to_owned(),
            layout: DirectionalLayoutSpec::default(),
            connectivity: DirectionalConnectivity::Hex,
        })
        .unwrap();

        for route in ["NEEEN", "NE2EN"] {
            let checks = build_directional_css_checks(&DirectionalCssSpec {
                route: route.to_owned(),
                torus: DirectionalTorusSpec {
                    period_x: 18,
                    period_y: 4,
                    vertical_period_x_shift: 0,
                },
                layout: DirectionalLayoutSpec::default(),
                connectivity: DirectionalConnectivity::Hex,
            })
            .unwrap();

            assert_eq!(checks.normalized_route, "NE3N");
            assert_eq!(checks.route_support, canonical.route_support);
            assert_eq!(checks.hx, canonical.hx);
            assert_eq!(checks.hz, canonical.hz);
        }
    }

    #[test]
    fn rejects_invalid_directional_specs() {
        assert!(build_directional_css_checks(&square_spec("NE")).is_err());
        assert!(
            build_directional_css_checks(&DirectionalCssSpec {
                connectivity: DirectionalConnectivity::Hex,
                route: "NE2N".to_owned(),
                ..square_spec("NE2N")
            })
            .is_err()
        );
        assert!(
            build_directional_css_checks(&DirectionalCssSpec {
                torus: DirectionalTorusSpec {
                    period_x: 8,
                    period_y: 6,
                    vertical_period_x_shift: 1,
                },
                ..square_spec("NE2N")
            })
            .is_err()
        );
    }

    #[test]
    fn generates_checks_for_a_swapped_valid_layout() {
        let checks = build_directional_css_checks(&DirectionalCssSpec {
            layout: DirectionalLayoutSpec {
                x_ancilla_coset: DirectionalAncillaCoset::EvenOdd,
                z_ancilla_coset: DirectionalAncillaCoset::OddEven,
            },
            ..square_spec("NE2N")
        })
        .unwrap();

        assert_eq!(checks.hx.len(), 12);
        assert_eq!(checks.hz.len(), 12);
        assert_eq!(checks.hx[0], vec![8, 12, 13, 18]);
    }
}
