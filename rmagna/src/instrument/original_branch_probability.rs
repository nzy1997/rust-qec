//! Owned single-sided scalar queries for already collected callback branches.
use super::*;
use crate::phase::PreparedPhaseResource;
use crate::unitary::{NetworkValue, PreparedProbability};

/// An owned query on frozen literal decisions, faults and true outcomes. Source
/// instruments, callback handles and the collected branch may all be dropped.
/// This does not construct a coherent side expansion or a pair certificate.
pub struct PreparedOriginalBranchProbability {
    reference: CompiledUnitary,
    query: PreparedProbability,
    outcomes: BitVector,
    records: Vec<OriginalRecord>,
    max_terms: usize,
}
impl PreparedOriginalBranchProbability {
    pub fn records(&self) -> &[OriginalRecord] {
        &self.records
    }
    /// Physical projection outcomes include each original measurement sign;
    /// they are not callback inputs or relabeled reported query coordinates.
    pub fn physical_outcomes(&self) -> &BitVector {
        &self.outcomes
    }
    pub fn compilation(&self) -> &CompilationDiagnostics {
        self.reference.compilation()
    }
    pub fn resource(&self) -> &PreparedPhaseResource {
        self.query
            .fixed_phase_resource()
            .expect("explicit scalar resource preparation")
    }
    pub fn evaluate(&self, a: &ScalarArithmetic) -> Result<NetworkValue, OriginalError> {
        Ok(self
            .query
            .evaluate(&self.reference, &self.outcomes, self.max_terms, a)?)
    }
}
impl OriginalBranch {
    /// Freeze an owned probability query for this already collected branch.
    /// Source construction and coherent lowering have already enforced their
    /// original initial/path/wire/gate budgets. This call admits the actual
    /// doubled scalar geometry and selected resource before copying event rows.
    /// Side-entry and pair budgets are irrelevant; no callback can be recollected.
    /// The fixed resource is selected only after physical legs are traced, with
    /// no implicit catalogue, resource fallback or private cache identity claim.
    pub fn prepare_scalar_probability(
        &self,
        options: &OriginalNetworkOptions<'_>,
        a: &ScalarArithmetic,
    ) -> Result<PreparedOriginalBranchProbability, OriginalError> {
        let events = self.fixed.len();
        if events != self.records.len() {
            return Err(OriginalError::DimensionMismatch);
        }
        admit(
            "max_event_rows",
            options
                .limits
                .max_event_rows
                .min(self.limits.expansion.max_event_rows)
                .min(self.limits.completed.max_event_rows),
            events as u128,
        )?;
        let bytes = self
            .records
            .iter()
            .map(|r| 96 + r.name.len() as u128)
            .sum::<u128>();
        admit("max_metadata_bytes", self.limits.max_metadata_bytes, bytes)?;
        // The shared helper is extracted from the existing static-network
        // admission without changing its source-specific budget clamps.
        super::trace::admit_owned_geometry(&self.unitary, events, events as u128, options)?;
        let reference = self.unitary.with_insertions(&[], a)?;
        let rows = self
            .fixed
            .iter()
            .map(|&(slot, _)| BitVector::from_indices(reference.width(), [slot]))
            .collect::<Result<Vec<_>, _>>()?;
        let outcomes = BitVector::from_indices(
            events,
            self.fixed
                .iter()
                .enumerate()
                .filter_map(|(j, &(_, value))| value.then_some(j)),
        )?;
        let query = reference.prepare_probability(&rows, a)?;
        let query = match options.resource {
            OriginalNetworkResource::Catalogue(policy) => {
                query.with_phase_resource(options.max_terms, policy, options.catalogue, a)?
            }
            OriginalNetworkResource::Native(policy) => {
                query.with_native_phase_resource(options.max_terms, policy, options.catalogue, a)?
            }
        };
        Ok(PreparedOriginalBranchProbability {
            reference,
            query,
            outcomes,
            records: self.records.clone(),
            max_terms: options.max_terms,
        })
    }
}
