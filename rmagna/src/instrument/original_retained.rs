//! Retained reported-record masses after physical diagonal trace elimination.
use super::{
    OriginalError, OriginalNetworkOptions, OriginalNetworkResource, OriginalRecordParity,
    OriginalTraceView, labelled_layout,
};
use crate::gf2::{AffineSupport, BitVector, Gf2Error};
use crate::phase::PreparedPhaseResource;
use crate::scalar::{ExactScalar, ScalarArithmetic};
use crate::stabilizer::AffineKet;
use crate::unitary::{CompiledUnitary, PreparedProbabilitySupport, UnitaryError};
use std::cmp::Ordering;

/// Immutable retained-record query bound to one actual original assignment.
/// Physical outputs, environments and unretained records are traced before
/// selecting the phase resource. Labels preserve order and dependencies.
pub struct PreparedOriginalNetworkRecords<'a> {
    unitary: &'a CompiledUnitary,
    shift: BitVector,
    query: PreparedProbabilitySupport,
    max_terms: usize,
    geometry_words: u128,
}

/// Coherent terms of a classical probability tensor. Sum term amplitudes at a
/// label to obtain its mass; squaring them again would change the distribution.
/// Individual terms need not be positive and do not define a positive mixture.
pub struct OriginalRecordProbabilities {
    terms: Vec<AffineKet>,
}

/// One certified positive constant mass on an independent affine support.
/// Sampling returns its normalized support law; the total need not be one.
pub struct UniformOriginalRecords {
    support: AffineSupport,
    mass_per_label: ExactScalar,
    total_mass: ExactScalar,
}

impl OriginalTraceView<'_> {
    /// Prepare ordered reported parities without expanding the coherent side.
    /// Report inversions and physical faults belong to this immutable view.
    /// Retained rows obey `OriginalLimits::completed.max_record_rows`; event-row
    /// limits apply to scalar event queries and do not restrict these labels.
    /// Metadata, doubled paths and packed geometry are admitted before allocation.
    pub fn prepare_record_probabilities(
        &self,
        keep: &[OriginalRecordParity],
        options: &OriginalNetworkOptions<'_>,
        a: &ScalarArithmetic,
    ) -> Result<PreparedOriginalNetworkRecords<'_>, OriginalError> {
        let (layout, rows) = labelled_layout(
            self.source,
            keep,
            options,
            "max_record_rows",
            self.source.limits.completed.max_record_rows,
        )?;
        let shift = layout.outcomes(self, &BitVector::zeros(keep.len())?)?;
        let query = match options.resource {
            OriginalNetworkResource::Catalogue(resource) => {
                self.unitary.prepare_probability_support_with_resource(
                    &rows,
                    options.max_terms,
                    resource,
                    options.catalogue,
                    a,
                )?
            }
            OriginalNetworkResource::Native(resource) => {
                self.unitary.prepare_probability_support_with_native(
                    &rows,
                    options.max_terms,
                    resource,
                    options.catalogue,
                    a,
                )?
            }
        };
        Ok(PreparedOriginalNetworkRecords {
            unitary: &self.unitary,
            shift,
            query,
            max_terms: options.max_terms,
            geometry_words: layout.geometry_words,
        })
    }
}

impl PreparedOriginalNetworkRecords<'_> {
    /// Actual selected resource of the trace-reduced probability parent.
    pub fn phase_resource(&self) -> &PreparedPhaseResource {
        self.query.fixed_phase_resource()
    }
    /// Conservative packed geometry admitted before constructing this query.
    pub fn geometry_words(&self) -> u128 {
        self.geometry_words
    }
    pub fn evaluate(
        &self,
        a: &ScalarArithmetic,
    ) -> Result<OriginalRecordProbabilities, OriginalError> {
        let zero = BitVector::zeros(self.shift.width())?;
        let terms = self.query.evaluate(self.unitary, self.max_terms, a)?;
        let terms = terms
            .into_iter()
            .map(|term| term.pauli(&self.shift, &zero).map_err(OriginalError::from))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(OriginalRecordProbabilities { terms })
    }
}

