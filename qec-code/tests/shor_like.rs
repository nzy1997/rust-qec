use std::path::{Path, PathBuf};

use clap::Parser;
use qec_code::QecError;
use qec_code::cli::{Cli, CssMatrixKind, run};
use qec_code::css::{CssCode, SparseRowsMatrix};
use qec_code::distance::compute_distance;
use qec_code::family_contract::{
    CssFamilySpec, RequestedFamilyId, ShorLikeSpec, construct_css, parse_css_construction_json,
    verify_css_orthogonality,
};
use tempfile::tempdir;

fn assert_canonical_sparse_rows(rows: &[Vec<usize>]) {
    for row in rows {
        assert!(
            row.windows(2).all(|window| window[0] < window[1]),
            "row must contain sorted unique supports: {row:?}"
        );
    }
}

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

fn cli_export_from_spec(spec: PathBuf, matrix: CssMatrixKind) -> String {
    let matrix = match matrix {
        CssMatrixKind::Hx => "hx",
        CssMatrixKind::Hz => "hz",
    };
    run(Cli::parse_from([
        "qec-code",
        "code",
        "css",
        "construct",
        "--spec",
        spec.to_str().expect("spec path should be UTF-8"),
        matrix,
    ]))
    .unwrap()
}

#[test]
fn shor_like_3x3_matches_fixture() {
    let expected_hx = vec![vec![0, 1, 2, 3, 4, 5], vec![3, 4, 5, 6, 7, 8]];
    let expected_hz = vec![
        vec![0, 1],
        vec![1, 2],
        vec![3, 4],
        vec![4, 5],
        vec![6, 7],
        vec![7, 8],
    ];
    let result = construct_css(
        CssFamilySpec::ShorLike(ShorLikeSpec {
            outer_blocks: 3,
            inner_block: 3,
        })
        .into(),
    )
    .unwrap();

    assert_eq!(result.schema_version, 1);
    assert_eq!(result.construction_id, "shor_like");
    assert_eq!(
        result.requested_family_id,
        Some(RequestedFamilyId::ShorLike)
    );
    assert_eq!(
        result.normalized_parameters["outer_blocks"],
        serde_json::json!(3)
    );
    assert_eq!(
        result.normalized_parameters["inner_block"],
        serde_json::json!(3)
    );
    assert_eq!(result.stats.n, 9);
    assert_eq!(result.stats.m_x, 2);
    assert_eq!(result.stats.m_z, 6);
    assert_eq!(result.stats.rank_x, 2);
    assert_eq!(result.stats.rank_z, 6);
    assert_eq!(result.stats.k, 1);
    assert_eq!(result.stats.d_x, Some(3));
    assert_eq!(result.stats.d_z, Some(3));
    assert_eq!(result.stats.d_x.unwrap().min(result.stats.d_z.unwrap()), 3);
    assert_eq!(result.checks.h_x, expected_hx);
    assert_eq!(result.checks.h_z, expected_hz);
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();
    assert_eq!(
        compute_distance(css_code_from_result(&result).code())
            .unwrap()
            .distance,
        3
    );

    let parsed = parse_css_construction_json(
        r#"{"schema_version":1,"construction":"shor_like","outer_blocks":3,"inner_block":3}"#,
    )
    .unwrap();
    assert_eq!(
        parsed,
        CssFamilySpec::ShorLike(ShorLikeSpec {
            outer_blocks: 3,
            inner_block: 3,
        })
        .into()
    );
    let repeated = construct_css(parsed).unwrap();
    assert_eq!(
        serde_json::to_string(&result).unwrap(),
        serde_json::to_string(&repeated).unwrap()
    );

    let dir = tempdir().unwrap();
    let spec_path = write_spec(
        dir.path(),
        "shor-like-3x3.json",
        r#"{"schema_version":1,"construction":"shor_like","outer_blocks":3,"inner_block":3}"#,
    );
    let cli_hx = cli_export_from_spec(spec_path, CssMatrixKind::Hx);
    assert_eq!(
        cli_hx,
        SparseRowsMatrix::new(result.stats.n, result.checks.h_x.clone())
            .unwrap()
            .to_json_string()
    );
}

