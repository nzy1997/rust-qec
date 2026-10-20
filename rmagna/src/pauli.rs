//! Packed Pauli words with exact phases and Clifford conjugation.

use crate::clifford::{CliffordCircuit, CliffordError, CliffordGate};
use crate::gf2::{BitVector, Gf2Error};
use crate::scalar::{ScalarArithmetic, ScalarError};
use crate::stabilizer::{AffineKet, StabilizerError};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PauliError {
    DimensionMismatch,
    InvalidPhase,
    Binary(Gf2Error),
    Clifford(CliffordError),
    Scalar(ScalarError),
    Stabilizer(StabilizerError),
}
impl fmt::Display for PauliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionMismatch => write!(f, "Pauli X/Z masks and ket dimensions disagree"),
            Self::InvalidPhase => {
                write!(f, "Pauli eighth-root phase must lie between zero and seven")
            }
            Self::Binary(e) => e.fmt(f),
            Self::Clifford(e) => e.fmt(f),
            Self::Scalar(e) => e.fmt(f),
            Self::Stabilizer(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for PauliError {}
impl From<Gf2Error> for PauliError {
    fn from(e: Gf2Error) -> Self {
        Self::Binary(e)
    }
}
impl From<CliffordError> for PauliError {
    fn from(e: CliffordError) -> Self {
        Self::Clifford(e)
    }
}
impl From<ScalarError> for PauliError {
    fn from(e: ScalarError) -> Self {
        Self::Scalar(e)
    }
}
impl From<StabilizerError> for PauliError {
    fn from(e: StabilizerError) -> Self {
        Self::Stabilizer(e)
    }
}

/// `omega^phase X^x Z^z`. Z acts first on a ket. A Hermitian Y on a single
/// qubit has both masks set and phase two (`i XZ = Y`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PauliWord {
    x: BitVector,
    z: BitVector,
    phase: u8,
}
impl PauliWord {
    pub fn new(x: BitVector, z: BitVector, phase: u8) -> Result<Self, PauliError> {
        if x.width() != z.width() {
            return Err(PauliError::DimensionMismatch);
        }
        if phase > 7 {
            return Err(PauliError::InvalidPhase);
        }
        Ok(Self { x, z, phase })
    }
    pub fn identity(width: usize) -> Result<Self, PauliError> {
        Self::new(BitVector::zeros(width)?, BitVector::zeros(width)?, 0)
    }
    pub fn width(&self) -> usize {
        self.x.width()
    }
    pub fn x(&self) -> &BitVector {
        &self.x
    }
    pub fn z(&self) -> &BitVector {
        &self.z
    }
    pub fn phase(&self) -> u8 {
        self.phase
    }

    /// Ordered operator product `self * right`.
    pub fn multiply(&self, right: &Self) -> Result<Self, PauliError> {
        if self.width() != right.width() {
            return Err(PauliError::DimensionMismatch);
        }
        let phase = (self.phase + right.phase + 4 * u8::from(self.z.dot(&right.x)?)) & 7;
        let mut x = self.x.clone();
        x.xor_assign(&right.x)?;
        let mut z = self.z.clone();
        z.xor_assign(&right.z)?;
        Self::new(x, z, phase)
    }

    /// Exact `gate * self * gate^dagger`, including the scalar phase.
    pub fn conjugated(&self, gate: CliffordGate) -> Result<Self, PauliError> {
        CliffordCircuit::new(self.width(), [gate])?;
        let mut result = self.clone();
        match gate {
            CliffordGate::H(q) => {
                let x = self.x.get(q)?;
                let z = self.z.get(q)?;
                result.phase = (result.phase + 4 * u8::from(x && z)) & 7;
                result.x.set(q, z)?;
                result.z.set(q, x)?;
            }
            CliffordGate::S { qubit, turns } => {
                let x = self.x.get(qubit)?;
                result.phase = (result.phase + 2 * turns * u8::from(x)) & 7;
                result
                    .z
                    .set(qubit, self.z.get(qubit)? ^ (x && turns & 1 != 0))?;
            }
            CliffordGate::X(q) => {
                result.phase = (result.phase + 4 * u8::from(self.z.get(q)?)) & 7;
            }
            CliffordGate::Z(q) => {
                result.phase = (result.phase + 4 * u8::from(self.x.get(q)?)) & 7;
            }
            CliffordGate::Cx { control, target } => {
                result
                    .x
                    .set(target, self.x.get(target)? ^ self.x.get(control)?)?;
                result
                    .z
                    .set(control, self.z.get(control)? ^ self.z.get(target)?)?;
            }
            CliffordGate::Cz { a, b } => {
                result.phase = (result.phase + 4 * u8::from(self.x.get(a)? && self.x.get(b)?)) & 7;
                result.z.set(a, self.z.get(a)? ^ self.x.get(b)?)?;
                result.z.set(b, self.z.get(b)? ^ self.x.get(a)?)?;
            }
            CliffordGate::Swap { a, b } => {
                result.x.set(a, self.x.get(b)?)?;
                result.x.set(b, self.x.get(a)?)?;
                result.z.set(a, self.z.get(b)?)?;
                result.z.set(b, self.z.get(a)?)?;
            }
        }
        Ok(result)
    }

    pub fn apply(
        &self,
        ket: &AffineKet,
        arithmetic: &ScalarArithmetic,
    ) -> Result<AffineKet, PauliError> {
        if ket.width() != self.width() {
            return Err(PauliError::DimensionMismatch);
        }
        let ket = ket.pauli(&self.x, &self.z)?;
        let coefficient = arithmetic.multiply(
            ket.coefficient(),
            &arithmetic.omega(i64::from(self.phase), 0)?,
        )?;
        Ok(AffineKet::new(
            ket.offset().clone(),
            ket.columns().to_vec(),
            ket.phase().clone(),
            coefficient,
        )?)
    }
}
