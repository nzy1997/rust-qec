mod support;

use std::collections::HashSet;

use qec_code::codes::built_in_css::{
    bivariate_bicycle_css_checks, built_in_css_catalog, built_in_css_checks,
    parse_built_in_css_code_spec, BivariateBicycleParams, BuiltInCssChecks, BuiltInCssCodeSpec,
    BuiltInCssFamily, BuiltInCssParams,
};
use qec_code::codes::quantum_tanner::{
    enumerate_quantum_tanner_cayley_faces, quantum_tanner_css_checks,
    quantum_tanner_css_checks_from_validated_parts, quantum_tanner_local_code_tensor_dual,
    quantum_tanner_spec_from_json_str, validate_quantum_tanner_group_table, ExplicitFiniteGroup,
    QuantumTannerCayleyComplex, QuantumTannerConstructionMode,
    QuantumTannerLocalCodeTensorDual, QuantumTannerLocalCodes, QuantumTannerSpec,
    ValidatedFiniteGroup,
};
use qec_code::codes::steane::Steane;
use qec_code::css::{sparse_rows_matrix_from_json_str, CssCode, SparseRowsMatrix};
use qec_code::distance::compute_distance;
use qec_code::{Pauli, QecError, StabilizerCode};
use serde_json::Value;
use support::apm_verifier::{
    verify_apm_css_matrices, ApmCssVerifierExpectations, ApmCssVerifierReport, ApmSparseMatrixView,
    GirthStatus, WeightStats,
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

fn apm_p192_expectations() -> ApmCssVerifierExpectations {
    ApmCssVerifierExpectations {
        num_cols: Some(2304),
        mx: Some(576),
        mz: Some(576),
        row_weight_x: Some(12),
        row_weight_z: Some(12),
        column_weight_x: Some(3),
        column_weight_z: Some(3),
        k: Some(1156),
        orthogonal: Some(true),
        girth_lower_bound: Some(6),
    }
}

fn verify_apm_checks(
    checks: &BuiltInCssChecks,
    expectations: &ApmCssVerifierExpectations,
) -> std::result::Result<ApmCssVerifierReport, String> {
    verify_apm_css_matrices(
        ApmSparseMatrixView {
            name: "Hx",
            num_cols: checks.num_cols,
            rows: &checks.hx,
        },
        ApmSparseMatrixView {
            name: "Hz",
            num_cols: checks.num_cols,
            rows: &checks.hz,
        },
        expectations,
    )
}

fn verify_apm_p96_fixture_stats(
    hx: &ApmSparseFixture,
    hz: &ApmSparseFixture,
) -> std::result::Result<ApmCssVerifierReport, String> {
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

fn verify_small_apm_sparse_rows(
    hx_num_cols: usize,
    hx_rows: &[Vec<usize>],
    hz_num_cols: usize,
    hz_rows: &[Vec<usize>],
    expectations: &ApmCssVerifierExpectations,
) -> std::result::Result<ApmCssVerifierReport, String> {
    verify_apm_css_matrices(
        ApmSparseMatrixView {
            name: "Hx",
            num_cols: hx_num_cols,
            rows: hx_rows,
        },
        ApmSparseMatrixView {
            name: "Hz",
            num_cols: hz_num_cols,
            rows: hz_rows,
        },
        expectations,
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
fn apm_verifier_rejects_invalid_small_shapes_before_reporting() {
    let one_column_row = vec![vec![0]];
    let empty_rows: Vec<Vec<usize>> = Vec::new();
    let expectations = ApmCssVerifierExpectations::default();

    let err = verify_small_apm_sparse_rows(2, &one_column_row, 3, &one_column_row, &expectations)
        .unwrap_err();
    assert!(err.contains("expected shared width"));

    let err =
        verify_small_apm_sparse_rows(0, &empty_rows, 0, &empty_rows, &expectations).unwrap_err();
    assert!(err.contains("invalid sparse-rows width 0"));

    let err = verify_small_apm_sparse_rows(1, &one_column_row, 1, &one_column_row, &expectations)
        .unwrap_err();
    assert!(err.contains("invalid CSS dimensions"));
}

#[test]
fn apm_verifier_reports_acyclic_girth_and_empty_row_weights() {
    let hx_rows = vec![vec![0], vec![1]];
    let hz_rows: Vec<Vec<usize>> = Vec::new();

    let report = verify_small_apm_sparse_rows(
        3,
        &hx_rows,
        3,
        &hz_rows,
        &ApmCssVerifierExpectations::default(),
    )
    .unwrap();

    assert_eq!(report.x.girth, GirthStatus::Acyclic);
    assert_eq!(report.z.girth, GirthStatus::Acyclic);
    assert!(report.x.girth.meets_lower_bound(100));
    assert!(GirthStatus::AtLeast(6).meets_lower_bound(6));
    assert!(!GirthStatus::AtLeast(6).meets_lower_bound(8));
    assert_eq!(
        report.z.row_weight,
        WeightStats {
            min: 0,
            average: 0.0,
            max: 0,
        }
    );
}

#[test]
fn apm_verifier_rejects_small_stat_expectation_mismatches() {
    let hx_rows = vec![vec![0], vec![1]];
    let hz_rows: Vec<Vec<usize>> = Vec::new();

    let err = verify_small_apm_sparse_rows(
        3,
        &hx_rows,
        3,
        &hz_rows,
        &ApmCssVerifierExpectations {
            num_cols: Some(4),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(err.contains("expected num_cols=4"));

    let err = verify_small_apm_sparse_rows(
        3,
        &hx_rows,
        3,
        &hz_rows,
        &ApmCssVerifierExpectations {
            mx: Some(3),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(err.contains("expected mx=3"));

    let err = verify_small_apm_sparse_rows(
        3,
        &hx_rows,
        3,
        &hz_rows,
        &ApmCssVerifierExpectations {
            row_weight_x: Some(2),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(err.contains("expected Hx row weight 2"));

    let err = verify_small_apm_sparse_rows(
        3,
        &hx_rows,
        3,
        &hz_rows,
        &ApmCssVerifierExpectations {
            column_weight_x: Some(1),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(err.contains("expected Hx column weight 1"));
}

#[test]
fn apm_verifier_rejects_girth_below_expected_bound_on_either_side() {
    let cycle_four_rows = vec![vec![0, 1], vec![0, 1]];
    let empty_rows: Vec<Vec<usize>> = Vec::new();
    let expectations = ApmCssVerifierExpectations {
        girth_lower_bound: Some(6),
        ..Default::default()
    };

    let err = verify_small_apm_sparse_rows(2, &cycle_four_rows, 2, &empty_rows, &expectations)
        .unwrap_err();
    assert!(err.contains("expected Hx Tanner girth >= 6"));

    let err = verify_small_apm_sparse_rows(2, &empty_rows, 2, &cycle_four_rows, &expectations)
        .unwrap_err();
    assert!(err.contains("expected Hz Tanner girth >= 6"));
}

fn extract_marked_json(doc: &str, marker: &str) -> Result<Value, String> {
    let marker_text = format!("<!-- {marker} -->");
    let after_marker = doc
        .split_once(&marker_text)
        .map(|(_, after)| after)
        .ok_or_else(|| format!("missing marker {marker_text}"))?;
    let fence_start = after_marker
        .find("```json")
        .ok_or_else(|| format!("missing json fence after {marker_text}"))?;
    let json_start = fence_start + "```json".len();
    let json_tail = &after_marker[json_start..];
    let json_end = json_tail
        .find("```")
        .ok_or_else(|| format!("missing closing json fence after {marker_text}"))?;
    serde_json::from_str(json_tail[..json_end].trim())
        .map_err(|error| format!("invalid json after {marker_text}: {error}"))
}

fn usize_array(value: &Value, path: &str) -> Vec<usize> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{path}: expected array"))
        .iter()
        .map(|entry| {
            entry
                .as_u64()
                .unwrap_or_else(|| panic!("{path}: expected unsigned integer")) as usize
        })
        .collect()
}

fn usize_matrix(value: &Value, path: &str) -> Vec<Vec<usize>> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{path}: expected matrix"))
        .iter()
        .enumerate()
        .map(|(row_index, row)| usize_array(row, &format!("{path}[{row_index}]")))
        .collect()
}

fn assert_group_table_shape(table: &[Vec<usize>], order: usize) {
    assert_eq!(table.len(), order, "multiplication table row count");
    for row in table {
        assert_eq!(row.len(), order, "multiplication table column count");
        for &entry in row {
            assert!(
                entry < order,
                "table entry {entry} out of range for order {order}"
            );
        }
    }
}

fn inverse_index(table: &[Vec<usize>], identity: usize, element: usize) -> Option<usize> {
    (0..table.len()).find(|&candidate| {
        table[element][candidate] == identity && table[candidate][element] == identity
    })
}

fn generators_are_symmetric(table: &[Vec<usize>], identity: usize, generators: &[usize]) -> bool {
    generators.iter().all(|&generator| {
        inverse_index(table, identity, generator)
            .map(|inverse| generators.contains(&inverse))
            .unwrap_or(false)
    })
}

const QUANTUM_TANNER_FIXTURE_DIR: &str = "tests/fixtures/quantum_tanner";
const QUANTUM_TANNER_VERIFIER_COMMAND: &str =
    "cargo test -p qec-code quantum_tanner_fixture_catalog_has_grounded_cases -q";

#[derive(Clone, Copy)]
enum QuantumTannerExpectedResult<'a> {
    Success,
    Rejection(&'a str),
}

fn qec_code_manifest_fixture_path(rel_path: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel_path)
}

fn load_quantum_tanner_fixture(path: &str) -> Value {
    let full_path = qec_code_manifest_fixture_path(path);
    let contents = std::fs::read_to_string(&full_path)
        .unwrap_or_else(|error| panic!("fixture {full_path:?} should be readable: {error}"));
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("fixture {full_path:?} should be valid JSON: {error}"))
}

fn nonempty_string_field<'a>(object: &'a Value, path: &str, key: &str) -> Result<&'a str, String> {
    let field_path = format!("{path}.{key}");
    let value = required_field(object, path, key)?
        .as_str()
        .ok_or_else(|| format!("{field_path}: expected string"))?;
    if value.trim().is_empty() {
        Err(format!("{field_path}: expected nonempty string"))
    } else {
        Ok(value)
    }
}

fn expect_u64_array_field(
    object: &Value,
    path: &str,
    key: &str,
    expected: &[u64],
) -> Result<(), String> {
    let values = required_array_field(object, path, key)?;
    let array_path = format!("{path}.{key}");
    expect_len(&array_path, values.len(), expected.len())?;
    for (index, expected_value) in expected.iter().enumerate() {
        expect_u64_value(
            &values[index],
            &format!("{array_path}[{index}]"),
            *expected_value,
        )?;
    }
    Ok(())
}

fn expect_usize_array_value(
    value: &Value,
    path: &str,
    expected: &[usize],
) -> Result<Vec<usize>, String> {
    let actual = usize_array(value, path);
    if actual.as_slice() == expected {
        Ok(actual)
    } else {
        Err(format!("{path}: expected {expected:?}, got {actual:?}"))
    }
}

fn expect_quantum_tanner_manifest_reference(
    reference: &Value,
    path: &str,
    kind: &str,
    key: &str,
    value: &str,
) -> Result<(), String> {
    expect_str_field(reference, path, "kind", kind)?;
    expect_str_field(reference, path, key, value)
}

fn validate_quantum_tanner_references(entry: &Value, path: &str) -> Result<(), String> {
    let references = required_array_field(entry, path, "references")?;
    let references_path = format!("{path}.references");
    expect_len(&references_path, references.len(), 5)?;
    expect_quantum_tanner_manifest_reference(
        &references[0],
        &format!("{references_path}[0]"),
        "local",
        "path",
        "drafts/qLDPC/src/qldpc/codes/quantum.py",
    )?;
    expect_quantum_tanner_manifest_reference(
        &references[1],
        &format!("{references_path}[1]"),
        "local",
        "path",
        "drafts/qLDPC/src/qldpc/objects.py",
    )?;
    expect_quantum_tanner_manifest_reference(
        &references[2],
        &format!("{references_path}[2]"),
        "local",
        "path",
        "drafts/qLDPC/src/qldpc/codes/quantum_test.py",
    )?;
    expect_quantum_tanner_manifest_reference(
        &references[3],
        &format!("{references_path}[3]"),
        "external",
        "url",
        "https://github.com/qLDPCOrg/qLDPC",
    )?;
    expect_quantum_tanner_manifest_reference(
        &references[4],
        &format!("{references_path}[4]"),
        "external",
        "url",
        "https://github.com/RebKatRad/qTanner",
    )
}

fn validate_quantum_tanner_provenance(entry: &Value, path: &str) -> Result<(), String> {
    let provenance = required_field(entry, path, "provenance")?;
    let provenance_path = format!("{path}.provenance");
    expect_str_field(
        provenance,
        &provenance_path,
        "kind",
        "reference_derived_known_answer",
    )?;
    let summary = nonempty_string_field(provenance, &provenance_path, "summary")?;
    if !summary.contains("no qLDPC implementation code is copied") {
        return Err(format!(
            "{provenance_path}.summary: expected no-code-copy provenance"
        ));
    }
    expect_string_array_field(
        provenance,
        &provenance_path,
        "source_grounded_fields",
        &[
            "construction_mode",
            "base_group",
            "a_generator_indices",
            "b_generator_indices",
            "local_codes",
            "expected_result",
        ],
    )
}

fn validate_quantum_tanner_contract_reference(entry: &Value, path: &str) -> Result<(), String> {
    let contract_reference = required_field(entry, path, "contract_reference")?;
    let reference_path = format!("{path}.contract_reference");
    expect_u64_field(contract_reference, &reference_path, "issue", 177)?;
    expect_str_field(
        contract_reference,
        &reference_path,
        "path",
        "qec-code/doc/quantum_tanner.md",
    )?;
    expect_u64_field(contract_reference, &reference_path, "schema_version", 1)
}

fn validate_quantum_tanner_expected_result(
    entry: &Value,
    path: &str,
    expected_result: QuantumTannerExpectedResult<'_>,
) -> Result<(), String> {
    let expected = required_field(entry, path, "expected_result")?;
    let expected_path = format!("{path}.expected_result");
    match expected_result {
        QuantumTannerExpectedResult::Success => {
            expect_str_field(expected, &expected_path, "kind", "success")?;
            expect_u64_field(expected, &expected_path, "n", 16)?;
            expect_u64_field(expected, &expected_path, "k", 2)?;
            expect_u64_field(expected, &expected_path, "d", 4)?;
            expect_u64_field(expected, &expected_path, "check_weight", 4)
        }
        QuantumTannerExpectedResult::Rejection(reason) => {
            expect_str_field(expected, &expected_path, "kind", "rejection")?;
            expect_str_field(expected, &expected_path, "reason", reason)
        }
    }
}

fn validate_quantum_tanner_expected_result_shape(entry: &Value, path: &str) -> Result<(), String> {
    let expected = required_field(entry, path, "expected_result")?;
    let expected_path = format!("{path}.expected_result");
    match nonempty_string_field(expected, &expected_path, "kind")? {
        "success" => {
            for key in ["n", "k", "d", "check_weight"] {
                required_field(expected, &expected_path, key)?
                    .as_u64()
                    .ok_or_else(|| format!("{expected_path}.{key}: expected unsigned integer"))?;
            }
            Ok(())
        }
        "rejection" => {
            nonempty_string_field(expected, &expected_path, "reason")?;
            Ok(())
        }
        other => Err(format!(
            "{expected_path}.kind: expected success or rejection, got {other:?}"
        )),
    }
}

fn validate_nonempty_u64_array_field(object: &Value, path: &str, key: &str) -> Result<(), String> {
    let values = required_array_field(object, path, key)?;
    let array_path = format!("{path}.{key}");
    if values.is_empty() {
        return Err(format!("{array_path}: expected nonempty array"));
    }
    for (index, value) in values.iter().enumerate() {
        value
            .as_u64()
            .ok_or_else(|| format!("{array_path}[{index}]: expected unsigned integer"))?;
    }
    Ok(())
}

fn validate_z4xz4_table(table: &[Vec<usize>], path: &str) -> Result<(), String> {
    for left in 0..16 {
        let (left_x, left_y) = (left / 4, left % 4);
        for right in 0..16 {
            let (right_x, right_y) = (right / 4, right % 4);
            let expected = 4 * ((left_x + right_x) % 4) + ((left_y + right_y) % 4);
            if table[left][right] != expected {
                return Err(format!(
                    "{path}[{left}][{right}]: expected {expected}, got {}",
                    table[left][right]
                ));
            }
        }
    }
    Ok(())
}

fn validate_quantum_tanner_local_codes(
    fixture: &Value,
    path: &str,
    expected_widths: Option<(usize, usize)>,
) -> Result<(), String> {
    let local_codes = required_field(fixture, path, "local_codes")?;
    let local_path = format!("{path}.local_codes");
    expect_str_field(local_codes, &local_path, "matrix_role", "parity_check")?;
    expect_str_field(local_codes, &local_path, "field", "GF(2)")?;

    let h_a = usize_matrix(
        required_field(local_codes, &local_path, "h_a")?,
        &format!("{local_path}.h_a"),
    );
    let h_b = usize_matrix(
        required_field(local_codes, &local_path, "h_b")?,
        &format!("{local_path}.h_b"),
    );
    if h_a.is_empty() || h_a.iter().any(Vec::is_empty) {
        return Err(format!("{local_path}.h_a: expected nonempty rows"));
    }
    if h_b.is_empty() || h_b.iter().any(Vec::is_empty) {
        return Err(format!("{local_path}.h_b: expected nonempty rows"));
    }
    if let Some((a_width, b_width)) = expected_widths {
        if (a_width, b_width) == (2, 2) {
            if h_a != vec![vec![1, 1]] {
                return Err(format!("{local_path}.h_a: expected [[1, 1]], got {h_a:?}"));
            }
            if h_b != vec![vec![1, 1]] {
                return Err(format!("{local_path}.h_b: expected [[1, 1]], got {h_b:?}"));
            }
        }
        if h_a.iter().any(|row| row.len() != a_width) {
            return Err(format!("{local_path}.h_a: expected row width {a_width}"));
        }
        if h_b.iter().any(|row| row.len() != b_width) {
            return Err(format!("{local_path}.h_b: expected row width {b_width}"));
        }
    }
    if h_a.iter().chain(&h_b).flatten().any(|&bit| bit > 1) {
        return Err(format!("{local_path}: expected GF(2) entries"));
    }
    Ok(())
}

fn validate_quantum_tanner_fixture(
    fixture: &Value,
    path: &str,
    fixture_id: &str,
    expected_result: QuantumTannerExpectedResult<'_>,
) -> Result<(), String> {
    expect_str_field(fixture, path, "fixture_id", fixture_id)?;
    expect_str_field(fixture, path, "construction_mode", "lr_cayley_no_cover_v1")?;
    let group = required_field(fixture, path, "base_group")?;
    let group_path = format!("{path}.base_group");
    expect_str_field(group, &group_path, "name", "Z4xZ4")?;
    expect_str_field(
        group,
        &group_path,
        "element_order",
        "id = 4*x + y for (x,y) in Z4 x Z4",
    )?;
    expect_u64_field(group, &group_path, "order", 16)?;
    expect_u64_field(group, &group_path, "identity", 0)?;

    let table_path = format!("{group_path}.multiplication_table");
    let table = usize_matrix(
        required_field(group, &group_path, "multiplication_table")?,
        &table_path,
    );
    let a_generators_path = format!("{path}.a_generator_indices");
    let b_generators_path = format!("{path}.b_generator_indices");

    match expected_result {
        QuantumTannerExpectedResult::Success => {
            assert_group_table_shape(&table, 16);
            validate_z4xz4_table(&table, &table_path)?;
            let a_generators = expect_usize_array_value(
                required_field(fixture, path, "a_generator_indices")?,
                &a_generators_path,
                &[4, 12],
            )?;
            let b_generators = expect_usize_array_value(
                required_field(fixture, path, "b_generator_indices")?,
                &b_generators_path,
                &[1, 3],
            )?;
            if !generators_are_symmetric(&table, 0, &a_generators) {
                return Err(format!("{a_generators_path}: expected symmetric set"));
            }
            if !generators_are_symmetric(&table, 0, &b_generators) {
                return Err(format!("{b_generators_path}: expected symmetric set"));
            }
            validate_quantum_tanner_local_codes(
                fixture,
                path,
                Some((a_generators.len(), b_generators.len())),
            )?;
            let face_count = documented_face_count(&table, &a_generators, &b_generators);
            if face_count != 16 {
                return Err(format!(
                    "{path}: expected 16 physical faces, got {face_count}"
                ));
            }
            Ok(())
        }
        QuantumTannerExpectedResult::Rejection("NonSymmetricGeneratorSet") => {
            assert_group_table_shape(&table, 16);
            validate_z4xz4_table(&table, &table_path)?;
            let a_generators = expect_usize_array_value(
                required_field(fixture, path, "a_generator_indices")?,
                &a_generators_path,
                &[4],
            )?;
            let b_generators = expect_usize_array_value(
                required_field(fixture, path, "b_generator_indices")?,
                &b_generators_path,
                &[1, 3],
            )?;
            if generators_are_symmetric(&table, 0, &a_generators) {
                return Err(format!("{a_generators_path}: expected non-symmetric set"));
            }
            if !generators_are_symmetric(&table, 0, &b_generators) {
                return Err(format!("{b_generators_path}: expected symmetric set"));
            }
            validate_quantum_tanner_local_codes(fixture, path, None)
        }
        QuantumTannerExpectedResult::Rejection("InvalidGroupTable") => {
            expect_len(&format!("{table_path}.rows"), table.len(), 16)?;
            if table.first().map(|row| row.len()) != Some(15) {
                return Err(format!("{table_path}[0]: expected malformed length 15"));
            }
            if !table.iter().any(|row| row.len() != 16) {
                return Err(format!("{table_path}: expected malformed table"));
            }
            for (row_index, row) in table.iter().enumerate() {
                for &entry in row {
                    if entry >= 16 {
                        return Err(format!(
                            "{table_path}[{row_index}]: entry {entry} out of range"
                        ));
                    }
                }
            }
            expect_usize_array_value(
                required_field(fixture, path, "a_generator_indices")?,
                &a_generators_path,
                &[4, 12],
            )?;
            expect_usize_array_value(
                required_field(fixture, path, "b_generator_indices")?,
                &b_generators_path,
                &[1, 3],
            )?;
            validate_quantum_tanner_local_codes(fixture, path, Some((2, 2)))
        }
        QuantumTannerExpectedResult::Rejection(reason) => {
            Err(format!("{path}: unrecognized rejection reason {reason}"))
        }
    }
}

fn validate_quantum_tanner_catalog_entry_metadata(entry: &Value, path: &str) -> Result<(), String> {
    let fixture_id = nonempty_string_field(entry, path, "fixture_id")?;
    let input_path = nonempty_string_field(entry, path, "input_path")?;
    let expected_input_path = format!("qec-code/{QUANTUM_TANNER_FIXTURE_DIR}/{fixture_id}.json");
    if input_path != expected_input_path {
        return Err(format!(
            "{path}.input_path: expected {expected_input_path:?}, got {input_path:?}"
        ));
    }

    validate_quantum_tanner_contract_reference(entry, path)?;
    validate_quantum_tanner_provenance(entry, path)?;
    validate_quantum_tanner_references(entry, path)?;
    validate_quantum_tanner_expected_result_shape(entry, path)?;
    expect_str_field(
        entry,
        path,
        "verifier_command",
        QUANTUM_TANNER_VERIFIER_COMMAND,
    )?;
    validate_nonempty_u64_array_field(entry, path, "consuming_issues")?;

    let fixture_rel_path = input_path
        .strip_prefix("qec-code/")
        .ok_or_else(|| format!("{path}.input_path: expected qec-code/ prefix"))?;
    let fixture = load_quantum_tanner_fixture(fixture_rel_path);
    expect_str_field(&fixture, fixture_rel_path, "fixture_id", fixture_id)
}

fn validate_quantum_tanner_catalog_entry(
    entry: &Value,
    path: &str,
    fixture_id: &str,
    expected_result: QuantumTannerExpectedResult<'_>,
) -> Result<(), String> {
    validate_quantum_tanner_catalog_entry_metadata(entry, path)?;
    let actual_fixture_id = nonempty_string_field(entry, path, "fixture_id")?;
    if actual_fixture_id != fixture_id {
        return Err(format!(
            "{path}.fixture_id: expected {fixture_id:?}, got {actual_fixture_id:?}"
        ));
    }

    let input_path = nonempty_string_field(entry, path, "input_path")?;
    let expected_input_path = format!("qec-code/{QUANTUM_TANNER_FIXTURE_DIR}/{fixture_id}.json");
    if input_path != expected_input_path {
        return Err(format!(
            "{path}.input_path: expected {expected_input_path:?}, got {input_path:?}"
        ));
    }

    validate_quantum_tanner_expected_result(entry, path, expected_result)?;
    expect_str_field(
        entry,
        path,
        "verifier_command",
        QUANTUM_TANNER_VERIFIER_COMMAND,
    )?;
    expect_u64_array_field(
        entry,
        path,
        "consuming_issues",
        &[178, 180, 181, 183, 184, 185, 186, 188],
    )?;

    let fixture_rel_path = input_path
        .strip_prefix("qec-code/")
        .ok_or_else(|| format!("{path}.input_path: expected qec-code/ prefix"))?;
    let fixture = load_quantum_tanner_fixture(fixture_rel_path);
    validate_quantum_tanner_fixture(&fixture, fixture_rel_path, fixture_id, expected_result)
}

fn validate_quantum_tanner_catalog(manifest: &Value) -> Result<(), String> {
    expect_u64_field(manifest, "manifest", "schema_version", 1)?;
    expect_str_field(
        manifest,
        "manifest",
        "manifest_id",
        "quantum_tanner_acceptance_v1",
    )?;
    let contract = required_field(manifest, "manifest", "contract")?;
    expect_u64_field(contract, "manifest.contract", "issue", 177)?;
    expect_str_field(
        contract,
        "manifest.contract",
        "path",
        "qec-code/doc/quantum_tanner.md",
    )?;
    expect_str_field(
        contract,
        "manifest.contract",
        "construction_mode",
        "lr_cayley_no_cover_v1",
    )?;
    expect_str_field(
        manifest,
        "manifest",
        "verifier_command",
        QUANTUM_TANNER_VERIFIER_COMMAND,
    )?;

    let entries = required_array_field(manifest, "manifest", "entries")?;
    if entries.is_empty() {
        return Err("manifest.entries: expected at least one entry".to_owned());
    }
    let mut seen_fixture_ids = HashSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let entry_path = format!("manifest.entries[{index}]");
        let fixture_id = nonempty_string_field(entry, &entry_path, "fixture_id")?;
        if !seen_fixture_ids.insert(fixture_id.to_owned()) {
            return Err(format!("{entry_path}.fixture_id: duplicate {fixture_id:?}"));
        }
        match fixture_id {
            "toric_d4" => validate_quantum_tanner_catalog_entry(
                entry,
                &entry_path,
                "toric_d4",
                QuantumTannerExpectedResult::Success,
            )?,
            "invalid_non_symmetric_a" => validate_quantum_tanner_catalog_entry(
                entry,
                &entry_path,
                "invalid_non_symmetric_a",
                QuantumTannerExpectedResult::Rejection("NonSymmetricGeneratorSet"),
            )?,
            "invalid_bad_table" => validate_quantum_tanner_catalog_entry(
                entry,
                &entry_path,
                "invalid_bad_table",
                QuantumTannerExpectedResult::Rejection("InvalidGroupTable"),
            )?,
            _ => validate_quantum_tanner_catalog_entry_metadata(entry, &entry_path)?,
        }
    }
    for required_fixture_id in ["toric_d4", "invalid_non_symmetric_a", "invalid_bad_table"] {
        if !seen_fixture_ids.contains(required_fixture_id) {
            return Err(format!(
                "manifest.entries: missing required fixture {required_fixture_id:?}"
            ));
        }
    }
    Ok(())
}

fn documented_face_count(
    table: &[Vec<usize>],
    a_generators: &[usize],
    b_generators: &[usize],
) -> usize {
    let mut faces = std::collections::BTreeSet::new();
    for g in 0..table.len() {
        for &a in a_generators {
            for &b in b_generators {
                let ag = table[a][g];
                let gb = table[g][b];
                let agb = table[ag][b];
                let mut face = vec![g, ag, gb, agb];
                face.sort_unstable();
                face.dedup();
                assert_eq!(face.len(), 4, "face must be nondegenerate");
                faces.insert(face);
            }
        }
    }
    faces.len()
}

fn quantum_tanner_group_table_validator_spec(
    order: usize,
    identity: usize,
    multiplication_table: Vec<Vec<usize>>,
    a_generator_indices: Vec<usize>,
    b_generator_indices: Vec<usize>,
) -> QuantumTannerSpec {
    let a_width = a_generator_indices.len();
    let b_width = b_generator_indices.len();
    QuantumTannerSpec {
        construction_mode: QuantumTannerConstructionMode::LeftRightCayleyNoCoverV1,
        base_group: ExplicitFiniteGroup {
            name: None,
            element_order: None,
            order,
            identity,
            multiplication_table,
        },
        a_generator_indices,
        b_generator_indices,
        local_codes: QuantumTannerLocalCodes {
            matrix_role: "parity_check".to_owned(),
            field: "GF(2)".to_owned(),
            h_a: vec![vec![1; a_width]],
            h_b: vec![vec![1; b_width]],
            g_a: None,
            g_b: None,
        },
    }
}

fn z2xz2_group_table() -> Vec<Vec<usize>> {
    vec![
        vec![0, 1, 2, 3],
        vec![1, 0, 3, 2],
        vec![2, 3, 0, 1],
        vec![3, 2, 1, 0],
    ]
}

#[test]
fn quantum_tanner_group_table_validator_accepts_z2xz2_and_safe_accessors() {
    let spec =
        quantum_tanner_group_table_validator_spec(4, 0, z2xz2_group_table(), vec![1, 2], vec![3]);

    let group = validate_quantum_tanner_group_table(&spec).unwrap();

    assert_eq!(group.order(), 4);
    assert_eq!(group.identity(), 0);
    assert_eq!(group.multiply(1, 2).unwrap(), 3);
    assert_eq!(group.multiply(2, 1).unwrap(), 3);
    assert_eq!(group.multiply(3, 3).unwrap(), 0);
    assert_eq!(group.inv(0).unwrap(), 0);
    assert_eq!(group.inv(1).unwrap(), 1);
    assert_eq!(group.inv(2).unwrap(), 2);
    assert_eq!(group.inv(3).unwrap(), 3);
    assert_eq!(group.a_generators(), &[1, 2]);
    assert_eq!(group.b_generators(), &[3]);
    assert_eq!(group.a_generator(0), Some(1));
    assert_eq!(group.a_generator(1), Some(2));
    assert_eq!(group.a_generator(2), None);
    assert_eq!(group.b_generator(0), Some(3));
    assert_eq!(group.b_generator(1), None);
}

#[test]
fn quantum_tanner_group_table_validator_accepts_toric_d4_catalog_fixture() {
    let spec =
        quantum_tanner_spec_from_json_str(include_str!("fixtures/quantum_tanner/toric_d4.json"))
            .unwrap();

    let group = validate_quantum_tanner_group_table(&spec).unwrap();

    assert_eq!(group.order(), 16);
    assert_eq!(group.identity(), 0);
    assert_eq!(group.multiply(4, 12).unwrap(), 0);
    assert_eq!(group.multiply(12, 4).unwrap(), 0);
    assert_eq!(group.inv(4).unwrap(), 12);
    assert_eq!(group.inv(12).unwrap(), 4);
    assert_eq!(group.multiply(1, 3).unwrap(), 0);
    assert_eq!(group.inv(1).unwrap(), 3);
    assert_eq!(group.inv(3).unwrap(), 1);
    assert_eq!(group.a_generator(0), Some(4));
    assert_eq!(group.a_generator(1), Some(12));
    assert_eq!(group.b_generator(0), Some(1));
    assert_eq!(group.b_generator(1), Some(3));
}

#[test]
fn quantum_tanner_group_table_validator_rejects_square_in_range_non_associative_table() {
    let non_associative_table = vec![
        vec![0, 1, 2, 3],
        vec![1, 0, 2, 3],
        vec![2, 3, 0, 1],
        vec![3, 2, 1, 0],
    ];
    let spec =
        quantum_tanner_group_table_validator_spec(4, 0, non_associative_table, vec![1], vec![2]);

    let error = validate_quantum_tanner_group_table(&spec).unwrap_err();
    let QecError::InvalidQuantumTannerGroupTable { reason } = error else {
        panic!("expected group-table validation error, got {error:?}");
    };
    assert!(
        reason.contains("associativity failed for (1, 2, 2)"),
        "expected the square in-range negative control to fail associativity, got {reason:?}"
    );
}

#[test]
fn quantum_tanner_group_table_validator_rejects_identity_and_inverse_contract_errors() {
    let declared_identity_mismatch =
        quantum_tanner_group_table_validator_spec(4, 1, z2xz2_group_table(), vec![1], vec![2]);
    let error = validate_quantum_tanner_group_table(&declared_identity_mismatch).unwrap_err();
    let QecError::InvalidQuantumTannerGroupTable { reason } = error else {
        panic!("expected group-table validation error, got {error:?}");
    };
    assert!(
        reason.contains("declared identity 1 does not match table identity 0"),
        "got {reason:?}"
    );

    let identity_out_of_range =
        quantum_tanner_group_table_validator_spec(4, 4, z2xz2_group_table(), vec![1], vec![2]);
    let error = validate_quantum_tanner_group_table(&identity_out_of_range).unwrap_err();
    let QecError::InvalidQuantumTannerGroupTable { reason } = error else {
        panic!("expected group-table validation error, got {error:?}");
    };
    assert!(
        reason.contains("identity 4 is out of range for order 4"),
        "got {reason:?}"
    );

    let no_identity_table = vec![vec![1, 1], vec![1, 1]];
    let no_identity =
        quantum_tanner_group_table_validator_spec(2, 0, no_identity_table, vec![1], vec![1]);
    let error = validate_quantum_tanner_group_table(&no_identity).unwrap_err();
    let QecError::InvalidQuantumTannerGroupTable { reason } = error else {
        panic!("expected group-table validation error, got {error:?}");
    };
    assert!(
        reason.contains("expected exactly one two-sided identity, found none"),
        "got {reason:?}"
    );

    let no_inverse_table = vec![vec![0, 1], vec![1, 1]];
    let no_inverse =
        quantum_tanner_group_table_validator_spec(2, 0, no_inverse_table, vec![1], vec![1]);
    let error = validate_quantum_tanner_group_table(&no_inverse).unwrap_err();
    let QecError::InvalidQuantumTannerGroupTable { reason } = error else {
        panic!("expected group-table validation error, got {error:?}");
    };
    assert!(
        reason.contains("element 1 has no two-sided inverse under identity 0"),
        "got {reason:?}"
    );

    let multiple_inverse_table = vec![vec![0, 1, 2], vec![1, 0, 0], vec![2, 0, 0]];
    let multiple_inverses =
        quantum_tanner_group_table_validator_spec(3, 0, multiple_inverse_table, vec![1], vec![2]);
    let error = validate_quantum_tanner_group_table(&multiple_inverses).unwrap_err();
    let QecError::InvalidQuantumTannerGroupTable { reason } = error else {
        panic!("expected group-table validation error, got {error:?}");
    };
    assert!(
        reason.contains("element 1 has multiple two-sided inverses under identity 0: [1, 2]"),
        "got {reason:?}"
    );
}

#[test]
fn quantum_tanner_group_table_validator_rejects_out_of_range_generators_and_elements() {
    let bad_generator_spec =
        quantum_tanner_group_table_validator_spec(4, 0, z2xz2_group_table(), vec![4], vec![1]);

    let error = validate_quantum_tanner_group_table(&bad_generator_spec).unwrap_err();
    assert!(matches!(
        error,
        QecError::InvalidQuantumTannerGeneratorIndex {
            set: "A",
            index: 0,
            element: 4,
            order: 4
        }
    ));

    let valid_spec =
        quantum_tanner_group_table_validator_spec(4, 0, z2xz2_group_table(), vec![1], vec![2]);
    let group = validate_quantum_tanner_group_table(&valid_spec).unwrap();

    assert!(matches!(
        group.multiply(4, 0).unwrap_err(),
        QecError::InvalidQuantumTannerGroupElement {
            element: 4,
            order: 4
        }
    ));
    assert!(matches!(
        group.multiply(0, 4).unwrap_err(),
        QecError::InvalidQuantumTannerGroupElement {
            element: 4,
            order: 4
        }
    ));
    assert!(matches!(
        group.inv(4).unwrap_err(),
        QecError::InvalidQuantumTannerGroupElement {
            element: 4,
            order: 4
        }
    ));
}

#[test]
fn quantum_tanner_cayley_faces_match_toric_d4_counts() {
    let spec =
        quantum_tanner_spec_from_json_str(include_str!("fixtures/quantum_tanner/toric_d4.json"))
            .unwrap();
    let group = validate_quantum_tanner_group_table(&spec).unwrap();

    let complex = enumerate_quantum_tanner_cayley_faces(spec.construction_mode, &group).unwrap();

    assert_eq!(complex.faces.len(), 16);
    assert_eq!(complex.oriented_faces.len(), 64);
    assert_eq!(complex.x_incidence.len(), 64);
    assert_eq!(complex.z_incidence.len(), 64);
    assert_eq!(
        complex
            .faces
            .iter()
            .map(|face| (face.id, face.vertices))
            .take(4)
            .collect::<Vec<_>>(),
        vec![
            (0, [0, 1, 4, 5]),
            (1, [0, 1, 12, 13]),
            (2, [0, 3, 4, 7]),
            (3, [0, 3, 12, 15]),
        ]
    );

    for source_vertex in 0..group.order() {
        let x_local = complex
            .x_incidence
            .iter()
            .filter(|record| record.source_vertex == source_vertex)
            .map(|record| {
                (
                    record.a_index,
                    record.a_generator,
                    record.b_index,
                    record.b_generator,
                )
            })
            .collect::<Vec<_>>();
        let z_local = complex
            .z_incidence
            .iter()
            .filter(|record| record.source_vertex == source_vertex)
            .map(|record| {
                (
                    record.a_index,
                    record.a_generator,
                    record.b_index,
                    record.b_generator,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            x_local,
            vec![(0, 4, 0, 1), (0, 4, 1, 3), (1, 12, 0, 1), (1, 12, 1, 3)]
        );
        assert_eq!(z_local, x_local);
    }

    let x_identity = complex
        .x_incidence
        .iter()
        .filter(|record| record.source_vertex == 0)
        .map(|record| (record.a_generator, record.b_generator, record.face_id))
        .collect::<Vec<_>>();
    assert_eq!(
        x_identity,
        vec![(4, 1, 0), (4, 3, 2), (12, 1, 1), (12, 3, 3)]
    );

    let z_source_four = complex
        .z_incidence
        .iter()
        .filter(|record| record.source_vertex == 4)
        .map(|record| (record.a_generator, record.b_generator, record.face_id))
        .collect::<Vec<_>>();
    assert_eq!(
        z_source_four,
        vec![(4, 1, 8), (4, 3, 9), (12, 1, 0), (12, 3, 2)]
    );

    let x_face = complex
        .x_incidence
        .iter()
        .find(|record| {
            record.source_vertex == 0 && record.a_generator == 4 && record.b_generator == 1
        })
        .unwrap()
        .face_id;
    let z_face = complex
        .z_incidence
        .iter()
        .find(|record| {
            record.source_vertex == 4 && record.a_generator == 12 && record.b_generator == 1
        })
        .unwrap()
        .face_id;
    assert_eq!(x_face, z_face);

    let non_symmetric_input = toric_d4_json_with(|fixture| {
        fixture["a_generator_indices"] = Value::Array(vec![Value::from(4_u64)]);
        fixture["local_codes"]["h_a"] = Value::Array(vec![Value::Array(vec![Value::from(1_u64)])]);
    });
    let non_symmetric_spec = quantum_tanner_spec_from_json_str(&non_symmetric_input).unwrap();
    let non_symmetric_group = validate_quantum_tanner_group_table(&non_symmetric_spec).unwrap();
    assert!(matches!(
        enumerate_quantum_tanner_cayley_faces(
            non_symmetric_spec.construction_mode,
            &non_symmetric_group
        )
        .unwrap_err(),
        QecError::InvalidQuantumTannerGeneratorSet { set: "A", .. }
    ));

    let unsupported_mode = toric_d4_json_with(|fixture| {
        fixture["construction_mode"] = Value::String("lr_cayley_quadripartite_cover_v1".to_owned());
    });
    assert!(matches!(
        quantum_tanner_spec_from_json_str(&unsupported_mode).unwrap_err(),
        QecError::UnsupportedQuantumTannerConstructionMode { mode }
            if mode == "lr_cayley_quadripartite_cover_v1"
    ));
}

#[test]
fn quantum_tanner_cayley_faces_reject_invalid_generator_sets_and_degenerate_faces() {
    let empty_a_spec =
        quantum_tanner_group_table_validator_spec(4, 0, z2xz2_group_table(), vec![], vec![1]);
    let empty_a_group = validate_quantum_tanner_group_table(&empty_a_spec).unwrap();
    let error =
        enumerate_quantum_tanner_cayley_faces(empty_a_spec.construction_mode, &empty_a_group)
            .unwrap_err();
    let QecError::InvalidQuantumTannerGeneratorSet { set, reason } = error else {
        panic!("expected generator-set error, got {error:?}");
    };
    assert_eq!(set, "A");
    assert!(reason.contains("nonempty"), "got {reason:?}");

    let duplicate_a_spec =
        quantum_tanner_group_table_validator_spec(4, 0, z2xz2_group_table(), vec![1, 1], vec![2]);
    let duplicate_a_group = validate_quantum_tanner_group_table(&duplicate_a_spec).unwrap();
    let error = enumerate_quantum_tanner_cayley_faces(
        duplicate_a_spec.construction_mode,
        &duplicate_a_group,
    )
    .unwrap_err();
    let QecError::InvalidQuantumTannerGeneratorSet { set, reason } = error else {
        panic!("expected generator-set error, got {error:?}");
    };
    assert_eq!(set, "A");
    assert!(
        reason.contains("duplicate generator 1 at coordinate 1"),
        "got {reason:?}"
    );

    let degenerate_spec = quantum_tanner_group_table_validator_spec(
        2,
        0,
        vec![vec![0, 1], vec![1, 0]],
        vec![1],
        vec![1],
    );
    let degenerate_group = validate_quantum_tanner_group_table(&degenerate_spec).unwrap();
    assert!(matches!(
        enumerate_quantum_tanner_cayley_faces(
            degenerate_spec.construction_mode,
            &degenerate_group,
        )
        .unwrap_err(),
        QecError::DegenerateQuantumTannerFace {
            root: 0,
            a: 1,
            b: 1,
            vertices,
        } if vertices == vec![0, 0, 1, 1]
    ));
}

#[test]
fn quantum_tanner_fixture_catalog_has_grounded_cases() {
    let manifest = load_quantum_tanner_fixture("tests/fixtures/quantum_tanner/manifest.json");
    validate_quantum_tanner_catalog(&manifest).unwrap();
}

#[test]
fn quantum_tanner_spec_json_accepts_toric_d4_and_rejects_bad_table() {
    let spec =
        quantum_tanner_spec_from_json_str(include_str!("fixtures/quantum_tanner/toric_d4.json"))
            .unwrap();

    assert_eq!(
        spec.construction_mode,
        QuantumTannerConstructionMode::LeftRightCayleyNoCoverV1
    );
    assert_eq!(spec.construction_mode.as_str(), "lr_cayley_no_cover_v1");
    assert_eq!(spec.base_group.order, 16);
    assert_eq!(spec.base_group.identity, 0);
    assert_eq!(spec.base_group.multiplication_table.len(), 16);
    assert!(spec
        .base_group
        .multiplication_table
        .iter()
        .all(|row| row.len() == 16));
    assert_eq!(spec.a_generator_indices, vec![4, 12]);
    assert_eq!(spec.b_generator_indices, vec![1, 3]);
    assert_eq!(spec.local_codes.matrix_role.as_str(), "parity_check");
    assert_eq!(spec.local_codes.field.as_str(), "GF(2)");
    assert_eq!(spec.local_codes.h_a, vec![vec![1, 1]]);
    assert_eq!(spec.local_codes.h_b, vec![vec![1, 1]]);

    let error = quantum_tanner_spec_from_json_str(include_str!(
        "fixtures/quantum_tanner/invalid_bad_table.json"
    ))
    .unwrap_err();

    assert!(
        matches!(error, QecError::InvalidQuantumTannerGroupTable { .. }),
        "expected malformed table to fail before construction, got {error:?}"
    );
    assert!(
        error.to_string().contains("row 0"),
        "malformed table error should identify the bad row: {error}"
    );

    let nonzero_identity_json = include_str!("fixtures/quantum_tanner/toric_d4.json")
        .replace("\"identity\": 0", "\"identity\": 1");
    let error = quantum_tanner_spec_from_json_str(&nonzero_identity_json).unwrap_err();
    assert!(
        matches!(error, QecError::InvalidQuantumTannerGroupTable { .. }),
        "expected nonzero identity to fail before construction, got {error:?}"
    );
    assert!(
        error.to_string().contains("identity"),
        "nonzero identity error should identify the bad field: {error}"
    );
}

#[test]
fn quantum_tanner_local_code_tensor_dual_repetition_example_rejects_bad_inputs() {
    let spec =
        quantum_tanner_spec_from_json_str(include_str!("fixtures/quantum_tanner/toric_d4.json"))
            .unwrap();
    let local = quantum_tanner_local_code_tensor_dual(&spec).unwrap();

    assert_eq!(local.code_a.width, 2);
    assert_eq!(local.code_a.generator_rows, vec![vec![1, 1]]);
    assert_eq!(local.code_a.dual_rows, vec![vec![1, 1]]);
    assert_eq!(local.code_b.width, 2);
    assert_eq!(local.code_b.generator_rows, vec![vec![1, 1]]);
    assert_eq!(local.code_b.dual_rows, vec![vec![1, 1]]);
    assert_eq!(local.x_sector_rows, vec![vec![1, 1, 1, 1]]);
    assert_eq!(local.z_sector_rows, vec![vec![1, 1, 1, 1]]);

    let nonbinary_h_a = toric_d4_json_with(|fixture| {
        fixture["local_codes"]["h_a"][0][0] = Value::from(2);
    });
    expect_quantum_tanner_local_code_matrix_error(&nonbinary_h_a, "h_a", "expected 0 or 1");

    let nonorthogonal_g_a = toric_d4_json_with(|fixture| {
        fixture["local_codes"]["g_a"] = serde_json::json!([[1, 0]]);
    });
    expect_quantum_tanner_local_code_matrix_error(&nonorthogonal_g_a, "code_a", "not orthogonal");

    let valid_supplied_generators = toric_d4_json_with(|fixture| {
        fixture["local_codes"]["g_a"] = serde_json::json!([[1, 1]]);
        fixture["local_codes"]["g_b"] = serde_json::json!([[1, 1]]);
    });
    let local = quantum_tanner_local_code_tensor_dual(
        &quantum_tanner_spec_from_json_str(&valid_supplied_generators).unwrap(),
    )
    .unwrap();
    assert_eq!(local.code_a.generator_rows, vec![vec![1, 1]]);
    assert_eq!(local.code_b.generator_rows, vec![vec![1, 1]]);
    assert_eq!(local.x_sector_rows, vec![vec![1, 1, 1, 1]]);

    let rank_mismatch_g_a = toric_d4_json_with(|fixture| {
        fixture["local_codes"]["g_a"] = serde_json::json!([[0, 0]]);
    });
    expect_quantum_tanner_local_code_matrix_error(&rank_mismatch_g_a, "code_a", "rank is 0");

    let nonorthogonal_g_b = toric_d4_json_with(|fixture| {
        fixture["local_codes"]["g_b"] = serde_json::json!([[1, 0]]);
    });
    expect_quantum_tanner_local_code_matrix_error(&nonorthogonal_g_b, "code_b", "not orthogonal");

    let mut corrupted_code_a = spec.clone();
    corrupted_code_a.local_codes.h_a[0][0] = 2;
    let error = quantum_tanner_local_code_tensor_dual(&corrupted_code_a).unwrap_err();
    let QecError::InvalidQuantumTannerLocalCodeMatrix { matrix, reason } = error else {
        panic!("expected InvalidQuantumTannerLocalCodeMatrix, got {error:?}");
    };
    assert_eq!(matrix, "code_a");
    assert!(reason.contains("expected 0 or 1"), "got {reason:?}");

    let mut corrupted_code_b = spec;
    corrupted_code_b.local_codes.h_b[0].push(1);
    let error = quantum_tanner_local_code_tensor_dual(&corrupted_code_b).unwrap_err();
    let QecError::InvalidQuantumTannerLocalCodeMatrix { matrix, reason } = error else {
        panic!("expected InvalidQuantumTannerLocalCodeMatrix, got {error:?}");
    };
    assert_eq!(matrix, "code_b");
    assert!(reason.contains("width 3"), "got {reason:?}");
}

fn toric_d4_json_with(mutator: impl FnOnce(&mut Value)) -> String {
    let mut fixture: Value =
        serde_json::from_str(include_str!("fixtures/quantum_tanner/toric_d4.json")).unwrap();
    mutator(&mut fixture);
    serde_json::to_string(&fixture).unwrap()
}

fn assert_sparse_css_orthogonal(num_cols: usize, hx: &[Vec<usize>], hz: &[Vec<usize>]) {
    for (x_index, x_row) in hx.iter().enumerate() {
        let x_support = x_row
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        for (z_index, z_row) in hz.iter().enumerate() {
            let overlap = z_row
                .iter()
                .filter(|support| x_support.contains(support))
                .count();
            assert_eq!(
                overlap % 2,
                0,
                "Hx row {x_index} and Hz row {z_index} have odd overlap in width {num_cols}"
            );
        }
    }
}

fn quantum_tanner_toric_d4_validated_parts() -> (
    QuantumTannerSpec,
    ValidatedFiniteGroup,
    QuantumTannerCayleyComplex,
    QuantumTannerLocalCodeTensorDual,
) {
    let spec =
        quantum_tanner_spec_from_json_str(include_str!("fixtures/quantum_tanner/toric_d4.json"))
            .unwrap();
    let group = validate_quantum_tanner_group_table(&spec).unwrap();
    let complex = enumerate_quantum_tanner_cayley_faces(spec.construction_mode, &group).unwrap();
    let local = quantum_tanner_local_code_tensor_dual(&spec).unwrap();
    (spec, group, complex, local)
}

fn expect_quantum_tanner_css_error(
    result: Result<qec_code::codes::quantum_tanner::QuantumTannerCssChecks, QecError>,
    expected_reason: &str,
) {
    let error = result.unwrap_err();
    let QecError::InvalidQuantumTannerCssConstruction { reason } = error else {
        panic!("expected quantum Tanner CSS construction error, got {error:?}");
    };
    assert!(
        reason.contains(expected_reason),
        "expected error containing {expected_reason:?}, got {reason:?}"
    );
}

#[test]
fn quantum_tanner_toric_d4_generates_css_checks() {
    let spec =
        quantum_tanner_spec_from_json_str(include_str!("fixtures/quantum_tanner/toric_d4.json"))
            .unwrap();

    let checks = quantum_tanner_css_checks(&spec).unwrap();

    assert_eq!(checks.num_cols, 16);
    assert!(!checks.hx.is_empty());
    assert!(!checks.hz.is_empty());
    for row in checks.hx.iter().chain(checks.hz.iter()) {
        if !row.is_empty() {
            assert_eq!(
                row.len(),
                4,
                "expected weight-4 stabilizer row, got {row:?}"
            );
        }
    }
    assert_sparse_css_orthogonal(checks.num_cols, &checks.hx, &checks.hz);

    let hx = SparseRowsMatrix::new(checks.num_cols, checks.hx.clone())
        .unwrap()
        .to_dense_rows();
    let hz = SparseRowsMatrix::new(checks.num_cols, checks.hz.clone())
        .unwrap()
        .to_dense_rows();
    let css = CssCode::from_hx_hz(hx, hz).unwrap();
    assert_eq!(css.code().num_logical_qubits(), 2);

    let distance = compute_distance(css.code()).unwrap();
    assert_eq!(distance.distance, 4);
    assert_eq!(distance.witness.weight(), 4);

    let invalid_non_symmetric_a = quantum_tanner_spec_from_json_str(include_str!(
        "fixtures/quantum_tanner/invalid_non_symmetric_a.json"
    ))
    .unwrap();
    assert!(matches!(
        quantum_tanner_css_checks(&invalid_non_symmetric_a).unwrap_err(),
        QecError::InvalidQuantumTannerGeneratorSet { set: "A", .. }
    ));
}

#[test]
fn quantum_tanner_css_constructor_rejects_inconsistent_validated_parts() {
    let (spec, group, complex, local) = quantum_tanner_toric_d4_validated_parts();

    let mut mismatched_spec = spec.clone();
    mismatched_spec.a_generator_indices.swap(0, 1);
    expect_quantum_tanner_css_error(
        quantum_tanner_css_checks_from_validated_parts(
            &mismatched_spec,
            &group,
            &complex,
            &local,
        ),
        "spec A generator indices",
    );

    let mut mismatched_spec = spec.clone();
    mismatched_spec.b_generator_indices.swap(0, 1);
    expect_quantum_tanner_css_error(
        quantum_tanner_css_checks_from_validated_parts(
            &mismatched_spec,
            &group,
            &complex,
            &local,
        ),
        "spec B generator indices",
    );

    let mut bad_local = local.clone();
    bad_local.code_a.width += 1;
    expect_quantum_tanner_css_error(
        quantum_tanner_css_checks_from_validated_parts(&spec, &group, &complex, &bad_local),
        "local code A width",
    );

    let mut bad_local = local.clone();
    bad_local.code_b.width += 1;
    expect_quantum_tanner_css_error(
        quantum_tanner_css_checks_from_validated_parts(&spec, &group, &complex, &bad_local),
        "local code B width",
    );

    let mut bad_local = local.clone();
    bad_local.x_sector_rows[0].pop();
    expect_quantum_tanner_css_error(
        quantum_tanner_css_checks_from_validated_parts(&spec, &group, &complex, &bad_local),
        "X local tensor row 0 has width",
    );

    let mut bad_local = local.clone();
    bad_local.z_sector_rows[0][0] = 2;
    expect_quantum_tanner_css_error(
        quantum_tanner_css_checks_from_validated_parts(&spec, &group, &complex, &bad_local),
        "Z local tensor row 0, column 0 is 2",
    );

    let mut sparse_local = local.clone();
    sparse_local.x_sector_rows[0] = vec![1, 0, 0, 0];
    assert!(
        quantum_tanner_css_checks_from_validated_parts(&spec, &group, &complex, &sparse_local)
            .is_err()
    );
}

#[test]
fn quantum_tanner_css_constructor_rejects_bad_incidence_records() {
    let (spec, group, complex, local) = quantum_tanner_toric_d4_validated_parts();

    let mut bad_complex = complex.clone();
    bad_complex.x_incidence[0].face_id = bad_complex.faces.len();
    expect_quantum_tanner_css_error(
        quantum_tanner_css_checks_from_validated_parts(&spec, &group, &bad_complex, &local),
        "outside",
    );

    let mut bad_complex = complex.clone();
    bad_complex.x_incidence[0].a_index = group.a_generators().len();
    expect_quantum_tanner_css_error(
        quantum_tanner_css_checks_from_validated_parts(&spec, &group, &bad_complex, &local),
        "out-of-range A coordinate",
    );

    let mut bad_complex = complex.clone();
    bad_complex.x_incidence[0].a_generator = group.identity();
    expect_quantum_tanner_css_error(
        quantum_tanner_css_checks_from_validated_parts(&spec, &group, &bad_complex, &local),
        "A coordinate",
    );

    let mut bad_complex = complex.clone();
    bad_complex.x_incidence[0].b_index = group.b_generators().len();
    expect_quantum_tanner_css_error(
        quantum_tanner_css_checks_from_validated_parts(&spec, &group, &bad_complex, &local),
        "out-of-range B coordinate",
    );

    let mut bad_complex = complex.clone();
    bad_complex.x_incidence[0].b_generator = group.identity();
    expect_quantum_tanner_css_error(
        quantum_tanner_css_checks_from_validated_parts(&spec, &group, &bad_complex, &local),
        "B coordinate",
    );

    let mut bad_complex = complex.clone();
    bad_complex.x_incidence.push(bad_complex.x_incidence[0]);
    expect_quantum_tanner_css_error(
        quantum_tanner_css_checks_from_validated_parts(&spec, &group, &bad_complex, &local),
        "duplicate local coordinate",
    );

    let missing = complex.x_incidence[0];
    let mut bad_complex = complex.clone();
    bad_complex.x_incidence.retain(|record| {
        !(record.source_vertex == missing.source_vertex
            && record.a_index == missing.a_index
            && record.b_index == missing.b_index)
    });
    expect_quantum_tanner_css_error(
        quantum_tanner_css_checks_from_validated_parts(&spec, &group, &bad_complex, &local),
        "missing local coordinate",
    );

    let mut folded_complex = complex.clone();
    let first = folded_complex.x_incidence[0];
    let same_source_second = folded_complex
        .x_incidence
        .iter()
        .position(|record| {
            record.source_vertex == first.source_vertex
                && (record.a_index != first.a_index || record.b_index != first.b_index)
        })
        .unwrap();
    folded_complex.x_incidence[same_source_second].face_id = first.face_id;
    assert!(
        matches!(
            quantum_tanner_css_checks_from_validated_parts(&spec, &group, &folded_complex, &local)
                .unwrap_err(),
            QecError::InvalidCssOrthogonality
        ),
        "duplicate face incidence should cancel one local support modulo 2 and fail CSS validation"
    );
}

#[test]
fn quantum_tanner_css_constructor_rejects_non_bipartite_cayley_sources() {
    let spec = quantum_tanner_group_table_validator_spec(
        3,
        0,
        vec![vec![0, 1, 2], vec![1, 2, 0], vec![2, 0, 1]],
        vec![1, 2],
        vec![1, 2],
    );
    let group = validate_quantum_tanner_group_table(&spec).unwrap();
    let local = quantum_tanner_local_code_tensor_dual(&spec).unwrap();
    let empty_complex = QuantumTannerCayleyComplex {
        faces: vec![],
        oriented_faces: vec![],
        x_incidence: vec![],
        z_incidence: vec![],
    };

    expect_quantum_tanner_css_error(
        quantum_tanner_css_checks_from_validated_parts(&spec, &group, &empty_complex, &local),
        "not bipartite",
    );
}

fn expect_quantum_tanner_group_table_error(input: &str, expected_reason_part: &str) {
    let error = quantum_tanner_spec_from_json_str(input).unwrap_err();
    let QecError::InvalidQuantumTannerGroupTable { reason } = error else {
        panic!("expected InvalidQuantumTannerGroupTable, got {error:?}");
    };
    assert!(
        reason.contains(expected_reason_part),
        "expected reason to contain {expected_reason_part:?}, got {reason:?}"
    );
}

fn expect_quantum_tanner_local_code_error(
    input: &str,
    expected_matrix: &'static str,
    expected_reason_part: &str,
) {
    let error = quantum_tanner_spec_from_json_str(input).unwrap_err();
    let QecError::InvalidQuantumTannerLocalCodeMatrix { matrix, reason } = error else {
        panic!("expected InvalidQuantumTannerLocalCodeMatrix, got {error:?}");
    };
    assert_eq!(matrix, expected_matrix);
    assert!(
        reason.contains(expected_reason_part),
        "expected reason to contain {expected_reason_part:?}, got {reason:?}"
    );
}

fn expect_quantum_tanner_local_code_matrix_error(
    input: &str,
    expected_matrix: &'static str,
    expected_reason_part: &str,
) {
    let error = quantum_tanner_spec_from_json_str(input)
        .and_then(|spec| quantum_tanner_local_code_tensor_dual(&spec))
        .unwrap_err();
    let QecError::InvalidQuantumTannerLocalCodeMatrix { matrix, reason } = error else {
        panic!("expected InvalidQuantumTannerLocalCodeMatrix, got {error:?}");
    };
    assert_eq!(matrix, expected_matrix);
    assert!(
        reason.contains(expected_reason_part),
        "expected reason to contain {expected_reason_part:?}, got {reason:?}"
    );
}

#[test]
fn quantum_tanner_spec_json_rejects_invalid_json() {
    assert!(matches!(
        quantum_tanner_spec_from_json_str("{").unwrap_err(),
        QecError::InvalidQuantumTannerSpecJson(_)
    ));
}

#[test]
fn quantum_tanner_spec_json_rejects_unsupported_construction_mode() {
    let input = toric_d4_json_with(|fixture| {
        fixture["construction_mode"] = Value::String("lr_cayley_quadripartite_cover_v1".to_owned());
    });

    assert!(matches!(
        quantum_tanner_spec_from_json_str(&input).unwrap_err(),
        QecError::UnsupportedQuantumTannerConstructionMode { mode }
            if mode == "lr_cayley_quadripartite_cover_v1"
    ));
}

#[test]
fn quantum_tanner_spec_json_rejects_group_table_contract_errors() {
    let zero_order = toric_d4_json_with(|fixture| {
        fixture["base_group"]["order"] = Value::from(0);
    });
    expect_quantum_tanner_group_table_error(&zero_order, "order must be positive");

    let short_table = toric_d4_json_with(|fixture| {
        fixture["base_group"]["multiplication_table"]
            .as_array_mut()
            .unwrap()
            .pop();
    });
    expect_quantum_tanner_group_table_error(&short_table, "expected 16 rows, got 15");

    let out_of_range_entry = toric_d4_json_with(|fixture| {
        fixture["base_group"]["multiplication_table"][0][0] = Value::from(16);
    });
    expect_quantum_tanner_group_table_error(&out_of_range_entry, "expected < 16");
}

#[test]
fn quantum_tanner_spec_json_rejects_invalid_local_code_shapes() {
    let bad_role = toric_d4_json_with(|fixture| {
        fixture["local_codes"]["matrix_role"] = Value::String("generator".to_owned());
    });
    expect_quantum_tanner_local_code_error(&bad_role, "local_codes", "matrix_role");

    let bad_field = toric_d4_json_with(|fixture| {
        fixture["local_codes"]["field"] = Value::String("GF(4)".to_owned());
    });
    expect_quantum_tanner_local_code_error(&bad_field, "local_codes", "field");

    let wrong_h_a_width = toric_d4_json_with(|fixture| {
        fixture["local_codes"]["h_a"][0]
            .as_array_mut()
            .unwrap()
            .push(Value::from(1));
    });
    expect_quantum_tanner_local_code_error(&wrong_h_a_width, "h_a", "width 3");

    let nonbinary_h_b = toric_d4_json_with(|fixture| {
        fixture["local_codes"]["h_b"][0][1] = Value::from(2);
    });
    expect_quantum_tanner_local_code_error(&nonbinary_h_b, "h_b", "expected 0 or 1");
}

const QUANTUM_TANNER_SOURCES_DOC: &str = include_str!("../doc/quantum_tanner_sources.md");

#[derive(Debug)]
struct QuantumTannerSourceRow<'a> {
    source: &'a str,
    location: &'a str,
    license: &'a str,
    intended_use: &'a str,
    copying_posture: &'a str,
    definition_of_done: &'a str,
}

const QUANTUM_TANNER_SOURCE_TABLE_HEADER: &str =
    "| Source | URL or local path | License status | Intended use | Copying/import posture | Definition of done for future work |";

fn markdown_cells(row: &str) -> Vec<&str> {
    row.trim()
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect()
}

fn quantum_tanner_source_rows(doc: &str) -> Vec<QuantumTannerSourceRow<'_>> {
    let mut lines = doc.lines();
    while let Some(line) = lines.next() {
        if line.trim() == QUANTUM_TANNER_SOURCE_TABLE_HEADER {
            let separator = lines
                .next()
                .expect("source table should include a separator row");
            assert!(
                separator
                    .trim()
                    .starts_with("| --- | --- | --- | --- | --- | --- |"),
                "source table separator has unexpected shape: {separator}"
            );

            return lines
                .take_while(|line| line.trim_start().starts_with('|'))
                .map(|line| {
                    let cells = markdown_cells(line);
                    assert_eq!(cells.len(), 6, "source table row has unexpected shape: {line}");
                    QuantumTannerSourceRow {
                        source: cells[0],
                        location: cells[1],
                        license: cells[2],
                        intended_use: cells[3],
                        copying_posture: cells[4],
                        definition_of_done: cells[5],
                    }
                })
                .collect();
        }
    }
    panic!("missing quantum Tanner source roadmap table");
}

fn expect_quantum_tanner_source_row<'a>(
    rows: &'a [QuantumTannerSourceRow<'a>],
    source: &str,
) -> &'a QuantumTannerSourceRow<'a> {
    rows.iter()
        .find(|row| row.source == source)
        .unwrap_or_else(|| panic!("missing roadmap row for {source}"))
}

fn assert_source_row_complete(row: &QuantumTannerSourceRow<'_>) {
    for (column, value) in [
        ("URL or local path", row.location),
        ("License status", row.license),
        ("Intended use", row.intended_use),
        ("Copying/import posture", row.copying_posture),
        ("Definition of done", row.definition_of_done),
    ] {
        assert!(
            !value.trim().is_empty() && value != "-",
            "{} row must have a nonempty {column}",
            row.source
        );
    }
}

fn assert_cell_contains(row: &QuantumTannerSourceRow<'_>, column: &str, value: &str) {
    let cell = match column {
        "location" => row.location,
        "license" => row.license,
        "intended_use" => row.intended_use,
        "copying_posture" => row.copying_posture,
        "definition_of_done" => row.definition_of_done,
        _ => panic!("unknown roadmap column {column}"),
    };
    assert!(
        cell.contains(value),
        "{} {column} should contain {value:?}, got {cell:?}",
        row.source
    );
}

#[test]
fn quantum_tanner_future_sources_doc_has_reference_table() {
    assert!(QUANTUM_TANNER_SOURCES_DOC.contains("future adapters/searchers"));
    assert!(QUANTUM_TANNER_SOURCES_DOC.contains("not part of the initial constructor"));
    assert!(QUANTUM_TANNER_SOURCES_DOC.contains("does not search for good groups"));
    assert!(QUANTUM_TANNER_SOURCES_DOC.contains("does not call GAP or Oscar"));

    let rows = quantum_tanner_source_rows(QUANTUM_TANNER_SOURCES_DOC);
    assert_eq!(rows.len(), 6);
    for row in &rows {
        assert_source_row_complete(row);
    }

    let qldpc = expect_quantum_tanner_source_row(&rows, "qLDPC local clone");
    assert_cell_contains(qldpc, "location", "drafts/qLDPC");
    assert_cell_contains(
        qldpc,
        "location",
        "drafts/qLDPC/src/qldpc/codes/quantum.py",
    );
    assert_cell_contains(qldpc, "location", "drafts/qLDPC/src/qldpc/objects.py");
    assert_cell_contains(qldpc, "location", "https://github.com/qLDPCOrg/qLDPC");
    assert_cell_contains(qldpc, "license", "Apache-2.0");
    assert_cell_contains(qldpc, "copying_posture", "cite");

    let quantum_expanders = expect_quantum_tanner_source_row(&rows, "QuantumExpanders.jl");
    assert_cell_contains(
        quantum_expanders,
        "location",
        "https://github.com/QuantumSavory/QuantumExpanders.jl",
    );
    assert_cell_contains(
        quantum_expanders,
        "intended_use",
        "mathematical/reference",
    );
    assert_cell_contains(
        quantum_expanders,
        "copying_posture",
        "unless license compatibility is confirmed",
    );

    let qtanner = expect_quantum_tanner_source_row(&rows, "qTanner");
    assert_cell_contains(qtanner, "location", "https://github.com/RebKatRad/qTanner");
    assert_cell_contains(qtanner, "intended_use", "source-grounded data/reference");
    assert_cell_contains(
        qtanner,
        "copying_posture",
        "unless license compatibility is confirmed",
    );

    let qtc = expect_quantum_tanner_source_row(&rows, "Giacomo-Fregona/QTC");
    assert_cell_contains(qtc, "location", "https://github.com/Giacomo-Fregona/QTC");
    assert_cell_contains(qtc, "license", "confirm");
    assert_cell_contains(qtc, "copying_posture", "No code reuse before license review");

    let sogrand = expect_quantum_tanner_source_row(&rows, "quantum-tanner-sogrand");
    assert_cell_contains(
        sogrand,
        "location",
        "https://github.com/grand-decoder/quantum-tanner-sogrand",
    );
    assert_cell_contains(sogrand, "license", "non-commercial academic");
    assert_cell_contains(sogrand, "copying_posture", "not suitable for code copying");

    let quits = expect_quantum_tanner_source_row(&rows, "QUITS");
    assert_cell_contains(quits, "location", "drafts/quits");
    assert_cell_contains(quits, "location", "https://github.com/mkangquantum/quits");
    assert_cell_contains(quits, "intended_use", "matrix-consumption inspiration");
    assert_cell_contains(quits, "copying_posture", "not a quantum Tanner constructor");
}

#[test]
fn quantum_tanner_contract_examples_compile() {
    let doc = include_str!("../doc/quantum_tanner.md");
    assert!(doc.contains("drafts/qLDPC/src/qldpc/codes/quantum.py"));
    assert!(doc.contains("drafts/qLDPC/src/qldpc/objects.py"));
    assert!(doc.contains("drafts/qLDPC/src/qldpc/codes/quantum_test.py"));
    assert!(doc.contains("https://github.com/qLDPCOrg/qLDPC"));
    assert!(doc.contains("https://github.com/QuantumSavory/QuantumExpanders.jl"));
    assert!(doc.contains("lr_cayley_no_cover_v1"));
    assert!(doc.contains("lr_cayley_bipartite_double_cover_v1"));
    assert!(doc.contains("lr_cayley_quadripartite_cover_v1"));
    assert!(doc.contains("UnsupportedConstructionMode"));
    assert!(doc.contains("<!-- quantum_tanner_contract:toric_d4_counting_convention -->"));
    assert!(doc.contains("n = |G| * |A| * |B| / 4 = 16 * 2 * 2 / 4 = 16"));
    assert!(doc.contains("<!-- quantum_tanner_contract:bad_non_symmetric_generator -->"));

    let toric = extract_marked_json(doc, "quantum_tanner_contract:toric_d4").unwrap();
    assert_eq!(toric["example_id"].as_str(), Some("toric_d4"));
    assert_eq!(
        toric["construction_mode"].as_str(),
        Some("lr_cayley_no_cover_v1")
    );

    let group = &toric["base_group"];
    assert_eq!(group["name"].as_str(), Some("Z4xZ4"));
    assert_eq!(group["identity"].as_u64(), Some(0));
    let table = usize_matrix(
        &group["multiplication_table"],
        "base_group.multiplication_table",
    );
    assert_group_table_shape(&table, 16);

    let a_generators = usize_array(&toric["a_generator_indices"], "a_generator_indices");
    let b_generators = usize_array(&toric["b_generator_indices"], "b_generator_indices");
    assert!(generators_are_symmetric(&table, 0, &a_generators));
    assert!(generators_are_symmetric(&table, 0, &b_generators));

    let expected = &toric["expected_css"];
    assert_eq!(expected["n"].as_u64(), Some(16));
    assert_eq!(expected["k"].as_u64(), Some(2));
    assert_eq!(expected["expected_distance"].as_u64(), Some(4));
    assert_eq!(
        documented_face_count(&table, &a_generators, &b_generators),
        expected["n"].as_u64().unwrap() as usize
    );

    let local_a = usize_matrix(&toric["local_codes"]["h_a"], "local_codes.h_a");
    let local_b = usize_matrix(&toric["local_codes"]["h_b"], "local_codes.h_b");
    assert!(local_a.iter().all(|row| row.len() == a_generators.len()));
    assert!(local_b.iter().all(|row| row.len() == b_generators.len()));
    assert!(local_a.iter().flatten().all(|&bit| bit <= 1));
    assert!(local_b.iter().flatten().all(|&bit| bit <= 1));

    let bad =
        extract_marked_json(doc, "quantum_tanner_contract:bad_non_symmetric_generator").unwrap();
    let bad_a = usize_array(&bad["a_generator_indices"], "bad.a_generator_indices");
    assert!(!generators_are_symmetric(&table, 0, &bad_a));
    assert_eq!(
        bad["expected_error"].as_str(),
        Some("NonSymmetricGeneratorSet")
    );
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
            "apm_kasai:p=96",
            "apm_kasai:p=192",
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
            .any(|entry| entry.spec == "apm_kasai:p=96" && entry.description.contains("P=96")),
        "apm_kasai entry should describe the fixed P=96 code: {catalog:?}"
    );
    assert!(
        catalog
            .iter()
            .any(|entry| entry.spec == "apm_kasai:p=192" && entry.description.contains("P=192")),
        "apm_kasai entry should describe the fixed P=192 code: {catalog:?}"
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
fn apm_kasai_p96_matches_expected_checks_and_rejects_other_p_values() {
    let checks = built_in_css_checks("apm_kasai:p=96").unwrap();

    assert_eq!(checks.code_id, "apm_kasai:p=96");
    assert_eq!(checks.num_cols, 1152);
    assert!(!checks.hx.is_empty());
    assert!(!checks.hz.is_empty());
    assert_strictly_increasing_rows(&checks.hx);
    assert_strictly_increasing_rows(&checks.hz);
    assert_rows_in_range(&checks.hx, checks.num_cols);
    assert_rows_in_range(&checks.hz, checks.num_cols);

    assert_eq!(
        built_in_css_checks("apm_kasai:p=128"),
        Err(QecError::UnsupportedBuiltInCssIntegerParameter {
            family: "apm_kasai".to_owned(),
            parameter: "p".to_owned(),
            value: 128,
            supported: "96, 192".to_owned(),
            note: "available Table A1 APM-CSS instances".to_owned(),
        })
    );
}

#[test]
fn apm_p192_builds_paper_stats() {
    let catalog = built_in_css_catalog();
    assert!(
        catalog.iter().any(|entry| entry.spec == "apm_kasai:p=192"),
        "catalog should expose apm_kasai:p=192: {catalog:?}"
    );

    let checks = built_in_css_checks("apm_kasai:p=192").unwrap();
    assert_eq!(checks.code_id, "apm_kasai:p=192");
    assert_eq!(checks.num_cols, 2304);
    assert_eq!(checks.hx.len(), 576);
    assert_eq!(checks.hz.len(), 576);
    assert_strictly_increasing_rows(&checks.hx);
    assert_strictly_increasing_rows(&checks.hz);
    assert_rows_in_range(&checks.hx, checks.num_cols);
    assert_rows_in_range(&checks.hz, checks.num_cols);

    let report = verify_apm_checks(&checks, &apm_p192_expectations()).unwrap();
    assert!(report.orthogonal);
    assert_eq!(report.num_cols, 2304);
    assert_eq!(report.mx, 576);
    assert_eq!(report.mz, 576);
    assert_eq!(report.k, 1156);
    assert_eq!(report.rank_x + report.rank_z, 1148);
    assert_eq!(
        report.x.row_weight,
        WeightStats {
            min: 12,
            average: 12.0,
            max: 12
        }
    );
    assert_eq!(
        report.z.row_weight,
        WeightStats {
            min: 12,
            average: 12.0,
            max: 12
        }
    );
    assert_eq!(
        report.x.column_weight,
        WeightStats {
            min: 3,
            average: 3.0,
            max: 3
        }
    );
    assert_eq!(
        report.z.column_weight,
        WeightStats {
            min: 3,
            average: 3.0,
            max: 3
        }
    );
    assert!(report.x.girth.meets_lower_bound(6));
    assert!(report.z.girth.meets_lower_bound(6));

    let mutated = apm_kasai_p192_checks_with_mutated_support();
    let err = verify_apm_checks(&mutated, &apm_p192_expectations()).unwrap_err();
    assert!(
        err.contains("expected orthogonal=true")
            || err.contains("expected k=1156")
            || err.contains("row weight")
            || err.contains("column weight"),
        "mutated P=192 support should fail structural verifier, got: {err}"
    );

    let unsupported = built_in_css_checks("apm_kasai:p=128").unwrap_err();
    let message = unsupported.to_string();
    assert!(
        message.contains("unsupported built-in CSS integer parameter p for family apm_kasai: 128"),
        "{message}"
    );
    assert!(message.contains("supported: 96, 192"), "{message}");
}

fn apm_kasai_p192_checks_with_mutated_support() -> BuiltInCssChecks {
    let mut checks = built_in_css_checks("apm_kasai:p=192").unwrap();
    let replacement = (0..checks.num_cols)
        .find(|candidate| !checks.hz[0].contains(candidate))
        .unwrap();
    checks.hz[0][0] = replacement;
    checks.hz[0].sort_unstable();
    checks
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
        parse_built_in_css_code_spec("apm_kasai:p=96"),
        Ok(BuiltInCssCodeSpec::Family {
            family: BuiltInCssFamily::ApmKasai,
            params: BuiltInCssParams::ApmKasai { p: 96 },
        })
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
        parse_built_in_css_code_spec("apm_kasai"),
        Err(QecError::MissingBuiltInCssParameter {
            family: "apm_kasai".to_owned(),
            parameter: "p".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("apm_kasai:"),
        Err(QecError::MissingBuiltInCssParameter {
            family: "apm_kasai".to_owned(),
            parameter: "p".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("apm_kasai:p=nope"),
        Err(QecError::InvalidBuiltInCssIntegerParameter {
            family: "apm_kasai".to_owned(),
            parameter: "p".to_owned(),
            value: "nope".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("apm_kasai:p=96,p=96"),
        Err(QecError::DuplicateBuiltInCssParameter {
            family: "apm_kasai".to_owned(),
            parameter: "p".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("apm_kasai:p"),
        Err(QecError::UnexpectedBuiltInCssParameter {
            family: "apm_kasai".to_owned(),
            parameter: "p".to_owned(),
        })
    );
    assert_eq!(
        parse_built_in_css_code_spec("apm_kasai:p=96,foo=1"),
        Err(QecError::UnexpectedBuiltInCssParameter {
            family: "apm_kasai".to_owned(),
            parameter: "foo".to_owned(),
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
