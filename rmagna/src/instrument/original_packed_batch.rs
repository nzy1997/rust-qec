//! Explicit no-feedback source preparation with batch-local fault transport reuse.
use super::*;
use crate::{phase::PhaseResourceOptions, records::CompletedProbability};

#[derive(Clone, Copy, Debug)]
pub struct OriginalPackedBatchOptions {
    pub resource: PhaseResourceOptions,
    pub max_record_occurrences: usize,
}
impl Default for OriginalPackedBatchOptions {
    fn default() -> Self {
        Self {
            resource: PhaseResourceOptions::default(),
            max_record_occurrences: 1 << 20,
        }
    }
}
/// One coherent source with no controls; probability laws are never cached here.
pub struct OriginalPackedAssignmentPlan {
    source: OriginalAssignmentPlan,
    trace: OriginalStaticTrace,
    keep: Vec<OriginalRecordParity>,
    options: OriginalPackedBatchOptions,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OriginalPackedBatchStatistics {
    pub inputs: usize,
    /// Successful calls to actual named-lifetime resolution and frame transport.
    pub packed_fault_groups: usize,
    pub prepared_record_laws: usize,
    pub values: usize,
}
pub struct OriginalPackedAssignmentBatch<'a, I> {
    plan: &'a OriginalPackedAssignmentPlan,
    inputs: I,
    cached: Option<(OriginalSharedFaults, Arc<CompletedProbability>)>,
    statistics: OriginalPackedBatchStatistics,
    stopped: bool,
}
impl OriginalAssignmentPlan {
    /// Compile one static source; all controls are rejected before callbacks.
    /// This is distinct from the existing literal-per-assignment pipeline.
    pub fn prepare_packed_probabilities(
        &self,
        options: OriginalPackedBatchOptions,
    ) -> Result<OriginalPackedAssignmentPlan, OriginalError> {
        if let Some(op) = self
            .0
            .instrument
            .operations
            .iter()
            .find(|op| op.gate == OriginalGate::Control)
        {
            return Err(invalid(
                "packed static assignments do not support controls",
                &op.id,
            ));
        }
        admit(
            "max_record_occurrences",
            options.max_record_occurrences,
            self.records().len() as u128,
        )?;
        admit(
            "max_metadata_bytes",
            self.0.instrument.limits.max_metadata_bytes,
            self.records()
                .iter()
                .map(|name| 64 + name.len() as u128)
                .sum(),
        )?;
        let trace = self.trace()?;
        let keep = self
            .records()
            .iter()
            .map(|name| OriginalRecordParity {
                records: vec![name.clone()],
                inverted: false,
            })
            .collect();
        Ok(OriginalPackedAssignmentPlan {
            source: self.clone(),
            trace,
            keep,
            options,
        })
    }
}
impl OriginalPackedAssignmentPlan {
    pub fn probabilities<I>(&self, inputs: I) -> OriginalPackedAssignmentBatch<'_, I::IntoIter>
    where
        I: IntoIterator,
        I::Item: Into<OriginalAssignmentInput>,
    {
        OriginalPackedAssignmentBatch {
            plan: self,
            inputs: inputs.into_iter(),
            cached: None,
            statistics: Default::default(),
            stopped: false,
        }
    }
}
impl<I> OriginalPackedAssignmentBatch<'_, I> {
    pub fn statistics(&self) -> OriginalPackedBatchStatistics {
        self.statistics
    }
    fn ordinary(
        &self,
        assignment: &OriginalAssignment,
    ) -> Result<(BitVector, OriginalSharedFaults, bool), OriginalError> {
        let instrument = &self.plan.source.0.instrument;
        instrument.admit_assignment_metadata(assignment)?;
        let values = instrument.assignment_true_values(assignment)?;
        let word = BitVector::from_indices(
            self.plan.keep.len(),
            self.plan
                .source
                .records()
                .iter()
                .enumerate()
                .filter_map(|(j, name)| values[name.as_str()].then_some(j)),
        )?;
        let group = self
            .plan
            .source
            .shared_faults(assignment.faults.clone(), assignment.report_flips.clone())?;
        Ok((word, group, false))
    }
    fn capture(
        &self,
        input: OriginalAssignmentInput,
    ) -> Result<(BitVector, OriginalSharedFaults, bool), OriginalError> {
        match input {
            OriginalAssignmentInput::Ordinary(assignment) => self.ordinary(&assignment),
            OriginalAssignmentInput::Records(assignment) => self.ordinary(
                &self
                    .plan
                    .source
                    .0
                    .instrument
                    .normalize_record_assignment(&assignment)?,
            ),
            OriginalAssignmentInput::Compact(assignment) => {
                if !Arc::ptr_eq(&self.plan.source.0.identity, &assignment.owner) {
                    return Err(invalid(
                        "compact assignment belongs to a different IR snapshot",
                        "",
                    ));
                }
                let group = &assignment.shared;
                self.plan
                    .source
                    .admit_group(group.faults(), group.report_flips())?;
                let metadata = self
                    .plan
                    .source
                    .records()
                    .iter()
                    .map(|r| 32 + r.len() as u128)
                    .sum::<u128>()
                    + group
                        .faults()
                        .iter()
                        .map(|f| 48 + f.after.len() as u128)
                        .sum::<u128>()
                    + group
                        .report_flips()
                        .iter()
                        .map(|r| 32 + r.len() as u128)
                        .sum::<u128>();
                admit(
                    "max_metadata_bytes",
                    self.plan.source.0.instrument.limits.max_metadata_bytes,
                    metadata,
                )?;
                Ok((assignment.word.clone(), group.clone(), true))
            }
        }
    }
    fn pack_group(
        &mut self,
        group: &OriginalSharedFaults,
    ) -> Result<Arc<CompletedProbability>, OriginalError> {
        // Words are physical true values. No control can turn report flips into
        // a quantum action; their names were still validated during capture.
        let assignment = OriginalTraceAssignment {
            faults: group.faults().to_vec(),
            report_flips: vec![],
        };
        let a = &self.plan.source.0.arithmetic;
        let view = self.plan.trace.with_assignment(&assignment, a)?;
        self.statistics.packed_fault_groups += 1;
        let options = OriginalTraceOptions {
            resource: self.plan.options.resource,
            catalogue: None,
            max_record_occurrences: self.plan.options.max_record_occurrences,
        };
        let law = view
            .probability(&self.plan.keep, &[], &options, a)?
            .distribution;
        self.statistics.prepared_record_laws += 1;
        Ok(Arc::new(law))
    }
}
impl<I> Iterator for OriginalPackedAssignmentBatch<'_, I>
where
    I: Iterator,
    I::Item: Into<OriginalAssignmentInput>,
{
    type Item = Result<ExactScalar, OriginalError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped {
            return None;
        }
        let Some(input) = self.inputs.next() else {
            self.stopped = true;
            self.cached = None;
            return None;
        };
        self.statistics.inputs += 1;
        let result = (|| {
            let (word, group, reusable) = self.capture(input.into())?;
            let law = if reusable
                && self
                    .cached
                    .as_ref()
                    .is_some_and(|(old, _)| old.shares_storage_with(&group))
            {
                Arc::clone(&self.cached.as_ref().unwrap().1)
            } else {
                self.cached = None;
                let law = self.pack_group(&group)?;
                if reusable {
                    self.cached = Some((group, Arc::clone(&law)));
                }
                law
            };
            let value = law.value(&word, &self.plan.source.0.arithmetic)?;
            self.statistics.values += 1;
            Ok(value)
        })();
        if result.is_err() {
            self.stopped = true;
            self.cached = None;
        }
        Some(result)
    }
}