#[test]
fn shor_like_rectangular_3x4_has_expected_parameters() {
    let result = construct_css(
        CssFamilySpec::ShorLike(ShorLikeSpec {
            outer_blocks: 3,
            inner_block: 4,
        })
        .into(),
    )
    .unwrap();

    assert_eq!(result.construction_id, "shor_like");
    assert_eq!(
        result.requested_family_id,
        Some(RequestedFamilyId::ShorLike)
    );
    assert_eq!(result.stats.n, 12);
    assert_eq!(result.stats.m_x, 2);
    assert_eq!(result.stats.m_z, 9);
    assert_eq!(result.stats.rank_x, 2);
    assert_eq!(result.stats.rank_z, 9);
    assert_eq!(result.stats.k, 1);
    assert_eq!(result.stats.d_x, Some(4));
    assert_eq!(result.stats.d_z, Some(3));
    assert_eq!(result.stats.d_x.unwrap().min(result.stats.d_z.unwrap()), 3);
    assert_eq!(
        result.checks.h_x,
        vec![vec![0, 1, 2, 3, 4, 5, 6, 7], vec![4, 5, 6, 7, 8, 9, 10, 11]]
    );
    assert_eq!(
        result.checks.h_z,
        vec![
            vec![0, 1],
            vec![1, 2],
            vec![2, 3],
            vec![4, 5],
            vec![5, 6],
            vec![6, 7],
            vec![8, 9],
            vec![9, 10],
            vec![10, 11],
        ]
    );
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();
    assert_eq!(
        compute_distance(css_code_from_result(&result).code())
            .unwrap()
            .distance,
        3
    );

    let dir = tempdir().unwrap();
    let spec_path = write_spec(
        dir.path(),
        "shor-like-3x4.json",
        r#"{"schema_version":1,"construction":"shor_like","outer_blocks":3,"inner_block":4}"#,
    );
    let cli_hz = cli_export_from_spec(spec_path, CssMatrixKind::Hz);
    assert_eq!(
        cli_hz,
        SparseRowsMatrix::new(result.stats.n, result.checks.h_z.clone())
            .unwrap()
            .to_json_string()
    );
}

#[test]
fn shor_like_rejects_invalid_dimensions() {
    for spec in [
        ShorLikeSpec {
            outer_blocks: 1,
            inner_block: 3,
        },
        ShorLikeSpec {
            outer_blocks: 3,
            inner_block: 1,
        },
        ShorLikeSpec {
            outer_blocks: 0,
            inner_block: 3,
        },
        ShorLikeSpec {
            outer_blocks: 3,
            inner_block: 0,
        },
    ] {
        assert!(matches!(
            construct_css(CssFamilySpec::ShorLike(spec).into()),
            Err(QecError::InvalidCssConstruction { construction, reason })
                if construction == "shor_like" && reason.contains("at least 2")
        ));
    }

    for body in [
        r#"{"schema_version":1,"construction":"shor_like","inner_block":3}"#,
        r#"{"schema_version":1,"construction":"shor_like","outer_blocks":3}"#,
        r#"{"schema_version":1,"construction":"shor_like","outer_blocks":0,"inner_block":3}"#,
        r#"{"schema_version":1,"construction":"shor_like","outer_blocks":3,"inner_block":0}"#,
    ] {
        assert!(matches!(
            parse_css_construction_json(body),
            Err(QecError::InvalidCssConstruction { construction, .. })
                if construction == "shor_like"
        ));
    }

    assert!(matches!(
        construct_css(CssFamilySpec::ShorLike(ShorLikeSpec {
            outer_blocks: usize::MAX,
            inner_block: 2,
        }).into()),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "shor_like" && reason.contains("overflow")
    ));

    let json_overflow = format!(
        r#"{{"schema_version":1,"construction":"shor_like","outer_blocks":{},"inner_block":2}}"#,
        usize::MAX
    );
    let parsed = parse_css_construction_json(&json_overflow).unwrap();
    assert!(matches!(
        construct_css(parsed),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "shor_like" && reason.contains("overflow")
    ));
}
