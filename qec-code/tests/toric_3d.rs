use std::path::PathBuf;
use std::process::Command;

use qec_code::QecError;
use qec_code::binary_chain_complex::{BinaryBoundaryMap, BinaryChainComplex};
use qec_code::codes::toric_3d::{
    Toric3dDistances, Toric3dSpec, toric_3d_chain_complex, toric_3d_css_checks,
};
use qec_code::css::SparseRowsMatrix;
use qec_code::family_contract::{
    CssFamilySpec, RequestedFamilyId, construct_css, parse_css_construction_json,
    verify_css_orthogonality,
};
use tempfile::tempdir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_rows(name: &str) -> Vec<Vec<usize>> {
    let path = workspace_root().join("tests/fixtures/css").join(name);
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("fixture should be readable"))
            .expect("fixture should be valid JSON");
    serde_json::from_value(value["rows"].clone()).expect("fixture rows should be arrays")
}

fn fixture_text(name: &str) -> String {
    let path = workspace_root().join("tests/fixtures/css").join(name);
    std::fs::read_to_string(path)
        .expect("fixture should be readable")
        .trim_end_matches('\n')
        .to_owned()
}

fn assert_all_row_weights(rows: &[Vec<usize>], weight: usize) {
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.len(), weight, "row {index} had support {row:?}");
    }
}

#[test]
fn toric_3d_3x3x3_matches_fixture() {
    let spec = Toric3dSpec {
        lx: 3,
        ly: 3,
        lz: 3,
    };
    let checks = toric_3d_css_checks(spec).unwrap();
    assert_eq!(checks.num_cols, 81);
    assert_eq!(
        checks.distances,
        Toric3dDistances {
            d_x: 9,
            d_z: 3,
            distance: 3
        }
    );
    assert_eq!(checks.hx[0], vec![0, 18, 27, 33, 54, 56]);
    assert_eq!(checks.hz[0], vec![0, 3, 27, 36]);
    assert_eq!(checks.hz[27], vec![0, 1, 54, 63]);
    assert_eq!(checks.hz[54], vec![27, 28, 54, 57]);
    assert_eq!(checks.hx, fixture_rows("toric_3d_3x3x3_hx.json"));
    assert_eq!(checks.hz, fixture_rows("toric_3d_3x3x3_hz.json"));
    assert_all_row_weights(&checks.hx, 6);
    assert_all_row_weights(&checks.hz, 4);
    verify_css_orthogonality(checks.num_cols, &checks.hx, &checks.hz).unwrap();

    let result = construct_css(CssFamilySpec::Toric3d(spec).into()).unwrap();
    assert_eq!(result.construction_id, "toric_3d");
    assert_eq!(result.requested_family_id, Some(RequestedFamilyId::Toric3d));
    assert_eq!(result.normalized_parameters["lx"], serde_json::json!(3));
    assert_eq!(result.normalized_parameters["ly"], serde_json::json!(3));
    assert_eq!(result.normalized_parameters["lz"], serde_json::json!(3));
    assert_eq!(result.stats.n, 81);
    assert_eq!(result.stats.m_x, 27);
    assert_eq!(result.stats.m_z, 81);
    assert_eq!(result.stats.rank_x, 26);
    assert_eq!(result.stats.rank_z, 52);
    assert_eq!(result.stats.k, 3);
    assert_eq!(result.stats.d_x, Some(9));
    assert_eq!(result.stats.d_z, Some(3));
    assert_eq!(result.checks.h_x, checks.hx);
    assert_eq!(result.checks.h_z, checks.hz);

    let hx_json = SparseRowsMatrix::new(result.stats.n, result.checks.h_x)
        .unwrap()
        .to_json_string();
    let hz_json = SparseRowsMatrix::new(result.stats.n, result.checks.h_z)
        .unwrap()
        .to_json_string();
    assert_eq!(hx_json, fixture_text("toric_3d_3x3x3_hx.json"));
    assert_eq!(hz_json, fixture_text("toric_3d_3x3x3_hz.json"));
}

