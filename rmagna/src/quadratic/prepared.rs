//! Immutable Gaussian programs for compatible quadratic phase updates.
use super::*;

/// Exact Gaussian sum with structural elimination prepared once.
///
/// Constants and even linear phases may change. Cross terms and linear parity
/// must match the reference. Preparation records every step even when the
/// reference sum is zero, since a compatible update can make it nonzero.
/// No phase-resource or circuit-query plan is implied by this primitive.
pub struct PreparedQuadraticSum {
    reference: QuadraticForm,
    actions: Vec<QuadraticAction>,
}
impl PreparedQuadraticSum {
    pub fn new(reference: &QuadraticForm) -> Result<Self, QuadraticError> {
        let mut state = QuadraticElimination::new(reference)?;
        let mut actions = Vec::new();
        loop {
            let next = (0..state.active.len())
                .find(|&i| state.active[i] && state.values.linear[i] & 1 != 0)
                .or_else(|| state.active.iter().position(|&bit| bit));
            let Some(x) = next else { break };
            let (_, action) = state.eliminate_recorded(x, &[])?;
            actions.push(action);
        }
        Ok(Self {
            reference: reference.clone(),
            actions,
        })
    }
    /// Reject a changed structural signature before executing any prepared step.
    pub fn evaluate(
        &self,
        updated: &QuadraticForm,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, QuadraticError> {
        if updated.num_variables() != self.reference.num_variables()
            || updated.upper != self.reference.upper
            || updated
                .linear
                .iter()
                .zip(&self.reference.linear)
                .any(|(a, b)| (a ^ b) & 1 != 0)
        {
            return Err(QuadraticError::IncompatiblePreparation);
        }
        let mut values = QuadraticValues::new(updated);
        for action in &self.actions {
            if !action.apply(&mut values)? {
                return Ok(ExactScalar::zero());
            }
        }
        values.scalar(arithmetic)
    }
}
