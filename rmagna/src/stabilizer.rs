//! Unnormalized affine quadratic kets with exact phase-preserving contraction.

use crate::gf2::{BitVector, Gf2Error, LinearFactorization};
use crate::quadratic::{EliminationStep, QuadraticElimination, QuadraticError, QuadraticForm};
use crate::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StabilizerError {
    DimensionMismatch,
    DimensionOverflow,
    Binary(Gf2Error),
    Quadratic(QuadraticError),
    Scalar(ScalarError),
}

impl fmt::Display for StabilizerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionMismatch => write!(
                f,
                "ket boundary, path, or constraint dimensions do not agree"
            ),
            Self::DimensionOverflow => {
                write!(f, "combined ket dimension exceeds the platform index range")
            }
            Self::Binary(error) => error.fmt(f),
            Self::Quadratic(error) => error.fmt(f),
            Self::Scalar(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for StabilizerError {}
impl From<Gf2Error> for StabilizerError {
    fn from(value: Gf2Error) -> Self {
        Self::Binary(value)
    }
}
impl From<QuadraticError> for StabilizerError {
    fn from(value: QuadraticError) -> Self {
        Self::Quadratic(value)
    }
}
impl From<ScalarError> for StabilizerError {
    fn from(value: ScalarError) -> Self {
        Self::Scalar(value)
    }
}

/// `coefficient * sum_x i^q(x) |offset XOR columns*x>`.
///
/// Columns may be dependent before contraction. `contract` sums hidden paths
/// and returns independent support columns, or `None` for a zero ket. All
/// operations retain the original scalar; projection never renormalizes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AffineKet {
    offset: BitVector,
    columns: Vec<BitVector>,
    phase: QuadraticForm,
    coefficient: ExactScalar,
}

impl AffineKet {
    pub fn new(
        offset: BitVector,
        columns: Vec<BitVector>,
        phase: QuadraticForm,
        coefficient: ExactScalar,
    ) -> Result<Self, StabilizerError> {
        if phase.num_variables() != columns.len()
            || columns.iter().any(|c| c.width() != offset.width())
        {
            return Err(StabilizerError::DimensionMismatch);
        }
        Ok(Self {
            offset,
            columns,
            phase,
            coefficient,
        })
    }

    pub fn width(&self) -> usize {
        self.offset.width()
    }
    pub fn offset(&self) -> &BitVector {
        &self.offset
    }
    pub fn columns(&self) -> &[BitVector] {
        &self.columns
    }
    pub fn phase(&self) -> &QuadraticForm {
        &self.phase
    }
    pub fn coefficient(&self) -> &ExactScalar {
        &self.coefficient
    }

    /// Complex conjugation in the computational basis, including global phase.
    pub fn conjugate(&self) -> Self {
        Self {
            offset: self.offset.clone(),
            columns: self.columns.clone(),
            phase: QuadraticForm::new(
                (4 - self.phase.constant()) & 3,
                self.phase.linear().iter().map(|&c| (4 - c) & 3).collect(),
                self.phase.upper().to_vec(),
            )
            .expect("negation preserves a valid quadratic form"),
            coefficient: self.coefficient.conjugate(),
        }
    }

    /// Sum every discarded boundary index against the unnormalized bra
    /// `(<0| + <1|)`, retaining the ordered indices in `keep`. This is a linear
    /// tensor contraction, not a partial trace of a density operator. Repeating
    /// an index copies its computational label to multiple retained legs.
    pub fn sum_boundary(
        &self,
        keep: &[usize],
        arithmetic: &ScalarArithmetic,
    ) -> Result<Option<Self>, StabilizerError> {
        let map = |value: &BitVector| -> Result<BitVector, Gf2Error> {
            let mut mapped = BitVector::zeros(keep.len())?;
            for (i, &j) in keep.iter().enumerate() {
                mapped.set(i, value.get(j)?)?;
            }
            Ok(mapped)
        };
        let offset = map(&self.offset)?;
        let columns = self
            .columns
            .iter()
            .map(map)
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(
            offset,
            columns,
            self.phase.clone(),
            self.coefficient.clone(),
        )?
        .contract(arithmetic)
    }

    pub fn prepare(&self) -> Result<PreparedKet, StabilizerError> {
        let rows = transpose(self.width(), &self.columns)?;
        let factor = LinearFactorization::new(self.columns.len(), &rows)?;
        let kernel = factor.nullspace()?;
        Ok(PreparedKet {
            ket: self.clone(),
            factor,
            kernel,
        })
    }