#[test]
fn toric_3d_accepts_rectangular_periods() {
    let spec = Toric3dSpec {
        lx: 3,
        ly: 4,
        lz: 5,
    };
    let checks = toric_3d_css_checks(spec).unwrap();
    assert_eq!(checks.num_cols, 180);
    assert_eq!(checks.hx.len(), 60);
    assert_eq!(checks.hz.len(), 180);
    assert_eq!(
        checks.distances,
        Toric3dDistances {
            d_x: 12,
            d_z: 3,
            distance: 3
        }
    );
    assert_all_row_weights(&checks.hx, 6);
    assert_all_row_weights(&checks.hz, 4);
    verify_css_orthogonality(checks.num_cols, &checks.hx, &checks.hz).unwrap();

    let parsed = parse_css_construction_json(
        r#"{"schema_version":1,"construction":"toric_3d","lx":3,"ly":4,"lz":5}"#,
    )
    .unwrap();
    assert_eq!(parsed, CssFamilySpec::Toric3d(spec).into());
    let result = construct_css(parsed).unwrap();
    assert_eq!(result.stats.n, 180);
    assert_eq!(result.stats.m_x, 60);
    assert_eq!(result.stats.m_z, 180);
    assert_eq!(result.stats.rank_x, 59);
    assert_eq!(result.stats.rank_z, 118);
    assert_eq!(result.stats.k, 3);
    assert_eq!(result.stats.d_x, Some(12));
    assert_eq!(result.stats.d_z, Some(3));
}

#[test]
fn toric_3d_rectangular_periods_work_through_cli() {
    let dir = tempdir().unwrap();
    let spec_path = dir.path().join("toric-3d.json");
    std::fs::write(
        &spec_path,
        r#"{"schema_version":1,"construction":"toric_3d","lx":3,"ly":4,"lz":5}"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_qec-code"))
        .args([
            "code",
            "css",
            "construct",
            "--spec",
            spec_path.to_str().unwrap(),
            "hx",
        ])
        .output()
        .expect("qec-code binary should run");
    assert!(
        output.status.success(),
        "CLI failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["format"], "sparse_rows");
    assert_eq!(value["num_cols"], 180);
    let rows: Vec<Vec<usize>> = serde_json::from_value(value["rows"].clone()).unwrap();
    assert_eq!(rows.len(), 60);
    assert_all_row_weights(&rows, 6);
}

#[test]
fn toric_3d_rejects_corrupted_boundary_composition() {
    let complex = toric_3d_chain_complex(Toric3dSpec {
        lx: 3,
        ly: 3,
        lz: 3,
    })
    .unwrap();
    let boundary_1 = complex.boundary_map(1).unwrap();
    let boundary_2 = complex.boundary_map(2).unwrap();
    let mut corrupted_rows = boundary_2.matrix().rows().to_vec();
    corrupted_rows[0].remove(0);
    let corrupted_matrix = qec_code::sparse_gf2::SparseGf2Matrix::new(
        boundary_2.num_codomain_cells(),
        boundary_2.num_domain_cells(),
        corrupted_rows,
    )
    .unwrap();
    let corrupted_boundary_2 = BinaryBoundaryMap::new(2, 1, corrupted_matrix).unwrap();

    assert!(matches!(
        BinaryChainComplex::new(vec![(*boundary_1).clone(), corrupted_boundary_2]),
        Err(QecError::NonzeroBoundaryComposition {
            lower_dimension: 1,
            upper_dimension: 2,
            ..
        })
    ));
}

#[test]
fn toric_3d_rejects_degenerate_periods() {
    for (spec, parameter) in [
        (
            Toric3dSpec {
                lx: 2,
                ly: 3,
                lz: 3,
            },
            "lx",
        ),
        (
            Toric3dSpec {
                lx: 3,
                ly: 2,
                lz: 3,
            },
            "ly",
        ),
        (
            Toric3dSpec {
                lx: 3,
                ly: 3,
                lz: 2,
            },
            "lz",
        ),
    ] {
        assert_eq!(
            toric_3d_css_checks(spec),
            Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
                family: "toric_3d".to_owned(),
                parameter: parameter.to_owned(),
                value: 2,
            })
        );
    }

    assert!(matches!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"toric_3d","lx":2,"ly":3,"lz":3}"#
        ),
        Err(QecError::OutOfRangeBuiltInCssIntegerParameter { family, parameter, value })
            if family == "toric_3d" && parameter == "lx" && value == 2
    ));
}

#[test]
fn toric_3d_rejects_overflowing_dimensions() {
    assert!(matches!(
        toric_3d_css_checks(Toric3dSpec {
            lx: usize::MAX,
            ly: 3,
            lz: 3,
        }),
        Err(QecError::SparseGf2DimensionOverflow {
            operation: "toric_3d"
        })
    ));

    let capacity_overflow_lx = isize::MAX as usize / std::mem::size_of::<Vec<usize>>() / 9 + 1;
    assert!(matches!(
        toric_3d_css_checks(Toric3dSpec {
            lx: capacity_overflow_lx,
            ly: 3,
            lz: 3,
        }),
        Err(QecError::SparseGf2DimensionOverflow {
            operation: "toric_3d"
        })
    ));
}
