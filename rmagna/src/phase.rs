//! Third-hierarchy parity phases, the Clifford radical, and prepared exact sums.

use crate::gf2::{BitVector, Gf2Error, LinearFactorization, transpose_columns};
use crate::quadratic::{QuadraticError, QuadraticForm};
use crate::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use crate::stabilizer::{AffineKet, StabilizerError};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PhaseError {
    InvalidPhase,
    DimensionMismatch,
    NotClifford,
    TermLimit {
        rank: usize,
        limit: usize,
    },
    FiberTermLimit {
        coordinates: usize,
        quotient_rank: usize,
        limit: usize,
    },
    Binary(Gf2Error),
    Quadratic(QuadraticError),
    Scalar(ScalarError),
    Stabilizer(StabilizerError),
}

impl fmt::Display for PhaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPhase => write!(f, "parity phases must lie between zero and seven"),
            Self::DimensionMismatch => write!(f, "phase and affine coordinate dimensions disagree"),
            Self::NotClifford => write!(f, "phase has a nonzero third-derivative signature"),
            Self::TermLimit { rank, limit } => write!(
                f,
                "quotient rank {rank} exceeds the limit of {limit} computational terms"
            ),
            Self::FiberTermLimit {
                coordinates,
                quotient_rank,
                limit,
            } => write!(
                f,
                "Clifford-fiber enumeration needs 2^{coordinates} terms for signature rank {quotient_rank}; limit is {limit}"
            ),
            Self::Binary(e) => e.fmt(f),
            Self::Quadratic(e) => e.fmt(f),
            Self::Scalar(e) => e.fmt(f),
            Self::Stabilizer(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for PhaseError {}
impl From<Gf2Error> for PhaseError {
    fn from(e: Gf2Error) -> Self {
        Self::Binary(e)
    }
}
impl From<QuadraticError> for PhaseError {
    fn from(e: QuadraticError) -> Self {
        Self::Quadratic(e)
    }
}
impl From<ScalarError> for PhaseError {
    fn from(e: ScalarError) -> Self {
        Self::Scalar(e)
    }
}
impl From<StabilizerError> for PhaseError {
    fn from(e: StabilizerError) -> Self {
        Self::Stabilizer(e)
    }
}

/// `constant + sum_(a,c) c * parity(a AND x) (mod 8)`.
///
/// Duplicate axes are merged modulo eight. This compact representation includes
/// T, controlled-S, CCZ and their inverses, without expanding cubic monomials.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParityPhase {
    width: usize,
    constant: u8,
    terms: Vec<(BitVector, u8)>,
}

impl ParityPhase {
    pub fn new(
        width: usize,
        constant: u8,
        terms: impl IntoIterator<Item = (BitVector, u8)>,
    ) -> Result<Self, PhaseError> {
        if constant > 7 {
            return Err(PhaseError::InvalidPhase);
        }
        let mut merged = BTreeMap::<Vec<u64>, (BitVector, u8)>::new();
        for (axis, coefficient) in terms {
            if axis.width() != width {
                return Err(PhaseError::DimensionMismatch);
            }
            if coefficient > 7 {
                return Err(PhaseError::InvalidPhase);
            }
            if !axis.is_zero() {
                let entry = merged.entry(axis.words().to_vec()).or_insert((axis, 0));
                entry.1 = (entry.1 + coefficient) & 7;
            }
        }
        Ok(Self {
            width,
            constant,
            terms: merged.into_values().filter(|(_, c)| *c != 0).collect(),
        })
    }
    pub fn num_variables(&self) -> usize {
        self.width
    }
    pub fn constant(&self) -> u8 {
        self.constant
    }
    pub fn terms(&self) -> &[(BitVector, u8)] {
        &self.terms
    }

    fn check_vector(&self, value: &BitVector) -> Result<(), PhaseError> {
        if value.width() != self.width {
            return Err(PhaseError::DimensionMismatch);
        }
        Ok(())
    }
    pub fn evaluate(&self, value: &BitVector) -> Result<u8, PhaseError> {
        self.check_vector(value)?;
        let mut phase = self.constant;
        for (axis, coefficient) in &self.terms {
            if axis.dot(value)? {
                phase = (phase + coefficient) & 7;
            }
        }
        Ok(phase)
    }

