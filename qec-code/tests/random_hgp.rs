use std::ffi::OsString;

use clap::Parser;
use qec_code::QecError;
use qec_code::cli::{Cli, run};
use qec_code::codes::random_hgp::{
    RandomHgpSpec, RegularClassicalCodeSpec, random_hgp_spec_from_json_str,
    sample_random_hgp_classical_matrices,
};
use qec_code::family_contract::{
    CssFamilySpec, RequestedFamilyId, construct_css, parse_css_construction_json,
    verify_css_orthogonality,
};
use qec_code::regular_classical::REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1;
use tempfile::tempdir;

fn regular_fixture_spec(seed: u64) -> RegularClassicalCodeSpec {
    RegularClassicalCodeSpec {
        column_count: 6,
        row_count: 4,
        column_weight: 2,
        row_weight: 3,
        seed,
        algorithm_version: REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1,
        retry_limit: 16,
    }
}

fn fixture_spec() -> RandomHgpSpec {
    RandomHgpSpec::new(regular_fixture_spec(7), regular_fixture_spec(7)).unwrap()
}

fn fixture_json() -> String {
    r#"{"schema_version":1,"construction":"random_hgp","left":{"column_count":6,"row_count":4,"column_weight":2,"row_weight":3,"seed":7,"algorithm_version":1,"retry_limit":16},"right":{"column_count":6,"row_count":4,"column_weight":2,"row_weight":3,"seed":7,"algorithm_version":1,"retry_limit":16}}"#.to_owned()
}

fn fixture_json_without_left_seed() -> String {
    r#"{"schema_version":1,"construction":"random_hgp","left":{"column_count":6,"row_count":4,"column_weight":2,"row_weight":3,"algorithm_version":1,"retry_limit":16},"right":{"column_count":6,"row_count":4,"column_weight":2,"row_weight":3,"seed":7,"algorithm_version":1,"retry_limit":16}}"#.to_owned()
}

fn expected_classical_rows() -> Vec<Vec<usize>> {
    vec![vec![0, 1, 2], vec![0, 3, 4], vec![1, 3, 5], vec![2, 4, 5]]
}

fn run_qec_code_in_process(args: &[&str]) -> Result<String, QecError> {
    let mut argv = vec![OsString::from("qec-code")];
    argv.extend(args.iter().map(OsString::from));
    run(Cli::parse_from(argv))
}

