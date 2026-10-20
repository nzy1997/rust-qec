//! Exact rational cyclotomic values for normalized and weighted observables.
use crate::scalar::ExactScalar;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};
use std::{cmp::Ordering, fmt};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldError {
    ZeroDenominator,
    DivisionByZero,
    NonReal,
    ResourceLimit { limit: u64, required: u128 },
}
impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDenominator => {
                write!(f, "rational cyclotomic coefficient has a zero denominator")
            }
            Self::DivisionByZero => write!(f, "cannot divide an exact cyclotomic value by zero"),
            Self::NonReal => write!(f, "cyclotomic value has a nonzero imaginary component"),
            Self::ResourceLimit { limit, required } => write!(
                f,
                "rational cyclotomic numerator or denominator requires {required} bits, limit is {limit}"
            ),
        }
    }
}
impl std::error::Error for FieldError {}
/// Canonical a+b*omega+c*omega^2+d*omega^3 with rational coefficients.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExactField {
    coefficients: [BigRational; 4],
}
impl ExactField {
    pub fn zero() -> Self {
        Self {
            coefficients: std::array::from_fn(|_| BigRational::zero()),
        }
    }
    pub fn one() -> Self {
        let mut value = Self::zero();
        value.coefficients[0] = BigRational::one();
        value
    }
    pub fn coefficients(&self) -> &[BigRational; 4] {
        &self.coefficients
    }
    pub fn is_zero(&self) -> bool {
        self.coefficients.iter().all(Zero::is_zero)
    }
    pub fn negated(&self) -> Self {
        Self {
            coefficients: self.coefficients.each_ref().map(|c| -c),
        }
    }
    pub fn conjugate(&self) -> Self {
        Self {
            coefficients: [
                self.coefficients[0].clone(),
                -self.coefficients[3].clone(),
                -self.coefficients[2].clone(),
                -self.coefficients[1].clone(),
            ],
        }
    }
    pub fn real_sign(&self) -> Result<Ordering, FieldError> {
        let [a, b, c, d] = &self.coefficients;
        if !c.is_zero() || *b != -d {
            return Err(FieldError::NonReal);
        }
        if a.is_zero() {
            return Ok(b.cmp(&BigRational::zero()));
        }
        if b.is_zero() || (a > &BigRational::zero()) == (b > &BigRational::zero()) {
            return Ok(a.cmp(&BigRational::zero()));
        }
        let order = (a * a).cmp(&(b * b * BigRational::from_integer(2.into())));
        Ok(if a > &BigRational::zero() {
            order
        } else {
            order.reverse()
        })
    }
    fn automorphism(&self, power: usize) -> Self {
        let mut result = Self::zero();
        for (j, c) in self.coefficients.iter().enumerate() {
            let exponent = (j * power) % 8;
            if exponent < 4 {
                result.coefficients[exponent] += c;
            } else {
                result.coefficients[exponent - 4] -= c;
            }
        }
        result
    }
}
#[derive(Clone, Copy, Debug)]
pub struct FieldArithmetic {
    max_bits: u64,
}
impl Default for FieldArithmetic {
    fn default() -> Self {
        Self { max_bits: 1 << 20 }
    }
}
impl FieldArithmetic {
    pub fn new(max_bits: u64) -> Self {
        Self { max_bits }
    }
    pub fn max_coefficient_bits(&self) -> u64 {
        self.max_bits
    }
    fn check(&self, value: ExactField) -> Result<ExactField, FieldError> {
        let required = value
            .coefficients
            .iter()
            .map(|c| {
                if c.is_zero() {
                    0
                } else {
                    c.numer().bits().max(c.denom().bits())
                }
            })
            .max()
            .unwrap_or(0);
        if required > self.max_bits {
            return Err(FieldError::ResourceLimit {
                limit: self.max_bits,
                required: required.into(),
            });
        }
        Ok(value)
    }
    pub fn value(&self, coefficients: [BigRational; 4]) -> Result<ExactField, FieldError> {
        let mut normalized = std::array::from_fn(|_| BigRational::zero());
        for (out, c) in normalized.iter_mut().zip(coefficients) {
            if c.denom().is_zero() {
                return Err(FieldError::ZeroDenominator);
            }
            *out = BigRational::new(c.numer().clone(), c.denom().clone());
        }
        self.check(ExactField {
            coefficients: normalized,
        })
    }
    pub fn from_rational(&self, value: BigRational) -> Result<ExactField, FieldError> {
        self.value([
            value,
            BigRational::zero(),
            BigRational::zero(),
            BigRational::zero(),
        ])
    }
    pub fn from_scalar(&self, value: &ExactScalar) -> Result<ExactField, FieldError> {
        if value.is_zero() {
            return self.check(ExactField::zero());
        }
        let power = i128::from(value.power());
        let required = if power >= 0 {
            u128::from(
                value
                    .coefficients()
                    .iter()
                    .map(BigInt::bits)
                    .max()
                    .unwrap_or(0),
            ) + power as u128
        } else {
            (-power) as u128 + 1
        };
        if required > u128::from(self.max_bits) {
            return Err(FieldError::ResourceLimit {
                limit: self.max_bits,
                required,
            });
        }
        let shift =
            usize::try_from(power.unsigned_abs()).map_err(|_| FieldError::ResourceLimit {
                limit: self.max_bits,
                required,
            })?;
        self.value(value.coefficients().each_ref().map(|c| {
            if power >= 0 {
                BigRational::from_integer(c << shift)
            } else {
                BigRational::new(c.clone(), BigInt::one() << shift)
            }
        }))
    }
    pub fn add(&self, left: &ExactField, right: &ExactField) -> Result<ExactField, FieldError> {
        self.check(ExactField {
            coefficients: std::array::from_fn(|j| &left.coefficients[j] + &right.coefficients[j]),
        })
    }
    pub fn subtract(
        &self,
        left: &ExactField,
        right: &ExactField,
    ) -> Result<ExactField, FieldError> {
        self.add(left, &right.negated())
    }
    pub fn multiply(
        &self,
        left: &ExactField,
        right: &ExactField,
    ) -> Result<ExactField, FieldError> {
        let mut result = ExactField::zero();
        for (j, a) in left.coefficients.iter().enumerate() {
            for (k, b) in right.coefficients.iter().enumerate() {
                if j + k < 4 {
                    result.coefficients[j + k] += a * b;
                } else {
                    result.coefficients[j + k - 4] -= a * b;
                }
            }
        }
        self.check(result)
    }
    pub fn scale(
        &self,
        value: &ExactField,
        weight: &BigRational,
    ) -> Result<ExactField, FieldError> {
        let weight = self.from_rational(weight.clone())?;
        self.multiply(value, &weight)
    }
    /// Normalize a joint exact mass by its physical history mass and apply an
    /// exact rational noise/proposal likelihood ratio.
    pub fn contribution(
        &self,
        joint: &ExactScalar,
        history: &ExactScalar,
        likelihood: &BigRational,
    ) -> Result<ExactField, FieldError> {
        let conditional = self.divide(&self.from_scalar(joint)?, &self.from_scalar(history)?)?;
        self.scale(&conditional, likelihood)
    }
    pub fn divide(
        &self,
        numerator: &ExactField,
        denominator: &ExactField,
    ) -> Result<ExactField, FieldError> {
        if denominator.is_zero() {
            return Err(FieldError::DivisionByZero);
        }
        let other = self.multiply(
            &self.multiply(&denominator.automorphism(3), &denominator.automorphism(5))?,
            &denominator.automorphism(7),
        )?;
        let norm = self.multiply(denominator, &other)?;
        debug_assert!(norm.coefficients[1..].iter().all(Zero::is_zero));
        debug_assert!(!norm.coefficients[0].is_zero());
        let product = self.multiply(numerator, &other)?;
        self.check(ExactField {
            coefficients: product
                .coefficients
                .each_ref()
                .map(|c| c / &norm.coefficients[0]),
        })
    }
}
