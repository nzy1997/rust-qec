use crate::Pauli;
use crate::binary::try_binary_rank;
use crate::error::{QecError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StabilizerCode {
    n: usize,
    stabilizers: Vec<Pauli>,
    stabilizer_rank: usize,
}

impl StabilizerCode {
    pub fn from_stabilizers(n: usize, stabilizers: Vec<Pauli>) -> Result<Self> {
        for stabilizer in &stabilizers {
            if stabilizer.n() != n {
                return Err(QecError::InvalidPauliWidth {
                    x_width: stabilizer.n(),
                    z_width: n,
                });
            }
        }

        for i in 0..stabilizers.len() {
            for j in (i + 1)..stabilizers.len() {
                if !stabilizers[i].try_commutes_with(&stabilizers[j])? {
                    return Err(QecError::NonCommutingStabilizers);
                }
            }
        }

        let symplectic_rows: Vec<Vec<u8>> =
            stabilizers.iter().map(Pauli::to_symplectic_row).collect();
        let stabilizer_rank = try_binary_rank(&symplectic_rows)?;

        if stabilizer_rank != stabilizers.len() {
            return Err(QecError::DependentStabilizers);
        }

        Ok(Self {
            n,
            stabilizers,
            stabilizer_rank,
        })
    }

    pub fn n(&self) -> usize {
        self.n
    }

    pub fn stabilizers(&self) -> &[Pauli] {
        &self.stabilizers
    }

    pub fn stabilizer_rows(&self) -> Vec<Vec<u8>> {
        self.stabilizers
            .iter()
            .map(Pauli::to_symplectic_row)
            .collect()
    }

    pub fn stabilizer_rank(&self) -> usize {
        self.stabilizer_rank
    }

    pub fn num_logical_qubits(&self) -> usize {
        self.n - self.stabilizer_rank
    }

    pub fn normalizer_basis(&self) -> Result<Vec<Pauli>> {
        crate::logical::compute_normalizer_basis(self)
    }

    pub fn logical_basis(&self) -> Result<crate::logical::LogicalBasis> {
        crate::logical::compute_logical_basis(self)
    }

    pub fn canonical_logical_basis(&self) -> Result<crate::logical::LogicalBasis> {
        crate::logical::compute_canonical_logical_basis(self)
    }
}
