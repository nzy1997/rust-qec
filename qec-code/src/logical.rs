use crate::Pauli;
use crate::binary::try_in_row_span;
use crate::code::StabilizerCode;
use crate::error::{QecError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalBasis {
    pub k: usize,
    pub logical_x: Vec<Pauli>,
    pub logical_z: Vec<Pauli>,
}

pub fn extract_logical_basis(code: &StabilizerCode) -> Result<LogicalBasis> {
    let k = code.num_logical_qubits();
    if k == 0 {
        return Ok(LogicalBasis {
            k,
            logical_x: Vec::new(),
            logical_z: Vec::new(),
        });
    }
    if k > 1 {
        return Err(QecError::UnsupportedLogicalBasis { k });
    }

    let stabilizer_rows = code.stabilizer_rows();
    let mut logicals = Vec::new();

    for candidate in all_paulis(code.n())? {
        if code
            .stabilizers()
            .iter()
            .all(|stabilizer| candidate.commutes_with(stabilizer))
            && !try_in_row_span(&stabilizer_rows, &candidate.to_symplectic_row())?
        {
            logicals.push(candidate);
        }
    }

    let (logical_x, logical_z) = select_anticommuting_pair(&logicals)?;
    Ok(LogicalBasis {
        k,
        logical_x: vec![logical_x],
        logical_z: vec![logical_z],
    })
}

fn all_paulis(n: usize) -> Result<Vec<Pauli>> {
    let symplectic_bits = n
        .checked_mul(2)
        .ok_or(QecError::UnsupportedExhaustiveEnumeration { n })?;
    let total = 1usize
        .checked_shl(symplectic_bits as u32)
        .ok_or(QecError::UnsupportedExhaustiveEnumeration { n })?;
    let mut paulis = Vec::with_capacity(total.saturating_sub(1));

    for mask in 1..total {
        let mut x = vec![0; n];
        let mut z = vec![0; n];

        for qubit in 0..n {
            x[qubit] = ((mask >> qubit) & 1) as u8;
            z[qubit] = ((mask >> (n + qubit)) & 1) as u8;
        }

        paulis.push(
            Pauli::from_xz_bits(x, z).expect("generated Pauli supports must be valid binary rows"),
        );
    }

    Ok(paulis)
}

fn select_anticommuting_pair(logicals: &[Pauli]) -> Result<(Pauli, Pauli)> {
    for i in 0..logicals.len() {
        for j in (i + 1)..logicals.len() {
            if logicals[i].try_anticommutes_with(&logicals[j])? {
                return Ok((logicals[i].clone(), logicals[j].clone()));
            }
        }
    }

    Err(QecError::LogicalBasisNotFound)
}

#[cfg(test)]
mod tests {
    use super::select_anticommuting_pair;
    use crate::{Pauli, QecError};

    #[test]
    fn select_anticommuting_pair_returns_the_first_found_pair() {
        let logical_x = Pauli::from_xz_bits(vec![1, 1], vec![0, 0]).unwrap();
        let logical_z = Pauli::from_xz_bits(vec![0, 0], vec![1, 0]).unwrap();
        let commuting = Pauli::from_xz_bits(vec![0, 0], vec![0, 1]).unwrap();

        let (x, z) = select_anticommuting_pair(&[logical_x.clone(), logical_z.clone(), commuting])
            .unwrap();

        assert_eq!(x, logical_x);
        assert_eq!(z, logical_z);
    }

    #[test]
    fn select_anticommuting_pair_reports_when_no_pair_exists() {
        let x0 = Pauli::from_xz_bits(vec![1, 0], vec![0, 0]).unwrap();
        let x1 = Pauli::from_xz_bits(vec![0, 1], vec![0, 0]).unwrap();

        assert_eq!(
            select_anticommuting_pair(&[x0, x1]),
            Err(QecError::LogicalBasisNotFound)
        );
    }
}
