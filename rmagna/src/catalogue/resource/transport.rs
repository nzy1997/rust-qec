//! Shared labelled geometry and immutable Gaussian transport of fixed indicators.
use super::*;
use crate::stabilizer::{PreparedAffineContraction, StabilizerError};
use std::{collections::BTreeMap, sync::Arc};
mod limits;
pub use limits::{CatalogueBoundaryOptions, CatalogueBoundaryUsage};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CatalogueBoundaryStatistics {
    /// Actual changing quadratic restrictions, one per shared direction map.
    /// The single original-to-quotient boundary conversion is not counted here.
    pub map_pullbacks: usize,
    pub shifted_terms: usize,
    pub gaussian_reuses: usize,
    pub gaussian_recomputations: usize,
    pub discarded_terms: usize,
}
#[derive(Debug)]
pub struct CatalogueBoundaryExpansion {
    pub states: Vec<AffineKet>,
    pub statistics: CatalogueBoundaryStatistics,
}
struct BoundaryTerm {
    resource_index: usize,
    shift: BitVector,
    fixed_offset: BitVector,
    phase: QuadraticForm,
    global: u8,
    contraction: PreparedAffineContraction,
}
struct Map {
    directions: Vec<BitVector>,
    columns: Vec<BitVector>,
    terms: Vec<BoundaryTerm>,
}
struct BuildingMap {
    map: Map,
    reference_pullback: QuadraticForm,
    reference_images: Vec<BitVector>,
}
/// Owned selected catalogue resource and frozen original label columns.
///
/// Distinct shifts with identical direction maps share the changing quadratic
/// pullback. Each consistent term retains its own fixed quadratic and Gaussian
/// contraction. Structurally compatible updates replay recorded steps; other
/// Clifford quadratics use exact elimination in the retained pivot/kernel basis.
/// Results preserve complex unnormalized amplitudes, original dependent labels,
/// and overlapping terms. No selection or mutable query cache is introduced.
pub struct PreparedCatalogueBoundary {
    resource: Arc<PreparedCataloguePhase>,
    width: usize,
    columns: Vec<BitVector>,
    reference_quadratic: QuadraticForm,
    maps: Vec<Map>,
    usage: CatalogueBoundaryUsage,
}

