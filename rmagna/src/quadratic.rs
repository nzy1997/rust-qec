//! Quadratic phases modulo four, affine restriction, and exact Gauss sums.

mod prepared;
pub use prepared::PreparedQuadraticSum;

use crate::gf2::{BitVector, Gf2Error};
use crate::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuadraticError {
    InvalidPhase,
    InvalidUpperTriangle,
    DimensionMismatch,
    IncompatiblePreparation,
    Binary(Gf2Error),
    Scalar(ScalarError),
}

impl fmt::Display for QuadraticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPhase => write!(
                f,
                "quadratic phases must be integers from zero through three"
            ),
            Self::InvalidUpperTriangle => write!(
                f,
                "quadratic cross terms must lie strictly above the diagonal"
            ),
            Self::DimensionMismatch => write!(
                f,
                "quadratic form, shift, and basis dimensions do not agree"
            ),
            Self::IncompatiblePreparation => write!(
                f,
                "prepared quadratic sum requires unchanged cross terms and odd linear coefficients"
            ),
            Self::Binary(error) => error.fmt(f),
            Self::Scalar(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for QuadraticError {}
impl From<Gf2Error> for QuadraticError {
    fn from(value: Gf2Error) -> Self {
        Self::Binary(value)
    }
}
impl From<ScalarError> for QuadraticError {
    fn from(value: ScalarError) -> Self {
        Self::Scalar(value)
    }
}

/// `q(x) = constant + sum_j linear[j]*x[j] + 2*sum_(j<k) upper[j,k]*x[j]*x[k] (mod 4)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuadraticForm {
    constant: u8,
    linear: Vec<u8>,
    upper: Vec<BitVector>,
}

impl QuadraticForm {
    pub fn new(
        constant: u8,
        linear: Vec<u8>,
        upper: Vec<BitVector>,
    ) -> Result<Self, QuadraticError> {
        if constant > 3 || linear.iter().any(|&phase| phase > 3) {
            return Err(QuadraticError::InvalidPhase);
        }
        let width = linear.len();
        if upper.len() != width || upper.iter().any(|row| row.width() != width) {
            return Err(QuadraticError::DimensionMismatch);
        }
        if upper
            .iter()
            .enumerate()
            .any(|(i, row)| row.ones().any(|j| j <= i))
        {
            return Err(QuadraticError::InvalidUpperTriangle);
        }
        Ok(Self {
            constant,
            linear,
            upper,
        })
    }

    pub fn num_variables(&self) -> usize {
        self.linear.len()
    }
    pub fn constant(&self) -> u8 {
        self.constant
    }
    pub fn linear(&self) -> &[u8] {
        &self.linear
    }
    pub fn upper(&self) -> &[BitVector] {
        &self.upper
    }

    fn check_vector(&self, value: &BitVector) -> Result<(), QuadraticError> {
        if value.width() != self.num_variables() {
            return Err(QuadraticError::DimensionMismatch);
        }
        Ok(())
    }

    pub fn evaluate(&self, value: &BitVector) -> Result<u8, QuadraticError> {
        self.check_vector(value)?;
        let mut phase = self.constant;
        for j in value.ones() {
            phase = (phase + self.linear[j] + 2 * u8::from(self.upper[j].dot(value)?)) & 3;
        }
        Ok(phase)
    }

    fn symmetric_matrix(&self, include_diagonal: bool) -> Result<Vec<BitVector>, QuadraticError> {
        let mut rows = self.upper.clone();
        for (i, upper) in self.upper.iter().enumerate() {
            if include_diagonal && self.linear[i] & 1 != 0 {
                rows[i].set(i, true)?;
            }
            for j in upper.ones() {
                rows[j].set(i, true)?;
            }
        }
        Ok(rows)
    }

    /// Substitute `x = shift XOR sum_j basis[j]*y[j]`. The basis may be dependent.
    pub fn restrict(&self, shift: &BitVector, basis: &[BitVector]) -> Result<Self, QuadraticError> {
        Ok(self.restrict_with_images(shift, basis)?.0)
    }

