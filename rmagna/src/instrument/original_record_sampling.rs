//! Bounded exact draws from coherent classical mass terms.
use super::OriginalRecordProbabilities;
use crate::gf2::{AffineSupport, BitVector, Gf2Error, LinearFactorization, transpose_columns};
use crate::records::pair::{MassError, evaluate_mass};
use crate::records::{BitSamplingError, bernoulli_ratio_bits};
use crate::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use crate::stabilizer::{AffineKet, StabilizerError};
use std::{cmp::Ordering, fmt};

/// Conservative packed geometry and word-operation admission. These bounds
/// include canonicalization, affine hull/coordinate factors, retained terms and
/// scratch, but exclude existing source storage and arbitrary-precision scalar
/// limbs. Scalar arithmetic has its own limits; this is not a process RSS bound.
#[derive(Clone, Copy, Debug)]
pub struct OriginalRecordSamplingLimits {
    pub max_record_rows: usize,
    pub max_terms: usize,
    pub max_path_bits: usize,
    pub max_support_dimension: usize,
    pub max_geometry_words: usize,
    pub max_work: usize,
}
impl Default for OriginalRecordSamplingLimits {
    fn default() -> Self {
        Self {
            max_record_rows: 256,
            max_terms: 1024,
            max_path_bits: 256,
            max_support_dimension: 256,
            max_geometry_words: 1 << 24,
            max_work: 1 << 30,
        }
    }
}

