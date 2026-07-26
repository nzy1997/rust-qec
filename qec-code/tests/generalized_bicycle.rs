use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use qec_code::cli::{run, Cli, CodeCommands, Commands, CssArgs, CssMatrixKind};
use qec_code::css::SparseRowsMatrix;
use qec_code::family_contract::{
    construct_css, parse_css_construction_json, CssFamilySpec, GeneralizedBicycleSpec,
    RequestedFamilyId, verify_css_orthogonality,
};
use qec_code::QecError;
use tempfile::tempdir;

fn qec_code_bin() -> &'static str {
    env!("CARGO_BIN_EXE_qec-code")
}

fn fixture_spec() -> GeneralizedBicycleSpec {
    GeneralizedBicycleSpec {
        order: 5,
        a_exponents: vec![0, 1],
        b_exponents: vec![0, 2],
    }
}

fn fixture_hx() -> Vec<Vec<usize>> {
    vec![
        vec![0, 1, 5, 7],
        vec![1, 2, 6, 8],
        vec![2, 3, 7, 9],
        vec![3, 4, 5, 8],
        vec![0, 4, 6, 9],
    ]
}

fn fixture_hz() -> Vec<Vec<usize>> {
    vec![
        vec![0, 3, 5, 9],
        vec![1, 4, 5, 6],
        vec![0, 2, 6, 7],
        vec![1, 3, 7, 8],
        vec![2, 4, 8, 9],
    ]
}

fn assert_canonical_sparse_rows(rows: &[Vec<usize>]) {
    for row in rows {
        assert!(
            row.windows(2).all(|window| window[0] < window[1]),
            "row must be sorted and duplicate-free: {row:?}"
        );
    }
}

fn write_spec(path: &Path, contents: &str) -> PathBuf {
    let spec = path.join("generalized-bicycle.json");
    fs::write(&spec, contents).expect("spec fixture should be writable");
    spec
}

fn run_qec_code_in_process_os(args: Vec<OsString>) -> Result<String, QecError> {
    let mut argv = vec![OsString::from("qec-code")];
    argv.extend(args);
    run(Cli::parse_from(argv))
}

#[test]
fn generalized_bicycle_order5_matches_fixture() {
    let result = construct_css(CssFamilySpec::GeneralizedBicycle(fixture_spec()).into()).unwrap();

    assert_eq!(result.construction_id, "generalized_bicycle");
    assert_eq!(
        result.requested_family_id,
        Some(RequestedFamilyId::GeneralizedBicycle)
    );
    assert_eq!(result.normalized_parameters["order"], serde_json::json!(5));
    assert_eq!(
        result.normalized_parameters["a_exponents"],
        serde_json::json!([0, 1])
    );
    assert_eq!(
        result.normalized_parameters["b_exponents"],
        serde_json::json!([0, 2])
    );
    assert_eq!(result.provenance.adapter, "generalized_bicycle");
    assert_eq!(result.provenance.source, "CssFamilySpec::GeneralizedBicycle");
    assert!(result
        .provenance
        .normalized_input_digest
        .starts_with("sha256:"));

    assert_eq!(result.stats.n, 10);
    assert_eq!(result.stats.m_x, 5);
    assert_eq!(result.stats.m_z, 5);
    assert_eq!(result.stats.rank_x, 4);
    assert_eq!(result.stats.rank_z, 4);
    assert_eq!(result.stats.k, 2);
    assert_eq!(result.stats.d_x, Some(3));
    assert_eq!(result.stats.d_z, Some(3));
    assert_eq!(result.checks.h_x, fixture_hx());
    assert_eq!(result.checks.h_z, fixture_hz());
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();

    let parsed = parse_css_construction_json(
        r#"{"schema_version":1,"construction":"generalized_bicycle","order":5,"a_exponents":[0,1],"b_exponents":[0,2]}"#,
    )
    .unwrap();
    assert_eq!(parsed, CssFamilySpec::GeneralizedBicycle(fixture_spec()).into());
    let parsed_result = construct_css(parsed).unwrap();
    assert_eq!(
        serde_json::to_string(&result).unwrap(),
        serde_json::to_string(&parsed_result).unwrap()
    );

    let unsorted = construct_css(
        CssFamilySpec::GeneralizedBicycle(GeneralizedBicycleSpec {
            order: 5,
            a_exponents: vec![1, 0],
            b_exponents: vec![2, 0],
        })
        .into(),
    )
    .unwrap();
    assert_eq!(
        unsorted.normalized_parameters["a_exponents"],
        serde_json::json!([0, 1])
    );
    assert_eq!(unsorted.checks.h_x, fixture_hx());

    let dir = tempdir().unwrap();
    let spec = write_spec(
        dir.path(),
        r#"{"schema_version":1,"construction":"generalized_bicycle","order":5,"a_exponents":[0,1],"b_exponents":[0,2]}"#,
    );
    let output = std::process::Command::new(qec_code_bin())
        .args(["code", "css", "construct", "--spec"])
        .arg(&spec)
        .arg("hx")
        .output()
        .expect("qec-code binary should run");
    assert!(output.status.success());
    assert_eq!(output.stderr, b"");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert_eq!(
        stdout,
        SparseRowsMatrix::new(10, fixture_hx()).unwrap().to_json_string()
    );

    let in_process = run_qec_code_in_process_os(vec![
        OsString::from("code"),
        OsString::from("css"),
        OsString::from("construct"),
        OsString::from("--spec"),
        spec.into_os_string(),
        OsString::from("hz"),
    ])
    .unwrap();
    assert_eq!(
        in_process,
        SparseRowsMatrix::new(10, fixture_hz()).unwrap().to_json_string()
    );
}

#[test]
fn generalized_bicycle_rejects_invalid_exponents() {
    for (case, expected) in [
        (
            GeneralizedBicycleSpec {
                order: 0,
                a_exponents: vec![0],
                b_exponents: vec![0],
            },
            "order must be nonzero",
        ),
        (
            GeneralizedBicycleSpec {
                order: 5,
                a_exponents: vec![0, 5],
                b_exponents: vec![0],
            },
            "a_exponents exponent 5 is out of range for order 5",
        ),
        (
            GeneralizedBicycleSpec {
                order: 5,
                a_exponents: vec![0, 1, 1],
                b_exponents: vec![0],
            },
            "a_exponents contains duplicate exponent 1",
        ),
        (
            GeneralizedBicycleSpec {
                order: 5,
                a_exponents: vec![],
                b_exponents: vec![0],
            },
            "a_exponents must not be empty",
        ),
        (
            GeneralizedBicycleSpec {
                order: 5,
                a_exponents: vec![0],
                b_exponents: vec![],
            },
            "b_exponents must not be empty",
        ),
    ] {
        assert!(matches!(
            construct_css(CssFamilySpec::GeneralizedBicycle(case).into()),
            Err(QecError::InvalidCssConstruction { construction, reason })
                if construction == "generalized_bicycle" && reason == expected
        ));
    }

    assert!(matches!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"generalized_bicycle","order":5,"a_exponents":[0],"b_exponents":[0,"x"]}"#
        ),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "generalized_bicycle"
                && reason == "b_exponents[1] must be a nonnegative integer"
    ));
}
