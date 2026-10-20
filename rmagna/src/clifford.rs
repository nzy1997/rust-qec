//! Exact Clifford circuits on affine quadratic kets.

use crate::gf2::{BitVector, Gf2Error, transpose_columns};
use crate::quadratic::{QuadraticError, QuadraticForm};
use crate::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use crate::stabilizer::{AffineKet, StabilizerError};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CliffordError {
    QubitOutOfBounds { qubit: usize, width: usize },
    RepeatedQubit { qubit: usize },
    InvalidTurns,
    DimensionMismatch,
    DimensionOverflow,
    InvalidCanonicalSupport,
    Binary(Gf2Error),
    Quadratic(QuadraticError),
    Scalar(ScalarError),
    Stabilizer(StabilizerError),
}
impl fmt::Display for CliffordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QubitOutOfBounds { qubit, width } => {
                write!(f, "Clifford qubit {qubit} is outside width {width}")
            }
            Self::RepeatedQubit { qubit } => {
                write!(f, "two-qubit Clifford gate repeats qubit {qubit}")
            }
            Self::InvalidTurns => write!(f, "S-gate turns must lie between zero and three"),
            Self::DimensionMismatch => write!(f, "Clifford circuit and input ket widths disagree"),
            Self::DimensionOverflow => {
                write!(f, "Clifford path count exceeds the platform index range")
            }
            Self::InvalidCanonicalSupport => write!(
                f,
                "Hadamard requires independent input support columns; deleting one row must lose at most one rank"
            ),
            Self::Binary(e) => e.fmt(f),
            Self::Quadratic(e) => e.fmt(f),
            Self::Scalar(e) => e.fmt(f),
            Self::Stabilizer(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for CliffordError {}
impl From<Gf2Error> for CliffordError {
    fn from(e: Gf2Error) -> Self {
        Self::Binary(e)
    }
}
impl From<QuadraticError> for CliffordError {
    fn from(e: QuadraticError) -> Self {
        Self::Quadratic(e)
    }
}
impl From<ScalarError> for CliffordError {
    fn from(e: ScalarError) -> Self {
        Self::Scalar(e)
    }
}
impl From<StabilizerError> for CliffordError {
    fn from(e: StabilizerError) -> Self {
        Self::Stabilizer(e)
    }
}

/// Unitary conventions: `H = (X + Z)/sqrt(2)` and `S = diag(1,i)`.
/// S turns are modulo four; all qubits and distinct endpoints are validated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliffordGate {
    H(usize),
    S { qubit: usize, turns: u8 },
    X(usize),
    Z(usize),
    Cx { control: usize, target: usize },
    Cz { a: usize, b: usize },
    Swap { a: usize, b: usize },
}

