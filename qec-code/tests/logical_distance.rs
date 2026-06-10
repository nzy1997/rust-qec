use qec_code::codes::steane::Steane;
use qec_code::distance::compute_distance;
use qec_code::logical::extract_logical_basis;
use qec_code::{Pauli, QecError, StabilizerCode};

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
fn logical_basis_rejects_multi_logical_codes_until_supported() {
    let code = StabilizerCode::from_stabilizers(2, vec![]).unwrap();

    assert_eq!(
        extract_logical_basis(&code),
        Err(QecError::UnsupportedLogicalBasis { k: 2 })
    );
}

#[test]
fn exhaustive_logical_and_distance_search_reject_large_codes_instead_of_panicking() {
    let stabilizers = (0..31)
        .map(|qubit| {
            let mut z = vec![0; 32];
            z[qubit] = 1;
            Pauli::from_xz_bits(vec![0; 32], z).unwrap()
        })
        .collect();
    let code = StabilizerCode::from_stabilizers(32, stabilizers).unwrap();

    assert_eq!(
        extract_logical_basis(&code),
        Err(QecError::UnsupportedExhaustiveEnumeration { n: 32 })
    );
    assert_eq!(
        compute_distance(&code),
        Err(QecError::UnsupportedExhaustiveEnumeration { n: 32 })
    );
}
