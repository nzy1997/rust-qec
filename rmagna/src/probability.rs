//! Exact rational probabilities and explicitly rounded 64-bit occurrence tests.

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, ToPrimitive, Zero};
use std::{fmt, str::FromStr};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProbabilityError {
    InvalidSyntax,
    ZeroDenominator,
    OutOfRange,
    EmptyTokenBudget,
    TokenLimit { bytes: usize, limit: usize },
    DecimalPowerLimit { limit: u32 },
}
impl fmt::Display for ProbabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSyntax => write!(
                f,
                "probability must be a decimal, scientific decimal, or integer fraction"
            ),
            Self::ZeroDenominator => write!(f, "probability fraction has a zero denominator"),
            Self::OutOfRange => write!(f, "probability must lie between zero and one"),
            Self::EmptyTokenBudget => write!(f, "probability token byte budget must be positive"),
            Self::TokenLimit { bytes, limit } => write!(
                f,
                "probability token has {bytes} bytes; configured limit is {limit}"
            ),
            Self::DecimalPowerLimit { limit } => write!(
                f,
                "probability decimal power exceeds the configured magnitude limit {limit}"
            ),
        }
    }
}
impl std::error::Error for ProbabilityError {}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Probability(BigRational);
impl Probability {
    pub fn from_ratio(numerator: BigInt, denominator: BigInt) -> Result<Self, ProbabilityError> {
        if denominator.is_zero() {
            return Err(ProbabilityError::ZeroDenominator);
        }
        let value = BigRational::new(numerator, denominator);
        if value < BigRational::zero() || value > BigRational::one() {
            return Err(ProbabilityError::OutOfRange);
        }
        Ok(Self(value))
    }
    pub fn from_rational(value: BigRational) -> Result<Self, ProbabilityError> {
        Self::from_ratio(value.numer().clone(), value.denom().clone())
    }
    pub fn value(&self) -> &BigRational {
        &self.0
    }
    pub fn zero() -> Self {
        Self(BigRational::zero())
    }
    pub fn one() -> Self {
        Self(BigRational::one())
    }
    pub fn complement(&self) -> Self {
        Self(BigRational::one() - &self.0)
    }
    pub fn checked_sum(values: impl IntoIterator<Item = Self>) -> Result<Self, ProbabilityError> {
        let mut sum = BigRational::zero();
        for value in values {
            sum += value.0;
            if sum > BigRational::one() {
                return Err(ProbabilityError::OutOfRange);
            }
        }
        Ok(Self(sum))
    }
    pub fn scaled(&self, factor: &BigRational) -> Result<Self, ProbabilityError> {
        if factor.denom().is_zero() {
            return Err(ProbabilityError::ZeroDenominator);
        }
        let factor = BigRational::new(factor.numer().clone(), factor.denom().clone());
        Self::from_rational(&self.0 * factor)
    }

