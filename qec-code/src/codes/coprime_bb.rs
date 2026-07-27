use serde::{Deserialize, Serialize};

use crate::codes::generalized_bicycle::{
    generalized_bicycle_known_distances, generalized_bicycle_sparse_checks, GeneralizedBicycleSpec,
};
use crate::error::{QecError, Result};

pub const COPRIME_BB_CONSTRUCTION_ID: &str = "coprime_bb";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoprimeBivariateBicycleSpec {
    pub l: usize,
    pub m: usize,
    pub a_exponents: Vec<usize>,
    pub b_exponents: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoprimeBivariateBicycleSparseChecks {
    pub num_cols: usize,
    pub h_x: Vec<Vec<usize>>,
    pub h_z: Vec<Vec<usize>>,
    pub normalized_spec: CoprimeBivariateBicycleSpec,
}

pub fn coprime_pi_power_index(l: usize, m: usize, exponent: usize) -> Result<(usize, usize)> {
    let normalized = normalize_periods(l, m)?;
    if exponent >= normalized.cyclic_order {
        return Err(invalid(format!(
            "pi exponent {exponent} is out of range for cyclic order {}",
            normalized.cyclic_order
        )));
    }
    Ok((exponent % l, exponent % m))
}

pub fn coprime_bb_sparse_checks(
    spec: &CoprimeBivariateBicycleSpec,
) -> Result<CoprimeBivariateBicycleSparseChecks> {
    let normalized_spec = normalize_spec(spec)?;
    let generalized_checks = generalized_bicycle_sparse_checks(&GeneralizedBicycleSpec {
        order: normalized_spec.l * normalized_spec.m,
        a_exponents: normalized_spec.a_exponents.clone(),
        b_exponents: normalized_spec.b_exponents.clone(),
    })?;

    Ok(CoprimeBivariateBicycleSparseChecks {
        num_cols: generalized_checks.num_cols,
        h_x: generalized_checks.h_x,
        h_z: generalized_checks.h_z,
        normalized_spec,
    })
}

pub fn coprime_bb_known_distances(spec: &CoprimeBivariateBicycleSpec) -> Option<(usize, usize)> {
    let normalized = normalize_periods(spec.l, spec.m).ok()?;
    generalized_bicycle_known_distances(&GeneralizedBicycleSpec {
        order: normalized.cyclic_order,
        a_exponents: spec.a_exponents.clone(),
        b_exponents: spec.b_exponents.clone(),
    })
    .or_else(|| {
        (spec.l == 3
            && spec.m == 5
            && spec.a_exponents == [0, 1, 2]
            && spec.b_exponents == [0, 2, 7])
        .then_some((6, 6))
    })
}

#[derive(Debug, Clone, Copy)]
struct NormalizedPeriods {
    cyclic_order: usize,
}

fn normalize_spec(spec: &CoprimeBivariateBicycleSpec) -> Result<CoprimeBivariateBicycleSpec> {
    let normalized_periods = normalize_periods(spec.l, spec.m)?;
    Ok(CoprimeBivariateBicycleSpec {
        l: spec.l,
        m: spec.m,
        a_exponents: normalize_exponents(
            "a_exponents",
            normalized_periods.cyclic_order,
            &spec.a_exponents,
        )?,
        b_exponents: normalize_exponents(
            "b_exponents",
            normalized_periods.cyclic_order,
            &spec.b_exponents,
        )?,
    })
}

fn normalize_periods(l: usize, m: usize) -> Result<NormalizedPeriods> {
    if l == 0 {
        return Err(invalid("l must be nonzero"));
    }
    if m == 0 {
        return Err(invalid("m must be nonzero"));
    }
    if gcd(l, m) != 1 {
        return Err(invalid(format!("periods l={l} and m={m} must be coprime")));
    }
    let cyclic_order = l
        .checked_mul(m)
        .ok_or_else(|| invalid(format!("cyclic order l={l} * m={m} overflows usize")))?;

    Ok(NormalizedPeriods { cyclic_order })
}

fn normalize_exponents(
    parameter: &'static str,
    cyclic_order: usize,
    exponents: &[usize],
) -> Result<Vec<usize>> {
    if exponents.is_empty() {
        return Err(invalid(format!("{parameter} must not be empty")));
    }

    let mut normalized = Vec::with_capacity(exponents.len());
    for &exponent in exponents {
        if exponent >= cyclic_order {
            return Err(invalid(format!(
                "{parameter} exponent {exponent} is out of range for cyclic order {cyclic_order}"
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

fn gcd(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

fn invalid(reason: impl Into<String>) -> QecError {
    QecError::InvalidCssConstruction {
        construction: COPRIME_BB_CONSTRUCTION_ID.to_owned(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_spec() -> CoprimeBivariateBicycleSpec {
        CoprimeBivariateBicycleSpec {
            l: 3,
            m: 5,
            a_exponents: vec![0, 1, 2],
            b_exponents: vec![0, 2, 7],
        }
    }

    #[test]
    fn pi_power_index_uses_coprime_period_coordinates() {
        assert_eq!(coprime_pi_power_index(3, 5, 7).unwrap(), (1, 2));
    }

    #[test]
    fn l3_m5_fixture_row_zero_matches_pi_lowering() {
        let checks = coprime_bb_sparse_checks(&fixture_spec()).unwrap();

        assert_eq!(checks.num_cols, 30);
        assert_eq!(checks.h_x[0], vec![0, 1, 2, 15, 17, 22]);
        assert_eq!(checks.h_z[0], vec![0, 8, 13, 15, 28, 29]);
        assert_eq!(checks.normalized_spec.l, 3);
        assert_eq!(checks.normalized_spec.m, 5);
        assert_eq!(checks.normalized_spec.a_exponents, vec![0, 1, 2]);
        assert_eq!(checks.normalized_spec.b_exponents, vec![0, 2, 7]);
    }

    #[test]
    fn rejects_non_coprime_periods() {
        let error = coprime_bb_sparse_checks(&CoprimeBivariateBicycleSpec {
            l: 3,
            m: 6,
            a_exponents: vec![0],
            b_exponents: vec![0],
        })
        .unwrap_err();

        assert!(matches!(
            error,
            QecError::InvalidCssConstruction { construction, reason }
                if construction == COPRIME_BB_CONSTRUCTION_ID
                    && reason == "periods l=3 and m=6 must be coprime"
        ));
    }
}