#[test]
fn random_hgp_seed7_matches_fixture() {
    let samples = sample_random_hgp_classical_matrices(&fixture_spec()).unwrap();
    assert_eq!(samples.left.rows, expected_classical_rows());
    assert_eq!(samples.right.rows, expected_classical_rows());

    let result = construct_css(CssFamilySpec::RandomHgp(fixture_spec()).into()).unwrap();
    assert_eq!(result.construction_id, "random_hgp");
    assert_eq!(
        result.requested_family_id,
        Some(RequestedFamilyId::RandomHgp)
    );
    assert_eq!(result.stats.n, 52);
    assert_eq!(result.stats.m_x, 24);
    assert_eq!(result.stats.m_z, 24);
    assert_eq!(result.stats.rank_x, 21);
    assert_eq!(result.stats.rank_z, 21);
    assert_eq!(result.stats.k, 10);
    assert_eq!(result.stats.d_x, None);
    assert_eq!(result.stats.d_z, None);
    assert!(result.checks.h_x.iter().all(|row| row.len() == 5));
    assert!(result.checks.h_z.iter().all(|row| row.len() == 5));
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();

    assert_eq!(
        result.normalized_parameters["left"]["classical_spec"]["seed"],
        serde_json::json!(7)
    );
    assert_eq!(
        result.normalized_parameters["left"]["classical_spec"]["algorithm_version"],
        serde_json::json!(REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1)
    );
    assert_eq!(
        result.normalized_parameters["left"]["sampler_version"],
        serde_json::json!(REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1)
    );
    assert_eq!(
        result.normalized_parameters["left"]["rows"],
        serde_json::json!(expected_classical_rows())
    );
    assert_eq!(
        result.normalized_parameters["right"]["classical_spec"]["seed"],
        serde_json::json!(7)
    );
    assert_eq!(
        result.normalized_parameters["right"]["sampler_version"],
        serde_json::json!(REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1)
    );
    assert_eq!(
        result.normalized_parameters["right"]["rows"],
        serde_json::json!(expected_classical_rows())
    );

    let repeated = construct_css(CssFamilySpec::RandomHgp(fixture_spec()).into()).unwrap();
    assert_eq!(
        serde_json::to_string(&result).unwrap(),
        serde_json::to_string(&repeated).unwrap()
    );

    let parsed = parse_css_construction_json(&fixture_json()).unwrap();
    assert_eq!(parsed, CssFamilySpec::RandomHgp(fixture_spec()).into());
    let parsed_result = construct_css(parsed).unwrap();
    assert_eq!(parsed_result.checks, result.checks);
    assert_eq!(
        parsed_result.normalized_parameters,
        result.normalized_parameters
    );

    let direct_spec = random_hgp_spec_from_json_str(&fixture_json()).unwrap();
    assert_eq!(direct_spec, fixture_spec());

    let dir = tempdir().unwrap();
    let spec_path = dir.path().join("random-hgp.json");
    std::fs::write(&spec_path, fixture_json()).unwrap();
    let path = spec_path.to_str().unwrap();

    let hx_json: serde_json::Value = serde_json::from_str(
        &run_qec_code_in_process(&["code", "css", "construct", "--spec", path, "hx"]).unwrap(),
    )
    .unwrap();
    let hz_json: serde_json::Value = serde_json::from_str(
        &run_qec_code_in_process(&["code", "css", "construct", "--spec", path, "hz"]).unwrap(),
    )
    .unwrap();
    let metadata: serde_json::Value = serde_json::from_str(
        &run_qec_code_in_process(&["code", "css", "construct", "--spec", path, "metadata"])
            .unwrap(),
    )
    .unwrap();

    assert_eq!(hx_json["format"], "sparse_rows");
    assert_eq!(hx_json["num_cols"], 52);
    assert_eq!(hx_json["rows"], serde_json::json!(result.checks.h_x));
    assert_eq!(hz_json["format"], "sparse_rows");
    assert_eq!(hz_json["num_cols"], 52);
    assert_eq!(hz_json["rows"], serde_json::json!(result.checks.h_z));
    assert_eq!(metadata["construction_id"], "random_hgp");
    assert_eq!(metadata["requested_family_id"], "random_hgp");
    assert_eq!(metadata["stats"]["k"], 10);
    assert_eq!(
        metadata["normalized_parameters"]["left"]["rows"],
        serde_json::json!(expected_classical_rows())
    );
}

#[test]
fn random_hgp_rejects_unreproducible_specs() {
    assert!(matches!(
        parse_css_construction_json(&fixture_json_without_left_seed()),
        Err(QecError::InvalidRandomHgpSpec { option: "seed", .. })
    ));

    let impossible = RandomHgpSpec::new(
        RegularClassicalCodeSpec {
            column_count: 5,
            row_count: 4,
            column_weight: 2,
            row_weight: 3,
            seed: 7,
            algorithm_version: REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1,
            retry_limit: 16,
        },
        regular_fixture_spec(7),
    )
    .unwrap();
    assert!(matches!(
        sample_random_hgp_classical_matrices(&impossible),
        Err(QecError::RegularClassicalMatrixStubCountMismatch {
            column_stubs: 10,
            row_stubs: 12,
        })
    ));

    let unknown_version = RandomHgpSpec::new(
        RegularClassicalCodeSpec {
            algorithm_version: 2,
            ..regular_fixture_spec(7)
        },
        regular_fixture_spec(7),
    )
    .unwrap();
    assert_eq!(
        sample_random_hgp_classical_matrices(&unknown_version),
        Err(QecError::UnsupportedRegularClassicalMatrixAlgorithm {
            algorithm_version: 2,
        })
    );

    let retry_exhausted = RandomHgpSpec::new(
        RegularClassicalCodeSpec {
            column_count: 3,
            row_count: 3,
            column_weight: 2,
            row_weight: 2,
            seed: 1,
            algorithm_version: REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1,
            retry_limit: 1,
        },
        regular_fixture_spec(7),
    )
    .unwrap();
    assert!(matches!(
        sample_random_hgp_classical_matrices(&retry_exhausted),
        Err(QecError::RegularClassicalMatrixGenerationExhausted {
            retry_limit: 1,
            attempts: 1,
            ..
        })
    ));
}
