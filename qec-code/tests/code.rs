use std::collections::HashSet;

use qec_code::codes::built_in_css::{
    bivariate_bicycle_css_checks, built_in_css_catalog, built_in_css_checks,
    parse_built_in_css_code_spec, BivariateBicycleParams, BuiltInCssCodeSpec, BuiltInCssFamily,
    BuiltInCssParams,
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

fn row_weight_counts(rows: &[Vec<usize>]) -> std::collections::BTreeMap<usize, usize> {
    let mut counts = std::collections::BTreeMap::new();
    for row in rows {
        *counts.entry(row.len()).or_insert(0) += 1;
    }
    counts
}

fn assert_surface_rotated_d5_weights(rows: &[Vec<usize>]) {
    let counts = row_weight_counts(rows);
    assert_eq!(counts.get(&2), Some(&4));
    assert_eq!(counts.get(&4), Some(&8));
    assert_eq!(counts.values().sum::<usize>(), 12);
}

fn bb72_bivariate_bicycle_params() -> BivariateBicycleParams {
    BivariateBicycleParams {
        lx: 6,
        ly: 6,
        a_terms: vec![(3, 0), (0, 1), (0, 2)],
        b_terms: vec![(0, 3), (1, 0), (2, 0)],
    }
}

fn bb144_bivariate_bicycle_params() -> BivariateBicycleParams {
    BivariateBicycleParams {
        lx: 12,
        ly: 6,
        a_terms: vec![(3, 0), (0, 1), (0, 2)],
        b_terms: vec![(0, 3), (1, 0), (2, 0)],
    }
}

fn bivariate_bicycle_large_shift_params() -> BivariateBicycleParams {
    BivariateBicycleParams {
        lx: 3,
        ly: 2,
        a_terms: vec![(usize::MAX, 1)],
        b_terms: vec![(1, usize::MAX)],
    }
}

fn bivariate_bicycle_normalized_shift_params() -> BivariateBicycleParams {
    BivariateBicycleParams {
        lx: 3,
        ly: 2,
        a_terms: vec![(0, 1)],
        b_terms: vec![(1, 1)],
    }
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
    let code = CssCode::from_hx_hz(vec![vec![1, 0], vec![0, 1], vec![1, 1]], vec![]).unwrap();

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
        vec![vec![0, 3, 5, 6], vec![1, 3, 4, 6], vec![2, 4, 5, 6],]
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
            "surface_rotated:d=<distance>",
            "toric:d=<distance>",
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
    assert!(
        catalog
            .iter()
            .any(|entry| entry.spec == "surface_rotated:d=<distance>"
                && entry.description.contains("distance >= 2")),
        "surface_rotated entry should describe the distance constraint: {catalog:?}"
    );
    assert!(
        catalog
            .iter()
            .any(|entry| entry.spec == "toric:d=<distance>"
                && entry.description.contains("distance >= 2")),
        "toric entry should describe the distance constraint: {catalog:?}"
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
fn bivariate_bicycle_css_checks_bb72_matches_fixed_alias() {
    let fixed = built_in_css_checks("bb72").unwrap();
    let generic = bivariate_bicycle_css_checks(bb72_bivariate_bicycle_params()).unwrap();

    assert_eq!(generic.code_id, "bb");
    assert_eq!(generic.num_cols, fixed.num_cols);
    assert_eq!(generic.hx, fixed.hx);
    assert_eq!(generic.hz, fixed.hz);
}

#[test]
fn bivariate_bicycle_css_checks_bb144_shape_orthogonality_and_canonical_rows() {
    let checks = bivariate_bicycle_css_checks(bb144_bivariate_bicycle_params()).unwrap();

    assert_eq!(checks.code_id, "bb");
    assert_eq!(checks.num_cols, 144);
    assert_eq!(checks.hx.len(), 72);
    assert_eq!(checks.hz.len(), 72);

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
fn bivariate_bicycle_css_checks_rejects_zero_lattice_dimension() {
    let mut params = bb144_bivariate_bicycle_params();
    params.lx = 0;

    assert_eq!(
        bivariate_bicycle_css_checks(params),
        Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "bb".to_owned(),
            parameter: "lx".to_owned(),
            value: 0,
        })
    );

    let mut params = bb144_bivariate_bicycle_params();
    params.ly = 0;

    assert_eq!(
        bivariate_bicycle_css_checks(params),
        Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "bb".to_owned(),
            parameter: "ly".to_owned(),
            value: 0,
        })
    );
}

#[test]
fn bivariate_bicycle_css_checks_rejects_empty_term_lists() {
    let mut params = bb72_bivariate_bicycle_params();
    params.a_terms = vec![];

    assert_eq!(
        bivariate_bicycle_css_checks(params),
        Err(QecError::MissingBuiltInCssParameter {
            family: "bb".to_owned(),
            parameter: "a_terms".to_owned(),
        })
    );

    let mut params = bb72_bivariate_bicycle_params();
    params.b_terms = vec![];

    assert_eq!(
        bivariate_bicycle_css_checks(params),
        Err(QecError::MissingBuiltInCssParameter {
            family: "bb".to_owned(),
            parameter: "b_terms".to_owned(),
        })
    );
}

