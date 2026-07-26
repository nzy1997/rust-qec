use std::path::PathBuf;

use qec_code::QecError;
use qec_code::codes::quantum_tanner::quantum_tanner_spec_from_json_str;
use qec_code::codes::toric_3d::Toric3dSpec;
use qec_code::css::SparseRowsMatrix;
use qec_code::family_contract::{
    CLASSICAL_IDENTITY_2, CssClassicalCheckSpec, CssConstructionSpec, CssFamilySpec,
    HypergraphProductSpec, RequestedFamilyId, SurfaceFamilySpec, construct_css,
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

fn assert_directional_json_error_contains(input: &str, expected_reason: &str) {
    match parse_css_construction_json(input) {
        Err(QecError::InvalidCssConstruction {
            construction,
            reason,
        }) => {
            assert_eq!(construction, "directional");
            assert!(
                reason.contains(expected_reason),
                "expected {reason:?} to contain {expected_reason:?}"
            );
        }
        other => panic!("expected invalid directional JSON, got {other:?}"),
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
        construct_css(CssFamilySpec::Surface(SurfaceFamilySpec { distance: 3 }).into()).unwrap();

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
        construct_css(CssFamilySpec::Surface(SurfaceFamilySpec { distance: 3 }).into()).unwrap();
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
    let rust_api = CssFamilySpec::Surface(SurfaceFamilySpec { distance: 3 }).into();

    assert_eq!(inline, json);
    assert_eq!(json, rust_api);
}

#[test]
fn inline_json_and_rust_routes_lower_to_same_toric_3d_spec() {
    let inline = CssConstructionSpec::from_inline("toric_3d:lx=3,ly=4,lz=5").unwrap();
    let json = parse_css_construction_json(
        r#"{"schema_version":1,"construction":"toric_3d","lx":3,"ly":4,"lz":5}"#,
    )
    .unwrap();
    let rust_api = CssFamilySpec::Toric3d(Toric3dSpec {
        lx: 3,
        ly: 4,
        lz: 5,
    })
    .into();

    assert_eq!(inline, json);
    assert_eq!(json, rust_api);
}

#[test]
fn planned_families_have_no_callable_stub() {
    assert_eq!(
        CssFamilySpec::callable_requested_family_ids(),
        &[
            RequestedFamilyId::Surface,
            RequestedFamilyId::QuantumTanner,
            RequestedFamilyId::Toric3d,
            RequestedFamilyId::RandomTwoBlock,
            RequestedFamilyId::Color666,
            RequestedFamilyId::Directional,
        ]
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
fn directional_json_adapter_constructs_square_fixture_with_deterministic_metadata() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/directional/square_ne2n_8x6.json")).unwrap();
    let request = serde_json::to_string(&fixture["request"]).unwrap();

    let parsed = parse_css_construction_json(&request).unwrap();
    let result = construct_css(parsed.clone()).unwrap();

    assert_eq!(result.construction_id, "directional");
    assert_eq!(
        result.requested_family_id,
        Some(RequestedFamilyId::Directional)
    );
    assert_eq!(result.stats.d_x, Some(3));
    assert_eq!(result.stats.d_z, Some(3));
    assert_eq!(
        result.checks.h_x,
        serde_json::from_value::<Vec<Vec<usize>>>(fixture["checks"]["h_x"].clone()).unwrap()
    );
    assert_eq!(
        result.checks.h_z,
        serde_json::from_value::<Vec<Vec<usize>>>(fixture["checks"]["h_z"].clone()).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&result.normalized_parameters).unwrap(),
        serde_json::json!({
            "torus": {
                "period_x": 8,
                "period_y": 6,
                "vertical_period_x_shift": 4
            },
            "route": "NE2N",
            "normalized_route": "NE2N",
            "route_support": [[0, 1], [1, 2], [3, 2], [4, 3]],
            "layout": {
                "x_ancilla_coset": "odd_even",
                "z_ancilla_coset": "even_odd"
            },
            "connectivity": "square"
        })
    );
    assert_eq!(result.provenance.adapter, "directional");
    assert_eq!(result.provenance.source, "CssFamilySpec::Directional");

    let repeated = construct_css(parsed).unwrap();
    assert_eq!(
        result.provenance.normalized_input_digest, repeated.provenance.normalized_input_digest,
        "directional normalized metadata digest should be deterministic"
    );
}

#[test]
fn directional_json_adapter_accepts_direct_top_level_spec() {
    let parsed = parse_css_construction_json(
        r#"{
            "schema_version": 1,
            "construction": "directional",
            "torus": {
                "period_x": 8,
                "period_y": 6,
                "vertical_period_x_shift": 4
            },
            "route": "NE2N",
            "connectivity": "square"
        }"#,
    )
    .unwrap();
    let result = construct_css(parsed).unwrap();

    assert_eq!(result.stats.d_x, Some(3));
    assert_eq!(result.stats.d_z, Some(3));
    assert_eq!(
        result.normalized_parameters["normalized_route"],
        serde_json::json!("NE2N")
    );
}

#[test]
fn directional_json_adapter_leaves_unknown_distances_unset() {
    let parsed = parse_css_construction_json(
        r#"{
            "schema_version": 1,
            "construction": "directional",
            "torus": {
                "period_x": 10,
                "period_y": 6,
                "vertical_period_x_shift": 0
            },
            "route": "NE2N",
            "connectivity": "square"
        }"#,
    )
    .unwrap();
    let result = construct_css(parsed).unwrap();

    assert_eq!(result.stats.n, 30);
    assert_eq!(result.stats.d_x, None);
    assert_eq!(result.stats.d_z, None);
}

