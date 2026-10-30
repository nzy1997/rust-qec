//! Explicit per-source plans for completed conjugate-pair geometry.
use super::*;
use crate::stabilizer::{
    PreparedAffineContraction, PreparedLinearBoundary, PreparedPathProjection,
};

/// Additional retained-plan limits. Cold raw-path limits remain unchanged.
#[derive(Clone, Copy, Debug)]
pub struct PreparedRecordLimits {
    /// Kernel dimension after the joint trace/event equations. This is neither
    /// raw joint storage nor the native deferred network's coordinate count.
    pub max_reduced_pair_paths: usize,
    pub max_geometry_words: usize,
}
impl Default for PreparedRecordLimits {
    fn default() -> Self {
        Self {
            max_reduced_pair_paths: 256,
            max_geometry_words: 1 << 24,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedRecordStatistics {
    pub source_entries: usize,
    pub ordered_pairs: usize,
    pub max_raw_pair_paths: usize,
    pub max_reduced_pair_paths: usize,
    pub estimated_geometry_words: u128,
    /// Actual successful PreparedPathProjection constructions.
    pub support_factorizations: usize,
    /// Actual successful PreparedAffineContraction constructions.
    pub pairing_factorizations: usize,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreparedRecordWork {
    pub support_factorizations: usize,
    pub pairing_factorizations: usize,
    pub support_plan_applications: usize,
    pub pairing_plan_applications: usize,
}
pub struct PreparedRecordResult<T> {
    pub distribution: T,
    /// Work in this pair-evaluation call, excluding later mass/sampling queries
    /// on the returned distribution and excluding preparation.
    pub work: PreparedRecordWork,
}
struct PreparedPair {
    left: usize,
    right: usize,
    support: PreparedPathProjection,
    boundary: PreparedLinearBoundary,
    pairing: PreparedAffineContraction,
}
/// Own one fixed source expansion and every ordered pair's actual support,
/// boundary and Gaussian contraction program. Event RHS and commuting Hermitian
/// Paulis may change. This is explicit per-assignment ownership, not automatic
/// caching, scalar memoization or cross-assignment decomposition reuse.
pub struct PreparedCompletedRecords {
    sources: Vec<AffineKet>,
    pairs: Vec<PreparedPair>,
    width: usize,
    records: usize,
    record_rows: Vec<BitVector>,
    event_rows: Vec<BitVector>,
    flips: BitVector,
    keep: Vec<usize>,
    limits: CompletedRecordLimits,
    statistics: PreparedRecordStatistics,
    cold_words: u128,
    construction_work: u128,
}
impl CompletedRecordSpec {
    pub(crate) fn event_outcomes(&self) -> &BitVector {
        &self.outcomes
    }

    /// Admit all retained storage before copying sources or allocating pair
    /// plans. Outcome consistency and zero scalar values do not skip structural
    /// preparation: another compatible RHS/Pauli may produce a nonzero result.
    pub fn prepare(
        &self,
        terms: &[AffineKet],
        limits: PreparedRecordLimits,
        a: &ScalarArithmetic,
    ) -> Result<PreparedCompletedRecords, CompletedRecordError> {
        admit(
            "max_source_entries",
            self.limits.max_source_entries,
            terms.len() as u128,
        )?;
        if terms.iter().any(|t| t.width() != self.width) {
            return Err(CompletedRecordError::DimensionMismatch);
        }
        let paths = terms
            .iter()
            .map(|t| t.columns().len())
            .max()
            .unwrap_or(0)
            .checked_mul(2)
            .ok_or(CompletedRecordError::DimensionOverflow)?;
        admit("max_pair_paths", self.limits.max_pair_paths, paths as u128)?;
        let pairs = (terms.len() as u128).pow(2);
        admit("max_pairs", self.limits.max_pairs, pairs)?;
        let cold_words = if pairs == 0 {
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
        admit(
            "max_geometry_words",
            self.limits.max_geometry_words,
            cold_words,
        )?;
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
        let (n, p, r, e) = (
            self.width as u128,
            paths as u128,
            self.records as u128,
            self.event_rows.len() as u128,
        );
        let vectors = |count: u128, bits: u128| count.saturating_mul(bits.div_ceil(64) + 4);
        let per_pair = vectors(p + 1, 2 * n)
            .saturating_add(vectors(n + e, p))
            .saturating_add(vectors(p, p))
            .saturating_add(vectors(p + 1, r))
            .saturating_add(n + e + p + r + 128);
        let words = cold_words
            .saturating_add(64_u128.saturating_mul(pairs).saturating_mul(per_pair))
            .saturating_add(vectors((terms.len() as u128).saturating_mul(p + 1), n));
        admit(
            "max_prepared_geometry_words",
            limits.max_geometry_words,
            words,
        )?;
        for term in terms {
            a.multiply(term.coefficient(), &ExactScalar::one())?;
        }
        let keep = (0..self.width).collect::<Vec<_>>();
        let mut statistics = PreparedRecordStatistics {
            source_entries: terms.len(),
            ordered_pairs: pairs as usize,
            max_raw_pair_paths: paths,
            max_reduced_pair_paths: 0,
            estimated_geometry_words: words,
            support_factorizations: 0,
            pairing_factorizations: 0,
        };
        let mut plans = Vec::with_capacity(pairs as usize);
        for (left, ket) in terms.iter().enumerate() {
            for (right, bra) in terms.iter().enumerate() {
                let joint = ket.tensor(&bra.conjugate(), a)?;
                let equations =
                    super::super::pair::pair_equations(&joint, self.width, &self.event_rows)?;
                let support = PreparedPathProjection::new(&joint, &equations)?;
                statistics.support_factorizations += 1;
                let reduced = support.reduced_paths();
                admit(
                    "max_reduced_pair_paths",
                    limits.max_reduced_pair_paths,
                    reduced as u128,
                )?;
                statistics.max_reduced_pair_paths = statistics.max_reduced_pair_paths.max(reduced);
                let structure = support.structure()?.retain_boundary(&keep)?;
                let boundary = PreparedLinearBoundary::new(&structure, self.geometry.rows())?;
                let pairing = PreparedAffineContraction::new(boundary.reference())?;
                statistics.pairing_factorizations += 1;
                plans.push(PreparedPair {
                    left,
                    right,
                    support,
                    boundary,
                    pairing,
                });
            }
        }
        // No event-value cache is retained. The spec's initial RHS is irrelevant.
        Ok(PreparedCompletedRecords {
            sources: terms.to_vec(),
            pairs: plans,
            width: self.width,
            records: self.records,
            record_rows: self.geometry.rows().to_vec(),
            event_rows: self.event_rows.clone(),
            flips: self.flips.clone(),
            keep,
            limits: self.limits,
            statistics,
            cold_words,
            construction_work: work,
        })
    }
}
impl PreparedCompletedRecords {
    pub fn statistics(&self) -> &PreparedRecordStatistics {
        &self.statistics
    }
    pub fn probability(
        &self,
        outcomes: &BitVector,
        a: &ScalarArithmetic,
    ) -> Result<PreparedRecordResult<CompletedProbability>, CompletedRecordError> {
        let (mass, work) = self.evaluate(outcomes, None, a)?;
        Ok(PreparedRecordResult {
            distribution: CompletedProbability(mass),
            work,
        })
    }
    pub fn moment(
        &self,
        outcomes: &BitVector,
        word: &PauliWord,
        a: &ScalarArithmetic,
    ) -> Result<PreparedRecordResult<CompletedMoment>, CompletedRecordError> {
        if word.width() != self.width {
            return Err(CompletedRecordError::DimensionMismatch);
        }
        if word.phase() & 3 != 2 * u8::from(word.x().dot(word.z())?) {
            return Err(CompletedRecordError::NonHermitianPauli);
        }
        for (kind, rows) in [("record", &self.record_rows), ("event", &self.event_rows)] {
            for (index, row) in rows.iter().enumerate() {
                if row.dot(word.x())? {
                    return Err(CompletedRecordError::NonCommutingPauli { kind, index });
                }
            }
        }
        let (mass, work) = self.evaluate(outcomes, Some(word), a)?;
        Ok(PreparedRecordResult {
            distribution: CompletedMoment(mass),
            work,
        })
    }
    fn evaluate(
        &self,
        outcomes: &BitVector,
        word: Option<&PauliWord>,
        a: &ScalarArithmetic,
    ) -> Result<(CompletedMass, PreparedRecordWork), CompletedRecordError> {
        if outcomes.width() != self.event_rows.len() {
            return Err(CompletedRecordError::DimensionMismatch);
        }
        let mut contributions = Vec::with_capacity(self.pairs.len());
        let mut work = PreparedRecordWork::default();
        for plan in &self.pairs {
            let ket = &self.sources[plan.left];
            let transformed = word.map(|p| p.apply(ket, a)).transpose()?;
            let joint = transformed
                .as_ref()
                .unwrap_or(ket)
                .tensor(&self.sources[plan.right].conjugate(), a)?;
            let rhs = super::super::pair::pair_rhs(&joint, self.width, &self.event_rows, outcomes)?;
            work.support_plan_applications += 1;
            let Some(projected) = plan.support.project(&joint, &rhs)? else {
                continue;
            };
            let mapped = plan
                .boundary
                .apply(&projected.retain_boundary(&self.keep)?)?;
            let (mut offset, columns, phase, coefficient) = mapped.into_parts();
            offset.xor_assign(&self.flips)?;
            let mapped = AffineKet::new(offset, columns, phase, coefficient)?;
            work.pairing_plan_applications += 1;
            if let Some(contribution) = plan.pairing.contract(&mapped, a)? {
                contributions.push(contribution);
            }
        }
        let statistics = CompletedRecordStatistics {
            source_entries: self.sources.len(),
            pair_work: self.pairs.len(),
            retained_terms: contributions.len(),
            max_pair_paths: self.statistics.max_raw_pair_paths,
            estimated_geometry_words: self.cold_words,
            construction_work_bound: self.construction_work,
        };
        Ok((
            CompletedMass {
                width: self.records,
                terms: contributions,
                limits: self.limits,
                statistics,
            },
            work,
        ))
    }
}

#[cfg(test)]
#[path = "prepared_tests.rs"]
mod tests;
