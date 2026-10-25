//! Unselected diagonal probability parents retaining computational labels.
use super::phase_map::PhaseRestriction;
use super::*;
use crate::fibers::{FiberOptions, PreparedCliffordFibers};
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
        Ok(PreparedProbabilitySupport {
            program: Arc::clone(&self.program),
            projection,
            contraction,
            boundary_map,
            labels: outputs.len(),
            empty_phase,
            phase_map,
            resource,
        })
    }
}
impl PreparedProbabilitySupport {
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
        Ok(self.resource.expand(&phase, &labelled, arithmetic)?)
    }
}
