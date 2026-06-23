#![allow(dead_code)]

use std::fmt;

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

impl std::error::Error for AffinePermutationError {}

fn add_mod(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    ((lhs as u128 + rhs as u128) % modulus as u128) as u64
}

fn mul_mod(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    ((lhs as u128 * rhs as u128) % modulus as u128) as u64
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
}
