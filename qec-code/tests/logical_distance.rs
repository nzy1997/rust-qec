use qec_code::codes::steane::Steane;
use qec_code::distance::compute_distance;
use qec_code::logical::extract_logical_basis;
use qec_code::{Pauli, QecError, StabilizerCode};

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
fn steane_logical_basis_contains_one_anticommuting_pair() {
    let steane = Steane::new().unwrap();
    let code = steane.code();

    let basis = extract_logical_basis(code).unwrap();

    assert_eq!(basis.k, 1);
    assert_eq!(basis.logical_x.len(), 1);
    assert_eq!(basis.logical_z.len(), 1);
    assert!(basis.logical_x[0].anticommutes_with(&basis.logical_z[0]));

    for stabilizer in code.stabilizers() {
        assert!(basis.logical_x[0].commutes_with(stabilizer));
        assert!(basis.logical_z[0].commutes_with(stabilizer));
    }
}

#[test]
fn steane_distance_is_three_with_a_commuting_weight_three_witness() {
    let steane = Steane::new().unwrap();
    let code = steane.code();

    let distance = compute_distance(code).unwrap();

    assert_eq!(distance.distance, 3);
    assert_eq!(distance.witness.weight(), 3);

    for stabilizer in code.stabilizers() {
        assert!(distance.witness.commutes_with(stabilizer));
    }
}

#[test]
fn logical_basis_supports_multi_logical_codes() {
    let code = StabilizerCode::from_stabilizers(4, vec![pauli(4, &[], &[0]), pauli(4, &[], &[1])])
        .unwrap();

    let basis = extract_logical_basis(&code).unwrap();

    assert_eq!(basis.k, 2);
    assert_eq!(basis.logical_x.len(), 2);
    assert_eq!(basis.logical_z.len(), 2);
    for index in 0..basis.k {
        assert!(basis.logical_x[index].anticommutes_with(&basis.logical_z[index]));
    }
    for stabilizer in code.stabilizers() {
        for logical in basis.logical_x.iter().chain(&basis.logical_z) {
            assert!(logical.commutes_with(stabilizer));
        }
    }
}

#[test]
fn logical_basis_for_zero_logical_qubits_is_empty() {
    let code =
        StabilizerCode::from_stabilizers(1, vec![Pauli::from_xz_bits(vec![1], vec![0]).unwrap()])
            .unwrap();

    let basis = extract_logical_basis(&code).unwrap();

    assert_eq!(basis.k, 0);
    assert!(basis.logical_x.is_empty());
    assert!(basis.logical_z.is_empty());
}

#[test]
fn distance_returns_no_witness_for_zero_logical_qubit_code() {
    let code = StabilizerCode::from_stabilizers(
        2,
        vec![
            Pauli::from_xz_bits(vec![1, 0], vec![0, 0]).unwrap(),
            Pauli::from_xz_bits(vec![0, 0], vec![0, 1]).unwrap(),
        ],
    )
    .unwrap();

    assert_eq!(
        compute_distance(&code),
        Err(QecError::DistanceWitnessNotFound)
    );
}

#[test]
fn large_code_logical_basis_avoids_exhaustive_enumeration() {
    let stabilizers = (0..31)
        .map(|qubit| {
            let mut z = vec![0; 32];
            z[qubit] = 1;
            Pauli::from_xz_bits(vec![0; 32], z).unwrap()
        })
        .collect();
    let code = StabilizerCode::from_stabilizers(32, stabilizers).unwrap();

    let basis = extract_logical_basis(&code).unwrap();

    assert_eq!(basis.k, 1);
    assert_eq!(basis.logical_x.len(), 1);
    assert_eq!(basis.logical_z.len(), 1);
    assert!(basis.logical_x[0].anticommutes_with(&basis.logical_z[0]));
    for stabilizer in code.stabilizers() {
        assert!(basis.logical_x[0].commutes_with(stabilizer));
        assert!(basis.logical_z[0].commutes_with(stabilizer));
    }
    #[cfg(not(feature = "distance-ilp-highs"))]
    assert_eq!(
        compute_distance(&code),
        Err(QecError::DistanceComputationUnsupported {
            n: 32,
            reason: "enable a distance ILP feature or use a smaller code".into(),
        })
    );
}

#[cfg(feature = "distance-ilp-highs")]
#[test]
fn steane_distance_matches_ilp_path() {
    let steane = Steane::new().unwrap();

    let distance = compute_distance(steane.code()).unwrap();

    assert_eq!(distance.distance, 3);
    assert_eq!(distance.witness.weight(), 3);
}

#[cfg(feature = "distance-ilp-highs")]
#[test]
fn multi_logical_code_returns_a_nontrivial_minimum_witness() {
    let code = StabilizerCode::from_stabilizers(4, vec![pauli(4, &[], &[0]), pauli(4, &[], &[1])])
        .unwrap();

    let distance = compute_distance(&code).unwrap();

    assert_eq!(distance.distance, 1);
    assert_eq!(distance.witness.weight(), 1);
    assert!(!distance
        .witness
        .x_bits()
        .iter()
        .chain(distance.witness.z_bits())
        .all(|&bit| bit == 0));
}

#[cfg(not(feature = "distance-ilp-highs"))]
#[test]
fn large_code_without_ilp_reports_configuration_specific_unsupported_error() {
    let stabilizers = (0..31)
        .map(|qubit| {
            let mut z = vec![0; 32];
            z[qubit] = 1;
            Pauli::from_xz_bits(vec![0; 32], z).unwrap()
        })
        .collect();
    let code = StabilizerCode::from_stabilizers(32, stabilizers).unwrap();

    let err = compute_distance(&code).unwrap_err();

    assert_eq!(
        err,
        QecError::DistanceComputationUnsupported {
            n: 32,
            reason: "enable a distance ILP feature or use a smaller code".into(),
        }
    );
}
