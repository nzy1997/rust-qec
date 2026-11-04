//! Explicit record-first, radical-absorbed original branch certificates.
use super::*;
use crate::catalogue::PhaseCatalogue;
use crate::phase::{CliffordRadicalAbsorption, PreparedPhaseResource, RadicalAbsorptionOptions};
use crate::records::CompletedRecordStatistics;
use crate::unitary::expansion::geometry_words;
use crate::unitary::projected::tensor_error;

/// Independent budgets for this opt-in certificate. Existing collector storage
/// is borrowed and excluded. Geometry bounds conservative eight-byte logical
/// payload/header/scratch words, excluding allocator overhead and scalar limbs.
/// Resource preparation and absorption retain their own explicit policies and
/// bounds; no error selects a different policy automatically.
#[derive(Clone, Copy)]
pub struct OriginalBranchCertificateOptions<'a> {
    pub resource: OriginalNetworkResource,
    pub catalogue: Option<&'a PhaseCatalogue>,
    pub max_quotient_rank: usize,
    pub max_terms: usize,
    pub max_raw_path_bits: usize,
    pub max_projection_rows: usize,
    pub max_geometry_words: usize,
    pub absorption: RadicalAbsorptionOptions,
    pub completed: CompletedRecordLimits,
}
impl Default for OriginalBranchCertificateOptions<'_> {
    fn default() -> Self {
        Self {
            resource: OriginalNetworkResource::default(),
            catalogue: None,
            max_quotient_rank: 8,
            max_terms: 1024,
            max_raw_path_bits: 4096,
            max_projection_rows: 256,
            max_geometry_words: 16 << 20,
            absorption: RadicalAbsorptionOptions::default(),
            completed: CompletedRecordLimits::default(),
        }
    }
}

/// Actual selected and emitted counts. An explicit zero tensor skips selection;
/// a zero probability from coherent cancellation can retain a resource.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalBranchCertificateStatistics {
    pub quotient_rank: Option<usize>,
    pub certified_terms: usize,
    pub side_entries: usize,
    pub probability: CompletedRecordStatistics,
}

/// Unnormalized coherent states in outputs/chronological-environment order,
/// with the actual post-absorption resource and full ordered-pair probability.
/// Metadata borrows the already collected branch; callbacks cannot be replayed.
/// This is a distinct opt-in interface, not native internal serialization.
pub struct OriginalBranchCertificate<'a> {
    branch: &'a OriginalBranch,
    absorption: CliffordRadicalAbsorption,
    resource: Option<PreparedPhaseResource>,
    states: Vec<AffineKet>,
    probability: ExactScalar,
    statistics: OriginalBranchCertificateStatistics,
}
impl OriginalBranchCertificate<'_> {
    pub fn branch(&self) -> &OriginalBranch {
        self.branch
    }
    pub fn states(&self) -> &[AffineKet] {
        &self.states
    }
    pub fn resource(&self) -> Option<&PreparedPhaseResource> {
        self.resource.as_ref()
    }
    pub fn absorption(&self) -> &CliffordRadicalAbsorption {
        &self.absorption
    }
    pub fn probability(&self) -> &ExactScalar {
        &self.probability
    }
    pub fn statistics(&self) -> &OriginalBranchCertificateStatistics {
        &self.statistics
    }
}
impl OriginalBranch {
    /// Project actual collected records, absorb Clifford radical paths, then
    /// select a branch resource. Stored temporal faults and feedback decisions
    /// are already in this branch. No source reconstruction or callback occurs.
    /// All failures leave this branch and supplied catalogue reusable.
    ///
    /// Existing `expansion`/`probability` continue selecting their source
    /// resource before projection, with their original limits and defaults.
    pub fn compile_after_absorption(
        &self,
        options: &OriginalBranchCertificateOptions<'_>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<OriginalBranchCertificate<'_>, OriginalError> {
        let limits = ExpansionLimits {
            max_path_bits: options.max_raw_path_bits,
            max_record_rows: options.max_projection_rows,
            max_geometry_words: options.max_geometry_words,
            ..ExpansionLimits::default()
        };
        let raw =
            self.unitary
                .projected_path_tensor(&self.fixed, &self.keep, limits, arithmetic)?;
        let absorption = raw
            .absorb_clifford_radical(options.absorption, arithmetic)
            .map_err(tensor_error)?;
        let tensor = absorption.tensor();
        let resource = if tensor.boundary().coefficient().is_zero() {
            None
        } else {
            let rank = tensor
                .phase()
                .quotient()
                .map_err(UnitaryError::from)?
                .rank();
            admit(
                "branch.max_quotient_rank",
                options.max_quotient_rank,
                rank as u128,
            )?;
            match options.resource {
                OriginalNetworkResource::Catalogue(policy) => tensor.prepare_resource(
                    options.max_terms,
                    policy,
                    options.catalogue,
                    arithmetic,
                ),
                OriginalNetworkResource::Native(policy) => tensor.prepare_native_resource(
                    options.max_terms,
                    policy,
                    options.catalogue,
                    arithmetic,
                ),
            }
            .map_err(tensor_error)?
        };
        let count = resource
            .as_ref()
            .map_or(0, PreparedPhaseResource::num_terms);
        // This certificate promises a full coherent probability. Admit its
        // conservative pair count before allocating the retained state vector.
        admit(
            "branch.max_source_entries",
            options.completed.max_source_entries,
            count as u128,
        )?;
        admit(
            "branch.max_pairs",
            options.completed.max_pairs,
            (count as u128).pow(2),
        )?;
        let words = geometry_words(
            self.width(),
            tensor.boundary().columns().len(),
            tensor.phase().terms().len(),
            count as u128 + 4,
        );
        admit(
            "branch.max_geometry_words",
            options.max_geometry_words,
            words,
        )?;
        let states = match &resource {
            Some(resource) => resource
                .expand(tensor.phase(), tensor.boundary(), arithmetic)
                .map_err(UnitaryError::from)?,
            None => vec![],
        };
        let spec = CompletedRecordSpec::new(
            self.width(),
            &[],
            &[],
            &BitVector::zeros(0)?,
            options.completed,
        )?;
        let completed = spec.probability(&states, arithmetic)?;
        let probability = completed.total(arithmetic)?;
        let statistics = OriginalBranchCertificateStatistics {
            quotient_rank: resource.as_ref().map(PreparedPhaseResource::quotient_rank),
            certified_terms: count,
            side_entries: states.len(),
            probability: completed.statistics().clone(),
        };
        Ok(OriginalBranchCertificate {
            branch: self,
            absorption,
            resource,
            states,
            probability,
            statistics,
        })
    }
}
