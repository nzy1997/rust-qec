use qec_code::finite_group::{
    left_regular_lift, right_regular_lift, FiniteGroupSpec, GroupAlgebraElement, LeftRegularLift,
    RightRegularLift, MAX_FINITE_GROUP_ORDER,
};
use qec_code::sparse_gf2::SparseGf2Matrix;
use qec_code::QecError;

fn assert_shape_and_rows(
    matrix: &SparseGf2Matrix,
    num_rows: usize,
    num_cols: usize,
    rows: &[Vec<usize>],
) {
    assert_eq!(matrix.num_rows(), num_rows);
    assert_eq!(matrix.num_cols(), num_cols);
    assert_eq!(matrix.rows(), rows);
}

fn c3_group() -> FiniteGroupSpec {
    FiniteGroupSpec::new(3, 0, vec![vec![0, 1, 2], vec![1, 2, 0], vec![2, 0, 1]]).unwrap()
}

fn c2_group() -> FiniteGroupSpec {
    FiniteGroupSpec::new(2, 0, vec![vec![0, 1], vec![1, 0]]).unwrap()
}

fn ga(group: &FiniteGroupSpec, support: Vec<usize>) -> GroupAlgebraElement {
    GroupAlgebraElement::new(group, support).unwrap()
}

fn s3_group() -> FiniteGroupSpec {
    let elements = vec![
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let mut table = Vec::new();
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
        table.push(row);
    }
    FiniteGroupSpec::new(elements.len(), 0, table).unwrap()
}

fn canonical_sparse_product(left: &SparseGf2Matrix, right: &SparseGf2Matrix) -> Vec<Vec<usize>> {
    assert_eq!(left.num_cols(), right.num_rows());
    left.rows()
        .iter()
        .map(|left_row| {
            let mut row = Vec::new();
            for &middle in left_row {
                row.extend(right.rows()[middle].iter().copied());
            }
            row.sort_unstable();
            let mut canonical = Vec::new();
            let mut index = 0;
            while index < row.len() {
                let support = row[index];
                let mut keep = false;
                while index < row.len() && row[index] == support {
                    keep = !keep;
                    index += 1;
                }
                if keep {
                    canonical.push(support);
                }
            }
            canonical
        })
        .collect()
}

#[test]
fn finite_group_left_lift_matches_c3_fixture() {
    let group = c3_group();
    assert_eq!(group.identity(), 0);
    assert_eq!(
        group.multiplication_table(),
        &[vec![0, 1, 2], vec![1, 2, 0], vec![2, 0, 1]]
    );
    assert_eq!(group.inverse_table(), &[0, 2, 1]);
    assert_eq!(group.multiply(2, 2).unwrap(), 1);
    assert_eq!(group.inverse(2).unwrap(), 1);
    assert_eq!(
        group.to_json_string(),
        r#"{"order":3,"identity":0,"multiplication_table":[[0,1,2],[1,2,0],[2,0,1]]}"#
    );
    assert_eq!(ga(&group, vec![0, 0, 1]).group_order(), 3);
    assert_eq!(
        ga(&group, vec![2, 1, 2, 2]).to_json_string(),
        r#"{"group_order":3,"support":[1,2]}"#
    );
    assert_eq!(ga(&group, vec![2, 1, 2]).support(), &[1]);

    let matrix = vec![
        vec![
            ga(&group, vec![1, 2]),
            ga(&group, vec![0]),
            ga(&group, vec![]),
        ],
        vec![
            ga(&group, vec![]),
            ga(&group, vec![0, 1]),
            ga(&group, vec![1]),
        ],
    ];

    let expected = vec![
        vec![1, 2, 3],
        vec![0, 2, 4],
        vec![0, 1, 5],
        vec![3, 5, 8],
        vec![3, 4, 6],
        vec![4, 5, 7],
    ];

    let typed = LeftRegularLift.lift(&group, &matrix).unwrap();
    assert_shape_and_rows(&typed, 6, 9, &expected);
    assert_eq!(left_regular_lift(&group, &matrix).unwrap(), typed);
}

#[test]
fn left_and_right_regular_s3_actions_commute() {
    let c3 = c3_group();
    let left_c3 = left_regular_lift(&c3, &[vec![ga(&c3, vec![1])]]).unwrap();
    let right_c3 = right_regular_lift(&c3, &[vec![ga(&c3, vec![1])]]).unwrap();
    assert_shape_and_rows(&left_c3, 3, 3, &[vec![2], vec![0], vec![1]]);
    assert_shape_and_rows(&right_c3, 3, 3, &[vec![1], vec![2], vec![0]]);
    assert_ne!(left_c3, right_c3);

    let group = s3_group();
    for g in 0..group.order() {
        for h in 0..group.order() {
            let left = left_regular_lift(&group, &[vec![ga(&group, vec![g])]]).unwrap();
            let right = right_regular_lift(&group, &[vec![ga(&group, vec![h])]]).unwrap();
            assert_eq!(
                canonical_sparse_product(&left, &right),
                canonical_sparse_product(&right, &left),
                "left/right regular actions should commute for g={g}, h={h}"
            );
        }
    }
}

