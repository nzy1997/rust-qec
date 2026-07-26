use serde::Serialize;

use crate::error::{QecError, Result};
use crate::sparse_gf2::SparseGf2Matrix;

/// Bounds group-table validation to 65,536 entries and 16,777,216 associativity triples.
pub const MAX_FINITE_GROUP_ORDER: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiniteGroupSpec {
    order: usize,
    identity: usize,
    multiplication_table: Vec<Vec<usize>>,
    inverse_table: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupAlgebraElement {
    group_order: usize,
    support: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeftRegularLift;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RightRegularLift;

impl FiniteGroupSpec {
    pub fn new(
        order: usize,
        identity: usize,
        multiplication_table: Vec<Vec<usize>>,
    ) -> Result<Self> {
        if order > MAX_FINITE_GROUP_ORDER {
            return Err(QecError::GroupOrderLimitExceeded {
                order,
                max_order: MAX_FINITE_GROUP_ORDER,
            });
        }
        if order == 0 {
            return Err(QecError::InvalidFiniteGroupTable {
                reason: "order must be positive".to_owned(),
            });
        }
        if identity >= order {
            return Err(QecError::InvalidFiniteGroupTable {
                reason: format!("identity {identity} is out of range for order {order}"),
            });
        }

        validate_group_table_shape(order, &multiplication_table)?;
        find_unique_table_identity(order, identity, &multiplication_table)?;
        let inverse_table = build_inverse_table(order, identity, &multiplication_table)?;
        validate_associativity(order, &multiplication_table)?;

        Ok(Self {
            order,
            identity,
            multiplication_table,
            inverse_table,
        })
    }

    pub fn order(&self) -> usize {
        self.order
    }

    pub fn identity(&self) -> usize {
        self.identity
    }

    pub fn multiplication_table(&self) -> &[Vec<usize>] {
        &self.multiplication_table
    }

    pub fn inverse_table(&self) -> &[usize] {
        &self.inverse_table
    }

    pub fn multiply(&self, left: usize, right: usize) -> Result<usize> {
        validate_group_element(self.order, left)?;
        validate_group_element(self.order, right)?;
        Ok(self.multiplication_table[left][right])
    }

    pub fn inverse(&self, element: usize) -> Result<usize> {
        validate_group_element(self.order, element)?;
        Ok(self.inverse_table[element])
    }

    pub fn to_json_string(&self) -> String {
        #[derive(Serialize)]
        struct GroupJson<'a> {
            order: usize,
            identity: usize,
            multiplication_table: &'a [Vec<usize>],
        }

        serde_json::to_string(&GroupJson {
            order: self.order,
            identity: self.identity,
            multiplication_table: &self.multiplication_table,
        })
        .expect("finite-group JSON serialization cannot fail")
    }
}

impl GroupAlgebraElement {
    pub fn new(group: &FiniteGroupSpec, support: Vec<usize>) -> Result<Self> {
        Ok(Self {
            group_order: group.order,
            support: canonicalize_support(group.order, support)?,
        })
    }

    pub fn group_order(&self) -> usize {
        self.group_order
    }

    pub fn support(&self) -> &[usize] {
        &self.support
    }

    pub fn to_json_string(&self) -> String {
        #[derive(Serialize)]
        struct GroupAlgebraElementJson<'a> {
            group_order: usize,
            support: &'a [usize],
        }

        serde_json::to_string(&GroupAlgebraElementJson {
            group_order: self.group_order,
            support: &self.support,
        })
        .expect("group-algebra JSON serialization cannot fail")
    }
}

impl LeftRegularLift {
    pub fn checked_output_shape(
        &self,
        group: &FiniteGroupSpec,
        matrix_rows: usize,
        matrix_cols: usize,
    ) -> Result<(usize, usize)> {
        regular_lift_shape(group, matrix_rows, matrix_cols)
    }

    pub fn lift(
        &self,
        group: &FiniteGroupSpec,
        matrix: &[Vec<GroupAlgebraElement>],
    ) -> Result<SparseGf2Matrix> {
        regular_lift(group, matrix, left_action)
    }
}

