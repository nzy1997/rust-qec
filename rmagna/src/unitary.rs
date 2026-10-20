//! Unitary third-hierarchy circuits lowered to Clifford boundaries and phases.

use crate::clifford::{CliffordCircuit, CliffordError, CliffordGate};
use crate::gf2::{BitVector, Gf2Error};
use crate::phase::{ClosedPhaseSum, ParityPhase, PhaseError};
use crate::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use crate::stabilizer::{AffineKet, StabilizerError};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnitaryError {
    DimensionMismatch,
    DimensionOverflow,
    InvalidTurns,
    RepeatedQubit,
    Binary(Gf2Error),
    Clifford(CliffordError),
    Phase(PhaseError),
    Scalar(ScalarError),
    Stabilizer(StabilizerError),
}
impl fmt::Display for UnitaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionMismatch => {
                write!(f, "unitary input, event, or target dimensions disagree")
            }
            Self::DimensionOverflow => {
                write!(f, "unitary network width exceeds the platform index range")
            }
            Self::InvalidTurns => write!(
                f,
                "T turns must be below eight and controlled-S turns below four"
            ),
            Self::RepeatedQubit => write!(f, "multi-qubit diagonal gate repeats a qubit"),
            Self::Binary(e) => e.fmt(f),
            Self::Clifford(e) => e.fmt(f),
            Self::Phase(e) => e.fmt(f),
            Self::Scalar(e) => e.fmt(f),
            Self::Stabilizer(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for UnitaryError {}
impl From<Gf2Error> for UnitaryError {
    fn from(e: Gf2Error) -> Self {
        Self::Binary(e)
    }
}
impl From<CliffordError> for UnitaryError {
    fn from(e: CliffordError) -> Self {
        Self::Clifford(e)
    }
}
impl From<PhaseError> for UnitaryError {
    fn from(e: PhaseError) -> Self {
        Self::Phase(e)
    }
}
impl From<ScalarError> for UnitaryError {
    fn from(e: ScalarError) -> Self {
        Self::Scalar(e)
    }
}
impl From<StabilizerError> for UnitaryError {
    fn from(e: StabilizerError) -> Self {
        Self::Stabilizer(e)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitaryGate {
    Clifford(CliffordGate),
    T { qubit: usize, turns: u8 },
    Cs { a: usize, b: usize, turns: u8 },
    Ccz { a: usize, b: usize, c: usize },
}

/// A coherent fixed-input circuit. Parsing, measurement records and stochastic
/// noise belong to the instrument layer; this type describes unitary gates.
#[derive(Clone, Debug)]
pub struct UnitaryCircuit {
    width: usize,
    gates: Vec<UnitaryGate>,
}
impl UnitaryCircuit {
    pub fn new(
        width: usize,
        gates: impl IntoIterator<Item = UnitaryGate>,
    ) -> Result<Self, UnitaryError> {
        let gates = gates.into_iter().collect::<Vec<_>>();
        for gate in &gates {
            match *gate {
                UnitaryGate::Clifford(c) => {
                    CliffordCircuit::new(width, [c])?;
                }
                UnitaryGate::T { qubit, turns } => {
                    BitVector::from_indices(width, [qubit])?;
                    if turns > 7 {
                        return Err(UnitaryError::InvalidTurns);
                    }
                }
                UnitaryGate::Cs { a, b, turns } => {
                    BitVector::from_indices(width, [a, b])?;
                    if a == b {
                        return Err(UnitaryError::RepeatedQubit);
                    }
                    if turns > 3 {
                        return Err(UnitaryError::InvalidTurns);
                    }
                }
                UnitaryGate::Ccz { a, b, c } => {
                    BitVector::from_indices(width, [a, b, c])?;
                    if a == b || a == c || b == c {
                        return Err(UnitaryError::RepeatedQubit);
                    }
                }
            }
        }
        Ok(Self { width, gates })
    }
    pub fn width(&self) -> usize {
        self.width
    }
    pub fn gates(&self) -> &[UnitaryGate] {
        &self.gates
    }

    /// Copy each diagonal parity to a fresh |0> ancilla using CNOTs. Its phase
    /// is applied only at the final scalar boundary. Summing that ancilla with
    /// an unnormalized <+| reproduces the original gate exactly, including
    /// noncommuting gates on either side. The physical state is never expanded.
    pub fn compile(
        &self,
        input: &BitVector,
        arithmetic: &ScalarArithmetic,
    ) -> Result<CompiledUnitary, UnitaryError> {
        if input.width() != self.width {
            return Err(UnitaryError::DimensionMismatch);
        }
        let mut lowered = Vec::<CliffordGate>::new();
        let mut coefficients = Vec::new();
        for &gate in &self.gates {
            match gate {
                UnitaryGate::Clifford(c) => {
                    lowered.push(c);
                    continue;
                }
                UnitaryGate::T { qubit, turns } if turns & 1 == 0 => {
                    lowered.push(CliffordGate::S {
                        qubit,
                        turns: turns / 2,
                    });
                    continue;
                }
                UnitaryGate::Cs { turns: 0, .. } => continue,
                UnitaryGate::Cs { a, b, turns: 2 } => {
                    lowered.push(CliffordGate::Cz { a, b });
                    continue;
                }
                _ => {}
            }
            for (qubits, coefficient) in diagonal_terms(gate) {
                if coefficient == 0 {
                    continue;
                }
                let target = self
                    .width
                    .checked_add(coefficients.len())
                    .ok_or(UnitaryError::DimensionOverflow)?;
                for control in qubits {
                    lowered.push(CliffordGate::Cx { control, target });
                }
                coefficients.push(coefficient);
            }
        }
        let width = self
            .width
            .checked_add(coefficients.len())
            .ok_or(UnitaryError::DimensionOverflow)?;
        let initial =
            CliffordCircuit::computational(BitVector::from_indices(width, input.ones())?)?;
        let boundary = CliffordCircuit::new(width, lowered)?
            .evolve(&initial, arithmetic)?
            .expect("unitary Clifford gates preserve a nonzero basis input");
        Ok(CompiledUnitary {
            physical_width: self.width,
            boundary,
            coefficients,
        })
    }
}

fn diagonal_terms(gate: UnitaryGate) -> Vec<(Vec<usize>, u8)> {
    match gate {
        UnitaryGate::T { qubit, turns } => vec![(vec![qubit], turns)],
        UnitaryGate::Cs { a, b, turns } => vec![
            (vec![a], turns),
            (vec![b], turns),
            (vec![a, b], (8 - turns) & 7),
        ],
        UnitaryGate::Ccz { a, b, c } => vec![
            (vec![a], 1),
            (vec![b], 1),
            (vec![c], 1),
            (vec![a, b], 7),
            (vec![a, c], 7),
            (vec![b, c], 7),
            (vec![a, b, c], 1),
        ],
        UnitaryGate::Clifford(_) => Vec::new(),
    }
}

/// An exact query result with the resource size used for this closed network.
#[derive(Clone, Debug)]
pub struct NetworkValue {
    pub value: ExactScalar,
    pub quotient_rank: usize,
    pub terms: usize,
}

/// The Clifford circuit is built once. Queries close the requested boundaries
/// before restricting the phase and constructing its computational quotient.
/// This initial compiler does not select commuting layers or reuse a prepared
/// resource between distinct queries.
pub struct CompiledUnitary {
    physical_width: usize,
    boundary: AffineKet,
    coefficients: Vec<u8>,
}
impl CompiledUnitary {
    pub fn width(&self) -> usize {
        self.physical_width
    }
    pub fn phase_legs(&self) -> usize {
        self.coefficients.len()
    }

    pub fn amplitude(
        &self,
        output: &BitVector,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        if output.width() != self.width() {
            return Err(UnitaryError::DimensionMismatch);
        }
        let total = self.boundary.width();
        let rows = (0..self.width())
            .map(|j| BitVector::from_indices(total, [j]))
            .collect::<Result<Vec<_>, _>>()?;
        let ket = self.boundary.project_z(&rows, output, arithmetic)?;
        close(
            ket,
            &[(self.width()..total).collect::<Vec<_>>()],
            std::slice::from_ref(&self.coefficients),
            max_terms,
            arithmetic,
        )
    }

    /// Exact probability of simultaneous Z-parity events on the output.
    /// Empty rows yield the total norm. Contradictory events yield exact zero.
    pub fn probability_z(
        &self,
        rows: &[BitVector],
        outcomes: &BitVector,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        if rows.len() != outcomes.width() || rows.iter().any(|r| r.width() != self.width()) {
            return Err(UnitaryError::DimensionMismatch);
        }
        let half = self.boundary.width();
        let joint = self
            .boundary
            .tensor(&self.boundary.conjugate(), arithmetic)?;
        let total = joint.width();
        let mut constraints = (0..self.width())
            .map(|j| BitVector::from_indices(total, [j, half + j]))
            .collect::<Result<Vec<_>, _>>()?;
        let mut rhs = BitVector::zeros(
            self.width()
                .checked_add(rows.len())
                .ok_or(UnitaryError::DimensionOverflow)?,
        )?;
        for (j, row) in rows.iter().enumerate() {
            constraints.push(BitVector::from_indices(total, row.ones())?);
            rhs.set(self.width() + j, outcomes.get(j)?)?;
        }
        let ket = joint.project_z(&constraints, &rhs, arithmetic)?;
        let legs = [
            (self.width()..half).collect::<Vec<_>>(),
            (half + self.width()..total).collect::<Vec<_>>(),
        ];
        let negative = self.coefficients.iter().map(|c| (8 - c) & 7).collect();
        close(
            ket,
            &legs,
            &[self.coefficients.clone(), negative],
            max_terms,
            arithmetic,
        )
    }

    /// Exact `<self|target>` for fixed pure unitary preparations.
    pub fn overlap(
        &self,
        target: &Self,
        max_terms: usize,
        arithmetic: &ScalarArithmetic,
    ) -> Result<NetworkValue, UnitaryError> {
        if target.width() != self.width() {
            return Err(UnitaryError::DimensionMismatch);
        }
        let half = self.boundary.width();
        let joint = self
            .boundary
            .conjugate()
            .tensor(&target.boundary, arithmetic)?;
        let total = joint.width();
        let rows = (0..self.width())
            .map(|j| BitVector::from_indices(total, [j, half + j]))
            .collect::<Result<Vec<_>, _>>()?;
        let ket = joint.project_z(&rows, &BitVector::zeros(self.width())?, arithmetic)?;
        let legs = [
            (self.width()..half).collect::<Vec<_>>(),
            (half + target.width()..total).collect::<Vec<_>>(),
        ];
        let negative = self.coefficients.iter().map(|c| (8 - c) & 7).collect();
        close(
            ket,
            &legs,
            &[negative, target.coefficients.clone()],
            max_terms,
            arithmetic,
        )
    }
}

fn close(
    ket: Option<AffineKet>,
    legs: &[Vec<usize>],
    coefficients: &[Vec<u8>],
    max_terms: usize,
    arithmetic: &ScalarArithmetic,
) -> Result<NetworkValue, UnitaryError> {
    let zero = || NetworkValue {
        value: ExactScalar::zero(),
        quotient_rank: 0,
        terms: 0,
    };
    let Some(ket) = ket else {
        return Ok(zero());
    };
    let keep = legs.iter().flatten().copied().collect::<Vec<_>>();
    let Some(ket) = ket.sum_boundary(&keep, arithmetic)? else {
        return Ok(zero());
    };
    let phase = ParityPhase::new(
        keep.len(),
        0,
        coefficients
            .iter()
            .flatten()
            .enumerate()
            .map(|(j, &c)| Ok((BitVector::from_indices(keep.len(), [j])?, c)))
            .collect::<Result<Vec<_>, Gf2Error>>()?,
    )?;
    let restricted = phase.restrict(ket.offset(), ket.columns())?;
    let prepared = ClosedPhaseSum::new(
        &restricted,
        ket.phase(),
        ket.coefficient(),
        max_terms,
        arithmetic,
    )?;
    Ok(NetworkValue {
        value: prepared.evaluate(arithmetic)?,
        quotient_rank: prepared.quotient_rank(),
        terms: prepared.num_terms(),
    })
}
