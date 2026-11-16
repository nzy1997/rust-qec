//! Explicit LRU for frozen scalar query plans, not probability result values.
use super::*;
use crate::{
    catalogue::PhaseCatalogue,
    phase::{PhaseResourceOptions, ResourcePreparation},
};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug)]
pub struct ScalarQueryCacheOptions {
    pub max_plans: usize,
    pub max_event_rows: usize,
    /// Conservative packed projection/tensor/metadata/scratch admission per
    /// query. Existing compiled owners, scalar limbs and phase-resource storage
    /// are excluded; the resource policy has its own independent bounds.
    pub max_geometry_words: usize,
    pub max_batch_queries: usize,
    pub max_terms: usize,
    pub resource: ResourcePreparation,
}
impl Default for ScalarQueryCacheOptions {
    fn default() -> Self {
        Self {
            max_plans: 32,
            max_event_rows: 4096,
            max_geometry_words: 1 << 24,
            max_batch_queries: 4096,
            max_terms: 256,
            resource: ResourcePreparation::Catalogue(PhaseResourceOptions::default()),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct ScalarQuery<'a> {
    pub events: &'a [BitVector],
    pub outcomes: &'a BitVector,
    pub pauli: &'a PauliWord,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScalarQueryCacheStatistics {
    pub hits: usize,
    pub preparation_attempts: usize,
    pub preparations: usize,
    pub evictions: usize,
    pub evaluations: usize,
}
struct Entry {
    events: Vec<BitVector>,
    pauli: PauliWord,
    plan: Arc<PreparedPauliMoment>,
}
/// One actual immutable insertion-program owner, with explicitly evicting plans.
/// Outcome RHSs and inserted faults remain evaluation inputs. Every call checks
/// owner, dimensions, row quotas, Hermiticity and commutation before lookup.
/// Resources are frozen with immutable policy/term bounds; warm calls still
/// execute exact evaluation and its arithmetic checks. A zero capacity prepares
/// ephemeral plans. No callback, result-value cache or backend fallback exists.
pub struct ScalarQueryCache {
    program: Arc<InsertionProgram>,
    width: usize,
    options: ScalarQueryCacheOptions,
    catalogue: Option<Arc<PhaseCatalogue>>,
    entries: VecDeque<Entry>,
    statistics: ScalarQueryCacheStatistics,
}
impl ScalarQueryCache {
    pub fn new(
        source: &CompiledUnitary,
        options: ScalarQueryCacheOptions,
        catalogue: Option<Arc<PhaseCatalogue>>,
    ) -> Self {
        Self {
            program: Arc::clone(&source.program),
            width: source.width(),
            options,
            catalogue,
            entries: VecDeque::new(),
            statistics: Default::default(),
        }
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn options(&self) -> ScalarQueryCacheOptions {
        self.options
    }
    pub fn statistics(&self) -> ScalarQueryCacheStatistics {
        self.statistics
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    pub fn set_max_plans(&mut self, capacity: usize) {
        self.options.max_plans = capacity;
        while self.entries.len() > capacity {
            self.evict();
        }
    }
    fn evict(&mut self) {
        if self.entries.pop_front().is_some() {
            self.statistics.evictions = self.statistics.evictions.saturating_add(1);
        }
    }
    pub(crate) fn validate_owner(&self, state: &CompiledUnitary) -> Result<(), UnitaryError> {
        if !Arc::ptr_eq(&self.program, &state.program) {
            return Err(UnitaryError::IncompatibleProgram);
        }
        if state.width() != self.width {
            return Err(UnitaryError::DimensionMismatch);
        }
        Ok(())
    }
    pub(crate) fn admit_event_count(&self, event_count: usize) -> Result<(), UnitaryError> {
        if event_count > self.options.max_event_rows {
            return Err(UnitaryError::QueryCacheLimit {
                option: "scalar_query_cache.max_event_rows",
                limit: self.options.max_event_rows,
                required: event_count as u128,
            });
        }
        let boundary = self.program.query_reference.width();
        let paths = self.program.query_reference.columns().len();
        let width = 2 * boundary as u128;
        let variables = 2 * paths as u128;
        let rows = (self.width as u128).saturating_add(event_count as u128);
        // Overcharge projection factors, kernels, quadratic maps, row storage
        // and working copies, before creating their doubled reference tensor.
        let words = variables
            .saturating_add(rows)
            .saturating_add(8)
            .saturating_mul(
                width
                    .div_ceil(64)
                    .saturating_add(variables.div_ceil(64))
                    .saturating_add(8),
            )
            .saturating_mul(64);
        if words > self.options.max_geometry_words as u128 {
            return Err(UnitaryError::QueryCacheLimit {
                option: "scalar_query_cache.max_geometry_words",
                limit: self.options.max_geometry_words,
                required: words,
            });
        }
        Ok(())
    }
    fn validate(
        &self,
        state: &CompiledUnitary,
        query: ScalarQuery<'_>,
    ) -> Result<(), UnitaryError> {
        self.validate_owner(state)?;
        if query.pauli.width() != self.width
            || query.outcomes.width() != query.events.len()
            || query.events.iter().any(|row| row.width() != self.width)
        {
            return Err(UnitaryError::DimensionMismatch);
        }
        self.admit_event_count(query.events.len())?;
        if query.pauli.phase() & 3 != 2 * u8::from(query.pauli.x().dot(query.pauli.z())?) {
            return Err(UnitaryError::NonHermitianScalarQuery);
        }
        for row in query.events {
            if row.dot(query.pauli.x())? {
                return Err(UnitaryError::NonCommutingScalarQuery);
            }
        }
        Ok(())
    }
    fn plan(
        &mut self,
        state: &CompiledUnitary,
        query: ScalarQuery<'_>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Arc<PreparedPauliMoment>, UnitaryError> {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.events == query.events && entry.pauli == *query.pauli)
        {
            let entry = self.entries.remove(index).expect("located scalar plan");
            let plan = Arc::clone(&entry.plan);
            self.entries.push_back(entry);
            self.statistics.hits = self.statistics.hits.saturating_add(1);
            return Ok(plan);
        }
        self.statistics.preparation_attempts =
            self.statistics.preparation_attempts.saturating_add(1);
        let plan = state.prepare_pauli_moment(query.pauli, query.events, arithmetic)?;
        let plan = match self.options.resource {
            ResourcePreparation::Catalogue(policy) => plan.with_phase_resource(
                self.options.max_terms,
                policy,
                self.catalogue.as_deref(),
                arithmetic,
            )?,
            ResourcePreparation::Native(policy) => plan.with_native_phase_resource(
                self.options.max_terms,
                policy,
                self.catalogue.as_deref(),
                arithmetic,
            )?,
        };
        self.statistics.preparations = self.statistics.preparations.saturating_add(1);
        let plan = Arc::new(plan);
        if self.options.max_plans != 0 {
            while self.entries.len() >= self.options.max_plans {
                self.evict();
            }
            self.entries.push_back(Entry {
                events: query.events.to_vec(),
                pauli: query.pauli.clone(),
                plan: Arc::clone(&plan),
            });
        }
        Ok(plan)
    }
    pub fn evaluate(
        &mut self,
        state: &CompiledUnitary,
        query: ScalarQuery<'_>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        self.validate(state, query)?;
        self.evaluate_validated(state, query, arithmetic)
    }
    /// Prepare without numeric evaluation. A returned owned plan may outlive
    /// eviction; max_plans bounds this cache's entries, not caller-owned Arcs.
    pub fn prepare_query(
        &mut self,
        state: &CompiledUnitary,
        query: ScalarQuery<'_>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Arc<PreparedPauliMoment>, UnitaryError> {
        self.validate(state, query)?;
        self.plan(state, query, arithmetic)
    }
    fn evaluate_validated(
        &mut self,
        state: &CompiledUnitary,
        query: ScalarQuery<'_>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        let plan = self.plan(state, query, arithmetic)?;
        self.statistics.evaluations = self.statistics.evaluations.saturating_add(1);
        plan.evaluate(state, query.outcomes, self.options.max_terms, arithmetic)
    }
    /// Admit every request shape before preparing anything. A later kernel
    /// failure returns no result vector, but successful plans/counters remain.
    pub fn evaluate_many(
        &mut self,
        state: &CompiledUnitary,
        queries: &[ScalarQuery<'_>],
        arithmetic: &ScalarArithmetic,
    ) -> Result<Vec<NetworkValue>, UnitaryError> {
        self.validate_owner(state)?;
        if queries.len() > self.options.max_batch_queries {
            return Err(UnitaryError::QueryCacheLimit {
                option: "scalar_query_cache.max_batch_queries",
                limit: self.options.max_batch_queries,
                required: queries.len() as u128,
            });
        }
        for query in queries {
            self.validate(state, *query)?;
        }
        queries
            .iter()
            .map(|query| self.evaluate_validated(state, *query, arithmetic))
            .collect()
    }
}
