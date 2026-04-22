use rbposd::{Correction, ParityCheckMatrix, Syndrome};

#[test]
fn sparse_rows_reject_an_out_of_bounds_column() {
    let err = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![3]])
        .unwrap_err();

    assert!(err.to_string().contains("out of bounds"));
}

#[test]
fn sparse_columns_and_sparse_rows_encode_the_same_code() {
    let from_rows =
        ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let from_cols =
        ParityCheckMatrix::from_sparse_columns(2, 3, vec![vec![0], vec![0, 1], vec![1]])
            .unwrap();

    let correction = Correction::from(vec![true, false, true]);
    let expected = Syndrome::from(vec![true, true]);

    assert_eq!(from_rows.multiply(&correction), expected);
    assert_eq!(from_cols.multiply(&correction), expected);
}

#[test]
fn sparse_columns_reject_an_out_of_bounds_row() {
    let err = ParityCheckMatrix::from_sparse_columns(2, 3, vec![vec![0], vec![2], vec![]])
        .unwrap_err();

    assert!(err.to_string().contains("row index 2"));
    assert!(err.to_string().contains("out of bounds"));
}
