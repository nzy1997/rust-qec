//! Exact Clifford phase functions on affine fibers of a third-level phase.
use crate::gf2::{BitVector, LinearFactorization};
use crate::phase::{ParityPhase, PhaseError, add_quadratic};
use crate::quadratic::QuadraticForm;
use crate::scalar::{ExactScalar, ScalarArithmetic};

#[derive(Clone, Copy, Debug)]
pub struct FiberOptions {
    /// Search only residual signature ranks at or below this value.
    pub max_search_rank: usize,
    /// Deterministic greedy orders; zero retains computational fibers.
    pub attempts: usize,
}
impl Default for FiberOptions {
    fn default() -> Self {
        Self {
            max_search_rank: 16,
            attempts: 256,
        }
    }
}
struct Fiber {
    shift: BitVector,
    global: u8,
    phase: QuadraticForm,
}
/// An exact partition into affine subspaces on which the phase is Clifford.
/// The search is deterministic and heuristic; it claims no optimal rank.
pub struct PreparedCliffordFibers {
    reference: ParityPhase,
    quotient_rank: usize,
    enumerated_coordinates: usize,
    directions: Vec<BitVector>,
    fibers: Vec<Fiber>,
}
impl PreparedCliffordFibers {
    pub fn new(
        reference: &ParityPhase,
        max_terms: usize,
        options: FiberOptions,
    ) -> Result<Self, PhaseError> {
        let quotient = reference.quotient()?;
        let rank = quotient.rank();
        let zero = BitVector::zeros(reference.num_variables())?;
        let resource = reference.restrict(&zero, quotient.complement())?;
        let mut best = Vec::new();
        let upper = if rank <= options.max_search_rank && options.attempts > 0 {
            isotropic_bound(&resource)?
        } else {
            0
        };
        if upper > 0 {
            for attempt in 0..options.attempts {
                let candidate = clifford_subspace(&resource, attempt)?;
                if candidate.len() > best.len() {
                    best = candidate;
                }
                if best.len() == upper {
                    break;
                }
            }
        }
        // Exact certification is independent of the greedy construction.
        resource
            .restrict(&BitVector::zeros(rank)?, &best)?
            .clifford()?;
        let enumerated_coordinates = rank - best.len();
        let count = u32::try_from(enumerated_coordinates)
            .ok()
            .and_then(|r| 1_usize.checked_shl(r));
        let Some(count) = count.filter(|&c| c <= max_terms) else {
            return Err(PhaseError::FiberTermLimit {
                coordinates: enumerated_coordinates,
                quotient_rank: rank,
                limit: max_terms,
            });
        };
        let mut complete = best.clone();
        let mut complement = Vec::new();
        for i in 0..rank {
            let vector = BitVector::from_indices(rank, [i])?;
            if !in_span(&vector, &complete)? {
                complete.push(vector.clone());
                complement.push(vector);
            }
        }
        debug_assert_eq!(complement.len(), enumerated_coordinates);
        let map = |vector: &BitVector| -> Result<BitVector, PhaseError> {
            let mut result = BitVector::zeros(reference.num_variables())?;
            for j in vector.ones() {
                result.xor_assign(&quotient.complement()[j])?;
            }
            Ok(result)
        };
        let mut directions = best.iter().map(map).collect::<Result<Vec<_>, _>>()?;
        directions.extend_from_slice(quotient.radical());
        let complement = complement.iter().map(map).collect::<Result<Vec<_>, _>>()?;
        let mut fibers = Vec::with_capacity(count);
        for assignment in 0..count {
            let mut shift = zero.clone();
            for (j, vector) in complement.iter().enumerate() {
                if assignment >> j & 1 != 0 {
                    shift.xor_assign(vector)?;
                }
            }
            let (global, phase) = reference.restrict(&shift, &directions)?.clifford()?;
            fibers.push(Fiber {
                shift,
                global,
                phase,
            });
        }
        Ok(Self {
            reference: reference.clone(),
            quotient_rank: rank,
            enumerated_coordinates,
            directions,
            fibers,
        })
    }
    pub fn quotient_rank(&self) -> usize {
        self.quotient_rank
    }
    pub fn enumerated_coordinates(&self) -> usize {
        self.enumerated_coordinates
    }
    pub fn num_terms(&self) -> usize {
        self.fibers.len()
    }
    pub fn clifford_directions(&self) -> &[BitVector] {
        &self.directions
    }
    /// Updates must differ by a Clifford phase. The prepared affine partition
    /// and exact reference phase on every fiber remain unchanged.
    pub fn evaluate(
        &self,
        updated: &ParityPhase,
        boundary: &QuadraticForm,
        coefficient: &ExactScalar,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, PhaseError> {
        if boundary.num_variables() != self.reference.num_variables()
            || updated.num_variables() != self.reference.num_variables()
        {
            return Err(PhaseError::DimensionMismatch);
        }
        let (global, correction) = updated.subtract(&self.reference)?.clifford()?;
        let boundary = add_quadratic(boundary, &correction)?;
        let mut total = ExactScalar::zero();
        for fiber in &self.fibers {
            let phase = add_quadratic(
                &fiber.phase,
                &boundary.restrict(&fiber.shift, &self.directions)?,
            )?;
            let sum = phase.gauss_sum(arithmetic)?;
            let weight = arithmetic.omega(i64::from((global + fiber.global) & 7), 0)?;
            total = arithmetic.add(&total, &arithmetic.multiply(&sum, &weight)?)?;
        }
        Ok(arithmetic.multiply(&total, coefficient)?)
    }
}
fn in_span(vector: &BitVector, rows: &[BitVector]) -> Result<bool, PhaseError> {
    let factor = LinearFactorization::new(vector.width(), rows)?;
    let mut remaining = vector.clone();
    for (row, &pivot) in factor.row_space_basis().iter().zip(factor.pivot_columns()) {
        if remaining.get(pivot)? {
            remaining.xor_assign(row)?;
        }
    }
    Ok(remaining.is_zero())
}
fn scramble(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
// A Clifford subspace is also totally isotropic for b(u,v)=sigma(u,u,v).
// The bilinear rank bounds its dimension by n-ceil(rank(b)/2). A nonzero
// cubic signature additionally rules out the entire domain.
fn isotropic_bound(phase: &ParityPhase) -> Result<usize, PhaseError> {
    let width = phase.num_variables();
    let mut rows = vec![BitVector::zeros(width)?; width];
    for (axis, c) in phase.terms() {
        if c & 1 != 0 {
            for i in axis.ones() {
                rows[i].xor_assign(axis)?;
            }
        }
    }
    let rank = LinearFactorization::new(width, &rows)?.rank();
    Ok((width - rank.div_ceil(2)).min(width.saturating_sub(1)))
}
fn clifford_subspace(phase: &ParityPhase, attempt: usize) -> Result<Vec<BitVector>, PhaseError> {
    let width = phase.num_variables();
    let odd = phase
        .terms()
        .iter()
        .filter(|(_, c)| c & 1 != 0)
        .map(|(axis, _)| axis)
        .collect::<Vec<_>>();
    let mut diagonal = BitVector::zeros(width)?;
    for axis in &odd {
        diagonal.xor_assign(axis)?;
    }
    let mut constraints = vec![diagonal];
    let mut basis = Vec::<BitVector>::new();
    loop {
        let allowed = LinearFactorization::new(width, &constraints)?.nullspace()?;
        let mut candidate = BitVector::zeros(width)?;
        if attempt != 0 {
            for (j, vector) in allowed.iter().enumerate() {
                if scramble(
                    (attempt as u64).wrapping_mul(0x243f6a8885a308d3)
                        ^ (basis.len() as u64).wrapping_mul(0x13198a2e03707344)
                        ^ j as u64,
                ) & 1
                    != 0
                {
                    candidate.xor_assign(vector)?;
                }
            }
        }
        if in_span(&candidate, &basis)? {
            let mut found = None;
            for vector in allowed {
                if !in_span(&vector, &basis)? {
                    found = Some(vector);
                    break;
                }
            }
            let Some(vector) = found else {
                break;
            };
            candidate = vector;
        }
        // For parity signatures sigma(v,v,w)=sigma(v,w,w). Together with
        // diagonal(v)=0 these linear slices enforce every new triple on B.
        basis.push(candidate.clone());
        for other in &basis {
            let mut slice = BitVector::zeros(width)?;
            for axis in &odd {
                if axis.dot(&candidate)? && axis.dot(other)? {
                    slice.xor_assign(axis)?;
                }
            }
            constraints.push(slice);
        }
    }
    Ok(basis)
}
