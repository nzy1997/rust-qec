//! Correctly rounded exports of exact real field values, roots and scalar ratios.
use crate::field::ExactField;
use num_bigint::BigInt;
use num_traits::{One, Signed, Zero};
use std::{cmp::Ordering, fmt};

#[derive(Clone, Copy, Debug)]
pub struct NumericOptions {
    /// Conservative bit cap for each intermediate integer, including squares.
    pub max_work_bits: u64,
}
impl Default for NumericOptions {
    fn default() -> Self {
        Self {
            max_work_bits: 16 << 20,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NumericError {
    NonReal,
    NegativeSquareRoot,
    ResourceLimit { limit: u64, required: u128 },
}
impl fmt::Display for NumericError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonReal => write!(f, "numeric export requires a real cyclotomic value"),
            Self::NegativeSquareRoot => {
                write!(f, "cannot export the real square root of a negative value")
            }
            Self::ResourceLimit { limit, required } => write!(
                f,
                "numeric export requires an intermediate integer budget of at least {required} bits; max_work_bits is {limit}"
            ),
        }
    }
}
impl std::error::Error for NumericError {}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScientificReal {
    /// Signed rounded mantissa with magnitude in [1,2), or exactly zero.
    pub mantissa: f64,
    /// The represented value is mantissa times 2 to this exponent.
    pub exponent: i64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Binary64 {
    Finite(f64),
    /// Rounded binary64 is signed zero, while the exact input is nonzero.
    Underflow {
        negative: bool,
    },
    /// Rounded binary64 is signed infinity.
    Overflow {
        negative: bool,
    },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundedReal {
    pub scientific: ScientificReal,
    /// Rounded independently of the scientific mantissa to avoid double rounding.
    pub binary64: Binary64,
}

struct Real {
    a: BigInt,
    b: BigInt,
    denominator: BigInt,
    limit: u64,
}
impl Real {
    fn check(&self, required: u128) -> Result<(), NumericError> {
        if required > self.limit as u128 || required > usize::MAX as u128 {
            return Err(NumericError::ResourceLimit {
                limit: self.limit,
                required,
            });
        }
        Ok(())
    }
    fn product(&self, x: &BigInt, y: &BigInt) -> Result<BigInt, NumericError> {
        if x.is_zero() || y.is_zero() {
            return Ok(BigInt::zero());
        }
        self.check(x.bits() as u128 + y.bits() as u128)?;
        Ok(x * y)
    }
    fn shift(&self, x: &BigInt, exponent: u64) -> Result<BigInt, NumericError> {
        if x.is_zero() {
            return Ok(BigInt::zero());
        }
        self.check(x.bits() as u128 + exponent as u128)?;
        Ok(x << exponent as usize)
    }
    fn sign_pair(&self, c: &BigInt, f: &BigInt) -> Result<Ordering, NumericError> {
        if c.is_zero() {
            return Ok(f.cmp(&BigInt::zero()));
        }
        if f.is_zero() || c.sign() == f.sign() {
            return Ok(c.cmp(&BigInt::zero()));
        }
        let c2 = self.product(c, c)?;
        let f2 = self.product(f, f)?;
        let two_f2 = self.shift(&f2, 1)?;
        let order = c2.cmp(&two_f2);
        Ok(if c.is_positive() {
            order
        } else {
            order.reverse()
        })
    }
    fn from_field(value: &ExactField, options: NumericOptions) -> Result<Self, NumericError> {
        let [a, b, c, d] = value.coefficients();
        // ExactField coefficients are canonical. Compare borrowed magnitudes
        // and signs so realness validation cannot clone an over-budget input.
        if !c.is_zero()
            || b.denom() != d.denom()
            || b.numer().magnitude() != d.numer().magnitude()
            || (!b.is_zero() && b.numer().sign() == d.numer().sign())
        {
            return Err(NumericError::NonReal);
        }
        let mut result = Self {
            a: BigInt::zero(),
            b: BigInt::zero(),
            denominator: BigInt::one(),
            limit: options.max_work_bits,
        };
        for x in [a, b] {
            result.check(x.numer().bits().max(x.denom().bits()) as u128)?;
        }
        if a.denom() == b.denom() {
            result.a = a.numer().clone();
            result.b = b.numer().clone();
            result.denominator = a.denom().clone();
        } else {
            result.a = result.product(a.numer(), b.denom())?;
            result.b = result.product(b.numer(), a.denom())?;
            result.denominator = result.product(a.denom(), b.denom())?;
        }
        Ok(result)
    }
    /// Compare the positive real (or its square root) with n*2^exponent.
    fn compare(&self, n: u64, exponent: i64, square_root: bool) -> Result<Ordering, NumericError> {
        let mut numerator = BigInt::from(n);
        let mut exponent = exponent;
        if square_root {
            numerator = self.product(&numerator, &numerator)?;
            exponent = exponent.checked_mul(2).ok_or(NumericError::ResourceLimit {
                limit: self.limit,
                required: u128::MAX,
            })?;
        }
        let denominator = if exponent >= 0 {
            numerator = self.shift(&numerator, exponent as u64)?;
            BigInt::one()
        } else {
            self.shift(&BigInt::one(), exponent.unsigned_abs())?
        };
        let aq = self.product(&self.a, &denominator)?;
        let dn = self.product(&self.denominator, &numerator)?;
        self.check(aq.bits().max(dn.bits()) as u128 + 1)?;
        let c = aq - dn;
        let f = self.product(&self.b, &denominator)?;
        self.sign_pair(&c, &f)
    }
    fn exponent(&self, square_root: bool) -> Result<i64, NumericError> {
        // The nonzero integer norm A^2-2B^2 and its conjugate bound
        // |A+B*sqrt(2)|/D away from zero, including severe cancellation.
        let bound = self.a.bits().max(self.b.bits()) as u128 + self.denominator.bits() as u128 + 3;
        self.check(bound)?;
        let bound = i64::try_from(bound).map_err(|_| NumericError::ResourceLimit {
            limit: self.limit,
            required: bound,
        })?;
        if bound > (i64::MAX - 128) / 4 {
            return Err(NumericError::ResourceLimit {
                limit: self.limit,
                required: u128::MAX,
            });
        }
        let (mut lower, mut upper) = (-bound, bound);
        while upper - lower > 1 {
            let middle = lower + (upper - lower) / 2;
            if self.compare(1, middle, square_root)? != Ordering::Less {
                lower = middle;
            } else {
                upper = middle;
            }
        }
        Ok(lower)
    }
    fn rounded_integer(
        &self,
        unit: i64,
        mut lower: u64,
        mut upper: u64,
        square_root: bool,
    ) -> Result<u64, NumericError> {
        while upper - lower > 1 {
            let middle = lower + (upper - lower) / 2;
            match self.compare(middle, unit, square_root)? {
                Ordering::Equal => return Ok(middle),
                Ordering::Less => upper = middle,
                Ordering::Greater => lower = middle,
            }
        }
        let midpoint = self.compare(2 * lower + 1, unit - 1, square_root)?;
        Ok(lower
            + u64::from(
                midpoint == Ordering::Greater || (midpoint == Ordering::Equal && lower & 1 != 0),
            ))
    }
    fn export(mut self, square_root: bool) -> Result<RoundedReal, NumericError> {
        let sign = self.sign_pair(&self.a, &self.b)?;
        if square_root && sign == Ordering::Less {
            return Err(NumericError::NegativeSquareRoot);
        }
        if sign == Ordering::Equal {
            return Ok(RoundedReal {
                scientific: ScientificReal {
                    mantissa: 0.0,
                    exponent: 0,
                },
                binary64: Binary64::Finite(0.0),
            });
        }
        let negative = sign == Ordering::Less;
        if negative {
            self.a = -self.a;
            self.b = -self.b;
        }
        let exponent = self.exponent(square_root)?;
        let mut scientific_exponent = exponent;
        let mut mantissa = self.rounded_integer(exponent - 52, 1 << 52, 1 << 53, square_root)?;
        if mantissa == 1 << 53 {
            mantissa >>= 1;
            scientific_exponent += 1;
        }
        let mantissa =
            (mantissa as f64) / ((1_u64 << 52) as f64) * if negative { -1.0 } else { 1.0 };
        let scientific = ScientificReal {
            mantissa,
            exponent: scientific_exponent,
        };
        let sign_bit = u64::from(negative) << 63;
        let binary64 = if exponent > 1023 {
            Binary64::Overflow { negative }
        } else if exponent < -1075 {
            Binary64::Underflow { negative }
        } else if exponent < -1022 {
            let bits = self.rounded_integer(-1074, 0, 1 << 52, square_root)?;
            if bits == 0 {
                Binary64::Underflow { negative }
            } else {
                Binary64::Finite(f64::from_bits(sign_bit | bits))
            }
        } else {
            let n = self.rounded_integer(exponent - 52, 1 << 52, 1 << 53, square_root)?;
            let (n, exponent) = if n == 1 << 53 {
                (n >> 1, exponent + 1)
            } else {
                (n, exponent)
            };
            if exponent > 1023 {
                Binary64::Overflow { negative }
            } else {
                let bits = ((exponent + 1023) as u64) << 52 | (n - (1 << 52));
                Binary64::Finite(f64::from_bits(sign_bit | bits))
            }
        };
        Ok(RoundedReal {
            scientific,
            binary64,
        })
    }
}

/// Round a real exact value without first converting its separate coefficients.
pub fn approximate_real(
    value: &ExactField,
    options: NumericOptions,
) -> Result<RoundedReal, NumericError> {
    Real::from_field(value, options)?.export(false)
}
/// Round the nonnegative real square root using exact squared-boundary tests.
pub fn approximate_sqrt(
    value: &ExactField,
    options: NumericOptions,
) -> Result<RoundedReal, NumericError> {
    Real::from_field(value, options)?.export(true)
}

/// Failure to export an exact scalar ratio as a scientific real.
///
/// Existing field exports retain their original `NumericError` variants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScalarRatioError {
    Numeric(NumericError),
    NonPositiveDenominator,
    ExponentOverflow,
}
impl From<NumericError> for ScalarRatioError {
    fn from(error: NumericError) -> Self {
        Self::Numeric(error)
    }
}
impl fmt::Display for ScalarRatioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Numeric(error) => write!(f, "{error}"),
            Self::NonPositiveDenominator => {
                write!(
                    f,
                    "scalar ratio export requires a positive real denominator"
                )
            }
            Self::ExponentOverflow => {
                write!(
                    f,
                    "scalar ratio scientific exponent exceeds the signed 64-bit range"
                )
            }
        }
    }
}
impl std::error::Error for ScalarRatioError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Numeric(error) => Some(error),
            _ => None,
        }
    }
}

