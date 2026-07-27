use std::ffi::OsString;
use std::path::Path;

use clap::Parser;
use qec_code::QecError;
use qec_code::binary::try_in_row_span;
use qec_code::cli::{Cli, run};
use qec_code::family_contract::{
    CssClassicalCheckSpec, CssConstructionSpec, FiniteGroupTableSpec, GroupAlgebraElementSpec,
    GroupAlgebraProtographSpec, HypergraphProductSpec, LiftedProductSpec, construct_css,
    parse_css_construction_json, verify_css_orthogonality,
};
use qec_code::finite_group::{FiniteGroupSpec, GroupAlgebraElement};
use qec_code::lifted_product::{
    checked_lifted_product_binary_shape, lifted_product_binary_checks, lifted_product_ring_checks,
};
use tempfile::tempdir;

fn c3_group_spec() -> FiniteGroupTableSpec {
    FiniteGroupTableSpec {
        order: 3,
        identity: 0,
        multiplication_table: vec![vec![0, 1, 2], vec![1, 2, 0], vec![2, 0, 1]],
    }
}

fn trivial_group_spec() -> FiniteGroupTableSpec {
    FiniteGroupTableSpec {
        order: 1,
        identity: 0,
        multiplication_table: vec![vec![0]],
    }
}

fn s3_group_spec() -> FiniteGroupTableSpec {
    let elements = vec![
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let mut multiplication_table = Vec::new();
    for left in &elements {
        let mut row = Vec::new();
        for right in &elements {
            let product = [left[right[0]], left[right[1]], left[right[2]]];
            row.push(
                elements
                    .iter()
                    .position(|candidate| *candidate == product)
                    .expect("S3 product should be in fixture list"),
            );
        }
        multiplication_table.push(row);
    }

    FiniteGroupTableSpec {
        order: elements.len(),
        identity: 0,
        multiplication_table,
    }
}

fn ga(support: &[usize]) -> GroupAlgebraElementSpec {
    GroupAlgebraElementSpec {
        support: support.to_vec(),
    }
}

fn c3_protograph() -> GroupAlgebraProtographSpec {
    GroupAlgebraProtographSpec {
        rows: vec![
            vec![ga(&[1, 2]), ga(&[0]), ga(&[])],
            vec![ga(&[]), ga(&[0, 1]), ga(&[1])],
        ],
    }
}

fn trivial_2x3_protograph() -> GroupAlgebraProtographSpec {
    GroupAlgebraProtographSpec {
        rows: vec![
            vec![ga(&[0]), ga(&[0]), ga(&[])],
            vec![ga(&[]), ga(&[0]), ga(&[0])],
        ],
    }
}

fn singleton_protograph(support: usize) -> GroupAlgebraProtographSpec {
    GroupAlgebraProtographSpec {
        rows: vec![vec![ga(&[support])]],
    }
}

fn empty_square_protograph(size: usize) -> GroupAlgebraProtographSpec {
    GroupAlgebraProtographSpec {
        rows: vec![vec![ga(&[]); size]; size],
    }
}

fn c3_spec() -> CssConstructionSpec {
    CssConstructionSpec::LiftedProduct(LiftedProductSpec {
        group: c3_group_spec(),
        left: c3_protograph(),
        right: c3_protograph(),
    })
}

fn c3_group_and_matrix() -> (FiniteGroupSpec, Vec<Vec<GroupAlgebraElement>>) {
    let group =
        FiniteGroupSpec::new(3, 0, vec![vec![0, 1, 2], vec![1, 2, 0], vec![2, 0, 1]]).unwrap();
    let matrix = vec![
        vec![
            GroupAlgebraElement::new(&group, vec![1, 2]).unwrap(),
            GroupAlgebraElement::new(&group, vec![0]).unwrap(),
            GroupAlgebraElement::new(&group, vec![]).unwrap(),
        ],
        vec![
            GroupAlgebraElement::new(&group, vec![]).unwrap(),
            GroupAlgebraElement::new(&group, vec![0, 1]).unwrap(),
            GroupAlgebraElement::new(&group, vec![1]).unwrap(),
        ],
    ];
    (group, matrix)
}

fn cyclic_group(order: usize) -> FiniteGroupSpec {
    let multiplication_table = (0..order)
        .map(|left| {
            (0..order)
                .map(|right| (left + right) % order)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    FiniteGroupSpec::new(order, 0, multiplication_table).unwrap()
}

fn s3_group() -> FiniteGroupSpec {
    let spec = s3_group_spec();
    FiniteGroupSpec::new(spec.order, spec.identity, spec.multiplication_table).unwrap()
}

fn s3_element(group: &FiniteGroupSpec, support: &[usize]) -> GroupAlgebraElement {
    GroupAlgebraElement::new(group, support.to_vec()).unwrap()
}

fn support_rows(matrix: &[Vec<GroupAlgebraElement>]) -> Vec<Vec<Vec<usize>>> {
    matrix
        .iter()
        .map(|row| row.iter().map(|entry| entry.support().to_vec()).collect())
        .collect()
}

fn expected_hx() -> Vec<Vec<usize>> {
    vec![
        vec![1, 2, 9, 28, 29],
        vec![0, 2, 10, 27, 29],
        vec![0, 1, 11, 27, 28],
        vec![4, 5, 12, 27, 30, 31],
        vec![3, 5, 13, 28, 31, 32],
        vec![3, 4, 14, 29, 30, 32],
        vec![7, 8, 15, 31],
        vec![6, 8, 16, 32],
        vec![6, 7, 17, 30],
        vec![9, 11, 20, 34, 35],
        vec![9, 10, 18, 33, 35],
        vec![10, 11, 19, 33, 34],
        vec![12, 14, 23, 33, 36, 37],
        vec![12, 13, 21, 34, 37, 38],
        vec![13, 14, 22, 35, 36, 38],
        vec![15, 17, 26, 37],
        vec![15, 16, 24, 38],
        vec![16, 17, 25, 36],
    ]
}

fn expected_hz() -> Vec<Vec<usize>> {
    vec![
        vec![1, 2, 3, 28, 29],
        vec![0, 2, 4, 27, 29],
        vec![0, 1, 5, 27, 28],
        vec![3, 5, 8, 31, 32],
        vec![3, 4, 6, 30, 32],
        vec![4, 5, 7, 30, 31],
        vec![10, 11, 12, 27, 33, 34],
        vec![9, 11, 13, 28, 34, 35],
        vec![9, 10, 14, 29, 33, 35],
        vec![12, 14, 17, 30, 36, 37],
        vec![12, 13, 15, 31, 37, 38],
        vec![13, 14, 16, 32, 36, 38],
        vec![19, 20, 21, 34],
        vec![18, 20, 22, 35],
        vec![18, 19, 23, 33],
        vec![21, 23, 26, 37],
        vec![21, 22, 24, 38],
        vec![22, 23, 25, 36],
    ]
}

fn dense_rows(n: usize, rows: &[Vec<usize>]) -> Vec<Vec<u8>> {
    rows.iter()
        .map(|row| {
            let mut dense = vec![0; n];
            for &column in row {
                dense[column] = 1;
            }
            dense
        })
        .collect()
}

fn has_component_logical(
    candidate: &[u8],
    kernel_checks: &[Vec<u8>],
    stabilizers: &[Vec<u8>],
) -> bool {
    kernel_checks.iter().all(|check| {
        check
            .iter()
            .zip(candidate)
            .fold(0_u8, |parity, (&entry, &bit)| parity ^ (entry & bit))
            == 0
    }) && !try_in_row_span(stabilizers, candidate).expect("fixture rows should be binary")
}

fn search_supports(
    n: usize,
    weight: usize,
    next: usize,
    candidate: &mut [u8],
    kernel_checks: &[Vec<u8>],
    stabilizers: &[Vec<u8>],
) -> bool {
    if weight == 0 {
        return has_component_logical(candidate, kernel_checks, stabilizers);
    }
    for column in next..=n - weight {
        candidate[column] = 1;
        if search_supports(
            n,
            weight - 1,
            column + 1,
            candidate,
            kernel_checks,
            stabilizers,
        ) {
            return true;
        }
        candidate[column] = 0;
    }
    false
}

fn exact_component_distance(
    n: usize,
    kernel_checks: &[Vec<usize>],
    stabilizers: &[Vec<usize>],
    maximum_distance: usize,
) -> usize {
    let kernel_checks = dense_rows(n, kernel_checks);
    let stabilizers = dense_rows(n, stabilizers);
    let mut candidate = vec![0; n];
    for weight in 1..=maximum_distance {
        if search_supports(n, weight, 0, &mut candidate, &kernel_checks, &stabilizers) {
            return weight;
        }
    }
    panic!("no component logical support found up to distance {maximum_distance}");
}

fn construct_cli_output(spec_path: &Path, output: &str) -> String {
    run(Cli::parse_from([
        OsString::from("qec-code"),
        OsString::from("code"),
        OsString::from("css"),
        OsString::from("construct"),
        OsString::from("--spec"),
        spec_path.as_os_str().to_owned(),
        OsString::from(output),
    ]))
    .unwrap()
}

#[test]
fn lifted_product_c3_matches_fixture() {
    let (group, matrix) = c3_group_and_matrix();
    let ring = lifted_product_ring_checks(&group, &matrix, &matrix).unwrap();
    assert_eq!(ring.shape.h_x_rows, 6);
    assert_eq!(ring.shape.h_z_rows, 6);
    assert_eq!(ring.shape.num_cols, 13);
    assert_eq!(ring.h_x.len(), 6);
    assert_eq!(ring.h_z.len(), 6);
    assert!(ring.h_x.iter().all(|row| row.len() == 13));
    assert!(ring.h_z.iter().all(|row| row.len() == 13));
    let result = construct_css(c3_spec()).unwrap();
    assert_eq!(result.construction_id, "lifted_product");
    assert_eq!(result.stats.n, 39);
    assert_eq!(result.stats.m_x, 18);
    assert_eq!(result.stats.m_z, 18);
    assert_eq!(result.stats.rank_x, 18);
    assert_eq!(result.stats.rank_z, 18);
    assert_eq!(result.stats.k, 3);
    assert_eq!(result.stats.d_x, Some(3));
    assert_eq!(result.stats.d_z, Some(3));
    assert_eq!(result.checks.h_x, expected_hx());
    assert_eq!(result.checks.h_z, expected_hz());
    assert_eq!(result.checks.h_x[0], vec![1, 2, 9, 28, 29]);
    assert_eq!(result.checks.h_z[0], vec![1, 2, 3, 28, 29]);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();
    assert_eq!(
        exact_component_distance(result.stats.n, &result.checks.h_z, &result.checks.h_x, 3),
        3
    );
    assert_eq!(
        exact_component_distance(result.stats.n, &result.checks.h_x, &result.checks.h_z, 3),
        3
    );
    let request = serde_json::json!({"schema_version": 1, "construction": "lifted_product", "group": c3_group_spec(), "left": c3_protograph(), "right": c3_protograph()});
    let parsed = parse_css_construction_json(&serde_json::to_string(&request).unwrap()).unwrap();
    assert_eq!(construct_css(parsed).unwrap().checks, result.checks);
    let dir = tempdir().unwrap();
    let spec_path = dir.path().join("lifted_product.json");
    std::fs::write(&spec_path, serde_json::to_string(&request).unwrap()).unwrap();
    let hx_json: serde_json::Value =
        serde_json::from_str(&construct_cli_output(&spec_path, "hx")).unwrap();
    let hz_json: serde_json::Value =
        serde_json::from_str(&construct_cli_output(&spec_path, "hz")).unwrap();
    let metadata: serde_json::Value =
        serde_json::from_str(&construct_cli_output(&spec_path, "metadata")).unwrap();
    assert_eq!(hx_json["format"], "sparse_rows");
    assert_eq!(hx_json["num_cols"], 39);
    assert_eq!(hx_json["rows"], serde_json::json!(expected_hx()));
    assert_eq!(hz_json["format"], "sparse_rows");
    assert_eq!(hz_json["num_cols"], 39);
    assert_eq!(hz_json["rows"], serde_json::json!(expected_hz()));
    assert_eq!(metadata["construction_id"], "lifted_product");
    assert_eq!(metadata["stats"]["d_x"], 3);
    assert_eq!(metadata["stats"]["d_z"], 3);
}

#[test]
fn lifted_product_trivial_group_matches_hgp() {
    let left = trivial_2x3_protograph();
    let right = trivial_2x3_protograph();
    let lifted = construct_css(CssConstructionSpec::LiftedProduct(LiftedProductSpec {
        group: trivial_group_spec(),
        left,
        right,
    }))
    .unwrap();
    let hgp = construct_css(CssConstructionSpec::HypergraphProduct(
        HypergraphProductSpec {
            left: CssClassicalCheckSpec {
                num_cols: 3,
                rows: vec![vec![0, 1], vec![1, 2]],
            },
            right: CssClassicalCheckSpec {
                num_cols: 3,
                rows: vec![vec![0, 1], vec![1, 2]],
            },
        },
    ))
    .unwrap();
    assert_eq!(lifted.checks.h_x, hgp.checks.h_x);
    assert_eq!(lifted.checks.h_z, hgp.checks.h_z);
    assert_eq!(lifted.stats.n, hgp.stats.n);
    assert_eq!(lifted.stats.m_x, hgp.stats.m_x);
    assert_eq!(lifted.stats.m_z, hgp.stats.m_z);
    assert_eq!(lifted.stats.rank_x, hgp.stats.rank_x);
    assert_eq!(lifted.stats.rank_z, hgp.stats.rank_z);
    assert_eq!(lifted.stats.k, hgp.stats.k);
    assert_eq!(lifted.stats.d_x, hgp.stats.d_x);
    assert_eq!(lifted.stats.d_z, hgp.stats.d_z);
}

#[test]
fn lifted_product_s3_noncommuting_singletons_are_orthogonal() {
    let result = construct_css(CssConstructionSpec::LiftedProduct(LiftedProductSpec {
        group: s3_group_spec(),
        left: singleton_protograph(1),
        right: singleton_protograph(2),
    }))
    .unwrap();

    assert_eq!(result.stats.n, 12);
    assert_eq!(result.stats.m_x, 6);
    assert_eq!(result.stats.m_z, 6);
    assert_eq!(result.checks.h_x[0], vec![1, 8]);
    assert_eq!(result.checks.h_z[0], vec![2, 7]);
    verify_css_orthogonality(result.stats.n, &result.checks.h_x, &result.checks.h_z).unwrap();
}

#[test]
fn lifted_product_s3_rectangular_multi_support_fixture_is_orthogonal() {
    let group = s3_group();
    let left = vec![vec![s3_element(&group, &[1, 2]), s3_element(&group, &[3])]];
    let right = vec![
        vec![s3_element(&group, &[2, 3])],
        vec![s3_element(&group, &[1])],
    ];
    let checks = lifted_product_binary_checks(&group, &left, &right).unwrap();

    assert_eq!(checks.num_cols, 24);
    assert_eq!(checks.h_x.len(), 6);
    assert_eq!(checks.h_z.len(), 24);
    assert_eq!(
        checks.h_x,
        vec![
            vec![1, 2, 10, 14, 15, 19],
            vec![0, 3, 11, 16, 17, 18],
            vec![0, 4, 7, 12, 13, 21],
            vec![1, 5, 6, 16, 17, 20],
            vec![2, 5, 9, 12, 13, 23],
            vec![3, 4, 8, 14, 15, 22],
        ]
    );
    assert_eq!(
        checks.h_z,
        vec![
            vec![2, 4, 13, 14],
            vec![2, 4, 12, 15],
            vec![0, 5, 12, 16],
            vec![0, 5, 13, 17],
            vec![1, 3, 14, 17],
            vec![1, 3, 15, 16],
            vec![1, 19, 20],
            vec![0, 18, 21],
            vec![3, 18, 22],
            vec![2, 19, 23],
            vec![5, 20, 23],
            vec![4, 21, 22],
            vec![8, 10, 15],
            vec![8, 10, 14],
            vec![6, 11, 17],
            vec![6, 11, 16],
            vec![7, 9, 12],
            vec![7, 9, 13],
            vec![7, 21],
            vec![6, 20],
            vec![9, 23],
            vec![8, 22],
            vec![11, 18],
            vec![10, 19],
        ]
    );
    verify_css_orthogonality(checks.num_cols, &checks.h_x, &checks.h_z).unwrap();
}

#[test]
fn lifted_product_s3_ring_fixture_applies_inverse_transpose() {
    let group = s3_group();
    let left = vec![vec![s3_element(&group, &[1, 2]), s3_element(&group, &[3])]];
    let right = vec![
        vec![s3_element(&group, &[2, 3])],
        vec![s3_element(&group, &[1])],
    ];
    let ring = lifted_product_ring_checks(&group, &left, &right).unwrap();

    assert_eq!(ring.shape.h_x_rows, 1);
    assert_eq!(ring.shape.h_z_rows, 4);
    assert_eq!(ring.shape.num_cols, 4);
    assert_eq!(
        support_rows(&ring.h_x),
        vec![vec![vec![1, 2], vec![3], vec![2, 4], vec![1]]]
    );
    assert_eq!(
        support_rows(&ring.h_z),
        vec![
            vec![vec![2, 3], vec![], vec![1, 2], vec![]],
            vec![vec![1], vec![], vec![], vec![1, 2]],
            vec![vec![], vec![2, 3], vec![4], vec![]],
            vec![vec![], vec![1], vec![], vec![4]],
        ]
    );
}

#[test]
fn lifted_product_rejects_post_lift_binary_overflow() {
    let group = cyclic_group(256);
    let zero = GroupAlgebraElement::new(&group, Vec::new()).unwrap();
    let matrix = vec![vec![zero; 18]; 18];

    let Err(err) = lifted_product_binary_checks(&group, &matrix, &matrix) else {
        panic!("expected post-lift binary size rejection");
    };
    assert!(
        matches!(&err, QecError::InvalidCssConstruction { construction, reason }
            if construction == "lifted_product"
                && reason.contains("binary H_X cell count")
                && reason.contains("exceeds maximum supported")),
        "expected post-lift binary size rejection, got {err:?}"
    );
}

#[test]
fn lifted_product_rejects_malformed_protographs() {
    let (group, matrix) = c3_group_and_matrix();
    let empty: Vec<Vec<GroupAlgebraElement>> = Vec::new();
    let Err(empty_err) = lifted_product_ring_checks(&group, &empty, &matrix) else {
        panic!("expected empty lifted-product protograph rejection");
    };
    assert!(
        matches!(&empty_err, QecError::InvalidCssConstruction { construction, reason }
            if construction == "lifted_product"
                && reason == "must contain at least one row"),
        "expected empty protograph rejection, got {empty_err:?}"
    );

    let no_columns = vec![Vec::new()];
    let Err(no_columns_err) = lifted_product_ring_checks(&group, &no_columns, &matrix) else {
        panic!("expected zero-column lifted-product protograph rejection");
    };
    assert!(
        matches!(&no_columns_err, QecError::InvalidCssConstruction { construction, reason }
            if construction == "lifted_product"
                && reason == "must contain at least one column"),
        "expected zero-column protograph rejection, got {no_columns_err:?}"
    );

    let other_group = cyclic_group(2);
    let mismatched = vec![vec![
        GroupAlgebraElement::new(&other_group, vec![0]).unwrap(),
    ]];
    assert_eq!(
        lifted_product_ring_checks(&group, &mismatched, &matrix),
        Err(QecError::GroupAlgebraOrderMismatch {
            expected: 3,
            actual: 2
        })
    );

    let malformed_json = serde_json::json!({
        "schema_version": 1,
        "construction": "lifted_product",
        "group": c3_group_spec(),
        "left": c3_protograph()
    });
    let parse_err = parse_css_construction_json(&serde_json::to_string(&malformed_json).unwrap());
    assert!(
        matches!(&parse_err, Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "lifted_product"
                && reason.contains("missing field `right`")),
        "expected lifted-product JSON parse rejection, got {parse_err:?}"
    );

    let out_of_range = construct_css(CssConstructionSpec::LiftedProduct(LiftedProductSpec {
        group: c3_group_spec(),
        left: GroupAlgebraProtographSpec {
            rows: vec![vec![ga(&[3])]],
        },
        right: c3_protograph(),
    }));
    assert_eq!(
        out_of_range,
        Err(QecError::InvalidGroupAlgebraElementSupport {
            support: 3,
            order: 3
        })
    );
    let ragged = construct_css(CssConstructionSpec::LiftedProduct(LiftedProductSpec {
        group: c3_group_spec(),
        left: GroupAlgebraProtographSpec {
            rows: vec![vec![ga(&[0])], vec![]],
        },
        right: c3_protograph(),
    }));
    assert_eq!(
        ragged,
        Err(QecError::GroupAlgebraMatrixRowWidthMismatch {
            expected: 1,
            actual: 0
        })
    );
    let missing_inverse = construct_css(CssConstructionSpec::LiftedProduct(LiftedProductSpec {
        group: FiniteGroupTableSpec {
            order: 2,
            identity: 0,
            multiplication_table: vec![vec![0, 1], vec![1, 1]],
        },
        left: c3_protograph(),
        right: c3_protograph(),
    }));
    assert!(
        matches!(missing_inverse, Err(QecError::InvalidFiniteGroupTable { reason }) if reason.contains("element 1 has no two-sided inverse"))
    );
    assert_eq!(
        checked_lifted_product_binary_shape(
            &FiniteGroupSpec::new(3, 0, vec![vec![0, 1, 2], vec![1, 2, 0], vec![2, 0, 1]]).unwrap(),
            usize::MAX,
            1,
            1,
            1
        ),
        Err(QecError::GroupAlgebraDimensionOverflow {
            operation: "lifted product ring shape"
        })
    );

    let oversized = construct_css(CssConstructionSpec::LiftedProduct(LiftedProductSpec {
        group: trivial_group_spec(),
        left: empty_square_protograph(32),
        right: empty_square_protograph(32),
    }));
    assert!(
        matches!(&oversized, Err(QecError::InvalidCssConstruction { construction, reason })
            if construction == "lifted_product"
                && reason.contains("ring cell count")
                && reason.contains("exceeds maximum supported")),
        "expected public lifted-product construction to reject oversized input, got {oversized:?}"
    );
}
