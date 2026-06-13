use qec_code::binary::{in_row_span, try_binary_rank};
use qec_code::codes::steane::Steane;
use qec_code::{Pauli, QecError, StabilizerCode};

fn trivial_k_two_code() -> StabilizerCode {
    StabilizerCode::from_stabilizers(
        4,
        vec![
            Pauli::from_xz_bits(vec![1, 1, 0, 0], vec![0, 0, 0, 0]).unwrap(),
            Pauli::from_xz_bits(vec![0, 0, 0, 0], vec![1, 1, 0, 0]).unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn steane_normalizer_basis_has_expected_dimension() {
    let steane = Steane::new().unwrap();
    let basis = steane.code().normalizer_basis().unwrap();
    let basis_rows = basis
        .iter()
        .map(Pauli::to_symplectic_row)
        .collect::<Vec<_>>();

    assert_eq!(basis.len(), 8);
    assert_eq!(try_binary_rank(&basis_rows).unwrap(), basis.len());
    for operator in &basis {
        for stabilizer in steane.code().stabilizers() {
            assert!(operator.commutes_with(stabilizer));
        }
    }
}

#[test]
fn stabilizers_lie_in_the_returned_normalizer_span() {
    let code = trivial_k_two_code();
    let basis_rows = code
        .normalizer_basis()
        .unwrap()
        .into_iter()
        .map(|pauli| pauli.to_symplectic_row())
        .collect::<Vec<_>>();

    for stabilizer in code.stabilizers() {
        assert!(in_row_span(&basis_rows, &stabilizer.to_symplectic_row()));
    }
}

#[test]
fn empty_stabilizer_code_normalizer_has_full_symplectic_dimension() {
    let code = StabilizerCode::from_stabilizers(2, vec![]).unwrap();
    let basis = code.normalizer_basis().unwrap();
    let basis_rows = basis
        .iter()
        .map(Pauli::to_symplectic_row)
        .collect::<Vec<_>>();

    assert_eq!(basis.len(), 4);
    assert_eq!(try_binary_rank(&basis_rows).unwrap(), basis.len());
    assert!(basis.iter().all(|operator| operator.n() == 2));
}

#[test]
fn normalizer_basis_rejects_qubit_counts_that_overflow_symplectic_width() {
    let n = usize::MAX / 2 + 1;
    let code = StabilizerCode::from_stabilizers(n, vec![]).unwrap();

    assert_eq!(
        code.normalizer_basis(),
        Err(QecError::UnsupportedExhaustiveEnumeration { n })
    );
}