    /// Sum all internal paths, retaining phase and boundary support exactly.
    pub fn contract(&self, arithmetic: &ScalarArithmetic) -> Result<Option<Self>, StabilizerError> {
        if self.coefficient.is_zero() {
            return Ok(None);
        }
        let prepared = self.prepare()?;
        let count = self.columns.len();
        let mut basis = prepared
            .factor
            .pivot_columns()
            .iter()
            .map(|&j| BitVector::from_indices(count, [j]))
            .collect::<Result<Vec<_>, _>>()?;
        basis.extend(prepared.kernel);
        let phase = self.phase.restrict(&BitVector::zeros(count)?, &basis)?;
        let mut columns = prepared
            .factor
            .pivot_columns()
            .iter()
            .map(|&j| self.columns[j].clone())
            .collect::<Vec<_>>();
        while columns.len() < count {
            columns.push(BitVector::zeros(self.width())?);
        }
        let mut offset = self.offset.clone();
        let mut elimination = QuadraticElimination::new(&phase)?;
        let mut hidden = columns.iter().map(BitVector::is_zero).collect::<Vec<_>>();
        while let Some(x) = (0..count).find(|&i| elimination.active[i] && hidden[i]) {
            match elimination.eliminate(x, &hidden)? {
                EliminationStep::Zero => return Ok(None),
                EliminationStep::Unary => {}
                EliminationStep::Parity {
                    pivot,
                    rhs,
                    neighbors,
                } => {
                    let column = columns[pivot].clone();
                    if rhs {
                        offset.xor_assign(&column)?;
                    }
                    for i in neighbors {
                        columns[i].xor_assign(&column)?;
                        hidden[i] = columns[i].is_zero();
                    }
                }
            }
        }
        let (phase, keep) = elimination.remaining()?;
        let columns = keep.into_iter().map(|i| columns[i].clone()).collect();
        let coefficient =
            arithmetic.multiply(&self.coefficient, &elimination.scalar(arithmetic)?)?;
        Ok(Some(Self::new(offset, columns, phase, coefficient)?))
    }

    /// Apply `X^x Z^z`; the Z phase is evaluated before the X shift.
    pub fn pauli(&self, x: &BitVector, z: &BitVector) -> Result<Self, StabilizerError> {
        if x.width() != self.width() || z.width() != self.width() {
            return Err(StabilizerError::DimensionMismatch);
        }
        let constant = (self.phase.constant() + 2 * u8::from(z.dot(&self.offset)?)) & 3;
        let linear = self
            .columns
            .iter()
            .zip(self.phase.linear())
            .map(|(c, &phase)| Ok((phase + 2 * u8::from(z.dot(c)?)) & 3))
            .collect::<Result<Vec<_>, Gf2Error>>()?;
        let phase = QuadraticForm::new(constant, linear, self.phase.upper().to_vec())?;
        let mut offset = self.offset.clone();
        offset.xor_assign(x)?;
        Self::new(
            offset,
            self.columns.clone(),
            phase,
            self.coefficient.clone(),
        )
    }

