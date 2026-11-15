//! Explicit full-word modulo rejection, separate from the default masked stream.
use super::{RandRangeSource, RandomSource};
use num_bigint::BigUint;
use num_traits::{One, ToPrimitive, Zero};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuloRangeError {
    EmptyRange,
    RangeTooWide,
    EntropyLimit { limit: usize },
}
impl fmt::Display for ModuloRangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRange=>f.write_str("modulo word range requires a positive exclusive bound"),
            Self::RangeTooWide=>f.write_str("modulo word range bound exceeds u64; use WordRangeSource for arbitrary-width masked rejection"),
            Self::EntropyLimit{limit}=>write!(f,"modulo word range exhausted max_entropy_words={limit}"),
        }
    }
}
impl std::error::Error for ModuloRangeError {}
/// Reject the incomplete low modulo bucket before returning word % bound.
/// This matches the finite-word convention used by the pinned Native index
/// helper. It does not promise Python Random trajectories. Bound one consumes
/// no entropy; the word budget is cumulative over the adapter's lifetime.
/// The existing [`super::WordRangeSource`] keeps its distinct masked convention.
pub struct ModuloWordRangeSource<'a, R> {
    random: &'a mut R,
    limit: usize,
    used: usize,
}
impl<'a, R: RandomSource> ModuloWordRangeSource<'a, R> {
    pub fn new(random: &'a mut R, max_entropy_words: usize) -> Self {
        Self {
            random,
            limit: max_entropy_words,
            used: 0,
        }
    }
    pub fn entropy_words(&self) -> usize {
        self.used
    }
}
impl<R: RandomSource> RandRangeSource for ModuloWordRangeSource<'_, R> {
    type Error = ModuloRangeError;
    fn randrange(&mut self, bound: &BigUint) -> Result<BigUint, Self::Error> {
        if bound.is_zero() {
            return Err(ModuloRangeError::EmptyRange);
        }
        if bound.is_one() {
            return Ok(BigUint::zero());
        }
        let count = bound.to_u64().ok_or(ModuloRangeError::RangeTooWide)?;
        let threshold = count.wrapping_neg() % count;
        loop {
            if self.used >= self.limit {
                return Err(ModuloRangeError::EntropyLimit { limit: self.limit });
            }
            let word = self.random.next_u64();
            self.used += 1;
            if word >= threshold {
                return Ok(BigUint::from(word % count));
            }
        }
    }
}
