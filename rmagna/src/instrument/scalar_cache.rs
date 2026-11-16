//! Explicit scalar-plan LRU over named instrument records and physical outputs.
use super::*;
use crate::unitary::{ScalarQuery, ScalarQueryCache, ScalarQueryCacheOptions};

#[derive(Clone, Copy, Debug)]
pub struct InstrumentScalarQuery<'a> {
    pub outputs: &'a [usize],
    pub pauli: &'a PauliWord,
    pub parities: &'a [BitExpr],
    pub outcomes: &'a BitVector,
}
impl CompiledInstrument {
    /// Bind an explicit evicting plan cache to this instrument's actual program.
    /// Resolved fixed-template histories share this owner; cold-prefix programs
    /// do not. This cache's capacity is separate from automatic family quotas.
    pub fn scalar_query_cache(
        &self,
        options: ScalarQueryCacheOptions,
        catalogue: Option<Arc<crate::catalogue::PhaseCatalogue>>,
    ) -> ScalarQueryCache {
        ScalarQueryCache::new(&self.unitary, options, catalogue)
    }
    /// Raw reported-event Pauli mass, excluding the instrument's selection
    /// predicates, as with reported_probability. Pass explicit reported rows
    /// for desired conditions. Actual decision offsets and active fault/report
    /// choices remain data; no callback is executed or result value cached here.
    pub fn reported_moment_cached(
        &self,
        cache: &mut ScalarQueryCache,
        query: InstrumentScalarQuery<'_>,
        faults: &[FaultSelection],
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        cache.validate_owner(&self.unitary)?;
        if query.outcomes.width() != query.parities.len() {
            return Err(UnitaryError::DimensionMismatch.into());
        }
        // Admit row copies and packed kernel geometry before mapping vectors.
        cache.admit_event_count(query.parities.len())?;
        let word = self.physical_word(query.outputs, query.pauli)?;
        let mut rows = Vec::with_capacity(query.parities.len());
        let mut outcomes = query.outcomes.clone();
        for (j, parity) in query.parities.iter().enumerate() {
            rows.push(self.parity_row(parity)?);
            outcomes.set(j, outcomes.get(j)? ^ parity.constant)?;
        }
        Ok(cache.evaluate(
            &self.inserted(faults, arithmetic)?,
            ScalarQuery {
                events: &rows,
                outcomes: &outcomes,
                pauli: &word,
            },
            arithmetic,
        )?)
    }
}
