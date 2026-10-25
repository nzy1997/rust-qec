//! Unselected diagonal probability parents retaining computational labels.
use super::phase_map::PhaseRestriction;
use super::*;
use crate::fibers::{FiberOptions, PreparedCliffordFibers};
use crate::quadratic::QuadraticForm;
use crate::stabilizer::{PreparedAffineContraction, PreparedLinearBoundary};

/// Immutable probability parent for ordered computational output labels.
/// Its exact Clifford terms certify an affine envelope, without claiming that
/// residual interference zeros are absent. Projection/acceptance is separate.
pub struct PreparedProbabilitySupport {
    program: Arc<InsertionProgram>,
    projection: PreparedZProjection,
    contraction: PreparedAffineContraction,
    boundary_map: PreparedLinearBoundary,
    labels: usize,
    empty_phase: ParityPhase,
    phase_map: PhaseRestriction,
    resource: PreparedCliffordFibers,
    clifford: Option<CliffordSupport>,
}
struct CliffordSupport {
    phase: ParityPhase,
    global: u8,
    quadratic: QuadraticForm,
    contraction: PreparedAffineContraction,
}
impl CompiledUnitary {
    /// Retain ordered computational parity rows in the unselected diagonal
    /// probability tensor. No postselection predicate is introduced.
    pub fn prepare_probability_support(
        &self,
        outputs: &[BitVector],
        max_terms: usize,
        options: FiberOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<PreparedProbabilitySupport, UnitaryError> {
        if outputs.iter().any(|row| row.width() != self.width()) {
            return Err(UnitaryError::DimensionMismatch);
        }
        let reference = &self.program.query_reference;
        let half = reference.width();
        let joint = reference.tensor(&reference.conjugate(), arithmetic)?;
        let rows = (0..self.width())
            .map(|j| BitVector::from_indices(joint.width(), [j, half + j]))
            .collect::<Result<Vec<_>, _>>()?;
        let projection = PreparedZProjection::new(&joint, &rows)?;
        let mut keep = outputs
            .iter()
            .map(|row| BitVector::from_indices(joint.width(), row.ones()))
            .collect::<Result<Vec<_>, _>>()?;
        keep.extend(
            (self.width()..half)
                .chain(half + self.width()..joint.width())
                .map(|j| BitVector::from_indices(joint.width(), [j]))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let boundary_map = projection.prepare_linear_boundary(&keep)?;
        let contraction = PreparedAffineContraction::new(boundary_map.reference())?;
        let magic = half - self.width();
        let empty_phase = ParityPhase::new(outputs.len(), 0, [])?;
        let phase = &self.program.query_reference_phase;
        let phase_map = PhaseRestriction::new(
            &[outputs.len(), magic, magic],
            &[(&empty_phase, false), (phase, false), (phase, true)],
            contraction.output_columns(),
        )?;
        let phase = phase_map.apply(
            &[(&empty_phase, false), (phase, false), (phase, true)],
            &BitVector::zeros(keep.len())?,
        )?;
        let resource = PreparedCliffordFibers::new(&phase, max_terms, options)?;
        let mut prepared = PreparedProbabilitySupport {
            program: Arc::clone(&self.program),
            projection,
            contraction,
            boundary_map,
            labels: outputs.len(),
            empty_phase,
            phase_map,
            resource,
            clifford: None,
        };
        prepared.prepare_clifford(&joint, self.width(), arithmetic)?;
        Ok(prepared)
    }
}
impl PreparedProbabilitySupport {
    fn prepare_clifford(
        &mut self,
        joint: &AffineKet,
        physical_width: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<(), UnitaryError> {
        let variables = self.contraction.output_columns().len();
        // A rank-zero resource has one Clifford term. Preserve the general
        // expansion's coordinate order explicitly; a different domain keeps
        // the general path, including its seeded support-column convention.
        let identity_domain = self.resource.quotient_rank() == 0
            && self.resource.clifford_directions().len() == variables
            && self
                .resource
                .clifford_directions()
                .iter()
                .enumerate()
                .all(|(j, v)| v.width() == variables && v.ones().eq(std::iter::once(j)));
        if !identity_domain {
            return Ok(());
        }
        let Some(projected) = self
            .projection
            .project(joint, &BitVector::zeros(physical_width)?)?
        else {
            return Ok(());
        };
        let Some(reference) = self
            .contraction
            .contract(&self.boundary_map.apply(&projected)?, arithmetic)?
        else {
            return Ok(());
        };
        let phase = self.phase_map.apply(
            &[
                (&self.empty_phase, false),
                (&self.program.query_reference_phase, false),
                (&self.program.query_reference_phase, true),
            ],
            reference.offset(),
        )?;
        let labelled = reference.retain_boundary(&(0..self.labels).collect::<Vec<_>>())?;
        let (global, quadratic) = phase.clifford()?;
        let contraction = PreparedAffineContraction::new(&clifford_probability_ket(
            labelled, global, &quadratic, arithmetic,
        )?)?;
        self.clifford = Some(CliffordSupport {
            phase,
            global,
            quadratic,
            contraction,
        });
        Ok(())
    }

    pub fn fixed_phase_resource(&self) -> &PreparedCliffordFibers {
        &self.resource
    }
    /// Exact diagonal probability terms, including coefficients and phases.
    /// Summing their amplitudes at a word gives that word's physical mass.
    pub fn evaluate(
        &self,
        state: &CompiledUnitary,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Vec<AffineKet>, UnitaryError> {
        if !Arc::ptr_eq(&self.program, &state.program) {
            return Err(UnitaryError::IncompatibleProgram);
        }
        if self.resource.num_terms() > max_terms {
            return Err(PhaseError::FiberTermLimit {
                coordinates: self.resource.enumerated_coordinates(),
                quotient_rank: self.resource.quotient_rank(),
                limit: max_terms,
            }
            .into());
        }
        let joint = state
            .boundary
            .tensor(&state.boundary.conjugate(), arithmetic)?;
        let Some(ket) = self
            .projection
            .project(&joint, &BitVector::zeros(state.width())?)?
        else {
            return Ok(vec![]);
        };
        let Some(ket) = self
            .contraction
            .contract(&self.boundary_map.apply(&ket)?, arithmetic)?
        else {
            return Ok(vec![]);
        };
        let phase = self.phase_map.apply(
            &[
                (&self.empty_phase, false),
                (&state.phase, false),
                (&state.phase, true),
            ],
            ket.offset(),
        )?;
        let labelled = ket.retain_boundary(&(0..self.labels).collect::<Vec<_>>())?;
        if let Some(prepared) = &self.clifford {
            // Certify only the changing correction. Common odd-axis terms
            // cancel before Clifford conversion; recertifying the complete
            // reference on each group can dominate the sampling workload.
            let (global, correction) = phase.subtract(&prepared.phase)?.clifford()?;
            let quadratic = crate::phase::add_quadratic(&prepared.quadratic, &correction)?;
            let labelled = clifford_probability_ket(
                labelled,
                (prepared.global + global) & 7,
                &quadratic,
                arithmetic,
            )?;
            let contracted = match prepared.contraction.contract(&labelled, arithmetic) {
                Ok(value) => value,
                // Clifford updates can change cross terms or odd linear
                // parity. Solve that exact geometry without growing a cache.
                Err(StabilizerError::IncompatibleContraction) => labelled.contract(arithmetic)?,
                Err(error) => return Err(error.into()),
            };
            return Ok(contracted.into_iter().collect());
        }
        Ok(self.resource.expand(&phase, &labelled, arithmetic)?)
    }
}

fn clifford_probability_ket(
    labelled: AffineKet,
    global: u8,
    phase: &QuadraticForm,
    arithmetic: &ScalarArithmetic,
) -> Result<AffineKet, UnitaryError> {
    let (offset, columns, boundary, coefficient) = labelled.into_parts();
    let quadratic = crate::phase::add_quadratic(&boundary, phase)?;
    let coefficient =
        arithmetic.multiply(&coefficient, &arithmetic.omega(i64::from(global), 0)?)?;
    Ok(AffineKet::new(offset, columns, quadratic, coefficient)?)
}