impl RightRegularLift {
    pub fn checked_output_shape(
        &self,
        group: &FiniteGroupSpec,
        matrix_rows: usize,
        matrix_cols: usize,
    ) -> Result<(usize, usize)> {
        regular_lift_shape(group, matrix_rows, matrix_cols)
    }

    pub fn lift(
        &self,
        group: &FiniteGroupSpec,
        matrix: &[Vec<GroupAlgebraElement>],
    ) -> Result<SparseGf2Matrix> {
        regular_lift(group, matrix, right_action)
    }
}

pub fn left_regular_lift(
    group: &FiniteGroupSpec,
    matrix: &[Vec<GroupAlgebraElement>],
) -> Result<SparseGf2Matrix> {
    LeftRegularLift.lift(group, matrix)
}

pub fn right_regular_lift(
    group: &FiniteGroupSpec,
    matrix: &[Vec<GroupAlgebraElement>],
) -> Result<SparseGf2Matrix> {
    RightRegularLift.lift(group, matrix)
}

fn validate_group_table_shape(order: usize, multiplication_table: &[Vec<usize>]) -> Result<()> {
    if multiplication_table.len() != order {
        return Err(QecError::InvalidFiniteGroupTable {
            reason: format!("expected {order} rows, got {}", multiplication_table.len()),
        });
    }

    for (row_index, row) in multiplication_table.iter().enumerate() {
        if row.len() != order {
            return Err(QecError::InvalidFiniteGroupTable {
                reason: format!("row {row_index} has width {}; expected {order}", row.len()),
            });
        }
        for (column_index, &entry) in row.iter().enumerate() {
            if entry >= order {
                return Err(QecError::InvalidFiniteGroupTable {
                    reason: format!(
                        "entry at row {row_index}, column {column_index} is {entry}; expected < {order}"
                    ),
                });
            }
        }
    }
    Ok(())
}

fn find_unique_table_identity(
    order: usize,
    declared_identity: usize,
    multiplication_table: &[Vec<usize>],
) -> Result<()> {
    let identities = (0..order)
        .filter(|&candidate| {
            (0..order).all(|element| {
                multiplication_table[candidate][element] == element
                    && multiplication_table[element][candidate] == element
            })
        })
        .collect::<Vec<_>>();

    match identities.as_slice() {
        [identity] if *identity == declared_identity => Ok(()),
        [identity] => Err(QecError::InvalidFiniteGroupTable {
            reason: format!(
                "declared identity {declared_identity} does not match table identity {identity}"
            ),
        }),
        [] => Err(QecError::InvalidFiniteGroupTable {
            reason: "table has no two-sided identity".to_owned(),
        }),
        _ => Err(QecError::InvalidFiniteGroupTable {
            reason: "table has multiple two-sided identities".to_owned(),
        }),
    }
}

fn build_inverse_table(
    order: usize,
    identity: usize,
    multiplication_table: &[Vec<usize>],
) -> Result<Vec<usize>> {
    let mut inverse_table = Vec::new();
    inverse_table
        .try_reserve_exact(order)
        .map_err(|_| QecError::InvalidFiniteGroupTable {
            reason: "could not allocate inverse table".to_owned(),
        })?;

    for element in 0..order {
        let inverses = (0..order)
            .filter(|&candidate| {
                multiplication_table[element][candidate] == identity
                    && multiplication_table[candidate][element] == identity
            })
            .collect::<Vec<_>>();
        match inverses.as_slice() {
            [inverse] => inverse_table.push(*inverse),
            [] => {
                return Err(QecError::InvalidFiniteGroupTable {
                    reason: format!("element {element} has no two-sided inverse"),
                });
            }
            _ => {
                return Err(QecError::InvalidFiniteGroupTable {
                    reason: format!("element {element} has multiple two-sided inverses"),
                });
            }
        }
    }
    Ok(inverse_table)
}

