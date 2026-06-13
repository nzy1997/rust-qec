use qec_code::binary::binary_rank;
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

fn logical_rows(code: &StabilizerCode) -> Vec<Vec<u8>> {
    let basis = code.logical_basis().unwrap();
    basis
        .logical_x
        .iter()
        .chain(&basis.logical_z)
        .map(Pauli::to_symplectic_row)
        .collect()
}

fn assert_canonical_commutation_matrix(basis: &qec_code::logical::LogicalBasis) {
    for x_index in 0..basis.k {
        for z_index in 0..basis.k {
            if x_index == z_index {
                assert!(basis.logical_x[x_index].anticommutes_with(&basis.logical_z[z_index]));
            } else {
                assert!(basis.logical_x[x_index].commutes_with(&basis.logical_z[z_index]));
            }
        }
    }

    for first in 0..basis.k {
        for second in (first + 1)..basis.k {
            assert!(basis.logical_x[first].commutes_with(&basis.logical_x[second]));
            assert!(basis.logical_z[first].commutes_with(&basis.logical_z[second]));
        }
    }
}

#[test]
fn trivial_k2_code_returns_two_canonical_logical_pairs() {
    let stabilizers = vec![pauli(4, &[], &[0]), pauli(4, &[], &[1])];
    let code = StabilizerCode::from_stabilizers(4, stabilizers).unwrap();

    let basis = code.logical_basis().unwrap();

    assert_eq!(basis.k, 2);
    assert_eq!(basis.logical_x.len(), 2);
    assert_eq!(basis.logical_z.len(), 2);
    assert_canonical_commutation_matrix(&basis);

    let mut rows = code.stabilizer_rows();
    rows.extend(logical_rows(&code));
    assert_eq!(binary_rank(&rows), 6);
}

#[test]
fn non_css_k1_code_returns_commuting_anticommuting_logicals() {
    let stabilizers = vec![pauli(2, &[0], &[0])];
    let code = StabilizerCode::from_stabilizers(2, stabilizers).unwrap();

    let basis = code.canonical_logical_basis().unwrap();

    assert_eq!(basis.k, 1);
    assert_eq!(basis.logical_x.len(), 1);
    assert_eq!(basis.logical_z.len(), 1);
    assert_canonical_commutation_matrix(&basis);
    for stabilizer in code.stabilizers() {
        assert!(basis.logical_x[0].commutes_with(stabilizer));
        assert!(basis.logical_z[0].commutes_with(stabilizer));
    }
}

#[test]
fn empty_stabilizer_code_returns_one_logical_pair_per_qubit() {
    let code = StabilizerCode::from_stabilizers(2, vec![]).unwrap();

    let basis = code.logical_basis().unwrap();

    assert_eq!(basis.k, 2);
    assert_eq!(basis.logical_x.len(), 2);
    assert_eq!(basis.logical_z.len(), 2);
    assert_canonical_commutation_matrix(&basis);
}
