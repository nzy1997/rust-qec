use std::path::PathBuf;

use qec_code::QecError;
use qec_code::cli::{
    Cli, CodeCommands, Commands, CssArgs, CssCommands, CssConstructionOutput, run,
};
use qec_code::css::{CssCode, SparseRowsMatrix};
use qec_code::distance::compute_distance;
use qec_code::family_contract::{
    CssClassicalCheckSpec, CssConstructionSpec, HypergraphProductSpec, construct_css,
    parse_css_construction_json, verify_css_orthogonality,
};
use tempfile::tempdir;

fn classical_2x3() -> CssClassicalCheckSpec {
    CssClassicalCheckSpec {
        num_cols: 3,
        rows: vec![vec![0, 1], vec![1, 2]],
    }
}

fn fixture_spec() -> CssConstructionSpec {
    CssConstructionSpec::HypergraphProduct(HypergraphProductSpec {
        left: classical_2x3(),
        right: classical_2x3(),
    })
}

fn fixture_json() -> &'static str {
    r#"{"schema_version":1,"construction":"hypergraph_product","left":{"num_cols":3,"rows":[[0,1],[1,2]]},"right":{"num_cols":3,"rows":[[0,1],[1,2]]}}"#
}

fn expected_hx() -> Vec<Vec<usize>> {
    vec![
        vec![0, 3, 9],
        vec![1, 4, 9, 10],
        vec![2, 5, 10],
        vec![3, 6, 11],
        vec![4, 7, 11, 12],
        vec![5, 8, 12],
    ]
}

fn expected_hz() -> Vec<Vec<usize>> {
    vec![
        vec![0, 1, 9],
        vec![1, 2, 10],
        vec![3, 4, 9, 11],
        vec![4, 5, 10, 12],
        vec![6, 7, 11],
        vec![7, 8, 12],
    ]
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

fn construct_cli_output(spec_path: PathBuf, output: CssConstructionOutput) -> String {
    run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs {
                command: Some(CssCommands::Construct {
                    spec: spec_path,
                    output,
                }),
                code_id: None,
                matrix: None,
            }),
        },
    })
    .unwrap()
}

#[test]
fn hypergraph_product_matches_2x3_fixture() {
    let result = construct_css(fixture_spec()).unwrap();

    assert_eq!(result.construction_id, "hypergraph_product");
    assert_eq!(result.requested_family_id, None);
    assert_eq!(
        result.normalized_parameters["left"]["num_cols"],
        serde_json::json!(3)
    );
    assert_eq!(
        result.normalized_parameters["left"]["rows"],
        serde_json::json!([[0, 1], [1, 2]])
    );
    assert_eq!(
        result.normalized_parameters["right"]["num_cols"],
        serde_json::json!(3)
    );
    assert_eq!(
        result.normalized_parameters["right"]["rows"],
        serde_json::json!([[0, 1], [1, 2]])
    );
    assert_eq!(result.stats.n, 13);
    assert_eq!(result.stats.m_x, 6);
    assert_eq!(result.stats.m_z, 6);
    assert_eq!(result.stats.rank_x, 6);
    assert_eq!(result.stats.rank_z, 6);
    assert_eq!(result.stats.k, 1);
    assert_eq!(result.stats.d_x, None);
    assert_eq!(result.stats.d_z, None);
    assert_eq!(result.checks.h_x, expected_hx());
    assert_eq!(result.checks.h_z, expected_hz());
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();

    let distance =
        compute_distance(css_code(result.stats.n, &result.checks.h_x, &result.checks.h_z).code())
            .unwrap();
    assert_eq!(distance.distance, 3);
    assert_eq!(distance.witness.weight(), 3);

    let parsed = parse_css_construction_json(fixture_json()).unwrap();
    let parsed_result = construct_css(parsed).unwrap();
    assert_eq!(parsed_result.checks, result.checks);
    assert_eq!(
        serde_json::to_string(&parsed_result.normalized_parameters).unwrap(),
        serde_json::to_string(&result.normalized_parameters).unwrap()
    );

    let dir = tempdir().unwrap();
    let spec_path = dir.path().join("hgp.json");
    std::fs::write(&spec_path, fixture_json()).unwrap();

    let hx_json: serde_json::Value = serde_json::from_str(&construct_cli_output(
        spec_path.clone(),
        CssConstructionOutput::Hx,
    ))
    .unwrap();
    let hz_json: serde_json::Value = serde_json::from_str(&construct_cli_output(
        spec_path.clone(),
        CssConstructionOutput::Hz,
    ))
    .unwrap();
    let metadata: serde_json::Value = serde_json::from_str(&construct_cli_output(
        spec_path,
        CssConstructionOutput::Metadata,
    ))
    .unwrap();

    assert_eq!(hx_json["format"], "sparse_rows");
    assert_eq!(hx_json["num_cols"], 13);
    assert_eq!(hx_json["rows"], serde_json::json!(expected_hx()));
    assert_eq!(hz_json["format"], "sparse_rows");
    assert_eq!(hz_json["num_cols"], 13);
    assert_eq!(hz_json["rows"], serde_json::json!(expected_hz()));
    assert_eq!(metadata["construction_id"], "hypergraph_product");
    assert_eq!(metadata["requested_family_id"], serde_json::Value::Null);
    assert_eq!(metadata["stats"]["n"], 13);
    assert_eq!(metadata["stats"]["m_x"], 6);
    assert_eq!(metadata["stats"]["m_z"], 6);
    assert_eq!(metadata["stats"]["rank_x"], 6);
    assert_eq!(metadata["stats"]["rank_z"], 6);
    assert_eq!(metadata["stats"]["k"], 1);
    assert_eq!(metadata["checks"]["h_x"], serde_json::json!(expected_hx()));
    assert_eq!(metadata["checks"]["h_z"], serde_json::json!(expected_hz()));
}

#[test]
fn hypergraph_product_rejects_out_of_range_input() {
    let err = construct_css(CssConstructionSpec::HypergraphProduct(
        HypergraphProductSpec {
            left: CssClassicalCheckSpec {
                num_cols: 3,
                rows: vec![vec![0, 3]],
            },
            right: classical_2x3(),
        },
    ))
    .unwrap_err();

    assert_eq!(
        err,
        QecError::SparseGf2SupportOutOfRange {
            row: 0,
            support: 3,
            num_cols: 3,
        }
    );
}
