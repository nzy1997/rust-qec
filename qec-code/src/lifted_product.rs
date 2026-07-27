use crate::error::{QecError, Result};
use crate::finite_group::{FiniteGroupSpec, GroupAlgebraElement, left_regular_lift};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiftedProductRingShape {
    pub h_x_rows: usize,
    pub h_z_rows: usize,
    pub num_cols: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiftedProductRingChecks {
    pub shape: LiftedProductRingShape,
    pub h_x: Vec<Vec<GroupAlgebraElement>>,
    pub h_z: Vec<Vec<GroupAlgebraElement>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiftedProductBinaryChecks {
    pub num_cols: usize,
    pub h_x: Vec<Vec<usize>>,
    pub h_z: Vec<Vec<usize>>,
}

pub fn checked_lifted_product_ring_shape(
    left_rows: usize,
    left_cols: usize,
    right_rows: usize,
    right_cols: usize,
) -> Result<LiftedProductRingShape> {
    let h_x_rows = left_rows.checked_mul(right_cols).ok_or(ring_overflow())?;
    let h_z_rows = left_cols.checked_mul(right_rows).ok_or(ring_overflow())?;
    let left_block_cols = left_cols.checked_mul(right_cols).ok_or(ring_overflow())?;
    let right_block_cols = left_rows.checked_mul(right_rows).ok_or(ring_overflow())?;
    let num_cols = left_block_cols
        .checked_add(right_block_cols)
        .ok_or(ring_overflow())?;
    Ok(LiftedProductRingShape {
        h_x_rows,
        h_z_rows,
        num_cols,
    })
}

pub fn checked_lifted_product_binary_shape(
    group: &FiniteGroupSpec,
    left_rows: usize,
    left_cols: usize,
    right_rows: usize,
    right_cols: usize,
) -> Result<LiftedProductRingShape> {
    let shape = checked_lifted_product_ring_shape(left_rows, left_cols, right_rows, right_cols)?;
    for value in [shape.h_x_rows, shape.h_z_rows, shape.num_cols] {
        value
            .checked_mul(group.order())
            .ok_or(QecError::GroupAlgebraDimensionOverflow {
                operation: "lifted product binary shape",
            })?;
    }
    Ok(shape)
}

pub fn lifted_product_ring_checks(
    group: &FiniteGroupSpec,
    left: &[Vec<GroupAlgebraElement>],
    right: &[Vec<GroupAlgebraElement>],
) -> Result<LiftedProductRingChecks> {
    let (left_rows, left_cols) = group_algebra_matrix_shape(left)?;
    let (right_rows, right_cols) = group_algebra_matrix_shape(right)?;
    validate_group_orders(group, left)?;
    validate_group_orders(group, right)?;
    let shape = checked_lifted_product_ring_shape(left_rows, left_cols, right_rows, right_cols)?;

    let h_x = hconcat(
        &matrix_kron_identity(group, left, right_cols)?,
        &identity_kron_matrix(group, left_rows, &inverse_transpose(group, right)?)?,
    )?;
    let h_z = hconcat(
        &identity_kron_matrix(group, left_cols, right)?,
        &matrix_kron_identity(group, &inverse_transpose(group, left)?, right_rows)?,
    )?;
    debug_assert_eq!(h_x.len(), shape.h_x_rows);
    debug_assert_eq!(h_z.len(), shape.h_z_rows);
    debug_assert!(h_x.iter().all(|row| row.len() == shape.num_cols));
    debug_assert!(h_z.iter().all(|row| row.len() == shape.num_cols));
    Ok(LiftedProductRingChecks { shape, h_x, h_z })
}

pub fn lifted_product_binary_checks(
    group: &FiniteGroupSpec,
    left: &[Vec<GroupAlgebraElement>],
    right: &[Vec<GroupAlgebraElement>],
) -> Result<LiftedProductBinaryChecks> {
    let ring = lifted_product_ring_checks(group, left, right)?;
    checked_lifted_product_binary_shape(
        group,
        left.len(),
        left.first().map_or(0, Vec::len),
        right.len(),
        right.first().map_or(0, Vec::len),
    )?;
    let h_x = left_regular_lift(group, &ring.h_x)?;
    let h_z = left_regular_lift(group, &ring.h_z)?;
    debug_assert_eq!(h_x.num_cols(), h_z.num_cols());
    Ok(LiftedProductBinaryChecks {
        num_cols: h_x.num_cols(),
        h_x: h_x.rows().to_vec(),
        h_z: h_z.rows().to_vec(),
    })
}

fn ring_overflow() -> QecError {
    QecError::GroupAlgebraDimensionOverflow {
        operation: "lifted product ring shape",
    }
}

fn group_algebra_matrix_shape(matrix: &[Vec<GroupAlgebraElement>]) -> Result<(usize, usize)> {
    let Some(first_row) = matrix.first() else {
        return Err(invalid_protograph("must contain at least one row"));
    };
    if first_row.is_empty() {
        return Err(invalid_protograph("must contain at least one column"));
    }
    for row in matrix {
        if row.len() != first_row.len() {
            return Err(QecError::GroupAlgebraMatrixRowWidthMismatch {
                expected: first_row.len(),
                actual: row.len(),
            });
        }
    }
    Ok((matrix.len(), first_row.len()))
}

fn validate_group_orders(
    group: &FiniteGroupSpec,
    matrix: &[Vec<GroupAlgebraElement>],
) -> Result<()> {
    for row in matrix {
        for element in row {
            if element.group_order() != group.order() {
                return Err(QecError::GroupAlgebraOrderMismatch {
                    expected: group.order(),
                    actual: element.group_order(),
                });
            }
        }
    }
    Ok(())
}

fn invalid_protograph(reason: &str) -> QecError {
    QecError::InvalidCssConstruction {
        construction: "lifted_product".to_owned(),
        reason: reason.to_owned(),
    }
}

fn inverse_transpose(
    group: &FiniteGroupSpec,
    matrix: &[Vec<GroupAlgebraElement>],
) -> Result<Vec<Vec<GroupAlgebraElement>>> {
    let rows = matrix.len();
    let cols = matrix[0].len();
    let mut output = Vec::with_capacity(cols);
    for col in 0..cols {
        let mut row = Vec::with_capacity(rows);
        for source_row in matrix {
            let support = source_row[col]
                .support()
                .iter()
                .map(|&element| group.inverse(element))
                .collect::<Result<Vec<_>>>()?;
            row.push(GroupAlgebraElement::new(group, support)?);
        }
        output.push(row);
    }
    Ok(output)
}

fn matrix_kron_identity(
    group: &FiniteGroupSpec,
    matrix: &[Vec<GroupAlgebraElement>],
    identity_size: usize,
) -> Result<Vec<Vec<GroupAlgebraElement>>> {
    let zero = GroupAlgebraElement::new(group, Vec::new())?;
    let mut output = Vec::with_capacity(matrix.len() * identity_size);
    for source_row in matrix {
        for diagonal in 0..identity_size {
            let mut row = Vec::with_capacity(source_row.len() * identity_size);
            for element in source_row {
                for column in 0..identity_size {
                    row.push(if column == diagonal {
                        element.clone()
                    } else {
                        zero.clone()
                    });
                }
            }
            output.push(row);
        }
    }
    Ok(output)
}

fn identity_kron_matrix(
    group: &FiniteGroupSpec,
    identity_size: usize,
    matrix: &[Vec<GroupAlgebraElement>],
) -> Result<Vec<Vec<GroupAlgebraElement>>> {
    let zero = GroupAlgebraElement::new(group, Vec::new())?;
    let matrix_cols = matrix[0].len();
    let mut output = Vec::with_capacity(identity_size * matrix.len());
    for diagonal in 0..identity_size {
        for source_row in matrix {
            let mut row = Vec::with_capacity(identity_size * matrix_cols);
            for block in 0..identity_size {
                if block == diagonal {
                    row.extend(source_row.iter().cloned());
                } else {
                    row.extend(std::iter::repeat_n(zero.clone(), matrix_cols));
                }
            }
            output.push(row);
        }
    }
    Ok(output)
}

fn hconcat(
    left: &[Vec<GroupAlgebraElement>],
    right: &[Vec<GroupAlgebraElement>],
) -> Result<Vec<Vec<GroupAlgebraElement>>> {
    if left.len() != right.len() {
        return Err(invalid_protograph("internal lifted-product row mismatch"));
    }
    Ok(left
        .iter()
        .zip(right)
        .map(|(left_row, right_row)| {
            let mut row = left_row.clone();
            row.extend(right_row.iter().cloned());
            row
        })
        .collect())
}
