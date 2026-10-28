//! Explicit native-coordinate resource selection, separate from runtime defaults.
use super::*;
use crate::phase::{CanonicalPhaseCore, CliffordQuotient, PhaseError};
use crate::quadratic::QuadraticForm;
use crate::stabilizer::AffineKet;

/// The route selected once when a frozen plan is constructed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseSelectionRoute {
    CertifiedCatalogue,
    PositiveTProduct,
    Walsh,
}
impl PhaseSelectionRoute {
    pub fn name(self) -> &'static str {
        match self {
            Self::CertifiedCatalogue => "catalogue",
            Self::PositiveTProduct => "positive_t_product",
            Self::Walsh => "walsh",
        }
    }
}

/// Independent logical construction bounds. These exclude allocator overhead
/// and do not reproduce the native implementation's in-memory layout.
#[derive(Clone, Copy, Debug)]
pub struct PhaseSelectionOptions {
    pub max_quotient_rank: usize,
    /// Four logical native bytes per core assignment, checked before allocation.
    pub max_core_table_bytes: usize,
    /// Upper bound on signature scanning word work, documented on the receipt.
    pub max_signature_work: u128,
    pub max_transform_work: u128,
    /// Applies separately to the selected certificate, including direct entries.
    /// Direct certificates are reverified without cloning their indicator data.
    pub construction: CatalogueLimits,
    pub preparation: CataloguePreparationOptions,
}
impl Default for PhaseSelectionOptions {
    fn default() -> Self {
        Self {
            max_quotient_rank: 8,
            max_core_table_bytes: 64 << 20,
            max_signature_work: 64 << 20,
            max_transform_work: 64 << 20,
            construction: CatalogueLimits::default(),
            preparation: CataloguePreparationOptions::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhaseSelectionReceipt {
    pub route: PhaseSelectionRoute,
    pub quotient_rank: usize,
    pub table_values: usize,
    pub logical_core_table_bytes: usize,
    /// Retained certified indicators, including supplied cancelling indicators.
    pub retained_terms: usize,
    /// `n*(n+1)/2 * (input_terms + 2*n + 3) * ceil(n/64)`.
    /// This bounds a complete lexicographic scan, including highest-bit reduction.
    pub signature_work_bound: u128,
    pub core_construction_work: u128,
    /// Zero for catalogue/product routes; rank * table_values for Walsh.
    pub transform_work: u128,
    pub certification: CatalogueUsage,
    /// Maximum of the bounded coordinate workspace and retained plan geometry.
    /// Retained geometry includes the separately owned original signature axes.
    pub geometry_words: usize,
}

/// One owned certificate and prepared transport in fixed native coordinates.
/// Clifford updates reuse this selection; a changed signature is an error.
pub struct FrozenPhasePlan {
    axes: Vec<BitVector>,
    certificate: Arc<CertifiedPhaseDecomposition>,
    prepared: Arc<PreparedCataloguePhase>,
    receipt: PhaseSelectionReceipt,
    injection: Option<PhaseInjectionReceipt>,
}
impl fmt::Debug for FrozenPhasePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrozenPhasePlan")
            .field("receipt", &self.receipt)
            .finish_non_exhaustive()
    }
}

fn checked(value: Option<u128>, field: &'static str) -> Result<u128, CatalogueError> {
    value.ok_or_else(|| error(field, "required construction bound exceeds u128"))
}
fn phase_error(field: &'static str, cause: PhaseError) -> CatalogueError {
    let field = match &cause {
        PhaseError::CoreLimit { option, .. } | PhaseError::ResourceLimit { option, .. } => *option,
        _ => field,
    };
    error(field, cause)
}
fn selected<T>(
    result: Result<T, CatalogueError>,
    route: PhaseSelectionRoute,
) -> Result<T, CatalogueError> {
    result.map_err(|e| route_error(e, route))
}
fn route_error(mut cause: CatalogueError, route: PhaseSelectionRoute) -> CatalogueError {
    cause.message = format!("selected {} route: {}", route.name(), cause.message);
    cause
}
fn geometry(required: u128, options: &PhaseSelectionOptions) -> Result<(), CatalogueError> {
    limit(
        "phase_selection.max_geometry_words",
        required,
        options.preparation.max_geometry_words as u128,
    )
}
fn highest(row: &BitVector) -> Option<usize> {
    let word = row.words().iter().rposition(|&value| value != 0)?;
    Some(64 * word + 63 - row.words()[word].leading_zeros() as usize)
}
fn native_axes(
    reference: &ParityPhase,
    options: &PhaseSelectionOptions,
) -> Result<(Vec<BitVector>, u128, u128), CatalogueError> {
    let n = reference.num_variables() as u128;
    let words = n.div_ceil(64);
    let scan_geometry = checked(
        checked(n.checked_mul(2), "phase_selection.geometry")?
            .checked_add(1)
            .and_then(|rows| rows.checked_mul(words)),
        "phase_selection.geometry",
    )?;
    geometry(scan_geometry, options)?;
    let pairs = checked(n.checked_mul(n + 1), "phase_selection.signature_work")? / 2;
    let per_pair = checked(
        checked(n.checked_mul(2), "phase_selection.signature_work")?
            .checked_add(reference.terms().len() as u128)
            .and_then(|work| work.checked_add(3)),
        "phase_selection.signature_work",
    )?;
    let work = checked(
        pairs
            .checked_mul(per_pair)
            .and_then(|work| work.checked_mul(words)),
        "phase_selection.signature_work",
    )?;
    limit(
        "phase_selection.max_signature_work",
        work,
        options.max_signature_work,
    )?;
    let mut pivots = BTreeMap::<usize, BitVector>::new();
    let mut axes = Vec::new();
    'rows: for i in 0..reference.num_variables() {
        for j in i..reference.num_variables() {
            let original = reference
                .signature_slice(i, j)
                .map_err(|e| phase_error("phase_selection.signature", e))?;
            let mut reduced = original.clone();
            while let Some(pivot) = highest(&reduced) {
                if let Some(row) = pivots.get(&pivot) {
                    reduced
                        .xor_assign(row)
                        .map_err(|e| error("phase_selection.signature", e))?;
                } else {
                    limit(
                        "phase_selection.max_quotient_rank",
                        axes.len() as u128 + 1,
                        options.max_quotient_rank as u128,
                    )?;
                    pivots.insert(pivot, reduced);
                    axes.push(original);
                    break;
                }
            }
            if axes.len() == reference.num_variables() {
                break 'rows;
            }
        }
    }
    Ok((axes, work, scan_geometry))
}
fn factors_present(catalogue: &PhaseCatalogue, rank: usize) -> bool {
    (0..rank).step_by(2).all(|offset| {
        let width = (rank - offset).min(2);
        let table: &[u8] = if width == 1 { &[0, 1] } else { &[0, 1, 1, 2] };
        catalogue.get(width, table).is_some()
    })
}
impl FrozenPhasePlan {
    /// Select a direct complete-table certificate first, then consecutive
    /// one-/two-T factors for exactly the positive-popcount core, then Walsh.
    /// `None` means no database, whereas `Some(empty)` still permits the rank-zero
    /// product unit. Missing factors choose Walsh before construction. Any error
    /// on a supported selected route is terminal and cannot trigger fallback.
    ///
    /// Coordinates retain the first independent original trilinear rows in
    /// lexicographic `(i,j)` order, using highest-bit independence elimination.
    /// The section solves their unit right-hand sides with all free bits zero.
    /// Existing quotient and runtime defaults are unaffected. Geometry admission
    /// covers signature rows, a conservative factorization workspace bound, and
    /// the retained prepared supports; logical words exclude allocator overhead.
    pub fn new(
        reference: &ParityPhase,
        catalogue: Option<&PhaseCatalogue>,
        options: &PhaseSelectionOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, CatalogueError> {
        Self::select(reference, catalogue, options, None, arithmetic)
    }
    /// Attempt bounded optional T-pair injection before applying final transport
    /// limits. Construction storage is bounded by `options.construction`;
    /// `options.preparation.max_terms` applies to the final selected certificate.
    /// Only explicit candidate budgets permit a skip. Arithmetic and certificate
    /// failures remain errors. Existing plans and catalogues remain immutable.
    pub fn new_with_injection(
        reference: &ParityPhase,
        catalogue: Option<&PhaseCatalogue>,
        options: &PhaseSelectionOptions,
        injection: &PhaseInjectionOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, CatalogueError> {
        Self::select(reference, catalogue, options, Some(injection), arithmetic)
    }
    fn select(
        reference: &ParityPhase,
        catalogue: Option<&PhaseCatalogue>,
        options: &PhaseSelectionOptions,
        injection: Option<&PhaseInjectionOptions>,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, CatalogueError> {
        let (axes, signature_work_bound, scan_geometry) = native_axes(reference, options)?;
        let rank = axes.len();
        let values = u32::try_from(rank)
            .ok()
            .and_then(|rank| 1_u128.checked_shl(rank))
            .ok_or_else(|| {
                error(
                    "phase_selection.core_table_capacity",
                    "core rank exceeds the table index range",
                )
            })?;
        let bytes = checked(values.checked_mul(4), "phase_selection.core_table_capacity")?;
        limit(
            "phase_selection.max_core_table_bytes",
            bytes,
            options.max_core_table_bytes as u128,
        )?;
        vector_capacity::<u8>("phase_selection.core_table_capacity", values)?;
        let n = reference.num_variables() as u128;
        let words = n.div_ceil(64);
        let r = rank as u128;
        // Full basis, four rank-row workspaces, RHS provenance and linear data.
        let coordinate_geometry = checked(
            checked(n.checked_add(4 * r), "phase_selection.geometry")?
                .checked_mul(words)
                .and_then(|bound| bound.checked_add(r * r.div_ceil(64)))
                .and_then(|bound| bound.checked_add(n.div_ceil(8))),
            "phase_selection.geometry",
        )?
        .max(scan_geometry);
        geometry(coordinate_geometry, options)?;
        let quotient = CliffordQuotient::from_dual_axes(reference.num_variables(), &axes)
            .map_err(|e| phase_error("phase_selection.coordinates", e))?;
        let resource = reference
            .restrict(
                &BitVector::zeros(reference.num_variables())
                    .map_err(|e| error("phase_selection.coordinates", e))?,
                quotient.complement(),
            )
            .map_err(|e| phase_error("phase_selection.restriction", e))?;
        let core = CanonicalPhaseCore::new(&resource, &options.preparation.core)
            .map_err(|e| phase_error("phase_selection.core", e))?;
        let walsh = || {
            PhaseCatalogue::from_walsh_table(
                rank,
                core.phase_table(),
                options.max_transform_work,
                &options.construction,
                arithmetic,
            )
        };
        let direct =
            catalogue.and_then(|catalogue| catalogue.shared_entry(rank, core.phase_table()));
        let positive = core
            .phase_table()
            .iter()
            .enumerate()
            .all(|(word, &phase)| phase == (word.count_ones() & 7) as u8);
        let (route, selected_catalogue) = if let Some(entry) = direct {
            (
                PhaseSelectionRoute::CertifiedCatalogue,
                selected(
                    PhaseCatalogue::from_shared(entry, &options.construction, arithmetic),
                    PhaseSelectionRoute::CertifiedCatalogue,
                )?,
            )
        } else if let Some(catalogue) =
            catalogue.filter(|catalogue| positive && factors_present(catalogue, rank))
        {
            let product = selected(
                catalogue.paired_t_product(rank, &options.construction, arithmetic),
                PhaseSelectionRoute::PositiveTProduct,
            )?
            .ok_or_else(|| {
                error(
                    "phase_selection.product",
                    "selected product factors disappeared",
                )
            })?;
            (PhaseSelectionRoute::PositiveTProduct, product)
        } else {
            (
                PhaseSelectionRoute::Walsh,
                selected(walsh(), PhaseSelectionRoute::Walsh)?,
            )
        };
        let mut certificate = selected_catalogue
            .shared_entry(rank, core.phase_table())
            .ok_or_else(|| {
                error(
                    "phase_selection.certificate",
                    "selected constructor did not return the requested core",
                )
            })?;
        let mut certification = selected_catalogue.usage();
        let injection = if let Some(injection) = injection {
            let (candidate, receipt) = super::injection::attempt(
                &certificate,
                catalogue,
                route,
                injection,
                &options.construction,
                arithmetic,
            )?;
            if let Some(candidate) = candidate {
                certificate = candidate
                    .shared_entry(rank, core.phase_table())
                    .ok_or_else(|| {
                        error(
                            "phase_injection.certificate",
                            "candidate lost its certified core",
                        )
                    })?;
                certification = candidate.usage();
            }
            Some(receipt)
        } else {
            None
        };
        let core_construction_work = core.work_units();
        let transform_work = if route == PhaseSelectionRoute::Walsh {
            rank as u128 * values
        } else {
            0
        };
        let extra_geometry = r * words;
        geometry(extra_geometry, options)?;
        let prepared = PreparedCataloguePhase::from_core(
            quotient,
            resource,
            core,
            &certificate,
            &options.preparation,
            arithmetic,
            extra_geometry as usize,
        )
        .map_err(|e| route_error(phase_error("phase_selection.preparation", e), route))?;
        let geometry_words = coordinate_geometry.max(prepared.geometry_words() as u128) as usize;
        let receipt = PhaseSelectionReceipt {
            route,
            quotient_rank: rank,
            table_values: values as usize,
            logical_core_table_bytes: bytes as usize,
            retained_terms: prepared.num_terms(),
            signature_work_bound,
            core_construction_work,
            transform_work,
            certification,
            geometry_words,
        };
        Ok(Self {
            axes,
            certificate,
            prepared: Arc::new(prepared),
            receipt,
            injection,
        })
    }
    pub fn receipt(&self) -> &PhaseSelectionReceipt {
        &self.receipt
    }
    /// Candidate diagnostics; `receipt().route` records the original selection.
    pub fn injection_receipt(&self) -> Option<&PhaseInjectionReceipt> {
        self.injection.as_ref()
    }
    pub fn signature_axes(&self) -> &[BitVector] {
        &self.axes
    }
    pub fn section_lifts(&self) -> &[BitVector] {
        self.prepared.quotient().complement()
    }
    pub fn certificate(&self) -> &CertifiedPhaseDecomposition {
        &self.certificate
    }
    pub fn num_variables(&self) -> usize {
        self.prepared.num_variables()
    }
    pub fn quotient_rank(&self) -> usize {
        self.receipt.quotient_rank
    }
    pub fn num_terms(&self) -> usize {
        self.receipt.retained_terms
    }
    pub fn evaluate(
        &self,
        updated: &ParityPhase,
        boundary: &QuadraticForm,
        coefficient: &ExactScalar,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, PhaseError> {
        self.prepared
            .evaluate(updated, boundary, coefficient, arithmetic)
    }
    pub fn expand(
        &self,
        updated: &ParityPhase,
        boundary: &AffineKet,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Vec<AffineKet>, PhaseError> {
        self.prepared.expand(updated, boundary, arithmetic)
    }

    pub(crate) fn admit_symbolic_boundary(
        &self,
        reference: &ParityPhase,
        width: usize,
        options: &super::CatalogueBoundaryOptions,
    ) -> Result<super::CatalogueBoundaryUsage, PhaseError> {
        self.prepared
            .admit_symbolic_boundary(reference, width, options)
    }

    /// Freeze a separately bounded labelled transport for this selected resource.
    /// The returned plan shares immutable owned data and does not reselect a core.
    pub fn prepare_boundary(
        &self,
        reference: &ParityPhase,
        boundary: &AffineKet,
        options: &super::CatalogueBoundaryOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<super::PreparedCatalogueBoundary, PhaseError> {
        self.prepared
            .prepare_boundary(reference, boundary, options, arithmetic)
    }
}
