//! Exact one-bit stream refinement with explicit bit-budget errors.
use super::{SamplingError, bernoulli_refine};
use crate::scalar::{ExactScalar, ScalarArithmetic};
use std::fmt;

/// Exact binary result and actual one-bit callback count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitDraw {
    /// True selects the numerator event.
    pub selected: bool,
    /// Number of Boolean callback invocations.
    pub entropy_bits: usize,
}
/// One-bit precision exhaustion or an existing exact sampling failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BitSamplingError {
    EntropyLimit { limit: usize },
    Sampling(SamplingError),
}
impl fmt::Display for BitSamplingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EntropyLimit { limit } => {
                write!(f, "binary sampling exhausted its {limit} random-bit budget")
            }
            Self::Sampling(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for BitSamplingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sampling(error) => Some(error),
            _ => None,
        }
    }
}

/// Sample numerator/total using exactly one Boolean callback per refined bit.
/// Mass validation precedes entropy. Endpoints consume no bits after arithmetic
/// admission. Refinement errors may consume an actual prefix; the callback is
/// never invoked beyond `max_entropy_bits`.
pub fn bernoulli_ratio_bits(
    numerator: &ExactScalar,
    total: &ExactScalar,
    random_bit: &mut (impl FnMut() -> bool + ?Sized),
    max_entropy_bits: usize,
    arithmetic: &ScalarArithmetic,
) -> Result<BitDraw, BitSamplingError> {
    bernoulli_refine(
        numerator,
        total,
        || u64::from(random_bit()),
        1,
        max_entropy_bits,
        arithmetic,
    )
    .map(|(selected, entropy_bits)| BitDraw {
        selected,
        entropy_bits,
    })
    .map_err(|error| match error {
        SamplingError::EntropyLimit { limit } => BitSamplingError::EntropyLimit { limit },
        other => BitSamplingError::Sampling(other),
    })
}