impl OriginalRecordProbabilities {
    pub fn terms(&self) -> &[AffineKet] {
        &self.terms
    }
    /// Conservatively certify one actual canonical positive constant-phase term.
    /// Refusal does not prove nonuniformity. Multiple coherent terms and residual
    /// variable phase are never reinterpreted as a positive sampling mixture.
    /// No truth table or further resource decomposition is attempted.
    pub fn uniform_affine(
        &self,
        a: &ScalarArithmetic,
    ) -> Result<Option<UniformOriginalRecords>, OriginalError> {
        let [term] = self.terms.as_slice() else {
            return Ok(None);
        };
        let Some(term) = term.contract(a)? else {
            return Ok(None);
        };
        if term.phase().linear().iter().any(|&c| c != 0)
            || term.phase().upper().iter().any(|row| !row.is_zero())
        {
            return Ok(None);
        }
        let phase = a.omega(2 * i64::from(term.phase().constant()), 0)?;
        let mass_per_label = a.multiply(term.coefficient(), &phase)?;
        if !mass_per_label.is_real() || mass_per_label.real_sign()? != Ordering::Greater {
            return Ok(None);
        }
        let support = AffineSupport::new(term.offset().clone(), term.columns().to_vec())?;
        let power =
            i64::try_from(support.dimension()).map_err(|_| UnitaryError::DimensionOverflow)?;
        let count = a.scalar([1.into(), 0.into(), 0.into(), 0.into()], power)?;
        let total_mass = a.multiply(&mass_per_label, &count)?;
        Ok(Some(UniformOriginalRecords {
            support,
            mass_per_label,
            total_mass,
        }))
    }
}

