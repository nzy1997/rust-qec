//! Bounded density-overlap plans distinguished by layouts and program identity.
use super::*;
use crate::unitary::{DensityBoundary, DensityLayout, PreparedDensityOverlap};
use std::sync::Mutex;

struct Entry {
    left_outputs: Vec<usize>,
    right_outputs: Vec<usize>,
    left_rows: Vec<BitVector>,
    right_rows: Vec<BitVector>,
    plan: Arc<PreparedDensityOverlap>,
}
impl Entry {
    fn matches(
        &self,
        left: &CompiledUnitary,
        boundary: &DensityLayout<'_>,
        right: &CompiledUnitary,
        target: &DensityLayout<'_>,
    ) -> bool {
        self.left_outputs == boundary.outputs
            && self.right_outputs == target.outputs
            && self.left_rows == boundary.event_rows
            && self.right_rows == target.event_rows
            && self.plan.matches_programs(left, right)
    }
}
pub(super) struct DensityQueries {
    mode: QueryPreparation,
    limit: usize,
    entries: Mutex<Vec<Entry>>,
}
impl DensityQueries {
    pub(super) fn new(mode: QueryPreparation, limit: usize) -> Result<Self, InstrumentError> {
        if !matches!(mode, QueryPreparation::Cold) && limit == 0 {
            return Err(InstrumentError::ResourceLimit {
                option: "max_prepared_densities",
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
        left: &CompiledUnitary,
        boundary: DensityBoundary<'_>,
        right: &CompiledUnitary,
        target: DensityBoundary<'_>,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, InstrumentError> {
        if matches!(self.mode, QueryPreparation::Cold) {
            return Ok(left.density_overlap(boundary, right, target, max_terms, arithmetic)?);
        }
        if boundary.event_rows.len() != boundary.outcomes.width()
            || target.event_rows.len() != target.outcomes.width()
        {
            return Err(UnitaryError::DimensionMismatch.into());
        }
        let (plan, _) = self.plan(
            left,
            boundary.layout(),
            right,
            target.layout(),
            max_terms,
            arithmetic,
        )?;
        Ok(plan.evaluate(
            left,
            right,
            boundary.outcomes,
            target.outcomes,
            max_terms,
            arithmetic,
        )?)
    }
    pub(super) fn prepare(
        &self,
        left: &CompiledUnitary,
        boundary: DensityLayout<'_>,
        right: &CompiledUnitary,
        target: DensityLayout<'_>,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<QueryPreparationOutcome, InstrumentError> {
        boundary.validate(left.width())?;
        target.validate(right.width())?;
        if boundary.outputs.len() != target.outputs.len() {
            return Err(UnitaryError::DimensionMismatch.into());
        }
        if matches!(self.mode, QueryPreparation::Cold) {
            return Ok(QueryPreparationOutcome::Cold);
        }
        let (plan, outcome) = self.plan(left, boundary, right, target, max_terms, arithmetic)?;
        super::preparation::validate_resource(plan.fixed_phase_resource(), max_terms)?;
        Ok(outcome)
    }
    fn plan(
        &self,
        left: &CompiledUnitary,
        boundary: DensityLayout<'_>,
        right: &CompiledUnitary,
        target: DensityLayout<'_>,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<(Arc<PreparedDensityOverlap>, QueryPreparationOutcome), InstrumentError> {
        let result = {
            let mut entries = self
                .entries
                .lock()
                .map_err(|_| InstrumentError::PreparedQueryCacheUnavailable)?;
            if let Some(entry) = entries
                .iter()
                .find(|e| e.matches(left, &boundary, right, &target))
            {
                (Arc::clone(&entry.plan), QueryPreparationOutcome::Reused)
            } else {
                if entries.len() >= self.limit {
                    return Err(InstrumentError::ResourceLimit {
                        option: "max_prepared_densities",
                        limit: self.limit,
                        required: entries.len().saturating_add(1),
                    });
                }
                let plan = match self.mode {
                    QueryPreparation::Trace => {
                        left.prepare_density_overlap(boundary, right, target, arithmetic)?
                    }
                    QueryPreparation::FixedFibers(options) => left
                        .prepare_density_overlap_with_fibers(
                            boundary, right, target, max_terms, options, arithmetic,
                        )?,
                    QueryPreparation::Cold => unreachable!("cold density queries returned above"),
                };
                let plan = Arc::new(plan);
                entries.push(Entry {
                    left_outputs: boundary.outputs.to_vec(),
                    right_outputs: target.outputs.to_vec(),
                    left_rows: boundary.event_rows.to_vec(),
                    right_rows: target.event_rows.to_vec(),
                    plan: Arc::clone(&plan),
                });
                (plan, QueryPreparationOutcome::Created)
            }
        };
        Ok(result)
    }
}
