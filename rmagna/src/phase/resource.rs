//! One immutable exact resource with explicit catalogue preference and fallback.
use super::{ParityPhase, PhaseError};
use crate::catalogue::{CataloguePreparationOptions, PhaseCatalogue, PreparedCataloguePhase};
use crate::fibers::{FiberOptions, PreparedCliffordFibers};
use crate::quadratic::QuadraticForm;
use crate::scalar::{ExactScalar, ScalarArithmetic};
use crate::stabilizer::AffineKet;

#[derive(Clone, Copy, Debug, Default)]
pub struct PhaseResourceOptions {
    pub fibers: FiberOptions,
    /// Applies only to the optional certified catalogue probe and construction.
    pub catalogue: CataloguePreparationOptions,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseResourceKind {
    CliffordFibers,
    CertifiedCatalogue,
}
enum Decomposition {
    Fibers(PreparedCliffordFibers),
    Catalogue(PreparedCataloguePhase),
}
/// Owned scalar/labelled resource selected once for one reference signature.
/// Missing catalogue keys fall back to deterministic fibers. Construction and
/// update errors are returned directly, without attempting another strategy.
pub struct PreparedPhaseResource {
    decomposition: Decomposition,
}
impl PreparedPhaseResource {
    /// An absent or empty catalogue selects fibers directly. A supplied matching
    /// certificate takes priority; its term bound is the smaller of max_terms
    /// and options.catalogue.max_terms. No catalogue is loaded implicitly.
    pub fn new(
        reference: &ParityPhase,
        max_terms: usize,
        options: PhaseResourceOptions,
        catalogue: Option<&PhaseCatalogue>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, PhaseError> {
        if let Some(catalogue) = catalogue {
            let mut limits = options.catalogue;
            limits.max_terms = limits.max_terms.min(max_terms);
            if let Some(resource) =
                PreparedCataloguePhase::new(reference, catalogue, &limits, arithmetic)?
            {
                return Ok(Self {
                    decomposition: Decomposition::Catalogue(resource),
                });
            }
        }
        Ok(Self {
            decomposition: Decomposition::Fibers(PreparedCliffordFibers::new(
                reference,
                max_terms,
                options.fibers,
            )?),
        })
    }
    pub fn kind(&self) -> PhaseResourceKind {
        match &self.decomposition {
            Decomposition::Fibers(_) => PhaseResourceKind::CliffordFibers,
            Decomposition::Catalogue(_) => PhaseResourceKind::CertifiedCatalogue,
        }
    }
    pub fn quotient_rank(&self) -> usize {
        match &self.decomposition {
            Decomposition::Fibers(resource) => resource.quotient_rank(),
            Decomposition::Catalogue(resource) => resource.quotient_rank(),
        }
    }
    pub fn num_terms(&self) -> usize {
        match &self.decomposition {
            Decomposition::Fibers(resource) => resource.num_terms(),
            Decomposition::Catalogue(resource) => resource.num_terms(),
        }
    }
    /// Return the actual selected fiber resource, never a substitute for an
    /// overlapping catalogue decomposition or an unused fallback candidate.
    pub fn clifford_fibers(&self) -> Option<&PreparedCliffordFibers> {
        match &self.decomposition {
            Decomposition::Fibers(resource) => Some(resource),
            Decomposition::Catalogue(_) => None,
        }
    }
    pub fn catalogue_resource(&self) -> Option<&PreparedCataloguePhase> {
        match &self.decomposition {
            Decomposition::Catalogue(resource) => Some(resource),
            Decomposition::Fibers(_) => None,
        }
    }
    /// Recheck a caller's possibly tighter per-query cap without rebuilding or
    /// changing the frozen choice. Zero-valued responses do not bypass this cap.
    pub fn check_term_limit(&self, max_terms: usize) -> Result<(), PhaseError> {
        if self.num_terms() <= max_terms {
            return Ok(());
        }
        match &self.decomposition {
            Decomposition::Fibers(resource) => Err(PhaseError::FiberTermLimit {
                coordinates: resource.enumerated_coordinates(),
                quotient_rank: resource.quotient_rank(),
                limit: max_terms,
            }),
            Decomposition::Catalogue(_) => Err(PhaseError::ResourceLimit {
                option: "phase_resource.max_terms",
                limit: max_terms as u128,
                required: self.num_terms() as u128,
            }),
        }
    }
    pub fn evaluate(
        &self,
        updated: &ParityPhase,
        boundary: &QuadraticForm,
        coefficient: &ExactScalar,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, PhaseError> {
        match &self.decomposition {
            Decomposition::Fibers(resource) => {
                resource.evaluate(updated, boundary, coefficient, arithmetic)
            }
            Decomposition::Catalogue(resource) => {
                resource.evaluate(updated, boundary, coefficient, arithmetic)
            }
        }
    }
    pub fn expand(
        &self,
        updated: &ParityPhase,
        boundary: &AffineKet,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Vec<AffineKet>, PhaseError> {
        match &self.decomposition {
            Decomposition::Fibers(resource) => resource.expand(updated, boundary, arithmetic),
            Decomposition::Catalogue(resource) => resource.expand(updated, boundary, arithmetic),
        }
    }
}
