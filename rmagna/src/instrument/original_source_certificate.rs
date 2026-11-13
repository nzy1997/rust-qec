//! Explicit source-resource admission before collected record projection.
use super::*;
use crate::catalogue::PhaseCatalogue;
use crate::phase::PreparedPhaseResource;
use crate::records::CompletedRecordStatistics;
use crate::unitary::expansion::geometry_words;
use crate::unitary::projected::tensor_error;

/// Per-call source admission. Unlike the absorbed certificate, the quotient and
/// selected resource belong to the complete coherent source before any fixed
/// record is projected. A failure never changes the collected branch or selects
/// another resource policy. Geometry excludes existing branch storage, allocator
/// overhead and scalar limbs, which retain their arithmetic limits.
#[derive(Clone, Copy)]
pub struct OriginalSourceCertificateOptions<'a> {
    pub resource: OriginalNetworkResource,
    pub catalogue: Option<&'a PhaseCatalogue>,
    pub max_quotient_rank: usize,
    pub max_terms: usize,
    pub max_raw_path_bits: usize,
    pub max_projection_rows: usize,
    pub max_geometry_words: usize,
    pub completed: CompletedRecordLimits,
}
impl Default for OriginalSourceCertificateOptions<'_> {
    fn default() -> Self {
        Self {
            resource: OriginalNetworkResource::default(),
            catalogue: None,
            max_quotient_rank: 8,
            max_terms: 1024,
            max_raw_path_bits: 256,
            max_projection_rows: 256,
            max_geometry_words: 16 << 20,
            completed: CompletedRecordLimits::default(),
        }
    }
}
/// Actual source selection and resulting projected coherent pair work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalSourceCertificateStatistics {
    pub quotient_rank: usize,
    pub certified_terms: usize,
    pub side_entries: usize,
    pub probability: CompletedRecordStatistics,
}
/// Immutable source resource with owned projected output/environment kets.
/// This borrows only the collected branch; no controller can be invoked again.
pub struct OriginalSourceCertificate<'a> {
    branch: &'a OriginalBranch,
    resource: PreparedPhaseResource,
    states: Vec<AffineKet>,
    probability: ExactScalar,
    statistics: OriginalSourceCertificateStatistics,
}
impl OriginalSourceCertificate<'_> {
    pub fn branch(&self) -> &OriginalBranch {
        self.branch
    }
    pub fn resource(&self) -> &PreparedPhaseResource {
        &self.resource
    }
    pub fn states(&self) -> &[AffineKet] {
        &self.states
    }
    pub fn probability(&self) -> &ExactScalar {
        &self.probability
    }
    pub fn statistics(&self) -> &OriginalSourceCertificateStatistics {
        &self.statistics
    }
}
impl OriginalBranch {
    /// Admit and select a fresh complete source resource using these immutable
    /// options, then project the branch's stored records. No post-projection
    /// absorption, cache reuse or implicit resource fallback can bypass admission.
    /// Previously returned certificates remain usable after any later refusal.
    pub fn compile_before_projection(
        &self,
        options: &OriginalSourceCertificateOptions<'_>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<OriginalSourceCertificate<'_>, OriginalError> {
        let limits = ExpansionLimits {
            max_path_bits: options.max_raw_path_bits,
            max_record_rows: options.max_projection_rows,
            max_geometry_words: options.max_geometry_words,
            max_side_terms: options.max_terms,
            ..Default::default()
        };
        admit(
            "source.max_geometry_words",
            limits.max_geometry_words,
            (self.unitary.width() as u128) * 4 + 128,
        )?;
        admit(
            "source.max_record_rows",
            limits.max_record_rows,
            self.fixed.len() as u128,
        )?;
        // Empty projection retains all physical source slots. Hidden phase axes
        // remain attached until the selected resource has been expanded.
        let keep = (0..self.unitary.width()).collect::<Vec<_>>();
        let tensor = self
            .unitary
            .projected_path_tensor(&[], &keep, limits, arithmetic)?;
        let rank = tensor
            .phase()
            .quotient()
            .map_err(UnitaryError::from)?
            .rank();
        admit(
            "source.max_quotient_rank",
            options.max_quotient_rank,
            rank as u128,
        )?;
        let resource = match options.resource {
            OriginalNetworkResource::Catalogue(policy) => {
                tensor.prepare_resource(options.max_terms, policy, options.catalogue, arithmetic)
            }
            OriginalNetworkResource::Native(policy) => tensor.prepare_native_resource(
                options.max_terms,
                policy,
                options.catalogue,
                arithmetic,
            ),
        }
        .map_err(tensor_error)?
        .ok_or_else(|| invalid("coherent source has zero coefficient before projection", ""))?;
        let count = resource.num_terms();
        admit(
            "source.max_side_terms",
            limits.max_side_terms,
            count as u128,
        )?;
        admit(
            "source.max_source_entries",
            options.completed.max_source_entries,
            count as u128,
        )?;
        admit(
            "source.max_pairs",
            options.completed.max_pairs,
            (count as u128).pow(2),
        )?;
        // Charge both retained raw-source entries and projected output copies.
        let width = self
            .unitary
            .width()
            .checked_add(self.width())
            .ok_or(OriginalError::DimensionMismatch)?;
        let words = geometry_words(
            width,
            tensor.boundary().columns().len(),
            tensor.phase().terms().len(),
            count as u128 + 4,
        );
        admit(
            "source.max_geometry_words",
            limits.max_geometry_words,
            words,
        )?;
        let entries = resource
            .expand(tensor.phase(), tensor.boundary(), arithmetic)
            .map_err(UnitaryError::from)?;
        let mut states = Vec::new();
        for ket in entries {
            let paths = ket.columns().len();
            admit("source.max_path_bits", limits.max_path_bits, paths as u128)?;
            let words = geometry_words(width, paths, self.fixed.len(), 4);
            admit(
                "source.max_geometry_words",
                limits.max_geometry_words,
                words,
            )?;
            if let Some(ket) = self.project_entry(&ket, arithmetic)? {
                states.push(ket);
            }
        }
        let spec = CompletedRecordSpec::new(
            self.width(),
            &[],
            &[],
            &BitVector::zeros(0)?,
            options.completed,
        )?;
        let completed = spec.probability(&states, arithmetic)?;
        let probability = completed.total(arithmetic)?;
        let statistics = OriginalSourceCertificateStatistics {
            quotient_rank: rank,
            certified_terms: count,
            side_entries: states.len(),
            probability: completed.statistics().clone(),
        };
        Ok(OriginalSourceCertificate {
            branch: self,
            resource,
            states,
            probability,
            statistics,
        })
    }
}