    /// Substitute `x = shift XOR sum_j basis[j]*y[j]`; dependence is allowed.
    pub fn restrict(&self, shift: &BitVector, basis: &[BitVector]) -> Result<Self, PhaseError> {
        self.check_vector(shift)?;
        if basis.is_empty() {
            return Self::new(0, self.evaluate(shift)?, []);
        }
        for vector in basis {
            self.check_vector(vector)?;
        }
        let basis_rows = if self.terms.len() >= 64 {
            Some(transpose_columns(self.width, basis)?)
        } else {
            None
        };
        let mut constant = self.constant;
        let mut terms = Vec::new();
        for (axis, coefficient) in &self.terms {
            let mut mapped = BitVector::zeros(basis.len())?;
            if let Some(basis_rows) = &basis_rows {
                for coordinate in axis.ones() {
                    mapped.xor_assign(&basis_rows[coordinate])?;
                }
            } else {
                for (j, vector) in basis.iter().enumerate() {
                    mapped.set(j, axis.dot(vector)?)?;
                }
            }
            let coefficient = if axis.dot(shift)? {
                constant = (constant + coefficient) & 7;
                (8 - coefficient) & 7
            } else {
                *coefficient
            };
            terms.push((mapped, coefficient));
        }
        Self::new(basis.len(), constant, terms)
    }

    pub fn subtract(&self, other: &Self) -> Result<Self, PhaseError> {
        if self.width != other.width {
            return Err(PhaseError::DimensionMismatch);
        }
        Self::new(
            self.width,
            (self.constant + 8 - other.constant) & 7,
            self.terms
                .iter()
                .cloned()
                .chain(other.terms.iter().map(|(a, c)| (a.clone(), (8 - c) & 7))),
        )
    }

    /// `Delta_u Delta_v Delta_w phase = 4 * signature(u,v,w) (mod 8)`.
    /// Repeated directions are included: even a single T has nonzero signature.
    pub fn signature(
        &self,
        u: &BitVector,
        v: &BitVector,
        w: &BitVector,
    ) -> Result<bool, PhaseError> {
        self.check_vector(u)?;
        self.check_vector(v)?;
        self.check_vector(w)?;
        let mut result = false;
        for (axis, coefficient) in &self.terms {
            if coefficient & 1 != 0 {
                result ^= axis.dot(u)? && axis.dot(v)? && axis.dot(w)?;
            }
        }
        Ok(result)
    }

    fn signature_slice(&self, i: usize, j: usize) -> Result<BitVector, PhaseError> {
        let mut row = BitVector::zeros(self.width)?;
        for (axis, coefficient) in &self.terms {
            if coefficient & 1 != 0 && axis.get(i)? && axis.get(j)? {
                row.xor_assign(axis)?;
            }
        }
        Ok(row)
    }

