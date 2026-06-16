use std::path::PathBuf;

use qec_code::QecError;
use qec_code::codes::built_in_css::built_in_css_checks;
use qec_code::css::SparseRowsMatrix;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read_fixture(rel_path: &str) -> String {
    std::fs::read_to_string(workspace_root().join(rel_path))
        .unwrap_or_else(|err| panic!("failed to read fixture {rel_path}: {err}"))
}

#[test]
fn steane_sparse_rows_json_matches_workspace_fixtures() {
    let checks = built_in_css_checks("steane").unwrap();

    let hx = SparseRowsMatrix::new(checks.num_cols, checks.hx.clone())
        .unwrap()
        .to_json_string();
    let hz = SparseRowsMatrix::new(checks.num_cols, checks.hz.clone())
        .unwrap()
        .to_json_string();

    let expected_hx = read_fixture("rsinter/tests/fixtures/css/steane_hx.json");
    let expected_hz = read_fixture("rsinter/tests/fixtures/css/steane_hz.json");

    assert_eq!(format!("{hx}\n"), expected_hx);
    assert_eq!(format!("{hz}\n"), expected_hz);
}

#[test]
fn sparse_rows_matrix_rejects_duplicate_or_out_of_range_supports() {
    assert_eq!(
        SparseRowsMatrix::new(3, vec![vec![0, 0]]),
        Err(QecError::DuplicateSparseRowSupport { row: 0, support: 0 })
    );

    assert_eq!(
        SparseRowsMatrix::new(3, vec![vec![3]]),
        Err(QecError::SparseRowSupportOutOfRange {
            row: 0,
            support: 3,
            num_cols: 3,
        })
    );
}

#[test]
fn sparse_rows_matrix_rejects_zero_width() {
    assert_eq!(
        SparseRowsMatrix::new(0, vec![]),
        Err(QecError::InvalidSparseRowsWidth { num_cols: 0 })
    );
}

#[test]
fn sparse_rows_matrix_preserves_row_order_without_normalizing() {
    let text = SparseRowsMatrix::new(5, vec![vec![3, 1], vec![4, 0]])
        .unwrap()
        .to_json_string();

    assert_eq!(
        text,
        "{\"format\":\"sparse_rows\",\"num_cols\":5,\"rows\":[[3,1],[4,0]]}"
    );
}
