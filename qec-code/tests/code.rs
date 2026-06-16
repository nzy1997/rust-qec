use std::collections::HashSet;

use qec_code::codes::built_in_css::{
    BuiltInCssCodeSpec, BuiltInCssFamily, BuiltInCssParams, built_in_css_catalog,
    built_in_css_checks, parse_built_in_css_code_spec,
};
use qec_code::codes::steane::Steane;
use qec_code::css::{CssCode, SparseRowsMatrix};
use qec_code::{Pauli, QecError, StabilizerCode};

fn assert_strictly_increasing_rows(rows: &[Vec<usize>]) {
    for row in rows {
        assert!(
            row.windows(2).all(|pair| pair[0] < pair[1]),
            "row is not canonical: {row:?}"
        );
    }
}

fn assert_rows_in_range(rows: &[Vec<usize>], num_cols: usize) {
    for row in rows {
        for &col in row {
            assert!(
                col < num_cols,
                "row contains out-of-range column {col} for width {num_cols}: {row:?}"
            );
        }
    }
}

fn dense_rows(rows: &[Vec<usize>], width: usize) -> Vec<Vec<u8>> {
    rows.iter().map(|row| dense_row(row, width)).collect()
}

fn dense_row(row: &[usize], width: usize) -> Vec<u8> {
    let mut dense = vec![0; width];
    for &col in row {
        dense[col] = 1;
    }
    dense
}

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
fn stabilizer_code_rejects_generators_with_the_wrong_width() {
    let x0 = Pauli::from_xz_bits(vec![1], vec![0]).unwrap();

    assert_eq!(
        StabilizerCode::from_stabilizers(2, vec![x0]),
        Err(QecError::InvalidPauliWidth {
            x_width: 1,
            z_width: 2,
        })
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
fn css_code_accepts_redundant_orthogonal_checks() {
    let code = CssCode::from_hx_hz(vec![vec![1, 0], vec![0, 1], vec![1, 1]], vec![])
        .unwrap();

    assert_eq!(code.code().n(), 2);
    assert_eq!(code.code().stabilizer_rank(), 2);
    assert_eq!(code.code().stabilizers().len(), 2);
    assert_eq!(code.code().num_logical_qubits(), 0);
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
    assert_eq!(code.stabilizer_rows().len(), 6);
    assert_eq!(code.stabilizer_rows()[0].len(), 14);
}

#[test]
fn built_in_css_registry_exposes_steane_checks() {
    let checks = built_in_css_checks("steane").unwrap();

    assert_eq!(checks.code_id, "steane");
    assert_eq!(checks.num_cols, 7);
    assert_eq!(
        checks.hx,
        vec![
            vec![0, 3, 5, 6],
            vec![1, 3, 4, 6],
            vec![2, 4, 5, 6],
        ]
    );
    assert_eq!(checks.hz, checks.hx);
    assert_strictly_increasing_rows(&checks.hx);
    assert_strictly_increasing_rows(&checks.hz);
}

#[test]
fn built_in_css_catalog_lists_supported_specs() {
    let catalog = built_in_css_catalog();
    let specs = catalog.iter().map(|entry| entry.spec).collect::<Vec<_>>();
    let unique_specs = specs.iter().copied().collect::<HashSet<_>>();

    assert_eq!(
        specs,
        vec![
            "steane",
            "bb72",
            "repetition_x:d=<distance>",
            "repetition_z:d=<distance>",
        ]
    );
    assert_eq!(unique_specs.len(), specs.len());
    assert!(
        catalog.iter().all(|entry| !entry.description.is_empty()),
        "all catalog entries need descriptions: {catalog:?}"
    );
    assert!(
        catalog
            .iter()
            .any(|entry| entry.spec == "repetition_x:d=<distance>"
                && entry.description.contains("distance >= 2")),
        "repetition_x entry should describe the distance constraint: {catalog:?}"
    );
    assert!(
        catalog
            .iter()
            .any(|entry| entry.spec == "repetition_z:d=<distance>"
                && entry.description.contains("distance >= 2")),
        "repetition_z entry should describe the distance constraint: {catalog:?}"
    );
}

#[test]
fn bb72_has_expected_shape_and_css_orthogonality() {
    let checks = built_in_css_checks("bb72").unwrap();

    assert_eq!(checks.code_id, "bb72");
    assert_eq!(checks.num_cols, 72);
    assert_eq!(checks.hx.len(), 36);
    assert_eq!(checks.hz.len(), 36);

    for row in checks.hx.iter().chain(checks.hz.iter()) {
        assert_eq!(row.len(), 6, "row has wrong weight: {row:?}");
    }

    assert_strictly_increasing_rows(&checks.hx);
    assert_strictly_increasing_rows(&checks.hz);
    assert_rows_in_range(&checks.hx, checks.num_cols);
    assert_rows_in_range(&checks.hz, checks.num_cols);

    CssCode::from_hx_hz(
        dense_rows(&checks.hx, checks.num_cols),
        dense_rows(&checks.hz, checks.num_cols),
    )
    .unwrap();
}

#[test]
fn built_in_css_code_spec_parses_fixed_and_parameterized_ids() {
    assert_eq!(
        parse_built_in_css_code_spec("steane"),
        Ok(BuiltInCssCodeSpec::Fixed { code_id: "steane" })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_x:d=5"),
        Ok(BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::RepetitionX,
            params: BuiltInCssParams { distance: 5 },
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_z:d=5"),
        Ok(BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::RepetitionZ,
            params: BuiltInCssParams { distance: 5 },
        })
    );
}

#[test]
fn bb72_code_spec_rejects_unexpected_parameters() {
    assert_eq!(
        parse_built_in_css_code_spec("bb72"),
        Ok(BuiltInCssCodeSpec::Fixed { code_id: "bb72" })
    );
    assert_eq!(
        parse_built_in_css_code_spec("bb72:d=3"),
        Err(QecError::UnknownBuiltInCssFamily {
            family: "bb72".to_owned(),
        })
    );
}

#[test]
fn built_in_css_code_spec_rejects_unknown_family_missing_distance_and_bad_integers() {
    assert_eq!(
        parse_built_in_css_code_spec("unknown:d=5"),
        Err(QecError::UnknownBuiltInCssFamily {
            family: "unknown".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_x"),
        Err(QecError::MissingBuiltInCssParameter {
            family: "repetition_x".to_owned(),
            parameter: "d".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_x:d=nope"),
        Err(QecError::InvalidBuiltInCssIntegerParameter {
            family: "repetition_x".to_owned(),
            parameter: "d".to_owned(),
            value: "nope".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("unknown"),
        Err(QecError::UnknownBuiltInCssCode {
            code_id: "unknown".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_x:"),
        Err(QecError::MissingBuiltInCssParameter {
            family: "repetition_x".to_owned(),
            parameter: "d".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_x:d"),
        Err(QecError::UnexpectedBuiltInCssParameter {
            family: "repetition_x".to_owned(),
            parameter: "d".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_x:d=5,d=7"),
        Err(QecError::DuplicateBuiltInCssParameter {
            family: "repetition_x".to_owned(),
            parameter: "d".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_x:d=0"),
        Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "repetition_x".to_owned(),
            parameter: "d".to_owned(),
            value: 0,
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_x:d=5,foo=1"),
        Err(QecError::UnexpectedBuiltInCssParameter {
            family: "repetition_x".to_owned(),
            parameter: "foo".to_owned(),
        })
    );
}

#[test]
fn repetition_x_d5_matches_chain_checks() {
    let checks = built_in_css_checks("repetition_x:d=5").unwrap();

    assert_eq!(checks.code_id, "repetition_x");
    assert_eq!(checks.num_cols, 5);
    assert_eq!(
        checks.hx,
        vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 4]]
    );
    assert_eq!(checks.hz, Vec::<Vec<usize>>::new());
    assert_strictly_increasing_rows(&checks.hx);
}

#[test]
fn repetition_z_d5_matches_chain_checks() {
    let checks = built_in_css_checks("repetition_z:d=5").unwrap();

    assert_eq!(checks.code_id, "repetition_z");
    assert_eq!(checks.num_cols, 5);
    assert_eq!(checks.hx, Vec::<Vec<usize>>::new());
    assert_eq!(
        checks.hz,
        vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 4]]
    );
    assert_strictly_increasing_rows(&checks.hz);
}

#[test]
fn repetition_family_rejects_distance_below_two() {
    assert_eq!(
        built_in_css_checks("repetition_x:d=1"),
        Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "repetition_x".to_owned(),
            parameter: "d".to_owned(),
            value: 1,
        })
    );
    assert_eq!(
        built_in_css_checks("repetition_z:d=1"),
        Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "repetition_z".to_owned(),
            parameter: "d".to_owned(),
            value: 1,
        })
    );
}

#[test]
fn sparse_rows_matrix_serializes_steane_supports() {
    let checks = built_in_css_checks("steane").unwrap();
    let text = SparseRowsMatrix::new(checks.num_cols, checks.hx.clone())
        .unwrap()
        .to_json_string();

    assert_eq!(
        text,
        "{\"format\":\"sparse_rows\",\"num_cols\":7,\"rows\":[[0,3,5,6],[1,3,4,6],[2,4,5,6]]}"
    );
}

#[test]
fn built_in_css_registry_rejects_unknown_code_id() {
    assert_eq!(
        built_in_css_checks("unknown"),
        Err(QecError::UnknownBuiltInCssCode {
            code_id: "unknown".to_owned(),
        })
    );
}