    pub(crate) fn restrict_with_images(
        &self,
        shift: &BitVector,
        basis: &[BitVector],
    ) -> Result<(Self, Vec<BitVector>), QuadraticError> {
        self.check_vector(shift)?;
        for vector in basis {
            self.check_vector(vector)?;
        }
        let polar = self.symmetric_matrix(true)?;
        let words = self.num_variables().div_ceil(64);
        // Only sparse products need cached indices. Dense lists would retain
        // one usize per set bit instead of the existing packed representation.
        let coordinates = basis
            .iter()
            .map(|v| (v.count_ones() < words).then(|| v.ones().collect::<Vec<_>>()))
            .collect::<Vec<_>>();
        let mut images = Vec::new();
        let mut linear = Vec::new();
        for vector in basis {
            // The polar matrix is symmetric, so B*v is the XOR of selected
            // packed rows. Unit directions avoid scanning every matrix row.
            let mut image = BitVector::zeros(self.num_variables())?;
            for j in vector.ones() {
                image.xor_assign(&polar[j])?;
            }
            linear.push(
                (self.evaluate(vector)? + 4 - self.constant + 2 * u8::from(image.dot(shift)?)) & 3,
            );
            images.push(image);
        }
        let mut upper = Vec::new();
        for (i, image) in images.iter().enumerate() {
            let mut row = BitVector::zeros(basis.len())?;
            for (j, vector) in basis.iter().enumerate().skip(i + 1) {
                let value = if let Some(selected) = &coordinates[j] {
                    selected
                        .iter()
                        .try_fold(false, |parity, &k| image.get(k).map(|bit| parity ^ bit))?
                } else {
                    image.dot(vector)?
                };
                row.set(j, value)?;
            }
            upper.push(row);
        }
        Ok((Self::new(self.evaluate(shift)?, linear, upper)?, images))
    }

    /// Evaluate `sum_x i^q(x)` without enumerating the binary assignments.
    ///
    /// An odd linear term eliminates one variable through `1 +/- i`. An even
    /// term enforces a binary parity equation, eliminating a second variable
    /// when necessary. All induced phases, including the global phase, remain.
    pub fn gauss_sum(&self, arithmetic: &ScalarArithmetic) -> Result<ExactScalar, QuadraticError> {
        let mut state = QuadraticElimination::new(self)?;
        loop {
            let next = (0..state.active.len())
                .find(|&i| state.active[i] && state.values.linear[i] & 1 != 0)
                .or_else(|| state.active.iter().position(|&bit| bit));
            let Some(x) = next else {
                break;
            };
            if matches!(state.eliminate(x, &[])?, EliminationStep::Zero) {
                return Ok(ExactScalar::zero());
            }
        }
        state.scalar(arithmetic)
    }
}

pub(crate) enum EliminationStep {
    Zero,
    Unary,
    Parity {
        pivot: usize,
        rhs: bool,
        neighbors: Vec<usize>,
    },
}

/// Numeric state shared by cold and prepared Gaussian elimination.
struct QuadraticValues {
    linear: Vec<u8>,
    phase: i16,
    sqrt2_power: i64,
}
impl QuadraticValues {
    fn new(form: &QuadraticForm) -> Self {
        Self {
            linear: form.linear.clone(),
            phase: i16::from(form.constant) * 2,
            sqrt2_power: 0,
        }
    }
    fn scalar(&self, arithmetic: &ScalarArithmetic) -> Result<ExactScalar, QuadraticError> {
        Ok(arithmetic.omega(i64::from(self.phase), self.sqrt2_power)?)
    }
}

/// Structural actions depend on adjacency and linear parity, not even phases.
/// The same value updates execute a cold step or a prepared query.
enum QuadraticAction {
    Odd {
        x: usize,
        neighbors: Vec<usize>,
    },
    Free {
        x: usize,
    },
    Pair {
        x: usize,
        y: usize,
        v: Vec<usize>,
        w: Vec<(usize, bool)>,
    },
}
impl QuadraticAction {
    fn apply(&self, values: &mut QuadraticValues) -> Result<bool, QuadraticError> {
        values.sqrt2_power = values
            .sqrt2_power
            .checked_add(if matches!(self, Self::Odd { .. }) {
                1
            } else {
                2
            })
            .ok_or(ScalarError::ExponentOverflow)?;
        match self {
            Self::Odd { x, neighbors } => {
                values.phase =
                    (values.phase + if values.linear[*x] == 1 { 1 } else { -1 }).rem_euclid(8);
                for &i in neighbors {
                    values.linear[i] = (values.linear[i] + 4 - values.linear[*x]) & 3;
                }
                Ok(true)
            }
            Self::Free { x } => Ok(values.linear[*x] == 0),
            Self::Pair { x, y, v, w } => {
                let rhs = values.linear[*x] / 2;
                values.phase =
                    (values.phase + 2 * i16::from(values.linear[*y] * rhs)).rem_euclid(8);
                let delta = if rhs == 0 {
                    values.linear[*y]
                } else {
                    (4 - values.linear[*y]) & 3
                };
                for &i in v {
                    values.linear[i] = (values.linear[i] + delta) & 3;
                }
                for &(i, adjacent) in w {
                    values.linear[i] = (values.linear[i] + 2 * rhs + 2 * u8::from(adjacent)) & 3;
                }
                Ok(true)
            }
        }
    }
}

