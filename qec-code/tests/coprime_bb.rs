use std::fs;
use std::path::Path;

use qec_code::css::SparseRowsMatrix;
use qec_code::family_contract::{
    construct_css, parse_css_construction_json, verify_css_orthogonality,
    CoprimeBivariateBicycleSpec, CssFamilySpec, RequestedFamilyId,
};
use qec_code::QecError;
use tempfile::tempdir;

fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/coprime_bb/l3_m5_pi_fixture.json"))
        .expect("coprime BB fixture should be valid JSON")
}

fn fixture_spec() -> CoprimeBivariateBicycleSpec {
    CoprimeBivariateBicycleSpec {
        l: 3,
        m: 5,
        a_exponents: vec![0, 1, 2],
        b_exponents: vec![0, 2, 7],
    }
}

fn fixture_rows(fixture: &serde_json::Value, name: &str) -> Vec<Vec<usize>> {
    serde_json::from_value(fixture["checks"][name].clone())
        .expect("fixture checks should be sparse rows")
}

fn write_request(path: &Path, fixture: &serde_json::Value) -> std::path::PathBuf {
    let spec = path.join("coprime-bb.json");
    fs::write(
        &spec,
        serde_json::to_string(&fixture["request"]).expect("fixture request is serializable"),
    )
    .expect("spec fixture should be writable");
    spec
}

#[test]
fn coprime_bb_3_5_matches_30_4_6_fixture() {
    let fixture = fixture();
    let expected_h_x = fixture_rows(&fixture, "h_x");
    let expected_h_z = fixture_rows(&fixture, "h_z");
    let result = construct_css(CssFamilySpec::CoprimeBb(fixture_spec()).into()).unwrap();

    assert_eq!(result.construction_id, "coprime_bb");
    assert_eq!(
        result.requested_family_id,
        Some(RequestedFamilyId::CoprimeBb)
    );
    assert_eq!(result.stats.n, 30);
    assert_eq!(result.stats.m_x, 15);
    assert_eq!(result.stats.m_z, 15);
    assert_eq!(result.stats.rank_x, 13);
    assert_eq!(result.stats.rank_z, 13);
    assert_eq!(result.stats.k, 4);
    assert_eq!(result.stats.d_x, Some(6));
    assert_eq!(result.stats.d_z, Some(6));
    assert_eq!(result.provenance.adapter, "coprime_bb");
    assert_eq!(result.provenance.source, "CssFamilySpec::CoprimeBb");
    assert!(result.provenance.normalized_input_digest.starts_with("sha256:"));
    assert_eq!(
        result.provenance.normalized_input_digest.len(),
        "sha256:".len() + 64
    );
    assert_eq!(result.checks.h_x[0], vec![0, 1, 2, 15, 17, 22]);
    assert_eq!(result.checks.h_z[0], vec![0, 8, 13, 15, 28, 29]);
    assert!(result.checks.h_x.iter().all(|row| row.len() == 6));
    assert!(result.checks.h_z.iter().all(|row| row.len() == 6));
    assert_eq!(result.checks.h_x, expected_h_x);
    assert_eq!(result.checks.h_z, expected_h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();
    assert_eq!(result.normalized_parameters["pi"], "xy");
    assert_eq!(result.normalized_parameters["cyclic_order"], 15);

    let parsed = parse_css_construction_json(
        &serde_json::to_string(&fixture["request"]).expect("fixture request is serializable"),
    )
    .unwrap();
    assert_eq!(parsed, CssFamilySpec::CoprimeBb(fixture_spec()).into());

    let dir = tempdir().unwrap();
    let spec = write_request(dir.path(), &fixture);
    for (matrix, rows) in [("hx", expected_h_x), ("hz", expected_h_z)] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_qec-code"))
            .args(["code", "css", "construct", "--spec"])
            .arg(&spec)
            .arg(matrix)
            .output()
            .expect("qec-code binary should run");
        assert!(output.status.success());
        assert_eq!(output.stderr, b"");
        assert_eq!(
            String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
            format!(
                "{}\n",
                SparseRowsMatrix::new(30, rows).unwrap().to_json_string()
            )
        );
    }
}

#[test]
fn coprime_bb_rejects_non_coprime_periods() {
    for (case, expected) in [
        (
            CoprimeBivariateBicycleSpec {
                l: 3,
                m: 6,
                a_exponents: vec![0],
                b_exponents: vec![0],
            },
            "periods l=3 and m=6 must be coprime",
        ),
        (
            CoprimeBivariateBicycleSpec {
                l: 0,
                m: 5,
                a_exponents: vec![0],
                b_exponents: vec![0],
            },
            "l must be nonzero",
        ),
        (
            CoprimeBivariateBicycleSpec {
                l: 3,
                m: 0,
                a_exponents: vec![0],
                b_exponents: vec![0],
            },
            "m must be nonzero",
        ),
        (
            CoprimeBivariateBicycleSpec {
                l: 3,
                m: 5,
                a_exponents: vec![0, 15],
                b_exponents: vec![0],
            },
            "a_exponents exponent 15 is out of range for cyclic order 15",
        ),
        (
            CoprimeBivariateBicycleSpec {
                l: 3,
                m: 5,
                a_exponents: vec![0],
                b_exponents: vec![15],
            },
            "b_exponents exponent 15 is out of range for cyclic order 15",
        ),
        (
            CoprimeBivariateBicycleSpec {
                l: 3,
                m: 5,
                a_exponents: vec![0, 1, 1],
                b_exponents: vec![0],
            },
            "a_exponents contains duplicate exponent 1",
        ),
        (
            CoprimeBivariateBicycleSpec {
                l: 3,
                m: 5,
                a_exponents: vec![0],
                b_exponents: vec![2, 0, 2],
            },
            "b_exponents contains duplicate exponent 2",
        ),
        (
            CoprimeBivariateBicycleSpec {
                l: 3,
                m: 5,
                a_exponents: vec![],
                b_exponents: vec![0],
            },
            "a_exponents must not be empty",
        ),
        (
            CoprimeBivariateBicycleSpec {
                l: 3,
                m: 5,
                a_exponents: vec![0],
                b_exponents: vec![],
            },
            "b_exponents must not be empty",
        ),
    ] {
        assert!(matches!(
            construct_css(CssFamilySpec::CoprimeBb(case).into()),
            Err(QecError::InvalidCssConstruction { construction, reason })
                if construction == "coprime_bb" && reason == expected
        ));
    }
}
