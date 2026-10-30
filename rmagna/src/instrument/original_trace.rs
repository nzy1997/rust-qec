//! Static coherent records, with no fabricated complete physical assignment.
use super::*;
use crate::catalogue::PhaseCatalogue;
use crate::phase::PhaseResourceOptions;
use crate::records::{CompletedMoment, CompletedProbability};
use crate::unitary::{ExpansionStatistics, OwnedStabilizerExpansion, PauliInsertion};

/// Ordered named XOR. Repeated names cancel; the constant is reported too.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OriginalRecordParity {
    pub records: Vec<String>,
    pub inverted: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalRecordEvent {
    pub parity: OriginalRecordParity,
    pub value: bool,
}
/// Replaces the reference fault/report assignment, never composes with a view.
#[derive(Clone, Debug, Default)]
pub struct OriginalTraceAssignment {
    pub faults: Vec<OriginalFault>,
    pub report_flips: Vec<String>,
}
/// Resource selection is explicit: no implicit catalogue or runtime cache.
#[derive(Clone, Copy, Debug)]
pub struct OriginalTraceOptions<'a> {
    pub resource: PhaseResourceOptions,
    pub catalogue: Option<&'a PhaseCatalogue>,
    /// Total named occurrences across retained parities and events. Requested
    /// name bytes also obey OriginalLimits::max_metadata_bytes before lookup
    /// or copying an unknown identity into an error.
    pub max_record_occurrences: usize,
}
impl Default for OriginalTraceOptions<'_> {
    fn default() -> Self {
        Self {
            resource: PhaseResourceOptions::default(),
            catalogue: None,
            max_record_occurrences: 1 << 20,
        }
    }
}
/// The certificate describes the actual owned side expansion. The distribution
/// statistics count the actual inherited ordered pairs, including zero traces.
pub struct OriginalTraceResult<T> {
    pub distribution: T,
    pub side: ExpansionStatistics,
}
impl<T> OriginalTraceResult<T> {
    pub fn resource_expansion(&self) -> &'static str {
        "inherited-product"
    }
}
struct Anchor {
    position: usize,
    gates: usize,
}
/// Immutable original coherent reference. Histories store lifetime transitions,
/// not a dense operation-by-wire table. Reset starts a fresh physical lifetime;
/// destructive readouts retain their physical record and retire the user wire.
pub struct OriginalStaticTrace {
    reference: CompiledUnitary,
    outputs: Vec<OriginalWire>,
    output_slots: Vec<usize>,
    environment: Vec<OriginalEnvironment>,
    record_names: Vec<String>,
    record_indices: BTreeMap<String, usize>,
    record_slots: Vec<(usize, bool)>,
    anchors: BTreeMap<String, Anchor>,
    histories: BTreeMap<OriginalWire, Vec<(usize, Option<usize>)>>,
    limits: OriginalLimits,
}
impl OriginalInstrument {
    /// Coherently lower every static measurement once. Unresolved controls are
    /// rejected before lowering, fault admission or any callback invocation.
    ///
    /// ```
    /// use rmagna::{clifford::CliffordCircuit, gf2::BitVector,
    ///     instrument::{OriginalInstrument, OriginalLimits, OriginalTraceAssignment,
    ///         OriginalTraceOptions}, scalar::{ExactScalar, ScalarArithmetic},
    ///     unitary::CompilerStrategy};
    /// let a = ScalarArithmetic::default();
    /// let instrument = OriginalInstrument::new(vec![], vec![],
    ///     CliffordCircuit::computational(BitVector::zeros(0)?)?, vec![], vec![],
    ///     OriginalLimits::default(), &a)?;
    /// let reference = instrument.compile_trace(CompilerStrategy::ParityCopies, &a)?;
    /// let view = reference.with_assignment(&OriginalTraceAssignment::default(), &a)?;
    /// let q = view.probability(&[], &[], &OriginalTraceOptions::default(), &a)?;
    /// assert_eq!(q.distribution.total(&a)?, ExactScalar::one());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn compile_trace(
        &self,
        strategy: CompilerStrategy,
        arithmetic: &ScalarArithmetic,
    ) -> Result<OriginalStaticTrace, OriginalError> {
        if let Some(op) = self
            .operations
            .iter()
            .find(|op| op.gate == OriginalGate::Control)
        {
            return Err(invalid("static trace requires resolved controls", &op.id));
        }
        // Admission precedes sparse-history/anchor/record ownership allocations.
        let occurrences = self
            .operations
            .iter()
            .map(|op| op.gate.wire_count() as u128)
            .sum::<u128>();
        admit(
            "max_metadata_bytes",
            self.limits.max_metadata_bytes,
            64 * (self.inputs.len() as u128 + occurrences + self.operations.len() as u128)
                + self
                    .operations
                    .iter()
                    .map(|o| o.id.len() as u128)
                    .sum::<u128>()
                + self
                    .records
                    .iter()
                    .map(|r| 96 + 2 * r.len() as u128)
                    .sum::<u128>(),
        )?;
        let mut lower = BranchLowering::new(&self.inputs, self.limits);
        let mut histories = BTreeMap::<_, Vec<_>>::new();
        for (&wire, &slot) in &lower.live {
            histories.insert(wire, vec![(0, Some(slot))]);
        }
        let mut anchors = BTreeMap::new();
        for (i, op) in self.operations.iter().enumerate() {
            lower.gate(&op.gate, &OriginalOrigin::Operation(op.id.clone()), None)?;
            let position = i + 1;
            anchors.insert(
                op.id.clone(),
                Anchor {
                    position,
                    gates: lower.gates.len(),
                },
            );
            for wire in op.gate.wires() {
                let current = lower.live.get(&wire).copied();
                let history = histories.entry(wire).or_default();
                if history.last().map(|(_, state)| *state) != Some(current) {
                    history.push((position, current));
                }
            }
        }
        if lower.live.len() != self.outputs.len()
            || self.outputs.iter().any(|w| !lower.live.contains_key(w))
        {
            return Err(OriginalError::OutputMismatch);
        }
        let output_slots = self.outputs.iter().map(|w| lower.live[w]).collect();
        admit(
            "max_geometry_words",
            self.limits.expansion.max_geometry_words,
            16 * (lower.slots as u128 * (self.initial.columns().len() as u128 + 1).div_ceil(64)
                + lower.slots as u128
                + self.initial.columns().len() as u128
                + 128),
        )?;
        let initial = with_zero_ancillas(&self.initial, lower.slots)?;
        let reference = UnitaryCircuit::new(lower.slots, lower.gates)?
            .compile_stabilizer_with_path_limit(
                &initial,
                arithmetic,
                strategy,
                self.limits.max_path_bits,
            )?;
        Ok(OriginalStaticTrace {
            reference,
            outputs: self.outputs.clone(),
            output_slots,
            environment: lower.environment,
            record_names: self.records.clone(),
            record_indices: self
                .records
                .iter()
                .enumerate()
                .map(|(j, r)| (r.clone(), j))
                .collect(),
            record_slots: lower.record_slots,
            anchors,
            histories,
            limits: self.limits,
        })
    }
}
impl OriginalStaticTrace {
    pub fn outputs(&self) -> &[OriginalWire] {
        &self.outputs
    }
    pub fn environment(&self) -> &[OriginalEnvironment] {
        &self.environment
    }
    pub fn records(&self) -> &[String] {
        &self.record_names
    }
    pub fn compilation(&self) -> &CompilationDiagnostics {
        self.reference.compilation()
    }
    pub fn physical_width(&self) -> usize {
        self.reference.width()
    }
    /// Each supplied fault is resolved at its named original anchor. Anchors
    /// with zero emitted gates remain distinct, in original chronological order.
    pub fn with_assignment<'a>(
        &'a self,
        assignment: &OriginalTraceAssignment,
        arithmetic: &ScalarArithmetic,
    ) -> Result<OriginalTraceView<'a>, OriginalError> {
        admit(
            "max_faults",
            self.limits.max_faults,
            assignment.faults.len() as u128,
        )?;
        let bytes = assignment
            .faults
            .iter()
            .map(|f| 64 + f.after.len() as u128)
            .sum::<u128>()
            + assignment
                .report_flips
                .iter()
                .map(|r| 32 + r.len() as u128)
                .sum::<u128>();
        admit("max_metadata_bytes", self.limits.max_metadata_bytes, bytes)?;
        admit(
            "max_geometry_words",
            self.limits.expansion.max_geometry_words,
            (self.physical_width() as u128).div_ceil(64) * 2 * assignment.faults.len() as u128
                + 16 * assignment.faults.len() as u128
                + self.record_names.len() as u128
                + 128,
        )?;
        let mut flips = vec![false; self.record_names.len()];
        for name in &assignment.report_flips {
            let &j = self
                .record_indices
                .get(name)
                .ok_or_else(|| invalid("unknown report flip", name))?;
            flips[j] = !flips[j];
        }
        let mut resolved = Vec::with_capacity(assignment.faults.len());
        for (ordinal, fault) in assignment.faults.iter().enumerate() {
            let anchor = self
                .anchors
                .get(&fault.after)
                .ok_or_else(|| invalid("unknown fault anchor", &fault.after))?;
            if fault.pauli == OriginalPauli::I {
                continue;
            }
            let slot = self
                .histories
                .get(&fault.wire)
                .and_then(|h| {
                    h.get(
                        h.partition_point(|(position, _)| *position <= anchor.position)
                            .checked_sub(1)?,
                    )
                    .and_then(|(_, state)| *state)
                })
                .ok_or(OriginalError::Lifetime {
                    wire: fault.wire,
                    expected_live: true,
                })?;
            resolved.push((anchor.position, ordinal, anchor.gates, slot, fault.pauli));
        }
        resolved.sort_by_key(|(position, ordinal, ..)| (*position, *ordinal));
        let mut insertions = Vec::with_capacity(resolved.len());
        for (_, _, before_gate, slot, pauli) in resolved {
            let x = if matches!(pauli, OriginalPauli::X | OriginalPauli::Y) {
                vec![slot]
            } else {
                vec![]
            };
            let z = if matches!(pauli, OriginalPauli::Z | OriginalPauli::Y) {
                vec![slot]
            } else {
                vec![]
            };
            insertions.push(PauliInsertion {
                before_gate,
                word: PauliWord::new(
                    BitVector::from_indices(self.physical_width(), x)?,
                    BitVector::from_indices(self.physical_width(), z)?,
                    if pauli == OriginalPauli::Y { 2 } else { 0 },
                )?,
            });
        }
        let unitary = self.reference.with_insertions(&insertions, arithmetic)?;
        Ok(OriginalTraceView {
            source: self,
            unitary,
            flips,
        })
    }
}
/// Independent frozen assignment view; the source is reusable on every error.
pub struct OriginalTraceView<'a> {
    source: &'a OriginalStaticTrace,
    unitary: CompiledUnitary,
    flips: Vec<bool>,
}
impl OriginalTraceView<'_> {
    pub fn expansion(
        &self,
        options: &OriginalTraceOptions<'_>,
        a: &ScalarArithmetic,
    ) -> Result<OwnedStabilizerExpansion, OriginalError> {
        Ok(self.unitary.stabilizer_expansion_with_resource(
            self.source.limits.expansion,
            options.resource,
            options.catalogue,
            a,
        )?)
    }
    fn parity(&self, parity: &OriginalRecordParity) -> Result<(BitVector, bool), OriginalError> {
        let mut row = BitVector::zeros(self.unitary.width())?;
        let mut flip = parity.inverted;
        for name in &parity.records {
            let &j = self
                .source
                .record_indices
                .get(name)
                .ok_or_else(|| invalid("unknown record parity", name))?;
            let (slot, inverted) = self.source.record_slots[j];
            row.set(slot, !row.get(slot)?)?;
            flip ^= self.flips[j] ^ inverted;
        }
        Ok((row, flip))
    }
    fn spec(
        &self,
        keep: &[OriginalRecordParity],
        events: &[OriginalRecordEvent],
        options: &OriginalTraceOptions<'_>,
    ) -> Result<CompletedRecordSpec, OriginalError> {
        let limits = self.source.limits;
        admit(
            "max_record_rows",
            limits
                .completed
                .max_record_rows
                .min(limits.expansion.max_record_rows),
            keep.len() as u128,
        )?;
        admit(
            "max_event_rows",
            limits
                .completed
                .max_event_rows
                .min(limits.expansion.max_event_rows),
            events.len() as u128,
        )?;
        let occurrences = keep
            .iter()
            .map(|p| p.records.len() as u128)
            .chain(events.iter().map(|e| e.parity.records.len() as u128))
            .sum::<u128>();
        admit(
            "max_record_occurrences",
            options.max_record_occurrences,
            occurrences,
        )?;
        let rows = keep.len() as u128 + events.len() as u128;
        let name_bytes = keep
            .iter()
            .chain(events.iter().map(|e| &e.parity))
            .flat_map(|p| &p.records)
            .map(|name| 32 + name.len() as u128)
            .sum::<u128>();
        admit(
            "max_metadata_bytes",
            limits.max_metadata_bytes,
            64 * rows + name_bytes,
        )?;
        admit(
            "max_geometry_words",
            limits
                .completed
                .max_geometry_words
                .min(limits.expansion.max_geometry_words),
            16 * (rows * ((self.unitary.width() as u128).div_ceil(64) + 8)
                + self.unitary.width() as u128
                + 128),
        )?;
        let mut flips = BitVector::zeros(keep.len())?;
        let records = keep
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let (row, flip) = self.parity(p)?;
                flips.set(i, flip)?;
                Ok(row)
            })
            .collect::<Result<Vec<_>, OriginalError>>()?;
        let mut outcomes = BitVector::zeros(events.len())?;
        let event_rows = events
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let (row, flip) = self.parity(&e.parity)?;
                outcomes.set(i, e.value ^ flip)?;
                Ok(row)
            })
            .collect::<Result<Vec<_>, OriginalError>>()?;
        Ok(CompletedRecordSpec::new(
            self.unitary.width(),
            &records,
            &event_rows,
            &outcomes,
            limits.completed,
        )?
        .with_report_flips(&flips)?)
    }
    fn admit_side(&self, side: &OwnedStabilizerExpansion) -> Result<(), OriginalError> {
        admit(
            "max_pairs",
            self.source.limits.expansion.max_pairs,
            (side.entries().len() as u128).pow(2),
        )
    }
    pub fn probability(
        &self,
        keep: &[OriginalRecordParity],
        events: &[OriginalRecordEvent],
        options: &OriginalTraceOptions<'_>,
        a: &ScalarArithmetic,
    ) -> Result<OriginalTraceResult<CompletedProbability>, OriginalError> {
        let spec = self.spec(keep, events, options)?;
        let side = self.expansion(options, a)?;
        self.admit_side(&side)?;
        Ok(OriginalTraceResult {
            distribution: spec.probability(side.entries(), a)?,
            side: side.statistics().clone(),
        })
    }
    /// Physical Hermitian output moment. Other live/retired/record legs carry I.
    /// Signed results expose no probability sampling method.
    pub fn output_moment(
        &self,
        keep: &[OriginalRecordParity],
        events: &[OriginalRecordEvent],
        word: &PauliWord,
        options: &OriginalTraceOptions<'_>,
        a: &ScalarArithmetic,
    ) -> Result<OriginalTraceResult<CompletedMoment>, OriginalError> {
        if word.width() != self.source.outputs.len() {
            return Err(OriginalError::DimensionMismatch);
        }
        let spec = self.spec(keep, events, options)?;
        let word = PauliWord::new(
            BitVector::from_indices(
                self.unitary.width(),
                word.x().ones().map(|j| self.source.output_slots[j]),
            )?,
            BitVector::from_indices(
                self.unitary.width(),
                word.z().ones().map(|j| self.source.output_slots[j]),
            )?,
            word.phase(),
        )?;
        let side = self.expansion(options, a)?;
        self.admit_side(&side)?;
        Ok(OriginalTraceResult {
            distribution: spec.moment(side.entries(), &word, a)?,
            side: side.statistics().clone(),
        })
    }
}
