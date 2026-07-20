use qec_code::QecError;
use qec_code::packed_gf2::{KernelWorkspace, PackedRow, ReducedRowSpace};

#[test]
fn external_consumer_can_use_packed_row_operations() {
    let mut left = PackedRow::from_dense(&[1, 0, 1, 1]).unwrap();
    let right = PackedRow::from_dense(&[1, 1, 0, 1]).unwrap();

    assert_eq!(left.width(), 4);
    assert_eq!(left.bit(3).unwrap(), 1);
    assert_eq!(left.weight(), 3);
    assert_eq!(left.dot_parity(&right).unwrap(), 0);
    assert!(matches!(
        left.bit(usize::MAX),
        Err(QecError::RowWidthMismatch {
            expected: 4,
            actual: usize::MAX
        })
    ));

    left.xor_assign(&right).unwrap();
    assert_eq!(left.to_dense(), vec![0, 1, 1, 0]);
    assert_eq!(left.weight(), 2);
    assert!(!left.is_zero());
    assert!(PackedRow::zeros(65).is_zero());
}

#[test]
fn external_consumer_can_reuse_a_reduced_row_space() {
    let rows = vec![vec![1, 0, 1, 0], vec![0, 1, 1, 0]];
    let space = ReducedRowSpace::from_dense_rows(&rows, 4).unwrap();

    assert_eq!(space.width(), 4);
    assert_eq!(space.rank(), 2);
    assert!(space.contains_dense(&[1, 1, 0, 0]).unwrap());
    assert!(!space.contains_dense(&[0, 0, 0, 1]).unwrap());

    let wrong_width = PackedRow::from_dense(&[1, 0]).unwrap();
    assert!(matches!(
        space.contains(&wrong_width),
        Err(QecError::RowWidthMismatch {
            expected: 4,
            actual: 2
        })
    ));
}

#[test]
fn external_consumer_can_reuse_kernel_allocations() {
    let rows = vec![vec![1, 0, 0, 1], vec![0, 1, 1, 0]];
    let mut workspace = KernelWorkspace::new();

    assert_eq!(
        workspace.kernel_basis(&rows, 4, &[0, 1, 2, 3]).unwrap(),
        &[vec![0, 1, 1, 0], vec![1, 0, 0, 1]]
    );
    assert_eq!(
        workspace.kernel_basis(&rows, 4, &[2, 0, 3, 1]).unwrap(),
        &[vec![1, 0, 0, 1], vec![0, 1, 1, 0]]
    );

    for permutation in [&[0, 1, 1, 3][..], &[0, 1, 2][..], &[0, 1, 2, 4][..]] {
        assert!(matches!(
            workspace.kernel_basis(&rows, 4, permutation),
            Err(QecError::InvalidColumnPermutation { .. })
        ));
    }
}