impl Real {
    fn scalar_parts(
        value: &crate::scalar::ExactScalar,
        options: NumericOptions,
    ) -> Result<(&BigInt, &BigInt), NumericError> {
        let [a, b, c, d] = value.coefficients();
        // Borrowed signs and magnitudes avoid negating or copying an input
        // before its coefficient budget is admitted.
        if !c.is_zero() || b.magnitude() != d.magnitude() || (!b.is_zero() && b.sign() == d.sign())
        {
            return Err(NumericError::NonReal);
        }
        let required = u128::from(a.bits().max(b.bits()).max(1));
        if required > u128::from(options.max_work_bits) || required > usize::MAX as u128 {
            return Err(NumericError::ResourceLimit {
                limit: options.max_work_bits,
                required,
            });
        }
        Ok((a, b))
    }

    fn difference(&self, x: &BigInt, y: &BigInt) -> Result<BigInt, NumericError> {
        self.check(u128::from(x.bits().max(y.bits())) + 1)?;
        Ok(x - y)
    }
}

/// Export a real exact scalar numerator divided by a positive real denominator.
///
/// The signed mantissa is correctly rounded to nearest, ties to even, with
/// magnitude in [1,2). Zero is exactly `{ mantissa: 0.0, exponent: 0 }`.
/// To use the native [0.5,1) convention, halve a nonzero mantissa and add one to
/// the exponent using checked or wider arithmetic.
///
/// Both coefficient vectors are validated by borrowing before any products.
/// `max_work_bits` bounds each coefficient and conservative intermediate
/// integer size. It does not include the magnitude of the separate binary
/// powers: those powers are combined only after rounding the coefficient ratio.
/// A final exponent outside i64 is a typed error; an intermediate exponent
/// difference outside i64 can still produce a valid final result.
///
/// Inputs are immutable and may be retried with a larger work budget. Even a
/// zero numerator requires a real positive denominator and admitted inputs.
/// This returns a scientific value, without a binary64 result after scaling.
pub fn approximate_scalar_ratio(
    numerator: &crate::scalar::ExactScalar,
    denominator: &crate::scalar::ExactScalar,
    options: NumericOptions,
) -> Result<ScientificReal, ScalarRatioError> {
    let (a, b) = Real::scalar_parts(numerator, options)?;
    let (c, d) = Real::scalar_parts(denominator, options)?;
    let arithmetic = Real {
        a: BigInt::zero(),
        b: BigInt::zero(),
        denominator: BigInt::one(),
        limit: options.max_work_bits,
    };
    if arithmetic.sign_pair(c, d)? != Ordering::Greater {
        return Err(ScalarRatioError::NonPositiveDenominator);
    }
    if numerator.is_zero() {
        return Ok(ScientificReal {
            mantissa: 0.0,
            exponent: 0,
        });
    }

    // Multiply by the coefficient denominator's conjugate. None of these
    // bounded products or shifts includes either original scalar power.
    let ac = arithmetic.product(a, c)?;
    let two_bd = arithmetic.shift(&arithmetic.product(b, d)?, 1)?;
    let mut real = arithmetic.difference(&ac, &two_bd)?;
    let bc = arithmetic.product(b, c)?;
    let ad = arithmetic.product(a, d)?;
    let mut root = arithmetic.difference(&bc, &ad)?;
    let c2 = arithmetic.product(c, c)?;
    let two_d2 = arithmetic.shift(&arithmetic.product(d, d)?, 1)?;
    let mut norm = arithmetic.difference(&c2, &two_d2)?;
    // A nonzero real integer pair has nonzero norm. Normalize its sign for
    // the existing exact boundary-comparison and half-even rounding kernel.
    if norm.is_negative() {
        real = -real;
        root = -root;
        norm = -norm;
    }
    let rounded = Real {
        a: real,
        b: root,
        denominator: norm,
        limit: options.max_work_bits,
    }
    .export(false)?
    .scientific;
    let exponent = i128::from(rounded.exponent) + i128::from(numerator.power())
        - i128::from(denominator.power());
    Ok(ScientificReal {
        mantissa: rounded.mantissa,
        exponent: i64::try_from(exponent).map_err(|_| ScalarRatioError::ExponentOverflow)?,
    })
}
