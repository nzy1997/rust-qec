use serde::{Deserialize, Serialize};

use crate::error::{QecError, Result};
use crate::family_contract::CssClassicalCheckSpec;
use crate::sparse_gf2::SparseGf2Matrix;

pub const LA_CROSS_CONSTRUCTION_ID: &str = "la_cross";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaCrossBoundary {
    Open,
    Periodic,
}

impl LaCrossBoundary {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Periodic => "periodic",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "open" => Ok(Self::Open),
            "periodic" => Ok(Self::Periodic),
            _ => Err(invalid(format!("unknown la_cross boundary {value}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaCrossSpec {
    pub seed_length: usize,
    pub reach: usize,
    pub boundary: LaCrossBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LaCrossClassicalCheck {
    pub spec: LaCrossSpec,
    pub check: CssClassicalCheckSpec,
}

pub(crate) fn la_cross_classical_check(spec: &LaCrossSpec) -> Result<LaCrossClassicalCheck> {
    validate_la_cross_spec(spec)?;
    let rows = match spec.boundary {
        LaCrossBoundary::Open => open_rows(spec.seed_length, spec.reach)?,
        LaCrossBoundary::Periodic => periodic_rows(spec.seed_length, spec.reach)?,
    };
    let matrix = SparseGf2Matrix::new(rows.len(), spec.seed_length, rows)?;
    Ok(LaCrossClassicalCheck {
        spec: spec.clone(),
        check: CssClassicalCheckSpec {
            num_cols: matrix.num_cols(),
            rows: matrix.rows().to_vec(),
        },
    })
}

pub(crate) fn la_cross_known_distances(spec: &LaCrossSpec) -> Option<(usize, usize)> {
    (spec.seed_length == 5 && spec.reach == 2 && spec.boundary == LaCrossBoundary::Open)
        .then_some((3, 3))
}

fn validate_la_cross_spec(spec: &LaCrossSpec) -> Result<()> {
    if spec.seed_length < 2 {
        return Err(invalid(format!(
            "seed_length must be at least 2, got {}",
            spec.seed_length
        )));
    }
    if spec.reach == 0 {
        return Err(invalid("reach must be nonzero"));
    }
    if spec.reach >= spec.seed_length {
        return Err(invalid(format!(
            "reach must be less than seed_length, got reach {} and seed_length {}",
            spec.reach, spec.seed_length
        )));
    }
    preflight_hgp_dimensions(spec)
}

fn preflight_hgp_dimensions(spec: &LaCrossSpec) -> Result<()> {
    let row_count = classical_row_count(spec);
    spec.seed_length
        .checked_mul(spec.seed_length)
        .and_then(|left| {
            row_count
                .checked_mul(row_count)
                .and_then(|right| left.checked_add(right))
        })
        .ok_or_else(|| overflow("HGP data qubit count"))?;
    row_count
        .checked_mul(spec.seed_length)
        .ok_or_else(|| overflow("HGP check count"))?;
    Ok(())
}

fn classical_row_count(spec: &LaCrossSpec) -> usize {
    match spec.boundary {
        LaCrossBoundary::Open => spec.seed_length - spec.reach,
        LaCrossBoundary::Periodic => spec.seed_length,
    }
}

fn open_rows(seed_length: usize, reach: usize) -> Result<Vec<Vec<usize>>> {
    let row_count = seed_length - reach;
    let mut rows = Vec::new();
    rows.try_reserve_exact(row_count)
        .map_err(|_| overflow("classical row allocation"))?;
    for row in 0..row_count {
        rows.push(vec![row, row + 1, row + reach]);
    }
    Ok(rows)
}

fn periodic_rows(seed_length: usize, reach: usize) -> Result<Vec<Vec<usize>>> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(seed_length)
        .map_err(|_| overflow("classical row allocation"))?;
    for row in 0..seed_length {
        rows.push(vec![
            row,
            periodic_add(row, 1, seed_length),
            periodic_add(row, reach, seed_length),
        ]);
    }
    Ok(rows)
}

fn periodic_add(value: usize, shift: usize, period: usize) -> usize {
    let shift = shift % period;
    if shift == 0 {
        value
    } else if value >= period - shift {
        value - (period - shift)
    } else {
        value + shift
    }
}

fn invalid(reason: impl Into<String>) -> QecError {
    QecError::InvalidCssConstruction {
        construction: LA_CROSS_CONSTRUCTION_ID.to_owned(),
        reason: reason.into(),
    }
}

fn overflow(operation: &'static str) -> QecError {
    invalid(format!("la_cross dimension overflow during {operation}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_5_2_classical_rows_match_issue_fixture() {
        let check = la_cross_classical_check(&LaCrossSpec {
            seed_length: 5,
            reach: 2,
            boundary: LaCrossBoundary::Open,
        })
        .unwrap();

        assert_eq!(check.check.num_cols, 5);
        assert_eq!(
            check.check.rows,
            vec![vec![0, 1, 2], vec![1, 2, 3], vec![2, 3, 4]]
        );
    }

    #[test]
    fn periodic_5_2_rows_wrap_deterministically() {
        let check = la_cross_classical_check(&LaCrossSpec {
            seed_length: 5,
            reach: 2,
            boundary: LaCrossBoundary::Periodic,
        })
        .unwrap();

        assert_eq!(
            check.check.rows,
            vec![
                vec![0, 1, 2],
                vec![1, 2, 3],
                vec![2, 3, 4],
                vec![0, 3, 4],
                vec![0, 1, 4],
            ]
        );
    }
}