    pub fn quotient(&self) -> Result<CliffordQuotient, PhaseError> {
        // Stream signature slices into a row basis. Retaining all n(n+1)/2
        // equations and their provenance would require unnecessary cubic space.
        let odd = self
            .terms
            .iter()
            .filter(|(_, c)| c & 1 != 0)
            .map(|(a, _)| a)
            .collect::<Vec<_>>();
        let mut rows = BTreeMap::<usize, BitVector>::new();
        'slices: for i in 0..self.width {
            if odd.is_empty() {
                break;
            }
            for j in i..self.width {
                let mut row = self.signature_slice(i, j)?;
                for (&pivot, basis) in &rows {
                    if row.get(pivot)? {
                        row.xor_assign(basis)?;
                    }
                }
                let pivot = row.ones().next();
                if let Some(pivot) = pivot {
                    rows.insert(pivot, row);
                }
                if rows.len() == self.width {
                    break 'slices;
                }
            }
        }
        let factor = LinearFactorization::new(self.width, &rows.into_values().collect::<Vec<_>>())?;
        let rank = factor.rank();
        let mut basis = factor
            .pivot_columns()
            .iter()
            .map(|&j| BitVector::from_indices(self.width, [j]))
            .collect::<Result<Vec<_>, _>>()?;
        basis.extend(factor.nullspace()?);
        Ok(CliffordQuotient {
            width: self.width,
            rank,
            basis,
        })
    }

    /// Return `omega^global * i^quadratic`. Non-Clifford phases are rejected.
    pub fn clifford(&self) -> Result<(u8, QuadraticForm), PhaseError> {
        // Pivot coordinates restrict the odd-axis span injectively. Thus a
        // zero trilinear signature on this smaller coordinate space is zero
        // on the full space; no quotient basis or decomposition is changed.
        let odd = self
            .terms
            .iter()
            .filter(|(_, c)| c & 1 != 0)
            .map(|(axis, _)| axis.clone())
            .collect::<Vec<_>>();
        if !odd.is_empty() {
            let compressed;
            let signature = if self.width > 64 {
                let pivots = LinearFactorization::new(self.width, &odd)?
                    .pivot_columns()
                    .to_vec();
                if pivots.len() < self.width {
                    compressed = Self::new(
                        pivots.len(),
                        0,
                        odd.iter()
                            .map(|axis| {
                                Ok((
                                    BitVector::from_indices(
                                        pivots.len(),
                                        pivots.iter().enumerate().filter_map(|(j, &k)| {
                                            axis.get(k).expect("validated coordinate").then_some(j)
                                        }),
                                    )?,
                                    1,
                                ))
                            })
                            .collect::<Result<Vec<_>, Gf2Error>>()?,
                    )?;
                    &compressed
                } else {
                    self
                }
            } else {
                self
            };
            for i in 0..signature.width {
                for j in i..signature.width {
                    if !signature.signature_slice(i, j)?.is_zero() {
                        return Err(PhaseError::NotClifford);
                    }
                }
            }
        }
        let mut linear = vec![0_u8; self.width];
        let mut pair = Vec::with_capacity(self.width);
        for (axis, coefficient) in &self.terms {
            for i in axis.ones() {
                linear[i] = (linear[i] + coefficient) & 7;
            }
        }
        // Sum each pair coefficient modulo four with two packed bit planes.
        // The low plane's carry updates the high plane before the low XOR.
        // Signature zero makes the final low plane zero and linear terms even.
        for i in 0..self.width {
            let mut low = BitVector::zeros(self.width)?;
            let mut high = BitVector::zeros(self.width)?;
            for (axis, coefficient) in &self.terms {
                if axis.get(i)? {
                    if coefficient & 1 != 0 {
                        high.xor_and_assign(&low, axis)?;
                        low.xor_assign(axis)?;
                    }
                    if coefficient & 2 != 0 {
                        high.xor_assign(axis)?;
                    }
                }
            }
            debug_assert!(low.is_zero());
            pair.push(BitVector::from_indices(
                self.width,
                high.ones().filter(|&j| j > i),
            )?);
        }
        debug_assert!(linear.iter().all(|c| c & 1 == 0));
        Ok((
            self.constant,
            QuadraticForm::new(0, linear.into_iter().map(|c| c / 2).collect(), pair)?,
        ))
    }
}

/// A full invertible coordinate basis: quotient representatives followed by
/// radical directions. The radical annihilates every signature slice.
#[derive(Clone, Debug)]
pub struct CliffordQuotient {
    width: usize,
    rank: usize,
    basis: Vec<BitVector>,
}
impl CliffordQuotient {
    pub fn num_variables(&self) -> usize {
        self.width
    }
    pub fn rank(&self) -> usize {
        self.rank
    }
    pub fn complement(&self) -> &[BitVector] {
        &self.basis[..self.rank]
    }
    pub fn radical(&self) -> &[BitVector] {
        &self.basis[self.rank..]
    }
    pub fn basis(&self) -> &[BitVector] {
        &self.basis
    }
}

