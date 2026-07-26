use std::path::PathBuf;

use qec_code::QecError;
use qec_code::codes::quantum_tanner::quantum_tanner_spec_from_json_str;
use qec_code::css::SparseRowsMatrix;
use qec_code::family_contract::{
    CLASSICAL_IDENTITY_2, CssClassicalCheckSpec, CssConstructionSpec, CssFamilySpec,
    HypergraphProductSpec, RequestedFamilyId, SurfaceSpec, construct_css,
    parse_css_construction_json, verify_css_orthogonality,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_rows(name: &str) -> Vec<Vec<usize>> {
    let path = workspace_root().join("tests/fixtures/css").join(name);
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("fixture should be readable"))
            .expect("fixture should be valid JSON");
    serde_json::from_value(value["rows"].clone()).expect("fixture rows should be arrays")
}

fn fixture_text(name: &str) -> String {
    let path = workspace_root().join("tests/fixtures/css").join(name);
    std::fs::read_to_string(path)
        .expect("fixture should be readable")
        .trim_end_matches('\n')
        .to_owned()
}

fn assert_canonical_sparse_rows(rows: &[Vec<usize>]) {
    for row in rows {
        assert!(
            row.windows(2).all(|window| window[0] < window[1]),
            "row must contain sorted unique supports: {row:?}"
        );
    }
}

#[test]
fn unified_family_contract_preserves_requested_family_ids() {
    let ids = serde_json::to_value(RequestedFamilyId::ALL).unwrap();
    let id_strings = RequestedFamilyId::ALL.map(RequestedFamilyId::as_str);

    assert_eq!(
        ids,
        serde_json::json!([
            "directional",
            "quantum_tanner",
            "generalized_bicycle",
            "la_cross",
            "random_hgp",
            "lifted_product",
            "hyperbolic_5_5",
            "coprime_bb",
            "toric_3d",
            "color_666",
            "surface",
            "shor_like",
            "random_two_block",
            "perturbed_hgp"
        ])
    );
    assert_eq!(
        id_strings,
        [
            "directional",
            "quantum_tanner",
            "generalized_bicycle",
            "la_cross",
            "random_hgp",
            "lifted_product",
            "hyperbolic_5_5",
            "coprime_bb",
            "toric_3d",
            "color_666",
            "surface",
            "shor_like",
            "random_two_block",
            "perturbed_hgp"
        ]
    );
}

#[test]
fn unified_family_contract_preserves_surface_d3() {
    let result =
        construct_css(CssFamilySpec::Surface(SurfaceSpec::rotated_square(3)).into()).unwrap();

    assert_eq!(result.construction_id, "surface_rotated");
    assert_eq!(result.requested_family_id, Some(RequestedFamilyId::Surface));
    assert_eq!(result.stats.n, 9);
    assert_eq!(result.stats.m_x, 4);
    assert_eq!(result.stats.m_z, 4);
    assert_eq!(result.stats.rank_x, 4);
    assert_eq!(result.stats.rank_z, 4);
    assert_eq!(result.stats.k, 1);
    assert_eq!(
        result.checks.h_x,
        fixture_rows("surface_rotated_d3_hx.json")
    );
    assert_eq!(
        result.checks.h_z,
        fixture_rows("surface_rotated_d3_hz.json")
    );

    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();

    let hx_json = SparseRowsMatrix::new(result.stats.n, result.checks.h_x.clone())
        .unwrap()
        .to_json_string();
    let hz_json = SparseRowsMatrix::new(result.stats.n, result.checks.h_z.clone())
        .unwrap()
        .to_json_string();
    assert_eq!(hx_json, fixture_text("surface_rotated_d3_hx.json"));
    assert_eq!(hz_json, fixture_text("surface_rotated_d3_hz.json"));

    let repeated =
        construct_css(CssFamilySpec::Surface(SurfaceSpec::rotated_square(3)).into()).unwrap();
    assert_eq!(
        serde_json::to_string(&result).unwrap(),
        serde_json::to_string(&repeated).unwrap(),
        "metadata serialization should be deterministic for the same construction"
    );

    let mut non_orthogonal = result.checks.h_z.clone();
    non_orthogonal[0] = vec![0, 3];
    assert_eq!(
        verify_css_orthogonality(result.stats.n, &result.checks.h_x, &non_orthogonal),
        Err(QecError::InvalidCssOrthogonality)
    );
}

#[test]
fn unified_family_contract_rejects_unknown_schema() {
    assert_eq!(
        parse_css_construction_json(
            r#"{"schema_version":2,"construction":"surface","distance":3}"#
        ),
        Err(QecError::UnsupportedCssConstructionSchemaVersion { version: 2 })
    );
}

#[test]
fn inline_json_and_rust_routes_lower_to_same_spec() {
    let inline = CssConstructionSpec::from_inline("surface_rotated:d=3").unwrap();
    let json = parse_css_construction_json(
        r#"{"schema_version":1,"construction":"surface","distance":3}"#,
    )
    .unwrap();
    let rust_api = CssFamilySpec::Surface(SurfaceSpec::rotated_square(3)).into();

    assert_eq!(inline, json);
    assert_eq!(json, rust_api);
}

#[test]
fn planned_families_have_no_callable_stub() {
    assert_eq!(
        CssFamilySpec::callable_requested_family_ids(),
        &[RequestedFamilyId::Surface, RequestedFamilyId::QuantumTanner]
    );
}

