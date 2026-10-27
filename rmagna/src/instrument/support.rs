//! One immutable record probability parent, shared by causal branches.
use super::*;
use crate::fibers::FiberOptions;
use crate::unitary::PreparedProbabilitySupport;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedSupportStatistics {
    pub labels: usize,
    pub quotient_rank: usize,
    pub terms: usize,
    pub resource_kind: crate::phase::PhaseResourceKind,
    pub phase_variables: usize,
    pub native_routes: crate::phase::NativeRouteCounts,
}
pub(super) struct SupportQueries {
    mode: QueryPreparation,
    transport: Option<Vec<BitVector>>,
    coordinates: Option<crate::gf2::LinearFactorization>,
    captured_coordinates: HashMap<BitVector, BitVector>,
    plan: Mutex<Option<Arc<PreparedProbabilitySupport>>>,
}
impl SupportQueries {
    pub(super) fn new(mode: QueryPreparation) -> Self {
        Self {
            mode,
            transport: None,
            coordinates: None,
            captured_coordinates: HashMap::new(),
            plan: Mutex::new(None),
        }
    }
    pub(super) fn with_transport(
        &self,
        width: usize,
        rows: &[BitVector],
    ) -> Result<Self, InstrumentError> {
        let transport = crate::gf2::LinearFactorization::new(width, rows)?
            .row_space_basis()
            .to_vec();
        let coordinates = crate::gf2::LinearFactorization::new(
            transport.len(),
            &crate::gf2::transpose_columns(width, &transport)?,
        )?;
        let captured_coordinates = rows
            .iter()
            .map(|row| {
                Ok((
                    row.clone(),
                    coordinates
                        .solve(row)?
                        .ok_or(InstrumentError::UncapturedSupportQuery)?,
                ))
            })
            .collect::<Result<_, InstrumentError>>()?;
        Ok(Self {
            mode: self.mode,
            transport: Some(transport),
            coordinates: Some(coordinates),
            captured_coordinates,
            plan: Mutex::new(None),
        })
    }
    pub(super) fn map_rows(&self, rows: &[BitVector]) -> Result<Vec<BitVector>, InstrumentError> {
        match &self.coordinates {
            None => Ok(rows.to_vec()),
            Some(factor) => rows
                .iter()
                .map(|row| {
                    if let Some(mapped) = self.captured_coordinates.get(row) {
                        return Ok(mapped.clone());
                    }
                    factor
                        .solve(row)?
                        .ok_or(InstrumentError::UncapturedSupportQuery)
                })
                .collect(),
        }
    }
    pub(super) fn statistics(
        &self,
        labels: usize,
    ) -> Result<Option<PreparedSupportStatistics>, InstrumentError> {
        let plan = self
            .plan
            .lock()
            .map_err(|_| InstrumentError::PreparedQueryCacheUnavailable)?;
        Ok(plan.as_ref().map(|plan| {
            let resource = plan.fixed_phase_resource();
            let mut native_routes = crate::phase::NativeRouteCounts::default();
            native_routes.record(resource);
            PreparedSupportStatistics {
                labels: self.transport.as_ref().map_or(labels, Vec::len),
                quotient_rank: resource.quotient_rank(),
                terms: resource.num_terms(),
                resource_kind: resource.kind(),
                phase_variables: resource.num_variables(),
                native_routes,
            }
        }))
    }
    pub(super) fn evaluate(
        &self,
        state: &CompiledUnitary,
        records: &[usize],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Vec<AffineKet>, InstrumentError> {
        let (plan, _) = self.plan(state, records, max_terms, arithmetic)?;
        Ok(plan.evaluate(state, max_terms, arithmetic)?)
    }
    pub(super) fn prepare(
        &self,
        state: &CompiledUnitary,
        records: &[usize],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<QueryPreparationOutcome, InstrumentError> {
        if !matches!(self.mode, QueryPreparation::Native(_)) {
            return Ok(QueryPreparationOutcome::Cold);
        }
        let (plan, outcome) = self.plan(state, records, max_terms, arithmetic)?;
        plan.fixed_phase_resource()
            .check_term_limit(max_terms)
            .map_err(UnitaryError::from)?;
        Ok(outcome)
    }
    fn plan(
        &self,
        state: &CompiledUnitary,
        records: &[usize],
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<(Arc<PreparedProbabilitySupport>, QueryPreparationOutcome), InstrumentError> {
        let plan = {
            let mut slot = self
                .plan
                .lock()
                .map_err(|_| InstrumentError::PreparedQueryCacheUnavailable)?;
            let outcome = if slot.is_none() {
                QueryPreparationOutcome::Created
            } else {
                QueryPreparationOutcome::Reused
            };
            if slot.is_none() {
                // Publish only complete successful preparation; a low-budget
                // failure must not poison a later adequately budgeted call.
                let rows = match &self.transport {
                    Some(transport) => transport
                        .iter()
                        .map(|row| {
                            BitVector::from_indices(state.width(), row.ones().map(|j| records[j]))
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    None => records
                        .iter()
                        .map(|&j| BitVector::from_indices(state.width(), [j]))
                        .collect::<Result<Vec<_>, _>>()?,
                };
                let plan = match self.mode {
                    QueryPreparation::Native(options) => state
                        .prepare_probability_support_with_native(
                            &rows,
                            max_terms,
                            options,
                            Some(crate::catalogue::builtin_phase_catalogue()?),
                            arithmetic,
                        )?,
                    QueryPreparation::FixedCatalogue(options) => state
                        .prepare_probability_support_with_resource(
                            &rows,
                            max_terms,
                            options,
                            Some(crate::catalogue::builtin_phase_catalogue()?),
                            arithmetic,
                        )?,
                    other => state.prepare_probability_support(
                        &rows,
                        max_terms,
                        match other {
                            QueryPreparation::FixedFibers(options) => options,
                            _ => FiberOptions::default(),
                        },
                        arithmetic,
                    )?,
                };
                *slot = Some(Arc::new(plan));
            }
            (
                Arc::clone(slot.as_ref().expect("prepared support")),
                outcome,
            )
        };
        Ok(plan)
    }
}
use crate::stabilizer::AffineKet;
