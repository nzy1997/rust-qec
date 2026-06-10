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

    for candidate in all_paulis(code.n()) {
        if code
            .stabilizers()
            .iter()
            .all(|stabilizer| candidate.commutes_with(stabilizer))
            && !try_in_row_span(&stabilizer_rows, &candidate.to_symplectic_row())?
        {
            logicals.push(candidate);
        }
    }

    for i in 0..logicals.len() {
        for j in (i + 1)..logicals.len() {
            if logicals[i].try_anticommutes_with(&logicals[j])? {
                return Ok(LogicalBasis {
                    k,
                    logical_x: vec![logicals[i].clone()],
                    logical_z: vec![logicals[j].clone()],
                });
            }
        }
    }

    Err(QecError::LogicalBasisNotFound)
}

fn all_paulis(n: usize) -> Vec<Pauli> {
    let symplectic_bits = n
        .checked_mul(2)
        .expect("symplectic width must fit in usize");
    let total = 1usize
        .checked_shl(symplectic_bits as u32)
        .expect("exhaustive Pauli enumeration requires 2n < usize::BITS");
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

    paulis
}