fn validate_associativity(order: usize, multiplication_table: &[Vec<usize>]) -> Result<()> {
    for left in 0..order {
        for middle in 0..order {
            for right in 0..order {
                let left_associated =
                    multiplication_table[multiplication_table[left][middle]][right];
                let right_associated =
                    multiplication_table[left][multiplication_table[middle][right]];
                if left_associated != right_associated {
                    return Err(QecError::InvalidFiniteGroupTable {
                        reason: format!(
                            "associativity failed for ({left} * {middle}) * {right} = {left_associated}, {left} * ({middle} * {right}) = {right_associated}"
                        ),
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_group_element(order: usize, element: usize) -> Result<()> {
    if element >= order {
        return Err(QecError::InvalidFiniteGroupElement { element, order });
    }
    Ok(())
}

fn canonicalize_support(order: usize, mut support: Vec<usize>) -> Result<Vec<usize>> {
    for &element in &support {
        if element >= order {
            return Err(QecError::InvalidGroupAlgebraElementSupport {
                support: element,
                order,
            });
        }
    }

    support.sort_unstable();
    let mut canonical = Vec::new();
    let mut index = 0;
    while index < support.len() {
        let element = support[index];
        let mut keep = false;
        while index < support.len() && support[index] == element {
            keep = !keep;
            index += 1;
        }
        if keep {
            canonical.push(element);
        }
    }
    Ok(canonical)
}

fn regular_lift(
    group: &FiniteGroupSpec,
    matrix: &[Vec<GroupAlgebraElement>],
    action: fn(&FiniteGroupSpec, usize, usize) -> Result<usize>,
) -> Result<SparseGf2Matrix> {
    let matrix_rows = matrix.len();
    let matrix_cols = matrix.first().map_or(0, Vec::len);
    for row in matrix {
        if row.len() != matrix_cols {
            return Err(QecError::GroupAlgebraMatrixRowWidthMismatch {
                expected: matrix_cols,
                actual: row.len(),
            });
        }
        for element in row {
            if element.group_order != group.order {
                return Err(QecError::GroupAlgebraOrderMismatch {
                    expected: group.order,
                    actual: element.group_order,
                });
            }
        }
    }

    let (num_rows, num_cols) = regular_lift_shape(group, matrix_rows, matrix_cols)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(num_rows)
        .map_err(|_| QecError::GroupAlgebraDimensionOverflow {
            operation: "regular lift rows",
        })?;

    for row in matrix {
        for x in 0..group.order {
            let mut output_row = Vec::new();
            for (matrix_col, element) in row.iter().enumerate() {
                let block_start = matrix_col.checked_mul(group.order).ok_or(
                    QecError::GroupAlgebraDimensionOverflow {
                        operation: "regular lift column index",
                    },
                )?;
                for &support in &element.support {
                    let acted = action(group, support, x)?;
                    output_row.push(block_start.checked_add(acted).ok_or(
                        QecError::GroupAlgebraDimensionOverflow {
                            operation: "regular lift column index",
                        },
                    )?);
                }
            }
            rows.push(output_row);
        }
    }

    SparseGf2Matrix::new(num_rows, num_cols, rows)
}

fn regular_lift_shape(
    group: &FiniteGroupSpec,
    matrix_rows: usize,
    matrix_cols: usize,
) -> Result<(usize, usize)> {
    let num_rows =
        matrix_rows
            .checked_mul(group.order)
            .ok_or(QecError::GroupAlgebraDimensionOverflow {
                operation: "regular lift shape",
            })?;
    let num_cols =
        matrix_cols
            .checked_mul(group.order)
            .ok_or(QecError::GroupAlgebraDimensionOverflow {
                operation: "regular lift shape",
            })?;
    Ok((num_rows, num_cols))
}

fn left_action(group: &FiniteGroupSpec, element: usize, x: usize) -> Result<usize> {
    group.multiply(group.inverse(element)?, x)
}

fn right_action(group: &FiniteGroupSpec, element: usize, x: usize) -> Result<usize> {
    group.multiply(x, element)
}