#[test]
fn finite_group_lifts_reject_invalid_tables() {
    assert!(matches!(
        FiniteGroupSpec::new(0, 0, Vec::new()),
        Err(QecError::InvalidFiniteGroupTable { reason })
            if reason.contains("order must be positive")
    ));

    assert!(matches!(
        FiniteGroupSpec::new(2, 2, vec![vec![0, 1], vec![1, 0]]),
        Err(QecError::InvalidFiniteGroupTable { reason })
            if reason.contains("identity 2 is out of range")
    ));

    assert!(matches!(
        FiniteGroupSpec::new(2, 0, vec![vec![0, 1]]),
        Err(QecError::InvalidFiniteGroupTable { reason })
            if reason.contains("expected 2 rows")
    ));

    assert!(matches!(
        FiniteGroupSpec::new(2, 0, vec![vec![0, 1], vec![1]]),
        Err(QecError::InvalidFiniteGroupTable { reason })
            if reason.contains("row 1 has width 1")
    ));

    assert!(matches!(
        FiniteGroupSpec::new(2, 1, vec![vec![0, 1], vec![1, 0]]),
        Err(QecError::InvalidFiniteGroupTable { reason })
            if reason.contains("declared identity 1 does not match")
    ));

    assert!(matches!(
        FiniteGroupSpec::new(2, 0, vec![vec![1, 1], vec![1, 1]]),
        Err(QecError::InvalidFiniteGroupTable { reason })
            if reason.contains("identity")
    ));

    assert!(matches!(
        FiniteGroupSpec::new(2, 0, vec![vec![0, 1], vec![1, 2]]),
        Err(QecError::InvalidFiniteGroupTable { reason })
            if reason.contains("entry at row 1, column 1")
    ));

    let non_associative = vec![
        vec![0, 1, 2, 3],
        vec![1, 0, 1, 2],
        vec![2, 3, 0, 1],
        vec![3, 2, 1, 0],
    ];
    assert!(matches!(
        FiniteGroupSpec::new(4, 0, non_associative),
        Err(QecError::InvalidFiniteGroupTable { reason })
            if reason.contains("associativity failed")
    ));

    let group = c3_group();
    assert_eq!(
        GroupAlgebraElement::new(&group, vec![3]),
        Err(QecError::InvalidGroupAlgebraElementSupport {
            support: 3,
            order: 3
        })
    );

    let no_inverse = vec![vec![0, 1], vec![1, 1]];
    assert!(matches!(
        FiniteGroupSpec::new(2, 0, no_inverse),
        Err(QecError::InvalidFiniteGroupTable { reason })
            if reason.contains("element 1 has no two-sided inverse")
    ));

    let multiple_inverse = vec![vec![0, 1, 2], vec![1, 0, 0], vec![2, 0, 0]];
    assert!(matches!(
        FiniteGroupSpec::new(3, 0, multiple_inverse),
        Err(QecError::InvalidFiniteGroupTable { reason })
            if reason.contains("element 1 has multiple two-sided inverses")
    ));

    let wrong_group_element = GroupAlgebraElement::new(&c2_group(), vec![1]).unwrap();
    assert_eq!(
        left_regular_lift(&group, &[vec![wrong_group_element]]),
        Err(QecError::GroupAlgebraOrderMismatch {
            expected: 3,
            actual: 2
        })
    );

    assert_eq!(
        group.multiply(3, 0),
        Err(QecError::InvalidFiniteGroupElement {
            element: 3,
            order: 3
        })
    );
    assert_eq!(
        left_regular_lift(&group, &[vec![ga(&group, vec![0])], vec![]]),
        Err(QecError::GroupAlgebraMatrixRowWidthMismatch {
            expected: 1,
            actual: 0
        })
    );

    assert_eq!(
        LeftRegularLift.checked_output_shape(&group, usize::MAX, 1),
        Err(QecError::GroupAlgebraDimensionOverflow {
            operation: "regular lift shape",
        })
    );
    assert_eq!(
        RightRegularLift.checked_output_shape(&group, 1, usize::MAX),
        Err(QecError::GroupAlgebraDimensionOverflow {
            operation: "regular lift shape",
        })
    );
}

#[test]
fn finite_group_lifts_reject_group_order_limit_before_allocation() {
    assert_eq!(
        FiniteGroupSpec::new(MAX_FINITE_GROUP_ORDER + 1, 0, Vec::new()),
        Err(QecError::GroupOrderLimitExceeded {
            order: MAX_FINITE_GROUP_ORDER + 1,
            max_order: MAX_FINITE_GROUP_ORDER,
        })
    );
}