#[test]
fn bivariate_bicycle_css_checks_rejects_modulo_duplicate_terms() {
    let mut params = bb72_bivariate_bicycle_params();
    params.a_terms = vec![(0, 0), (6, 0)];

    assert!(bivariate_bicycle_css_checks(params).is_err());
}

#[test]
fn bivariate_bicycle_css_checks_normalizes_large_shifts_before_row_generation() {
    let large = bivariate_bicycle_css_checks(bivariate_bicycle_large_shift_params()).unwrap();
    let normalized =
        bivariate_bicycle_css_checks(bivariate_bicycle_normalized_shift_params()).unwrap();

    assert_eq!(large, normalized);
}

#[test]
fn surface_rotated_d3_matches_expected_checks() {
    let checks = built_in_css_checks("surface_rotated:d=3").unwrap();

    assert_eq!(checks.code_id, "surface_rotated");
    assert_eq!(checks.num_cols, 9);
    assert_eq!(
        checks.hx,
        vec![vec![0, 3], vec![1, 2, 4, 5], vec![3, 4, 6, 7], vec![5, 8],]
    );
    assert_eq!(
        checks.hz,
        vec![vec![1, 2], vec![0, 1, 3, 4], vec![4, 5, 7, 8], vec![6, 7],]
    );
    assert_strictly_increasing_rows(&checks.hx);
    assert_strictly_increasing_rows(&checks.hz);
}

#[test]
fn surface_rotated_d5_has_expected_check_counts_and_weights() {
    let checks = built_in_css_checks("surface_rotated:d=5").unwrap();

    assert_eq!(checks.code_id, "surface_rotated");
    assert_eq!(checks.num_cols, 25);
    assert_eq!(checks.hx.len(), 12);
    assert_eq!(checks.hz.len(), 12);
    assert_surface_rotated_d5_weights(&checks.hx);
    assert_surface_rotated_d5_weights(&checks.hz);
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
fn surface_rotated_rejects_distance_below_two() {
    assert_eq!(
        built_in_css_checks("surface_rotated:d=1"),
        Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "surface_rotated".to_owned(),
            parameter: "d".to_owned(),
            value: 1,
        })
    );
}