pub(crate) struct QuadraticElimination {
    pub(crate) active: Vec<bool>,
    values: QuadraticValues,
    adjacency: Vec<BitVector>,
}

impl QuadraticElimination {
    pub(crate) fn new(form: &QuadraticForm) -> Result<Self, QuadraticError> {
        Ok(Self {
            active: vec![true; form.num_variables()],
            values: QuadraticValues::new(form),
            adjacency: form.symmetric_matrix(false)?,
        })
    }
    pub(crate) fn scalar(
        &self,
        arithmetic: &ScalarArithmetic,
    ) -> Result<ExactScalar, QuadraticError> {
        self.values.scalar(arithmetic)
    }
    pub(crate) fn eliminate(
        &mut self,
        x: usize,
        preferred: &[bool],
    ) -> Result<EliminationStep, QuadraticError> {
        let (nonzero, action) = self.eliminate_recorded(x, preferred)?;
        if !nonzero {
            return Ok(EliminationStep::Zero);
        }
        Ok(match action {
            QuadraticAction::Pair { x, y, v, .. } => EliminationStep::Parity {
                pivot: y,
                rhs: self.values.linear[x] != 0,
                neighbors: v,
            },
            _ => EliminationStep::Unary,
        })
    }
    fn eliminate_recorded(
        &mut self,
        x: usize,
        preferred: &[bool],
    ) -> Result<(bool, QuadraticAction), QuadraticError> {
        self.active[x] = false;
        let neighbors = self.adjacency[x]
            .ones()
            .filter(|&i| self.active[i])
            .collect::<Vec<_>>();
        let action = if self.values.linear[x] & 1 != 0 {
            toggle_clique(&mut self.adjacency, &neighbors)?;
            QuadraticAction::Odd { x, neighbors }
        } else {
            let chosen = neighbors
                .iter()
                .find(|&&i| preferred.get(i).copied().unwrap_or(false))
                .or_else(|| neighbors.first());
            if let Some(&y) = chosen {
                self.active[y] = false;
                let v = neighbors
                    .into_iter()
                    .filter(|&i| self.active[i])
                    .collect::<Vec<_>>();
                let w = self.adjacency[y]
                    .ones()
                    .filter(|&i| self.active[i])
                    .map(|i| self.adjacency[x].get(i).map(|bit| (i, bit)))
                    .collect::<Result<Vec<_>, _>>()?;
                if self.values.linear[y] & 1 != 0 {
                    toggle_clique(&mut self.adjacency, &v)?;
                }
                for &i in &v {
                    for &(j, _) in &w {
                        if i != j {
                            toggle_edge(&mut self.adjacency, i, j)?;
                        }
                    }
                }
                QuadraticAction::Pair { x, y, v, w }
            } else {
                QuadraticAction::Free { x }
            }
        };
        let step = action.apply(&mut self.values)?;
        Ok((step, action))
    }
    pub(crate) fn remaining(&self) -> Result<(QuadraticForm, Vec<usize>), QuadraticError> {
        let keep = self
            .active
            .iter()
            .enumerate()
            .filter_map(|(i, &active)| active.then_some(i))
            .collect::<Vec<_>>();
        let linear = keep.iter().map(|&i| self.values.linear[i]).collect();
        let mut upper = Vec::new();
        for (i, &old) in keep.iter().enumerate() {
            let mut row = BitVector::zeros(keep.len())?;
            for (j, &other) in keep.iter().enumerate().skip(i + 1) {
                row.set(j, self.adjacency[old].get(other)?)?;
            }
            upper.push(row);
        }
        Ok((QuadraticForm::new(0, linear, upper)?, keep))
    }
}

fn toggle_edge(rows: &mut [BitVector], i: usize, j: usize) -> Result<(), Gf2Error> {
    let bit = !rows[i].get(j)?;
    rows[i].set(j, bit)?;
    rows[j].set(i, bit)?;
    Ok(())
}

fn toggle_clique(rows: &mut [BitVector], vertices: &[usize]) -> Result<(), Gf2Error> {
    for (position, &i) in vertices.iter().enumerate() {
        for &j in &vertices[position + 1..] {
            toggle_edge(rows, i, j)?;
        }
    }
    Ok(())
}
