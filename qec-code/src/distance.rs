use crate::Pauli;
use crate::binary::try_in_row_span;
use crate::code::StabilizerCode;
use crate::error::{QecError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalClass {
    XLike,
    ZLike,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistanceResult {
    pub distance: usize,
    pub witness: Pauli,
    pub logical_class: LogicalClass,
}

pub fn compute_distance(code: &StabilizerCode) -> Result<DistanceResult> {
    let mut best_witness: Option<Pauli> = None;

    for candidate in all_normalizer_candidates(code)? {
        let replace = match &best_witness {
            Some(current) => candidate.weight() < current.weight(),
            None => true,
        };

        if replace {
            best_witness = Some(candidate);
        }
    }

    let witness = best_witness.ok_or(QecError::DistanceWitnessNotFound)?;

    Ok(DistanceResult {
        distance: witness.weight(),
        logical_class: classify_logical(&witness),
        witness,
    })
}

fn all_normalizer_candidates(code: &StabilizerCode) -> Result<Vec<Pauli>> {
    let n = code.n();
    let symplectic_bits = n
        .checked_mul(2)
        .ok_or(QecError::UnsupportedExhaustiveEnumeration { n })?;
    let total = 1usize
        .checked_shl(symplectic_bits as u32)
        .ok_or(QecError::UnsupportedExhaustiveEnumeration { n })?;
    let stabilizer_rows = code.stabilizer_rows();
    let mut candidates = Vec::new();

    for mask in 1..total {
        let mut x = vec![0; n];
        let mut z = vec![0; n];

        for qubit in 0..n {
            x[qubit] = ((mask >> qubit) & 1) as u8;
            z[qubit] = ((mask >> (n + qubit)) & 1) as u8;
        }

        let candidate =
            Pauli::from_xz_bits(x, z).expect("generated Pauli supports must be valid binary rows");

        if code
            .stabilizers()
            .iter()
            .all(|stabilizer| candidate.commutes_with(stabilizer))
            && !try_in_row_span(&stabilizer_rows, &candidate.to_symplectic_row())?
        {
            candidates.push(candidate);
        }
    }

    Ok(candidates)
}

fn classify_logical(pauli: &Pauli) -> LogicalClass {
    let has_x = pauli.x_bits().contains(&1);
    let has_z = pauli.z_bits().contains(&1);

    match (has_x, has_z) {
        (true, false) => LogicalClass::XLike,
        (false, true) => LogicalClass::ZLike,
        (true, true) => LogicalClass::Mixed,
        (false, false) => unreachable!("logical witnesses are non-identity"),
    }
}
