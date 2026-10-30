//! Shared completed-pair trace in path coordinates, with no square wire masks.
use crate::gf2::BitVector;
use crate::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use crate::stabilizer::{AffineKet, StabilizerError};

#[derive(Debug)]
pub(crate) struct PairGeometry {
    width: usize,
    keep: Vec<usize>,
    rows: Vec<BitVector>,
}
impl PairGeometry {
    pub(crate) fn rows(&self) -> &[BitVector] {
        &self.rows
    }
    /// Caller admits finite dimensions/storage before these metadata copies.
    pub(crate) fn new(width: usize, rows: &[BitVector]) -> Result<Self, StabilizerError> {
        if rows.iter().any(|row| row.width() != width) {
            return Err(StabilizerError::DimensionMismatch);
        }
        Ok(Self {
            width,
            keep: (0..width).collect(),
            rows: rows.to_vec(),
        })
    }
}

/// Caller admits path, row, pair and peak-storage requirements before entry.
/// Apply any commuting Hermitian Pauli to the LEFT ket before this operation.
pub(crate) fn traced_record_pair(
    left: &AffineKet,
    right: &AffineKet,
    geometry: &PairGeometry,
    event_rows: &[BitVector],
    outcomes: &BitVector,
    report_flips: &BitVector,
    arithmetic: &ScalarArithmetic,
) -> Result<Option<AffineKet>, StabilizerError> {
    let n = geometry.width;
    if left.width() != n
        || right.width() != n
        || event_rows.iter().any(|row| row.width() != n)
        || outcomes.width() != event_rows.len()
        || report_flips.width() != geometry.rows.len()
    {
        return Err(StabilizerError::DimensionMismatch);
    }
    let equations_count = n
        .checked_add(event_rows.len())
        .ok_or(StabilizerError::DimensionOverflow)?;
    let joint = left.tensor(&right.conjugate(), arithmetic)?;
    let paths = joint.columns().len();
    let mut equations = Vec::with_capacity(equations_count);
    let mut rhs = BitVector::zeros(equations_count)?;
    // One n-by-p equation array. LinearFactorization compresses RHS provenance
    // for tall systems; no n-by-2n boundary equality matrix is constructed.
    for q in 0..n {
        let mut equation = BitVector::zeros(paths)?;
        for (j, column) in joint.columns().iter().enumerate() {
            equation.set(j, column.get(q)? ^ column.get(n + q)?)?;
        }
        rhs.set(q, joint.offset().get(q)? ^ joint.offset().get(n + q)?)?;
        equations.push(equation);
    }
    let parity = |row: &BitVector, value: &BitVector| -> Result<bool, StabilizerError> {
        let mut bit = false;
        for q in row.ones() {
            bit ^= value.get(q)?;
        }
        Ok(bit)
    };
    for (k, row) in event_rows.iter().enumerate() {
        let mut equation = BitVector::zeros(paths)?;
        for (j, column) in joint.columns().iter().enumerate() {
            equation.set(j, parity(row, column)?)?;
        }
        rhs.set(n + k, outcomes.get(k)? ^ parity(row, joint.offset())?)?;
        equations.push(equation);
    }
    let Some(traced) = joint.constrain_paths(&equations, &rhs)? else {
        return Ok(None);
    };
    let mapped = traced
        .retain_boundary(&geometry.keep)?
        .linear_boundary(&geometry.rows)?;
    let (mut offset, columns, phase, coefficient) = mapped.into_parts();
    offset.xor_assign(report_flips)?;
    AffineKet::new(offset, columns, phase, coefficient)?.contract(arithmetic)
}

/// Conservative eight-byte payload/header/scratch units. Existing source kets
/// and scalar limbs are excluded. Overflow saturates above every usize budget.
pub(crate) fn pair_geometry_words(
    width: usize,
    paths: usize,
    records: usize,
    events: usize,
    pairs: u128,
) -> u128 {
    let (n, p, r, e) = (
        width as u128,
        paths as u128,
        records as u128,
        events as u128,
    );
    let vectors = |count: u128, bits: u128| count.saturating_mul(bits.div_ceil(64) + 4);
    let equations = n + e;
    let provenance = if equations > 64 && equations > 2 * p {
        p
    } else {
        equations
    };
    let shared = vectors(r + e, n).saturating_add(n).saturating_add(64);
    let scratch = vectors(equations, p)
        .saturating_add(vectors(equations, provenance))
        .saturating_add(vectors(p + 1, 2 * n))
        .saturating_add(vectors(p, p))
        .saturating_add(vectors(p + 1, r))
        .saturating_add(equations.div_ceil(64) + 64);
    let retained = 64_u128
        .saturating_add(vectors(p + 1, r))
        .saturating_add(vectors(p, p))
        .saturating_add(p.div_ceil(8));
    shared
        .saturating_add(16_u128.saturating_mul(scratch))
        .saturating_add(pairs.saturating_mul(retained))
}
/// Conservative logical row/path visits for projection, affine restriction and
/// Gaussian contraction. Packed operations are bounded by their unpacked size;
/// arbitrary-precision scalar work remains under ScalarArithmetic's limits.
pub(crate) fn pair_construction_work(
    width: usize,
    paths: usize,
    records: usize,
    events: usize,
    pairs: u128,
) -> u128 {
    let (n, p, r, e) = (
        width as u128,
        paths as u128,
        records as u128,
        events as u128,
    );
    let per_pair = (n + e + r + 1)
        .saturating_mul((p + 1).saturating_pow(3))
        .saturating_add(n.saturating_mul(e + r).saturating_mul(p + 1));
    16_u128.saturating_mul(pairs).saturating_mul(per_pair)
}

pub(crate) enum MassError {
    Stabilizer(StabilizerError),
    Scalar(ScalarError),
}
impl From<StabilizerError> for MassError {
    fn from(e: StabilizerError) -> Self {
        Self::Stabilizer(e)
    }
}
impl From<ScalarError> for MassError {
    fn from(e: ScalarError) -> Self {
        Self::Scalar(e)
    }
}
pub(crate) struct MassEvaluation {
    pub value: ExactScalar,
    pub gaussian_sums: usize,
}
/// Classical mass is the LINEAR sum of these unnormalized ket amplitudes.
pub(crate) fn evaluate_mass(
    width: usize,
    terms: &[AffineKet],
    rows: &[BitVector],
    outcomes: &BitVector,
    arithmetic: &ScalarArithmetic,
) -> Result<MassEvaluation, MassError> {
    if outcomes.width() != rows.len() || rows.iter().any(|row| row.width() != width) {
        return Err(StabilizerError::DimensionMismatch.into());
    }
    let mut value = ExactScalar::zero();
    let mut gaussian_sums = 0;
    for contribution in terms {
        if let Some(term) = contribution.constrain_z(rows, outcomes)? {
            gaussian_sums += 1;
            let sum = term
                .phase()
                .gauss_sum(arithmetic)
                .map_err(StabilizerError::from)?;
            let mass = arithmetic.multiply(term.coefficient(), &sum)?;
            value = arithmetic.add(&value, &mass)?;
        }
    }
    Ok(MassEvaluation {
        value,
        gaussian_sums,
    })
}
