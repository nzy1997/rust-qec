//! Canonical scalars in Z[omega, 1/2], where omega = exp(i*pi/4).

use num_bigint::{BigInt, Sign};
use num_traits::{One, Zero};
use std::cmp::Ordering;
use std::fmt;

/// An exact value `(c[0] + c[1]*omega + c[2]*omega^2 + c[3]*omega^3) * 2^power`.
///
/// Zero has power zero. Every nonzero value has at least one odd coefficient.
/// Private fields keep equality and hashing independent of representation.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExactScalar {
    coefficients: [BigInt; 4],
    power: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScalarError {
    EmptyBitBudget,
    CoefficientLimit { required: u64, limit: u64 },
    CoefficientSizeOverflow,
    ExponentOverflow,
    NonReal,
}

impl fmt::Display for ScalarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBitBudget => write!(f, "scalar coefficient bit budget must be positive"),
            Self::CoefficientLimit { required, limit } => {
                write!(
                    f,
                    "scalar needs {required} coefficient bits; configured limit is {limit}"
                )
            }
            Self::CoefficientSizeOverflow => write!(
                f,
                "scalar coefficient bit length exceeds the unsigned 64-bit range"
            ),
            Self::ExponentOverflow => {
                write!(f, "scalar binary exponent exceeds the signed 64-bit range")
            }
            Self::NonReal => write!(f, "scalar has a nonzero imaginary part"),
        }
    }
}

impl std::error::Error for ScalarError {}

/// Checked arithmetic with an explicit coefficient/alignment bit budget.
///
/// Values use arbitrary-width integers, with no floating-point arithmetic. The
/// budget bounds canonical coefficients and addition's aligned operands. It is
/// not a process-memory cap; multiplication/sign tests use larger temporaries.
#[derive(Clone, Copy, Debug)]
pub struct ScalarArithmetic {
    max_bits: u64,
}

impl Default for ScalarArithmetic {
    fn default() -> Self {
        Self { max_bits: 1 << 20 }
    }
}

impl ExactScalar {
    pub fn zero() -> Self {
        Self {
            coefficients: std::array::from_fn(|_| BigInt::zero()),
            power: 0,
        }
    }

    pub fn one() -> Self {
        Self {
            coefficients: [
                BigInt::one(),
                BigInt::zero(),
                BigInt::zero(),
                BigInt::zero(),
            ],
            power: 0,
        }
    }

    pub fn coefficients(&self) -> &[BigInt; 4] {
        &self.coefficients
    }

    pub fn power(&self) -> i64 {
        self.power
    }

    pub fn is_zero(&self) -> bool {
        self.coefficients.iter().all(Zero::is_zero)
    }

    pub fn is_real(&self) -> bool {
        self.coefficients[2].is_zero() && self.coefficients[1] == -&self.coefficients[3]
    }

    pub fn negated(&self) -> Self {
        Self {
            coefficients: self.coefficients.each_ref().map(|c| -c),
            power: self.power,
        }
    }

    pub fn conjugate(&self) -> Self {
        let [a, b, c, d] = self.coefficients.each_ref();
        Self {
            coefficients: [a.clone(), -d, -c, -b],
            power: self.power,
        }
    }

    /// Compare an exactly real value with zero, even after severe cancellation.
    pub fn real_sign(&self) -> Result<Ordering, ScalarError> {
        if !self.is_real() {
            return Err(ScalarError::NonReal);
        }
        let [a, b, _, _] = self.coefficients.each_ref();
        let sign = |x: &BigInt| match x.sign() {
            Sign::Minus => Ordering::Less,
            Sign::NoSign => Ordering::Equal,
            Sign::Plus => Ordering::Greater,
        };
        if a.is_zero() {
            return Ok(sign(b));
        }
        if b.is_zero() || a.sign() == b.sign() {
            return Ok(sign(a));
        }
        let magnitude = (a * a).cmp(&((b * b) << 1_usize));
        Ok(if a.sign() == Sign::Minus {
            magnitude.reverse()
        } else {
            magnitude
        })
    }
}

impl ScalarArithmetic {
    pub fn new(max_coefficient_bits: u64) -> Result<Self, ScalarError> {
        if max_coefficient_bits == 0 {
            return Err(ScalarError::EmptyBitBudget);
        }
        Ok(Self {
            max_bits: max_coefficient_bits,
        })
    }

    pub fn max_coefficient_bits(&self) -> u64 {
        self.max_bits
    }

    fn check_bits(&self, bits: u64) -> Result<(), ScalarError> {
        if bits > self.max_bits {
            return Err(ScalarError::CoefficientLimit {
                required: bits,
                limit: self.max_bits,
            });
        }
        Ok(())
    }

