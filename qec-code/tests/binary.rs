use qec_code::Pauli;
use qec_code::QecError;
use qec_code::binary::{
    binary_rank, eliminate_to_row_echelon, in_row_span, try_binary_rank,
    try_eliminate_to_row_echelon, try_in_row_span,
};

#[test]
fn binary_rank_ignores_dependent_rows() {
    let matrix = vec![vec![1, 0, 1, 1], vec![0, 1, 1, 0], vec![1, 1, 0, 1]];

    assert_eq!(binary_rank(&matrix), 2);
}

#[test]
fn in_row_span_detects_membership_and_absence() {
    let matrix = vec![vec![1, 0, 1, 0], vec![0, 1, 1, 1]];

    assert!(in_row_span(&matrix, &[1, 1, 0, 1]));
    assert!(!in_row_span(&matrix, &[1, 0, 0, 0]));
}

#[test]
fn in_row_span_for_empty_generator_set_only_contains_zero_vector() {
    let matrix: Vec<Vec<u8>> = vec![];

    assert!(in_row_span(&matrix, &[]));
    assert!(in_row_span(&matrix, &[0, 0, 0]));
    assert!(!in_row_span(&matrix, &[0, 1, 0]));
}

#[test]
fn pauli_from_xz_bits_rejects_non_binary_values() {
    assert_eq!(
        Pauli::from_xz_bits(vec![1, 0], vec![0]),
        Err(QecError::InvalidPauliWidth {
            x_width: 2,
            z_width: 1,
        })
    );
    assert_eq!(
        Pauli::from_xz_bits(vec![2, 0], vec![0, 1]),
        Err(QecError::InvalidPauliBit {
            which: "X",
            index: 0,
            value: 2,
        })
    );
    assert_eq!(
        Pauli::from_xz_bits(vec![1, 0], vec![0, 3]),
        Err(QecError::InvalidPauliBit {
            which: "Z",
            index: 1,
            value: 3,
        })
    );
}

#[test]
fn pauli_symplectic_rows_round_trip_through_constructor() {
    let row = vec![1, 0, 1, 0, 1, 1];
    let pauli = Pauli::from_symplectic_row(row.clone()).unwrap();

    assert_eq!(pauli.x_bits(), &[1, 0, 1]);
    assert_eq!(pauli.z_bits(), &[0, 1, 1]);
    assert_eq!(pauli.to_symplectic_row(), row);
}

#[test]
fn pauli_from_symplectic_row_rejects_invalid_rows() {
    assert_eq!(
        Pauli::from_symplectic_row(vec![1, 0, 1]),
        Err(QecError::InvalidSymplecticRowWidth { width: 3 })
    );
    assert_eq!(
        Pauli::from_symplectic_row(vec![1, 2, 0, 1]),
        Err(QecError::InvalidPauliBit {
            which: "X",
            index: 1,
            value: 2,
        })
    );
    assert_eq!(
        Pauli::from_symplectic_row(vec![1, 0, 0, 3]),
        Err(QecError::InvalidPauliBit {
            which: "Z",
            index: 1,
            value: 3,
        })
    );
}

#[test]
fn checked_binary_helpers_report_invalid_input_without_panicking() {
    assert_eq!(
        try_binary_rank(&[vec![1, 0], vec![1]]),
        Err(QecError::RowWidthMismatch {
            expected: 2,
            actual: 1,
        })
    );
    assert_eq!(
        try_in_row_span(&[vec![1, 0], vec![0, 1]], &[1, 2]),
        Err(QecError::InvalidBinaryEntry {
            row: 0,
            col: 1,
            value: 2,
        })
    );
}

#[test]
fn row_echelon_helpers_handle_pivots_elimination_and_input_validation() {
    let matrix = vec![vec![0, 1, 1], vec![0, 1, 0]];

    assert_eq!(
        eliminate_to_row_echelon(&matrix),
        vec![vec![0, 1, 1], vec![0, 0, 1]]
    );
    assert_eq!(
        try_eliminate_to_row_echelon(&matrix),
        Ok(vec![vec![0, 1, 1], vec![0, 0, 1]])
    );
    assert_eq!(
        try_eliminate_to_row_echelon(&[vec![1, 2]]),
        Err(QecError::InvalidBinaryEntry {
            row: 0,
            col: 1,
            value: 2,
        })
    );
}

#[test]
fn checked_row_span_reports_empty_and_width_mismatch_cases() {
    assert_eq!(try_in_row_span(&[], &[0, 1, 0]), Ok(false));
    assert_eq!(
        try_in_row_span(&[vec![1, 0], vec![0, 1]], &[1, 0, 0]),
        Err(QecError::RowWidthMismatch {
            expected: 2,
            actual: 3,
        })
    );
}

#[test]
fn checked_pauli_helpers_report_width_mismatch_without_panicking() {
    let short = Pauli::from_xz_bits(vec![1, 0], vec![0, 1]).unwrap();
    let long = Pauli::from_xz_bits(vec![1, 0, 0], vec![0, 1, 1]).unwrap();

    assert_eq!(
        short.try_symplectic_product(&long),
        Err(QecError::InvalidPauliWidth {
            x_width: 2,
            z_width: 3,
        })
    );
    assert_eq!(
        short.try_commutes_with(&long),
        Err(QecError::InvalidPauliWidth {
            x_width: 2,
            z_width: 3,
        })
    );
    assert_eq!(
        short.try_anticommutes_with(&long),
        Err(QecError::InvalidPauliWidth {
            x_width: 2,
            z_width: 3,
        })
    );
}

#[test]
fn pauli_commutation_and_weight_match_symplectic_overlap() {
    let xz = Pauli::from_xz_bits(vec![1, 0, 1], vec![0, 1, 1]).unwrap();
    let anticommutes = Pauli::from_xz_bits(vec![0, 0, 0], vec![1, 1, 0]).unwrap();
    let commuting = Pauli::from_xz_bits(vec![1, 0, 0], vec![0, 1, 0]).unwrap();

    assert_eq!(xz.n(), 3);
    assert_eq!(xz.x_bits(), &[1, 0, 1]);
    assert_eq!(xz.z_bits(), &[0, 1, 1]);
    assert_eq!(xz.to_symplectic_row(), vec![1, 0, 1, 0, 1, 1]);
    assert_eq!(xz.weight(), 3);
    assert_eq!(anticommutes.weight(), 2);
    assert_eq!(xz.symplectic_product(&anticommutes), 1);
    assert!(!xz.commutes_with(&anticommutes));
    assert!(xz.anticommutes_with(&anticommutes));
    assert!(xz.commutes_with(&commuting));
    assert!(!xz.anticommutes_with(&commuting));
}
