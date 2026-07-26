use std::path::{Path, PathBuf};

use clap::Parser;
use qec_code::QecError;
use qec_code::cli::{CodeCommands, Commands, CssArgs, CssMatrixKind, Cli, run};
use qec_code::css::SparseRowsMatrix;
use qec_code::family_contract::{
    CssConstructionSpec, CssFamilySpec, SurfaceLayout, SurfaceSpec, construct_css,
    parse_css_construction_json, verify_css_orthogonality,
};
use tempfile::tempdir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_text(name: &str) -> String {
    std::fs::read_to_string(workspace_root().join("tests/fixtures/css").join(name))
        .expect("fixture should be readable")
        .trim_end_matches('\n')
        .to_owned()
}

fn assert_canonical_sparse_rows(rows: &[Vec<usize>]) {
    for row in rows {
        assert!(
            row.windows(2).all(|window| window[0] < window[1]),
            "row must contain sorted unique supports: {row:?}"
        );
    }
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
fn rectangular_rotated_surface_3x5_matches_fixture() {
    let expected_hx = vec![
        vec![0, 5],
        vec![1, 2, 6, 7],
        vec![3, 4, 8, 9],
        vec![5, 6, 10, 11],
        vec![7, 8, 12, 13],
        vec![9, 14],
    ];
    let expected_hz = vec![
        vec![1, 2],
        vec![3, 4],
        vec![0, 1, 5, 6],
        vec![2, 3, 7, 8],
        vec![6, 7, 11, 12],
        vec![8, 9, 13, 14],
        vec![10, 11],
        vec![12, 13],
    ];

    let spec = SurfaceSpec {
        layout: SurfaceLayout::Rotated,
        row_distance: 3,
        column_distance: 5,
    };
    let result = construct_css(CssFamilySpec::Surface(spec.clone()).into()).unwrap();

    assert_eq!(result.construction_id, "surface_rotated");
    assert_eq!(result.normalized_parameters["layout"], serde_json::json!("rotated"));
    assert_eq!(result.normalized_parameters["row_distance"], serde_json::json!(3));
    assert_eq!(result.normalized_parameters["column_distance"], serde_json::json!(5));
    assert_eq!(result.stats.n, 15);
    assert_eq!(result.stats.m_x, 6);
    assert_eq!(result.stats.m_z, 8);
    assert_eq!(result.stats.rank_x, 6);
    assert_eq!(result.stats.rank_z, 8);
    assert_eq!(result.stats.k, 1);
    assert_eq!(result.stats.d_x, Some(5));
    assert_eq!(result.stats.d_z, Some(3));
    assert_eq!(result.checks.h_x, expected_hx);
    assert_eq!(result.checks.h_z, expected_hz);
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();

    let json = parse_css_construction_json(
        r#"{"schema_version":1,"construction":"surface","layout":"rotated","row_distance":3,"column_distance":5}"#,
    )
    .unwrap();
    assert_eq!(json, CssFamilySpec::Surface(spec).into());
}

#[test]
fn ordinary_surface_d3_matches_fixture() {
    let expected_hx = vec![
        vec![0, 3, 5],
        vec![1, 3, 4, 6],
        vec![2, 4, 7],
        vec![5, 8, 10],
        vec![6, 8, 9, 11],
        vec![7, 9, 12],
    ];
    let expected_hz = vec![
        vec![0, 1, 3],
        vec![1, 2, 4],
        vec![3, 5, 6, 8],
        vec![4, 6, 7, 9],
        vec![8, 10, 11],
        vec![9, 11, 12],
    ];
    let spec = SurfaceSpec {
        layout: SurfaceLayout::Unrotated,
        row_distance: 3,
        column_distance: 3,
    };

    let result = construct_css(CssFamilySpec::Surface(spec).into()).unwrap();

    assert_eq!(result.construction_id, "surface_unrotated");
    assert_eq!(result.stats.n, 13);
    assert_eq!(result.stats.m_x, 6);
    assert_eq!(result.stats.m_z, 6);
    assert_eq!(result.stats.rank_x, 6);
    assert_eq!(result.stats.rank_z, 6);
    assert_eq!(result.stats.k, 1);
    assert_eq!(result.stats.d_x, Some(3));
    assert_eq!(result.stats.d_z, Some(3));
    assert_eq!(result.checks.h_x, expected_hx);
    assert_eq!(result.checks.h_z, expected_hz);
    assert_canonical_sparse_rows(&result.checks.h_x);
    assert_canonical_sparse_rows(&result.checks.h_z);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();

    let dir = tempdir().unwrap();
    let spec_path = write_spec(
        dir.path(),
        "surface-unrotated-d3.json",
        r#"{"schema_version":1,"construction":"surface","layout":"unrotated","row_distance":3,"column_distance":3}"#,
    );
    let cli_hx = cli_export_from_spec(spec_path, CssMatrixKind::Hx);
    assert_eq!(
        cli_hx,
        SparseRowsMatrix::new(result.stats.n, expected_hx)
            .unwrap()
            .to_json_string()
    );
}

#[test]
fn legacy_rotated_surface_outputs_are_unchanged() {
    for distance in 2..=6 {
        let inline = CssConstructionSpec::from_inline(&format!("surface_rotated:d={distance}"))
            .unwrap();
        let typed = CssFamilySpec::Surface(SurfaceSpec::rotated_square(distance)).into();
        assert_eq!(inline, typed);

        let legacy = construct_css(inline).unwrap();
        let direct = construct_css(typed).unwrap();
        assert_eq!(legacy.checks, direct.checks);
        assert_eq!(legacy.stats, direct.stats);
        assert_eq!(legacy.stats.d_x, Some(distance));
        assert_eq!(legacy.stats.d_z, Some(distance));
    }

    let d3_hx = run(qec_code::cli::Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export(
                "surface_rotated:d=3".to_owned(),
                CssMatrixKind::Hx,
            )),
        },
    })
    .unwrap();
    let d3_hz = run(qec_code::cli::Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export(
                "surface_rotated:d=3".to_owned(),
                CssMatrixKind::Hz,
            )),
        },
    })
    .unwrap();
    let d4_hx = run(qec_code::cli::Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export(
                "surface_rotated:d=4".to_owned(),
                CssMatrixKind::Hx,
            )),
        },
    })
    .unwrap();
    let d4_hz = run(qec_code::cli::Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export(
                "surface_rotated:d=4".to_owned(),
                CssMatrixKind::Hz,
            )),
        },
    })
    .unwrap();

    assert_eq!(d3_hx, fixture_text("surface_rotated_d3_hx.json"));
    assert_eq!(d3_hz, fixture_text("surface_rotated_d3_hz.json"));
    assert_eq!(d4_hx, fixture_text("surface_rotated_d4_hx.json"));
    assert_eq!(d4_hz, fixture_text("surface_rotated_d4_hz.json"));
}

