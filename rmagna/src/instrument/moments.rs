//! Bounded immutable Pauli/event families shared by instrument branches.
use super::*;
use crate::unitary::PreparedPauliMoment;
use std::sync::Mutex;

struct Entry {
    word: PauliWord,
    rows: Vec<BitVector>,
    plan: Arc<PreparedPauliMoment>,
}
pub(super) struct MomentQueries {
    mode: QueryPreparation,
    limit: usize,
    entries: Mutex<Vec<Entry>>,
}
impl MomentQueries {
    pub(super) fn new(mode: QueryPreparation, limit: usize) -> Result<Self, InstrumentError> {
        if !matches!(mode, QueryPreparation::Cold) && limit == 0 {
            return Err(InstrumentError::ResourceLimit {
                option: "max_prepared_moments",
                limit,
                required: 1,
            });
        }
        Ok(Self {
            mode,
            limit,
            entries: Mutex::new(Vec::new()),
        })
    }
    pub(super) fn statistics(&self) -> Result<PreparedQueryStatistics, InstrumentError> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| InstrumentError::PreparedQueryCacheUnavailable)?;
        Ok(PreparedQueryStatistics::from_resources(
            entries.len(),
            entries
                .iter()
                .filter_map(|entry| entry.plan.fixed_phase_resource()),
        ))
    }
    pub(super) fn evaluate(
        &self,
        state: &CompiledUnitary,
        word: &PauliWord,
        rows: &[BitVector],
        outcomes: &BitVector,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        if word.width() != state.width()
            || rows.len() != outcomes.width()
            || rows.iter().any(|r| r.width() != state.width())
        {
            return Err(UnitaryError::DimensionMismatch.into());
        }
        if matches!(self.mode, QueryPreparation::Cold) {
            return Ok(state.pauli_moment(word, rows, outcomes, max_terms, arithmetic)?);
        }
        let (plan, _) = self.plan(state, word, rows, max_terms, arithmetic)?;
        Ok(plan.evaluate(state, outcomes, max_terms, arithmetic)?)
    }
    pub(super) fn prepare(
        &self,
        state: &CompiledUnitary,
        word: &PauliWord,
        rows: &[BitVector],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<QueryPreparationOutcome, InstrumentError> {
        if word.width() != state.width() || rows.iter().any(|r| r.width() != state.width()) {
            return Err(UnitaryError::DimensionMismatch.into());
        }
        state
            .width()
            .checked_add(rows.len())
            .ok_or(UnitaryError::DimensionOverflow)?;
        if matches!(self.mode, QueryPreparation::Cold) {
            return Ok(QueryPreparationOutcome::Cold);
        }
        let (plan, outcome) = self.plan(state, word, rows, max_terms, arithmetic)?;
        super::preparation::validate_resource(plan.fixed_phase_resource(), max_terms)?;
        Ok(outcome)
    }
    fn plan(
        &self,
        state: &CompiledUnitary,
        word: &PauliWord,
        rows: &[BitVector],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<(Arc<PreparedPauliMoment>, QueryPreparationOutcome), InstrumentError> {
        let result = {
            let mut entries = self
                .entries
                .lock()
                .map_err(|_| InstrumentError::PreparedQueryCacheUnavailable)?;
            if let Some(entry) = entries.iter().find(|e| e.word == *word && e.rows == rows) {
                (Arc::clone(&entry.plan), QueryPreparationOutcome::Reused)
            } else {
                if entries.len() >= self.limit {
                    return Err(InstrumentError::ResourceLimit {
                        option: "max_prepared_moments",
                        limit: self.limit,
                        required: entries.len().saturating_add(1),
                    });
                }
                let plan = match self.mode {
                    QueryPreparation::Native(options) => state
                        .prepare_pauli_moment(word, rows, arithmetic)?
                        .with_native_phase_resource(
                            max_terms,
                            options,
                            Some(crate::catalogue::builtin_phase_catalogue()?),
                            arithmetic,
                        )?,
                    QueryPreparation::Trace => {
                        state.prepare_pauli_moment(word, rows, arithmetic)?
                    }
                    QueryPreparation::FixedFibers(options) => state
                        .prepare_pauli_moment_with_fibers(
                            word, rows, max_terms, options, arithmetic,
                        )?,
                    QueryPreparation::FixedCatalogue(options) => state
                        .prepare_pauli_moment(word, rows, arithmetic)?
                        .with_phase_resource(
                            max_terms,
                            options,
                            Some(crate::catalogue::builtin_phase_catalogue()?),
                            arithmetic,
                        )?,
                    QueryPreparation::Cold => unreachable!("cold moments returned above"),
                };
                let plan = Arc::new(plan);
                entries.push(Entry {
                    word: word.clone(),
                    rows: rows.to_vec(),
                    plan: Arc::clone(&plan),
                });
                (plan, QueryPreparationOutcome::Created)
            }
        };
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_certificates_occupy_one_family_without_a_phase_resource() {
        let a = ScalarArithmetic::default();
        let state = crate::unitary::UnitaryCircuit::new(3, [])
            .unwrap()
            .compile(&BitVector::zeros(3).unwrap(), &a)
            .unwrap();
        let row = BitVector::from_indices(3, [0]).unwrap();
        let word = PauliWord::new(row.clone(), BitVector::zeros(3).unwrap(), 2).unwrap();
        for mode in [
            QueryPreparation::FixedFibers(Default::default()),
            QueryPreparation::FixedCatalogue(Default::default()),
        ] {
            let queries = MomentQueries::new(mode, 1).unwrap();
            assert_eq!(
                queries
                    .prepare(&state, &word, std::slice::from_ref(&row), 0, &a)
                    .unwrap(),
                QueryPreparationOutcome::Created
            );
            assert_eq!(
                queries
                    .prepare(&state, &word, std::slice::from_ref(&row), 0, &a)
                    .unwrap(),
                QueryPreparationOutcome::Reused
            );
            for rhs in [
                BitVector::zeros(1).unwrap(),
                BitVector::from_indices(1, [0]).unwrap(),
            ] {
                let value = queries
                    .evaluate(&state, &word, std::slice::from_ref(&row), &rhs, 0, &a)
                    .unwrap();
                assert_eq!(value.value, ExactScalar::zero());
                assert_eq!((value.quotient_rank, value.terms), (0, 0));
            }
            assert_eq!(
                queries.statistics().unwrap(),
                PreparedQueryStatistics {
                    cached_queries: 1,
                    fixed_resources: 0,
                    catalogue_resources: 0,
                    native_routes: Default::default(),
                    native_transports: Default::default(),
                    maximum_quotient_rank: None,
                    maximum_terms: None,
                    maximum_phase_variables: None,
                }
            );
            assert!(matches!(
                queries.evaluate(
                    &state,
                    &word,
                    std::slice::from_ref(&row),
                    &BitVector::zeros(2).unwrap(),
                    0,
                    &a
                ),
                Err(InstrumentError::Unitary(UnitaryError::DimensionMismatch))
            ));
            assert_eq!(queries.statistics().unwrap().cached_queries, 1);
        }
    }
}
