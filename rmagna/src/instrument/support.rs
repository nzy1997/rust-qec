//! One immutable record probability parent, shared by causal branches.
use super::*;
use crate::fibers::FiberOptions;
use crate::unitary::PreparedProbabilitySupport;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedSupportStatistics {
    pub labels: usize,
    pub quotient_rank: usize,
    pub terms: usize,
}
pub(super) struct SupportQueries {
    options: FiberOptions,
    transport: Option<Vec<BitVector>>,
    coordinates: Option<crate::gf2::LinearFactorization>,
    plan: Mutex<Option<Arc<PreparedProbabilitySupport>>>,
}
impl SupportQueries {
    pub(super) fn new(mode: QueryPreparation) -> Self {
        Self {
            options: match mode {
                QueryPreparation::FixedFibers(options) => options,
                _ => FiberOptions::default(),
            },
            transport: None,
            coordinates: None,
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
        Ok(Self {
            options: self.options,
            transport: Some(transport),
            coordinates: Some(coordinates),
            plan: Mutex::new(None),
        })
    }
    pub(super) fn map_rows(&self, rows: &[BitVector]) -> Result<Vec<BitVector>, InstrumentError> {
        match &self.coordinates {
            None => Ok(rows.to_vec()),
            Some(factor) => rows
                .iter()
                .map(|row| {
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
            PreparedSupportStatistics {
                labels: self.transport.as_ref().map_or(labels, Vec::len),
                quotient_rank: resource.quotient_rank(),
                terms: resource.num_terms(),
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
        let plan = {
            let mut slot = self
                .plan
                .lock()
                .map_err(|_| InstrumentError::PreparedQueryCacheUnavailable)?;
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
                *slot = Some(Arc::new(state.prepare_probability_support(
                    &rows,
                    max_terms,
                    self.options,
                    arithmetic,
                )?));
            }
            Arc::clone(slot.as_ref().expect("prepared support"))
        };
        Ok(plan.evaluate(state, max_terms, arithmetic)?)
    }
}
use crate::stabilizer::AffineKet;
