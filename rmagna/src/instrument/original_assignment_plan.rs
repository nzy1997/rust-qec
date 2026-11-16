//! Owner-bound compact assignments over one immutable original source snapshot.
use super::*;
use sha2::{Digest, Sha256};
use std::sync::{
    OnceLock,
    atomic::{AtomicUsize, Ordering},
};
#[path = "original_assignment_certificate.rs"]
mod certificate;
pub use certificate::OriginalAssignmentCertificate;
#[path = "original_noise_layout.rs"]
mod noise_layout;
pub use noise_layout::{
    OriginalCompactNoiseChannel, OriginalCompactNoiseLayout, OriginalNoiseAssignmentStream,
    OriginalNoiseCodeSelection,
};

/// Immutable, owned physical faults and XOR-normalized report names. Sharing is
/// explicit through Arc; mutable ordinary assignments are snapshotted per yield.
#[derive(Clone)]
pub struct OriginalSharedFaults(Arc<SharedFaultData>);
struct SharedFaultData {
    faults: Vec<OriginalFault>,
    report_flips: Vec<String>,
}
impl OriginalSharedFaults {
    pub fn empty() -> Self {
        static EMPTY: OnceLock<Arc<SharedFaultData>> = OnceLock::new();
        Self(Arc::clone(EMPTY.get_or_init(|| {
            Arc::new(SharedFaultData {
                faults: Vec::new(),
                report_flips: Vec::new(),
            })
        })))
    }
    /// Faults cannot be mutated through a shared group.
    /// ```compile_fail
    /// use rmagna::instrument::{OriginalSharedFaults, OriginalPauli};
    /// let group = OriginalSharedFaults::empty();
    /// group.faults()[0].pauli = OriginalPauli::Z;
    /// ```
    pub fn faults(&self) -> &[OriginalFault] {
        &self.0.faults
    }
    pub fn report_flips(&self) -> &[String] {
        &self.0.report_flips
    }
    pub fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
#[path = "original_trace_cache.rs"]
mod trace_cache;
pub use trace_cache::{OriginalTraceCache, OriginalTraceCacheStatistics};
#[path = "original_probability_cache.rs"]
mod probability_cache;
pub use probability_cache::{
    OriginalCachedAssignment, OriginalProbabilityCache, OriginalProbabilityCacheStatistics,
};

#[path = "original_record_proposals.rs"]
mod record_proposals;
pub use record_proposals::{
    OriginalAffineRecordProposal, OriginalIndependentRecordProposal, OriginalProposalDraw,
    OriginalProposalDrawError, OriginalProposalOptions,
};

struct Snapshot {
    instrument: OriginalInstrument,
    strategy: CompilerStrategy,
    arithmetic: ScalarArithmetic,
    input_key_generations: AtomicUsize,
    identity: Arc<()>,
}
/// The source is captured once; every assignment is then compiled literally.
/// Resource refusals and unexpected errors propagate without another backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginalAssignmentExecutionPolicy {
    LiteralPerAssignment,
}
/// A prepared source identity, independent of the lifetime of the caller's IR.
/// Clones share this identity. Constructing a new plan creates a new identity,
/// even for a byte-for-byte equivalent source; compact assignments are local to
/// that explicit preparation session, not portable serialized record inputs.
#[derive(Clone)]
pub struct OriginalAssignmentPlan(Arc<Snapshot>);
pub struct OriginalCompactAssignment {
    owner: Arc<()>,
    word: BitVector,
    shared: OriginalSharedFaults,
}
impl OriginalCompactAssignment {
    /// Compact record bits cannot be edited through the public getter.
    /// ```compile_fail
    /// use rmagna::instrument::OriginalCompactAssignment;
    /// fn edit(assignment: &OriginalCompactAssignment) {
    ///     assignment.word().set(0, true).unwrap();
    /// }
    /// ```
    pub fn word(&self) -> &BitVector {
        &self.word
    }
    pub fn shared_faults(&self) -> &OriginalSharedFaults {
        &self.shared
    }
}
/// Ordinary inputs retain the public complete named-record boundary. Compact
/// inputs carry their source owner as well as their immutable true-bit word.
pub enum OriginalAssignmentInput {
    Ordinary(Arc<OriginalAssignment>),
    Records(Arc<OriginalRecordAssignment>),
    Compact(Arc<OriginalCompactAssignment>),
}
impl From<OriginalAssignment> for OriginalAssignmentInput {
    fn from(value: OriginalAssignment) -> Self {
        Self::Ordinary(Arc::new(value))
    }
}
impl From<Arc<OriginalAssignment>> for OriginalAssignmentInput {
    fn from(value: Arc<OriginalAssignment>) -> Self {
        Self::Ordinary(value)
    }
}
impl From<Arc<OriginalCompactAssignment>> for OriginalAssignmentInput {
    fn from(value: Arc<OriginalCompactAssignment>) -> Self {
        Self::Compact(value)
    }
}
impl From<OriginalRecordAssignment> for OriginalAssignmentInput {
    fn from(value: OriginalRecordAssignment) -> Self {
        Self::Records(Arc::new(value))
    }
}
impl From<Arc<OriginalRecordAssignment>> for OriginalAssignmentInput {
    fn from(value: Arc<OriginalRecordAssignment>) -> Self {
        Self::Records(value)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OriginalAssignmentBatchStatistics {
    pub inputs: usize,
    /// Immutable group references captured in this batch. Consecutive compact
    /// inputs sharing one group reuse that reference; ordinary inputs are
    /// always copied afresh because a unique Arc can change between yields.
    /// This does not count the compiler's independent fault-lowering work.
    pub fault_group_snapshots: usize,
    pub compiled_branches: usize,
    pub collected_callback_decisions: usize,
    pub values: usize,
}
/// The source snapshot is captured at construction, not on the first next().
/// This stream retains only the last immutable shared group, never a consumed
/// assignment or branch. No probabilities or input fingerprints are cached.
pub struct OriginalAssignmentBatch<I> {
    plan: OriginalAssignmentPlan,
    inputs: I,
    previous_group: Option<OriginalSharedFaults>,
    statistics: OriginalAssignmentBatchStatistics,
    stopped: bool,
}
impl OriginalAssignmentPlan {
    pub fn execution_policy(&self) -> OriginalAssignmentExecutionPolicy {
        OriginalAssignmentExecutionPolicy::LiteralPerAssignment
    }
    pub fn new(
        instrument: &OriginalInstrument,
        strategy: CompilerStrategy,
        arithmetic: ScalarArithmetic,
    ) -> Self {
        Self::from_owned(instrument.clone(), strategy, arithmetic)
    }
    pub fn from_owned(
        instrument: OriginalInstrument,
        strategy: CompilerStrategy,
        arithmetic: ScalarArithmetic,
    ) -> Self {
        Self(Arc::new(Snapshot {
            instrument,
            strategy,
            arithmetic,
            input_key_generations: AtomicUsize::new(0),
            identity: Arc::new(()),
        }))
    }
    /// Actual SHA-256 input keys generated by explicit certificate compilation.
    /// Probability and streaming paths never request an input key.
    pub fn input_key_generations(&self) -> usize {
        self.0.input_key_generations.load(Ordering::Relaxed)
    }
    /// Compile one owned input/result certificate. This is not a probability
    /// cache: callbacks still execute freshly, and every JSON export is owned.
    /// The opaque input fingerprint identifies the captured assignment only;
    /// source/session ownership is represented separately by the plan token.
    pub fn compile(
        &self,
        input: impl Into<OriginalAssignmentInput>,
    ) -> Result<OriginalAssignmentCertificate, OriginalError> {
        self.compile_with_kernel(input, |branch, arithmetic| branch.probability(arithmetic))
    }
    /// Evaluate an already captured and collected branch with an explicit
    /// caller-supplied probability kernel, for backend comparisons. The caller's
    /// kernel must return this branch's exact probability mass.
    /// The kernel runs once, after input capture/callback collection. Its error
    /// propagates unchanged; no fallback or callback replay is attempted.
    pub fn compile_with_kernel(
        &self,
        input: impl Into<OriginalAssignmentInput>,
        kernel: impl FnOnce(&OriginalBranch, &ScalarArithmetic) -> Result<ExactScalar, OriginalError>,
    ) -> Result<OriginalAssignmentCertificate, OriginalError> {
        self.probabilities([input.into()])
            .next_with_kernel(|assignment, branch, arithmetic| {
                let value = kernel(&branch, &arithmetic)?;
                let encoded = certificate::input_key_json(&assignment).to_string();
                self.0.input_key_generations.fetch_add(1, Ordering::Relaxed);
                let input_fingerprint = format!("{:x}", Sha256::digest(encoded.as_bytes()));
                Ok(OriginalAssignmentCertificate::new(
                    assignment,
                    branch,
                    value,
                    input_fingerprint,
                    Arc::clone(&self.0.identity),
                ))
            })
            .expect("one input")
    }
    /// Prepare a coherent trace from the captured source. Borrowed construction
    /// cloned that source; owned construction moved it. The returned trace is
    /// independent of this session and owns no callback handles.
    pub fn trace(&self) -> Result<OriginalStaticTrace, OriginalError> {
        self.0
            .instrument
            .compile_trace(self.0.strategy, &self.0.arithmetic)
    }
    pub fn records(&self) -> &[String] {
        &self.0.instrument.records
    }
    pub(super) fn instrument(&self) -> &OriginalInstrument {
        &self.0.instrument
    }
    /// Snapshot group metadata without invoking a controller or a quantum
    /// kernel. Physical wire lifetimes are still checked at branch compilation.
    pub fn shared_faults(
        &self,
        faults: Vec<OriginalFault>,
        report_flips: Vec<String>,
    ) -> Result<OriginalSharedFaults, OriginalError> {
        self.admit_group(&faults, &report_flips)?;
        let instrument = &self.0.instrument;
        let mut flipped = BTreeSet::new();
        for name in report_flips {
            if !flipped.remove(&name) {
                flipped.insert(name);
            }
        }
        let report_flips = instrument
            .records
            .iter()
            .filter(|name| flipped.contains(*name))
            .cloned()
            .collect::<Vec<_>>();
        if faults.is_empty() && report_flips.is_empty() {
            return Ok(OriginalSharedFaults::empty());
        }
        Ok(OriginalSharedFaults(Arc::new(SharedFaultData {
            faults,
            report_flips,
        })))
    }
    pub fn compact(
        &self,
        word: BitVector,
        shared: OriginalSharedFaults,
    ) -> Result<Arc<OriginalCompactAssignment>, OriginalError> {
        if word.width() != self.records().len() {
            return Err(invalid("compact record word has foreign width", ""));
        }
        // A group is transferable data, not an owner token: resolve its names
        // in this source before associating it with a compact assignment.
        self.admit_group(shared.faults(), shared.report_flips())?;
        Ok(Arc::new(OriginalCompactAssignment {
            owner: Arc::clone(&self.0.identity),
            word,
            shared,
        }))
    }
    pub(super) fn admit_group(
        &self,
        faults: &[OriginalFault],
        flips: &[String],
    ) -> Result<(), OriginalError> {
        let instrument = &self.0.instrument;
        admit(
            "max_faults",
            instrument.limits.max_faults,
            faults.len() as u128,
        )?;
        let bytes = faults
            .iter()
            .map(|f| 48 + f.after.len() as u128)
            .sum::<u128>()
            + flips.iter().map(|s| 32 + s.len() as u128).sum::<u128>();
        admit(
            "max_metadata_bytes",
            instrument.limits.max_metadata_bytes,
            bytes,
        )?;
        for name in flips {
            if !instrument.records.contains(name) {
                return Err(invalid("unknown report flip", name));
            }
        }
        for fault in faults {
            if !instrument.operations.iter().any(|op| op.id == fault.after) {
                return Err(invalid("unknown fault anchor", &fault.after));
            }
        }
        Ok(())
    }
    pub fn probabilities<I>(&self, inputs: I) -> OriginalAssignmentBatch<I::IntoIter>
    where
        I: IntoIterator,
        I::Item: Into<OriginalAssignmentInput>,
    {
        OriginalAssignmentBatch {
            plan: self.clone(),
            inputs: inputs.into_iter(),
            previous_group: None,
            statistics: Default::default(),
            stopped: false,
        }
    }
    pub fn probability(
        &self,
        input: impl Into<OriginalAssignmentInput>,
    ) -> Result<ExactScalar, OriginalError> {
        self.probabilities([input.into()])
            .next()
            .expect("one input")
    }
}
impl<I> OriginalAssignmentBatch<I> {
    pub fn statistics(&self) -> OriginalAssignmentBatchStatistics {
        self.statistics
    }
}
impl<I> OriginalAssignmentBatch<I>
where
    I: Iterator,
    I::Item: Into<OriginalAssignmentInput>,
{
    // Both ordinary streaming and explicit certificate compilation execute this
    // same pipeline. FnOnce also prevents an implicit second backend attempt.
    fn next_with_kernel<T>(
        &mut self,
        kernel: impl FnOnce(
            OriginalAssignment,
            OriginalBranch,
            ScalarArithmetic,
        ) -> Result<T, OriginalError>,
    ) -> Option<Result<T, OriginalError>> {
        if self.stopped {
            return None;
        }
        let Some(input) = self.inputs.next() else {
            self.stopped = true;
            return None;
        };
        self.statistics.inputs += 1;
        let result = (|| {
            let assignment = match input.into() {
                OriginalAssignmentInput::Ordinary(assignment) => {
                    self.previous_group = None;
                    self.plan
                        .0
                        .instrument
                        .admit_assignment_metadata(&assignment)?;
                    self.statistics.fault_group_snapshots += 1;
                    (*assignment).clone()
                }
                OriginalAssignmentInput::Records(assignment) => {
                    self.previous_group = None;
                    self.statistics.fault_group_snapshots += 1;
                    self.plan
                        .0
                        .instrument
                        .normalize_record_assignment(&assignment)?
                }
                OriginalAssignmentInput::Compact(assignment) => {
                    // Identity is checked before even looking at the shared
                    // empty-group address. Equal empty faults cannot bypass it.
                    if !Arc::ptr_eq(&self.plan.0.identity, &assignment.owner) {
                        return Err(invalid(
                            "compact assignment belongs to a different IR snapshot",
                            "",
                        ));
                    }
                    if !self
                        .previous_group
                        .as_ref()
                        .is_some_and(|last| last.shares_storage_with(&assignment.shared))
                    {
                        self.previous_group = Some(assignment.shared.clone());
                        self.statistics.fault_group_snapshots += 1;
                    }
                    let metadata = self
                        .plan
                        .records()
                        .iter()
                        .map(|r| 32 + r.len() as u128)
                        .sum::<u128>()
                        + assignment
                            .shared
                            .faults()
                            .iter()
                            .map(|f| 48 + f.after.len() as u128)
                            .sum::<u128>()
                        + assignment
                            .shared
                            .report_flips()
                            .iter()
                            .map(|r| 32 + r.len() as u128)
                            .sum::<u128>();
                    admit(
                        "max_metadata_bytes",
                        self.plan.0.instrument.limits.max_metadata_bytes,
                        metadata,
                    )?;
                    OriginalAssignment {
                        true_values: self
                            .plan
                            .records()
                            .iter()
                            .enumerate()
                            .map(|(j, name)| {
                                (
                                    name.clone(),
                                    assignment.word.get(j).expect("validated record width"),
                                )
                            })
                            .collect(),
                        faults: assignment.shared.faults().to_vec(),
                        report_flips: assignment.shared.report_flips().to_vec(),
                    }
                }
            };
            let snapshot = &self.plan.0;
            let branch = snapshot.instrument.compile(
                &assignment,
                snapshot.strategy,
                &snapshot.arithmetic,
            )?;
            self.statistics.compiled_branches += 1;
            self.statistics.collected_callback_decisions += branch.decisions().len();
            let value = kernel(assignment, branch, snapshot.arithmetic)?;
            self.statistics.values += 1;
            Ok(value)
        })();
        self.stopped = result.is_err();
        Some(result)
    }
}

impl<I> Iterator for OriginalAssignmentBatch<I>
where
    I: Iterator,
    I::Item: Into<OriginalAssignmentInput>,
{
    type Item = Result<ExactScalar, OriginalError>;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_with_kernel(|_assignment, branch, arithmetic| branch.probability(&arithmetic))
    }
}
