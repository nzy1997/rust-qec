use qec_code::codes::quantum_tanner::{
    quantum_tanner_css_checks, quantum_tanner_spec_from_json_str,
};
use qec_code::css::{CssCode, SparseRowsMatrix};
use qec_code::distance::compute_distance;
use qec_code::family_contract::{
    construct_css, parse_css_construction_json, verify_css_orthogonality, CssFamilySpec,
    RequestedFamilyId,
};
use qec_code::QecError;

fn canonical(mut rows: Vec<Vec<usize>>) -> Vec<Vec<usize>> {
    for row in &mut rows {
        row.sort_unstable();
    }
    rows
}

fn assert_canonical_sparse_rows(rows: &[Vec<usize>]) {
    for row in rows {
        assert!(row.windows(2).all(|window| window[0] < window[1]));
    }
}

fn quantum_tanner_request(spec_json: &str) -> String {
    format!(r#"{{"schema_version":1,"construction":"quantum_tanner","spec":{spec_json}}}"#)
}

#[test]
fn quantum_tanner_toric_d4_matches_legacy_constructor() {
    let fixture = include_str!("fixtures/quantum_tanner/toric_d4.json");
    let spec = quantum_tanner_spec_from_json_str(fixture).unwrap();
    let legacy = quantum_tanner_css_checks(&spec).unwrap();

    let common = construct_css(CssFamilySpec::QuantumTanner(spec).into()).unwrap();

    assert_eq!(common.construction_id, "quantum_tanner");
    assert_eq!(
        common.requested_family_id,
        Some(RequestedFamilyId::QuantumTanner)
    );
    assert_eq!(common.checks.h_x, canonical(legacy.hx));
    assert_eq!(common.checks.h_z, canonical(legacy.hz));
    assert_eq!(common.stats.n, 16);
    assert_eq!(common.stats.k, 2);
    assert!(common
        .checks
        .h_x
        .iter()
        .chain(common.checks.h_z.iter())
        .all(|row| row.len() == 4));
    assert_canonical_sparse_rows(&common.checks.h_x);
    assert_canonical_sparse_rows(&common.checks.h_z);
    verify_css_orthogonality(common.stats.n, &common.checks.h_x, &common.checks.h_z).unwrap();

    let hx = SparseRowsMatrix::new(common.stats.n, common.checks.h_x.clone())
        .unwrap()
        .to_dense_rows();
    let hz = SparseRowsMatrix::new(common.stats.n, common.checks.h_z.clone())
        .unwrap()
        .to_dense_rows();
    let css = CssCode::from_hx_hz(hx, hz).unwrap();
    let distance = compute_distance(css.code()).unwrap();
    assert_eq!(distance.distance, 4);
    assert_eq!(distance.witness.weight(), 4);

    assert_eq!(common.provenance.adapter, "quantum_tanner");
    assert_eq!(common.provenance.source, "CssFamilySpec::QuantumTanner");
    assert!(common
        .provenance
        .normalized_input_digest
        .starts_with("sha256:"));
    assert_eq!(
        common.provenance.normalized_input_digest.len(),
        "sha256:".len() + 64
    );

    let json_common =
        construct_css(parse_css_construction_json(&quantum_tanner_request(fixture)).unwrap())
            .unwrap();
    assert_eq!(
        json_common.provenance.normalized_input_digest,
        common.provenance.normalized_input_digest
    );
    assert_eq!(json_common.checks, common.checks);
}

#[test]
fn quantum_tanner_contract_preserves_typed_errors() {
    let non_symmetric = include_str!("fixtures/quantum_tanner/invalid_non_symmetric_a.json");
    let non_symmetric_spec = parse_css_construction_json(&quantum_tanner_request(non_symmetric))
        .expect("non-symmetric generator set parses before construction validation");
    assert!(matches!(
        construct_css(non_symmetric_spec),
        Err(QecError::InvalidQuantumTannerGeneratorSet { set: "A", .. })
    ));

    let bad_table = include_str!("fixtures/quantum_tanner/invalid_bad_table.json");
    assert!(matches!(
        parse_css_construction_json(&quantum_tanner_request(bad_table)),
        Err(QecError::InvalidQuantumTannerGroupTable { .. })
    ));
}
