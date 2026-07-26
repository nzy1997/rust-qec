use std::path::PathBuf;

use qec_code::codes::built_in_css::built_in_css_checks;
use qec_code::codes::color_666::COLOR_666_STEANE_PERMUTATION;
use qec_code::css::{CssCode, SparseRowsMatrix};
use qec_code::distance::compute_distance;
use qec_code::family_contract::{
    construct_css, parse_css_construction_json, Color666FamilySpec, Color666Layout,
    CssConstructionSpec, CssFamilySpec, RequestedFamilyId,
};
use qec_code::QecError;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_rows(name: &str) -> Vec<Vec<usize>> {
    let path = workspace_root().join("tests/fixtures/css").join(name);
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    serde_json::from_value(value["rows"].clone()).unwrap()
}

fn css_code(num_cols: usize, h_x: &[Vec<usize>], h_z: &[Vec<usize>]) -> CssCode {
    CssCode::from_hx_hz(
        SparseRowsMatrix::new(num_cols, h_x.to_vec())
            .unwrap()
            .to_dense_rows(),
        SparseRowsMatrix::new(num_cols, h_z.to_vec())
            .unwrap()
            .to_dense_rows(),
    )
    .unwrap()
}

fn row_weights(rows: &[Vec<usize>]) -> Vec<usize> {
    let mut weights = rows.iter().map(Vec::len).collect::<Vec<_>>();
    weights.sort_unstable();
    weights
}

fn permute_rows(rows: &[Vec<usize>], permutation: &[usize]) -> Vec<Vec<usize>> {
    let mut mapped = rows
        .iter()
        .map(|row| {
            let mut mapped = row
                .iter()
                .map(|&qubit| permutation[qubit])
                .collect::<Vec<_>>();
            mapped.sort_unstable();
            mapped
        })
        .collect::<Vec<_>>();
    mapped.sort();
    mapped
}

#[test]
fn color_666_d3_matches_steane_under_stable_permutation() {
    assert_eq!(COLOR_666_STEANE_PERMUTATION, [0, 3, 6, 5, 1, 4, 2]);

    let result = construct_css(
        CssFamilySpec::Color666(Color666FamilySpec {
            distance: 3,
            layout: Color666Layout::Triangular,
        })
        .into(),
    )
    .unwrap();

    assert_eq!(result.construction_id, "color_666");
    assert_eq!(
        result.requested_family_id,
        Some(RequestedFamilyId::Color666)
    );
    assert_eq!(
        result.normalized_parameters["distance"],
        serde_json::json!(3)
    );
    assert_eq!(
        result.normalized_parameters["layout"],
        serde_json::json!("triangular")
    );
    assert_eq!(result.provenance.adapter, "color_666");
    assert_eq!(result.stats.n, 7);
    assert_eq!(result.stats.m_x, 3);
    assert_eq!(result.stats.m_z, 3);
    assert_eq!(result.stats.rank_x, 3);
    assert_eq!(result.stats.rank_z, 3);
    assert_eq!(result.stats.k, 1);
    assert_eq!(result.checks.h_x, fixture_rows("color_666_d3_hx.json"));
    assert_eq!(result.checks.h_z, fixture_rows("color_666_d3_hz.json"));

    let steane = built_in_css_checks("steane").unwrap();
    assert_eq!(
        permute_rows(&result.checks.h_x, &COLOR_666_STEANE_PERMUTATION),
        {
            let mut rows = steane.hx.clone();
            rows.sort();
            rows
        }
    );

    let distance =
        compute_distance(css_code(result.stats.n, &result.checks.h_x, &result.checks.h_z).code())
            .unwrap();
    assert_eq!(distance.distance, 3);
}

#[test]
fn color_666_d5_matches_fixture() {
    let result = construct_css(
        CssFamilySpec::Color666(Color666FamilySpec {
            distance: 5,
            layout: Color666Layout::Triangular,
        })
        .into(),
    )
    .unwrap();

    assert_eq!(result.stats.n, 19);
    assert_eq!(result.stats.m_x, 9);
    assert_eq!(result.stats.m_z, 9);
    assert_eq!(result.stats.rank_x, 9);
    assert_eq!(result.stats.rank_z, 9);
    assert_eq!(result.stats.k, 1);
    assert_eq!(result.checks.h_x, fixture_rows("color_666_d5_hx.json"));
    assert_eq!(result.checks.h_z, fixture_rows("color_666_d5_hz.json"));
    assert_eq!(
        row_weights(&result.checks.h_x),
        vec![4, 4, 4, 4, 4, 4, 6, 6, 6]
    );
    assert_eq!(
        row_weights(&result.checks.h_z),
        vec![4, 4, 4, 4, 4, 4, 6, 6, 6]
    );

    let distance =
        compute_distance(css_code(result.stats.n, &result.checks.h_x, &result.checks.h_z).code())
            .unwrap();
    assert_eq!(distance.distance, 5);
}

#[test]
fn color_666_rejects_even_distance() {
    assert!(matches!(
        construct_css(
            CssFamilySpec::Color666(Color666FamilySpec {
                distance: 4,
                layout: Color666Layout::Triangular,
            })
            .into()
        ),
        Err(QecError::InvalidCssConstruction {
            construction,
            reason
        }) if construction == "color_666" && reason.contains("odd")
    ));
}

#[test]
fn color_666_rejects_distance_below_three() {
    assert!(matches!(
        construct_css(
            CssFamilySpec::Color666(Color666FamilySpec {
                distance: 2,
                layout: Color666Layout::Triangular,
            })
            .into()
        ),
        Err(QecError::InvalidCssConstruction {
            construction,
            reason
        }) if construction == "color_666" && reason.contains("at least 3")
    ));
}

#[test]
fn color_666_rejects_distance_overflow() {
    assert!(matches!(
        construct_css(
            CssFamilySpec::Color666(Color666FamilySpec {
                distance: usize::MAX,
                layout: Color666Layout::Triangular,
            })
            .into()
        ),
        Err(QecError::InvalidCssConstruction {
            construction,
            reason
        }) if construction == "color_666" && reason.contains("overflow")
    ));
}

#[test]
fn color_666_rejects_unsupported_layout() {
    assert!(matches!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"color_666","distance":5,"layout":"square"}"#
        ),
        Err(QecError::InvalidCssConstruction {
            construction,
            reason
        }) if construction == "color_666" && reason.contains("unsupported layout")
    ));
}

#[test]
fn color_666_inline_spec_defaults_to_triangular_layout() {
    assert_eq!(
        CssConstructionSpec::from_inline("color_666:d=5").unwrap(),
        CssFamilySpec::Color666(Color666FamilySpec {
            distance: 5,
            layout: Color666Layout::Triangular,
        })
        .into()
    );
}

#[test]
fn color_666_json_spec_defaults_to_triangular_layout() {
    assert_eq!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"color_666","distance":5}"#
        )
        .unwrap(),
        CssFamilySpec::Color666(Color666FamilySpec {
            distance: 5,
            layout: Color666Layout::Triangular,
        })
        .into()
    );
}
