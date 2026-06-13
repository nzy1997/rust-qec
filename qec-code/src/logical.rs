use crate::Pauli;
use crate::code::StabilizerCode;
use crate::error::{QecError, Result};
use crate::gf2::BinaryRow;
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
    let k = code.num_logical_qubits();
    let logical_rows = logical_quotient_rows(code)?;
    let pairs = symplectic::symplectic_gram_schmidt(&logical_rows)?;

    if pairs.len() != k {
        return Err(QecError::LogicalBasisNotFound);
    }

    let mut logical_x = Vec::with_capacity(k);
    let mut logical_z = Vec::with_capacity(k);
    for (x_like, z_like) in pairs {
        logical_x.push(Pauli::from_symplectic_row(x_like)?);
        logical_z.push(Pauli::from_symplectic_row(z_like)?);
    }

    Ok(LogicalBasis {
        k,
        logical_x,
        logical_z,
    })
}

fn logical_quotient_rows(code: &StabilizerCode) -> Result<Vec<BinaryRow>> {
    let width = symplectic_width(code)?;
    let target_count = code
        .num_logical_qubits()
        .checked_mul(2)
        .ok_or(QecError::UnsupportedExhaustiveEnumeration { n: code.n() })?;
    let mut span_rows = code.stabilizer_rows();
    gf2::validate_rows_with_width(&span_rows, width)?;

    let mut logical_rows = Vec::with_capacity(target_count);
    for row in normalizer_basis_rows(code)? {
        if gf2::try_in_row_span_with_width(&span_rows, width, &row)? {
            continue;
        }

        span_rows.push(row.clone());
        logical_rows.push(row);
        if logical_rows.len() == target_count {
            return Ok(logical_rows);
        }
    }

    if logical_rows.len() == target_count {
        Ok(logical_rows)
    } else {
        Err(QecError::LogicalBasisNotFound)
    }
}

fn normalizer_basis_rows(code: &StabilizerCode) -> Result<Vec<BinaryRow>> {
    let width = symplectic_width(code)?;
    let stabilizer_rows = code.stabilizer_rows();
    let constraints = symplectic::commutation_constraints_with_width(&stabilizer_rows, width)?;
    gf2::try_nullspace_basis_with_width(&constraints, width)
}

fn symplectic_width(code: &StabilizerCode) -> Result<usize> {
    let n = code.n();
    n.checked_mul(2)
        .ok_or(QecError::UnsupportedExhaustiveEnumeration { n })
}
