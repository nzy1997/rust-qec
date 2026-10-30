//! Explicit retained geometry for one immutable original assignment view.
use super::*;
use crate::records::{
    PreparedCompletedRecords, PreparedRecordLimits, PreparedRecordStatistics, PreparedRecordWork,
};

/// Distinct preparation-only source-certificate and retained-record budgets.
#[derive(Clone, Copy, Debug)]
pub struct OriginalTracePreparationLimits {
    pub max_source_quotient_rank: usize,
    pub records: PreparedRecordLimits,
}
impl Default for OriginalTracePreparationLimits {
    fn default() -> Self {
        Self {
            max_source_quotient_rank: usize::MAX,
            records: PreparedRecordLimits::default(),
        }
    }
}
pub struct OriginalPreparedTrace<'a> {
    source: &'a OriginalStaticTrace,
    plan: PreparedCompletedRecords,
    event_flips: BitVector,
    side: ExpansionStatistics,
}
pub struct OriginalPreparedTraceResult<T> {
    pub distribution: T,
    pub side: ExpansionStatistics,
    pub work: PreparedRecordWork,
}
impl<T> OriginalPreparedTraceResult<T> {
    pub fn resource_expansion(&self) -> &'static str {
        "inherited-product"
    }
}
impl OriginalTraceView<'_> {
    /// Prepare actual conjugate-pair support and Gaussian contraction programs
    /// for this fixed assignment, retained rows and event parities. The supplied
    /// event values do not select preparation geometry. Subsequent calls supply
    /// a complete reported event vector, in the same order, including duplicate
    /// and constant rows. Their inversions and report flips remain fixed.
    ///
    /// Owns the actual side expansion, never reconstructs a phase certificate,
    /// and does not install an automatic cache. Changing faults/report flips
    /// requires another preparation. Later distribution mass/sampling queries
    /// still use the existing CompletedProbability/CompletedMoment API.
    pub fn prepare_records(
        &self,
        keep: &[OriginalRecordParity],
        events: &[OriginalRecordEvent],
        options: &OriginalTraceOptions<'_>,
        limits: PreparedRecordLimits,
        a: &ScalarArithmetic,
    ) -> Result<OriginalPreparedTrace<'_>, OriginalError> {
        self.prepare_records_with_limits(
            keep,
            events,
            options,
            OriginalTracePreparationLimits {
                records: limits,
                ..Default::default()
            },
            a,
        )
    }
    /// Also bound the actual source phase quotient before constructing its
    /// resource expansion. This cap is separate from cold raw structural paths.
    pub fn prepare_records_with_limits(
        &self,
        keep: &[OriginalRecordParity],
        events: &[OriginalRecordEvent],
        options: &OriginalTraceOptions<'_>,
        limits: OriginalTracePreparationLimits,
        a: &ScalarArithmetic,
    ) -> Result<OriginalPreparedTrace<'_>, OriginalError> {
        let spec = self.spec(keep, events, options)?;
        let side = self.unitary.stabilizer_expansion_with_resource_rank_limit(
            self.source.limits.expansion,
            options.resource,
            options.catalogue,
            limits.max_source_quotient_rank,
            a,
        )?;
        self.admit_side(&side)?;
        let plan = spec.prepare(side.entries(), limits.records, a)?;
        let mut event_flips = spec.event_outcomes().clone();
        for (j, event) in events.iter().enumerate() {
            event_flips.set(j, event_flips.get(j)? ^ event.value)?;
        }
        Ok(OriginalPreparedTrace {
            source: self.source,
            plan,
            event_flips,
            side: side.statistics().clone(),
        })
    }
}
impl OriginalPreparedTrace<'_> {
    pub fn statistics(&self) -> &PreparedRecordStatistics {
        self.plan.statistics()
    }
    pub fn side_statistics(&self) -> &ExpansionStatistics {
        &self.side
    }
    fn outcomes(&self, reported: &BitVector) -> Result<BitVector, OriginalError> {
        if reported.width() != self.event_flips.width() {
            return Err(OriginalError::DimensionMismatch);
        }
        let mut physical = reported.clone();
        physical.xor_assign(&self.event_flips)?;
        Ok(physical)
    }
    pub fn probability(
        &self,
        outcomes: &BitVector,
        a: &ScalarArithmetic,
    ) -> Result<OriginalPreparedTraceResult<CompletedProbability>, OriginalError> {
        let q = self.plan.probability(&self.outcomes(outcomes)?, a)?;
        Ok(OriginalPreparedTraceResult {
            distribution: q.distribution,
            side: self.side.clone(),
            work: q.work,
        })
    }
    pub fn output_moment(
        &self,
        outcomes: &BitVector,
        word: &PauliWord,
        a: &ScalarArithmetic,
    ) -> Result<OriginalPreparedTraceResult<CompletedMoment>, OriginalError> {
        if word.width() != self.source.outputs.len() {
            return Err(OriginalError::DimensionMismatch);
        }
        let physical = self.outcomes(outcomes)?;
        let word = PauliWord::new(
            BitVector::from_indices(
                self.source.physical_width(),
                word.x().ones().map(|j| self.source.output_slots[j]),
            )?,
            BitVector::from_indices(
                self.source.physical_width(),
                word.z().ones().map(|j| self.source.output_slots[j]),
            )?,
            word.phase(),
        )?;
        let q = self.plan.moment(&physical, &word, a)?;
        Ok(OriginalPreparedTraceResult {
            distribution: q.distribution,
            side: self.side.clone(),
            work: q.work,
        })
    }
}
