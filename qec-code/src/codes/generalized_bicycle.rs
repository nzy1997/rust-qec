use serde::{Deserialize, Serialize};

use crate::error::{QecError, Result};
use crate::sparse_gf2::SparseGf2Matrix;

pub const GENERALIZED_BICYCLE_CONSTRUCTION_ID: &str = "generalized_bicycle";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneralizedBicycleSpec {
    pub order: usize,
    pub a_exponents: Vec<usize>,
    pub b_exponents: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralizedBicycleSparseChecks {
    pub num_cols: usize,
    pub h_x: Vec<Vec<usize>>,
    pub h_z: Vec<Vec<usize>>,
    pub normalized_spec: GeneralizedBicycleSpec,
}

pub fn generalized_bicycle_sparse_checks(
    spec: &GeneralizedBicycleSpec,
) -> Result<GeneralizedBicycleSparseChecks> {
    let normalized_spec = normalize_spec(spec)?;
    let a = cyclic_circulant(normalized_spec.order, &normalized_spec.a_exponents)?;
    let b = cyclic_circulant(normalized_spec.order, &normalized_spec.b_exponents)?;
    let h_x = a.hconcat(&b)?;
    let h_z = b.transpose()?.hconcat(&a.transpose()?)?;

    Ok(GeneralizedBicycleSparseChecks {
        num_cols: h_x.num_cols(),
        h_x: h_x.rows().to_vec(),
        h_z: h_z.rows().to_vec(),
        normalized_spec,
    })
}

pub fn generalized_bicycle_known_distances(
    spec: &GeneralizedBicycleSpec,
) -> Option<(usize, usize)> {
    (spec.order == 5 && spec.a_exponents == [0, 1] && spec.b_exponents == [0, 2]).then_some((3, 3))
}

fn normalize_spec(spec: &GeneralizedBicycleSpec) -> Result<GeneralizedBicycleSpec> {
    if spec.order == 0 {
        return Err(invalid("order must be nonzero"));
    }
    Ok(GeneralizedBicycleSpec {
        order: spec.order,
        a_exponents: normalize_exponents("a_exponents", spec.order, &spec.a_exponents)?,
        b_exponents: normalize_exponents("b_exponents", spec.order, &spec.b_exponents)?,
    })
}

fn normalize_exponents(
    parameter: &'static str,
    order: usize,
    exponents: &[usize],
) -> Result<Vec<usize>> {
    if exponents.is_empty() {
        return Err(invalid(format!("{parameter} must not be empty")));
    }

    let mut normalized = Vec::with_capacity(exponents.len());
    for &exponent in exponents {
        if exponent >= order {
            return Err(invalid(format!(
                "{parameter} exponent {exponent} is out of range for order {order}"
            )));
        }
        normalized.push(exponent);
    }
    normalized.sort_unstable();

    for window in normalized.windows(2) {
        if window[0] == window[1] {
            return Err(invalid(format!(
                "{parameter} contains duplicate exponent {}",
                window[0]
            )));
        }
    }

    Ok(normalized)
}

fn cyclic_circulant(order: usize, exponents: &[usize]) -> Result<SparseGf2Matrix> {
    let mut rows = Vec::with_capacity(order);
    for row in 0..order {
        rows.push(
            exponents
                .iter()
                .map(|&exponent| periodic_add(row, exponent, order))
                .collect(),
        );
    }
    SparseGf2Matrix::new(order, order, rows)
}

fn periodic_add(value: usize, shift: usize, period: usize) -> usize {
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
        construction: GENERALIZED_BICYCLE_CONSTRUCTION_ID.to_owned(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order5_rows_match_issue_fixture() {
        let checks = generalized_bicycle_sparse_checks(&GeneralizedBicycleSpec {
            order: 5,
            a_exponents: vec![0, 1],
            b_exponents: vec![0, 2],
        })
        .unwrap();

        assert_eq!(checks.num_cols, 10);
        assert_eq!(
            checks.h_x,
            vec![
                vec![0, 1, 5, 7],
                vec![1, 2, 6, 8],
                vec![2, 3, 7, 9],
                vec![3, 4, 5, 8],
                vec![0, 4, 6, 9],
            ]
        );
        assert_eq!(
            checks.h_z,
            vec![
                vec![0, 3, 5, 9],
                vec![1, 4, 5, 6],
                vec![0, 2, 6, 7],
                vec![1, 3, 7, 8],
                vec![2, 4, 8, 9],
            ]
        );
    }
}
