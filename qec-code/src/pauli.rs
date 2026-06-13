use crate::error::{QecError, Result};
use crate::symplectic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pauli {
    x: Vec<u8>,
    z: Vec<u8>,
}

impl Pauli {
    pub fn from_xz_bits(x: Vec<u8>, z: Vec<u8>) -> Result<Self> {
        if x.len() != z.len() {
            return Err(QecError::InvalidPauliWidth {
                x_width: x.len(),
                z_width: z.len(),
            });
        }
        validate_pauli_bits("X", &x)?;
        validate_pauli_bits("Z", &z)?;
        Ok(Self { x, z })
    }

    pub fn from_symplectic_row(row: Vec<u8>) -> Result<Self> {
        if row.len() % 2 != 0 {
            return Err(QecError::InvalidSymplecticRowWidth { width: row.len() });
        }

        let qubits = row.len() / 2;
        let (x, z) = row.split_at(qubits);
        Self::from_xz_bits(x.to_vec(), z.to_vec())
    }

    pub fn n(&self) -> usize {
        self.x.len()
    }

    pub fn x_bits(&self) -> &[u8] {
        &self.x
    }

    pub fn z_bits(&self) -> &[u8] {
        &self.z
    }

    pub fn try_symplectic_product(&self, other: &Self) -> Result<u8> {
        if self.n() != other.n() {
            return Err(QecError::InvalidPauliWidth {
                x_width: self.n(),
                z_width: other.n(),
            });
        }

        symplectic::symplectic_product(&self.to_symplectic_row(), &other.to_symplectic_row())
    }

    pub fn symplectic_product(&self, other: &Self) -> u8 {
        self.try_symplectic_product(other)
            .expect("Pauli widths must match")
    }

    pub fn weight(&self) -> usize {
        self.x
            .iter()
            .zip(&self.z)
            .filter(|(x, z)| (**x | **z) == 1)
            .count()
    }

    pub fn try_commutes_with(&self, other: &Self) -> Result<bool> {
        if self.n() != other.n() {
            return Err(QecError::InvalidPauliWidth {
                x_width: self.n(),
                z_width: other.n(),
            });
        }

        symplectic::commutes(&self.to_symplectic_row(), &other.to_symplectic_row())
    }

    pub fn commutes_with(&self, other: &Self) -> bool {
        self.try_commutes_with(other)
            .expect("Pauli widths must match")
    }

    pub fn try_anticommutes_with(&self, other: &Self) -> Result<bool> {
        Ok(!self.try_commutes_with(other)?)
    }

    pub fn anticommutes_with(&self, other: &Self) -> bool {
        self.try_anticommutes_with(other)
            .expect("Pauli widths must match")
    }

    pub fn to_symplectic_row(&self) -> Vec<u8> {
        let mut row = Vec::with_capacity(self.n() * 2);
        row.extend_from_slice(&self.x);
        row.extend_from_slice(&self.z);
        row
    }
}

fn validate_pauli_bits(which: &'static str, bits: &[u8]) -> Result<()> {
    for (index, bit) in bits.iter().enumerate() {
        if *bit > 1 {
            return Err(QecError::InvalidPauliBit {
                which,
                index,
                value: *bit,
            });
        }
    }
    Ok(())
}