#[test]
fn surface_family_rejects_invalid_dimensions() {
    assert!(construct_css(CssFamilySpec::Surface(SurfaceSpec {
        layout: SurfaceLayout::Rotated,
        row_distance: 1,
        column_distance: 3,
    })
    .into())
    .is_err());

    assert!(construct_css(CssFamilySpec::Surface(SurfaceSpec {
        layout: SurfaceLayout::Unrotated,
        row_distance: 3,
        column_distance: 1,
    })
    .into())
    .is_err());

    assert!(matches!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"surface","layout":"diagonal","row_distance":3,"column_distance":5}"#,
        ),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "surface" && reason.contains("unknown surface layout")
    ));

    assert!(matches!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"surface","distance":3,"layout":"rotated","row_distance":3,"column_distance":3}"#,
        ),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "surface" && reason.contains("conflicting")
    ));

    assert!(matches!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"surface","layout":"rotated","row_distance":18446744073709551616,"column_distance":3}"#,
        ),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "surface" && reason.contains("row_distance")
    ));

    assert!(matches!(
        construct_css(CssFamilySpec::Surface(SurfaceSpec {
            layout: SurfaceLayout::Unrotated,
            row_distance: usize::MAX,
            column_distance: 2,
        })
        .into()),
        Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "surface" && reason.contains("overflow")
    ));
}
