mod support;

use std::collections::HashSet;

use qec_code::codes::built_in_css::{
    BivariateBicycleParams, BuiltInCssCodeSpec, BuiltInCssFamily, BuiltInCssParams,
    bivariate_bicycle_css_checks, built_in_css_catalog, built_in_css_checks,
    parse_built_in_css_code_spec,
};
use qec_code::codes::steane::Steane;
use qec_code::css::{CssCode, SparseRowsMatrix, sparse_rows_matrix_from_json_str};
use qec_code::{Pauli, QecError, StabilizerCode};
use serde_json::Value;
use support::apm_verifier::{
    ApmCssVerifierExpectations, ApmSparseMatrixView, GirthStatus, verify_apm_css_matrices,
};

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

#[derive(Debug, Clone, Copy)]
struct ExpectedApmEntry {
    code_id: &'static str,
    p: u64,
    n: u64,
    mx: u64,
    mz: u64,
    k: u64,
    distance_upper_bound: u64,
    rate: &'static str,
    f: [(u64, u64); 6],
    g: [(u64, u64); 6],
    column_component_modulus: u64,
    column_component_group: &'static str,
}

const EXPECTED_APM_TABLE_A1: &[ExpectedApmEntry] = &[
    ExpectedApmEntry {
        code_id: "apm_kasai:p=96",
        p: 96,
        n: 1152,
        mx: 288,
        mz: 288,
        k: 580,
        distance_upper_bound: 12,
        rate: "0.503",
        f: [(5, 41), (85, 77), (73, 66), (1, 0), (1, 72), (37, 9)],
        g: [(61, 15), (1, 24), (89, 62), (25, 22), (85, 93), (25, 78)],
        column_component_modulus: 32,
        column_component_group: "Z32",
    },
    ExpectedApmEntry {
        code_id: "apm_kasai:p=192",
        p: 192,
        n: 2304,
        mx: 576,
        mz: 576,
        k: 1156,
        distance_upper_bound: 14,
        rate: "0.502",
        f: [
            (71, 127),
            (97, 80),
            (67, 117),
            (163, 165),
            (25, 60),
            (187, 33),
        ],
        g: [
            (163, 165),
            (55, 183),
            (167, 79),
            (139, 41),
            (109, 78),
            (31, 27),
        ],
        column_component_modulus: 64,
        column_component_group: "Z32xZ2",
    },
];

fn required_field<'a>(object: &'a Value, path: &str, key: &str) -> Result<&'a Value, String> {
    object
        .get(key)
        .ok_or_else(|| format!("{path}.{key}: missing field"))
}

fn required_array_field<'a>(
    object: &'a Value,
    path: &str,
    key: &str,
) -> Result<&'a Vec<Value>, String> {
    let field_path = format!("{path}.{key}");
    required_field(object, path, key)?
        .as_array()
        .ok_or_else(|| format!("{field_path}: expected array"))
}

fn expect_len(path: &str, actual: usize, expected: usize) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{path}: expected length {expected}, got {actual}"))
    }
}

fn expect_u64_value(value: &Value, path: &str, expected: u64) -> Result<(), String> {
    match value.as_u64() {
        Some(actual) if actual == expected => Ok(()),
        Some(actual) => Err(format!("{path}: expected {expected}, got {actual}")),
        None => Err(format!("{path}: expected unsigned integer")),
    }
}

fn expect_str_value(value: &Value, path: &str, expected: &str) -> Result<(), String> {
    match value.as_str() {
        Some(actual) if actual == expected => Ok(()),
        Some(actual) => Err(format!("{path}: expected {expected:?}, got {actual:?}")),
        None => Err(format!("{path}: expected string")),
    }
}

fn expect_u64_field(object: &Value, path: &str, key: &str, expected: u64) -> Result<(), String> {
    let field_path = format!("{path}.{key}");
    expect_u64_value(required_field(object, path, key)?, &field_path, expected)
}