    fn check_value(&self, value: &ExactScalar) -> Result<(), ScalarError> {
        self.check_bits(
            value
                .coefficients
                .iter()
                .map(BigInt::bits)
                .max()
                .unwrap_or(0),
        )
    }

    /// Construct and normalize a value, rejecting exponent overflow explicitly.
    pub fn scalar(
        &self,
        coefficients: [BigInt; 4],
        power: i64,
    ) -> Result<ExactScalar, ScalarError> {
        self.scalar_wide(coefficients, i128::from(power))
    }

    fn scalar_wide(
        &self,
        mut coefficients: [BigInt; 4],
        power: i128,
    ) -> Result<ExactScalar, ScalarError> {
        let Some(shift) = coefficients.iter().filter_map(BigInt::trailing_zeros).min() else {
            return Ok(ExactScalar::zero());
        };
        let power =
            i64::try_from(power + i128::from(shift)).map_err(|_| ScalarError::ExponentOverflow)?;
        let shift = usize::try_from(shift).map_err(|_| ScalarError::ExponentOverflow)?;
        for coefficient in &mut coefficients {
            *coefficient >>= shift;
        }
        let value = ExactScalar {
            coefficients,
            power,
        };
        self.check_value(&value)?;
        Ok(value)
    }

    pub fn integer(&self, value: impl Into<BigInt>) -> Result<ExactScalar, ScalarError> {
        self.scalar(
            [value.into(), BigInt::zero(), BigInt::zero(), BigInt::zero()],
            0,
        )
    }

    /// Construct `omega^phase * sqrt(2)^sqrt2_power`, including negative powers.
    pub fn omega(&self, phase: i64, sqrt2_power: i64) -> Result<ExactScalar, ScalarError> {
        let phase = phase.rem_euclid(8) as usize;
        let mut coefficients = std::array::from_fn(|_| BigInt::zero());
        coefficients[phase % 4] = BigInt::from(if phase < 4 { 1 } else { -1 });
        let value = self.scalar(coefficients, sqrt2_power.div_euclid(2))?;
        if sqrt2_power.rem_euclid(2) == 0 {
            return Ok(value);
        }
        let root2 = self.scalar([0.into(), 1.into(), 0.into(), (-1).into()], 0)?;
        self.multiply(&value, &root2)
    }

    pub fn add(&self, a: &ExactScalar, b: &ExactScalar) -> Result<ExactScalar, ScalarError> {
        self.check_value(a)?;
        self.check_value(b)?;
        if a.is_zero() {
            return Ok(b.clone());
        }
        if b.is_zero() {
            return Ok(a.clone());
        }
        let power = a.power.min(b.power);
        let align = |value: &ExactScalar| -> Result<[BigInt; 4], ScalarError> {
            let shift = u64::try_from(i128::from(value.power) - i128::from(power))
                .map_err(|_| ScalarError::ExponentOverflow)?;
            for c in &value.coefficients {
                if !c.is_zero() {
                    self.check_bits(
                        c.bits()
                            .checked_add(shift)
                            .ok_or(ScalarError::CoefficientSizeOverflow)?,
                    )?;
                }
            }
            let shift = usize::try_from(shift).map_err(|_| ScalarError::ExponentOverflow)?;
            Ok(value.coefficients.each_ref().map(|c| c << shift))
        };
        let left = align(a)?;
        let right = align(b)?;
        self.scalar(std::array::from_fn(|i| &left[i] + &right[i]), power)
    }

    pub fn subtract(&self, a: &ExactScalar, b: &ExactScalar) -> Result<ExactScalar, ScalarError> {
        self.add(a, &b.negated())
    }

    pub fn multiply(&self, a: &ExactScalar, b: &ExactScalar) -> Result<ExactScalar, ScalarError> {
        self.check_value(a)?;
        self.check_value(b)?;
        if a.is_zero() || b.is_zero() {
            return Ok(ExactScalar::zero());
        }
        let power = i128::from(a.power) + i128::from(b.power);
        let mut coefficients: [BigInt; 4] = std::array::from_fn(|_| BigInt::zero());
        for (i, x) in a.coefficients.iter().enumerate() {
            for (j, y) in b.coefficients.iter().enumerate() {
                let product = x * y;
                if i + j < 4 {
                    coefficients[(i + j) % 4] += product;
                } else {
                    coefficients[(i + j) % 4] -= product;
                }
            }
        }
        self.scalar_wide(coefficients, power)
    }

    pub fn norm_squared(&self, value: &ExactScalar) -> Result<ExactScalar, ScalarError> {
        self.multiply(value, &value.conjugate())
    }
}