#[test]
fn toric_d3_matches_expected_checks() {
    let checks = built_in_css_checks("toric:d=3").unwrap();

    assert_eq!(checks.code_id, "toric");
    assert_eq!(checks.num_cols, 18);
    assert_eq!(
        checks.hx,
        vec![
            vec![0, 2, 9, 15],
            vec![0, 1, 10, 16],
            vec![1, 2, 11, 17],
            vec![3, 5, 9, 12],
            vec![3, 4, 10, 13],
            vec![4, 5, 11, 14],
            vec![6, 8, 12, 15],
            vec![6, 7, 13, 16],
            vec![7, 8, 14, 17],
        ]
    );
    assert_eq!(
        checks.hz,
        vec![
            vec![0, 3, 9, 10],
            vec![1, 4, 10, 11],
            vec![2, 5, 9, 11],
            vec![3, 6, 12, 13],
            vec![4, 7, 13, 14],
            vec![5, 8, 12, 14],
            vec![0, 6, 15, 16],
            vec![1, 7, 16, 17],
            vec![2, 8, 15, 17],
        ]
    );
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
fn toric_d4_has_expected_counts_and_weight_four_rows() {
    let checks = built_in_css_checks("toric:d=4").unwrap();

    assert_eq!(checks.code_id, "toric");
    assert_eq!(checks.num_cols, 32);
    assert_eq!(checks.hx.len(), 16);
    assert_eq!(checks.hz.len(), 16);

    for row in checks.hx.iter().chain(checks.hz.iter()) {
        assert_eq!(row.len(), 4, "row has wrong weight: {row:?}");
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
fn toric_family_rejects_distance_below_two() {
    assert_eq!(
        built_in_css_checks("toric:d=1"),
        Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "toric".to_owned(),
            parameter: "d".to_owned(),
            value: 1,
        })
    );
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
            params: BuiltInCssParams::Distance { distance: 5 },
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("repetition_z:d=5"),
        Ok(BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::RepetitionZ,
            params: BuiltInCssParams::Distance { distance: 5 },
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("surface_rotated:d=3"),
        Ok(BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::SurfaceRotated,
            params: BuiltInCssParams::Distance { distance: 3 },
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("toric:d=3"),
        Ok(BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::Toric,
            params: BuiltInCssParams::Distance { distance: 3 },
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0"),
        Ok(BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::BivariateBicycle,
            params: BuiltInCssParams::BivariateBicycle(bb144_bivariate_bicycle_params()),
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
        parse_built_in_css_code_spec("surface_rotated"),
        Err(QecError::MissingBuiltInCssParameter {
            family: "surface_rotated".to_owned(),
            parameter: "d".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("toric"),
        Err(QecError::MissingBuiltInCssParameter {
            family: "toric".to_owned(),
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
fn built_in_css_code_spec_rejects_bad_bivariate_bicycle_params() {
    assert_eq!(
        parse_built_in_css_code_spec("bb:lx=12,ly=6,b=0:3|1:0|2:0"),
        Err(QecError::MissingBuiltInCssParameter {
            family: "bb".to_owned(),
            parameter: "a".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("bb:lx=0,ly=6,a=3:0,b=0:3"),
        Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
            family: "bb".to_owned(),
            parameter: "lx".to_owned(),
            value: 0,
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("bb:lx=12,lx=6,ly=6,a=3:0,b=0:3"),
        Err(QecError::DuplicateBuiltInCssParameter {
            family: "bb".to_owned(),
            parameter: "lx".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("bb:lx=12,ly=6,a=3:0,b=0:3,foo=1"),
        Err(QecError::UnexpectedBuiltInCssParameter {
            family: "bb".to_owned(),
            parameter: "foo".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("bb:lx=12,ly=6,a=3,b=0:3"),
        Err(QecError::InvalidBuiltInCssIntegerParameter {
            family: "bb".to_owned(),
            parameter: "a".to_owned(),
            value: "3".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("bb:lx=6,ly=6,a=0:0|6:0,b=0:3"),
        Err(QecError::DuplicateBuiltInCssParameter {
            family: "bb".to_owned(),
            parameter: "a_terms".to_owned(),
        })
    );
}

#[test]
fn built_in_css_code_spec_rejects_malformed_bivariate_bicycle_shapes() {
    let cases = [
        (
            "bb",
            QecError::MissingBuiltInCssParameter {
                family: "bb".to_owned(),
                parameter: "lx".to_owned(),
            },
        ),
        (
            "bb:",
            QecError::MissingBuiltInCssParameter {
                family: "bb".to_owned(),
                parameter: "lx".to_owned(),
            },
        ),
        (
            "bb:lx",
            QecError::UnexpectedBuiltInCssParameter {
                family: "bb".to_owned(),
                parameter: "lx".to_owned(),
            },
        ),
        (
            "bb:lx=nope,ly=6,a=3:0,b=0:3",
            QecError::InvalidBuiltInCssIntegerParameter {
                family: "bb".to_owned(),
                parameter: "lx".to_owned(),
                value: "nope".to_owned(),
            },
        ),
        (
            "bb:lx=12,a=3:0,b=0:3",
            QecError::MissingBuiltInCssParameter {
                family: "bb".to_owned(),
                parameter: "ly".to_owned(),
            },
        ),
        (
            "bb:lx=12,ly=6,a=3:0",
            QecError::MissingBuiltInCssParameter {
                family: "bb".to_owned(),
                parameter: "b".to_owned(),
            },
        ),
        (
            "bb:lx=12,ly=6,a=3:0,a=0:1,b=0:3",
            QecError::DuplicateBuiltInCssParameter {
                family: "bb".to_owned(),
                parameter: "a".to_owned(),
            },
        ),
        (
            "bb:lx=12,ly=6,a=3:0,b=0:3,b=1:0",
            QecError::DuplicateBuiltInCssParameter {
                family: "bb".to_owned(),
                parameter: "b".to_owned(),
            },
        ),
        (
            "bb:lx=12,ly=6,a=x:0,b=0:3",
            QecError::InvalidBuiltInCssIntegerParameter {
                family: "bb".to_owned(),
                parameter: "a".to_owned(),
                value: "x".to_owned(),
            },
        ),
        (
            "bb:lx=12,ly=6,a=3:x,b=0:3",
            QecError::InvalidBuiltInCssIntegerParameter {
                family: "bb".to_owned(),
                parameter: "a".to_owned(),
                value: "x".to_owned(),
            },
        ),
    ];

    for (spec, expected) in cases {
        assert_eq!(parse_built_in_css_code_spec(spec), Err(expected), "{spec}");
    }
}

#[test]
fn built_in_css_checks_rejects_parser_only_bivariate_bicycle_specs() {
    let spec = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0";

    assert_eq!(
        parse_built_in_css_code_spec(spec),
        Ok(BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::BivariateBicycle,
            params: BuiltInCssParams::BivariateBicycle(bb144_bivariate_bicycle_params()),
        })
    );
    assert_eq!(
        built_in_css_checks(spec),
        Err(QecError::UnknownBuiltInCssCode {
            code_id: spec.to_owned(),
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
