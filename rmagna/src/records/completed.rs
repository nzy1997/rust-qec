//! Explicit completed marginals of arbitrary coherent stabilizer terms.
use super::pair::{
    MassError, MassEvaluation, PairGeometry, evaluate_mass, pair_construction_work,
    pair_geometry_words, traced_record_pair,
};
use super::{SamplingError, bernoulli_ratio};
use crate::gf2::{BitVector, Gf2Error};
use crate::noise::RandomSource;
use crate::pauli::{PauliError, PauliWord};
use crate::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use crate::stabilizer::{AffineKet, StabilizerError};
use std::{cmp::Ordering, fmt};

/// Logical storage uses eight-byte payload/header/scratch units, excluding
/// existing caller storage and scalar limbs (separately bounded by arithmetic).
#[derive(Clone, Copy, Debug)]
pub struct CompletedRecordLimits {
    pub max_source_entries: usize,
    /// Raw joint ket/bra path count, not a phase-quotient certificate.
    pub max_pair_paths: usize,
    pub max_record_rows: usize,
    pub max_event_rows: usize,
    pub max_query_rows: usize,
    pub max_pairs: usize,
    pub max_geometry_words: usize,
    /// Conservative structural row/path work units, excluding scalar-limb cost.
    /// This is not an elapsed-time or process-memory limit.
    pub max_construction_work: usize,
}
impl Default for CompletedRecordLimits {
    fn default() -> Self {
        Self {
            max_source_entries: 1024,
            max_pair_paths: 256,
            max_record_rows: 256,
            max_event_rows: 256,
            max_query_rows: 256,
            max_pairs: 4096,
            max_geometry_words: 1 << 24,
            max_construction_work: 1 << 30,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletedRecordError {
    DimensionMismatch,
    DimensionOverflow,
    NonHermitianPauli,
    NonCommutingPauli {
        kind: &'static str,
        index: usize,
    },
    ResourceLimit {
        option: &'static str,
        limit: usize,
        required: u128,
    },
    Binary(Gf2Error),
    Stabilizer(StabilizerError),
    Scalar(ScalarError),
    Pauli(PauliError),
    Sampling(SamplingError),
}
impl fmt::Display for CompletedRecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionMismatch => write!(
                f,
                "completed-record term, row, outcome or Pauli dimensions disagree"
            ),
            Self::DimensionOverflow => write!(
                f,
                "completed-record dimensions exceed the platform index range"
            ),
            Self::NonHermitianPauli => {
                write!(f, "completed output moment requires a Hermitian Pauli word")
            }
            Self::NonCommutingPauli { kind, index } => {
                write!(f, "output Pauli must commute with {kind} row {index}")
            }
            Self::ResourceLimit {
                option,
                limit,
                required,
            } => write!(
                f,
                "completed records need {option} >= {required}; configured limit is {limit}"
            ),
            Self::Binary(e) => e.fmt(f),
            Self::Stabilizer(e) => e.fmt(f),
            Self::Scalar(e) => e.fmt(f),
            Self::Pauli(e) => e.fmt(f),
            Self::Sampling(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for CompletedRecordError {}
impl From<Gf2Error> for CompletedRecordError {
    fn from(e: Gf2Error) -> Self {
        Self::Binary(e)
    }
}
impl From<StabilizerError> for CompletedRecordError {
    fn from(e: StabilizerError) -> Self {
        Self::Stabilizer(e)
    }
}
impl From<ScalarError> for CompletedRecordError {
    fn from(e: ScalarError) -> Self {
        Self::Scalar(e)
    }
}
impl From<PauliError> for CompletedRecordError {
    fn from(e: PauliError) -> Self {
        Self::Pauli(e)
    }
}
impl From<SamplingError> for CompletedRecordError {
    fn from(e: SamplingError) -> Self {
        Self::Sampling(e)
    }
}
impl From<MassError> for CompletedRecordError {
    fn from(e: MassError) -> Self {
        match e {
            MassError::Scalar(e) => Self::Scalar(e),
            MassError::Stabilizer(e) => Self::Stabilizer(e),
        }
    }
}
fn admit(option: &'static str, limit: usize, required: u128) -> Result<(), CompletedRecordError> {
    if required > limit as u128 {
        Err(CompletedRecordError::ResourceLimit {
            option,
            limit,
            required,
        })
    } else {
        Ok(())
    }
}
fn metadata_words(width: usize, records: usize, events: usize) -> u128 {
    let n = width as u128;
    (records as u128 + events as u128)
        .saturating_mul(n.div_ceil(64) + 4)
        .saturating_add(n)
        .saturating_add((records as u128 + events as u128).div_ceil(64))
        .saturating_add(128)
}
/// Own fixed physical parity rows/outcomes; evaluation accepts arbitrary exact
/// coherent terms, including subnormalized and cancelling sums. No phase rank
/// certificate is assigned to caller-supplied terms. Each evaluation constructs
/// fresh pairs; this is not a prepared Gaussian plan or automatic runtime cache.
#[derive(Debug)]
pub struct CompletedRecordSpec {
    width: usize,
    records: usize,
    geometry: PairGeometry,
    event_rows: Vec<BitVector>,
    outcomes: BitVector,
    flips: BitVector,
    limits: CompletedRecordLimits,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedRecordStatistics {
    pub source_entries: usize,
    /// Actual ordered pairs visited, including pairs whose trace vanishes.
    pub pair_work: usize,
    pub retained_terms: usize,
    pub max_pair_paths: usize,
    pub estimated_geometry_words: u128,
    pub construction_work_bound: u128,
}
impl CompletedRecordSpec {
    pub fn new(
        physical_width: usize,
        records: &[BitVector],
        event_rows: &[BitVector],
        outcomes: &BitVector,
        limits: CompletedRecordLimits,
    ) -> Result<Self, CompletedRecordError> {
        if outcomes.width() != event_rows.len()
            || records
                .iter()
                .chain(event_rows)
                .any(|row| row.width() != physical_width)
        {
            return Err(CompletedRecordError::DimensionMismatch);
        }
        physical_width
            .checked_mul(2)
            .ok_or(CompletedRecordError::DimensionOverflow)?;
        physical_width
            .checked_add(event_rows.len())
            .ok_or(CompletedRecordError::DimensionOverflow)?;
        admit(
            "max_record_rows",
            limits.max_record_rows,
            records.len() as u128,
        )?;
        admit(
            "max_event_rows",
            limits.max_event_rows,
            event_rows.len() as u128,
        )?;
        admit(
            "max_geometry_words",
            limits.max_geometry_words,
            metadata_words(physical_width, records.len(), event_rows.len()),
        )?;
        Ok(Self {
            width: physical_width,
            records: records.len(),
            geometry: PairGeometry::new(physical_width, records)?,
            event_rows: event_rows.to_vec(),
            outcomes: outcomes.clone(),
            flips: BitVector::zeros(records.len())?,
            limits,
        })
    }
    pub fn physical_width(&self) -> usize {
        self.width
    }
    pub fn record_width(&self) -> usize {
        self.records
    }
    /// XOR fixed reported offsets after the retained physical parity map.
    /// Event outcomes are physical; callers must separately undo their offsets.
    pub fn with_report_flips(mut self, flips: &BitVector) -> Result<Self, CompletedRecordError> {
        if flips.width() != self.records {
            return Err(CompletedRecordError::DimensionMismatch);
        }
        self.flips = flips.clone();
        Ok(self)
    }
    pub fn probability(
        &self,
        terms: &[AffineKet],
        arithmetic: &ScalarArithmetic,
    ) -> Result<CompletedProbability, CompletedRecordError> {
        Ok(CompletedProbability(
            self.evaluate(terms, None, arithmetic)?,
        ))
    }
    pub fn moment(
        &self,
        terms: &[AffineKet],
        pauli: &PauliWord,
        arithmetic: &ScalarArithmetic,
    ) -> Result<CompletedMoment, CompletedRecordError> {
        if pauli.width() != self.width {
            return Err(CompletedRecordError::DimensionMismatch);
        }
        if pauli.phase() & 3 != 2 * u8::from(pauli.x().dot(pauli.z())?) {
            return Err(CompletedRecordError::NonHermitianPauli);
        }
        for (kind, rows) in [
            ("record", self.geometry.rows()),
            ("event", self.event_rows.as_slice()),
        ] {
            for (index, row) in rows.iter().enumerate() {
                if row.dot(pauli.x())? {
                    return Err(CompletedRecordError::NonCommutingPauli { kind, index });
                }
            }
        }
        Ok(CompletedMoment(self.evaluate(
            terms,
            Some(pauli),
            arithmetic,
        )?))
    }
    fn evaluate(
        &self,
        terms: &[AffineKet],
        pauli: Option<&PauliWord>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<CompletedMass, CompletedRecordError> {
        admit(
            "max_source_entries",
            self.limits.max_source_entries,
            terms.len() as u128,
        )?;
        if terms.iter().any(|term| term.width() != self.width) {
            return Err(CompletedRecordError::DimensionMismatch);
        }
        let paths = terms
            .iter()
            .map(|term| term.columns().len())
            .max()
            .unwrap_or(0)
            .checked_mul(2)
            .ok_or(CompletedRecordError::DimensionOverflow)?;
        admit("max_pair_paths", self.limits.max_pair_paths, paths as u128)?;
        let pairs = (terms.len() as u128).pow(2);
        admit("max_pairs", self.limits.max_pairs, pairs)?;
        let words = if pairs == 0 {
            metadata_words(self.width, self.records, self.event_rows.len())
        } else {
            pair_geometry_words(
                self.width,
                paths,
                self.records,
                self.event_rows.len(),
                pairs,
            )
        };
        admit("max_geometry_words", self.limits.max_geometry_words, words)?;
        let work = pair_construction_work(
            self.width,
            paths,
            self.records,
            self.event_rows.len(),
            pairs,
        );
        admit(
            "max_construction_work",
            self.limits.max_construction_work,
            work,
        )?;
        for term in terms {
            arithmetic.multiply(term.coefficient(), &ExactScalar::one())?;
        }
        let mut contributions = Vec::with_capacity(pairs as usize);
        let mut pair_work = 0;
        for left in terms {
            let transformed = pauli.map(|word| word.apply(left, arithmetic)).transpose()?;
            let left = transformed.as_ref().unwrap_or(left);
            for right in terms {
                pair_work += 1;
                if let Some(term) = traced_record_pair(
                    left,
                    right,
                    &self.geometry,
                    &self.event_rows,
                    &self.outcomes,
                    &self.flips,
                    arithmetic,
                )? {
                    contributions.push(term);
                }
            }
        }
        let statistics = CompletedRecordStatistics {
            source_entries: terms.len(),
            pair_work,
            retained_terms: contributions.len(),
            max_pair_paths: paths,
            estimated_geometry_words: words,
            construction_work_bound: work,
        };
        Ok(CompletedMass {
            width: self.records,
            terms: contributions,
            limits: self.limits,
            statistics,
        })
    }
}
#[derive(Debug)]
struct CompletedMass {
    width: usize,
    terms: Vec<AffineKet>,
    limits: CompletedRecordLimits,
    statistics: CompletedRecordStatistics,
}
impl CompletedMass {
    fn admit_query(&self, rows: usize) -> Result<(), CompletedRecordError> {
        admit("max_query_rows", self.limits.max_query_rows, rows as u128)?;
        let paths = self
            .terms
            .iter()
            .map(|term| term.columns().len())
            .max()
            .unwrap_or(0);
        let words = pair_geometry_words(
            self.width,
            paths,
            self.width,
            rows,
            self.terms.len() as u128,
        );
        admit("max_geometry_words", self.limits.max_geometry_words, words)
    }
    fn query(
        &self,
        rows: &[BitVector],
        outcomes: &BitVector,
        arithmetic: &ScalarArithmetic,
    ) -> Result<MassEvaluation, CompletedRecordError> {
        if outcomes.width() != rows.len() || rows.iter().any(|row| row.width() != self.width) {
            return Err(CompletedRecordError::DimensionMismatch);
        }
        self.admit_query(rows.len())?;
        Ok(evaluate_mass(
            self.width,
            &self.terms,
            rows,
            outcomes,
            arithmetic,
        )?)
    }
    fn value(
        &self,
        word: &BitVector,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, CompletedRecordError> {
        if word.width() != self.width {
            return Err(CompletedRecordError::DimensionMismatch);
        }
        self.admit_query(self.width)?;
        let rows = (0..self.width)
            .map(|j| BitVector::from_indices(self.width, [j]))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(evaluate_mass(self.width, &self.terms, &rows, word, arithmetic)?.value)
    }
    fn total(&self, arithmetic: &ScalarArithmetic) -> Result<ExactScalar, CompletedRecordError> {
        Ok(self.query(&[], &BitVector::zeros(0)?, arithmetic)?.value)
    }
}
/// Physical unnormalized probabilities. Only this result type exposes sampling.
#[derive(Debug)]
pub struct CompletedProbability(CompletedMass);
/// Signed Hermitian moments per record; these values are never sample weights.
#[derive(Debug)]
pub struct CompletedMoment(CompletedMass);
impl CompletedProbability {
    pub fn width(&self) -> usize {
        self.0.width
    }
    pub fn statistics(&self) -> &CompletedRecordStatistics {
        &self.0.statistics
    }
    /// Classical contributions: sum amplitudes, without squaring them again.
    pub fn contributions(&self) -> &[AffineKet] {
        &self.0.terms
    }
    pub fn value(
        &self,
        word: &BitVector,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, CompletedRecordError> {
        self.0.value(word, arithmetic)
    }
    pub fn mass(
        &self,
        rows: &[BitVector],
        outcomes: &BitVector,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, CompletedRecordError> {
        Ok(self.0.query(rows, outcomes, arithmetic)?.value)
    }
    pub fn total(
        &self,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, CompletedRecordError> {
        self.0.total(arithmetic)
    }
}
impl CompletedMoment {
    pub fn width(&self) -> usize {
        self.0.width
    }
    pub fn statistics(&self) -> &CompletedRecordStatistics {
        &self.0.statistics
    }
    pub fn contributions(&self) -> &[AffineKet] {
        &self.0.terms
    }
    pub fn value(
        &self,
        word: &BitVector,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, CompletedRecordError> {
        self.0.value(word, arithmetic)
    }
    pub fn mass(
        &self,
        rows: &[BitVector],
        outcomes: &BitVector,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, CompletedRecordError> {
        Ok(self.0.query(rows, outcomes, arithmetic)?.value)
    }
    pub fn total(
        &self,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, CompletedRecordError> {
        self.0.total(arithmetic)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct CompletedSamplingOptions {
    pub max_mass_queries: usize,
    pub max_entropy_words: usize,
}
impl Default for CompletedSamplingOptions {
    fn default() -> Self {
        Self {
            max_mass_queries: 4097,
            max_entropy_words: 1 << 20,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedRecordDraw {
    pub values: BitVector,
    /// Joint unnormalized event/selected-record mass, not conditional mass one.
    pub mass: ExactScalar,
    pub condition_mass: ExactScalar,
    pub entropy_words: usize,
    pub mass_queries: usize,
    pub gaussian_sums: u128,
}
impl CompletedProbability {
    /// Draw ordered record bits by exact conditional ratios. RandomSource words
    /// refine a uniform interval as in bernoulli_ratio; no Python stream identity
    /// is promised. Deterministic decisions consume no entropy. Metadata
    /// admission fails before draws; a later arithmetic or entropy error may
    /// advance the caller's stream while leaving this immutable marginal reusable.
    pub fn sample(
        &self,
        random: &mut impl RandomSource,
        options: &CompletedSamplingOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<CompletedRecordDraw, CompletedRecordError> {
        let required = self
            .width()
            .checked_add(1)
            .ok_or(CompletedRecordError::DimensionOverflow)?;
        admit(
            "max_mass_queries",
            options.max_mass_queries,
            required as u128,
        )?;
        self.0.admit_query(self.width())?;
        let initial = self.0.query(&[], &BitVector::zeros(0)?, arithmetic)?;
        let mut mass = initial.value;
        if mass.is_zero() {
            return Err(SamplingError::ImpossibleCondition.into());
        }
        if mass.real_sign()? != Ordering::Greater {
            return Err(SamplingError::InvalidMass.into());
        }
        let condition_mass = mass.clone();
        let mut gaussian_sums = initial.gaussian_sums as u128;
        let mut mass_queries = 1;
        let mut rows = Vec::with_capacity(self.width());
        let mut outcomes = Vec::with_capacity(self.width());
        let mut values = BitVector::zeros(self.width())?;
        let mut entropy_words = 0;
        for j in 0..self.width() {
            rows.push(BitVector::from_indices(self.width(), [j])?);
            outcomes.push(false);
            let word = BitVector::from_indices(
                outcomes.len(),
                outcomes
                    .iter()
                    .enumerate()
                    .filter_map(|(j, &v)| v.then_some(j)),
            )?;
            let zero = self.0.query(&rows, &word, arithmetic)?;
            mass_queries += 1;
            gaussian_sums += zero.gaussian_sums as u128;
            let draw = bernoulli_ratio(
                &zero.value,
                &mass,
                random,
                options.max_entropy_words.saturating_sub(entropy_words),
                arithmetic,
            )
            .map_err(|e| {
                if matches!(e, SamplingError::EntropyLimit { .. }) {
                    SamplingError::EntropyLimit {
                        limit: options.max_entropy_words,
                    }
                } else {
                    e
                }
            })?;
            entropy_words += draw.entropy_words;
            values.set(j, !draw.selected)?;
            outcomes[j] = !draw.selected;
            mass = if draw.selected {
                zero.value
            } else {
                arithmetic.subtract(&mass, &zero.value)?
            };
        }
        Ok(CompletedRecordDraw {
            values,
            mass,
            condition_mass,
            entropy_words,
            mass_queries,
            gaussian_sums,
        })
    }
}