    /// File-style occurrence semantics: floor(p * 2^64), with exact endpoints.
    /// Tiny positive probabilities can round to `Never`; exact queries retain
    /// the original rational in this object and are unaffected by this method.
    pub fn threshold_64(&self) -> OccurrenceThreshold {
        if self.0.is_one() {
            return OccurrenceThreshold::Always;
        }
        let cutoff = ((self.0.numer() << 64_usize) / self.0.denom())
            .to_u64()
            .expect("probability below one has a 64-bit cutoff");
        if cutoff == 0 {
            OccurrenceThreshold::Never
        } else {
            OccurrenceThreshold::Below(cutoff)
        }
    }
}
impl fmt::Display for Probability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for Probability {
    type Err = ProbabilityError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        ProbabilityParser::default().parse(text)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OccurrenceThreshold {
    Never,
    Always,
    Below(u64),
}
impl OccurrenceThreshold {
    pub fn accepts(&self, uniform_word: u64) -> bool {
        match *self {
            Self::Never => false,
            Self::Always => true,
            Self::Below(cutoff) => uniform_word < cutoff,
        }
    }
}

/// A resource budget for exact text parsing, not a precision setting. Excessive
/// input or decimal powers fail without rounding. Fractions use signed decimal
/// integers; decimal notation has an optional sign, point and base-ten exponent.
#[derive(Clone, Copy, Debug)]
pub struct ProbabilityParser {
    max_token_bytes: usize,
    max_decimal_power: u32,
}
impl Default for ProbabilityParser {
    fn default() -> Self {
        Self {
            max_token_bytes: 100_000,
            max_decimal_power: 100_000,
        }
    }
}
impl ProbabilityParser {
    pub fn new(max_token_bytes: usize, max_decimal_power: u32) -> Result<Self, ProbabilityError> {
        if max_token_bytes == 0 {
            return Err(ProbabilityError::EmptyTokenBudget);
        }
        Ok(Self {
            max_token_bytes,
            max_decimal_power,
        })
    }
    pub fn parse(&self, text: &str) -> Result<Probability, ProbabilityError> {
        if text.len() > self.max_token_bytes {
            return Err(ProbabilityError::TokenLimit {
                bytes: text.len(),
                limit: self.max_token_bytes,
            });
        }
        let text = text.trim();
        if let Some((numerator, denominator)) = text.split_once('/') {
            return Probability::from_ratio(
                integer(numerator.trim())?,
                integer(denominator.trim())?,
            );
        }
        let (mantissa, exponent) = if let Some(position) = text.find(['e', 'E']) {
            let exponent = integer(&text[position + 1..])?.to_i64().ok_or(
                ProbabilityError::DecimalPowerLimit {
                    limit: self.max_decimal_power,
                },
            )?;
            (&text[..position], exponent)
        } else {
            (text, 0)
        };
        if exponent.unsigned_abs() > u64::from(self.max_decimal_power) {
            return Err(ProbabilityError::DecimalPowerLimit {
                limit: self.max_decimal_power,
            });
        }
        let (negative, unsigned) = match mantissa.as_bytes().first() {
            Some(b'-') => (true, &mantissa[1..]),
            Some(b'+') => (false, &mantissa[1..]),
            _ => (false, mantissa),
        };
        let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
        let whole = digit_run(whole)?;
        let fraction = digit_run(fraction)?;
        if whole.is_empty() && fraction.is_empty() {
            return Err(ProbabilityError::InvalidSyntax);
        }
        let mut digits = whole;
        digits.push_str(&fraction);
        let mut numerator =
            BigInt::parse_bytes(digits.as_bytes(), 10).ok_or(ProbabilityError::InvalidSyntax)?;
        if negative {
            numerator = -numerator;
        }
        let fraction_digits =
            i64::try_from(fraction.len()).map_err(|_| ProbabilityError::DecimalPowerLimit {
                limit: self.max_decimal_power,
            })?;
        let power =
            fraction_digits
                .checked_sub(exponent)
                .ok_or(ProbabilityError::DecimalPowerLimit {
                    limit: self.max_decimal_power,
                })?;
        if power.unsigned_abs() > u64::from(self.max_decimal_power) {
            return Err(ProbabilityError::DecimalPowerLimit {
                limit: self.max_decimal_power,
            });
        }
        let scale = BigInt::from(10).pow(power.unsigned_abs() as u32);
        if power >= 0 {
            Probability::from_ratio(numerator, scale)
        } else {
            Probability::from_ratio(numerator * scale, BigInt::one())
        }
    }
}

fn integer(text: &str) -> Result<BigInt, ProbabilityError> {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    let digits = digit_run(digits)?;
    if digits.is_empty() {
        return Err(ProbabilityError::InvalidSyntax);
    }
    let value =
        BigInt::parse_bytes(digits.as_bytes(), 10).ok_or(ProbabilityError::InvalidSyntax)?;
    Ok(if text.starts_with('-') { -value } else { value })
}

fn digit_run(text: &str) -> Result<String, ProbabilityError> {
    let bytes = text.as_bytes();
    for (i, &byte) in bytes.iter().enumerate() {
        if !(byte.is_ascii_digit()
            || (byte == b'_'
                && i > 0
                && i + 1 < bytes.len()
                && bytes[i - 1].is_ascii_digit()
                && bytes[i + 1].is_ascii_digit()))
        {
            return Err(ProbabilityError::InvalidSyntax);
        }
    }
    Ok(text.chars().filter(|&c| c != '_').collect())
}
