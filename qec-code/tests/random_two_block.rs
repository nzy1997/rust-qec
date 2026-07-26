use clap::Parser;
use qec_code::QecError;
use qec_code::cli::Cli;
use qec_code::cli::run;
use qec_code::codes::random_two_block::{
    RANDOM_TWO_BLOCK_ALGORITHM_V1, RandomTwoBlockSpec, random_two_block_css_checks,
};
use qec_code::css::{CssCode, SparseRowsMatrix};
use qec_code::distance::compute_distance;
use qec_code::family_contract::{
    CssFamilySpec, RequestedFamilyId, construct_css, parse_css_construction_json,
    verify_css_orthogonality,
};
use qec_code::finite_group::{FiniteGroupSpec, MAX_FINITE_GROUP_ORDER};
use tempfile::tempdir;

fn s3_table() -> Vec<Vec<usize>> {
    vec![
        vec![0, 1, 2, 3, 4, 5],
        vec![1, 2, 0, 4, 5, 3],
        vec![2, 0, 1, 5, 3, 4],
        vec![3, 5, 4, 0, 2, 1],
        vec![4, 3, 5, 1, 0, 2],
        vec![5, 4, 3, 2, 1, 0],
    ]
}

fn s3_group() -> FiniteGroupSpec {
    FiniteGroupSpec::new(6, 0, s3_table()).unwrap()
}

fn s3_spec() -> RandomTwoBlockSpec {
    RandomTwoBlockSpec::new(s3_group(), 2, 2, 7, RANDOM_TWO_BLOCK_ALGORITHM_V1).unwrap()
}

fn s3_request_json(include_seed: bool) -> String {
    let seed_field = if include_seed { r#","seed":7"# } else { "" };
    format!(
        r#"{{"schema_version":1,"construction":"random_two_block","group":{{"name":"S3","element_order":"0=e,1=r,2=r^2,3=s,4=rs,5=r^2s","order":6,"identity":0,"multiplication_table":{}}},"support_a_weight":2,"support_b_weight":2{seed_field},"algorithm_version":1}}"#,
        serde_json::to_string(&s3_table()).unwrap()
    )
}

fn run_qec_code_in_process(args: &[&str]) -> Result<String, QecError> {
    let mut argv = vec!["qec-code"];
    argv.extend(args);
    run(Cli::parse_from(argv))
}

fn expected_hx() -> Vec<Vec<usize>> {
    vec![
        vec![3, 5, 6, 10],
        vec![4, 5, 7, 11],
        vec![3, 4, 8, 9],
        vec![0, 2, 8, 9],
        vec![1, 2, 6, 10],
        vec![0, 1, 7, 11],
    ]
}

fn expected_hz() -> Vec<Vec<usize>> {
    vec![
        vec![0, 4, 9, 11],
        vec![1, 5, 10, 11],
        vec![2, 3, 9, 10],
        vec![2, 3, 6, 8],
        vec![0, 4, 7, 8],
        vec![1, 5, 6, 7],
    ]
}

fn css_code_from_sparse(n: usize, h_x: &[Vec<usize>], h_z: &[Vec<usize>]) -> CssCode {
    let hx = SparseRowsMatrix::new(n, h_x.to_vec())
        .unwrap()
        .to_dense_rows();
    let hz = SparseRowsMatrix::new(n, h_z.to_vec())
        .unwrap()
        .to_dense_rows();
    CssCode::from_hx_hz(hx, hz).unwrap()
}

#[test]
fn random_two_block_s3_seed7_matches_fixture() {
    let checks = random_two_block_css_checks(&s3_spec()).unwrap();

    assert_eq!(checks.num_cols, 12);
    assert_eq!(checks.support_a, vec![3, 5]);
    assert_eq!(checks.support_b, vec![0, 4]);
    assert_eq!(checks.h_x, expected_hx());
    assert_eq!(checks.h_z, expected_hz());
    assert_eq!(checks.metadata.seed, 7);
    assert_eq!(checks.metadata.support_a_weight, 2);
    assert_eq!(checks.metadata.support_b_weight, 2);
    assert_eq!(
        checks.metadata.algorithm_version,
        RANDOM_TWO_BLOCK_ALGORITHM_V1
    );
    assert!(checks.metadata.group_digest.starts_with("sha256:"));
    assert_eq!(checks.metadata.group_digest.len(), "sha256:".len() + 64);

    verify_css_orthogonality(checks.num_cols, &checks.h_x, &checks.h_z).unwrap();
    let css = css_code_from_sparse(checks.num_cols, &checks.h_x, &checks.h_z);
    assert_eq!(css.code().n(), 12);
    assert_eq!(css.code().num_logical_qubits(), 2);
    let distance = compute_distance(css.code()).unwrap();
    assert_eq!(distance.distance, 2);

    let common = construct_css(CssFamilySpec::RandomTwoBlock(s3_spec()).into()).unwrap();
    assert_eq!(common.construction_id, "random_two_block");
    assert_eq!(
        common.requested_family_id,
        Some(RequestedFamilyId::RandomTwoBlock)
    );
    assert_eq!(common.stats.n, 12);
    assert_eq!(common.stats.rank_x, 5);
    assert_eq!(common.stats.rank_z, 5);
    assert_eq!(common.stats.k, 2);
    assert_eq!(common.checks.h_x, expected_hx());
    assert_eq!(common.checks.h_z, expected_hz());
    assert_eq!(common.normalized_parameters["seed"], serde_json::json!(7));
    assert_eq!(
        common.normalized_parameters["support_a_weight"],
        serde_json::json!(2)
    );
    assert_eq!(
        common.normalized_parameters["support_b_weight"],
        serde_json::json!(2)
    );
    assert_eq!(
        common.normalized_parameters["algorithm_version"],
        serde_json::json!(RANDOM_TWO_BLOCK_ALGORITHM_V1)
    );
    assert_eq!(
        common.normalized_parameters["support_a"],
        serde_json::json!([3, 5])
    );
    assert_eq!(
        common.normalized_parameters["support_b"],
        serde_json::json!([0, 4])
    );
    assert_eq!(
        common.normalized_parameters["group_digest"],
        serde_json::json!(checks.metadata.group_digest)
    );

    let parsed = parse_css_construction_json(&s3_request_json(true)).unwrap();
    let parsed_common = construct_css(parsed).unwrap();
    assert_eq!(parsed_common.checks, common.checks);
    assert_eq!(
        parsed_common.normalized_parameters,
        common.normalized_parameters
    );

    let dir = tempdir().unwrap();
    let spec_path = dir.path().join("random-two-block-s3.json");
    std::fs::write(&spec_path, s3_request_json(true)).unwrap();
    let cli_hx = run_qec_code_in_process(&[
        "code",
        "css",
        "construct",
        "--spec",
        spec_path.to_str().unwrap(),
        "hx",
    ])
    .unwrap();
    let cli_hz = run_qec_code_in_process(&[
        "code",
        "css",
        "construct",
        "--spec",
        spec_path.to_str().unwrap(),
        "hz",
    ])
    .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&cli_hx).unwrap()["rows"],
        serde_json::json!(expected_hx())
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&cli_hz).unwrap()["rows"],
        serde_json::json!(expected_hz())
    );
}

