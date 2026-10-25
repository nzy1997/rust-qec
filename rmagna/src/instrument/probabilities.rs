//! Bounded immutable probability plans shared by instrument branches.
use super::*;
use crate::fibers::FiberOptions;
use crate::unitary::PreparedProbability;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, Default)]
pub enum QueryPreparation {
    #[default]
    Cold,
    Trace,
    FixedFibers(FiberOptions),
    /// Prefer a matching built-in certificate, otherwise retain fixed fibers.
    FixedCatalogue(crate::phase::PhaseResourceOptions),
}
impl QueryPreparation {
    pub fn is_fixed(self) -> bool {
        matches!(self, Self::FixedFibers(_) | Self::FixedCatalogue(_))
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Trace => "trace",
            Self::FixedFibers(_) => "fixed",
            Self::FixedCatalogue(_) => "catalogue",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedQueryStatistics {
    pub cached_queries: usize,
    pub fixed_resources: usize,
    /// Actual selected catalogue resources; missing keys count as fibers.
    pub catalogue_resources: usize,
}
/// Compatibility names for the probability-only preparation interface.
pub use QueryPreparation as ProbabilityPreparation;
pub type PreparedProbabilityStatistics = PreparedQueryStatistics;
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
            catalogue_resources: plans
                .values()
                .filter_map(|p| p.fixed_phase_resource())
                .filter(|r| r.kind() == crate::phase::PhaseResourceKind::CertifiedCatalogue)
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
        let (plan, _) = self.plan(state, rows, max_terms, arithmetic)?;
        Ok(plan.evaluate(state, outcomes, max_terms, arithmetic)?)
    }
    pub(super) fn prepare(
        &self,
        state: &CompiledUnitary,
        rows: &[BitVector],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<QueryPreparationOutcome, InstrumentError> {
        if rows.iter().any(|r| r.width() != state.width()) {
            return Err(UnitaryError::DimensionMismatch.into());
        }
        state
            .width()
            .checked_add(rows.len())
            .ok_or(UnitaryError::DimensionOverflow)?;
        if matches!(self.mode, ProbabilityPreparation::Cold) {
            return Ok(QueryPreparationOutcome::Cold);
        }
        let (plan, outcome) = self.plan(state, rows, max_terms, arithmetic)?;
        super::preparation::validate_resource(plan.fixed_phase_resource(), max_terms)?;
        Ok(outcome)
    }
    fn plan(
        &self,
        state: &CompiledUnitary,
        rows: &[BitVector],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<(Arc<PreparedProbability>, QueryPreparationOutcome), InstrumentError> {
        // One lock serializes preparation and enforces the bound across threads.
        // Evaluation runs after releasing it. Plans own programs, while programs
        // never own this cache, so retaining branches creates no reference cycle.
        let result = {
            let mut plans = self
                .plans
                .lock()
                .map_err(|_| InstrumentError::PreparedQueryCacheUnavailable)?;
            if let Some(plan) = plans.get(rows) {
                (Arc::clone(plan), QueryPreparationOutcome::Reused)
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
                    ProbabilityPreparation::FixedCatalogue(options) => state
                        .prepare_probability(rows, arithmetic)?
                        .with_phase_resource(
                            max_terms,
                            options,
                            Some(crate::catalogue::builtin_phase_catalogue()?),
                            arithmetic,
                        )?,
                    ProbabilityPreparation::Cold => unreachable!("cold queries returned above"),
                };
                let plan = Arc::new(plan);
                plans.insert(rows.to_vec(), Arc::clone(&plan));
                (plan, QueryPreparationOutcome::Created)
            }
        };
        Ok(result)
    }
}
