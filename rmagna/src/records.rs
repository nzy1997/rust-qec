//! Exact physical reported-record sampling, separate from postselection.
use crate::circuit::{BitAtom, BitExpr};
use crate::gf2::BitVector;
use crate::instrument::{CompiledInstrument, FaultSelection, InstrumentError};
use crate::noise::RandomSource;
use crate::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use num_bigint::BigInt;
use std::{cmp::Ordering, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SamplingError {
    InvalidMass,
    ImpossibleCondition,
    EntropyLimit { limit: usize },
    ExponentRange,
    Instrument(InstrumentError),
    Scalar(ScalarError),
}
impl fmt::Display for SamplingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImpossibleCondition => {
                write!(f, "reported history condition has zero physical mass")
            }
            Self::InvalidMass => write!(
                f,
                "binary sampling requires a positive total and a numerator between zero and that total"
            ),
            Self::EntropyLimit { limit } => write!(
                f,
                "reported sampling exhausted its {limit} random-word budget"
            ),
            Self::ExponentRange => {
                write!(f, "random prefix exceeds the exact scalar exponent range")
            }
            Self::Instrument(error) => error.fmt(f),
            Self::Scalar(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for SamplingError {}
impl From<InstrumentError> for SamplingError {
    fn from(error: InstrumentError) -> Self {
        Self::Instrument(error)
    }
}
impl From<ScalarError> for SamplingError {
    fn from(error: ScalarError) -> Self {
        Self::Scalar(error)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct SamplingOptions {
    pub max_terms: usize,
    pub max_entropy_words: usize,
}
impl Default for SamplingOptions {
    fn default() -> Self {
        Self {
            max_terms: 256,
            max_entropy_words: 1 << 20,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BinaryDraw {
    pub selected: bool,
    pub entropy_words: usize,
}
/// Sample numerator / total by refining a uniform binary interval. All boundary
/// comparisons are exact; irrational probabilities need no floating embedding.
pub fn bernoulli_ratio(
    numerator: &ExactScalar,
    total: &ExactScalar,
    random: &mut impl RandomSource,
    max_entropy_words: usize,
    arithmetic: &ScalarArithmetic,
) -> Result<BinaryDraw, SamplingError> {
    let remaining = arithmetic.subtract(total, numerator)?;
    if total.real_sign()? != Ordering::Greater
        || numerator.real_sign()? == Ordering::Less
        || remaining.real_sign()? == Ordering::Less
    {
        return Err(SamplingError::InvalidMass);
    }
    if numerator.is_zero() {
        return Ok(BinaryDraw {
            selected: false,
            entropy_words: 0,
        });
    }
    if remaining.is_zero() {
        return Ok(BinaryDraw {
            selected: true,
            entropy_words: 0,
        });
    }
    let mut prefix = BigInt::from(0);
    for consumed in 1..=max_entropy_words {
        let power = i64::try_from(consumed)
            .ok()
            .and_then(|n| n.checked_mul(-64))
            .ok_or(SamplingError::ExponentRange)?;
        prefix = (prefix << 64) + BigInt::from(random.next_u64());
        let lower = arithmetic.scalar([prefix.clone(), 0.into(), 0.into(), 0.into()], power)?;
        let lower = arithmetic.multiply(total, &lower)?;
        if arithmetic.subtract(numerator, &lower)?.real_sign()? != Ordering::Greater {
            return Ok(BinaryDraw {
                selected: false,
                entropy_words: consumed,
            });
        }
        let upper = arithmetic.scalar([&prefix + 1, 0.into(), 0.into(), 0.into()], power)?;
        let upper = arithmetic.multiply(total, &upper)?;
        if arithmetic.subtract(numerator, &upper)?.real_sign()? != Ordering::Less {
            return Ok(BinaryDraw {
                selected: true,
                entropy_words: consumed,
            });
        }
    }
    Err(SamplingError::EntropyLimit {
        limit: max_entropy_words,
    })
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportedDraw {
    pub values: Vec<bool>,
    /// Joint physical mass of the given conditions and returned parity values.
    pub mass: ExactScalar,
    pub condition_mass: ExactScalar,
    pub entropy_words: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct RecordQuery<'a> {
    pub given: &'a [(BitExpr, bool)],
    pub parities: &'a [BitExpr],
}
/// Sample ordered affine parities using physical prefix marginals. The compiled
/// instrument's postselection predicates are deliberately not included.
pub fn sample_reported(
    instrument: &CompiledInstrument,
    parities: &[BitExpr],
    faults: &[FaultSelection],
    random: &mut impl RandomSource,
    options: &SamplingOptions,
    arithmetic: &ScalarArithmetic,
) -> Result<ReportedDraw, SamplingError> {
    sample_reported_conditioned(
        instrument,
        &RecordQuery {
            given: &[],
            parities,
        },
        faults,
        random,
        options,
        arithmetic,
    )
}
/// Retain previously sampled physical conditions while sampling further inputs.
/// Zero-mass conditions fail before reading entropy; given values are not redrawn.
pub fn sample_reported_conditioned(
    instrument: &CompiledInstrument,
    query: &RecordQuery<'_>,
    faults: &[FaultSelection],
    random: &mut impl RandomSource,
    options: &SamplingOptions,
    arithmetic: &ScalarArithmetic,
) -> Result<ReportedDraw, SamplingError> {
    for parity in query.given.iter().map(|(p, _)| p).chain(query.parities) {
        for atom in &parity.terms {
            match atom {
                BitAtom::Record(record) if *record < instrument.num_records() => {}
                BitAtom::Record(record) => {
                    return Err(InstrumentError::RecordOutOfBounds {
                        record: *record,
                        count: instrument.num_records(),
                    }
                    .into());
                }
                _ => return Err(InstrumentError::NonAffineQuery.into()),
            }
        }
    }
    let mut rows = query
        .given
        .iter()
        .map(|(p, _)| p.clone())
        .collect::<Vec<_>>();
    let mut outcomes = query.given.iter().map(|(_, v)| *v).collect::<Vec<_>>();
    let bits = |values: &[bool]| {
        BitVector::from_indices(
            values.len(),
            values
                .iter()
                .enumerate()
                .filter_map(|(j, &v)| v.then_some(j)),
        )
        .map_err(InstrumentError::from)
    };
    let mut mass = instrument
        .reported_probability(
            &rows,
            &bits(&outcomes)?,
            faults,
            options.max_terms,
            arithmetic,
        )?
        .value;
    if mass.is_zero() {
        return Err(SamplingError::ImpossibleCondition);
    }
    let condition_mass = mass.clone();
    let mut values: Vec<bool> = Vec::with_capacity(query.parities.len());
    let mut entropy_words = 0;
    for parity in query.parities {
        rows.push(parity.clone());
        outcomes.push(false);
        let zero = instrument
            .reported_probability(
                &rows,
                &bits(&outcomes)?,
                faults,
                options.max_terms,
                arithmetic,
            )?
            .value;
        let draw = bernoulli_ratio(
            &zero,
            &mass,
            random,
            options.max_entropy_words.saturating_sub(entropy_words),
            arithmetic,
        )
        .map_err(|error| {
            if matches!(error, SamplingError::EntropyLimit { .. }) {
                SamplingError::EntropyLimit {
                    limit: options.max_entropy_words,
                }
            } else {
                error
            }
        })?;
        entropy_words += draw.entropy_words;
        values.push(!draw.selected);
        *outcomes.last_mut().unwrap() = !draw.selected;
        mass = if draw.selected {
            zero
        } else {
            arithmetic.subtract(&mass, &zero)?
        };
    }
    Ok(ReportedDraw {
        values,
        mass,
        condition_mass,
        entropy_words,
    })
}
