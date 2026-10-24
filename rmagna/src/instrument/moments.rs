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
        Ok(PreparedQueryStatistics {
            cached_queries: entries.len(),
            fixed_resources: entries
                .iter()
                .filter(|e| e.plan.fixed_phase_resource().is_some())
                .count(),
        })
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
        let plan = {
            let mut entries = self
                .entries
                .lock()
                .map_err(|_| InstrumentError::PreparedQueryCacheUnavailable)?;
            if let Some(entry) = entries.iter().find(|e| e.word == *word && e.rows == rows) {
                Arc::clone(&entry.plan)
            } else {
                if entries.len() >= self.limit {
                    return Err(InstrumentError::ResourceLimit {
                        option: "max_prepared_moments",
                        limit: self.limit,
                        required: entries.len().saturating_add(1),
                    });
                }
                let plan = match self.mode {
                    QueryPreparation::Trace => {
                        state.prepare_pauli_moment(word, rows, arithmetic)?
                    }
                    QueryPreparation::FixedFibers(options) => state
                        .prepare_pauli_moment_with_fibers(
                            word, rows, max_terms, options, arithmetic,
                        )?,
                    QueryPreparation::Cold => unreachable!("cold moments returned above"),
                };
                let plan = Arc::new(plan);
                entries.push(Entry {
                    word: word.clone(),
                    rows: rows.to_vec(),
                    plan: Arc::clone(&plan),
                });
                plan
            }
        };
        Ok(plan.evaluate(state, outcomes, max_terms, arithmetic)?)
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
        let queries =
            MomentQueries::new(QueryPreparation::FixedFibers(Default::default()), 1).unwrap();
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
                fixed_resources: 0
            }
        );
        assert!(matches!(
            queries.evaluate(&state, &word, &[row], &BitVector::zeros(2).unwrap(), 0, &a),
            Err(InstrumentError::Unitary(UnitaryError::DimensionMismatch))
        ));
        assert_eq!(queries.statistics().unwrap().cached_queries, 1);
    }
}