fn expect_str_field(object: &Value, path: &str, key: &str, expected: &str) -> Result<(), String> {
    let field_path = format!("{path}.{key}");
    expect_str_value(required_field(object, path, key)?, &field_path, expected)
}

fn expect_string_array_field(
    object: &Value,
    path: &str,
    key: &str,
    expected: &[&str],
) -> Result<(), String> {
    let values = required_array_field(object, path, key)?;
    let array_path = format!("{path}.{key}");
    expect_len(&array_path, values.len(), expected.len())?;
    for (index, expected_value) in expected.iter().enumerate() {
        let value_path = format!("{array_path}[{index}]");
        expect_str_value(&values[index], &value_path, expected_value)?;
    }
    Ok(())
}

fn validate_affine_family(
    entry: &Value,
    code_id: &str,
    family_key: &str,
    coefficient_keys: (&str, &str),
    expected_coefficients: &[(u64, u64); 6],
) -> Result<(), String> {
    let family = required_array_field(entry, code_id, family_key)?;
    expect_len(
        &format!("{code_id}.{family_key}"),
        family.len(),
        expected_coefficients.len(),
    )?;
    for (index, (expected_left, expected_right)) in expected_coefficients.iter().enumerate() {
        let coefficient_path = format!("{code_id} {family_key}[{index}]");
        expect_u64_field(&family[index], &coefficient_path, "i", index as u64)?;
        expect_u64_field(
            &family[index],
            &coefficient_path,
            coefficient_keys.0,
            *expected_left,
        )?;
        expect_u64_field(
            &family[index],
            &coefficient_path,
            coefficient_keys.1,
            *expected_right,
        )?;
    }
    Ok(())
}

fn validate_expected_code_shape(entry: &Value, expected: ExpectedApmEntry) -> Result<(), String> {
    let path = format!("{} expected_code_shape", expected.code_id);
    let shape = required_field(entry, expected.code_id, "expected_code_shape")?;
    expect_u64_field(shape, &path, "n", expected.n)?;
    expect_u64_field(shape, &path, "mx", expected.mx)?;
    expect_u64_field(shape, &path, "mz", expected.mz)?;
    expect_u64_field(shape, &path, "k", expected.k)?;
    expect_str_field(shape, &path, "rate", expected.rate)?;

    let distance_path = format!("{path}.distance");
    let distance = required_field(shape, &path, "distance")?;
    expect_str_field(distance, &distance_path, "kind", "upper_bound")?;
    expect_u64_field(
        distance,
        &distance_path,
        "value",
        expected.distance_upper_bound,
    )
}

fn validate_expected_weights(entry: &Value, code_id: &str) -> Result<(), String> {
    let path = format!("{code_id} expected_weights");
    let weights = required_field(entry, code_id, "expected_weights")?;
    expect_u64_field(weights, &path, "hx_row", 12)?;
    expect_u64_field(weights, &path, "hz_row", 12)?;
    expect_u64_field(weights, &path, "hx_column", 3)?;
    expect_u64_field(weights, &path, "hz_column", 3)?;
    expect_u64_field(weights, &path, "combined_data_qubit_degree", 6)
}

fn validate_girth(entry: &Value, code_id: &str) -> Result<(), String> {
    let path = format!("{code_id} girth");
    let girth = required_field(entry, code_id, "girth")?;
    expect_str_field(girth, &path, "kind", "lower_bound")?;
    expect_u64_field(girth, &path, "value", 6)
}

