//! Unselected diagonal probability parents retaining computational labels.
use super::phase_map::PhaseRestriction;
use super::*;
use crate::catalogue::PhaseCatalogue;
use crate::fibers::FiberOptions;
use crate::phase::{PhaseResourceOptions, PreparedPhaseResource};
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
    resource: PreparedPhaseResource,
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
        self.prepare_probability_support_with_resource(
            outputs,
            max_terms,
            PhaseResourceOptions {
                fibers: options,
                ..Default::default()
            },
            None,
            arithmetic,
        )
    }
    /// Prepare a labelled probability parent with an explicit catalogue/fiber
    /// choice. The plan owns the selected resource after the catalogue is dropped.
    pub fn prepare_probability_support_with_resource(
        &self,
        outputs: &[BitVector],
        max_terms: usize,
        options: PhaseResourceOptions,
        catalogue: Option<&PhaseCatalogue>,
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
        let resource =
            PreparedPhaseResource::new(&phase, max_terms, options, catalogue, arithmetic)?;
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
        // Preserve the existing fiber expansion's coordinate order; a different
        // domain keeps the general path and its seeded support convention.
        // A catalogue with quotient rank zero represents the complete Clifford
        // function on these original coordinates, independently of its indicator
        // expansion. The existing fiber convention still requires identity order.
        let identity_domain = self.resource.quotient_rank() == 0
            && self.resource.clifford_fibers().is_none_or(|fibers| {
                fibers.clifford_directions().len() == variables
                    && fibers
                        .clifford_directions()
                        .iter()
                        .enumerate()
                        .all(|(j, v)| v.width() == variables && v.ones().eq(std::iter::once(j)))
            });
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

    pub fn fixed_phase_resource(&self) -> &PreparedPhaseResource {
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
        self.resource.check_term_limit(max_terms)?;
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
            let contracted = match prepared
                .contraction
                .contract_with_phase_updates(&labelled, arithmetic)
            {
                Ok(value) => value,
                // A changed boundary map also invalidates the retained basis.
                // Keep the general exact route without growing a cache.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogue::builtin_phase_catalogue;
    use crate::clifford::CliffordGate;
    use crate::pauli::PauliWord;
    #[test]
    fn catalogue_rank_zero_retains_the_clifford_shortcut_and_literal_faulted_masses() {
        let arithmetic = ScalarArithmetic::default();
        let row = |mask: usize| {
            BitVector::from_indices(3, (0..3).filter(|j| mask >> j & 1 != 0)).unwrap()
        };
        let circuit = UnitaryCircuit::new(
            3,
            [
                UnitaryGate::Clifford(CliffordGate::H(0)),
                UnitaryGate::Clifford(CliffordGate::Cx {
                    control: 0,
                    target: 1,
                }),
                UnitaryGate::Clifford(CliffordGate::H(2)),
            ],
        )
        .unwrap();
        for strategy in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            let state = circuit
                .compile_with_strategy(&row(0), &arithmetic, strategy)
                .unwrap();
            let plan = state
                .prepare_probability_support_with_resource(
                    &[row(1), row(3), row(4)],
                    1,
                    PhaseResourceOptions::default(),
                    Some(builtin_phase_catalogue().unwrap()),
                    &arithmetic,
                )
                .unwrap();
            assert!(
                plan.clifford.is_some(),
                "rank-zero catalogue lost its retained shortcut"
            );
            let changed = state
                .with_insertions(
                    &[PauliInsertion {
                        before_gate: 3,
                        word: PauliWord::new(row(2), row(0), 0).unwrap(),
                    }],
                    &arithmetic,
                )
                .unwrap();
            let terms = plan.evaluate(&changed, 1, &arithmetic).unwrap();
            assert_eq!(terms.len(), 1);
            let ket = terms[0].prepare().unwrap();
            for label in 0..8 {
                let expected = if label & 2 != 0 {
                    arithmetic.omega(0, -4).unwrap()
                } else {
                    ExactScalar::zero()
                };
                assert_eq!(ket.amplitude(&row(label), &arithmetic).unwrap(), expected);
            }
        }
    }
}