impl UniformOriginalRecords {
    pub fn support(&self) -> &AffineSupport {
        &self.support
    }
    pub fn mass_per_label(&self) -> &ExactScalar {
        &self.mass_per_label
    }
    pub fn total_mass(&self) -> &ExactScalar {
        &self.total_mass
    }
    /// Consume exactly one caller-owned bit per independent support column.
    /// Independent unbiased callback bits give the uniform normalized law.
    pub fn sample_bits(
        &self,
        next_bit: &mut (impl FnMut() -> bool + ?Sized),
    ) -> Result<BitVector, Gf2Error> {
        let mut label = self.support.offset().clone();
        for column in self.support.columns() {
            if next_bit() {
                label.xor_assign(column)?;
            }
        }
        Ok(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quadratic::QuadraticForm;

    fn term(coefficient: ExactScalar, constant: u8, linear: u8) -> AffineKet {
        AffineKet::new(
            BitVector::zeros(1).unwrap(),
            vec![BitVector::from_indices(1, [0]).unwrap()],
            QuadraticForm::new(constant, vec![linear], vec![BitVector::zeros(1).unwrap()]).unwrap(),
            coefficient,
        )
        .unwrap()
    }

    #[test]
    fn certificate_refuses_coherent_sums_and_nonpositive_or_variable_phase() {
        let a = ScalarArithmetic::default();
        for terms in [
            vec![],
            vec![term(ExactScalar::one(), 0, 0); 2],
            vec![term(ExactScalar::one().negated(), 0, 0)],
            vec![term(a.omega(1, 0).unwrap(), 0, 0)],
            vec![term(ExactScalar::zero(), 0, 0)],
            vec![term(ExactScalar::one(), 0, 1)],
            vec![term(ExactScalar::one(), 0, 2)],
            vec![term(ExactScalar::one(), 0, 3)],
        ] {
            assert!(
                OriginalRecordProbabilities { terms }
                    .uniform_affine(&a)
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn certificate_preserves_mass_scale_and_folds_only_constant_phase() {
        let a = ScalarArithmetic::default();
        let quarter = a
            .scalar([1.into(), 0.into(), 0.into(), 0.into()], -2)
            .unwrap();
        let half = a
            .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
            .unwrap();
        for (coefficient, constant) in [(quarter.clone(), 0), (quarter.negated(), 2)] {
            let probability = OriginalRecordProbabilities {
                terms: vec![term(coefficient, constant, 0)],
            };
            let certificate = probability.uniform_affine(&a).unwrap().unwrap();
            assert_eq!(certificate.mass_per_label(), &quarter);
            assert_eq!(certificate.total_mass(), &half);
            assert_eq!(certificate.support().dimension(), 1);
            let mut calls = 0;
            let mut bit = || {
                calls += 1;
                calls == 2
            };
            let dynamic: &mut dyn FnMut() -> bool = &mut bit;
            let first = certificate.sample_bits(dynamic).unwrap();
            let second = certificate.sample_bits(dynamic).unwrap();
            assert_eq!(calls, 2);
            assert_ne!(first, second);
            assert_eq!(
                [first.get(0).unwrap(), second.get(0).unwrap()],
                [false, true]
            );
        }
    }

    #[test]
    fn retained_record_admission_precedes_network_factorization() {
        use crate::clifford::CliffordCircuit;
        use crate::gf2::FACTORIZATION_CALLS;
        use crate::instrument::{
            OriginalGate as G, OriginalInstrument, OriginalLimits, OriginalOperation,
            OriginalTraceAssignment, ReadoutBasis,
        };
        use crate::unitary::CompilerStrategy;
        let a = ScalarArithmetic::default();
        let mut limits = OriginalLimits::default();
        limits.completed.max_record_rows = 1;
        let instrument = OriginalInstrument::new(
            vec![0],
            vec![],
            CliffordCircuit::computational(BitVector::zeros(1).unwrap()).unwrap(),
            vec![
                OriginalOperation {
                    id: "h".into(),
                    gate: G::H(0),
                },
                OriginalOperation {
                    id: "m".into(),
                    gate: G::Destructive {
                        wire: 0,
                        basis: ReadoutBasis::Z,
                        record: "r".into(),
                    },
                },
            ],
            vec![],
            limits,
            &a,
        )
        .unwrap();
        let source = instrument
            .compile_trace(CompilerStrategy::ParityCopies, &a)
            .unwrap();
        let view = source
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        let keep = OriginalRecordParity {
            records: vec!["r".into()],
            inverted: false,
        };
        let options = OriginalNetworkOptions::default();
        FACTORIZATION_CALLS.with(|v| v.set(0));
        let good = view
            .prepare_record_probabilities(std::slice::from_ref(&keep), &options, &a)
            .unwrap();
        assert!(FACTORIZATION_CALLS.with(|v| v.get()) > 0);
        let mut occurrences = options;
        occurrences.max_record_occurrences = 0;
        let mut paths = options;
        paths.limits.max_joint_paths = 0;
        let mut geometry = options;
        geometry.limits.max_geometry_words = good.geometry_words() as usize - 1;
        for bad in [occurrences, paths, geometry] {
            FACTORIZATION_CALLS.with(|v| v.set(0));
            assert!(matches!(
                view.prepare_record_probabilities(std::slice::from_ref(&keep), &bad, &a),
                Err(OriginalError::ResourceLimit { .. })
            ));
            assert_eq!(FACTORIZATION_CALLS.with(|v| v.get()), 0);
        }
        FACTORIZATION_CALLS.with(|v| v.set(0));
        assert!(matches!(
            view.prepare_record_probabilities(&[keep.clone(), keep], &options, &a),
            Err(OriginalError::ResourceLimit {
                option: "max_record_rows",
                ..
            })
        ));
        assert_eq!(FACTORIZATION_CALLS.with(|v| v.get()), 0);
        assert_eq!(
            good.evaluate(&a)
                .unwrap()
                .uniform_affine(&a)
                .unwrap()
                .unwrap()
                .total_mass(),
            &ExactScalar::one()
        );
    }
}
