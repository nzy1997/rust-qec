use std::path::PathBuf;

use qec_code::codes::built_in_css::built_in_css_checks;
use qec_code::css::SparseRowsMatrix;
use qec_code::QecError;

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

    assert_eq!(hx, expected_hx);
    assert_eq!(hz, expected_hz);
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
