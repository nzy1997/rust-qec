//! Fixed structural Gaussian contraction of compatible affine quadratic kets.
use super::*;
use crate::quadratic::{QuadraticAction, QuadraticValues};

struct Step {
    action: QuadraticAction,
    offset_column: Option<BitVector>,
}

/// Immutable affine contraction across offset and even phase updates.
///
/// Coefficients, offsets, constants and even linear terms may change. Boundary
/// width, support columns, quadratic cross terms and linear parity must agree.
/// Every structural step is recorded even for a zero reference. Evaluation
/// preserves unnormalized amplitudes and performs no support factorization.
/// Non-Clifford phase resources and complete query closure are separate.
pub struct PreparedAffineContraction {
    reference: AffineKet,
    basis: Vec<BitVector>,
    restricted: QuadraticForm,
    steps: Vec<Step>,
    columns: Vec<BitVector>,
    keep: Vec<usize>,
    upper: Vec<BitVector>,
}
impl PreparedAffineContraction {
    pub fn new(reference: &AffineKet) -> Result<Self, StabilizerError> {
        let count = reference.columns.len();
        let factor = LinearFactorization::new(
            count,
            &transpose_columns(reference.width(), &reference.columns)?,
        )?;
        let mut basis = factor
            .pivot_columns()
            .iter()
            .map(|&j| BitVector::from_indices(count, [j]))
            .collect::<Result<Vec<_>, _>>()?;
        basis.extend(factor.nullspace()?);
        let restricted = reference
            .phase
            .restrict(&BitVector::zeros(count)?, &basis)?;
        let mut columns = factor
            .pivot_columns()
            .iter()
            .map(|&j| reference.columns[j].clone())
            .collect::<Vec<_>>();
        columns.resize(count, BitVector::zeros(reference.width())?);
        let mut hidden = columns.iter().map(BitVector::is_zero).collect::<Vec<_>>();
        let mut state = QuadraticElimination::new(&restricted)?;
        let mut steps = Vec::new();
        while let Some(x) = (0..count).find(|&i| state.active[i] && hidden[i]) {
            let (_, action) = state.eliminate_recorded(x, &hidden)?;
            let offset_column = if let QuadraticAction::Pair { y, v, .. } = &action {
                let column = columns[*y].clone();
                for &i in v {
                    columns[i].xor_assign(&column)?;
                    hidden[i] = columns[i].is_zero();
                }
                (!column.is_zero()).then_some(column)
            } else {
                None
            };
            steps.push(Step {
                action,
                offset_column,
            });
        }
        let (remaining, keep) = state.remaining()?;
        let columns = keep.iter().map(|&j| columns[j].clone()).collect();
        Ok(Self {
            reference: reference.clone(),
            basis,
            restricted,
            steps,
            columns,
            upper: remaining.upper().to_vec(),
            keep,
        })
    }

    pub(crate) fn output_columns(&self) -> &[BitVector] {
        &self.columns
    }

    /// Contract a compatible ket, including updates to a zero reference.
    /// An incompatible structure is rejected before returning a zero result.
    pub fn contract(
        &self,
        input: &AffineKet,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Option<AffineKet>, StabilizerError> {
        if !input.same_structure(&self.reference) {
            return Err(StabilizerError::IncompatibleContraction);
        }
        if input.coefficient.is_zero() {
            return Ok(None);
        }
        let delta = BitVector::from_indices(
            input.columns.len(),
            input
                .phase
                .linear()
                .iter()
                .zip(self.reference.phase.linear())
                .enumerate()
                .filter_map(|(j, (a, b))| (a != b).then_some(j)),
        )?;
        let linear = self
            .restricted
            .linear()
            .iter()
            .zip(&self.basis)
            .map(|(&c, v)| v.dot(&delta).map(|bit| (c + 2 * u8::from(bit)) & 3))
            .collect::<Result<Vec<_>, _>>()?;
        let mut values = QuadraticValues::from_linear(input.phase.constant(), linear);
        let mut offset = input.offset.clone();
        for step in &self.steps {
            if !step.action.apply(&mut values)? {
                return Ok(None);
            }
            if let (QuadraticAction::Pair { x, .. }, Some(column)) =
                (&step.action, &step.offset_column)
                && values.linear[*x] != 0
            {
                offset.xor_assign(column)?;
            }
        }
        let coefficient = arithmetic.multiply(&input.coefficient, &values.scalar(arithmetic)?)?;
        let phase = QuadraticForm::new(
            0,
            self.keep.iter().map(|&j| values.linear[j]).collect(),
            self.upper.clone(),
        )?;
        Ok(Some(AffineKet::new(
            offset,
            self.columns.clone(),
            phase,
            coefficient,
        )?))
    }
}