impl CliffordGate {
    fn validate(&self, width: usize) -> Result<(), CliffordError> {
        let single = |qubit| {
            if qubit < width {
                Ok(())
            } else {
                Err(CliffordError::QubitOutOfBounds { qubit, width })
            }
        };
        let pair = |a, b| {
            single(a)?;
            single(b)?;
            if a == b {
                Err(CliffordError::RepeatedQubit { qubit: a })
            } else {
                Ok(())
            }
        };
        match *self {
            Self::H(q) | Self::X(q) | Self::Z(q) => single(q),
            Self::S { qubit, turns } => {
                single(qubit)?;
                if turns > 3 {
                    Err(CliffordError::InvalidTurns)
                } else {
                    Ok(())
                }
            }
            Self::Cx { control, target } => pair(control, target),
            Self::Cz { a, b } | Self::Swap { a, b } => pair(a, b),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CliffordCircuit {
    width: usize,
    gates: Vec<CliffordGate>,
}
impl CliffordCircuit {
    pub fn new(
        width: usize,
        gates: impl IntoIterator<Item = CliffordGate>,
    ) -> Result<Self, CliffordError> {
        let gates = gates.into_iter().collect::<Vec<_>>();
        for gate in &gates {
            gate.validate(width)?;
        }
        Ok(Self { width, gates })
    }
    pub fn width(&self) -> usize {
        self.width
    }
    pub fn gates(&self) -> &[CliffordGate] {
        &self.gates
    }

    pub fn adjoint(&self) -> Self {
        let gates = self
            .gates
            .iter()
            .rev()
            .map(|&gate| match gate {
                CliffordGate::S { qubit, turns } => CliffordGate::S {
                    qubit,
                    turns: (4 - turns) & 3,
                },
                other => other,
            })
            .collect();
        Self {
            width: self.width,
            gates,
        }
    }

    pub fn computational(value: BitVector) -> Result<AffineKet, CliffordError> {
        Ok(AffineKet::new(
            value,
            Vec::new(),
            QuadraticForm::new(0, Vec::new(), Vec::new())?,
            ExactScalar::one(),
        )?)
    }

    /// Evolve an unnormalized ket. Global phases and its initial scalar remain.
    /// Zero input/interference returns `None`. Hadamard paths are contracted
    /// after each gate, so their number stays bounded by the boundary width.
    pub fn evolve(
        &self,
        input: &AffineKet,
        arithmetic: &ScalarArithmetic,
    ) -> Result<Option<AffineKet>, CliffordError> {
        if input.width() != self.width {
            return Err(CliffordError::DimensionMismatch);
        }
        let Some(mut ket) = input.contract(arithmetic)? else {
            return Ok(None);
        };
        for &gate in &self.gates {
            let Some(next) = apply(gate, &ket, arithmetic)? else {
                return Ok(None);
            };
            ket = next;
        }
        Ok(Some(ket))
    }
}

fn add_phase(left: &QuadraticForm, right: &QuadraticForm) -> Result<QuadraticForm, CliffordError> {
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

fn apply(
    gate: CliffordGate,
    ket: &AffineKet,
    arithmetic: &ScalarArithmetic,
) -> Result<Option<AffineKet>, CliffordError> {
    let mut offset = ket.offset().clone();
    let mut columns = ket.columns().to_vec();
    let mut phase = ket.phase().clone();
    let mut coefficient = ket.coefficient().clone();
    match gate {
        CliffordGate::X(qubit) => {
            let bit = offset.get(qubit)?;
            offset.set(qubit, !bit)?;
        }
        CliffordGate::Z(qubit) => {
            return Ok(Some(ket.pauli(
                &BitVector::zeros(ket.width())?,
                &BitVector::from_indices(ket.width(), [qubit])?,
            )?));
        }
        CliffordGate::Cx { control, target } => {
            for vector in std::iter::once(&mut offset).chain(&mut columns) {
                vector.set(target, vector.get(target)? ^ vector.get(control)?)?;
            }
        }
        CliffordGate::Swap { a, b } => {
            for vector in std::iter::once(&mut offset).chain(&mut columns) {
                let left = vector.get(a)?;
                let right = vector.get(b)?;
                vector.set(a, right)?;
                vector.set(b, left)?;
            }
        }
        CliffordGate::S { qubit, turns } => {
            let one = QuadraticForm::new(0, vec![turns], vec![BitVector::zeros(1)?])?;
            let shift = BitVector::from_indices(1, offset.get(qubit)?.then_some(0))?;
            let basis = columns
                .iter()
                .map(|c| BitVector::from_indices(1, c.get(qubit)?.then_some(0)))
                .collect::<Result<Vec<_>, Gf2Error>>()?;
            phase = add_phase(&phase, &one.restrict(&shift, &basis)?)?;
        }
        CliffordGate::Cz { a, b } => {
            let n = columns.len();
            let left = columns
                .iter()
                .map(|c| c.get(a))
                .collect::<Result<Vec<_>, _>>()?;
            let right = columns
                .iter()
                .map(|c| c.get(b))
                .collect::<Result<Vec<_>, _>>()?;
            let a0 = offset.get(a)?;
            let b0 = offset.get(b)?;
            let linear = (0..n)
                .map(|i| 2 * u8::from((a0 && right[i]) ^ (b0 && left[i]) ^ (left[i] && right[i])))
                .collect();
            let mut upper = vec![BitVector::zeros(n)?; n];
            for (i, row) in upper.iter_mut().enumerate() {
                for j in i + 1..n {
                    row.set(j, (left[i] && right[j]) ^ (left[j] && right[i]))?;
                }
            }
            phase = add_phase(
                &phase,
                &QuadraticForm::new(2 * u8::from(a0 && b0), linear, upper)?,
            )?;
        }
        CliffordGate::H(qubit) => {
            // evolve canonicalizes the input, and invertible Clifford label
            // maps preserve independence. If this row is fixed, clearing it
            // changes no column; the new unit column is independent of all old
            // columns. There are no hidden paths to eliminate in this case.
            let deterministic = columns
                .iter()
                .all(|column| !column.get(qubit).expect("validated qubit"));
            let kernel = if deterministic {
                None
            } else {
                deleted_row_kernel(&columns, qubit)?
            };
            let n = columns.len();
            let paths = n.checked_add(1).ok_or(CliffordError::DimensionOverflow)?;
            let mut upper = phase
                .upper()
                .iter()
                .map(|row| BitVector::from_indices(paths, row.ones()))
                .collect::<Result<Vec<_>, _>>()?;
            for (i, column) in columns.iter_mut().enumerate() {
                upper[i].set(n, column.get(qubit)?)?;
                column.set(qubit, false)?;
            }
            upper.push(BitVector::zeros(paths)?);
            let mut linear = phase.linear().to_vec();
            linear.push(2 * u8::from(offset.get(qubit)?));
            offset.set(qubit, false)?;
            columns.push(BitVector::from_indices(ket.width(), [qubit])?);
            phase = QuadraticForm::new(phase.constant(), linear, upper)?;
            coefficient = arithmetic.multiply(&coefficient, &arithmetic.omega(0, -1)?)?;
            let next = AffineKet::new(offset, columns, phase, coefficient)?;
            return if let Some(kernel) = kernel {
                // A*v = e_qubit before clearing the row. In the new matrix,
                // (v,0) is its unique hidden direction. Removing any original
                // column in v leaves an independent image basis; the new unit
                // column cannot belong to the cleared old-column span.
                let removed = kernel
                    .ones()
                    .next()
                    .ok_or(CliffordError::InvalidCanonicalSupport)?;
                let direction = BitVector::from_indices(paths, kernel.ones())?;
                let pivots = (0..paths).filter(|&j| j != removed).collect::<Vec<_>>();
                Ok(next.contract_with_basis(&pivots, &[direction], arithmetic)?)
            } else {
                Ok(Some(next))
            };
        }
    }
    Ok(Some(AffineKet::new(offset, columns, phase, coefficient)?))
}

/// Nullspace of canonical support after removing one boundary row. Streaming
/// packed row reduction needs no equation provenance and stops at full rank.
fn deleted_row_kernel(
    columns: &[BitVector],
    qubit: usize,
) -> Result<Option<BitVector>, CliffordError> {
    let count = columns.len();
    let Some(first) = columns.first() else {
        return Ok(None);
    };
    let mut echelon = BTreeMap::<usize, BitVector>::new();
    for (index, mut row) in transpose_columns(first.width(), columns)?
        .into_iter()
        .enumerate()
    {
        if index == qubit {
            continue;
        }
        loop {
            let Some(pivot) = row.ones().next() else {
                break;
            };
            if let Some(previous) = echelon.get(&pivot) {
                row.xor_assign(previous)?;
            } else {
                echelon.insert(pivot, row);
                break;
            }
        }
        if echelon.len() == count {
            return Ok(None);
        }
    }
    if echelon.len().checked_add(1) != Some(count) {
        return Err(CliffordError::InvalidCanonicalSupport);
    }
    let free = (0..count)
        .find(|j| !echelon.contains_key(j))
        .ok_or(CliffordError::InvalidCanonicalSupport)?;
    let mut kernel = BitVector::from_indices(count, [free])?;
    for (&pivot, row) in echelon.iter().rev() {
        kernel.set(pivot, row.dot(&kernel)?)?;
    }
    Ok(Some(kernel))
}
