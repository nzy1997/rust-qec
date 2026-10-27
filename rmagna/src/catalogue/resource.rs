//! Frozen affine indicators in one fixed Clifford-quotient coordinate basis.
use super::{CertifiedPhaseDecomposition, PhaseCatalogue};
use crate::gf2::{BitVector, LinearFactorization};
use crate::phase::{
    CanonicalPhaseCore, CliffordQuotient, ParityPhase, PhaseCoreLimits, PhaseError, add_quadratic,
};
use crate::quadratic::QuadraticForm;
use crate::scalar::{ExactScalar, ScalarArithmetic};
use crate::stabilizer::AffineKet;

#[derive(Clone, Copy, Debug)]
pub struct CataloguePreparationOptions {
    pub core: PhaseCoreLimits,
    pub max_terms: usize,
    /// Logical retained bit-vector words plus rounded quadratic linear bytes.
    /// This excludes allocator overhead, input data and temporary workspaces.
    pub max_geometry_words: usize,
}
impl Default for CataloguePreparationOptions {
    fn default() -> Self {
        Self {
            core: PhaseCoreLimits::default(),
            max_terms: 256,
            max_geometry_words: 16 << 20,
        }
    }
}
struct SmallSupport {
    shift: BitVector,
    directions: Vec<BitVector>,
    phase: QuadraticForm,
    global: u8,
}
struct Support {
    shift: BitVector,
    directions: Vec<BitVector>,
    original_shift: BitVector,
    original_directions: Vec<BitVector>,
    phase: QuadraticForm,
    global: u8,
}
struct Term {
    coefficient: ExactScalar,
    support: Option<Support>,
}
/// Exact constrained resource, independent of source/catalogue lifetimes.
/// A signature-changing update is an error, not a request to select a fallback.
pub struct PreparedCataloguePhase {
    quotient: CliffordQuotient,
    reference_resource: ParityPhase,
    core_table: Vec<u8>,
    terms: Vec<Term>,
    geometry_words: usize,
}
fn check(option: &'static str, required: u128, limit: usize) -> Result<(), PhaseError> {
    if required > limit as u128 {
        Err(PhaseError::ResourceLimit {
            option,
            limit: limit as u128,
            required,
        })
    } else {
        Ok(())
    }
}
fn embed(vector: &BitVector, width: usize) -> Result<BitVector, PhaseError> {
    Ok(BitVector::from_indices(width, vector.ones())?)
}
fn image(columns: &[BitVector], vector: &BitVector, width: usize) -> Result<BitVector, PhaseError> {
    let mut value = BitVector::zeros(width)?;
    for j in vector.ones() {
        value.xor_assign(&columns[j])?;
    }
    Ok(value)
}
fn lift_quadratic(q: &QuadraticForm, width: usize) -> Result<QuadraticForm, PhaseError> {
    let mut linear = vec![0; width];
    linear[..q.num_variables()].copy_from_slice(q.linear());
    let upper = (0..width)
        .map(|j| {
            if j < q.num_variables() {
                embed(&q.upper()[j], width)
            } else {
                Ok(BitVector::zeros(width)?)
            }
        })
        .collect::<Result<Vec<_>, PhaseError>>()?;
    Ok(QuadraticForm::new(q.constant(), linear, upper)?)
}
impl PreparedCataloguePhase {
    /// None means no certified dimension/table matches. The probe may spend
    /// bounded quotient/core construction work. An empty catalogue is immediate.
    pub fn new(
        reference: &ParityPhase,
        catalogue: &PhaseCatalogue,
        options: &CataloguePreparationOptions,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Option<Self>, PhaseError> {
        if catalogue.usage().entries == 0 {
            return Ok(None);
        }
        let width = reference.num_variables();
        let words = (width as u128).div_ceil(64);
        let geometry_words = width as u128 * words;
        check(
            "catalogue_preparation.max_geometry_words",
            geometry_words,
            options.max_geometry_words,
        )?;
        let quotient = reference.quotient()?;
        let rank = quotient.rank();
        if !catalogue.entries().any(|e| e.dimension() == rank) {
            return Ok(None);
        }
        let resource = reference.restrict(&BitVector::zeros(width)?, quotient.complement())?;
        let core = CanonicalPhaseCore::new(&resource, &options.core)?;
        let Some(entry) = catalogue.get(rank, core.phase_table()) else {
            return Ok(None);
        };
        Self::from_core(quotient, resource, core, entry, options, arithmetic, 0).map(Some)
    }
    pub(crate) fn from_core(
        quotient: CliffordQuotient,
        resource: ParityPhase,
        core: CanonicalPhaseCore,
        entry: &CertifiedPhaseDecomposition,
        options: &CataloguePreparationOptions,
        arithmetic: &ScalarArithmetic,
        extra_geometry_words: usize,
    ) -> Result<Self, PhaseError> {
        let width = quotient.num_variables();
        let rank = quotient.rank();
        if resource.num_variables() != rank || core.dimension() != rank || entry.dimension() != rank
        {
            return Err(PhaseError::DimensionMismatch);
        }
        let words = (width as u128).div_ceil(64);
        let linear_words = (width as u128).div_ceil(8);
        let mut geometry_words = extra_geometry_words as u128 + width as u128 * words;
        check(
            "catalogue_preparation.max_geometry_words",
            geometry_words,
            options.max_geometry_words,
        )?;
        check(
            "catalogue_preparation.max_terms",
            entry.terms().len() as u128,
            options.max_terms,
        )?;
        geometry_words += resource.terms().len() as u128 * words;
        check(
            "catalogue_preparation.max_geometry_words",
            geometry_words,
            options.max_geometry_words,
        )?;
        let (core_global, core_q) = core.clifford_correction();
        let mut staged = Vec::with_capacity(entry.terms().len());
        for term in entry.terms() {
            arithmetic.multiply(&term.coefficient, &ExactScalar::one())?;
            if term.coefficient.is_zero() {
                staged.push(None);
                continue;
            }
            let rows = term
                .constraints
                .iter()
                .map(|(row, _)| row.clone())
                .collect::<Vec<_>>();
            let outcomes = BitVector::from_indices(
                rows.len(),
                term.constraints
                    .iter()
                    .enumerate()
                    .filter_map(|(j, (_, bit))| bit.then_some(j)),
            )?;
            let factor = LinearFactorization::new(rank, &rows)?;
            let Some(shift) = factor.solve(&outcomes)? else {
                staged.push(None);
                continue;
            };
            let directions = factor.nullspace()?;
            let free = width - rank + directions.len();
            geometry_words += width as u128 * words + linear_words + 2 * (free as u128 + 1) * words;
            check(
                "catalogue_preparation.max_geometry_words",
                geometry_words,
                options.max_geometry_words,
            )?;
            let (global, phase) = term.phase.clifford()?;
            staged.push(Some(SmallSupport {
                shift,
                directions,
                phase: add_quadratic(core_q, &phase)?,
                global: (core_global + global) & 7,
            }));
        }
        let reference_resource = ParityPhase::new(
            width,
            resource.constant(),
            resource
                .terms()
                .iter()
                .map(|(axis, c)| Ok((embed(axis, width)?, *c)))
                .collect::<Result<Vec<_>, PhaseError>>()?,
        )?;
        let mut terms = Vec::with_capacity(staged.len());
        for (term, small) in entry.terms().iter().zip(staged) {
            let support = small
                .map(|small| {
                    let shift = embed(&small.shift, width)?;
                    let directions = small
                        .directions
                        .iter()
                        .map(|column| embed(column, width))
                        .chain((rank..width).map(|j| Ok(BitVector::from_indices(width, [j])?)))
                        .collect::<Result<Vec<_>, PhaseError>>()?;
                    let original_shift = image(quotient.basis(), &shift, width)?;
                    let original_directions = directions
                        .iter()
                        .map(|column| image(quotient.basis(), column, width))
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok::<_, PhaseError>(Support {
                        shift,
                        directions,
                        original_shift,
                        original_directions,
                        phase: lift_quadratic(&small.phase, width)?,
                        global: small.global,
                    })
                })
                .transpose()?;
            terms.push(Term {
                coefficient: term.coefficient.clone(),
                support,
            });
        }
        Ok(Self {
            quotient,
            reference_resource,
            core_table: core.phase_table().to_vec(),
            terms,
            geometry_words: geometry_words as usize,
        })
    }
    pub fn num_variables(&self) -> usize {
        self.quotient.num_variables()
    }
    pub(super) fn quotient(&self) -> &CliffordQuotient {
        &self.quotient
    }
    pub fn quotient_rank(&self) -> usize {
        self.quotient.rank()
    }
    pub fn num_terms(&self) -> usize {
        self.terms.len()
    }
    pub fn canonical_phase_table(&self) -> &[u8] {
        &self.core_table
    }
    pub fn geometry_words(&self) -> usize {
        self.geometry_words
    }
    fn updated_boundary(
        &self,
        updated: &ParityPhase,
        boundary: &QuadraticForm,
    ) -> Result<(u8, QuadraticForm), PhaseError> {
        let width = self.quotient.num_variables();
        if updated.num_variables() != width || boundary.num_variables() != width {
            return Err(PhaseError::DimensionMismatch);
        }
        let zero = BitVector::zeros(width)?;
        let phase = updated.restrict(&zero, self.quotient.basis())?;
        let (global, correction) = phase.subtract(&self.reference_resource)?.clifford()?;
        Ok((
            global,
            add_quadratic(
                &boundary.restrict(&zero, self.quotient.basis())?,
                &correction,
            )?,
        ))
    }
    /// coefficient * sum_x omega^updated(x) i^boundary(x).
    pub fn evaluate(
        &self,
        updated: &ParityPhase,
        boundary: &QuadraticForm,
        coefficient: &ExactScalar,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, PhaseError> {
        let (global, boundary) = self.updated_boundary(updated, boundary)?;
        arithmetic.multiply(coefficient, &ExactScalar::one())?;
        if coefficient.is_zero() {
            return Ok(ExactScalar::zero());
        }
        let mut result = ExactScalar::zero();
        for term in &self.terms {
            if let Some(support) = &term.support {
                let phase = add_quadratic(&boundary, &support.phase)?
                    .restrict(&support.shift, &support.directions)?;
                let sum = phase.gauss_sum(arithmetic)?;
                let factor = arithmetic.multiply(
                    &term.coefficient,
                    &arithmetic.omega(i64::from((global + support.global) & 7), 0)?,
                )?;
                result = arithmetic.add(&result, &arithmetic.multiply(&sum, &factor)?)?;
            }
        }
        Ok(arithmetic.multiply(coefficient, &result)?)
    }
    /// Expand arbitrary labelled boundaries, including dependent output labels.
    /// Different terms may overlap and cancel; no output words are enumerated.
    pub fn expand(
        &self,
        updated: &ParityPhase,
        boundary: &AffineKet,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Vec<AffineKet>, PhaseError> {
        if boundary.columns().len() != self.quotient.num_variables() {
            return Err(PhaseError::DimensionMismatch);
        }
        let (global, quadratic) = self.updated_boundary(updated, boundary.phase())?;
        arithmetic.multiply(boundary.coefficient(), &ExactScalar::one())?;
        if boundary.coefficient().is_zero() {
            return Ok(Vec::new());
        }
        let mut output = Vec::with_capacity(self.terms.len());
        for term in &self.terms {
            if let Some(support) = &term.support {
                let mut offset = boundary.offset().clone();
                offset.xor_assign(&image(
                    boundary.columns(),
                    &support.original_shift,
                    boundary.width(),
                )?)?;
                let columns = support
                    .original_directions
                    .iter()
                    .map(|v| image(boundary.columns(), v, boundary.width()))
                    .collect::<Result<Vec<_>, _>>()?;
                let phase = add_quadratic(&quadratic, &support.phase)?
                    .restrict(&support.shift, &support.directions)?;
                let factor = arithmetic.multiply(
                    &term.coefficient,
                    &arithmetic.omega(i64::from((global + support.global) & 7), 0)?,
                )?;
                let coefficient = arithmetic.multiply(boundary.coefficient(), &factor)?;
                if let Some(ket) =
                    AffineKet::new(offset, columns, phase, coefficient)?.contract(arithmetic)?
                {
                    output.push(ket);
                }
            }
        }
        Ok(output)
    }
}
