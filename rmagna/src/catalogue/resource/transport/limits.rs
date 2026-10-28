use super::*;

#[derive(Clone, Copy, Debug)]
pub struct CatalogueBoundaryOptions {
    pub max_terms: usize,
    /// Conservative logical geometry, including the retained selected resource,
    /// indices (one word each), packed vectors and rounded quadratic linear bytes.
    /// Includes a temporary-workspace allowance; excludes scalar limbs and
    /// allocator overhead. This is not a process-memory bound.
    pub max_geometry_words: usize,
    /// Conservative construction operation estimate. Checked before grouping,
    /// cloning geometry, quadratic pullbacks or Gaussian elimination.
    pub max_construction_work: u128,
}
impl Default for CatalogueBoundaryOptions {
    fn default() -> Self {
        Self {
            max_terms: 256,
            max_geometry_words: 16 << 20,
            max_construction_work: 1 << 40,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatalogueBoundaryUsage {
    pub terms: usize,
    pub consistent_terms: usize,
    pub shared_maps: usize,
    /// Admitted conservative estimate, before map sharing reduces storage.
    pub geometry_words: usize,
    pub construction_work: u128,
}

fn checked(value: Option<u128>, field: &'static str) -> Result<u128, PhaseError> {
    value.ok_or_else(|| {
        PhaseError::Catalogue(crate::catalogue::error(
            field,
            "conservative construction bound exceeds u128",
        ))
    })
}
fn admit(field: &'static str, required: u128, limit: u128) -> Result<(), PhaseError> {
    if required > limit {
        Err(PhaseError::ResourceLimit {
            option: field,
            required,
            limit,
        })
    } else {
        Ok(())
    }
}
pub(super) fn admission(
    resource: &PreparedCataloguePhase,
    reference: &ParityPhase,
    width: usize,
    options: &CatalogueBoundaryOptions,
    include_symbolic_input: bool,
) -> Result<CatalogueBoundaryUsage, PhaseError> {
    let terms = resource.num_terms();
    admit(
        "catalogue_boundary.max_terms",
        terms as u128,
        options.max_terms as u128,
    )?;
    let n = resource.num_variables() as u128;
    let b = width as u128;
    let t = terms as u128;
    let nw = n.div_ceil(64);
    let bw = b.div_ceil(64);
    let linear = n.div_ceil(8);
    // Per-term allowance covers the full prepared reference, retained basis,
    // elimination actions (including dense index lists), map keys/directions,
    // label images, fixed quadratics and construction workspaces. Charge every
    // selected term before inspecting zero/inconsistent supports or sharing.
    let per_term = checked(
        8_u128
            .checked_mul(n + 1)
            .and_then(|v| v.checked_mul(n + 4 * nw + 2 * bw + linear + 8)),
        "catalogue_boundary.max_geometry_words",
    )?;
    // Affine contraction factors a transpose with b rows, even for n=0.
    // Charge input/reduced/original rows, dense-or-compressed provenance and
    // origins indices separately; packed label-image words alone are too small.
    let row_geometry = checked(
        b.checked_mul(4 * nw + 8).and_then(|v| v.checked_mul(t + 1)),
        "catalogue_boundary.max_geometry_words",
    )?;
    let symbolic_geometry = if include_symbolic_input {
        // Affine/scalar headers, rounded linear bytes, vector headers and
        // packed quadratic/label buffers, including collection workspace.
        checked(
            2_u128
                .checked_mul(n)
                .and_then(|v| v.checked_mul(nw + bw + 8))
                .and_then(|v| v.checked_add(64 + 4 * bw + linear)),
            "catalogue_boundary.max_geometry_words",
        )?
    } else {
        0
    };
    let geometry = checked(
        per_term
            .checked_mul(t + 1)
            .and_then(|v| v.checked_add(row_geometry))
            .and_then(|v| v.checked_add(resource.geometry_words() as u128))
            .and_then(|v| v.checked_add(symbolic_geometry)),
        "catalogue_boundary.max_geometry_words",
    )?;
    admit(
        "catalogue_boundary.max_geometry_words",
        geometry,
        options.max_geometry_words as u128,
    )?;
    // Packed quadratic restrictions/elimination, parity conversion, and a
    // conservative quadratic bound on map comparisons are charged separately.
    let cube = checked(
        (n + 1).checked_pow(3),
        "catalogue_boundary.max_construction_work",
    )?;
    let matrix = checked(
        cube.checked_mul(t + 1)
            .and_then(|v| v.checked_mul(nw + bw + 1)),
        "catalogue_boundary.max_construction_work",
    )?;
    let parities = checked(
        (reference.terms().len() as u128 + 1)
            .checked_mul(n + 1)
            .and_then(|v| v.checked_mul(n + 1))
            .and_then(|v| v.checked_mul(nw + 1)),
        "catalogue_boundary.max_construction_work",
    )?;
    let grouping = checked(
        t.checked_mul(t)
            .and_then(|v| v.checked_mul(n + 1))
            .and_then(|v| v.checked_mul(nw + 1)),
        "catalogue_boundary.max_construction_work",
    )?;
    // Tall transpose rows and their index/provenance construction remain real
    // work when there are no path coordinates to eliminate.
    let row_work = checked(
        (b + 1)
            .checked_mul(n + 1)
            .and_then(|v| v.checked_mul(nw + 1))
            .and_then(|v| v.checked_mul(t + 1)),
        "catalogue_boundary.max_construction_work",
    )?;
    let symbolic_work = if include_symbolic_input {
        // Direct packed prefixes and the quadratic clone, without elimination
        // or scalar arithmetic. This is logical work, not elapsed time.
        checked(
            512_u128
                .checked_mul(n + 1)
                .and_then(|v| v.checked_mul(nw + bw + 1)),
            "catalogue_boundary.max_construction_work",
        )?
    } else {
        0
    };
    let work = checked(
        matrix
            .checked_add(parities)
            .and_then(|v| v.checked_add(grouping))
            .and_then(|v| v.checked_add(row_work))
            .and_then(|v| v.checked_mul(64))
            .and_then(|v| v.checked_add(symbolic_work)),
        "catalogue_boundary.max_construction_work",
    )?;
    admit(
        "catalogue_boundary.max_construction_work",
        work,
        options.max_construction_work,
    )?;
    Ok(CatalogueBoundaryUsage {
        terms,
        consistent_terms: 0,
        shared_maps: 0,
        geometry_words: geometry as usize,
        construction_work: work,
    })
}
