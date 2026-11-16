//! Unselected diagonal probability parents retaining computational labels.
use super::phase_map::PhaseRestriction;
use super::*;
use crate::catalogue::{CatalogueBoundaryStatistics, PhaseCatalogue, PreparedCatalogueBoundary};
use crate::fibers::FiberOptions;
use crate::phase::{
    NativePhaseOptions, PhaseResourceOptions, PreparedPhaseResource, ResourcePreparation,
};
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
    native_transport: Option<PreparedCatalogueBoundary>,
    clifford: Option<CliffordSupport>,
}
/// Exact labelled probability terms and actual per-query native Gaussian work.
#[derive(Debug)]
pub struct ProbabilitySupportExpansion {
    pub states: Vec<AffineKet>,
    pub native_transport: Option<CatalogueBoundaryStatistics>,
}
struct CliffordSupport {
    phase: ParityPhase,
    global: u8,
    quadratic: QuadraticForm,
    contraction: PreparedAffineContraction,
}
/// Temporary geometry shared by full preparation and the resource-free probe.
struct ProbabilityGeometry {
    joint: AffineKet,
    projection: PreparedZProjection,
    contraction: PreparedAffineContraction,
    boundary_map: PreparedLinearBoundary,
    empty_phase: ParityPhase,
    phase_map: PhaseRestriction,
    phase: ParityPhase,
}
impl ProbabilityGeometry {
    fn new(
        state: &CompiledUnitary,
        outputs: &[BitVector],
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, UnitaryError> {
        if outputs.iter().any(|row| row.width() != state.width()) {
            return Err(UnitaryError::DimensionMismatch);
        }
        let reference = &state.program.query_reference;
        let half = reference.width();
        let joint = reference.tensor(&reference.conjugate(), arithmetic)?;
        let rows = (0..state.width())
            .map(|j| BitVector::from_indices(joint.width(), [j, half + j]))
            .collect::<Result<Vec<_>, _>>()?;
        let projection = PreparedZProjection::new(&joint, &rows)?;
        let work = &state.program.work;
        work.record(&work.support_projections);
        let mut keep = outputs
            .iter()
            .map(|row| BitVector::from_indices(joint.width(), row.ones()))
            .collect::<Result<Vec<_>, _>>()?;
        keep.extend(
            (state.width()..half)
                .chain(half + state.width()..joint.width())
                .map(|j| BitVector::from_indices(joint.width(), [j]))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let boundary_map = projection.prepare_linear_boundary(&keep)?;
        work.record(&work.support_boundary_maps);
        let contraction = PreparedAffineContraction::new(boundary_map.reference())?;
        work.record(&work.support_contractions);
        let magic = half - state.width();
        let empty_phase = ParityPhase::new(outputs.len(), 0, [])?;
        let phase = &state.program.query_reference_phase;
        let phase_map = PhaseRestriction::new(
            &[outputs.len(), magic, magic],
            &[(&empty_phase, false), (phase, false), (phase, true)],
            contraction.output_columns(),
        )?;
        work.record(&work.support_phase_maps);
        let phase = phase_map.apply(
            &[(&empty_phase, false), (phase, false), (phase, true)],
            &BitVector::zeros(keep.len())?,
        )?;
        Ok(Self {
            joint,
            projection,
            contraction,
            boundary_map,
            empty_phase,
            phase_map,
            phase,
        })
    }
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
        self.prepare_probability_support_policy(
            outputs,
            max_terms,
            ResourcePreparation::Catalogue(options),
            catalogue,
            arithmetic,
        )
    }
    /// Retain a labelled parent with native selection; construction errors are
    /// terminal and never select a different decomposition strategy.
    pub fn prepare_probability_support_with_native(
        &self,
        outputs: &[BitVector],
        max_terms: usize,
        options: NativePhaseOptions,
        catalogue: Option<&PhaseCatalogue>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<PreparedProbabilitySupport, UnitaryError> {
        self.prepare_probability_support_policy(
            outputs,
            max_terms,
            ResourcePreparation::Native(options),
            catalogue,
            arithmetic,
        )
    }
    /// Conservatively recognize an actual Clifford labelled probability tensor.
    /// `None` is uncertified, never proof of nonuniformity. No phase resource or
    /// catalogue is constructed. The caller admits rows and packed geometry.
    pub(crate) fn probe_probability_support_affine(
        &self,
        outputs: &[BitVector],
        max_signature_work: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Option<Vec<AffineKet>>, UnitaryError> {
        let geometry = ProbabilityGeometry::new(self, outputs, arithmetic)?;
        let joint = self
            .boundary
            .tensor(&self.boundary.conjugate(), arithmetic)?;
        let Some(projected) = geometry
            .projection
            .project(&joint, &BitVector::zeros(self.width())?)?
        else {
            return Ok(Some(vec![]));
        };
        let Some(ket) = geometry
            .contraction
            .contract(&geometry.boundary_map.apply(&projected)?, arithmetic)?
        else {
            return Ok(Some(vec![]));
        };
        let phase = geometry.phase_map.apply(
            &[
                (&geometry.empty_phase, false),
                (&self.phase, false),
                (&self.phase, true),
            ],
            ket.offset(),
        )?;
        // Deliberately overcharge the full coordinate space, including even
        // terms. This bounds signature scans and the quadratic conversion;
        // compressed odd-axis scans may perform less work, never more.
        let n = phase.num_variables() as u128;
        let per_pair = n
            .saturating_mul(2)
            .saturating_add(phase.terms().len() as u128)
            .saturating_add(3);
        let required = n
            .saturating_mul(n.saturating_add(1))
            .saturating_div(2)
            .saturating_mul(per_pair)
            .saturating_mul(n.div_ceil(64));
        if required > max_signature_work as u128 {
            return Err(crate::phase::PhaseError::ResourceLimit {
                option: "affine_probe.max_signature_work",
                limit: max_signature_work as u128,
                required,
            }
            .into());
        }
        let (global, quadratic) = match phase.clifford() {
            Ok(value) => value,
            Err(crate::phase::PhaseError::NotClifford) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let labelled = ket.retain_boundary(&(0..outputs.len()).collect::<Vec<_>>())?;
        Ok(Some(
            clifford_probability_ket(labelled, global, &quadratic, arithmetic)?
                .contract(arithmetic)?
                .into_iter()
                .collect(),
        ))
    }
    fn prepare_probability_support_policy(
        &self,
        outputs: &[BitVector],
        max_terms: usize,
        options: ResourcePreparation,
        catalogue: Option<&PhaseCatalogue>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<PreparedProbabilitySupport, UnitaryError> {
        let ProbabilityGeometry {
            joint,
            projection,
            contraction,
            boundary_map,
            empty_phase,
            phase_map,
            phase,
        } = ProbabilityGeometry::new(self, outputs, arithmetic)?;
        let work = &self.program.work;
        let mut resource = options.prepare(&phase, max_terms, catalogue, arithmetic)?;
        work.record(&work.support_phase_resources);
        let transport_requested = resource.native_transport_requested();
        let native_transport = resource.prepare_native_transport(
            &phase,
            outputs.len(),
            || Ok(contraction.symbolic_boundary(outputs.len())?),
            arithmetic,
        )?;
        let mut prepared = PreparedProbabilitySupport {
            program: Arc::clone(&self.program),
            projection,
            contraction,
            boundary_map,
            labels: outputs.len(),
            empty_phase,
            phase_map,
            resource,
            native_transport,
            clifford: None,
        };
        // The explicit prepared policy creates a symbolic reference only.
        // Off and non-native modes retain their existing Clifford convention.
        if !transport_requested {
            prepared.prepare_clifford(&joint, self.width(), arithmetic)?;
        }
        work.record(&work.support_parents);
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
        self.program
            .work
            .record(&self.program.work.support_contractions);
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
        Ok(self
            .evaluate_with_statistics(state, max_terms, arithmetic)?
            .states)
    }
    pub fn evaluate_with_statistics(
        &self,
        state: &CompiledUnitary,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ProbabilitySupportExpansion, UnitaryError> {
        if !Arc::ptr_eq(&self.program, &state.program) {
            return Err(UnitaryError::IncompatibleProgram);
        }
        self.resource.check_term_limit(max_terms)?;
        self.program
            .work
            .record(&self.program.work.support_evaluations);
        let joint = state
            .boundary
            .tensor(&state.boundary.conjugate(), arithmetic)?;
        let Some(ket) = self
            .projection
            .project(&joint, &BitVector::zeros(state.width())?)?
        else {
            return Ok(ProbabilitySupportExpansion {
                states: vec![],
                native_transport: None,
            });
        };
        let Some(ket) = self
            .contraction
            .contract(&self.boundary_map.apply(&ket)?, arithmetic)?
        else {
            return Ok(ProbabilitySupportExpansion {
                states: vec![],
                native_transport: None,
            });
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
        if let Some(plan) = &self.native_transport {
            let expansion = plan.expand(&phase, &labelled, arithmetic)?;
            return Ok(ProbabilitySupportExpansion {
                states: expansion.states,
                native_transport: Some(expansion.statistics),
            });
        }
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
                Err(StabilizerError::IncompatibleContraction) => {
                    self.program
                        .work
                        .record(&self.program.work.support_changed_boundary_fallbacks);
                    labelled.contract(arithmetic)?
                }
                Err(error) => return Err(error.into()),
            };
            return Ok(ProbabilitySupportExpansion {
                states: contracted.into_iter().collect(),
                native_transport: None,
            });
        }
        Ok(ProbabilitySupportExpansion {
            states: self.resource.expand(&phase, &labelled, arithmetic)?,
            native_transport: None,
        })
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