/// Exact computational decomposition of the residual quotient, prepared once.
///
/// Evaluation accepts phases with the same signature (including affine shifts,
/// sign changes and Clifford corrections). It reuses the reference quotient and
/// resource coefficients; only quadratic sums depend on the update. No physical
/// state vector is constructed. The explicit term budget rejects large ranks.
pub struct PreparedPhaseSum {
    quotient: CliffordQuotient,
    reference_resource: ParityPhase,
    coefficients: Vec<ExactScalar>,
}
impl PreparedPhaseSum {
    pub fn new(
        reference: &ParityPhase,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, PhaseError> {
        let quotient = reference.quotient()?;
        Self::from_quotient(reference, quotient, max_terms, arithmetic)
    }
    fn from_quotient(
        reference: &ParityPhase,
        quotient: CliffordQuotient,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, PhaseError> {
        let terms = u32::try_from(quotient.rank)
            .ok()
            .and_then(|shift| 1_usize.checked_shl(shift));
        let Some(terms) = terms.filter(|&n| n <= max_terms) else {
            return Err(PhaseError::TermLimit {
                rank: quotient.rank,
                limit: max_terms,
            });
        };
        let resource =
            reference.restrict(&BitVector::zeros(reference.width)?, quotient.complement())?;
        let mut coefficients = Vec::new();
        for x in 0..terms {
            let vector = assignment(quotient.rank, x)?;
            coefficients.push(arithmetic.omega(i64::from(resource.evaluate(&vector)?), 0)?);
        }
        let lifted = ParityPhase::new(
            reference.width,
            resource.constant,
            resource
                .terms
                .iter()
                .map(|(axis, c)| Ok((BitVector::from_indices(reference.width, axis.ones())?, *c)))
                .collect::<Result<Vec<_>, Gf2Error>>()?,
        )?;
        Ok(Self {
            quotient,
            reference_resource: lifted,
            coefficients,
        })
    }
    pub fn quotient(&self) -> &CliffordQuotient {
        &self.quotient
    }
    pub fn num_terms(&self) -> usize {
        self.coefficients.len()
    }

    /// Evaluate `coefficient * sum_x omega^updated(x) i^boundary(x)`.
    /// The boundary is expressed in the original coordinates and need not be
    /// normalized. A signature mismatch fails before producing a result.
    pub fn evaluate(
        &self,
        updated: &ParityPhase,
        boundary: &QuadraticForm,
        coefficient: &ExactScalar,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, PhaseError> {
        let width = self.quotient.width;
        if updated.width != width || boundary.num_variables() != width {
            return Err(PhaseError::DimensionMismatch);
        }
        let zero = BitVector::zeros(width)?;
        let transformed = updated.restrict(&zero, self.quotient.basis())?;
        let difference = transformed.subtract(&self.reference_resource)?;
        let (global, clifford) = difference.clifford()?;
        let boundary = boundary.restrict(&zero, self.quotient.basis())?;
        let phase = add_quadratic(&boundary, &clifford)?;
        let remaining = (self.quotient.rank..width)
            .map(|j| BitVector::from_indices(width, [j]))
            .collect::<Result<Vec<_>, _>>()?;
        let mut total = ExactScalar::zero();
        for (x, resource) in self.coefficients.iter().enumerate() {
            let shift = assignment(width, x)?;
            let sum = phase.restrict(&shift, &remaining)?.gauss_sum(arithmetic)?;
            total = arithmetic.add(&total, &arithmetic.multiply(resource, &sum)?)?;
        }
        Ok(arithmetic.multiply(
            &arithmetic.multiply(&total, &arithmetic.omega(i64::from(global), 0)?)?,
            coefficient,
        )?)
    }
}

/// One polynomial-time radical contraction, before resource enumeration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadicalReduction {
    pub input_variables: usize,
    pub signature_rank: usize,
    pub output_variables: usize,
}

/// A closed network with all Clifford radical directions summed first. The
/// resulting affine constraints can further reduce the signature, so reduction
/// repeats until the residual radical is empty. The term budget is imposed only
/// after these exact polynomial-time steps. This cold plan has no update API;
/// `PreparedPhaseSum` separately supports a fixed quotient with Clifford updates.
pub struct ClosedPhaseSum {
    plan: Option<ClosedPlan>,
    reductions: Vec<RadicalReduction>,
}
enum ClosedDecomposition {
    Computational(PreparedPhaseSum),
    Fibers(crate::fibers::PreparedCliffordFibers),
}
struct ClosedPlan {
    prepared: ClosedDecomposition,
    phase: ParityPhase,
    boundary: QuadraticForm,
    coefficient: ExactScalar,
}
impl ClosedPhaseSum {
    pub fn new(
        phase: &ParityPhase,
        boundary: &QuadraticForm,
        coefficient: &ExactScalar,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, PhaseError> {
        Self::construct(phase, boundary, coefficient, max_terms, arithmetic, None)
    }
    /// Select certified Clifford fibers after radical absorption. The original
    /// `new` method retains its deterministic computational expansion contract.
    pub fn new_adaptive(
        phase: &ParityPhase,
        boundary: &QuadraticForm,
        coefficient: &ExactScalar,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
        options: crate::fibers::FiberOptions,
    ) -> Result<Self, PhaseError> {
        Self::construct(
            phase,
            boundary,
            coefficient,
            max_terms,
            arithmetic,
            Some(options),
        )
    }
    fn construct(
        phase: &ParityPhase,
        boundary: &QuadraticForm,
        coefficient: &ExactScalar,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
        options: Option<crate::fibers::FiberOptions>,
    ) -> Result<Self, PhaseError> {
        if phase.width != boundary.num_variables() {
            return Err(PhaseError::DimensionMismatch);
        }
        let mut output = Self {
            plan: None,
            reductions: Vec::new(),
        };
        if coefficient.is_zero() {
            return Ok(output);
        }
        let mut phase = phase.clone();
        let mut boundary = boundary.clone();
        let mut coefficient = coefficient.clone();
        loop {
            let quotient = phase.quotient()?;
            let width = phase.width;
            let rank = quotient.rank;
            if rank == width {
                let prepared = if let Some(options) = options {
                    ClosedDecomposition::Fibers(crate::fibers::PreparedCliffordFibers::new(
                        &phase, max_terms, options,
                    )?)
                } else {
                    ClosedDecomposition::Computational(PreparedPhaseSum::from_quotient(
                        &phase, quotient, max_terms, arithmetic,
                    )?)
                };
                output.plan = Some(ClosedPlan {
                    prepared,
                    phase,
                    boundary,
                    coefficient,
                });
                return Ok(output);
            }
            let zero = BitVector::zeros(width)?;
            let transformed = phase.restrict(&zero, quotient.basis())?;
            let resource = phase.restrict(&zero, quotient.complement())?;
            let lifted = ParityPhase::new(
                width,
                resource.constant,
                resource
                    .terms
                    .iter()
                    .map(|(axis, c)| Ok((BitVector::from_indices(width, axis.ones())?, *c)))
                    .collect::<Result<Vec<_>, Gf2Error>>()?,
            )?;
            let (global, correction) = transformed.subtract(&lifted)?.clifford()?;
            let transformed_boundary = boundary.restrict(&zero, quotient.basis())?;
            let corrected_boundary = add_quadratic(&transformed_boundary, &correction)?;
            let columns = (0..width)
                .map(|j| BitVector::from_indices(rank, (j < rank).then_some(j)))
                .collect::<Result<Vec<_>, _>>()?;
            let weighted_coefficient =
                arithmetic.multiply(&coefficient, &arithmetic.omega(i64::from(global), 0)?)?;
            let Some(ket) = AffineKet::new(
                BitVector::zeros(rank)?,
                columns,
                corrected_boundary,
                weighted_coefficient,
            )?
            .contract(arithmetic)?
            else {
                output.reductions.push(RadicalReduction {
                    input_variables: width,
                    signature_rank: rank,
                    output_variables: 0,
                });
                return Ok(output);
            };
            output.reductions.push(RadicalReduction {
                input_variables: width,
                signature_rank: rank,
                output_variables: ket.columns().len(),
            });
            phase = resource.restrict(ket.offset(), ket.columns())?;
            boundary = ket.phase().clone();
            coefficient = ket.coefficient().clone();
        }
    }
    pub fn reductions(&self) -> &[RadicalReduction] {
        &self.reductions
    }
    pub fn quotient_rank(&self) -> usize {
        self.plan.as_ref().map_or(0, |p| match &p.prepared {
            ClosedDecomposition::Computational(plan) => plan.quotient().rank(),
            ClosedDecomposition::Fibers(plan) => plan.quotient_rank(),
        })
    }
    pub fn num_terms(&self) -> usize {
        self.plan.as_ref().map_or(0, |p| match &p.prepared {
            ClosedDecomposition::Computational(plan) => plan.num_terms(),
            ClosedDecomposition::Fibers(plan) => plan.num_terms(),
        })
    }
    pub fn evaluate(&self, arithmetic: &ScalarArithmetic) -> Result<ExactScalar, PhaseError> {
        let Some(plan) = &self.plan else {
            return Ok(ExactScalar::zero());
        };
        match &plan.prepared {
            ClosedDecomposition::Computational(prepared) => {
                prepared.evaluate(&plan.phase, &plan.boundary, &plan.coefficient, arithmetic)
            }
            ClosedDecomposition::Fibers(prepared) => {
                prepared.evaluate(&plan.phase, &plan.boundary, &plan.coefficient, arithmetic)
            }
        }
    }
}

pub(crate) fn add_quadratic(
    left: &QuadraticForm,
    right: &QuadraticForm,
) -> Result<QuadraticForm, PhaseError> {
    let linear = left
        .linear()
        .iter()
        .zip(right.linear())
        .map(|(a, b)| (a + b) & 3)
        .collect();
    let mut upper = left.upper().to_vec();
    for (a, b) in upper.iter_mut().zip(right.upper()) {
        a.xor_assign(b)?;
    }
    Ok(QuadraticForm::new(
        (left.constant() + right.constant()) & 3,
        linear,
        upper,
    )?)
}

fn assignment(width: usize, value: usize) -> Result<BitVector, Gf2Error> {
    BitVector::from_indices(
        width,
        (0..width.min(usize::BITS as usize)).filter(|&j| value >> j & 1 != 0),
    )
}
