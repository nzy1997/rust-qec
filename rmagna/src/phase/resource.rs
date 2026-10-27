//! One immutable exact resource with explicit catalogue preference and fallback.
use super::{ParityPhase, PhaseError};
use crate::catalogue::{CataloguePreparationOptions, PhaseCatalogue, PreparedCataloguePhase};
use crate::catalogue::{
    FrozenPhasePlan, PhaseInjectionOptions, PhaseSelectionOptions, PhaseSelectionRoute,
};
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
/// Opt-in native selection. Candidate construction and final retained terms have
/// independent bounds; optional injection may reduce a larger initial resource.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativePhaseOptions {
    pub selection: PhaseSelectionOptions,
    pub injection: Option<PhaseInjectionOptions>,
}
/// Counts actual retained routes. An injected resource counts only as injected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeRouteCounts {
    pub direct: usize,
    pub product: usize,
    pub walsh: usize,
    pub injected: usize,
    pub injection_budget_skips: usize,
}
impl NativeRouteCounts {
    pub fn record(&mut self, resource: &PreparedPhaseResource) {
        let Some(plan) = resource.native_resource() else {
            return;
        };
        let injected = plan.injection_receipt().is_some_and(|r| r.selected);
        self.injection_budget_skips += usize::from(
            plan.injection_receipt()
                .is_some_and(|r| r.budget_skip.is_some()),
        );
        if injected {
            self.injected += 1;
            return;
        }
        match plan.receipt().route {
            PhaseSelectionRoute::CertifiedCatalogue => self.direct += 1,
            PhaseSelectionRoute::PositiveTProduct => self.product += 1,
            PhaseSelectionRoute::Walsh => self.walsh += 1,
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) enum ResourcePreparation {
    Catalogue(PhaseResourceOptions),
    Native(NativePhaseOptions),
}
impl ResourcePreparation {
    pub(crate) fn prepare(
        self,
        reference: &ParityPhase,
        max_terms: usize,
        catalogue: Option<&PhaseCatalogue>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<PreparedPhaseResource, PhaseError> {
        match self {
            Self::Catalogue(options) => {
                PreparedPhaseResource::new(reference, max_terms, options, catalogue, arithmetic)
            }
            Self::Native(options) => PreparedPhaseResource::new_native(
                reference, max_terms, options, catalogue, arithmetic,
            ),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseResourceKind {
    CliffordFibers,
    CertifiedCatalogue,
    NativeSelection,
}
impl PhaseResourceKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::CliffordFibers => "clifford_fibers",
            Self::CertifiedCatalogue => "certified_catalogue",
            Self::NativeSelection => "native_selection",
        }
    }
}
enum Decomposition {
    Fibers(PreparedCliffordFibers),
    Catalogue(PreparedCataloguePhase),
    Native(Box<FrozenPhasePlan>),
}
/// Owned scalar/labelled resource selected once for one reference signature.
/// Missing catalogue keys fall back to deterministic fibers. Construction and
/// update errors are returned directly, without attempting another strategy.
pub struct PreparedPhaseResource {
    decomposition: Decomposition,
}
impl PreparedPhaseResource {
    /// Freeze native direct/product/Walsh selection and optional exact injection.
    /// Failure is returned without attempting fibers. No database is implicit.
    pub fn new_native(
        reference: &ParityPhase,
        max_terms: usize,
        mut options: NativePhaseOptions,
        catalogue: Option<&PhaseCatalogue>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, PhaseError> {
        options.selection.preparation.max_terms =
            options.selection.preparation.max_terms.min(max_terms);
        let plan = match options.injection {
            Some(injection) => FrozenPhasePlan::new_with_injection(
                reference,
                catalogue,
                &options.selection,
                &injection,
                arithmetic,
            ),
            None => FrozenPhasePlan::new(reference, catalogue, &options.selection, arithmetic),
        }
        .map_err(PhaseError::Catalogue)?;
        Ok(Self {
            decomposition: Decomposition::Native(Box::new(plan)),
        })
    }
    pub fn native_resource(&self) -> Option<&FrozenPhasePlan> {
        match &self.decomposition {
            Decomposition::Native(plan) => Some(plan),
            _ => None,
        }
    }
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
            Decomposition::Native(_) => PhaseResourceKind::NativeSelection,
        }
    }
    pub fn num_variables(&self) -> usize {
        match &self.decomposition {
            Decomposition::Fibers(resource) => resource.num_variables(),
            Decomposition::Catalogue(resource) => resource.num_variables(),
            Decomposition::Native(resource) => resource.num_variables(),
        }
    }
    pub fn quotient_rank(&self) -> usize {
        match &self.decomposition {
            Decomposition::Fibers(resource) => resource.quotient_rank(),
            Decomposition::Catalogue(resource) => resource.quotient_rank(),
            Decomposition::Native(resource) => resource.quotient_rank(),
        }
    }
    pub fn num_terms(&self) -> usize {
        match &self.decomposition {
            Decomposition::Fibers(resource) => resource.num_terms(),
            Decomposition::Catalogue(resource) => resource.num_terms(),
            Decomposition::Native(resource) => resource.num_terms(),
        }
    }
    /// Return the actual selected fiber resource, never a substitute for an
    /// overlapping catalogue decomposition or an unused fallback candidate.
    pub fn clifford_fibers(&self) -> Option<&PreparedCliffordFibers> {
        match &self.decomposition {
            Decomposition::Fibers(resource) => Some(resource),
            Decomposition::Catalogue(_) | Decomposition::Native(_) => None,
        }
    }
    pub fn catalogue_resource(&self) -> Option<&PreparedCataloguePhase> {
        match &self.decomposition {
            Decomposition::Catalogue(resource) => Some(resource),
            Decomposition::Fibers(_) | Decomposition::Native(_) => None,
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
            Decomposition::Catalogue(_) | Decomposition::Native(_) => {
                Err(PhaseError::ResourceLimit {
                    option: "phase_resource.max_terms",
                    limit: max_terms as u128,
                    required: self.num_terms() as u128,
                })
            }
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
            Decomposition::Native(resource) => {
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
            Decomposition::Native(resource) => resource.expand(updated, boundary, arithmetic),
        }
    }
}
