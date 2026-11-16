//! Draw-local numeric assignment shared by terminal scalar-query families.
use super::*;

/// Construction is private to the instrument/runtime boundary. A cached value
/// or a view from another instrument cannot enter this path through public APIs.
/// Query plans still validate their shape, outcomes and current scalar budgets.
pub(crate) struct AssignedInstrument<'a> {
    instrument: &'a CompiledInstrument,
    state: Arc<CompiledUnitary>,
}
impl<'a> AssignedInstrument<'a> {
    pub(crate) fn new(instrument: &'a CompiledInstrument, state: Arc<CompiledUnitary>) -> Self {
        Self { instrument, state }
    }
    pub(crate) fn into_state(self) -> Arc<CompiledUnitary> {
        self.state
    }
    pub(crate) fn acceptance(
        &self,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        let i = self.instrument;
        i.probability_queries.evaluate(
            &self.state,
            &i.event_rows,
            &i.outcomes,
            max_terms,
            arithmetic,
        )
    }
    pub(crate) fn accepted_probability(
        &self,
        parities: &[BitExpr],
        outcomes: &BitVector,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        let i = self.instrument;
        let (rows, rhs) = i.accepted_equations(parities, outcomes)?;
        i.probability_queries
            .evaluate(&self.state, &rows, &rhs, max_terms, arithmetic)
    }
    pub(crate) fn reported_probability(
        &self,
        parities: &[BitExpr],
        outcomes: &BitVector,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        let (rows, rhs) = self.instrument.reported_equations(parities, outcomes)?;
        self.instrument.probability_queries.evaluate(
            &self.state,
            &rows,
            &rhs,
            max_terms,
            arithmetic,
        )
    }
    pub(crate) fn target_density_weight(
        &self,
        outputs: &[usize],
        target: &CompiledUnitary,
        target_boundary: crate::unitary::DensityBoundary<'_>,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        let i = self.instrument;
        let physical = i.output_wires(outputs)?;
        i.density_queries.evaluate(
            &self.state,
            crate::unitary::DensityBoundary {
                outputs: &physical,
                event_rows: &i.event_rows,
                outcomes: &i.outcomes,
            },
            target,
            target_boundary,
            max_terms,
            arithmetic,
        )
    }
    pub(crate) fn pauli_moment(
        &self,
        outputs: &[usize],
        word: &PauliWord,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        let i = self.instrument;
        let word = i.physical_word(outputs, word)?;
        i.moment_queries.evaluate(
            &self.state,
            &word,
            &i.event_rows,
            &i.outcomes,
            max_terms,
            arithmetic,
        )
    }
    pub(crate) fn masses(
        &self,
        outputs: &[usize],
        target: &CompiledUnitary,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactMasses, InstrumentError> {
        let accepted = self.acceptance(max_terms, arithmetic)?;
        let target_outputs = (0..target.width()).collect::<Vec<_>>();
        let empty = BitVector::zeros(0)?;
        let target_success = self.target_density_weight(
            outputs,
            target,
            crate::unitary::DensityBoundary {
                outputs: &target_outputs,
                event_rows: &[],
                outcomes: &empty,
            },
            max_terms,
            arithmetic,
        )?;
        let error = arithmetic.subtract(&accepted.value, &target_success.value)?;
        Ok(ExactMasses {
            accepted,
            target_success,
            error,
        })
    }
}

impl CompiledInstrument {
    pub(super) fn accepted_equations(
        &self,
        parities: &[BitExpr],
        outcomes: &BitVector,
    ) -> Result<(Vec<BitVector>, BitVector), InstrumentError> {
        if parities.len() != outcomes.width() {
            return Err(UnitaryError::DimensionMismatch.into());
        }
        let mut rows = self.event_rows.as_ref().clone();
        let count = rows
            .len()
            .checked_add(parities.len())
            .ok_or(UnitaryError::DimensionOverflow)?;
        let mut rhs = BitVector::from_indices(count, self.outcomes.ones())?;
        for (j, parity) in parities.iter().enumerate() {
            rows.push(self.parity_row(parity)?);
            rhs.set(
                self.event_rows.len() + j,
                outcomes.get(j)? ^ parity.constant,
            )?;
        }
        Ok((rows, rhs))
    }
    pub(super) fn reported_equations(
        &self,
        parities: &[BitExpr],
        outcomes: &BitVector,
    ) -> Result<(Vec<BitVector>, BitVector), InstrumentError> {
        if parities.len() != outcomes.width() {
            return Err(UnitaryError::DimensionMismatch.into());
        }
        let mut rows = Vec::with_capacity(parities.len());
        let mut rhs = outcomes.clone();
        for (j, parity) in parities.iter().enumerate() {
            rows.push(self.parity_row(parity)?);
            rhs.set(j, outcomes.get(j)? ^ parity.constant)?;
        }
        Ok((rows, rhs))
    }
}