fn validate_required_commuting_pairs(
    entry: &Value,
    expected: ExpectedApmEntry,
) -> Result<(), String> {
    let pairs = required_array_field(entry, expected.code_id, "required_commuting_pairs")?;
    expect_len(
        &format!("{}.required_commuting_pairs", expected.code_id),
        pairs.len(),
        3,
    )?;
    let expected_pairs = [
        ("column_component:f0", "column_component:f1"),
        ("column_component:f0", "column_component:g0"),
        ("column_component:g0", "column_component:g1"),
    ];
    for (index, (left, right)) in expected_pairs.iter().enumerate() {
        let path = format!("{} required_commuting_pairs[{index}]", expected.code_id);
        expect_str_field(&pairs[index], &path, "left", left)?;
        expect_str_field(&pairs[index], &path, "right", right)?;
        expect_u64_field(
            &pairs[index],
            &path,
            "modulus",
            expected.column_component_modulus,
        )?;
    }
    Ok(())
}

fn validate_required_noncommuting_pairs(entry: &Value, code_id: &str) -> Result<(), String> {
    let pairs = required_array_field(entry, code_id, "required_noncommuting_pairs")?;
    expect_len(
        &format!("{code_id}.required_noncommuting_pairs"),
        pairs.len(),
        2,
    )?;
    let expected_pairs = [(0, 3), (1, 2)];
    for (index, (left, right)) in expected_pairs.iter().enumerate() {
        let path = format!("{code_id} required_noncommuting_pairs[{index}]");
        expect_u64_field(&pairs[index], &path, "left_index", *left)?;
        expect_u64_field(&pairs[index], &path, "right_index", *right)?;
    }
    Ok(())
}

fn validate_structural_expectations(
    entry: &Value,
    expected: ExpectedApmEntry,
) -> Result<(), String> {
    let path = format!("{} structural_expectations", expected.code_id);
    let structural = required_field(entry, expected.code_id, "structural_expectations")?;
    expect_u64_field(structural, &path, "active_block_rows", 3)?;
    expect_u64_field(structural, &path, "block_columns", 12)?;
    expect_u64_field(structural, &path, "apm_maps_per_family", 6)?;
    expect_u64_field(
        structural,
        &path,
        "column_component_modulus",
        expected.column_component_modulus,
    )?;
    expect_str_field(
        structural,
        &path,
        "column_component_group_status",
        "abelian",
    )?;
    expect_str_field(
        structural,
        &path,
        "column_component_group",
        expected.column_component_group,
    )
}

fn validate_provenance(entry: &Value, code_id: &str) -> Result<(), String> {
    let path = format!("{code_id} provenance");
    let provenance = required_field(entry, code_id, "provenance")?;
    expect_str_field(provenance, &path, "paper", "arXiv:2604.16209v1")?;
    expect_str_field(provenance, &path, "table", "Table A1")?;
    expect_string_array_field(
        provenance,
        &path,
        "source_grounded_fields",
        &[
            "P",
            "J",
            "L",
            "L2",
            "f",
            "g",
            "expected_code_shape.n",
            "expected_code_shape.k",
            "expected_code_shape.rate",
            "expected_code_shape.distance",
            "girth",
            "required_noncommuting_pairs",
        ],
    )?;
    expect_string_array_field(
        provenance,
        &path,
        "derived_fields",
        &[
            "expected_code_shape.mx",
            "expected_code_shape.mz",
            "expected_weights",
            "required_commuting_pairs",
            "structural_expectations",
        ],
    )
}

fn validate_references(entry: &Value, code_id: &str) -> Result<(), String> {
    let references = required_array_field(entry, code_id, "references")?;
    expect_len(&format!("{code_id}.references"), references.len(), 4)?;

    let paper_references = [
        ("https://arxiv.org/abs/2604.16209", "Appendix A / Table A1"),
        ("https://arxiv.org/pdf/2604.16209", "Appendix D.2"),
    ];
    for (index, (url, section)) in paper_references.iter().enumerate() {
        let path = format!("{code_id} references[{index}]");
        expect_str_field(&references[index], &path, "kind", "paper")?;
        expect_str_field(&references[index], &path, "url", url)?;
        expect_str_field(&references[index], &path, "section", section)?;
    }

    let local_references = [
        "drafts/construct_apm_css_code/README.md",
        "drafts/joint_BP_plus_PP/README.md",
    ];
    for (offset, local_path) in local_references.iter().enumerate() {
        let index = offset + paper_references.len();
        let path = format!("{code_id} references[{index}]");
        expect_str_field(&references[index], &path, "kind", "local")?;
        expect_str_field(&references[index], &path, "path", local_path)?;
    }

    Ok(())
}