    /// Project onto Z-parity outcomes. The returned ket is unnormalized.
    pub fn project_z(
        &self,
        rows: &[BitVector],
        outcomes: &BitVector,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Option<Self>, StabilizerError> {
        if rows.len() != outcomes.width() || rows.iter().any(|row| row.width() != self.width()) {
            return Err(StabilizerError::DimensionMismatch);
        }
        // Trace boundaries have many sparse parity rows. Share the transpose
        // instead of scanning every boundary word for every row/column pair.
        // Small event sets keep the direct path and avoid another matrix.
        let boundary_rows = if rows.len() >= 64 {
            Some(transpose(self.width(), &self.columns)?)
        } else {
            None
        };
        let mut equations = Vec::new();
        let mut rhs = outcomes.clone();
        for (i, row) in rows.iter().enumerate() {
            let mut equation = BitVector::zeros(self.columns.len())?;
            if let Some(boundary_rows) = &boundary_rows {
                for boundary in row.ones() {
                    equation.xor_assign(&boundary_rows[boundary])?;
                }
            } else {
                for (j, column) in self.columns.iter().enumerate() {
                    equation.set(j, row.dot(column)?)?;
                }
            }
            rhs.set(i, rhs.get(i)? ^ row.dot(&self.offset)?)?;
            equations.push(equation);
        }
        let factor = LinearFactorization::new(self.columns.len(), &equations)?;
        let Some(shift) = factor.solve(&rhs)? else {
            return Ok(None);
        };
        let kernel = factor.nullspace()?;
        let mut offset = self.offset.clone();
        offset.xor_assign(&image(self.width(), &self.columns, &shift)?)?;
        let columns = kernel
            .iter()
            .map(|v| image(self.width(), &self.columns, v))
            .collect::<Result<Vec<_>, _>>()?;
        let phase = self.phase.restrict(&shift, &kernel)?;
        Self::new(offset, columns, phase, self.coefficient.clone())?.contract(arithmetic)
    }

    /// Exact `<self|other>`; traced boundary equality is solved by elimination.
    pub fn overlap(
        &self,
        other: &Self,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, StabilizerError> {
        if self.width() != other.width() {
            return Err(StabilizerError::DimensionMismatch);
        }
        if self.coefficient.is_zero() || other.coefficient.is_zero() {
            return Ok(ExactScalar::zero());
        }
        let mut columns = self.columns.clone();
        columns.extend_from_slice(&other.columns);
        let mut rhs = self.offset.clone();
        rhs.xor_assign(&other.offset)?;
        let factor = LinearFactorization::new(columns.len(), &transpose(self.width(), &columns)?)?;
        let Some(shift) = factor.solve(&rhs)? else {
            return Ok(ExactScalar::zero());
        };
        let phase = combined_phase(&self.phase, &other.phase, true)?;
        let sum = phase
            .restrict(&shift, &factor.nullspace()?)?
            .gauss_sum(arithmetic)?;
        let coefficient = arithmetic.multiply(&self.coefficient.conjugate(), &other.coefficient)?;
        Ok(arithmetic.multiply(&coefficient, &sum)?)
    }

    pub fn norm_squared(
        &self,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, StabilizerError> {
        self.overlap(self, arithmetic)
    }

    /// Tensor ordering places `self` on low boundary indices and `other` above it.
    pub fn tensor(
        &self,
        other: &Self,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Self, StabilizerError> {
        let width = self
            .width()
            .checked_add(other.width())
            .ok_or(StabilizerError::DimensionOverflow)?;
        let mut offset = BitVector::from_indices(width, self.offset.ones())?;
        for j in other.offset.ones() {
            offset.set(self.width() + j, true)?;
        }
        let mut columns = self
            .columns
            .iter()
            .map(|c| BitVector::from_indices(width, c.ones()))
            .collect::<Result<Vec<_>, _>>()?;
        for column in &other.columns {
            columns.push(BitVector::from_indices(
                width,
                column.ones().map(|j| self.width() + j),
            )?);
        }
        let phase = combined_phase(&self.phase, &other.phase, false)?;
        let coefficient = arithmetic.multiply(&self.coefficient, &other.coefficient)?;
        Self::new(offset, columns, phase, coefficient)
    }
}

/// Matrix factorization and kernel shared by repeated amplitude queries.
pub struct PreparedKet {
    ket: AffineKet,
    factor: LinearFactorization,
    kernel: Vec<BitVector>,
}

impl PreparedKet {
    pub fn amplitude(
        &self,
        value: &BitVector,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, StabilizerError> {
        if value.width() != self.ket.width() {
            return Err(StabilizerError::DimensionMismatch);
        }
        if self.ket.coefficient.is_zero() {
            return Ok(ExactScalar::zero());
        }
        let mut rhs = value.clone();
        rhs.xor_assign(&self.ket.offset)?;
        let Some(shift) = self.factor.solve(&rhs)? else {
            return Ok(ExactScalar::zero());
        };
        let sum = self
            .ket
            .phase
            .restrict(&shift, &self.kernel)?
            .gauss_sum(arithmetic)?;
        Ok(arithmetic.multiply(&self.ket.coefficient, &sum)?)
    }
}

fn transpose(width: usize, columns: &[BitVector]) -> Result<Vec<BitVector>, Gf2Error> {
    let mut rows = (0..width)
        .map(|_| BitVector::zeros(columns.len()))
        .collect::<Result<Vec<_>, _>>()?;
    for (j, column) in columns.iter().enumerate() {
        for i in column.ones() {
            rows[i].set(j, true)?;
        }
    }
    Ok(rows)
}

fn image(width: usize, columns: &[BitVector], value: &BitVector) -> Result<BitVector, Gf2Error> {
    let mut result = BitVector::zeros(width)?;
    for j in value.ones() {
        result.xor_assign(&columns[j])?;
    }
    Ok(result)
}

fn combined_phase(
    left: &QuadraticForm,
    right: &QuadraticForm,
    conjugate_left: bool,
) -> Result<QuadraticForm, StabilizerError> {
    let n = left.num_variables();
    let width = n
        .checked_add(right.num_variables())
        .ok_or(StabilizerError::DimensionOverflow)?;
    let signed = |v: u8| if conjugate_left { (4 - v) & 3 } else { v };
    let constant = (signed(left.constant()) + right.constant()) & 3;
    let mut linear = left.linear().iter().map(|&v| signed(v)).collect::<Vec<_>>();
    linear.extend_from_slice(right.linear());
    let mut upper = Vec::new();
    for row in left.upper() {
        upper.push(BitVector::from_indices(width, row.ones())?);
    }
    for row in right.upper() {
        upper.push(BitVector::from_indices(width, row.ones().map(|i| n + i))?);
    }
    Ok(QuadraticForm::new(constant, linear, upper)?)
}