fn difference(a: &QuadraticForm, b: &QuadraticForm) -> Result<QuadraticForm, PhaseError> {
    let upper = a
        .upper()
        .iter()
        .zip(b.upper())
        .map(|(a, b)| {
            let mut row = a.clone();
            row.xor_assign(b)?;
            Ok(row)
        })
        .collect::<Result<Vec<_>, PhaseError>>()?;
    Ok(QuadraticForm::new(
        (a.constant() + 4 - b.constant()) & 3,
        a.linear()
            .iter()
            .zip(b.linear())
            .map(|(a, b)| (a + 4 - b) & 3)
            .collect(),
        upper,
    )?)
}
// q(s+D*x) = q(s) + q(D*x)-q(0) + 2*B(s,D*x), where B
// includes the odd linear diagonal. Images B*D come from one shared pullback.
fn shifted(
    original: &QuadraticForm,
    restricted: &QuadraticForm,
    images: &[BitVector],
    shift: &BitVector,
) -> Result<QuadraticForm, PhaseError> {
    let linear = restricted
        .linear()
        .iter()
        .zip(images)
        .map(|(&c, image)| Ok((c + 2 * u8::from(image.dot(shift)?)) & 3))
        .collect::<Result<Vec<_>, PhaseError>>()?;
    Ok(QuadraticForm::new(
        original.evaluate(shift)?,
        linear,
        restricted.upper().to_vec(),
    )?)
}
impl PreparedCataloguePhase {
    /// Freeze labelled transport while sharing this immutable selected resource.
    /// Construction limits are independent of resource selection/preparation.
    pub fn prepare_boundary(
        self: &Arc<Self>,
        reference: &ParityPhase,
        boundary: &AffineKet,
        options: &CatalogueBoundaryOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<PreparedCatalogueBoundary, PhaseError> {
        PreparedCatalogueBoundary::new(self, reference, boundary, options, arithmetic)
    }
}
impl PreparedCatalogueBoundary {
    pub fn new(
        resource: &Arc<PreparedCataloguePhase>,
        reference: &ParityPhase,
        boundary: &AffineKet,
        options: &CatalogueBoundaryOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, PhaseError> {
        let n = resource.num_variables();
        if reference.num_variables() != n || boundary.columns().len() != n {
            return Err(PhaseError::DimensionMismatch);
        }
        let mut usage = limits::admission(resource, reference, boundary.width(), options)?;
        arithmetic.multiply(boundary.coefficient(), &ExactScalar::one())?;
        for term in &resource.terms {
            arithmetic.multiply(&term.coefficient, &ExactScalar::one())?;
        }
        let (_, quadratic) = resource.updated_boundary(reference, boundary.phase())?;
        let zero = BitVector::zeros(n)?;
        let mut lookup = BTreeMap::new();
        let mut builders: Vec<BuildingMap> = Vec::new();
        for (resource_index, term) in resource.terms.iter().enumerate() {
            let Some(support) = &term.support else {
                continue;
            };
            let key = support
                .directions
                .iter()
                .map(|v| v.words().to_vec())
                .collect::<Vec<_>>();
            let index = if let Some(&index) = lookup.get(&key) {
                index
            } else {
                let (reference_pullback, reference_images) =
                    quadratic.restrict_with_images(&zero, &support.directions)?;
                let columns = support
                    .original_directions
                    .iter()
                    .map(|v| image(boundary.columns(), v, boundary.width()))
                    .collect::<Result<Vec<_>, _>>()?;
                let index = builders.len();
                builders.push(BuildingMap {
                    map: Map {
                        directions: support.directions.clone(),
                        columns,
                        terms: Vec::new(),
                    },
                    reference_pullback,
                    reference_images,
                });
                lookup.insert(key, index);
                index
            };
            let builder = &mut builders[index];
            let phase = add_quadratic(
                &shifted(
                    &quadratic,
                    &builder.reference_pullback,
                    &builder.reference_images,
                    &support.shift,
                )?,
                &support
                    .phase
                    .restrict(&support.shift, &support.directions)?,
            )?;
            let fixed_offset = image(
                boundary.columns(),
                &support.original_shift,
                boundary.width(),
            )?;
            let mut offset = boundary.offset().clone();
            offset.xor_assign(&fixed_offset)?;
            // Record every structural step even when the supplied reference has
            // zero coefficient or a cancelling hidden-path Gaussian sum.
            let reference = AffineKet::new(
                offset,
                builder.map.columns.clone(),
                phase.clone(),
                ExactScalar::one(),
            )?;
            let contraction = PreparedAffineContraction::new(&reference)?;
            builder.map.terms.push(BoundaryTerm {
                resource_index,
                shift: support.shift.clone(),
                fixed_offset,
                phase,
                global: support.global,
                contraction,
            });
            usage.consistent_terms += 1;
        }
        usage.shared_maps = builders.len();
        Ok(Self {
            resource: Arc::clone(resource),
            width: boundary.width(),
            columns: boundary.columns().to_vec(),
            reference_quadratic: quadratic,
            maps: builders.into_iter().map(|b| b.map).collect(),
            usage,
        })
    }
    pub fn usage(&self) -> CatalogueBoundaryUsage {
        self.usage
    }
    pub fn num_variables(&self) -> usize {
        self.resource.num_variables()
    }
    pub fn width(&self) -> usize {
        self.width
    }
    /// Expand a compatible phase on exactly the frozen original label columns.
    /// Offsets, coefficients and all Clifford quadratic values may change.
    /// Shape/signature validation precedes the zero shortcut. Statistics belong
    /// only to this call; failed or concurrent calls cannot mutate the plan.
    pub fn expand(
        &self,
        updated: &ParityPhase,
        boundary: &AffineKet,
        arithmetic: &ScalarArithmetic,
    ) -> Result<CatalogueBoundaryExpansion, PhaseError> {
        if boundary.width() != self.width || boundary.columns() != self.columns {
            return Err(StabilizerError::IncompatibleBoundaryMap.into());
        }
        let (global, quadratic) = self.resource.updated_boundary(updated, boundary.phase())?;
        arithmetic.multiply(boundary.coefficient(), &ExactScalar::one())?;
        let mut statistics = CatalogueBoundaryStatistics::default();
        let mut states = Vec::new();
        if boundary.coefficient().is_zero() {
            return Ok(CatalogueBoundaryExpansion { states, statistics });
        }
        let delta = difference(&quadratic, &self.reference_quadratic)?;
        let zero = BitVector::zeros(self.num_variables())?;
        for map in &self.maps {
            let (restricted, images) = delta.restrict_with_images(&zero, &map.directions)?;
            statistics.map_pullbacks += 1;
            for term in &map.terms {
                let phase = add_quadratic(
                    &term.phase,
                    &shifted(&delta, &restricted, &images, &term.shift)?,
                )?;
                let mut offset = boundary.offset().clone();
                offset.xor_assign(&term.fixed_offset)?;
                let factor = arithmetic.multiply(
                    &self.resource.terms[term.resource_index].coefficient,
                    &arithmetic.omega(i64::from((global + term.global) & 7), 0)?,
                )?;
                let coefficient = arithmetic.multiply(boundary.coefficient(), &factor)?;
                let input = AffineKet::new(offset, map.columns.clone(), phase, coefficient)?;
                statistics.shifted_terms += 1;
                let result = match term.contraction.contract(&input, arithmetic) {
                    Err(StabilizerError::IncompatibleContraction) => {
                        statistics.gaussian_recomputations += 1;
                        term.contraction
                            .contract_with_phase_updates(&input, arithmetic)?
                    }
                    result => {
                        statistics.gaussian_reuses += 1;
                        result?
                    }
                };
                if let Some(ket) = result {
                    states.push(ket);
                } else {
                    statistics.discarded_terms += 1;
                }
            }
        }
        Ok(CatalogueBoundaryExpansion { states, statistics })
    }
}