#[test]
fn generic_construction_identity_is_not_a_requested_family() {
    let result = construct_css(CssConstructionSpec::HypergraphProduct(
        HypergraphProductSpec {
            left: CLASSICAL_IDENTITY_2.clone(),
            right: CLASSICAL_IDENTITY_2.clone(),
        },
    ))
    .unwrap();

    assert_eq!(result.construction_id, "hypergraph_product");
    assert_eq!(result.requested_family_id, None);
}

#[test]
fn hypergraph_product_constructs_nontrivial_canonical_css_checks() {
    let check = CssClassicalCheckSpec {
        num_cols: 2,
        rows: vec![vec![0, 1]],
    };
    let result = construct_css(CssConstructionSpec::HypergraphProduct(
        HypergraphProductSpec {
            left: check.clone(),
            right: check,
        },
    ))
    .unwrap();

    assert_eq!(result.checks.h_x, vec![vec![0, 2, 4], vec![1, 3, 4]]);
    assert_eq!(result.checks.h_z, vec![vec![0, 1, 4], vec![2, 3, 4]]);
    assert_eq!(result.stats.n, 5);
    assert_eq!(result.stats.m_x, 2);
    assert_eq!(result.stats.m_z, 2);
    assert_eq!(result.stats.rank_x, 2);
    assert_eq!(result.stats.rank_z, 2);
    assert_eq!(result.stats.k, 1);
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();
}

#[test]
fn quantum_tanner_json_adapter_constructs_fixture() {
    let fixture = include_str!("fixtures/quantum_tanner/toric_d4.json");
    let spec = quantum_tanner_spec_from_json_str(fixture).unwrap();
    let request =
        format!(r#"{{"schema_version":1,"construction":"quantum_tanner","spec":{fixture}}}"#);

    let parsed = parse_css_construction_json(&request).unwrap();
    assert_eq!(parsed, CssFamilySpec::QuantumTanner(spec).into());

    let result = construct_css(parsed.clone()).unwrap();
    assert_eq!(result.construction_id, "quantum_tanner");
    assert_eq!(
        result.requested_family_id,
        Some(RequestedFamilyId::QuantumTanner)
    );
    assert_eq!(result.provenance.adapter, "quantum_tanner");
    assert_eq!(
        result.normalized_parameters["construction_mode"],
        serde_json::json!("lr_cayley_no_cover_v1")
    );
    assert_eq!(
        result.normalized_parameters["a_generator_indices"],
        serde_json::json!([4, 12])
    );
    assert_eq!(
        result.normalized_parameters["b_generator_indices"],
        serde_json::json!([1, 3])
    );
    assert_eq!(
        result.normalized_parameters["base_group"]["name"],
        serde_json::json!("Z4xZ4")
    );
    assert_eq!(
        result.normalized_parameters["base_group"]["order"],
        serde_json::json!(16)
    );
    assert_eq!(
        result.normalized_parameters["local_codes"]["h_a"],
        serde_json::json!([[1, 1]])
    );
    assert_eq!(
        result.normalized_parameters["local_codes"]["g_a"],
        serde_json::Value::Null
    );
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();

    let repeated = construct_css(parsed).unwrap();
    assert_eq!(
        serde_json::to_string(&result.normalized_parameters).unwrap(),
        serde_json::to_string(&repeated.normalized_parameters).unwrap(),
        "quantum Tanner normalized parameters should serialize deterministically"
    );
}

#[test]
fn legacy_built_in_json_adapter_constructs_steane() {
    let spec = parse_css_construction_json(
        r#"{"schema_version":1,"construction":"legacy_built_in","code_id":"steane"}"#,
    )
    .unwrap();
    let result = construct_css(spec).unwrap();

    assert_eq!(result.construction_id, "steane");
    assert_eq!(result.requested_family_id, None);
    assert_eq!(result.provenance.adapter, "built_in_css");
    assert_eq!(result.stats.n, 7);
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();
}

#[test]
fn construction_json_rejects_malformed_and_unknown_requests() {
    assert!(matches!(
        parse_css_construction_json("{"),
        Err(QecError::InvalidCssConstructionJson(_))
    ));
    assert!(matches!(
        parse_css_construction_json("[]"),
        Err(QecError::InvalidCssConstructionJson(message))
            if message == "construction request must be a JSON object"
    ));
    assert_eq!(
        parse_css_construction_json(r#"{"schema_version":1,"construction":"unknown"}"#),
        Err(QecError::UnknownCssConstruction {
            construction: "unknown".to_owned(),
        })
    );
    assert_eq!(
        parse_css_construction_json(r#"{"schema_version":1,"construction":"surface"}"#),
        Err(QecError::InvalidCssConstruction {
            construction: "surface".to_owned(),
            reason: "missing or invalid distance".to_owned(),
        })
    );
    assert_eq!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"quantum_tanner","spec":false}"#
        ),
        Err(QecError::InvalidCssConstruction {
            construction: "quantum_tanner".to_owned(),
            reason: "spec must be a JSON object".to_owned(),
        })
    );
    assert!(matches!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"hypergraph_product","left":{"num_cols":2,"rows":[]}}"#
        ),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "hypergraph_product" && reason.contains("missing field `right`")
    ));
    assert!(matches!(
        construct_css(CssConstructionSpec::HypergraphProduct(
            HypergraphProductSpec {
                left: CssClassicalCheckSpec {
                    num_cols: 2,
                    rows: vec![vec![0, 0]],
                },
                right: CLASSICAL_IDENTITY_2.clone(),
            },
        )),
        Err(QecError::DuplicateSparseRowSupport { .. })
    ));
}
