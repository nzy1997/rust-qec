use qec_code::QecError;
use qec_code::sparse_gf2::{SparseGf2Matrix, hconcat, identity, kron, transpose};

fn assert_shape_and_rows(
    matrix: &SparseGf2Matrix,
    num_rows: usize,
    num_cols: usize,
    rows: &[Vec<usize>],
) {
    assert_eq!(matrix.num_rows(), num_rows);
    assert_eq!(matrix.num_cols(), num_cols);
    assert_eq!(matrix.rows(), rows);
}

#[test]
fn sparse_gf2_composition_matches_known_answers() {
    let a = SparseGf2Matrix::new(2, 2, vec![vec![0], vec![0, 1]]).unwrap();
    let b = SparseGf2Matrix::new(2, 2, vec![vec![1], vec![0]]).unwrap();

    assert_shape_and_rows(&identity(2).unwrap(), 2, 2, &[vec![0], vec![1]]);
    assert_shape_and_rows(&transpose(&a).unwrap(), 2, 2, &[vec![0, 1], vec![1]]);
    assert_shape_and_rows(&hconcat(&a, &b).unwrap(), 2, 4, &[vec![0, 3], vec![0, 1, 2]]);
    assert_shape_and_rows(&kron(&a, &b).unwrap(), 4, 4, &[vec![1], vec![0], vec![1, 3], vec![0, 2]]);

    let canonicalized =
        SparseGf2Matrix::new(2, 4, vec![vec![3, 1, 3, 2, 1], vec![2, 2]]).unwrap();
    assert_shape_and_rows(&canonicalized, 2, 4, &[vec![2], vec![]]);

    assert_shape_and_rows(&identity(0).unwrap(), 0, 0, &[]);

    let empty_wide = SparseGf2Matrix::new(0, 3, vec![]).unwrap();
    assert_shape_and_rows(&transpose(&empty_wide).unwrap(), 3, 0, &[vec![], vec![], vec![]]);

    let empty_rows_left = SparseGf2Matrix::new(0, 2, vec![]).unwrap();
    let empty_rows_right = SparseGf2Matrix::new(0, 5, vec![]).unwrap();
    assert_shape_and_rows(&hconcat(&empty_rows_left, &empty_rows_right).unwrap(), 0, 7, &[]);
    assert_shape_and_rows(&kron(&empty_rows_left, &empty_rows_right).unwrap(), 0, 10, &[]);
}

#[test]
fn sparse_gf2_composition_rejects_invalid_shapes() {
    assert_eq!(
        SparseGf2Matrix::new(1, 2, vec![vec![2]]),
        Err(QecError::SparseGf2SupportOutOfRange {
            row: 0,
            support: 2,
            num_cols: 2,
        })
    );

    assert_eq!(
        SparseGf2Matrix::new(2, 2, vec![vec![]]),
        Err(QecError::SparseGf2RowCountMismatch {
            expected: 2,
            actual: 1,
        })
    );

    let one_row = SparseGf2Matrix::new(1, 2, vec![vec![0]]).unwrap();
    let two_rows = SparseGf2Matrix::new(2, 2, vec![vec![0], vec![1]]).unwrap();
    assert_eq!(
        hconcat(&one_row, &two_rows),
        Err(QecError::SparseGf2HorizontalRowMismatch {
            left_rows: 1,
            right_rows: 2,
        })
    );

    let max_width = SparseGf2Matrix::new(0, usize::MAX, vec![]).unwrap();
    let one_col = SparseGf2Matrix::new(0, 1, vec![]).unwrap();
    assert_eq!(
        hconcat(&max_width, &one_col),
        Err(QecError::SparseGf2DimensionOverflow {
            operation: "hconcat",
        })
    );

    let two_cols = SparseGf2Matrix::new(0, 2, vec![]).unwrap();
    assert_eq!(
        kron(&max_width, &two_cols),
        Err(QecError::SparseGf2DimensionOverflow { operation: "kron" })
    );
}