/// A draw admits every coordinate query before allocating scratch or consuming
/// entropy. Irrational probabilities can exhaust the finite bit budget.
#[derive(Clone, Copy, Debug)]
pub struct OriginalRecordDrawOptions {
    pub max_conditional_queries: usize,
    pub max_entropy_bits: usize,
    pub max_geometry_words: usize,
    pub max_work: usize,
}
impl Default for OriginalRecordDrawOptions {
    fn default() -> Self {
        Self {
            max_conditional_queries: 256,
            max_entropy_bits: 1 << 20,
            max_geometry_words: 1 << 24,
            max_work: 1 << 30,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OriginalRecordSamplingError {
    ResourceLimit {
        option: &'static str,
        limit: usize,
        required: u128,
    },
    InvalidMass,
    IncompatibleSupport,
    Binary(Gf2Error),
    Stabilizer(StabilizerError),
    Scalar(ScalarError),
    Bits(BitSamplingError),
}
impl fmt::Display for OriginalRecordSamplingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit {
                option,
                limit,
                required,
            } => write!(
                f,
                "retained record sampling needs {option} >= {required}; configured limit is {limit}"
            ),
            Self::InvalidMass => write!(
                f,
                "retained record sampling requires a positive real total and conditional masses between zero and their parent mass"
            ),
            Self::IncompatibleSupport => write!(
                f,
                "retained mass terms do not fit the prepared affine coordinate map"
            ),
            Self::Binary(e) => e.fmt(f),
            Self::Stabilizer(e) => e.fmt(f),
            Self::Scalar(e) => e.fmt(f),
            Self::Bits(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for OriginalRecordSamplingError {}
macro_rules! from_error {
    ($($t:ty => $v:ident),* $(,)?) => {$(impl From<$t> for OriginalRecordSamplingError {
        fn from(e: $t) -> Self { Self::$v(e) }
    })*};
}
from_error!(Gf2Error=>Binary, StabilizerError=>Stabilizer, ScalarError=>Scalar, BitSamplingError=>Bits);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalRecordSamplingStatistics {
    pub record_rows: usize,
    pub source_terms: usize,
    pub canonical_terms: usize,
    pub max_source_paths: usize,
    pub support_dimension: usize,
    pub preparation_geometry_words: u128,
    pub preparation_work: u128,
}

/// An immutable classical law in independent coordinates of a conservative
/// affine hull. Canceled labels may remain in the bound. No minimum true support
/// or general native commuting-axis certificate is implied.
pub struct PreparedOriginalRecordSampling {
    support: AffineSupport,
    terms: Vec<AffineKet>,
    total: ExactScalar,
    statistics: OriginalRecordSamplingStatistics,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalRecordDraw {
    pub label: BitVector,
    pub label_mass: ExactScalar,
    pub total_mass: ExactScalar,
    pub support_dimension: usize,
    pub conditional_queries: usize,
    pub entropy_bits: usize,
}

fn admit(
    option: &'static str,
    limit: usize,
    required: u128,
) -> Result<(), OriginalRecordSamplingError> {
    if required > limit as u128 {
        Err(OriginalRecordSamplingError::ResourceLimit {
            option,
            limit,
            required,
        })
    } else {
        Ok(())
    }
}
fn ket_words(width: u128, paths: u128) -> u128 {
    (paths + 1)
        .saturating_mul(3 + width.div_ceil(64))
        .saturating_add(paths.saturating_mul(4 + paths.div_ceil(64)))
        .saturating_add(64)
}
fn mass(
    width: usize,
    terms: &[AffineKet],
    rows: &[BitVector],
    outcomes: &BitVector,
    a: &ScalarArithmetic,
) -> Result<ExactScalar, OriginalRecordSamplingError> {
    evaluate_mass(width, terms, rows, outcomes, a)
        .map(|r| r.value)
        .map_err(|e| match e {
            MassError::Stabilizer(e) => e.into(),
            MassError::Scalar(e) => e.into(),
        })
}

impl OriginalRecordProbabilities {
    /// Prepare exact sampling without enumerating reported labels or building
    /// another phase resource. Canonical term amplitudes are classical masses:
    /// they are added linearly, including complex/negative cross terms.
    pub fn prepare_affine_sampling(
        &self,
        limits: OriginalRecordSamplingLimits,
        a: &ScalarArithmetic,
    ) -> Result<PreparedOriginalRecordSampling, OriginalRecordSamplingError> {
        let source = self.terms();
        let first = source
            .first()
            .ok_or(OriginalRecordSamplingError::InvalidMass)?;
        let width = first.width();
        if source.iter().any(|t| t.width() != width) {
            return Err(OriginalRecordSamplingError::IncompatibleSupport);
        }
        let paths = source.iter().map(|t| t.columns().len()).max().unwrap_or(0);
        let candidates = source.iter().fold(0_u128, |n, t| {
            n.saturating_add(t.columns().len() as u128 + 1)
        });
        admit("max_record_rows", limits.max_record_rows, width as u128)?;
        admit("max_terms", limits.max_terms, source.len() as u128)?;
        admit("max_path_bits", limits.max_path_bits, paths as u128)?;
        let size = (width as u128)
            .saturating_add(candidates)
            .saturating_add(paths as u128)
            .saturating_add(1);
        let geometry = 4_u128
            .saturating_mul(source.len() as u128)
            .saturating_mul(ket_words(width as u128, paths as u128))
            .saturating_add(8_u128.saturating_mul(size.saturating_pow(2)));
        let work = geometry.saturating_mul(size).saturating_mul(8);
        admit("max_geometry_words", limits.max_geometry_words, geometry)?;
        admit("max_work", limits.max_work, work)?;
        // All source/basis/coordinate geometry and work admission precedes
        // canonicalization, factorization and retained allocations.
        let mut canonical = Vec::with_capacity(source.len());
        for term in source {
            if let Some(term) = term.contract(a)? {
                canonical.push(term);
            }
        }
        let first = canonical
            .first()
            .ok_or(OriginalRecordSamplingError::InvalidMass)?;
        let mut offset = first.offset().clone();
        let mut columns = Vec::new();
        for term in &canonical {
            let mut difference = term.offset().clone();
            difference.xor_assign(&offset)?;
            columns.push(difference);
            columns.extend_from_slice(term.columns());
        }
        let hull = LinearFactorization::new(width, &columns)?;
        let dimension = hull.rank();
        admit(
            "max_support_dimension",
            limits.max_support_dimension,
            dimension as u128,
        )?;
        // Choose a deterministic least-pivot representative of the same hull.
        for (row, &pivot) in hull.row_space_basis().iter().zip(hull.pivot_columns()) {
            if offset.get(pivot)? {
                offset.xor_assign(row)?;
            }
        }
        let support = AffineSupport::new(offset, hull.row_space_basis().to_vec())?;
        let coordinate_map =
            LinearFactorization::new(dimension, &transpose_columns(width, support.columns())?)?;
        let coordinate = |v: &BitVector| -> Result<BitVector, OriginalRecordSamplingError> {
            coordinate_map
                .solve(v)?
                .ok_or(OriginalRecordSamplingError::IncompatibleSupport)
        };
        let mut terms = Vec::with_capacity(canonical.len());
        for term in canonical {
            let mut offset = term.offset().clone();
            offset.xor_assign(support.offset())?;
            let offset = coordinate(&offset)?;
            let columns = term
                .columns()
                .iter()
                .map(coordinate)
                .collect::<Result<Vec<_>, _>>()?;
            terms.push(AffineKet::new(
                offset,
                columns,
                term.phase().clone(),
                term.coefficient().clone(),
            )?);
        }
        let total = mass(dimension, &terms, &[], &BitVector::zeros(0)?, a)?;
        if !total.is_real() || total.real_sign()? != Ordering::Greater {
            return Err(OriginalRecordSamplingError::InvalidMass);
        }
        let statistics = OriginalRecordSamplingStatistics {
            record_rows: width,
            source_terms: source.len(),
            canonical_terms: terms.len(),
            max_source_paths: paths,
            support_dimension: dimension,
            preparation_geometry_words: geometry,
            preparation_work: work,
        };
        Ok(PreparedOriginalRecordSampling {
            support,
            terms,
            total,
            statistics,
        })
    }
}

impl PreparedOriginalRecordSampling {
    pub fn support_bound(&self) -> &AffineSupport {
        &self.support
    }
    pub fn total_mass(&self) -> &ExactScalar {
        &self.total
    }
    pub fn statistics(&self) -> &OriginalRecordSamplingStatistics {
        &self.statistics
    }
    /// Relabeled canonical mass terms with unchanged coefficients/quadratics.
    /// Add their amplitudes; do not square them or sample them as a mixture.
    pub fn coordinate_terms(&self) -> &[AffineKet] {
        &self.terms
    }

    /// Independent unbiased Boolean callback bits produce the normalized law.
    /// Errors never mutate this plan. An entropy/scalar failure can consume an
    /// actual prefix, but never invokes the callback beyond the supplied cap.
    pub fn sample_bits(
        &self,
        options: OriginalRecordDrawOptions,
        next_bit: &mut (impl FnMut() -> bool + ?Sized),
        a: &ScalarArithmetic,
    ) -> Result<OriginalRecordDraw, OriginalRecordSamplingError> {
        let dimension = self.support.dimension();
        let d = dimension as u128;
        let p = self.statistics.max_source_paths as u128;
        let t = self.terms.len() as u128;
        let w = self.support.width() as u128;
        let geometry = 4_u128
            .saturating_mul(t)
            .saturating_mul(ket_words(d, p))
            .saturating_add(8_u128.saturating_mul((d + p + 1).saturating_pow(2)))
            .saturating_add(
                4_u128
                    .saturating_mul(w + d + 1)
                    .saturating_mul(w.div_ceil(64) + d.div_ceil(64) + 8),
            );
        let work = (d + 1).saturating_mul(
            geometry.saturating_add(
                t.saturating_mul((p + 1).saturating_pow(3))
                    .saturating_mul(64),
            ),
        );
        admit(
            "max_conditional_queries",
            options.max_conditional_queries,
            d,
        )?;
        admit("max_geometry_words", options.max_geometry_words, geometry)?;
        admit("max_work", options.max_work, work)?;
        let total = a.add(&self.total, &ExactScalar::zero())?;
        let mut parent = total.clone();
        let mut label = self.support.offset().clone();
        let mut chosen = Vec::with_capacity(dimension);
        let rows = (0..dimension)
            .map(|j| BitVector::from_indices(dimension, [j]))
            .collect::<Result<Vec<_>, _>>()?;
        let mut entropy_bits = 0;
        for j in 0..dimension {
            let mut outcomes = BitVector::zeros(j + 1)?;
            for (i, &bit) in chosen.iter().enumerate() {
                outcomes.set(i, bit)?;
            }
            outcomes.set(j, true)?;
            let numerator = mass(dimension, &self.terms, &rows[..=j], &outcomes, a)?;
            let complement = a.subtract(&parent, &numerator)?;
            if !numerator.is_real()
                || !complement.is_real()
                || numerator.real_sign()? == Ordering::Less
                || complement.real_sign()? == Ordering::Less
            {
                return Err(OriginalRecordSamplingError::InvalidMass);
            }
            let draw = bernoulli_ratio_bits(
                &numerator,
                &parent,
                next_bit,
                options.max_entropy_bits - entropy_bits,
                a,
            )?;
            entropy_bits += draw.entropy_bits;
            chosen.push(draw.selected);
            if draw.selected {
                label.xor_assign(&self.support.columns()[j])?;
            }
            parent = if draw.selected { numerator } else { complement };
        }
        Ok(OriginalRecordDraw {
            label,
            label_mass: parent,
            total_mass: total,
            support_dimension: dimension,
            conditional_queries: dimension,
            entropy_bits,
        })
    }
}
