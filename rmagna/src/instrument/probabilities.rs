//! Bounded immutable probability plans shared by instrument branches.
use super::*;
use crate::fibers::FiberOptions;
use crate::unitary::PreparedProbability;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, Default)]
pub enum ProbabilityPreparation {
    #[default]
    Cold,
    Trace,
    FixedFibers(FiberOptions),
}
impl ProbabilityPreparation {
    pub fn name(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Trace => "trace",
            Self::FixedFibers(_) => "fixed",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedProbabilityStatistics {
    pub cached_queries: usize,
    pub fixed_resources: usize,
}
pub(super) struct ProbabilityQueries {
    mode: ProbabilityPreparation,
    limit: usize,
    plans: Mutex<HashMap<Vec<BitVector>, Arc<PreparedProbability>>>,
}
impl ProbabilityQueries {
    pub(super) fn new(mode: ProbabilityPreparation, limit: usize) -> Result<Self, InstrumentError> {
        if !matches!(mode, ProbabilityPreparation::Cold) && limit == 0 {
            return Err(InstrumentError::ResourceLimit {
                option: "max_prepared_probabilities",
                limit,
                required: 1,
            });
        }
        Ok(Self {
            mode,
            limit,
            plans: Mutex::new(HashMap::new()),
        })
    }
    pub(super) fn statistics(&self) -> Result<PreparedProbabilityStatistics, InstrumentError> {
        let plans = self
            .plans
            .lock()
            .map_err(|_| InstrumentError::PreparedQueryCacheUnavailable)?;
        Ok(PreparedProbabilityStatistics {
            cached_queries: plans.len(),
            fixed_resources: plans
                .values()
                .filter(|p| p.fixed_phase_resource().is_some())
                .count(),
        })
    }
    pub(super) fn evaluate(
        &self,
        state: &CompiledUnitary,
        rows: &[BitVector],
        outcomes: &BitVector,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        if matches!(self.mode, ProbabilityPreparation::Cold) {
            return Ok(state.probability_z(rows, outcomes, max_terms, arithmetic)?);
        }
        // One lock serializes preparation and enforces the bound across threads.
        // Evaluation runs after releasing it. Plans own programs, while programs
        // never own this cache, so retaining branches creates no reference cycle.
        let plan = {
            let mut plans = self
                .plans
                .lock()
                .map_err(|_| InstrumentError::PreparedQueryCacheUnavailable)?;
            if let Some(plan) = plans.get(rows) {
                Arc::clone(plan)
            } else {
                if plans.len() >= self.limit {
                    return Err(InstrumentError::ResourceLimit {
                        option: "max_prepared_probabilities",
                        limit: self.limit,
                        required: plans.len().saturating_add(1),
                    });
                }
                let plan = match self.mode {
                    ProbabilityPreparation::Trace => state.prepare_probability(rows, arithmetic)?,
                    ProbabilityPreparation::FixedFibers(options) => state
                        .prepare_probability_with_fibers(rows, max_terms, options, arithmetic)?,
                    ProbabilityPreparation::Cold => unreachable!("cold queries returned above"),
                };
                let plan = Arc::new(plan);
                plans.insert(rows.to_vec(), Arc::clone(&plan));
                plan
            }
        };
        Ok(plan.evaluate(state, outcomes, max_terms, arithmetic)?)
    }
}
