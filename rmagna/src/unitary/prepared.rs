//! Fixed scalar closure for probability, Pauli moments and density overlaps.
use super::phase_map::PhaseRestriction;
use super::*;
use crate::catalogue::{PhaseCatalogue, PreparedCatalogueBoundary};
use crate::phase::{
    NativePhaseOptions, PhaseResourceOptions, PreparedPhaseResource, ResourcePreparation,
};
use crate::stabilizer::PreparedAffineContraction;

struct FixedResource {
    decomposition: PreparedPhaseResource,
    phase_map: PhaseRestriction,
    transport: Option<PreparedCatalogueBoundary>,
}

struct ScalarClosure {
    projection: PreparedZProjection,
    contraction: PreparedAffineContraction,
    keep: Vec<usize>,
    widths: Vec<usize>,
    resource: Option<FixedResource>,
}
impl ScalarClosure {
    fn new(
        reference: &AffineKet,
        rows: &[BitVector],
        legs: Vec<Vec<usize>>,
    ) -> Result<Self, UnitaryError> {
        let projection = PreparedZProjection::new(reference, rows)?;
        let keep = legs.iter().flatten().copied().collect::<Vec<_>>();
        let contraction = projection.prepare_sum(&keep)?;
        Ok(Self {
            projection,
            contraction,
            keep,
            widths: legs.iter().map(Vec::len).collect(),
            resource: None,
        })
    }
    fn freeze(
        &mut self,
        phases: &[(&ParityPhase, bool)],
        max_terms: usize,
        options: ResourcePreparation,
        catalogue: Option<&PhaseCatalogue>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<(), UnitaryError> {
        let phase_map =
            PhaseRestriction::new(&self.widths, phases, self.contraction.output_columns())?;
        let phase = phase_map.apply(phases, &BitVector::zeros(self.keep.len())?)?;
        let mut decomposition = options.prepare(&phase, max_terms, catalogue, arithmetic)?;
        let transport = decomposition.prepare_native_transport(
            &phase,
            0,
            || Ok(self.contraction.symbolic_boundary(0)?),
            arithmetic,
        )?;
        self.resource = Some(FixedResource {
            decomposition,
            phase_map,
            transport,
        });
        Ok(())
    }
    fn evaluate(
        &self,
        joint: &AffineKet,
        outcomes: &BitVector,
        phases: &[(&ParityPhase, bool)],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        let retained = self
            .projection
            .project(joint, outcomes)?
            .map(|ket| ket.retain_boundary(&self.keep))
            .transpose()?;
        let contracted = match retained {
            Some(ket) => self.contraction.contract(&ket, arithmetic)?,
            None => None,
        };
        let Some(resource) = &self.resource else {
            return close_retained(contracted, &self.widths, phases, max_terms, arithmetic);
        };
        let Some(ket) = contracted else {
            return Ok(zero_network_value());
        };
        resource.decomposition.check_term_limit(max_terms)?;
        debug_assert_eq!(ket.columns(), self.contraction.output_columns());
        let phase = resource.phase_map.apply(phases, ket.offset())?;
        let (value, native_transport) = match &resource.transport {
            Some(plan) => {
                let scalar = AffineKet::new(
                    BitVector::zeros(0)?,
                    vec![BitVector::zeros(0)?; ket.phase().num_variables()],
                    ket.phase().clone(),
                    ket.coefficient().clone(),
                )?;
                let expanded = plan.expand(&phase, &scalar, arithmetic)?;
                let mut value = ExactScalar::zero();
                for term in expanded.states {
                    debug_assert_eq!(term.width(), 0);
                    debug_assert!(term.columns().is_empty());
                    let global = arithmetic.omega(2 * i64::from(term.phase().constant()), 0)?;
                    let weighted = arithmetic.multiply(term.coefficient(), &global)?;
                    value = arithmetic.add(&value, &weighted)?;
                }
                (value, Some(expanded.statistics))
            }
            None => (
                resource.decomposition.evaluate(
                    &phase,
                    ket.phase(),
                    ket.coefficient(),
                    arithmetic,
                )?,
                None,
            ),
        };
        Ok(NetworkValue {
            value,
            quotient_rank: resource.decomposition.quotient_rank(),
            terms: resource.decomposition.num_terms(),
            native_transport,
        })
    }
}

/// Fixed trace/event constraints and retained Gaussian contraction for one
/// program's probability family. Exact phase-resource preparation is optional.
pub struct PreparedProbability {
    program: Arc<InsertionProgram>,
    physical_width: usize,
    event_count: usize,
    closure: ScalarClosure,
}
impl PreparedProbability {
    /// Freeze bounded native selection against the original probability family.
    pub fn with_native_phase_resource(
        mut self,
        max_terms: usize,
        options: NativePhaseOptions,
        catalogue: Option<&PhaseCatalogue>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, UnitaryError> {
        let phase = &self.program.query_reference_phase;
        self.closure.freeze(
            &[(phase, false), (phase, true)],
            max_terms,
            ResourcePreparation::Native(options),
            catalogue,
            arithmetic,
        )?;
        Ok(self)
    }
    pub(super) fn prepare(
        state: &CompiledUnitary,
        rows: &[BitVector],
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, UnitaryError> {
        if rows.iter().any(|r| r.width() != state.width()) {
            return Err(UnitaryError::DimensionMismatch);
        }
        state
            .width()
            .checked_add(rows.len())
            .ok_or(UnitaryError::DimensionOverflow)?;
        let reference = &state.program.query_reference;
        let half = reference.width();
        let joint = reference.tensor(&reference.conjugate(), arithmetic)?;
        let total = joint.width();
        let mut constraints = (0..state.width())
            .map(|j| BitVector::from_indices(total, [j, half + j]))
            .collect::<Result<Vec<_>, _>>()?;
        constraints.extend(
            rows.iter()
                .map(|r| BitVector::from_indices(total, r.ones()))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let legs = vec![
            (state.width()..half).collect(),
            (half + state.width()..total).collect(),
        ];
        Ok(Self {
            program: Arc::clone(&state.program),
            physical_width: state.width(),
            event_count: rows.len(),
            closure: ScalarClosure::new(&joint, &constraints, legs)?,
        })
    }
    /// Freeze an explicit phase resource against this family's original reference.
    /// This consumes the family. Missing catalogue keys use fibers; failed
    /// construction is returned directly without silently changing strategies.
    pub fn with_phase_resource(
        mut self,
        max_terms: usize,
        options: PhaseResourceOptions,
        catalogue: Option<&PhaseCatalogue>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, UnitaryError> {
        let phase = &self.program.query_reference_phase;
        self.closure.freeze(
            &[(phase, false), (phase, true)],
            max_terms,
            ResourcePreparation::Catalogue(options),
            catalogue,
            arithmetic,
        )?;
        Ok(self)
    }
    /// Fixed counts precede adaptive radical absorption and can be larger.
    pub fn fixed_phase_resource(&self) -> Option<&PreparedPhaseResource> {
        self.closure.resource.as_ref().map(|r| &r.decomposition)
    }
    pub fn evaluate(
        &self,
        state: &CompiledUnitary,
        outcomes: &BitVector,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        if !Arc::ptr_eq(&self.program, &state.program) {
            return Err(UnitaryError::IncompatibleProgram);
        }
        if outcomes.width() != self.event_count {
            return Err(UnitaryError::DimensionMismatch);
        }
        let rhs = BitVector::from_indices(
            self.physical_width + self.event_count,
            outcomes.ones().map(|j| self.physical_width + j),
        )?;
        let joint = state
            .boundary
            .tensor(&state.boundary.conjugate(), arithmetic)?;
        self.closure.evaluate(
            &joint,
            &rhs,
            &[(&state.phase, false), (&state.phase, true)],
            max_terms,
            arithmetic,
        )
    }
}

/// Immutable exact `<psi|Pi P Pi|psi>` family for one reference program, Pauli
/// word and Z-event layout, retaining global complex phase. Outcome changes and
/// Pauli insertions reuse its Gaussian constraints and optional phase resource.
pub struct PreparedPauliMoment {
    program: Arc<InsertionProgram>,
    word: PauliWord,
    physical_width: usize,
    event_count: usize,
    closure: Option<ScalarClosure>,
}
fn moment_joint(
    boundary: &AffineKet,
    word: &PauliWord,
    arithmetic: &ScalarArithmetic,
) -> Result<AffineKet, UnitaryError> {
    Ok(boundary
        .conjugate()
        .tensor(&word.apply(boundary, arithmetic)?, arithmetic)?)
}
impl PreparedPauliMoment {
    /// Certified zero moments retain no resource; other families freeze once.
    pub fn with_native_phase_resource(
        mut self,
        max_terms: usize,
        options: NativePhaseOptions,
        catalogue: Option<&PhaseCatalogue>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, UnitaryError> {
        if let Some(closure) = &mut self.closure {
            let phase = &self.program.query_reference_phase;
            closure.freeze(
                &[(phase, true), (phase, false)],
                max_terms,
                ResourcePreparation::Native(options),
                catalogue,
                arithmetic,
            )?;
        }
        Ok(self)
    }
    pub(super) fn prepare(
        state: &CompiledUnitary,
        word: &PauliWord,
        rows: &[BitVector],
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, UnitaryError> {
        if word.width() != state.width() || rows.iter().any(|r| r.width() != state.width()) {
            return Err(UnitaryError::DimensionMismatch);
        }
        state
            .width()
            .checked_add(rows.len())
            .ok_or(UnitaryError::DimensionOverflow)?;
        let mut zero = false;
        for row in rows {
            zero |= row.dot(word.x())?;
        }
        let reference = &state.program.query_reference;
        let half = reference.width();
        let lifted = PauliWord::new(
            BitVector::from_indices(half, word.x().ones())?,
            BitVector::from_indices(half, word.z().ones())?,
            word.phase(),
        )?;
        let closure = if zero {
            // If P anticommutes with a Z-row observable, P exchanges its eigenspaces,
            // so the corresponding event projector satisfies Pi P Pi = 0.
            None
        } else {
            let joint = moment_joint(reference, &lifted, arithmetic)?;
            let total = joint.width();
            let mut constraints = (0..state.width())
                .map(|q| BitVector::from_indices(total, [q, half + q]))
                .collect::<Result<Vec<_>, _>>()?;
            constraints.extend(
                rows.iter()
                    .map(|r| BitVector::from_indices(total, r.ones()))
                    .collect::<Result<Vec<_>, _>>()?,
            );
            // P commutes with every event. Equality of the physical bra/ket
            // labels therefore enforces both projectors using only bra rows.
            let legs = vec![
                (state.width()..half).collect(),
                (half + state.width()..total).collect(),
            ];
            Some(ScalarClosure::new(&joint, &constraints, legs)?)
        };
        Ok(Self {
            program: Arc::clone(&state.program),
            word: lifted,
            physical_width: state.width(),
            event_count: rows.len(),
            closure,
        })
    }
    /// Freeze an explicit phase resource against this family's original reference.
    /// This consumes the family. Missing catalogue keys use fibers; failed
    /// construction is returned directly without silently changing strategies.
    pub fn with_phase_resource(
        mut self,
        max_terms: usize,
        options: PhaseResourceOptions,
        catalogue: Option<&PhaseCatalogue>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, UnitaryError> {
        if let Some(closure) = &mut self.closure {
            let phase = &self.program.query_reference_phase;
            closure.freeze(
                &[(phase, true), (phase, false)],
                max_terms,
                ResourcePreparation::Catalogue(options),
                catalogue,
                arithmetic,
            )?;
        }
        Ok(self)
    }
    /// A fixed Pauli/event anticommutation certificate, independent of outcomes.
    pub fn has_zero_certificate(&self) -> bool {
        self.closure.is_none()
    }
    /// Fixed counts precede adaptive radical absorption and can be larger.
    /// A certified zero response needs no phase resource, even in fixed mode.
    pub fn fixed_phase_resource(&self) -> Option<&PreparedPhaseResource> {
        self.closure
            .as_ref()
            .and_then(|c| c.resource.as_ref())
            .map(|r| &r.decomposition)
    }
    pub fn evaluate(
        &self,
        state: &CompiledUnitary,
        outcomes: &BitVector,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        if !Arc::ptr_eq(&self.program, &state.program) {
            return Err(UnitaryError::IncompatibleProgram);
        }
        if outcomes.width() != self.event_count {
            return Err(UnitaryError::DimensionMismatch);
        }
        let Some(closure) = &self.closure else {
            return Ok(zero_network_value());
        };
        let rhs = BitVector::from_indices(
            self.physical_width + self.event_count,
            outcomes.ones().map(|j| self.physical_width + j),
        )?;
        let joint = moment_joint(&state.boundary, &self.word, arithmetic)?;
        closure.evaluate(
            &joint,
            &rhs,
            &[(&state.phase, true), (&state.phase, false)],
            max_terms,
            arithmetic,
        )
    }
}

/// Ordered reduced-density outputs and fixed Z-event rows; outcomes are supplied
/// separately when evaluating a prepared family. Empty outputs trace all wires.
#[derive(Clone, Copy)]
pub struct DensityLayout<'a> {
    pub outputs: &'a [usize],
    pub event_rows: &'a [BitVector],
}
impl DensityLayout<'_> {
    pub(crate) fn validate(&self, width: usize) -> Result<(), UnitaryError> {
        if self.event_rows.iter().any(|r| r.width() != width) {
            return Err(UnitaryError::DimensionMismatch);
        }
        if self
            .outputs
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != self.outputs.len()
        {
            return Err(UnitaryError::RepeatedOutput);
        }
        BitVector::from_indices(width, self.outputs.iter().copied())?;
        Ok(())
    }
}

/// Immutable four-copy density-overlap family. Both programs may receive new
/// Pauli assignments and event outcomes; ordered layouts and program identities
/// remain fixed. Preparation always uses the original references.
pub struct PreparedDensityOverlap {
    left: Arc<InsertionProgram>,
    right: Arc<InsertionProgram>,
    left_events: usize,
    right_events: usize,
    trace_count: usize,
    closure: ScalarClosure,
}
impl PreparedDensityOverlap {
    /// Freeze bounded native selection for the original four-copy family.
    pub fn with_native_phase_resource(
        mut self,
        max_terms: usize,
        options: NativePhaseOptions,
        catalogue: Option<&PhaseCatalogue>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, UnitaryError> {
        let l = &self.left.query_reference_phase;
        let r = &self.right.query_reference_phase;
        self.closure.freeze(
            &[(l, false), (l, true), (r, true), (r, false)],
            max_terms,
            ResourcePreparation::Native(options),
            catalogue,
            arithmetic,
        )?;
        Ok(self)
    }
    pub(super) fn prepare(
        left: &CompiledUnitary,
        boundary: DensityLayout<'_>,
        right: &CompiledUnitary,
        target_boundary: DensityLayout<'_>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, UnitaryError> {
        let l = &left.program.query_reference;
        let r = &right.program.query_reference;
        let DensityNetwork {
            rows,
            trace_count,
            legs,
        } = density_rows(
            left.width(),
            l.width(),
            boundary,
            right.width(),
            r.width(),
            target_boundary,
        )?;
        let joint = density_joint(l, r, arithmetic)?;
        Ok(Self {
            left: Arc::clone(&left.program),
            right: Arc::clone(&right.program),
            left_events: boundary.event_rows.len(),
            right_events: target_boundary.event_rows.len(),
            trace_count,
            closure: ScalarClosure::new(&joint, &rows, legs)?,
        })
    }
    /// Freeze an explicit phase resource against this family's original reference.
    /// This consumes the family. Missing catalogue keys use fibers; failed
    /// construction is returned directly without silently changing strategies.
    pub fn with_phase_resource(
        mut self,
        max_terms: usize,
        options: PhaseResourceOptions,
        catalogue: Option<&PhaseCatalogue>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, UnitaryError> {
        let l = &self.left.query_reference_phase;
        let r = &self.right.query_reference_phase;
        self.closure.freeze(
            &[(l, false), (l, true), (r, true), (r, false)],
            max_terms,
            ResourcePreparation::Catalogue(options),
            catalogue,
            arithmetic,
        )?;
        Ok(self)
    }
    pub(crate) fn matches_programs(&self, left: &CompiledUnitary, right: &CompiledUnitary) -> bool {
        Arc::ptr_eq(&self.left, &left.program) && Arc::ptr_eq(&self.right, &right.program)
    }
    pub fn fixed_phase_resource(&self) -> Option<&PreparedPhaseResource> {
        self.closure.resource.as_ref().map(|r| &r.decomposition)
    }
    pub fn evaluate(
        &self,
        left: &CompiledUnitary,
        right: &CompiledUnitary,
        left_outcomes: &BitVector,
        right_outcomes: &BitVector,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        if !self.matches_programs(left, right) {
            return Err(UnitaryError::IncompatibleProgram);
        }
        if left_outcomes.width() != self.left_events || right_outcomes.width() != self.right_events
        {
            return Err(UnitaryError::DimensionMismatch);
        }
        let joint = density_joint(&left.boundary, &right.boundary, arithmetic)?;
        let rhs = density_rhs(self.trace_count, left_outcomes, right_outcomes)?;
        self.closure.evaluate(
            &joint,
            &rhs,
            &[
                (&left.phase, false),
                (&left.phase, true),
                (&right.phase, true),
                (&right.phase, false),
            ],
            max_terms,
            arithmetic,
        )
    }
}

pub(super) fn density_joint(
    left: &AffineKet,
    right: &AffineKet,
    arithmetic: &ScalarArithmetic,
) -> Result<AffineKet, UnitaryError> {
    Ok(left
        .tensor(&left.conjugate(), arithmetic)?
        .tensor(&right.conjugate(), arithmetic)?
        .tensor(right, arithmetic)?)
}
pub(super) fn density_rhs(
    trace: usize,
    left: &BitVector,
    right: &BitVector,
) -> Result<BitVector, UnitaryError> {
    let count = left
        .width()
        .checked_add(right.width())
        .and_then(|n| n.checked_mul(2))
        .and_then(|n| trace.checked_add(n))
        .ok_or(UnitaryError::DimensionOverflow)?;
    let mut rhs = BitVector::zeros(count)?;
    let mut offset = trace;
    for values in [left, right] {
        for i in values.ones() {
            rhs.set(offset + 2 * i, true)?;
            rhs.set(offset + 2 * i + 1, true)?;
        }
        offset += 2 * values.width();
    }
    Ok(rhs)
}
pub(super) struct DensityNetwork {
    pub(super) rows: Vec<BitVector>,
    pub(super) trace_count: usize,
    pub(super) legs: Vec<Vec<usize>>,
}
pub(super) fn density_rows(
    lw: usize,
    n: usize,
    left: DensityLayout<'_>,
    rw: usize,
    m: usize,
    right: DensityLayout<'_>,
) -> Result<DensityNetwork, UnitaryError> {
    if n < lw || m < rw {
        return Err(UnitaryError::DimensionMismatch);
    }
    left.validate(lw)?;
    right.validate(rw)?;
    if left.outputs.len() != right.outputs.len() {
        return Err(UnitaryError::DimensionMismatch);
    }
    let second = n.checked_mul(2).ok_or(UnitaryError::DimensionOverflow)?;
    let total = m
        .checked_mul(2)
        .and_then(|v| second.checked_add(v))
        .ok_or(UnitaryError::DimensionOverflow)?;
    let positions = left
        .outputs
        .iter()
        .enumerate()
        .map(|(j, &q)| (q, right.outputs[j]))
        .collect::<BTreeMap<_, _>>();
    let mut rows = Vec::new();
    for q in 0..lw {
        if let Some(&r) = positions.get(&q) {
            rows.push(BitVector::from_indices(total, [q, second + r])?);
            rows.push(BitVector::from_indices(total, [n + q, second + m + r])?);
        } else {
            rows.push(BitVector::from_indices(total, [q, n + q])?);
        }
    }
    let right_kept = right
        .outputs
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    for q in 0..rw {
        if !right_kept.contains(&q) {
            rows.push(BitVector::from_indices(
                total,
                [second + q, second + m + q],
            )?);
        }
    }
    let trace_count = rows.len();
    for (layout, offset, half) in [(left, 0, n), (right, second, m)] {
        for row in layout.event_rows {
            rows.push(BitVector::from_indices(
                total,
                row.ones().map(|j| offset + j),
            )?);
            rows.push(BitVector::from_indices(
                total,
                row.ones().map(|j| offset + half + j),
            )?);
        }
    }
    let legs = vec![
        (lw..n).collect(),
        (n + lw..second).collect(),
        (second + rw..second + m).collect(),
        (second + m + rw..total).collect(),
    ];
    Ok(DensityNetwork {
        rows,
        trace_count,
        legs,
    })
}
