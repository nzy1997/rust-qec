//! A suffix compiled from a shared raw reference, applied to existing frames.
use super::*;
use crate::compact::CompactPauliWord;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug)]
pub struct ContinuationOptions {
    /// None inherits the source's compiler strategy.
    pub compiler: Option<CompilerStrategy>,
    pub insertions: InsertionPreparationOptions,
    pub max_path_bits: usize,
    pub max_coherent_width: usize,
    pub max_suffix_gates: usize,
    /// Conservative dimension-based geometry/scratch admission, in u64 words.
    /// Excludes the existing shared source, arbitrary-precision scalar limbs,
    /// allocator overhead and separately prepared scalar-query resources.
    pub max_model_geometry_words: usize,
}
impl Default for ContinuationOptions {
    fn default() -> Self {
        Self {
            compiler: None,
            insertions: Default::default(),
            max_path_bits: 65536,
            max_coherent_width: 65536,
            max_suffix_gates: 1_000_000,
            max_model_geometry_words: 1 << 24,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContinuationStatistics {
    pub source_physical_width: usize,
    pub target_physical_width: usize,
    pub source_phase_variables: usize,
    pub suffix_phase_variables: usize,
    pub endpoint_generators: usize,
    pub source_original_positions: usize,
    pub suffix_original_positions: usize,
    pub query_original_positions: usize,
    pub unique_program_owners: usize,
    pub unique_reference_boundaries: usize,
    pub source_phase_axes: usize,
    pub estimated_model_geometry_words: u128,
    pub suffix_compilations: u64,
    pub transport_tables_prepared: u64,
    pub frame_applications: u64,
    pub numeric_phase_updates: u64,
    pub saturated: bool,
}
#[derive(Default)]
struct Work {
    compiles: AtomicU64,
    tables: AtomicU64,
    applications: AtomicU64,
    phase_updates: AtomicU64,
}
impl Work {
    fn record(counter: &AtomicU64) {
        let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
            Some(v.saturating_add(1))
        });
    }
}
pub struct PreparedContinuation {
    source: Arc<InsertionProgram>,
    source_physical: usize,
    suffix: CompiledUnitary,
    reference: CompiledUnitary,
    transport: PreparedInsertions,
    lifted: Vec<usize>,
    phase_axes: HashMap<BitVector, BitVector>,
    suffix_terms: Vec<(BitVector, u8)>,
    suffix_constant: u8,
    model_words: u128,
    work: Work,
}
/// A numeric continuation view. Its source fault locations are not replayed.
pub struct ContinuedUnitary {
    inner: CompiledUnitary,
}
fn admit(option: &'static str, required: u128, limit: usize) -> Result<(), UnitaryError> {
    if required > limit as u128 {
        return Err(UnitaryError::ContinuationResourceLimit {
            option,
            limit,
            required,
        });
    }
    Ok(())
}
fn phase_bound(
    source: &InsertionProgram,
    suffix: &UnitaryCircuit,
    physical: usize,
    options: ContinuationOptions,
) -> Result<u128, UnitaryError> {
    admit(
        "max_suffix_gates",
        suffix.gates().len() as u128,
        options.max_suffix_gates,
    )?;
    let extra = suffix
        .gates()
        .iter()
        .try_fold(0usize, |n, gate| {
            n.checked_add(match gate {
                UnitaryGate::Clifford(_) => 0,
                UnitaryGate::T { .. } => 1,
                UnitaryGate::Cs { .. } => 3,
                UnitaryGate::Ccz { .. } => 7,
            })
        })
        .ok_or(UnitaryError::DimensionOverflow)?;
    let width = physical
        .checked_add(source.reference_phase.num_variables())
        .and_then(|n| n.checked_add(extra))
        .ok_or(UnitaryError::DimensionOverflow)?;
    admit(
        "max_coherent_width",
        width as u128,
        options.max_coherent_width,
    )?;
    let hidden = source.shared.as_ref().map_or(0, |s| s.phase_axes().count());
    let terms = source.reference_phase.terms().len() as u128 + hidden as u128 + extra as u128 + 1;
    let d = width as u128 + 1;
    let row = d.div_ceil(64) + 4;
    // Covers dense working Gaussian/coordinate matrices, the endpoint catalog
    // and its suffix table, phase-map keys/values and expanded gate metadata.
    let words = 256u128
        .saturating_mul(d)
        .saturating_mul(row)
        .saturating_add(32u128.saturating_mul(terms).saturating_mul(row))
        .saturating_add(1024u128.saturating_mul(suffix.gates().len() as u128 + 1));
    admit(
        "max_model_geometry_words",
        words,
        options.max_model_geometry_words,
    )?;
    Ok(words)
}
fn lift_bits(
    bits: &BitVector,
    width: usize,
    source_physical: usize,
    target_physical: usize,
) -> Result<BitVector, Gf2Error> {
    BitVector::from_indices(
        width,
        bits.ones().map(|q| {
            if q < source_physical {
                q
            } else {
                target_physical + (q - source_physical)
            }
        }),
    )
}
fn embed_input(
    source: &InsertionProgram,
    old: usize,
    new: usize,
) -> Result<AffineKet, UnitaryError> {
    let width = new
        .checked_add(source.reference_phase.num_variables())
        .ok_or(UnitaryError::DimensionOverflow)?;
    let input = &source.reference;
    Ok(AffineKet::new(
        lift_bits(input.offset(), width, old, new)?,
        input
            .columns()
            .iter()
            .map(|c| lift_bits(c, width, old, new))
            .collect::<Result<_, _>>()?,
        input.phase().clone(),
        input.coefficient().clone(),
    )?)
}
fn extend_phase(axis: &BitVector, total: usize, offset: usize) -> Result<BitVector, Gf2Error> {
    BitVector::from_indices(total, axis.ones().map(|q| q + offset))
}
fn unreduced(mut suffix: CompiledUnitary) -> Result<CompiledUnitary, UnitaryError> {
    let program = Arc::get_mut(&mut suffix.program).ok_or(UnitaryError::IncompatibleProgram)?;
    program.query_reference = Arc::clone(&program.reference);
    program.query_reference_phase = Arc::clone(&program.reference_phase);
    program.observation_reduction = None;
    suffix.boundary = Arc::clone(&program.reference);
    suffix.phase = Arc::clone(&program.reference_phase);
    let mut diagnostics = (*suffix.compilation).clone();
    diagnostics.coherent_width = program.reference.width();
    diagnostics.shared_phase_coordinates = diagnostics.shared_observations_before_reduction;
    suffix.compilation = Arc::new(diagnostics);
    Ok(suffix)
}
fn endpoint_catalog(width: usize) -> Result<Vec<PauliInsertion>, UnitaryError> {
    let count = width
        .checked_mul(2)
        .ok_or(UnitaryError::DimensionOverflow)?;
    let mut catalog = Vec::with_capacity(count);
    for q in 0..width {
        catalog.push(PauliInsertion {
            before_gate: 0,
            word: PauliWord::new(
                BitVector::from_indices(width, [q])?,
                BitVector::zeros(width)?,
                0,
            )?,
        });
        catalog.push(PauliInsertion {
            before_gate: 0,
            word: PauliWord::new(
                BitVector::zeros(width)?,
                BitVector::from_indices(width, [q])?,
                0,
            )?,
        });
    }
    Ok(catalog)
}
impl CompiledUnitary {
    /// Freeze a suffix against this immutable original reference. Applying a
    /// numeric view later carries its raw frame/phase, without source replay.
    /// Suffix physical slots include the source's existing dilation slots;
    /// additional physical zeros are inserted before the old magic coordinates.
    pub fn prepare_continuation(
        &self,
        suffix: &UnitaryCircuit,
        options: ContinuationOptions,
        a: &ScalarArithmetic,
    ) -> Result<PreparedContinuation, UnitaryError> {
        if suffix.width() < self.width() {
            return Err(UnitaryError::DimensionMismatch);
        }
        let source = &self.program;
        if source.reference.width().checked_sub(self.width())
            != Some(source.reference_phase.num_variables())
        {
            return Err(UnitaryError::DimensionMismatch);
        }
        let model_words = phase_bound(source, suffix, suffix.width(), options)?;
        let input_width = suffix
            .width()
            .checked_add(source.reference_phase.num_variables())
            .ok_or(UnitaryError::DimensionOverflow)?;
        let count = input_width
            .checked_mul(2)
            .ok_or(UnitaryError::DimensionOverflow)?;
        if count > options.insertions.max_catalog_entries {
            return Err(UnitaryError::InsertionResourceLimit {
                option: "max_catalog_entries",
                limit: options.insertions.max_catalog_entries,
                required: count as u128,
            });
        }
        let input = embed_input(source, self.width(), suffix.width())?;
        let circuit = UnitaryCircuit::new(input.width(), suffix.gates().iter().copied())?;
        let compiled = unreduced(circuit.compile_stabilizer_with_path_limit(
            &input,
            a,
            options.compiler.unwrap_or(self.compilation.strategy),
            options.max_path_bits,
        )?)?;
        let work = Work::default();
        Work::record(&work.compiles);
        let count = input
            .width()
            .checked_mul(2)
            .ok_or(UnitaryError::DimensionOverflow)?;
        compiled.insertion_preparation_requirement(count, options.insertions)?;
        let transport =
            compiled.prepare_insertions(&endpoint_catalog(input.width())?, options.insertions)?;
        Work::record(&work.tables);
        PreparedContinuation::assemble(self, compiled, transport, model_words, work)
    }
}
impl PreparedContinuation {
    fn assemble(
        source: &CompiledUnitary,
        suffix: CompiledUnitary,
        transport: PreparedInsertions,
        model_words: u128,
        work: Work,
    ) -> Result<Self, UnitaryError> {
        let old = source.program.reference_phase.num_variables();
        let new = suffix.program.reference_phase.num_variables();
        let total = old
            .checked_add(new)
            .ok_or(UnitaryError::DimensionOverflow)?;
        let mut phase_axes = HashMap::new();
        for (axis, _) in source.program.reference_phase.terms() {
            phase_axes.insert(axis.clone(), extend_phase(axis, total, 0)?);
        }
        if let Some(updates) = &source.program.shared {
            for axis in updates.phase_axes() {
                phase_axes
                    .entry(axis.clone())
                    .or_insert(extend_phase(axis, total, 0)?);
            }
        }
        let suffix_terms = suffix
            .program
            .reference_phase
            .terms()
            .iter()
            .map(|(axis, c)| Ok((extend_phase(axis, total, old)?, *c)))
            .collect::<Result<Vec<_>, UnitaryError>>()?;
        let suffix_constant = suffix.program.reference_phase.constant();
        let phase = Arc::new(combine_phase(
            &source.program.reference_phase,
            &phase_axes,
            &suffix_terms,
            suffix_constant,
            total,
        )?);
        let physical = suffix.width() - old;
        let reference_boundary = Arc::clone(&suffix.program.reference);
        let program = Arc::new(InsertionProgram {
            scalar_geometry: Default::default(),
            work: Arc::default(),
            reference: Arc::clone(&reference_boundary),
            query_reference: Arc::clone(&reference_boundary),
            observation_reduction: None,
            reference_phase: Arc::clone(&phase),
            query_reference_phase: Arc::clone(&phase),
            shared: None,
            circuit: CliffordCircuit::new(reference_boundary.width(), [])?,
            positions: vec![0],
        });
        let reference = CompiledUnitary {
            physical_width: physical,
            boundary: reference_boundary,
            phase,
            assignment: None,
            program,
            compilation: Arc::clone(&suffix.compilation),
        };
        let lifted = (0..source.program.reference.width())
            .map(|q| {
                if q < source.width() {
                    q
                } else {
                    physical + (q - source.width())
                }
            })
            .collect();
        Ok(Self {
            source: Arc::clone(&source.program),
            source_physical: source.width(),
            suffix,
            reference,
            transport,
            lifted,
            phase_axes,
            suffix_terms,
            suffix_constant,
            model_words,
            work,
        })
    }
    /// Apply only a numeric assignment owned by the captured source program.
    /// No suffix compilation or symbolic-map construction occurs here.
    pub fn apply(
        &self,
        source: &CompiledUnitary,
        a: &ScalarArithmetic,
    ) -> Result<ContinuedUnitary, UnitaryError> {
        if !Arc::ptr_eq(&source.program, &self.source) || source.width() != self.source_physical {
            return Err(UnitaryError::IncompatibleProgram);
        }
        let original = source.assignment.as_deref();
        if let Some(assignment) = original {
            if assignment.frame.width() != self.source.reference.width()
                || assignment.phase.num_variables() != self.source.reference_phase.num_variables()
            {
                return Err(UnitaryError::DimensionMismatch);
            }
        }
        let mut indices = Vec::new();
        if let Some(assignment) = original {
            // The catalog compositor left-multiplies: Z inputs precede X to
            // represent the source convention omega^p X^x Z^z exactly.
            indices.extend(assignment.frame.z().ones().map(|q| 2 * self.lifted[q] + 1));
            indices.extend(assignment.frame.x().ones().map(|q| 2 * self.lifted[q]));
        }
        let mut frame = self.transport.transported_frame(&indices)?;
        if let Some(assignment) = original {
            frame = frame.with_phase((frame.phase() + assignment.frame.phase()) & 7)?;
        }
        let phase = if let Some(assignment) = original {
            let phase = Arc::new(combine_phase(
                &assignment.phase,
                &self.phase_axes,
                &self.suffix_terms,
                self.suffix_constant,
                self.reference.phase.num_variables(),
            )?);
            Work::record(&self.work.phase_updates);
            phase
        } else {
            Arc::clone(&self.reference.phase)
        };
        let boundary = if frame.phase() == 0
            && frame.x().ones().next().is_none()
            && frame.z().ones().next().is_none()
        {
            a.multiply(self.reference.boundary.coefficient(), &ExactScalar::one())?;
            Arc::clone(&self.reference.boundary)
        } else {
            Arc::new(frame.materialize()?.apply(&self.reference.boundary, a)?)
        };
        let assignment = Some(Arc::new(InsertionAssignment {
            frame,
            phase: Arc::clone(&phase),
        }));
        let inner = CompiledUnitary {
            physical_width: self.reference.width(),
            boundary,
            phase,
            assignment,
            program: Arc::clone(&self.reference.program),
            compilation: Arc::clone(&self.reference.compilation),
        };
        Work::record(&self.work.applications);
        Ok(ContinuedUnitary { inner })
    }
    pub fn statistics(&self) -> ContinuationStatistics {
        let programs = [&self.source, &self.suffix.program, &self.reference.program];
        let owners = programs
            .iter()
            .map(|p| Arc::as_ptr(p))
            .collect::<HashSet<_>>()
            .len();
        let boundaries = programs
            .iter()
            .flat_map(|p| [Arc::as_ptr(&p.reference), Arc::as_ptr(&p.query_reference)])
            .collect::<HashSet<_>>()
            .len();
        let counts = [
            &self.work.compiles,
            &self.work.tables,
            &self.work.applications,
            &self.work.phase_updates,
        ]
        .map(|v| v.load(Ordering::Relaxed));
        ContinuationStatistics {
            source_physical_width: self.source_physical,
            target_physical_width: self.reference.width(),
            source_phase_variables: self.source.reference_phase.num_variables(),
            suffix_phase_variables: self.suffix.phase.num_variables(),
            endpoint_generators: self.transport.len(),
            source_original_positions: self.source.positions.len() - 1,
            suffix_original_positions: self.suffix.program.positions.len() - 1,
            query_original_positions: self.reference.program.positions.len() - 1,
            unique_program_owners: owners,
            unique_reference_boundaries: boundaries,
            source_phase_axes: self.phase_axes.len(),
            estimated_model_geometry_words: self.model_words,
            suffix_compilations: counts[0],
            transport_tables_prepared: counts[1],
            frame_applications: counts[2],
            numeric_phase_updates: counts[3],
            saturated: counts.contains(&u64::MAX),
        }
    }
    pub fn query_work_statistics(&self) -> QueryWorkStatistics {
        self.reference.query_work_statistics()
    }
    pub fn prepare_probability(
        &self,
        rows: &[BitVector],
        a: &ScalarArithmetic,
    ) -> Result<PreparedProbability, UnitaryError> {
        self.reference.prepare_probability(rows, a)
    }
    pub fn prepare_probability_with_fibers(
        &self,
        rows: &[BitVector],
        max_terms: usize,
        options: crate::fibers::FiberOptions,
        a: &ScalarArithmetic,
    ) -> Result<PreparedProbability, UnitaryError> {
        self.reference
            .prepare_probability_with_fibers(rows, max_terms, options, a)
    }
}
fn combine_phase(
    source: &ParityPhase,
    axes: &HashMap<BitVector, BitVector>,
    suffix: &[(BitVector, u8)],
    constant: u8,
    total: usize,
) -> Result<ParityPhase, UnitaryError> {
    let mut terms = Vec::with_capacity(source.terms().len() + suffix.len());
    for (axis, c) in source.terms() {
        terms.push((
            axes.get(axis)
                .ok_or(UnitaryError::IncompatibleProgram)?
                .clone(),
            *c,
        ));
    }
    terms.extend_from_slice(suffix);
    Ok(ParityPhase::new(
        total,
        (source.constant() + constant) & 7,
        terms,
    )?)
}
impl ContinuedUnitary {
    pub fn width(&self) -> usize {
        self.inner.width()
    }
    pub fn phase_legs(&self) -> usize {
        self.inner.phase_legs()
    }
    pub fn transported_frame(&self) -> &CompactPauliWord {
        self.inner
            .assignment_frame()
            .expect("continued view owns its frame")
    }
    pub fn evaluate_probability(
        &self,
        plan: &PreparedProbability,
        outcomes: &BitVector,
        max_terms: usize,
        a: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        plan.evaluate(&self.inner, outcomes, max_terms, a)
    }
    pub fn stabilizer_expansion(
        &self,
        limits: ExpansionLimits,
        a: &ScalarArithmetic,
    ) -> Result<OwnedStabilizerExpansion, UnitaryError> {
        self.inner.stabilizer_expansion(limits, a)
    }
}
