use crate::error::{QecError, Result};

pub(crate) type BinaryRow = Vec<u8>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReducedRows {
    pub rows: Vec<BinaryRow>,
    pub pivot_cols: Vec<usize>,
    pub width: usize,
}

#[derive(Debug, Default)]
pub(crate) struct RandomWindowKernelWorkspace {
    permuted_rows: Vec<BinaryRow>,
    permuted_len: usize,
    pivot_cols: Vec<usize>,
    pivot_seen: Vec<bool>,
    permutation_seen: Vec<bool>,
    basis_rows: Vec<BinaryRow>,
    basis_len: usize,
}

impl RandomWindowKernelWorkspace {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn try_kernel_basis_with_width(
        &mut self,
        matrix: &[BinaryRow],
        width: usize,
        column_permutation: &[usize],
    ) -> Result<&[BinaryRow]> {
        self.reset_logical_state();
        validate_rows_with_width(matrix, width)?;
        validate_column_permutation_with_seen(
            column_permutation,
            width,
            &mut self.permutation_seen,
        )?;

        self.fill_permuted_rows(matrix, width, column_permutation);
        self.reduce_permuted_rows(width);
        self.fill_original_order_basis(width, column_permutation);

        Ok(&self.basis_rows[..self.basis_len])
    }

    fn reset_logical_state(&mut self) {
        self.permuted_len = 0;
        self.basis_len = 0;
        self.pivot_cols.clear();
    }

    fn fill_permuted_rows(
        &mut self,
        matrix: &[BinaryRow],
        width: usize,
        column_permutation: &[usize],
    ) {
        self.permuted_len = matrix.len();
        for (row_index, row) in matrix.iter().enumerate() {
            if row_index == self.permuted_rows.len() {
                self.permuted_rows.push(Vec::new());
            }
            let permuted_row = &mut self.permuted_rows[row_index];
            permuted_row.clear();
            permuted_row.resize(width, 0);
            for (permuted_col, &original_col) in column_permutation.iter().enumerate() {
                permuted_row[permuted_col] = row[original_col];
            }
        }
    }

    fn reduce_permuted_rows(&mut self, width: usize) {
        let rows = &mut self.permuted_rows[..self.permuted_len];
        let mut pivot_row = 0;

        for col in 0..width {
            let Some(pivot) = (pivot_row..rows.len()).find(|&row| rows[row][col] == 1) else {
                continue;
            };
            rows.swap(pivot_row, pivot);

            for row in 0..rows.len() {
                if row != pivot_row && rows[row][col] == 1 {
                    for k in col..width {
                        rows[row][k] ^= rows[pivot_row][k];
                    }
                }
            }

            self.pivot_cols.push(col);
            pivot_row += 1;
            if pivot_row == rows.len() {
                break;
            }
        }
    }

    fn fill_original_order_basis(&mut self, width: usize, column_permutation: &[usize]) {
        self.pivot_seen.clear();
        self.pivot_seen.resize(width, false);
        for &pivot_col in &self.pivot_cols {
            self.pivot_seen[pivot_col] = true;
        }

        let mut basis_len = 0;
        for free_col in 0..width {
            if self.pivot_seen[free_col] {
                continue;
            }
            if basis_len == self.basis_rows.len() {
                self.basis_rows.push(Vec::new());
            }
            let vector = &mut self.basis_rows[basis_len];
            vector.clear();
            vector.resize(width, 0);
            vector[column_permutation[free_col]] = 1;
            for (pivot_row, &pivot_col) in self.pivot_cols.iter().enumerate() {
                if self.permuted_rows[pivot_row][free_col] == 1 {
                    vector[column_permutation[pivot_col]] = 1;
                }
            }
            basis_len += 1;
        }

        self.basis_len = basis_len;
    }
}

pub(crate) fn validate_rows(matrix: &[BinaryRow]) -> Result<usize> {
    let width = matrix.first().map_or(0, Vec::len);
    validate_rows_with_width(matrix, width)?;
    Ok(width)
}