#[test]
fn directional_json_adapter_canonicalizes_hex_route_spellings() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/directional/hex_ne3n_18x4.json")).unwrap();
    let expected_h_x: Vec<Vec<usize>> =
        serde_json::from_value(fixture["checks"]["h_x"].clone()).unwrap();
    let expected_h_z: Vec<Vec<usize>> =
        serde_json::from_value(fixture["checks"]["h_z"].clone()).unwrap();

    for route in ["NEEEN", "NE2EN"] {
        let mut request = fixture["request"].clone();
        request["spec"]["route"] = serde_json::json!(route);
        let parsed = parse_css_construction_json(&serde_json::to_string(&request).unwrap())
            .expect("equivalent hex route spelling should parse");
        let result = construct_css(parsed).expect("equivalent hex route spelling should construct");

        assert_eq!(result.stats.d_x, Some(4));
        assert_eq!(result.stats.d_z, Some(4));
        assert_eq!(
            result.normalized_parameters["normalized_route"],
            serde_json::json!("NE3N")
        );
        assert_eq!(result.checks.h_x, expected_h_x);
        assert_eq!(result.checks.h_z, expected_h_z);
    }
}

#[test]
fn directional_json_adapter_rejects_misspelled_fields() {
    let direct_top_level = r#"{
        "schema_version": 1,
        "construction": "directional",
        "torus": {
            "period_x": 8,
            "period_y": 6,
            "vertical_period_x_shfit": 4
        },
        "route": "NE2N",
        "connectivity": "square"
    }"#;
    assert_directional_json_error_contains(direct_top_level, "unknown field");
    assert_directional_json_error_contains(direct_top_level, "vertical_period_x_shfit");

    assert_directional_json_error_contains(
        r#"{
            "schema_version": 1,
            "construction": "directional",
            "spec": {
                "torus": {"period_x": 8, "period_y": 6, "vertical_period_x_shift": 4},
                "route": "NE2N",
                "connectivty": "hex"
            }
        }"#,
        "connectivty",
    );
    assert_directional_json_error_contains(
        r#"{
            "schema_version": 1,
            "construction": "directional",
            "spec": {
                "torus": {"period_x": 8, "period_y": 6, "vertical_period_x_shift": 4},
                "route": "NE2N"
            },
            "connectivty": "hex"
        }"#,
        "connectivty",
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
