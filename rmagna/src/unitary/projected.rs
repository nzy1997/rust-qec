//! Record projection while the complete compiler phase boundary is retained.
use super::expansion::{admit, geometry_words};
use super::{CompiledUnitary, ExpansionLimits, UnitaryError};
use crate::gf2::BitVector;
use crate::phase::{ParityPhase, PathTensorError, PhasePathTensor};
use crate::quadratic::QuadraticForm;
use crate::scalar::{ExactScalar, ScalarArithmetic};
use crate::stabilizer::{AffineKet, PreparedZProjection};

pub(crate) fn tensor_error(error: PathTensorError) -> UnitaryError {
    match error {
        PathTensorError::DimensionOverflow => UnitaryError::DimensionOverflow,
        PathTensorError::Algebra(error) => UnitaryError::Phase(error),
    }
}

impl CompiledUnitary {
    /// Admit the original shape before projection and raw parity pullback.
    /// Projecting the full phase-labelled boundary prevents Gaussian
    /// contraction from discarding a direction that still carries odd phase.
    pub(crate) fn projected_path_tensor(
        &self,
        fixed: &[(usize, bool)],
        keep: &[usize],
        limits: ExpansionLimits,
        arithmetic: &ScalarArithmetic,
    ) -> Result<PhasePathTensor, UnitaryError> {
        self.projected_path_tensor_with(fixed, keep, limits, arithmetic, PreparedZProjection::new)
    }
    fn projected_path_tensor_with(
        &self,
        fixed: &[(usize, bool)],
        keep: &[usize],
        limits: ExpansionLimits,
        arithmetic: &ScalarArithmetic,
        projection: impl FnOnce(
            &AffineKet,
            &[BitVector],
        )
            -> Result<PreparedZProjection, crate::stabilizer::StabilizerError>,
    ) -> Result<PhasePathTensor, UnitaryError> {
        let paths = self.boundary.columns().len();
        let raw_paths = self.compilation.raw_path_variables.max(paths);
        admit("max_path_bits", limits.max_path_bits, raw_paths as u128)?;
        admit(
            "max_record_rows",
            limits.max_record_rows,
            fixed.len() as u128,
        )?;
        if self.width().checked_add(self.phase.num_variables()) != Some(self.boundary.width())
            || fixed.iter().any(|&(q, _)| q >= self.width())
            || keep.iter().any(|&q| q >= self.width())
        {
            return Err(UnitaryError::DimensionMismatch);
        }
        // Copies of the full ket, projection equations/provenance, pullback
        // axes and retained labels are all covered before any factory call.
        let width = self
            .boundary
            .width()
            .checked_add(fixed.len())
            .and_then(|n| n.checked_add(keep.len()))
            .ok_or(UnitaryError::DimensionOverflow)?;
        admit(
            "max_geometry_words",
            limits.max_geometry_words,
            geometry_words(width, paths, self.phase.terms().len(), 4),
        )?;
        arithmetic.multiply(self.boundary.coefficient(), &ExactScalar::one())?;
        let rows = fixed
            .iter()
            .map(|&(q, _)| BitVector::from_indices(self.boundary.width(), [q]))
            .collect::<Result<Vec<_>, _>>()?;
        let rhs = BitVector::from_indices(
            fixed.len(),
            fixed
                .iter()
                .enumerate()
                .filter_map(|(j, &(_, value))| value.then_some(j)),
        )?;
        let plan = projection(&self.boundary, &rows)?;
        let Some(projected) = plan.project(&self.boundary, &rhs)? else {
            return PhasePathTensor::new(
                AffineKet::new(
                    BitVector::zeros(keep.len())?,
                    vec![],
                    QuadraticForm::new(0, vec![], vec![])
                        .map_err(crate::phase::PhaseError::from)?,
                    ExactScalar::zero(),
                )?,
                ParityPhase::new(0, 0, [])?,
            )
            .map_err(tensor_error);
        };
        let phase_part = |value: &BitVector| {
            BitVector::from_indices(
                self.phase.num_variables(),
                value.ones().filter_map(|j| j.checked_sub(self.width())),
            )
        };
        let offset = phase_part(projected.offset())?;
        let columns = projected
            .columns()
            .iter()
            .map(phase_part)
            .collect::<Result<Vec<_>, _>>()?;
        let phase = self.phase.restrict(&offset, &columns)?;
        PhasePathTensor::new(projected.retain_boundary(keep)?, phase).map_err(tensor_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clifford::CliffordGate;
    use crate::unitary::{UnitaryCircuit, UnitaryGate};
    #[test]
    fn raw_projection_admission_precedes_the_actual_projection_factory() {
        let a = ScalarArithmetic::default();
        let compiled = UnitaryCircuit::new(
            1,
            [
                UnitaryGate::Clifford(CliffordGate::H(0)),
                UnitaryGate::T { qubit: 0, turns: 1 },
            ],
        )
        .unwrap()
        .compile(&BitVector::zeros(1).unwrap(), &a)
        .unwrap();
        for (name, limits) in [
            (
                "max_path_bits",
                ExpansionLimits {
                    max_path_bits: 0,
                    ..Default::default()
                },
            ),
            (
                "max_record_rows",
                ExpansionLimits {
                    max_record_rows: 0,
                    ..Default::default()
                },
            ),
            (
                "max_geometry_words",
                ExpansionLimits {
                    max_geometry_words: 0,
                    ..Default::default()
                },
            ),
        ] {
            let error = compiled
                .projected_path_tensor_with(&[(0, false)], &[], limits, &a, |_, _| {
                    panic!("structural refusal must precede projection")
                })
                .unwrap_err();
            assert!(
                matches!(error, UnitaryError::ExpansionResourceLimit { option, .. } if option == name)
            );
        }
        let tensor = compiled
            .projected_path_tensor(&[(0, false)], &[], ExpansionLimits::default(), &a)
            .unwrap();
        assert_eq!(tensor.boundary().width(), 0);
    }
}