fn validate_apm_table_a1_entry(
    entry: &Value,
    index: usize,
    expected: ExpectedApmEntry,
) -> Result<(), String> {
    let index_path = format!("entries[{index}]");
    expect_str_field(entry, &index_path, "code_id", expected.code_id)?;
    expect_u64_field(entry, expected.code_id, "P", expected.p)?;
    expect_u64_field(entry, expected.code_id, "J", 3)?;
    expect_u64_field(entry, expected.code_id, "L", 12)?;
    expect_u64_field(entry, expected.code_id, "L2", 6)?;
    validate_affine_family(entry, expected.code_id, "f", ("a", "b"), &expected.f)?;
    validate_affine_family(entry, expected.code_id, "g", ("c", "d"), &expected.g)?;
    validate_expected_code_shape(entry, expected)?;
    validate_expected_weights(entry, expected.code_id)?;
    validate_girth(entry, expected.code_id)?;
    validate_required_commuting_pairs(entry, expected)?;
    validate_required_noncommuting_pairs(entry, expected.code_id)?;
    validate_structural_expectations(entry, expected)?;
    validate_provenance(entry, expected.code_id)?;
    validate_references(entry, expected.code_id)
}

fn validate_apm_table_a1_manifest(manifest: &Value) -> std::result::Result<(), String> {
    expect_u64_field(manifest, "manifest", "schema_version", 1)?;
    expect_str_field(manifest, "manifest", "manifest_id", "apm_kasai_table_a1")?;
    let entries = required_array_field(manifest, "manifest", "entries")?;
    expect_len(
        "manifest.entries",
        entries.len(),
        EXPECTED_APM_TABLE_A1.len(),
    )?;
    for (index, expected) in EXPECTED_APM_TABLE_A1.iter().copied().enumerate() {
        validate_apm_table_a1_entry(&entries[index], index, expected)?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct ApmSparseFixture {
    num_cols: usize,
    rows: Vec<Vec<usize>>,
}

fn load_apm_sparse_fixture(input: &str) -> ApmSparseFixture {
    let matrix = sparse_rows_matrix_from_json_str(input).unwrap();
    ApmSparseFixture {
        num_cols: matrix.num_cols(),
        rows: matrix.rows().to_vec(),
    }
}

fn apm_p96_expectations() -> ApmCssVerifierExpectations {
    ApmCssVerifierExpectations {
        num_cols: Some(1152),
        mx: Some(288),
        mz: Some(288),
        row_weight_x: Some(12),
        row_weight_z: Some(12),
        column_weight_x: Some(3),
        column_weight_z: Some(3),
        k: Some(580),
        orthogonal: Some(true),
        girth_lower_bound: Some(6),
    }
}

fn verify_apm_p96_fixture_stats(
    hx: &ApmSparseFixture,
    hz: &ApmSparseFixture,
) -> std::result::Result<support::apm_verifier::ApmCssVerifierReport, String> {
    verify_apm_css_matrices(
        ApmSparseMatrixView {
            name: "Hx",
            num_cols: hx.num_cols,
            rows: &hx.rows,
        },
        ApmSparseMatrixView {
            name: "Hz",
            num_cols: hz.num_cols,
            rows: &hz.rows,
        },
        &apm_p96_expectations(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DocumentedAffineMap {
    a: u64,
    b: u64,
    modulus: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DocumentedApmShape {
    n: u64,
    mx: u64,
    mz: u64,
}

fn gcd_u64(mut lhs: u64, mut rhs: u64) -> u64 {
    while rhs != 0 {
        let next = lhs % rhs;
        lhs = rhs;
        rhs = next;
    }
    lhs
}

fn parse_documented_affine_map(
    a: u64,
    b: u64,
    modulus: u64,
) -> Result<DocumentedAffineMap, String> {
    if modulus == 0 {
        return Err("affine map modulus must be positive".to_owned());
    }
    if gcd_u64(a, modulus) != 1 {
        return Err(format!("affine slope {a} is not a unit modulo {modulus}"));
    }
    Ok(DocumentedAffineMap {
        a: a % modulus,
        b: b % modulus,
        modulus,
    })
}

fn mod_i128(value: i128, modulus: u64) -> u64 {
    let modulus = modulus as i128;
    value.rem_euclid(modulus) as u64
}

fn affine_commutation_residual(lhs: DocumentedAffineMap, rhs: DocumentedAffineMap) -> u64 {
    assert_eq!(
        lhs.modulus, rhs.modulus,
        "affine residual requires a shared modulus"
    );
    mod_i128(
        lhs.a as i128 * rhs.b as i128 + lhs.b as i128
            - rhs.a as i128 * lhs.b as i128
            - rhs.b as i128,
        lhs.modulus,
    )
}

fn documented_apm_shape(p: u64, j: u64, l: u64) -> DocumentedApmShape {
    DocumentedApmShape {
        n: p * l,
        mx: p * j,
        mz: p * j,
    }
}

fn apm_entry_by_code_id<'a>(manifest: &'a Value, code_id: &str) -> &'a Value {
    manifest["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["code_id"] == code_id)
        .unwrap()
}

fn u64_json(value: &Value) -> u64 {
    value.as_u64().unwrap()
}

fn documented_manifest_map(
    entry: &Value,
    label: &str,
    modulus: u64,
) -> Result<DocumentedAffineMap, String> {
    let label = label.strip_prefix("column_component:").unwrap_or(label);
    let (family, index) = label.split_at(1);
    let index: usize = index.parse().unwrap();
    let map = &entry[family][index];
    match family {
        "f" => parse_documented_affine_map(u64_json(&map["a"]), u64_json(&map["b"]), modulus),
        "g" => parse_documented_affine_map(u64_json(&map["c"]), u64_json(&map["d"]), modulus),
        _ => panic!("unknown APM family label {label}"),
    }
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
fn apm_table_a1_manifest_pins_table_a1_reference_data() {
    let manifest: Value =
        serde_json::from_str(include_str!("fixtures/apm/table_a1_manifest.json")).unwrap();

    validate_apm_table_a1_manifest(&manifest).unwrap();
}

#[test]
fn apm_table_a1_manifest_rejects_mutated_affine_coefficient() {
    let mut manifest: Value =
        serde_json::from_str(include_str!("fixtures/apm/table_a1_manifest.json")).unwrap();
    manifest["entries"][0]["f"][0]["a"] = Value::from(7);

    let err = validate_apm_table_a1_manifest(&manifest).unwrap_err();
    assert!(
        err.contains("apm_kasai:p=96") && err.contains("f[0].a"),
        "error should identify the changed coefficient and code id: {err}"
    );
}

#[test]
fn apm_p96_fixture_matches_reference_stats() {
    let hx = load_apm_sparse_fixture(include_str!("fixtures/apm/p96_hx.json"));
    let hz = load_apm_sparse_fixture(include_str!("fixtures/apm/p96_hz.json"));

    let report = verify_apm_p96_fixture_stats(&hx, &hz).unwrap();
    assert!(report.orthogonal);
    assert_eq!(report.num_cols, 1152);
    assert_eq!(report.mx, 288);
    assert_eq!(report.mz, 288);
    assert_eq!(report.k, 580);
    assert_eq!(
        report.x.row_weight,
        support::apm_verifier::WeightStats {
            min: 12,
            average: 12.0,
            max: 12
        }
    );
    assert_eq!(
        report.z.row_weight,
        support::apm_verifier::WeightStats {
            min: 12,
            average: 12.0,
            max: 12
        }
    );
    assert_eq!(
        report.x.column_weight,
        support::apm_verifier::WeightStats {
            min: 3,
            average: 3.0,
            max: 3
        }
    );
    assert_eq!(
        report.z.column_weight,
        support::apm_verifier::WeightStats {
            min: 3,
            average: 3.0,
            max: 3
        }
    );
    assert!(matches!(report.x.girth, GirthStatus::Exact(girth) if girth >= 6));
    assert!(matches!(report.z.girth, GirthStatus::Exact(girth) if girth >= 6));
}

#[test]
fn apm_p96_fixture_rejects_mutated_support() {
    let hx = load_apm_sparse_fixture(include_str!("fixtures/apm/p96_hx.json"));
    let mut hz = load_apm_sparse_fixture(include_str!("fixtures/apm/p96_hz.json"));
    let replacement = (0..hz.num_cols)
        .find(|candidate| !hz.rows[0].contains(candidate))
        .unwrap();
    hz.rows[0][0] = replacement;
    hz.rows[0].sort_unstable();

    let err = verify_apm_p96_fixture_stats(&hx, &hz).unwrap_err();
    assert!(
        err.contains("column") || err.contains("overlap") || err.contains("rank"),
        "mutating one support should trip a structural verifier, got: {err}"
    );
}

#[test]
fn apm_p96_fixture_rejects_duplicate_support_before_rank_checks() {
    let hx = load_apm_sparse_fixture(include_str!("fixtures/apm/p96_hx.json"));
    let mut hz = load_apm_sparse_fixture(include_str!("fixtures/apm/p96_hz.json"));
    hz.rows[0][1] = hz.rows[0][0];

    let err = verify_apm_p96_fixture_stats(&hx, &hz).unwrap_err();
    assert!(err.contains("duplicate support"));
}

#[test]
fn apm_p96_fixture_rejects_out_of_range_support_before_rank_checks() {
    let hx = load_apm_sparse_fixture(include_str!("fixtures/apm/p96_hx.json"));
    let mut hz = load_apm_sparse_fixture(include_str!("fixtures/apm/p96_hz.json"));
    hz.rows[0][0] = hz.num_cols;

    let err = verify_apm_p96_fixture_stats(&hx, &hz).unwrap_err();
    assert!(err.contains("out-of-range support"));
}

#[test]
fn apm_p96_fixture_rejects_structural_stat_mismatches() {
    let hx = load_apm_sparse_fixture(include_str!("fixtures/apm/p96_hx.json"));
    let hz = load_apm_sparse_fixture(include_str!("fixtures/apm/p96_hz.json"));

    let mut wrong_width = hz.clone();
    wrong_width.num_cols -= 1;
    let err = verify_apm_p96_fixture_stats(&hx, &wrong_width).unwrap_err();
    assert!(err.contains("out-of-range support"));

    let mut missing_row = hz.clone();
    missing_row.rows.pop();
    let err = verify_apm_p96_fixture_stats(&hx, &missing_row).unwrap_err();
    assert!(err.contains("expected mz=288"));

    let mut short_row = hz.clone();
    short_row.rows[0].pop();
    let err = verify_apm_p96_fixture_stats(&hx, &short_row).unwrap_err();
    assert!(err.contains("expected Hz row weight 12"));
}

#[test]
fn apm_p96_fixture_rejects_balanced_nonorthogonal_swap() {
    let hx = load_apm_sparse_fixture(include_str!("fixtures/apm/p96_hx.json"));
    let mut hz = load_apm_sparse_fixture(include_str!("fixtures/apm/p96_hz.json"));
    hz.rows[0][0] = 58;
    hz.rows[1][0] = 69;

    let err = verify_apm_p96_fixture_stats(&hx, &hz).unwrap_err();
    assert!(
        err.contains("expected orthogonal=true, got false"),
        "balanced swap should preserve degrees but break orthogonality, got: {err}"
    );
}

#[test]
fn apm_p96_fixture_rejects_low_rank_shape() {
    let rows = (0..288)
        .map(|row| {
            let start = (row % 96) * 12;
            (start..start + 12).collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let low_rank = ApmSparseFixture {
        num_cols: 1152,
        rows,
    };

    let err = verify_apm_p96_fixture_stats(&low_rank, &low_rank).unwrap_err();
    assert!(err.contains("expected k=580"));
}

#[test]
fn apm_contract_doc_examples_compile() {
    let doc = include_str!("../doc/apm_css.md");
    assert!(doc.contains("AffineMap { a, b, modulus }"));
    assert!(doc.contains("Delta"));
    assert!(doc.contains("Gamma"));
    assert!(doc.contains("qec-code/tests/fixtures/apm/table_a1_manifest.json"));

    let manifest: Value =
        serde_json::from_str(include_str!("fixtures/apm/table_a1_manifest.json")).unwrap();
    let p96 = apm_entry_by_code_id(&manifest, "apm_kasai:p=96");

    assert_eq!(
        documented_apm_shape(
            u64_json(&p96["P"]),
            u64_json(&p96["J"]),
            u64_json(&p96["L"])
        ),
        DocumentedApmShape {
            n: 1152,
            mx: 288,
            mz: 288,
        }
    );

    let gamma_pair = &p96["required_commuting_pairs"][0];
    let gamma_modulus = u64_json(&gamma_pair["modulus"]);
    let gamma_left =
        documented_manifest_map(p96, gamma_pair["left"].as_str().unwrap(), gamma_modulus).unwrap();
    let gamma_right =
        documented_manifest_map(p96, gamma_pair["right"].as_str().unwrap(), gamma_modulus).unwrap();
    assert_eq!(affine_commutation_residual(gamma_left, gamma_right), 0);

    let noncommuting_pair = &p96["required_noncommuting_pairs"][0];
    let noncommuting_left = documented_manifest_map(
        p96,
        &format!("f{}", u64_json(&noncommuting_pair["left_index"])),
        u64_json(&p96["P"]),
    )
    .unwrap();
    let noncommuting_right = documented_manifest_map(
        p96,
        &format!("g{}", u64_json(&noncommuting_pair["right_index"])),
        u64_json(&p96["P"]),
    )
    .unwrap();
    assert_ne!(
        affine_commutation_residual(noncommuting_left, noncommuting_right),
        0
    );

    let invalid = parse_documented_affine_map(2, 0, 96).unwrap_err();
    assert!(invalid.contains("not a unit modulo 96"));
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
            "bb:lx=<period-x>,ly=<period-y>,a=<dx>:<dy>|...,b=<dx>:<dy>|...",
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
fn bb72_fixed_alias_is_generic_bivariate_bicycle_preset() {
    let fixed = built_in_css_checks("bb72").unwrap();
    let mut generic = bivariate_bicycle_css_checks(bb72_bivariate_bicycle_params()).unwrap();
    generic.code_id = "bb72";

    assert_eq!(fixed, generic);
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
fn built_in_css_checks_accepts_bivariate_bicycle_specs() {
    let spec = "bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0";
    let expected = bivariate_bicycle_css_checks(bb144_bivariate_bicycle_params()).unwrap();

    assert_eq!(
        parse_built_in_css_code_spec(spec),
        Ok(BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::BivariateBicycle,
            params: BuiltInCssParams::BivariateBicycle(bb144_bivariate_bicycle_params()),
        })
    );
    assert_eq!(built_in_css_checks(spec), Ok(expected));
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
