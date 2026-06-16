use qec_code::QecError;
use qec_code::css::{SparseRowsMatrix, sparse_rows_matrix_from_json_str};

#[test]
fn sparse_rows_json_parses_and_converts_to_dense_rows() {
    let matrix = sparse_rows_matrix_from_json_str(
        r#"{"format":"sparse_rows","num_cols":5,"rows":[[0,3],[1,4],[]]}"#,
    )
    .unwrap();

    assert_eq!(matrix.num_cols(), 5);
    assert_eq!(matrix.rows(), &[vec![0, 3], vec![1, 4], vec![]]);
    assert_eq!(
        matrix.to_dense_rows(),
        vec![
            vec![1, 0, 0, 1, 0],
            vec![0, 1, 0, 0, 1],
            vec![0, 0, 0, 0, 0],
        ]
    );
}

#[test]
fn sparse_rows_json_rejects_missing_format() {
    assert_eq!(
        sparse_rows_matrix_from_json_str(r#"{"num_cols":3,"rows":[[0]]}"#),
        Err(QecError::MissingCssMatrixFormat)
    );
}

#[test]
fn sparse_rows_json_rejects_dense_matrix_shape_as_unsupported_format() {
    assert_eq!(
        sparse_rows_matrix_from_json_str(r#"{"format":"dense","rows":[[1,0,1]]}"#),
        Err(QecError::UnsupportedCssMatrixFormat {
            format: "dense".to_owned(),
        })
    );
}

#[test]
fn sparse_rows_json_rejects_malformed_json() {
    let err =
        sparse_rows_matrix_from_json_str(r#"{"format":"sparse_rows","num_cols":"bad","rows":[]}"#)
            .unwrap_err();

    assert!(
        err.to_string().contains("invalid CSS matrix JSON"),
        "error was: {err}"
    );
}

#[test]
fn sparse_rows_json_reuses_sparse_row_validation() {
    assert_eq!(
        sparse_rows_matrix_from_json_str(r#"{"format":"sparse_rows","num_cols":3,"rows":[[0,3]]}"#,),
        Err(QecError::SparseRowSupportOutOfRange {
            row: 0,
            support: 3,
            num_cols: 3,
        })
    );
}

#[test]
fn sparse_rows_matrix_dense_conversion_preserves_empty_rows() {
    let matrix = SparseRowsMatrix::new(3, vec![vec![], vec![0, 2]]).unwrap();

    assert_eq!(matrix.to_dense_rows(), vec![vec![0, 0, 0], vec![1, 0, 1]]);
}