#[test]
fn random_two_block_s3_seed1_uses_swap_sampling() {
    let spec = RandomTwoBlockSpec::new(s3_group(), 2, 3, 1, RANDOM_TWO_BLOCK_ALGORITHM_V1)
        .unwrap();
    let checks = random_two_block_css_checks(&spec).unwrap();

    assert_eq!(checks.support_a, vec![0, 5]);
    assert_eq!(checks.support_b, vec![0, 1, 3]);
}

#[test]
fn random_two_block_rejects_invalid_sampling_specs() {
    assert!(matches!(
        RandomTwoBlockSpec::new(s3_group(), 7, 2, 7, RANDOM_TWO_BLOCK_ALGORITHM_V1),
        Err(QecError::InvalidRandomTwoBlockSpec {
            option: "support_a_weight",
            ..
        })
    ));
    assert!(matches!(
        RandomTwoBlockSpec::new(s3_group(), 2, 7, 7, RANDOM_TWO_BLOCK_ALGORITHM_V1),
        Err(QecError::InvalidRandomTwoBlockSpec {
            option: "support_b_weight",
            ..
        })
    ));
    assert!(matches!(
        RandomTwoBlockSpec::new(s3_group(), 0, 2, 7, RANDOM_TWO_BLOCK_ALGORITHM_V1),
        Err(QecError::InvalidRandomTwoBlockSpec {
            option: "support_a_weight",
            ..
        })
    ));
    assert!(matches!(
        RandomTwoBlockSpec::new(s3_group(), 2, 0, 7, RANDOM_TWO_BLOCK_ALGORITHM_V1),
        Err(QecError::InvalidRandomTwoBlockSpec {
            option: "support_b_weight",
            ..
        })
    ));
    assert_eq!(
        RandomTwoBlockSpec::new(s3_group(), 2, 2, 7, 2),
        Err(QecError::UnsupportedRandomTwoBlockAlgorithm {
            algorithm_version: 2,
        })
    );
    assert_eq!(
        FiniteGroupSpec::new(MAX_FINITE_GROUP_ORDER + 1, 0, Vec::new()),
        Err(QecError::GroupOrderLimitExceeded {
            order: MAX_FINITE_GROUP_ORDER + 1,
            max_order: MAX_FINITE_GROUP_ORDER,
        })
    );
    assert!(matches!(
        FiniteGroupSpec::new(6, 0, vec![vec![0, 1, 2, 3, 4, 6]; 6]),
        Err(QecError::InvalidFiniteGroupTable { .. })
    ));
    assert!(matches!(
        parse_css_construction_json(&s3_request_json(false)),
        Err(QecError::InvalidRandomTwoBlockSpec { option: "seed", .. })
    ));
    assert!(matches!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"random_two_block","group":{"order":257,"identity":0,"multiplication_table":[]},"support_a_weight":1,"support_b_weight":1,"seed":7,"algorithm_version":1}"#
        ),
        Err(QecError::GroupOrderLimitExceeded { .. })
    ));
}
