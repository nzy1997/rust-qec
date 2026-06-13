use crate::Pauli;
use crate::code::StabilizerCode;
use crate::error::Result;
use crate::{gf2, symplectic};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalBasis {
    pub k: usize,
    pub logical_x: Vec<Pauli>,
    pub logical_z: Vec<Pauli>,
}

pub fn extract_logical_basis(code: &StabilizerCode) -> Result<LogicalBasis> {
    code.logical_basis()
}

pub fn compute_normalizer_basis(code: &StabilizerCode) -> Result<Vec<Pauli>> {
    normalizer_basis_rows(code)?
        .into_iter()
        .map(Pauli::from_symplectic_row)
        .collect()
}

pub fn compute_logical_basis(code: &StabilizerCode) -> Result<LogicalBasis> {
    compute_canonical_logical_basis(code)
}

pub fn compute_canonical_logical_basis(code: &StabilizerCode) -> Result<LogicalBasis> {
    Ok(LogicalBasis {
        k: code.num_logical_qubits(),
        logical_x: Vec::new(),
        logical_z: Vec::new(),
    })
}

fn normalizer_basis_rows(code: &StabilizerCode) -> Result<Vec<Vec<u8>>> {
    let width = 2 * code.n();
    let stabilizer_rows = code.stabilizer_rows();
    let constraints = symplectic::commutation_constraints_with_width(&stabilizer_rows, width)?;
    gf2::try_nullspace_basis_with_width(&constraints, width)
}
