use crate::Pauli;
use crate::binary::try_in_row_span;
use crate::code::StabilizerCode;
#[cfg(feature = "distance-ilp-highs")]
use crate::distance_exact::ExactCssDistanceSolverStatus;
use crate::distance_exact::{
    ExactCssDistanceBackend, ExactCssDistanceSolverOptions, ExactCssDistanceSolverReport,
};
use crate::error::{QecError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

#[derive(Debug, Clone, PartialEq)]
pub struct ExactCssDistanceComputation {
    pub distance: DistanceResult,
    pub solver_report: Option<ExactCssDistanceSolverReport>,
}

pub fn compute_distance(code: &StabilizerCode) -> Result<DistanceResult> {
    Ok(
        compute_distance_with_solver_options(code, ExactCssDistanceSolverOptions::default())?
            .distance,
    )
}

pub fn compute_distance_with_solver_options(
    code: &StabilizerCode,
    solver: ExactCssDistanceSolverOptions,
) -> Result<ExactCssDistanceComputation> {
    if code.num_logical_qubits() == 0 {
        return Err(QecError::DistanceWitnessNotFound);
    }

    #[cfg(feature = "distance-ilp-highs")]
    {
        compute_distance_via_ilp(code, solver)
    }

    #[cfg(not(feature = "distance-ilp-highs"))]
    {
        compute_distance_without_ilp(code, solver)
    }
}

#[cfg(feature = "distance-ilp-highs")]
fn compute_distance_via_ilp(
    code: &StabilizerCode,
    solver: ExactCssDistanceSolverOptions,
) -> Result<ExactCssDistanceComputation> {
    let lowered = crate::distance_ilp::lower_distance_problem(code)?;
    let config = qec_ilp_core::BinaryIlpConfig {
        backend: qec_ilp_core::BackendConfig {
            kind: backend_kind_to_ilp(solver.backend),
            time_limit_seconds: solver.time_limit_seconds,
            mip_gap: solver.mip_gap,
            threads: solver.threads,
            verbose: solver.verbose_solver,
        },
    };
    let mut backend = qec_ilp_core::backend::build_binary_backend(&lowered.model, &config)?;
    let backend_kind = backend.kind();
    let solution = backend.solve()?;
    let start = lowered.symplectic_var_offset;
    let end = start + code.n() * 2;
    let row = solution.binary_values[start..end]
        .iter()
        .map(|&bit| u8::from(bit))
        .collect::<Vec<_>>();
    let witness = Pauli::from_symplectic_row(row)?;
    post_validate_distance_witness(code, &witness)?;

    Ok(ExactCssDistanceComputation {
        distance: DistanceResult {
            distance: witness.weight(),
            logical_class: classify_logical(&witness),
            witness,
        },
        solver_report: Some(ExactCssDistanceSolverReport {
            backend: backend_kind_from_ilp(backend_kind),
            status: solver_status_from_ilp(solution.status),
        }),
    })
}

#[cfg(not(feature = "distance-ilp-highs"))]
fn compute_distance_without_ilp(
    code: &StabilizerCode,
    solver: ExactCssDistanceSolverOptions,
) -> Result<ExactCssDistanceComputation> {
    if solver != ExactCssDistanceSolverOptions::default() {
        if solver.backend == ExactCssDistanceBackend::Gurobi {
            return Err(QecError::IlpBackendUnavailable("Gurobi".into()));
        }
        return Err(QecError::DistanceComputationUnsupported {
            n: code.n(),
            reason: "solver options require an ILP-enabled build".into(),
        });
    }
    Ok(ExactCssDistanceComputation {
        distance: compute_distance_via_exhaustive_search(code)?,
        solver_report: None,
    })
}

#[cfg(not(feature = "distance-ilp-highs"))]
fn compute_distance_via_exhaustive_search(code: &StabilizerCode) -> Result<DistanceResult> {
    validate_exhaustive_search_width(code.n())?;
    for weight in 1..=code.n() {
        if let Some(witness) = find_normalizer_witness_of_weight(code, weight)? {
            return Ok(DistanceResult {
                distance: weight,
                logical_class: classify_logical(&witness),
                witness,
            });
        }
    }
    Err(QecError::DistanceWitnessNotFound)
}

#[cfg(not(feature = "distance-ilp-highs"))]
fn validate_exhaustive_search_width(n: usize) -> Result<()> {
    let symplectic_bits = n
        .checked_mul(2)
        .ok_or(QecError::DistanceComputationUnsupported {
            n,
            reason: "enable a distance ILP feature or use a smaller code".into(),
        })?;
    let _ = 1usize.checked_shl(symplectic_bits as u32).ok_or(
        QecError::DistanceComputationUnsupported {
            n,
            reason: "enable a distance ILP feature or use a smaller code".into(),
        },
    )?;
    Ok(())
}

#[cfg(not(feature = "distance-ilp-highs"))]
fn find_normalizer_witness_of_weight(
    code: &StabilizerCode,
    weight: usize,
) -> Result<Option<Pauli>> {
    let stabilizer_rows = code.stabilizer_rows();
    let mut support = Vec::with_capacity(weight);
    search_supports(code, &stabilizer_rows, weight, 0, &mut support)
}

#[cfg(not(feature = "distance-ilp-highs"))]
fn search_supports(
    code: &StabilizerCode,
    stabilizer_rows: &[Vec<u8>],
    target_weight: usize,
    next_qubit: usize,
    support: &mut Vec<usize>,
) -> Result<Option<Pauli>> {
    if support.len() == target_weight {
        let mut x = vec![0; code.n()];
        let mut z = vec![0; code.n()];
        return search_pauli_assignments(code, stabilizer_rows, support, 0, &mut x, &mut z);
    }

    let remaining = target_weight - support.len();
    let max_qubit = code.n() - remaining;
    for qubit in next_qubit..=max_qubit {
        support.push(qubit);
        if let Some(witness) =
            search_supports(code, stabilizer_rows, target_weight, qubit + 1, support)?
        {
            return Ok(Some(witness));
        }
        support.pop();
    }
    Ok(None)
}

#[cfg(not(feature = "distance-ilp-highs"))]
fn search_pauli_assignments(
    code: &StabilizerCode,
    stabilizer_rows: &[Vec<u8>],
    support: &[usize],
    support_index: usize,
    x: &mut [u8],
    z: &mut [u8],
) -> Result<Option<Pauli>> {
    if support_index == support.len() {
        let candidate = Pauli::from_xz_bits(x.to_vec(), z.to_vec())?;
        return if is_nontrivial_normalizer_witness(code, stabilizer_rows, &candidate)? {
            Ok(Some(candidate))
        } else {
            Ok(None)
        };
    }

    let qubit = support[support_index];
    for (x_bit, z_bit) in [(1, 0), (0, 1), (1, 1)] {
        x[qubit] = x_bit;
        z[qubit] = z_bit;
        if let Some(witness) =
            search_pauli_assignments(code, stabilizer_rows, support, support_index + 1, x, z)?
        {
            return Ok(Some(witness));
        }
    }
    x[qubit] = 0;
    z[qubit] = 0;
    Ok(None)
}

#[cfg(not(feature = "distance-ilp-highs"))]
fn is_nontrivial_normalizer_witness(
    code: &StabilizerCode,
    stabilizer_rows: &[Vec<u8>],
    candidate: &Pauli,
) -> Result<bool> {
    Ok(code
        .stabilizers()
        .iter()
        .all(|stabilizer| candidate.commutes_with(stabilizer))
        && !try_in_row_span(stabilizer_rows, &candidate.to_symplectic_row())?)
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
fn backend_kind_to_ilp(kind: ExactCssDistanceBackend) -> qec_ilp_core::BackendKind {
    match kind {
        ExactCssDistanceBackend::Auto => qec_ilp_core::BackendKind::Auto,
        ExactCssDistanceBackend::Highs => qec_ilp_core::BackendKind::Highs,
        ExactCssDistanceBackend::Gurobi => qec_ilp_core::BackendKind::Gurobi,
    }
}

#[cfg(feature = "distance-ilp-highs")]
fn backend_kind_from_ilp(kind: qec_ilp_core::BackendKind) -> ExactCssDistanceBackend {
    match kind {
        qec_ilp_core::BackendKind::Auto => ExactCssDistanceBackend::Auto,
        qec_ilp_core::BackendKind::Highs => ExactCssDistanceBackend::Highs,
        qec_ilp_core::BackendKind::Gurobi => ExactCssDistanceBackend::Gurobi,
    }
}

#[cfg(feature = "distance-ilp-highs")]
fn solver_status_from_ilp(
    status: qec_ilp_core::model::ModelSolutionStatus,
) -> ExactCssDistanceSolverStatus {
    match status {
        qec_ilp_core::model::ModelSolutionStatus::Optimal => ExactCssDistanceSolverStatus::Optimal,
        qec_ilp_core::model::ModelSolutionStatus::TimeLimit => {
            ExactCssDistanceSolverStatus::TimeLimit
        }
        qec_ilp_core::model::ModelSolutionStatus::SolutionLimit => {
            ExactCssDistanceSolverStatus::SolutionLimit
        }
        qec_ilp_core::model::ModelSolutionStatus::SubOptimal => {
            ExactCssDistanceSolverStatus::SubOptimal
        }
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

    if witness.weight() == 0 {
        return Err(QecError::IlpInfeasible);
    }

    if try_in_row_span(&code.stabilizer_rows(), &witness.to_symplectic_row())? {
        return Err(QecError::IlpSolveFailed(
            "returned witness lies in stabilizer span".into(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{LogicalClass, classify_logical};
    use crate::Pauli;
    #[cfg(feature = "distance-ilp-highs")]
    use crate::{QecError, StabilizerCode};

    #[cfg(feature = "distance-ilp-highs")]
    fn single_qubit_z_stabilizer_code() -> StabilizerCode {
        StabilizerCode::from_stabilizers(1, vec![Pauli::from_xz_bits(vec![0], vec![1]).unwrap()])
            .unwrap()
    }

    #[test]
    fn classify_logical_distinguishes_x_z_and_mixed_supports() {
        let x_like = Pauli::from_xz_bits(vec![1, 0], vec![0, 0]).unwrap();
        let z_like = Pauli::from_xz_bits(vec![0, 0], vec![0, 1]).unwrap();
        let mixed = Pauli::from_xz_bits(vec![0, 1], vec![0, 1]).unwrap();

        assert_eq!(classify_logical(&x_like), LogicalClass::XLike);
        assert_eq!(classify_logical(&z_like), LogicalClass::ZLike);
        assert_eq!(classify_logical(&mixed), LogicalClass::Mixed);
    }

    #[cfg(feature = "distance-ilp-highs")]
    #[test]
    fn post_validate_distance_witness_rejects_non_commuting_witnesses() {
        let code = single_qubit_z_stabilizer_code();
        let witness = Pauli::from_xz_bits(vec![1], vec![0]).unwrap();

        assert_eq!(
            super::post_validate_distance_witness(&code, &witness),
            Err(QecError::IlpSolveFailed(
                "returned witness does not commute with stabilizers".into(),
            ))
        );
    }

    #[cfg(feature = "distance-ilp-highs")]
    #[test]
    fn post_validate_distance_witness_rejects_stabilizer_span_elements() {
        let code = single_qubit_z_stabilizer_code();
        let witness = Pauli::from_xz_bits(vec![0], vec![1]).unwrap();

        assert_eq!(
            super::post_validate_distance_witness(&code, &witness),
            Err(QecError::IlpSolveFailed(
                "returned witness lies in stabilizer span".into(),
            ))
        );
    }

    #[cfg(feature = "distance-ilp-highs")]
    #[test]
    fn post_validate_distance_witness_rejects_zero_weight_witnesses() {
        let code = StabilizerCode::from_stabilizers(1, vec![]).unwrap();
        let witness = Pauli::from_xz_bits(vec![0], vec![0]).unwrap();

        assert_eq!(
            super::post_validate_distance_witness(&code, &witness),
            Err(QecError::IlpInfeasible)
        );
    }
}
