#![cfg(feature = "distance-ilp-highs")]

use qec_code::codes::steane::Steane;
use qec_code::distance_ilp::lower_distance_problem;
use qec_code::{Pauli, StabilizerCode};

fn pauli(n: usize, x_support: &[usize], z_support: &[usize]) -> Pauli {
    let mut x = vec![0; n];
    let mut z = vec![0; n];
    for &qubit in x_support {
        x[qubit] = 1;
    }
    for &qubit in z_support {
        z[qubit] = 1;
    }
    Pauli::from_xz_bits(x, z).unwrap()
}

#[test]
fn steane_distance_problem_has_expected_variable_shape() {
    let steane = Steane::new().unwrap();
    let lowered = lower_distance_problem(steane.code()).unwrap();

    assert_eq!(lowered.model.binary_vars.len(), 6 + 2 + 14 + 7);
    assert_eq!(lowered.model.integer_vars.len(), 14);
    assert_eq!(lowered.model.constraints.len(), 14 + 1 + (7 * 3));
    assert_eq!(lowered.model.solution_binary_prefix_len, 6 + 2 + 14 + 7);
}

#[test]
fn multi_logical_code_gets_one_nonzero_logical_constraint() {
    let code = StabilizerCode::from_stabilizers(4, vec![pauli(4, &[], &[0]), pauli(4, &[], &[1])])
        .unwrap();

    let lowered = lower_distance_problem(&code).unwrap();
    let logical_constraint = lowered
        .nonzero_logical_constraint_row
        .as_ref()
        .expect("nonzero logical row");

    assert_eq!(logical_constraint.binary_terms.len(), 4);
    assert_eq!(logical_constraint.sense, qec_ilp_core::ConstraintSense::Ge);
    assert_eq!(logical_constraint.rhs, 1.0);
}

#[test]
fn zero_logical_qubit_code_is_rejected_before_lowering() {
    let code = StabilizerCode::from_stabilizers(
        2,
        vec![
            Pauli::from_xz_bits(vec![1, 0], vec![0, 0]).unwrap(),
            Pauli::from_xz_bits(vec![0, 0], vec![0, 1]).unwrap(),
        ],
    )
    .unwrap();

    let err = lower_distance_problem(&code).unwrap_err();

    assert_eq!(err.to_string(), "distance witness not found");
}
