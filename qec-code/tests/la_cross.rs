use std::path::{Path, PathBuf};

use clap::Parser;
use qec_code::cli::{run, Cli};
#[cfg(feature = "distance-ilp-highs")]
use qec_code::css::{CssCode, SparseRowsMatrix};
#[cfg(feature = "distance-ilp-highs")]
use qec_code::distance::compute_distance;
use qec_code::family_contract::{
    construct_css, parse_css_construction_json, verify_css_orthogonality, CssFamilySpec,
    LaCrossBoundary, LaCrossSpec, RequestedFamilyId,
};
use qec_code::QecError;
use tempfile::tempdir;

fn open_spec() -> LaCrossSpec {
    LaCrossSpec {
        seed_length: 5,
        reach: 2,
        boundary: LaCrossBoundary::Open,
    }
}

fn periodic_spec() -> LaCrossSpec {
    LaCrossSpec {
        seed_length: 5,
        reach: 2,
        boundary: LaCrossBoundary::Periodic,
    }
}

fn open_json() -> &'static str {
    r#"{"schema_version":1,"construction":"la_cross","seed_length":5,"reach":2,"boundary":"open"}"#
}

fn periodic_json() -> &'static str {
    r#"{"schema_version":1,"construction":"la_cross","seed_length":5,"reach":2,"boundary":"periodic"}"#
}

fn expected_open_classical_rows() -> Vec<Vec<usize>> {
    vec![vec![0, 1, 2], vec![1, 2, 3], vec![2, 3, 4]]
}

fn expected_periodic_classical_rows() -> Vec<Vec<usize>> {
    vec![
        vec![0, 1, 2],
        vec![1, 2, 3],
        vec![2, 3, 4],
        vec![0, 3, 4],
        vec![0, 1, 4],
    ]
}

fn assert_canonical_sparse_rows(rows: &[Vec<usize>]) {
    for row in rows {
        assert!(
            row.windows(2).all(|window| window[0] < window[1]),
            "row must contain sorted unique supports: {row:?}"
        );
    }
}

#[cfg(feature = "distance-ilp-highs")]
fn css_code_from_result(result: &qec_code::family_contract::CssConstructionResult) -> CssCode {
    let hx = SparseRowsMatrix::new(result.stats.n, result.checks.h_x.clone())
        .unwrap()
        .to_dense_rows();
    let hz = SparseRowsMatrix::new(result.stats.n, result.checks.h_z.clone())
        .unwrap()
        .to_dense_rows();
    CssCode::from_hx_hz(hx, hz).unwrap()
}

fn write_spec(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).expect("spec should be writable");
    path
}

fn cli_construct_output(spec: &Path, output: &str) -> String {
    run(Cli::parse_from([
        "qec-code",
        "code",
        "css",
        "construct",
        "--spec",
        spec.to_str().expect("spec path should be UTF-8"),
        output,
    ]))
    .unwrap()
}

