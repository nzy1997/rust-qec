use qec_code::codes::steane::Steane;
use qec_code::css::CssCode;
use qec_code::{Pauli, QecError, StabilizerCode};

#[test]
fn stabilizer_code_rejects_noncommuting_generators() {
    let x0 = Pauli::from_xz_bits(vec![1], vec![0]).unwrap();
    let z0 = Pauli::from_xz_bits(vec![0], vec![1]).unwrap();

    assert_eq!(
        StabilizerCode::from_stabilizers(1, vec![x0, z0]),
        Err(QecError::NonCommutingStabilizers)
    );
}

#[test]
fn stabilizer_code_rejects_dependent_commuting_generators() {
    let x0 = Pauli::from_xz_bits(vec![1], vec![0]).unwrap();
    let duplicate_x0 = Pauli::from_xz_bits(vec![1], vec![0]).unwrap();

    assert_eq!(
        StabilizerCode::from_stabilizers(1, vec![x0, duplicate_x0]),
        Err(QecError::DependentStabilizers)
    );
}

#[test]
fn css_code_rejects_non_orthogonal_checks() {
    assert_eq!(
        CssCode::from_hx_hz(vec![vec![1]], vec![vec![1]]),
        Err(QecError::InvalidCssOrthogonality)
    );
}

#[test]
fn css_code_rejects_ragged_row_widths() {
    assert_eq!(
        CssCode::from_hx_hz(vec![vec![1, 0], vec![1]], vec![]),
        Err(QecError::RowWidthMismatch {
            expected: 2,
            actual: 1,
        })
    );
    assert_eq!(
        CssCode::from_hx_hz(vec![], vec![vec![1, 0], vec![0]]),
        Err(QecError::RowWidthMismatch {
            expected: 2,
            actual: 1,
        })
    );
}

#[test]
fn css_code_rejects_non_binary_matrix_entries() {
    assert_eq!(
        CssCode::from_hx_hz(vec![vec![2]], vec![]),
        Err(QecError::InvalidBinaryEntry {
            row: 0,
            col: 0,
            value: 2,
        })
    );
    assert_eq!(
        CssCode::from_hx_hz(vec![], vec![vec![3]]),
        Err(QecError::InvalidBinaryEntry {
            row: 0,
            col: 0,
            value: 3,
        })
    );
}

#[test]
fn steane_exposes_expected_invariants() {
    let steane = Steane::new().unwrap();
    let code = steane.code();

    assert_eq!(code.n(), 7);
    assert_eq!(code.stabilizer_rank(), 6);
    assert_eq!(code.num_logical_qubits(), 1);
    assert_eq!(code.stabilizers().len(), 6);
}
