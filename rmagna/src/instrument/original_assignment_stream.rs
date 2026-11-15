//! Strict integer record import and lazy, fail-fast complete-assignment evaluation.
use super::*;
use serde_json::Value;

/// Dynamic record boundary. Only JSON integer 0/1 is accepted; booleans,
/// floating-point numbers, nulls and all other JSON values are rejected.
/// Record names must form the source's complete set, exactly once.
#[derive(Clone, Debug, Default)]
pub struct OriginalRecordAssignment {
    pub records: Vec<(String, Value)>,
    pub report_flips: Vec<String>,
    pub faults: Vec<OriginalFault>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OriginalAssignmentStreamStatistics {
    pub inputs: usize,
    pub compiled_branches: usize,
    pub collected_callback_decisions: usize,
    pub values: usize,
}
/// Holds no consumed assignment or compiled branch. Errors terminate the stream;
/// statistics count completed operations, including a compiled zero branch.
pub struct OriginalAssignmentStream<'a, I> {
    instrument: &'a OriginalInstrument,
    assignments: I,
    strategy: CompilerStrategy,
    arithmetic: &'a ScalarArithmetic,
    statistics: OriginalAssignmentStreamStatistics,
    stopped: bool,
}
impl OriginalInstrument {
    /// Validate the complete ordinary record mapping before compiling any
    /// callback, fault insertion or probability query. Resource admission runs
    /// before constructing the owned typed assignment.
    pub fn compile_record_assignment(
        &self,
        assignment: OriginalRecordAssignment,
        strategy: CompilerStrategy,
        arithmetic: &ScalarArithmetic,
    ) -> Result<OriginalBranch, OriginalError> {
        let typed = self.normalize_record_assignment(&assignment)?;
        self.compile(&typed, strategy, arithmetic)
    }
    pub(super) fn normalize_record_assignment(
        &self,
        assignment: &OriginalRecordAssignment,
    ) -> Result<OriginalAssignment, OriginalError> {
        admit(
            "max_faults",
            self.limits.max_faults,
            assignment.faults.len() as u128,
        )?;
        let bytes = assignment
            .records
            .iter()
            .map(|(r, _)| 32 + r.len() as u128)
            .sum::<u128>()
            + assignment
                .report_flips
                .iter()
                .map(|r| 32 + r.len() as u128)
                .sum::<u128>()
            + assignment
                .faults
                .iter()
                .map(|f| 48 + f.after.len() as u128)
                .sum::<u128>();
        admit("max_metadata_bytes", self.limits.max_metadata_bytes, bytes)?;
        let known = self
            .records
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        for (name, value) in &assignment.records {
            if !known.contains(name.as_str()) {
                return Err(invalid("unknown true record", name));
            }
            if !seen.insert(name.as_str()) {
                return Err(invalid("duplicate true record", name));
            }
            if !matches!(value.as_i64(), Some(0 | 1)) {
                return Err(invalid("record value must be integer zero or one", name));
            }
        }
        for name in &self.records {
            if !seen.contains(name.as_str()) {
                return Err(invalid("missing true record", name));
            }
        }
        drop(seen);
        Ok(OriginalAssignment {
            true_values: assignment
                .records
                .iter()
                .map(|(name, value)| (name.clone(), value.as_i64() == Some(1)))
                .collect(),
            report_flips: assignment.report_flips.clone(),
            faults: assignment.faults.clone(),
        })
    }
    /// No input is pulled, branch compiled or callback invoked at construction.
    /// Empty streams therefore have zero values and all-zero work counters.
    pub fn evaluate_record_assignments<'a, I>(
        &'a self,
        assignments: I,
        strategy: CompilerStrategy,
        arithmetic: &'a ScalarArithmetic,
    ) -> OriginalAssignmentStream<'a, I::IntoIter>
    where
        I: IntoIterator<Item = OriginalRecordAssignment>,
    {
        OriginalAssignmentStream {
            instrument: self,
            assignments: assignments.into_iter(),
            strategy,
            arithmetic,
            statistics: Default::default(),
            stopped: false,
        }
    }
}
impl<I> OriginalAssignmentStream<'_, I> {
    pub fn statistics(&self) -> OriginalAssignmentStreamStatistics {
        self.statistics
    }
}
impl<I: Iterator<Item = OriginalRecordAssignment>> Iterator for OriginalAssignmentStream<'_, I> {
    type Item = Result<ExactScalar, OriginalError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped {
            return None;
        }
        let Some(assignment) = self.assignments.next() else {
            self.stopped = true;
            return None;
        };
        self.statistics.inputs += 1;
        let result = (|| {
            let branch = self.instrument.compile_record_assignment(
                assignment,
                self.strategy,
                self.arithmetic,
            )?;
            self.statistics.compiled_branches += 1;
            self.statistics.collected_callback_decisions += branch.decisions().len();
            let value = branch.probability(self.arithmetic)?;
            self.statistics.values += 1;
            Ok(value)
        })();
        self.stopped = result.is_err();
        Some(result)
    }
}
