#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApmActiveRowSets {
    pub(crate) delta: Vec<usize>,
    pub(crate) gamma: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApmActiveRowSetError {
    OddBlockColumnCount { l: usize },
    EmptyActiveRows,
    EmptyHalfBlockColumnCount { l: usize },
    ActiveRowsExceedHalfBlockColumnCount { j: usize, l2: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AffinePermutation {
    modulus: u64,
    slope: u64,
    offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AffinePermutationError {
    InvalidModulus,
    NonUnitSlope { slope: u64, modulus: u64 },
    ModulusMismatch { lhs: u64, rhs: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AffineCommutationExpectation {
    Commutes,
    DoesNotCommute,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AffineCommutationCheck<'a> {
    pub(crate) code_id: &'a str,
    pub(crate) left_label: &'a str,
    pub(crate) right_label: &'a str,
    pub(crate) left: AffinePermutation,
    pub(crate) right: AffinePermutation,
    pub(crate) expected: AffineCommutationExpectation,
}

impl<'a> AffineCommutationCheck<'a> {
    pub(crate) fn new(
        code_id: &'a str,
        left_label: &'a str,
        right_label: &'a str,
        left: AffinePermutation,
        right: AffinePermutation,
        expected: AffineCommutationExpectation,
    ) -> Self {
        Self {
            code_id,
            left_label,
            right_label,
            left,
            right,
            expected,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AffineCommutationError {
    ModulusMismatch {
        code_id: String,
        left_label: String,
        right_label: String,
        lhs: u64,
        rhs: u64,
    },
    UnexpectedCommutation {
        code_id: String,
        left_label: String,
        right_label: String,
        residual: u64,
    },
    UnexpectedNoncommutation {
        code_id: String,
        left_label: String,
        right_label: String,
        residual: u64,
    },
}

pub(crate) fn build_apm_active_row_sets(
    j: usize,
    l: usize,
) -> Result<ApmActiveRowSets, ApmActiveRowSetError> {
    if l % 2 != 0 {
        return Err(ApmActiveRowSetError::OddBlockColumnCount { l });
    }

    let l2 = l / 2;
    if l2 == 0 {
        return Err(ApmActiveRowSetError::EmptyHalfBlockColumnCount { l });
    }
    if j == 0 {
        return Err(ApmActiveRowSetError::EmptyActiveRows);
    }
    if j > l2 {
        return Err(ApmActiveRowSetError::ActiveRowsExceedHalfBlockColumnCount { j, l2 });
    }

    let mut delta_set = BTreeSet::new();
    for i in 0..j {
        for k in 0..j {
            delta_set.insert((k + l2 - i) % l2);
        }
    }

    let delta = delta_set.iter().copied().collect::<Vec<_>>();
    let mut gamma = Vec::new();
    for left in 0..l2 {
        for right in 0..l2 {
            if delta_set.contains(&((left + right) % l2)) {
                gamma.push((left, right));
            }
        }
    }

    Ok(ApmActiveRowSets { delta, gamma })
}

impl AffinePermutation {
    pub(crate) fn new(
        modulus: u64,
        slope: u64,
        offset: u64,
    ) -> Result<Self, AffinePermutationError> {
        if modulus == 0 {
            return Err(AffinePermutationError::InvalidModulus);
        }

        if gcd_u64(slope % modulus, modulus) != 1 {
            return Err(AffinePermutationError::NonUnitSlope { slope, modulus });
        }

        Ok(Self {
            modulus,
            slope: slope % modulus,
            offset: offset % modulus,
        })
    }

    pub(crate) fn apply(&self, index: u64) -> u64 {
        add_mod(
            mul_mod(self.slope, index % self.modulus, self.modulus),
            self.offset,
            self.modulus,
        )
    }

    pub(crate) fn inverse(&self) -> Self {
        let inverse_slope = modular_inverse(self.slope, self.modulus)
            .expect("validated affine permutation slope must have a modular inverse");
        let inverse_offset = neg_mod(
            mul_mod(inverse_slope, self.offset, self.modulus),
            self.modulus,
        );

        Self {
            modulus: self.modulus,
            slope: inverse_slope,
            offset: inverse_offset,
        }
    }

    pub(crate) fn compose_after(&self, inner: &Self) -> Result<Self, AffinePermutationError> {
        if self.modulus != inner.modulus {
            return Err(AffinePermutationError::ModulusMismatch {
                lhs: self.modulus,
                rhs: inner.modulus,
            });
        }

        Ok(Self {
            modulus: self.modulus,
            slope: mul_mod(self.slope, inner.slope, self.modulus),
            offset: add_mod(
                mul_mod(self.slope, inner.offset, self.modulus),
                self.offset,
                self.modulus,
            ),
        })
    }

    pub(crate) fn is_unit_slope(&self) -> bool {
        gcd_u64(self.slope, self.modulus) == 1
    }

    pub(crate) fn commutation_residual(&self, other: &Self) -> Result<u64, AffinePermutationError> {
        if self.modulus != other.modulus {
            return Err(AffinePermutationError::ModulusMismatch {
                lhs: self.modulus,
                rhs: other.modulus,
            });
        }

        let lhs = add_mod(
            mul_mod(self.slope, other.offset, self.modulus),
            self.offset,
            self.modulus,
        );
        let rhs = add_mod(
            mul_mod(other.slope, self.offset, self.modulus),
            other.offset,
            self.modulus,
        );

        Ok(sub_mod(lhs, rhs, self.modulus))
    }

    pub(crate) fn commutes_with(&self, other: &Self) -> Result<bool, AffinePermutationError> {
        Ok(self.commutation_residual(other)? == 0)
    }
}

pub(crate) fn validate_affine_commutation_checks(
    checks: &[AffineCommutationCheck<'_>],
) -> Result<(), Vec<AffineCommutationError>> {
    let mut errors = Vec::new();

    for check in checks {
        match check.left.commutation_residual(&check.right) {
            Err(AffinePermutationError::ModulusMismatch { lhs, rhs }) => {
                errors.push(AffineCommutationError::ModulusMismatch {
                    code_id: check.code_id.to_owned(),
                    left_label: check.left_label.to_owned(),
                    right_label: check.right_label.to_owned(),
                    lhs,
                    rhs,
                });
            }
            Err(AffinePermutationError::InvalidModulus)
            | Err(AffinePermutationError::NonUnitSlope { .. }) => {
                unreachable!("validated affine permutations must be constructible")
            }
            Ok(residual) => match check.expected {
                AffineCommutationExpectation::Commutes if residual != 0 => {
                    errors.push(AffineCommutationError::UnexpectedNoncommutation {
                        code_id: check.code_id.to_owned(),
                        left_label: check.left_label.to_owned(),
                        right_label: check.right_label.to_owned(),
                        residual,
                    });
                }
                AffineCommutationExpectation::DoesNotCommute if residual == 0 => {
                    errors.push(AffineCommutationError::UnexpectedCommutation {
                        code_id: check.code_id.to_owned(),
                        left_label: check.left_label.to_owned(),
                        right_label: check.right_label.to_owned(),
                        residual,
                    });
                }
                _ => {}
            },
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

impl fmt::Display for AffinePermutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidModulus => {
                write!(formatter, "affine permutation modulus must be positive")
            }
            Self::NonUnitSlope { slope, modulus } => {
                write!(
                    formatter,
                    "affine slope {slope} is not a unit modulo {modulus}"
                )
            }
            Self::ModulusMismatch { lhs, rhs } => {
                write!(
                    formatter,
                    "affine permutation modulus mismatch: {lhs} != {rhs}"
                )
            }
        }
    }
}

impl fmt::Display for ApmActiveRowSetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OddBlockColumnCount { l } => {
                write!(
                    formatter,
                    "APM block column count L must be even, got L={l}"
                )
            }
            Self::EmptyActiveRows => write!(formatter, "APM active row count J must be > 0"),
            Self::EmptyHalfBlockColumnCount { l } => {
                write!(
                    formatter,
                    "APM half block column count L2 must be > 0, got L={l}"
                )
            }
            Self::ActiveRowsExceedHalfBlockColumnCount { j, l2 } => write!(
                formatter,
                "APM active row count J must be <= L/2, got J={j} and L/2={l2}"
            ),
        }
    }
}

impl fmt::Display for AffineCommutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModulusMismatch {
                code_id,
                left_label,
                right_label,
                lhs,
                rhs,
            } => write!(
                formatter,
                "{code_id}: commutation check {left_label} vs {right_label} requires shared modulus, got {lhs} and {rhs}"
            ),
            Self::UnexpectedCommutation {
                code_id,
                left_label,
                right_label,
                residual,
            } => write!(
                formatter,
                "{code_id}: commutation check {left_label} vs {right_label} unexpectedly commuted (residual {residual})"
            ),
            Self::UnexpectedNoncommutation {
                code_id,
                left_label,
                right_label,
                residual,
            } => write!(
                formatter,
                "{code_id}: commutation check {left_label} vs {right_label} unexpectedly failed to commute (residual {residual})"
            ),
        }
    }
}

impl std::error::Error for AffinePermutationError {}
impl std::error::Error for ApmActiveRowSetError {}
impl std::error::Error for AffineCommutationError {}

fn add_mod(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    ((lhs as u128 + rhs as u128) % modulus as u128) as u64
}

fn mul_mod(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    ((lhs as u128 * rhs as u128) % modulus as u128) as u64
}

fn sub_mod(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    ((lhs as u128 + modulus as u128 - rhs as u128) % modulus as u128) as u64
}

fn neg_mod(value: u64, modulus: u64) -> u64 {
    if value == 0 {
        0
    } else {
        modulus - value
    }
}

fn gcd_u64(mut lhs: u64, mut rhs: u64) -> u64 {
    while rhs != 0 {
        let next = lhs % rhs;
        lhs = rhs;
        rhs = next;
    }
    lhs
}

fn modular_inverse(value: u64, modulus: u64) -> Option<u64> {
    let mut t = 0_i128;
    let mut next_t = 1_i128;
    let mut r = modulus as i128;
    let mut next_r = value as i128;

    while next_r != 0 {
        let quotient = r / next_r;

        let new_t = t - quotient * next_t;
        t = next_t;
        next_t = new_t;

        let new_r = r - quotient * next_r;
        r = next_r;
        next_r = new_r;
    }

    if r != 1 {
        return None;
    }

    if t < 0 {
        t += modulus as i128;
    }

    Some((t % modulus as i128) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn u64_json(value: &Value) -> u64 {
        value.as_u64().unwrap()
    }

    fn apm_entry_by_code_id<'a>(manifest: &'a Value, code_id: &str) -> &'a Value {
        manifest["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["code_id"] == code_id)
            .unwrap()
    }

    fn parse_documented_affine_map(entry: &Value, label: &str, modulus: u64) -> AffinePermutation {
        let label = label.strip_prefix("column_component:").unwrap_or(label);
        let (family, index) = label.split_at(1);
        let index: usize = index.parse().unwrap();
        let map = &entry[family][index];
        let (slope, offset) = match family {
            "f" => (u64_json(&map["a"]), u64_json(&map["b"])),
            "g" => (u64_json(&map["c"]), u64_json(&map["d"])),
            _ => panic!("unknown APM family label {label}"),
        };
        AffinePermutation::new(modulus, slope, offset).unwrap()
    }

    fn commutation_check_from_pair<'a>(
        entry: &'a Value,
        code_id: &'a str,
        left_label: &'a str,
        right_label: &'a str,
        modulus: u64,
        expected: AffineCommutationExpectation,
    ) -> AffineCommutationCheck<'a> {
        AffineCommutationCheck::new(
            code_id,
            left_label,
            right_label,
            parse_documented_affine_map(entry, left_label, modulus),
            parse_documented_affine_map(entry, right_label, modulus),
            expected,
        )
    }

    #[test]
    fn affine_permutation_round_trips_and_composes() {
        let cases = [(96, (5, 41), (25, 22)), (192, (71, 127), (55, 183))];

        for (modulus, outer_params, inner_params) in cases {
            let outer = AffinePermutation::new(modulus, outer_params.0, outer_params.1).unwrap();
            let inner = AffinePermutation::new(modulus, inner_params.0, inner_params.1).unwrap();
            let inverse = outer.inverse();
            assert!(inverse.is_unit_slope());

            let composed = outer.compose_after(&inner).unwrap();
            let samples = [0, 1, modulus / 3, modulus - 1];

            for index in samples {
                assert_eq!(inverse.apply(outer.apply(index)), index);
                assert_eq!(composed.apply(index), outer.apply(inner.apply(index)));
            }
        }
    }

    #[test]
    fn affine_permutation_rejects_non_unit_slope() {
        let err = AffinePermutation::new(96, 2, 1).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("slope 2"), "{message}");
        assert!(message.contains("modulo 96"), "{message}");
    }

    #[test]
    fn affine_permutation_rejects_zero_modulus() {
        assert_eq!(
            AffinePermutation::new(0, 1, 0),
            Err(AffinePermutationError::InvalidModulus)
        );
    }

    #[test]
    fn affine_permutation_displays_validation_errors() {
        assert_eq!(
            AffinePermutationError::InvalidModulus.to_string(),
            "affine permutation modulus must be positive"
        );
        assert_eq!(
            AffinePermutationError::ModulusMismatch { lhs: 96, rhs: 192 }.to_string(),
            "affine permutation modulus mismatch: 96 != 192"
        );
    }

    #[test]
    fn affine_permutation_inverse_handles_zero_offset() {
        let permutation = AffinePermutation::new(96, 5, 0).unwrap();
        let inverse = permutation.inverse();

        assert_eq!(inverse.apply(permutation.apply(0)), 0);
        assert_eq!(inverse.apply(permutation.apply(95)), 95);
    }

    #[test]
    fn affine_permutation_rejects_modulus_mismatch_composition() {
        let lhs = AffinePermutation::new(96, 5, 41).unwrap();
        let rhs = AffinePermutation::new(192, 71, 127).unwrap();

        assert_eq!(
            lhs.compose_after(&rhs),
            Err(AffinePermutationError::ModulusMismatch { lhs: 96, rhs: 192 })
        );
    }

    #[test]
    fn modular_inverse_returns_none_for_non_unit() {
        assert_eq!(modular_inverse(2, 96), None);
    }

    #[test]
    fn affine_commutation_residual_handles_large_parameters() {
        let modulus = u64::MAX - 58;
        let lhs = AffinePermutation::new(modulus, modulus - 1, modulus - 2).unwrap();
        let rhs = AffinePermutation::new(modulus, modulus - 1, modulus - 3).unwrap();

        let residual = lhs.commutation_residual(&rhs).unwrap();
        assert_ne!(residual, 0);

        for x in [0, 1, modulus / 3, modulus - 1] {
            let lhs_rhs = lhs.apply(rhs.apply(x));
            let rhs_lhs = rhs.apply(lhs.apply(x));
            assert_eq!(lhs_rhs == rhs_lhs, residual == 0);
        }
    }

    #[test]
    fn affine_commutation_matches_table_a1() {
        let manifest: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/apm/table_a1_manifest.json"
        ))
        .unwrap();
        let p96 = apm_entry_by_code_id(&manifest, "apm_kasai:p=96");

        let mut checks = Vec::new();
        for pair in p96["required_commuting_pairs"].as_array().unwrap() {
            let modulus = u64_json(&pair["modulus"]);
            checks.push(commutation_check_from_pair(
                p96,
                "apm_kasai:p=96",
                pair["left"].as_str().unwrap(),
                pair["right"].as_str().unwrap(),
                modulus,
                AffineCommutationExpectation::Commutes,
            ));
        }
        for pair in p96["required_noncommuting_pairs"].as_array().unwrap() {
            let modulus = u64_json(&p96["P"]);
            let left_label = format!("f{}", u64_json(&pair["left_index"]));
            let right_label = format!("g{}", u64_json(&pair["right_index"]));
            checks.push(commutation_check_from_pair(
                p96,
                "apm_kasai:p=96",
                Box::leak(left_label.into_boxed_str()),
                Box::leak(right_label.into_boxed_str()),
                modulus,
                AffineCommutationExpectation::DoesNotCommute,
            ));
        }

        assert!(validate_affine_commutation_checks(&checks).is_ok());

        for check in &checks {
            let commutes = check.left.commutes_with(&check.right).unwrap();
            assert_eq!(
                commutes,
                matches!(check.expected, AffineCommutationExpectation::Commutes)
            );
            let residual = check.left.commutation_residual(&check.right).unwrap();
            for x in 0..check.left.modulus {
                let lhs_rhs = check.left.apply(check.right.apply(x));
                let rhs_lhs = check.right.apply(check.left.apply(x));
                assert_eq!(lhs_rhs == rhs_lhs, residual == 0);
            }
        }

        let mut negative_checks = checks.clone();
        negative_checks
            .iter_mut()
            .find(|check| {
                check.code_id == "apm_kasai:p=96"
                    && check.left_label == "f0"
                    && check.right_label == "g3"
            })
            .unwrap()
            .expected = AffineCommutationExpectation::Commutes;
        let errors = validate_affine_commutation_checks(&negative_checks).unwrap_err();
        let error = &errors[0];
        let message = error.to_string();
        assert!(message.contains("apm_kasai:p=96"), "{message}");
        assert!(message.contains("f0"), "{message}");
        assert!(message.contains("g3"), "{message}");
    }

    #[test]
    fn affine_commutation_rejects_modulus_mismatch() {
        let lhs = AffinePermutation::new(96, 5, 41).unwrap();
        let rhs = AffinePermutation::new(192, 71, 127).unwrap();
        assert_eq!(
            lhs.commutes_with(&rhs),
            Err(AffinePermutationError::ModulusMismatch { lhs: 96, rhs: 192 })
        );
        let checks = [AffineCommutationCheck::new(
            "apm_kasai:p=96",
            "f0",
            "g0",
            lhs,
            rhs,
            AffineCommutationExpectation::Commutes,
        )];
        let errors = validate_affine_commutation_checks(&checks).unwrap_err();
        assert!(matches!(
            errors[0],
            AffineCommutationError::ModulusMismatch {
                lhs: 96,
                rhs: 192,
                ..
            }
        ));
        assert!(errors[0].to_string().contains("apm_kasai:p=96"));
    }

    #[test]
    fn affine_commutation_validator_rejects_unexpected_commutation() {
        let left = AffinePermutation::new(96, 5, 41).unwrap();
        let right = left;
        let checks = [AffineCommutationCheck::new(
            "apm_kasai:p=96",
            "f0",
            "f0",
            left,
            right,
            AffineCommutationExpectation::DoesNotCommute,
        )];

        let errors = validate_affine_commutation_checks(&checks).unwrap_err();
        assert_eq!(
            errors[0],
            AffineCommutationError::UnexpectedCommutation {
                code_id: "apm_kasai:p=96".to_owned(),
                left_label: "f0".to_owned(),
                right_label: "f0".to_owned(),
                residual: 0,
            }
        );
        let message = errors[0].to_string();
        assert!(message.contains("apm_kasai:p=96"), "{message}");
        assert!(message.contains("f0 vs f0"), "{message}");
        assert!(message.contains("unexpectedly commuted"), "{message}");
    }

    #[test]
    fn apm_active_row_sets_reject_invalid_parameters() {
        let odd_l = build_apm_active_row_sets(1, 5).unwrap_err();
        assert_eq!(odd_l, ApmActiveRowSetError::OddBlockColumnCount { l: 5 });
        assert_eq!(
            odd_l.to_string(),
            "APM block column count L must be even, got L=5"
        );

        let empty_j = build_apm_active_row_sets(0, 2).unwrap_err();
        assert_eq!(empty_j, ApmActiveRowSetError::EmptyActiveRows);
        assert_eq!(empty_j.to_string(), "APM active row count J must be > 0");

        let empty_l2 = build_apm_active_row_sets(1, 0).unwrap_err();
        assert_eq!(
            empty_l2,
            ApmActiveRowSetError::EmptyHalfBlockColumnCount { l: 0 }
        );
        assert_eq!(
            empty_l2.to_string(),
            "APM half block column count L2 must be > 0, got L=0"
        );

        let too_many_rows = build_apm_active_row_sets(4, 6).unwrap_err();
        assert_eq!(
            too_many_rows,
            ApmActiveRowSetError::ActiveRowsExceedHalfBlockColumnCount { j: 4, l2: 3 }
        );
        assert!(
            too_many_rows.to_string().contains("J must be <= L/2"),
            "{}",
            too_many_rows
        );
    }

    #[test]
    fn apm_delta_gamma_matches_kasai_reference() {
        let manifest: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/apm/table_a1_manifest.json"
        ))
        .unwrap();
        let p96 = apm_entry_by_code_id(&manifest, "apm_kasai:p=96");

        let active_sets =
            build_apm_active_row_sets(u64_json(&p96["J"]) as usize, u64_json(&p96["L"]) as usize)
                .unwrap();

        assert_eq!(active_sets.delta, vec![0, 1, 2, 4, 5]);
        assert_eq!(
            active_sets.gamma,
            vec![
                (0, 0),
                (0, 1),
                (0, 2),
                (0, 4),
                (0, 5),
                (1, 0),
                (1, 1),
                (1, 3),
                (1, 4),
                (1, 5),
                (2, 0),
                (2, 2),
                (2, 3),
                (2, 4),
                (2, 5),
                (3, 1),
                (3, 2),
                (3, 3),
                (3, 4),
                (3, 5),
                (4, 0),
                (4, 1),
                (4, 2),
                (4, 3),
                (4, 4),
                (5, 0),
                (5, 1),
                (5, 2),
                (5, 3),
                (5, 5),
            ]
        );

        let gamma_labels = active_sets
            .gamma
            .iter()
            .map(|(left, right)| (format!("f{left}"), format!("g{right}")))
            .collect::<Vec<_>>();
        let checks = gamma_labels
            .iter()
            .map(|(left_label, right_label)| {
                commutation_check_from_pair(
                    p96,
                    "apm_kasai:p=96",
                    left_label,
                    right_label,
                    u64_json(&p96["P"]),
                    AffineCommutationExpectation::Commutes,
                )
            })
            .collect::<Vec<_>>();
        validate_affine_commutation_checks(&checks).unwrap();

        let invalid = build_apm_active_row_sets(4, 6).unwrap_err();
        assert!(
            invalid.to_string().contains("J must be <= L/2"),
            "{}",
            invalid
        );
    }
}