pub(crate) fn validate_rows_with_width(matrix: &[BinaryRow], width: usize) -> Result<()> {
    for (row_index, row) in matrix.iter().enumerate() {
        if row.len() != width {
            return Err(QecError::RowWidthMismatch {
                expected: width,
                actual: row.len(),
            });
        }
        for (col_index, bit) in row.iter().enumerate() {
            if *bit > 1 {
                return Err(QecError::InvalidBinaryEntry {
                    row: row_index,
                    col: col_index,
                    value: *bit,
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_target(target: &[u8]) -> Result<()> {
    for (index, bit) in target.iter().enumerate() {
        if *bit > 1 {
            return Err(QecError::InvalidBinaryEntry {
                row: 0,
                col: index,
                value: *bit,
            });
        }
    }
    Ok(())
}

pub(crate) fn try_row_echelon(matrix: &[BinaryRow]) -> Result<Vec<BinaryRow>> {
    let width = validate_rows(matrix)?;
    let mut rows = matrix.to_vec();
    let mut pivot_row = 0;

    for col in 0..width {
        let Some(pivot) = (pivot_row..rows.len()).find(|&row| rows[row][col] == 1) else {
            continue;
        };
        rows.swap(pivot_row, pivot);

        for row in (pivot_row + 1)..rows.len() {
            if rows[row][col] == 1 {
                for k in col..width {
                    rows[row][k] ^= rows[pivot_row][k];
                }
            }
        }

        pivot_row += 1;
        if pivot_row == rows.len() {
            break;
        }
    }

    Ok(rows)
}

pub(crate) fn try_rref(matrix: &[BinaryRow]) -> Result<ReducedRows> {
    let width = validate_rows(matrix)?;
    try_rref_with_width(matrix, width)
}

pub(crate) fn try_rref_with_width(matrix: &[BinaryRow], width: usize) -> Result<ReducedRows> {
    validate_rows_with_width(matrix, width)?;
    let mut rows = matrix.to_vec();
    let mut pivot_cols = Vec::new();
    let mut pivot_row = 0;

    for col in 0..width {
        let Some(pivot) = (pivot_row..rows.len()).find(|&row| rows[row][col] == 1) else {
            continue;
        };
        rows.swap(pivot_row, pivot);

        for row in 0..rows.len() {
            if row != pivot_row && rows[row][col] == 1 {
                for k in col..width {
                    rows[row][k] ^= rows[pivot_row][k];
                }
            }
        }

        pivot_cols.push(col);
        pivot_row += 1;
        if pivot_row == rows.len() {
            break;
        }
    }

    Ok(ReducedRows {
        rows,
        pivot_cols,
        width,
    })
}

pub(crate) fn try_rank(matrix: &[BinaryRow]) -> Result<usize> {
    Ok(try_rref(matrix)?.pivot_cols.len())
}

pub(crate) fn try_in_row_span(matrix: &[BinaryRow], target: &[u8]) -> Result<bool> {
    let width = validate_rows(matrix)?;
    validate_target(target)?;

    if matrix.is_empty() {
        return Ok(!target.iter().any(|bit| *bit != 0));
    }

    try_in_row_span_with_width(matrix, width, target)
}

pub(crate) fn try_in_row_span_with_width(
    matrix: &[BinaryRow],
    width: usize,
    target: &[u8],
) -> Result<bool> {
    validate_rows_with_width(matrix, width)?;
    validate_target(target)?;

    if target.len() != width {
        return Err(QecError::RowWidthMismatch {
            expected: width,
            actual: target.len(),
        });
    }

    if matrix.is_empty() {
        return Ok(!target.iter().any(|bit| *bit != 0));
    }

    let reduced = try_rref_with_width(matrix, width)?;
    try_in_reduced_row_span(&reduced, target)
}

pub(crate) fn try_in_reduced_row_span(reduced: &ReducedRows, target: &[u8]) -> Result<bool> {
    validate_target(target)?;

    if target.len() != reduced.width {
        return Err(QecError::RowWidthMismatch {
            expected: reduced.width,
            actual: target.len(),
        });
    }

    let mut remainder = target.to_vec();
    for (pivot_row, pivot_col) in reduced.pivot_cols.iter().copied().enumerate() {
        if remainder[pivot_col] == 1 {
            for col in pivot_col..reduced.width {
                remainder[col] ^= reduced.rows[pivot_row][col];
            }
        }
    }

    Ok(!remainder.iter().any(|bit| *bit != 0))
}

pub(crate) fn try_select_independent_rows(matrix: &[BinaryRow]) -> Result<Vec<BinaryRow>> {
    let width = validate_rows(matrix)?;
    let mut basis = Vec::new();

    for row in matrix {
        if !try_in_row_span_with_width(&basis, width, row)? {
            basis.push(row.clone());
        }
    }

    Ok(basis)
}

pub(crate) fn try_nullspace_basis_with_width(
    matrix: &[BinaryRow],
    width: usize,
) -> Result<Vec<BinaryRow>> {
    let reduced = try_rref_with_width(matrix, width)?;
    let width = reduced.width;
    let pivot_cols = reduced.pivot_cols;
    let free_cols = (0..width)
        .filter(|col| !pivot_cols.contains(col))
        .collect::<Vec<_>>();

    if free_cols.is_empty() {
        return Ok(Vec::new());
    }

    let mut basis = Vec::with_capacity(free_cols.len());
    for free_col in free_cols {
        let mut vector = vec![0; width];
        vector[free_col] = 1;
        for (pivot_row, pivot_col) in pivot_cols.iter().copied().enumerate() {
            vector[pivot_col] = reduced.rows[pivot_row][free_col];
        }
        basis.push(vector);
    }

    Ok(basis)
}

pub(crate) fn try_random_window_kernel_basis_with_width(
    matrix: &[BinaryRow],
    width: usize,
    column_permutation: &[usize],
) -> Result<Vec<BinaryRow>> {
    let mut workspace = RandomWindowKernelWorkspace::new();
    let basis = workspace.try_kernel_basis_with_width(matrix, width, column_permutation)?;
    Ok(basis.to_vec())
}

#[allow(dead_code)]
fn validate_column_permutation(column_permutation: &[usize], width: usize) -> Result<()> {
    let mut seen = Vec::new();
    validate_column_permutation_with_seen(column_permutation, width, &mut seen)
}

fn validate_column_permutation_with_seen(
    column_permutation: &[usize],
    width: usize,
    seen: &mut Vec<bool>,
) -> Result<()> {
    if column_permutation.len() != width {
        return Err(QecError::InvalidColumnPermutation {
            reason: format!("expected length {width}, got {}", column_permutation.len()),
        });
    }

    seen.clear();
    seen.resize(width, false);
    for &column in column_permutation {
        if column >= width {
            return Err(QecError::InvalidColumnPermutation {
                reason: format!("column {column} out of range for width {width}"),
            });
        }
        if seen[column] {
            return Err(QecError::InvalidColumnPermutation {
                reason: format!("duplicate column {column}"),
            });
        }
        seen[column] = true;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::error::QecError;

    use super::{
        try_in_reduced_row_span, try_in_row_span_with_width, try_nullspace_basis_with_width,
        try_random_window_kernel_basis_with_width, try_rank, try_select_independent_rows,
        RandomWindowKernelWorkspace,
    };

    fn dot(lhs: &[u8], rhs: &[u8]) -> u8 {
        lhs.iter()
            .zip(rhs)
            .fold(0, |parity, (left, right)| parity ^ (*left & *right))
    }

    #[test]
    fn nullspace_basis_annihilates_every_constraint_row() {
        let matrix = vec![vec![1, 0, 1, 0], vec![0, 1, 1, 0]];
        let basis = try_nullspace_basis_with_width(&matrix, 4).unwrap();

        assert_eq!(basis.len(), 2);
        assert!(basis.iter().all(|row| row.len() == 4));
        for row in &matrix {
            for vector in &basis {
                assert_eq!(dot(row, vector), 0);
            }
        }
    }

    #[test]
    fn select_independent_rows_drops_dependent_generators() {
        let rows = vec![vec![1, 0, 1], vec![0, 1, 1], vec![1, 1, 0]];

        assert_eq!(
            try_select_independent_rows(&rows).unwrap(),
            vec![vec![1, 0, 1], vec![0, 1, 1]]
        );
    }

    #[test]
    fn width_aware_nullspace_basis_for_empty_constraints_spans_full_space() {
        let basis = try_nullspace_basis_with_width(&[], 3).unwrap();

        assert_eq!(basis, vec![vec![1, 0, 0], vec![0, 1, 0], vec![0, 0, 1]]);
    }

    #[test]
    fn width_aware_row_span_path_checks_known_empty_constraint_width() {
        assert_eq!(try_in_row_span_with_width(&[], 3, &[0, 0, 0]), Ok(true));
        assert_eq!(try_in_row_span_with_width(&[], 3, &[1, 0, 0]), Ok(false));
        assert_eq!(
            try_in_row_span_with_width(&[], 3, &[0, 0]),
            Err(QecError::RowWidthMismatch {
                expected: 3,
                actual: 2,
            })
        );
    }

    #[test]
    fn reduced_row_span_membership_reuses_rref() {
        let reduced = super::try_rref_with_width(&[vec![1, 1, 0], vec![0, 1, 1]], 3).unwrap();

        assert_eq!(try_in_reduced_row_span(&reduced, &[1, 0, 1]), Ok(true));
        assert_eq!(try_in_reduced_row_span(&reduced, &[1, 0, 0]), Ok(false));
    }

    #[test]
    fn reduced_row_span_membership_rejects_invalid_targets() {
        let reduced = super::try_rref_with_width(&[vec![1, 1, 0], vec![0, 1, 1]], 3).unwrap();

        assert_eq!(
            try_in_reduced_row_span(&reduced, &[1, 0]),
            Err(QecError::RowWidthMismatch {
                expected: 3,
                actual: 2,
            })
        );
        assert_eq!(
            try_in_reduced_row_span(&reduced, &[1, 2, 0]),
            Err(QecError::InvalidBinaryEntry {
                row: 0,
                col: 1,
                value: 2,
            })
        );
    }

    fn assert_kernel_vector(matrix: &[Vec<u8>], vector: &[u8]) {
        for row in matrix {
            assert_eq!(dot(row, vector), 0);
        }
    }

    #[test]
    fn gf2_random_window_kernel_basis_contract() {
        let matrix = vec![vec![1, 1, 0, 0], vec![0, 1, 1, 0]];
        let permutation = vec![3, 0, 2, 1];

        let basis = try_random_window_kernel_basis_with_width(&matrix, 4, &permutation).unwrap();
        let repeated = try_random_window_kernel_basis_with_width(&matrix, 4, &permutation).unwrap();
        let original_nullspace = try_nullspace_basis_with_width(&matrix, 4).unwrap();

        assert_eq!(basis, vec![vec![0, 0, 0, 1], vec![1, 1, 1, 0]]);
        assert_eq!(basis, repeated);
        assert!(basis.iter().all(|row| row.len() == 4));
        for vector in &basis {
            assert_kernel_vector(&matrix, vector);
        }
        assert_eq!(basis.len(), original_nullspace.len());
        assert_eq!(try_rank(&basis).unwrap(), original_nullspace.len());
    }

    #[test]
    fn gf2_random_window_workspace_matches_existing_kernel_basis() {
        let cases = vec![
            (
                Vec::<Vec<u8>>::new(),
                3,
                vec![vec![0, 1, 2], vec![2, 1, 0], vec![1, 2, 0]],
            ),
            (
                vec![vec![1, 1, 0, 0], vec![0, 1, 1, 0]],
                4,
                vec![vec![0, 1, 2, 3], vec![3, 0, 2, 1], vec![2, 3, 1, 0]],
            ),
            (
                vec![
                    vec![1, 0, 1, 1, 0],
                    vec![0, 1, 1, 0, 1],
                    vec![1, 1, 0, 1, 1],
                ],
                5,
                vec![vec![0, 1, 2, 3, 4], vec![4, 2, 0, 3, 1], vec![1, 3, 4, 0, 2]],
            ),
        ];
        let mut workspace = RandomWindowKernelWorkspace::new();

        for (matrix, width, permutations) in cases {
            for permutation in permutations {
                let expected =
                    try_random_window_kernel_basis_with_width(&matrix, width, &permutation)
                        .unwrap();
                let actual = workspace
                    .try_kernel_basis_with_width(&matrix, width, &permutation)
                    .unwrap()
                    .to_vec();

                assert_eq!(actual, expected, "width {width} permutation {permutation:?}");
                assert!(actual.iter().all(|row| row.len() == width));
                for vector in &actual {
                    assert_kernel_vector(&matrix, vector);
                }
            }
        }
    }

    #[test]
    fn gf2_random_window_workspace_reuse_resets_state() {
        let mut workspace = RandomWindowKernelWorkspace::new();

        let wide = vec![
            vec![1, 0, 1, 0, 1],
            vec![0, 1, 1, 1, 0],
            vec![1, 1, 0, 0, 1],
        ];
        let wide_permutation = vec![4, 2, 0, 3, 1];
        let expected_wide =
            try_random_window_kernel_basis_with_width(&wide, 5, &wide_permutation).unwrap();
        assert_eq!(
            workspace
                .try_kernel_basis_with_width(&wide, 5, &wide_permutation)
                .unwrap(),
            expected_wide.as_slice()
        );

        let narrow = vec![vec![1, 1], vec![0, 0]];
        let narrow_permutation = vec![1, 0];
        let expected_narrow =
            try_random_window_kernel_basis_with_width(&narrow, 2, &narrow_permutation).unwrap();
        let actual_narrow = workspace
            .try_kernel_basis_with_width(&narrow, 2, &narrow_permutation)
            .unwrap()
            .to_vec();
        assert_eq!(actual_narrow, expected_narrow);
        assert!(actual_narrow.iter().all(|row| row.len() == 2));
        for vector in &actual_narrow {
            assert_kernel_vector(&narrow, vector);
        }

        let larger = vec![vec![1, 0, 0, 1], vec![0, 1, 1, 0]];
        let larger_permutation = vec![2, 0, 3, 1];
        let expected_larger =
            try_random_window_kernel_basis_with_width(&larger, 4, &larger_permutation).unwrap();
        let actual_larger = workspace
            .try_kernel_basis_with_width(&larger, 4, &larger_permutation)
            .unwrap()
            .to_vec();
        assert_eq!(actual_larger, expected_larger);
        assert!(actual_larger.iter().all(|row| row.len() == 4));
        for vector in &actual_larger {
            assert_kernel_vector(&larger, vector);
        }
    }

    #[test]
    fn gf2_random_window_workspace_rejects_stale_or_invalid_inputs() {
        let mut workspace = RandomWindowKernelWorkspace::new();
        let previous_wide = vec![vec![1, 0, 1, 0], vec![0, 1, 1, 1]];
        let previous_permutation = vec![3, 0, 2, 1];
        workspace
            .try_kernel_basis_with_width(&previous_wide, 4, &previous_permutation)
            .unwrap();

        let duplicate_permutation = vec![0, 1, 1, 3];
        assert_eq!(
            workspace
                .try_kernel_basis_with_width(&previous_wide, 4, &duplicate_permutation)
                .unwrap_err(),
            try_random_window_kernel_basis_with_width(&previous_wide, 4, &duplicate_permutation)
                .unwrap_err()
        );

        let invalid_binary = vec![vec![1, 2, 0, 0]];
        assert_eq!(
            workspace
                .try_kernel_basis_with_width(&invalid_binary, 4, &[0, 1, 2, 3])
                .unwrap_err(),
            try_random_window_kernel_basis_with_width(&invalid_binary, 4, &[0, 1, 2, 3])
                .unwrap_err()
        );

        let mismatched_width = vec![vec![1, 0, 0, 1], vec![1, 0]];
        assert_eq!(
            workspace
                .try_kernel_basis_with_width(&mismatched_width, 4, &[0, 1, 2, 3])
                .unwrap_err(),
            try_random_window_kernel_basis_with_width(&mismatched_width, 4, &[0, 1, 2, 3])
                .unwrap_err()
        );

        let narrow = vec![vec![1, 1]];
        let narrow_permutation = vec![1, 0];
        let expected_narrow =
            try_random_window_kernel_basis_with_width(&narrow, 2, &narrow_permutation).unwrap();
        let actual_narrow = workspace
            .try_kernel_basis_with_width(&narrow, 2, &narrow_permutation)
            .unwrap()
            .to_vec();
        assert_eq!(actual_narrow, expected_narrow);
        assert!(actual_narrow.iter().all(|row| row.len() == 2));
        for vector in &actual_narrow {
            assert_kernel_vector(&narrow, vector);
        }
    }

    #[test]
    fn gf2_random_window_kernel_basis_rejects_bad_permutation() {
        let matrix = vec![vec![1, 0, 1, 0], vec![0, 1, 1, 0]];
        let error =
            try_random_window_kernel_basis_with_width(&matrix, 4, &[0, 1, 1, 3]).unwrap_err();
        let short_error =
            try_random_window_kernel_basis_with_width(&matrix, 4, &[0, 1, 2]).unwrap_err();
        let out_of_range_error =
            try_random_window_kernel_basis_with_width(&matrix, 4, &[0, 1, 2, 4]).unwrap_err();

        assert_eq!(
            error,
            QecError::InvalidColumnPermutation {
                reason: "duplicate column 1".to_owned(),
            }
        );
        assert!(error.to_string().contains("invalid column permutation"));
        assert_eq!(
            short_error,
            QecError::InvalidColumnPermutation {
                reason: "expected length 4, got 3".to_owned(),
            }
        );
        assert_eq!(
            out_of_range_error,
            QecError::InvalidColumnPermutation {
                reason: "column 4 out of range for width 4".to_owned(),
            }
        );
    }

    #[test]
    fn random_window_kernel_basis_rejects_invalid_matrix_inputs() {
        assert_eq!(
            try_random_window_kernel_basis_with_width(&[vec![1, 2]], 2, &[0, 1]),
            Err(QecError::InvalidBinaryEntry {
                row: 0,
                col: 1,
                value: 2,
            })
        );
        assert_eq!(
            try_random_window_kernel_basis_with_width(&[vec![1, 0], vec![1]], 2, &[0, 1]),
            Err(QecError::RowWidthMismatch {
                expected: 2,
                actual: 1,
            })
        );
    }
}
