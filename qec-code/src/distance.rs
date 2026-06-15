use crate::binary::try_in_row_span;
use crate::code::StabilizerCode;
use crate::error::{QecError, Result};
use crate::Pauli;

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
    if code.num_logical_qubits() == 0 {
        return Err(QecError::DistanceWitnessNotFound);
    }

    #[cfg(feature = "distance-ilp-highs")]
    {
        compute_distance_via_ilp(code)
    }

    #[cfg(not(feature = "distance-ilp-highs"))]
    {
        compute_distance_via_exhaustive_search(code)
    }
}

#[cfg(feature = "distance-ilp-highs")]
fn compute_distance_via_ilp(code: &StabilizerCode) -> Result<DistanceResult> {
    let lowered = crate::distance_ilp::lower_distance_problem(code)?;
    let mut backend = qec_ilp_core::backend::build_binary_backend(
        &lowered.model,
        &qec_ilp_core::BinaryIlpConfig::default(),
    )?;
    let solution = backend.solve()?;
    let start = lowered.symplectic_var_offset;
    let end = start + code.n() * 2;
    let row = solution.binary_values[start..end]
        .iter()
        .map(|&bit| u8::from(bit))
        .collect::<Vec<_>>();
    let witness = Pauli::from_symplectic_row(row)?;
    post_validate_distance_witness(code, &witness)?;

    Ok(DistanceResult {
        distance: witness.weight(),
        logical_class: classify_logical(&witness),
        witness,
    })
}

#[cfg(not(feature = "distance-ilp-highs"))]
fn compute_distance_via_exhaustive_search(code: &StabilizerCode) -> Result<DistanceResult> {
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

#[cfg(not(feature = "distance-ilp-highs"))]
fn all_normalizer_candidates(code: &StabilizerCode) -> Result<Vec<Pauli>> {
    let n = code.n();
    let symplectic_bits = n
        .checked_mul(2)
        .ok_or(QecError::DistanceComputationUnsupported {
            n,
            reason: "enable a distance ILP feature or use a smaller code".into(),
        })?;
    let total = 1usize.checked_shl(symplectic_bits as u32).ok_or(
        QecError::DistanceComputationUnsupported {
            n,
            reason: "enable a distance ILP feature or use a smaller code".into(),
        },
    )?;
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

#[cfg(feature = "distance-ilp-highs")]
fn post_validate_distance_witness(code: &StabilizerCode, witness: &Pauli) -> Result<()> {
    if !code
        .stabilizers()
        .iter()
        .all(|stabilizer| witness.commutes_with(stabilizer))
    {
        return Err(QecError::IlpSolveFailed(
            "returned witness does not commute with stabilizers".into(),
        ));
    }

    if try_in_row_span(&code.stabilizer_rows(), &witness.to_symplectic_row())? {
        return Err(QecError::IlpSolveFailed(
            "returned witness lies in stabilizer span".into(),
        ));
    }

    if witness.weight() == 0 {
        return Err(QecError::IlpInfeasible);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{classify_logical, LogicalClass};
    use crate::Pauli;

    #[test]
    fn classify_logical_distinguishes_x_z_and_mixed_supports() {
        let x_like = Pauli::from_xz_bits(vec![1, 0], vec![0, 0]).unwrap();
        let z_like = Pauli::from_xz_bits(vec![0, 0], vec![0, 1]).unwrap();
        let mixed = Pauli::from_xz_bits(vec![0, 1], vec![0, 1]).unwrap();

        assert_eq!(classify_logical(&x_like), LogicalClass::XLike);
        assert_eq!(classify_logical(&z_like), LogicalClass::ZLike);
        assert_eq!(classify_logical(&mixed), LogicalClass::Mixed);
    }
}