#[test]
fn la_cross_open_5_2_matches_fixture() {
    let result = construct_css(CssFamilySpec::LaCross(open_spec()).into()).unwrap();

    assert_eq!(result.schema_version, 1);
    assert_eq!(result.construction_id, "la_cross");
    assert_eq!(result.requested_family_id, Some(RequestedFamilyId::LaCross));
    assert_eq!(
        result.normalized_parameters["seed_length"],
        serde_json::json!(5)
    );
    assert_eq!(result.normalized_parameters["reach"], serde_json::json!(2));
    assert_eq!(
        result.normalized_parameters["boundary"],
        serde_json::json!("open")
    );
    assert_eq!(
        result.normalized_parameters["classical_check"],
        serde_json::json!({"num_cols": 5, "rows": expected_open_classical_rows()})
    );
    assert_eq!(result.provenance.adapter, "la_cross");
    assert_eq!(result.provenance.source, "CssFamilySpec::LaCross");
    assert!(result
        .provenance
        .normalized_input_digest
        .starts_with("sha256:"));

    assert_eq!(result.stats.n, 34);
    assert_eq!(result.stats.m_x, 15);
    assert_eq!(result.stats.m_z, 15);
    assert_eq!(result.stats.rank_x, 15);
    assert_eq!(result.stats.rank_z, 15);
    assert_eq!(result.stats.k, 4);
    assert_eq!(result.stats.d_x, Some(3));
    assert_eq!(result.stats.d_z, Some(3));
    assert_eq!(
        result
            .stats
            .d_x
            .zip(result.stats.d_z)
            .map(|(d_x, d_z)| d_x.min(d_z)),
        Some(3)
    );
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();
    #[cfg(feature = "distance-ilp-highs")]
    assert_eq!(
        compute_distance(css_code_from_result(&result).code())
            .unwrap()
            .distance,
        3
    );

    let parsed = parse_css_construction_json(open_json()).unwrap();
    assert_eq!(parsed, CssFamilySpec::LaCross(open_spec()).into());
    let parsed_result = construct_css(parsed).unwrap();
    assert_eq!(
        serde_json::to_string(&result).unwrap(),
        serde_json::to_string(&parsed_result).unwrap()
    );

    let repeated = construct_css(CssFamilySpec::LaCross(open_spec()).into()).unwrap();
    assert_eq!(
        result.provenance.normalized_input_digest,
        repeated.provenance.normalized_input_digest
    );
    assert_eq!(
        serde_json::to_string(&result.normalized_parameters).unwrap(),
        serde_json::to_string(&repeated.normalized_parameters).unwrap()
    );

    let dir = tempdir().unwrap();
    let spec_path = write_spec(dir.path(), "la-cross-open.json", open_json());
    let hx_json: serde_json::Value =
        serde_json::from_str(&cli_construct_output(&spec_path, "hx")).unwrap();
    let hz_json: serde_json::Value =
        serde_json::from_str(&cli_construct_output(&spec_path, "hz")).unwrap();
    let metadata: serde_json::Value =
        serde_json::from_str(&cli_construct_output(&spec_path, "metadata")).unwrap();

    assert_eq!(hx_json["format"], "sparse_rows");
    assert_eq!(hx_json["num_cols"], 34);
    assert_eq!(hx_json["rows"], serde_json::json!(result.checks.h_x));
    assert_eq!(hz_json["format"], "sparse_rows");
    assert_eq!(hz_json["num_cols"], 34);
    assert_eq!(hz_json["rows"], serde_json::json!(result.checks.h_z));
    assert_eq!(metadata["construction_id"], "la_cross");
    assert_eq!(metadata["requested_family_id"], "la_cross");
    assert_eq!(
        metadata["normalized_parameters"]["classical_check"]["rows"],
        serde_json::json!(expected_open_classical_rows())
    );
    assert_eq!(metadata["provenance"]["adapter"], "la_cross");
}

#[test]
fn la_cross_periodic_5_2_is_orthogonal() {
    let result = construct_css(CssFamilySpec::LaCross(periodic_spec()).into()).unwrap();

    assert_eq!(result.construction_id, "la_cross");
    assert_eq!(result.requested_family_id, Some(RequestedFamilyId::LaCross));
    assert_eq!(
        result.normalized_parameters["seed_length"],
        serde_json::json!(5)
    );
    assert_eq!(result.normalized_parameters["reach"], serde_json::json!(2));
    assert_eq!(
        result.normalized_parameters["boundary"],
        serde_json::json!("periodic")
    );
    assert_eq!(
        result.normalized_parameters["classical_check"],
        serde_json::json!({"num_cols": 5, "rows": expected_periodic_classical_rows()})
    );
    assert_eq!(result.stats.n, 50);
    assert_eq!(result.stats.m_x, 25);
    assert_eq!(result.stats.m_z, 25);
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();

    let parsed = parse_css_construction_json(periodic_json()).unwrap();
    assert_eq!(parsed, CssFamilySpec::LaCross(periodic_spec()).into());
    let parsed_result = construct_css(parsed).unwrap();
    assert_eq!(parsed_result.checks, result.checks);
    assert_eq!(
        serde_json::to_string(&parsed_result.normalized_parameters).unwrap(),
        serde_json::to_string(&result.normalized_parameters).unwrap()
    );
}

#[test]
fn la_cross_rejects_invalid_reach() {
    for (spec, expected) in [
        (
            LaCrossSpec {
                seed_length: 5,
                reach: 0,
                boundary: LaCrossBoundary::Open,
            },
            "reach must be nonzero",
        ),
        (
            LaCrossSpec {
                seed_length: 5,
                reach: 5,
                boundary: LaCrossBoundary::Open,
            },
            "reach must be less than seed_length",
        ),
        (
            LaCrossSpec {
                seed_length: 1,
                reach: 1,
                boundary: LaCrossBoundary::Periodic,
            },
            "seed_length must be at least 2",
        ),
    ] {
        assert!(matches!(
            construct_css(CssFamilySpec::LaCross(spec).into()),
            Err(QecError::InvalidCssConstruction { construction, reason })
                if construction == "la_cross" && reason.contains(expected)
        ));
    }

    assert!(matches!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"la_cross","seed_length":5,"reach":2,"boundary":"closed"}"#
        ),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "la_cross" && reason.contains("unknown la_cross boundary")
    ));

    assert!(matches!(
        construct_css(
            CssFamilySpec::LaCross(LaCrossSpec {
                seed_length: usize::MAX,
                reach: 1,
                boundary: LaCrossBoundary::Periodic,
            })
            .into()
        ),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "la_cross" && reason.contains("overflow")
    ));
}
