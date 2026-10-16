use crate::error::{QecError, Result};

pub(crate) type BinaryRow = Vec<u8>;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct BitPackedRow {
    width: usize,
    words: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReducedRows {
    pub rows: Vec<BinaryRow>,
    pub pivot_cols: Vec<usize>,
    pub width: usize,
}

#[allow(dead_code)]
fn word_count(width: usize) -> usize {
    width.div_ceil(64)
}

#[allow(dead_code)]
fn tail_mask(width: usize) -> u64 {
    if width == 0 {
        return 0;
    }
    let tail_bits = width % 64;
    if tail_bits == 0 {
        u64::MAX
    } else {
        (1u64 << tail_bits) - 1
    }
}

#[allow(dead_code)]
impl BitPackedRow {
    pub(crate) fn try_from_dense(row: &[u8], width: usize) -> Result<Self> {
        validate_target(row)?;
        if row.len() != width {
            return Err(QecError::RowWidthMismatch {
                expected: width,
                actual: row.len(),
            });
        }

        let mut words = vec![0u64; word_count(width)];
        for (index, bit) in row.iter().copied().enumerate() {
            if bit == 1 {
                words[index / 64] |= 1u64 << (index % 64);
            }
        }

        let mut row = Self { width, words };
        row.clear_padding_bits();
        Ok(row)
    }

    pub(crate) fn zeros(width: usize) -> Self {
        Self {
            width,
            words: vec![0; word_count(width)],
        }
    }

    fn reset_zero_width(&mut self, width: usize) {
        self.width = width;
        self.words.clear();
        self.words.resize(word_count(width), 0);
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn try_bit(&self, index: usize) -> Result<u8> {
        if index >= self.width {
            return Err(QecError::RowWidthMismatch {
                expected: self.width,
                actual: index + 1,
            });
        }
        Ok(self.bit(index))
    }

    fn bit(&self, index: usize) -> u8 {
        debug_assert!(index < self.width);
        let word = self.words[index / 64];
        u8::from(((word >> (index % 64)) & 1) == 1)
    }

    fn set_bit(&mut self, index: usize) {
        debug_assert!(index < self.width);
        self.words[index / 64] |= 1u64 << (index % 64);
    }

    pub(crate) fn to_dense(&self) -> Vec<u8> {
        let mut dense = vec![0; self.width];
        for index in 0..self.width {
            let word = self.words[index / 64];
            dense[index] = u8::from(((word >> (index % 64)) & 1) == 1);
        }
        dense
    }

    pub(crate) fn xor_assign(&mut self, rhs: &Self) -> Result<()> {
        if self.width != rhs.width {
            return Err(QecError::RowWidthMismatch {
                expected: self.width,
                actual: rhs.width,
            });
        }

        for (left, right) in self.words.iter_mut().zip(&rhs.words) {
            *left ^= *right;
        }
        self.clear_padding_bits();
        Ok(())
    }

    fn xor_assign_from_col(&mut self, rhs: &Self, start_col: usize) {
        debug_assert_eq!(self.width, rhs.width);
        debug_assert!(start_col < self.width);

        if self.words.is_empty() {
            return;
        }

        let start_word = start_col / 64;
        let offset = start_col % 64;
        if offset == 0 {
            for word in start_word..self.words.len() {
                self.words[word] ^= rhs.words[word];
            }
        } else {
            self.words[start_word] ^= rhs.words[start_word] & (u64::MAX << offset);
            for word in (start_word + 1)..self.words.len() {
                self.words[word] ^= rhs.words[word];
            }
        }
        self.clear_padding_bits();
    }

    pub(crate) fn dot_parity(&self, rhs: &Self) -> Result<u8> {
        if self.width != rhs.width {
            return Err(QecError::RowWidthMismatch {
                expected: self.width,
                actual: rhs.width,
            });
        }

        if self.words.is_empty() {
            return Ok(0);
        }

        let last = self.words.len() - 1;
        let mut parity = 0u32;
        for (left, right) in self.words[..last].iter().zip(&rhs.words[..last]) {
            parity ^= (*left & *right).count_ones();
        }
        let mask = tail_mask(self.width);
        parity ^= ((self.words[last] & rhs.words[last]) & mask).count_ones();

        Ok((parity & 1) as u8)
    }

    pub(crate) fn weight(&self) -> usize {
        if self.words.is_empty() {
            return 0;
        }

        let last = self.words.len() - 1;
        let mut weight = 0usize;
        for word in &self.words[..last] {
            weight += word.count_ones() as usize;
        }
        weight += (self.words[last] & tail_mask(self.width)).count_ones() as usize;
        weight
    }

    pub(crate) fn eq_logical(&self, rhs: &Self) -> Result<bool> {
        if self.width != rhs.width {
            return Err(QecError::RowWidthMismatch {
                expected: self.width,
                actual: rhs.width,
            });
        }

        if self.words.len() != rhs.words.len() {
            return Ok(false);
        }

        if self.words.is_empty() {
            return Ok(true);
        }

        let last = self.words.len() - 1;
        if self.words[..last] != rhs.words[..last] {
            return Ok(false);
        }
        let mask = tail_mask(self.width);
        Ok((self.words[last] & mask) == (rhs.words[last] & mask))
    }

    pub(crate) fn is_zero(&self) -> bool {
        if self.words.is_empty() {
            return true;
        }

        let last = self.words.len() - 1;
        if self.words[..last].iter().any(|word| *word != 0) {
            return false;
        }
        (self.words[last] & tail_mask(self.width)) == 0
    }

    fn clear_padding_bits(&mut self) {
        if let Some(last) = self.words.last_mut() {
            *last &= tail_mask(self.width);
        }
    }

    #[cfg(test)]
    pub(crate) fn set_storage_padding_for_test(&mut self) {
        if let Some(last) = self.words.last_mut() {
            *last |= !tail_mask(self.width);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct PackedReducedRows {
    rows: Vec<BitPackedRow>,
    pivot_cols: Vec<usize>,
    width: usize,
}

#[allow(dead_code)]
impl PackedReducedRows {
    pub(crate) fn try_from_reduced_rows(reduced: &ReducedRows) -> Result<Self> {
        let rows = reduced
            .rows
            .iter()
            .map(|row| BitPackedRow::try_from_dense(row, reduced.width))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            rows,
            pivot_cols: reduced.pivot_cols.clone(),
            width: reduced.width,
        })
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }
}

#[allow(dead_code)]
pub(crate) fn try_in_packed_reduced_row_span(
    reduced: &PackedReducedRows,
    target: &BitPackedRow,
) -> Result<bool> {
    if target.width() != reduced.width {
        return Err(QecError::RowWidthMismatch {
            expected: reduced.width,
            actual: target.width(),
        });
    }

    let mut remainder = target.clone();
    for (pivot_row, pivot_col) in reduced.pivot_cols.iter().copied().enumerate() {
        if remainder.try_bit(pivot_col)? == 1 {
            remainder.xor_assign(&reduced.rows[pivot_row])?;
        }
    }

    Ok(remainder.is_zero())
}

#[derive(Debug, Default)]
pub(crate) struct RandomWindowKernelWorkspace {
    permuted_rows: Vec<BitPackedRow>,
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
                self.permuted_rows.push(BitPackedRow::zeros(width));
            }
            let permuted_row = &mut self.permuted_rows[row_index];
            permuted_row.reset_zero_width(width);
            for (permuted_col, &original_col) in column_permutation.iter().enumerate() {
                if row[original_col] == 1 {
                    permuted_row.set_bit(permuted_col);
                }
            }
        }
    }

    fn reduce_permuted_rows(&mut self, width: usize) {
        let rows = &mut self.permuted_rows[..self.permuted_len];
        let mut pivot_row = 0;

        for col in 0..width {
            let Some(pivot) = (pivot_row..rows.len()).find(|&row| rows[row].bit(col) == 1) else {
                continue;
            };
            rows.swap(pivot_row, pivot);

            for row in 0..rows.len() {
                if row != pivot_row && rows[row].bit(col) == 1 {
                    xor_packed_row_from_col(rows, row, pivot_row, col);
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
                if self.permuted_rows[pivot_row].bit(free_col) == 1 {
                    vector[column_permutation[pivot_col]] = 1;
                }
            }
            basis_len += 1;
        }

        self.basis_len = basis_len;
    }
}

fn xor_packed_row_from_col(
    rows: &mut [BitPackedRow],
    target_row: usize,
    pivot_row: usize,
    start_col: usize,
) {
    if target_row < pivot_row {
        let (before_pivot, pivot_and_after) = rows.split_at_mut(pivot_row);
        before_pivot[target_row].xor_assign_from_col(&pivot_and_after[0], start_col);
    } else {
        let (before_target, target_and_after) = rows.split_at_mut(target_row);
        target_and_after[0].xor_assign_from_col(&before_target[pivot_row], start_col);
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

#[allow(dead_code)]
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
        try_in_packed_reduced_row_span, try_in_reduced_row_span, try_in_row_span_with_width,
        try_nullspace_basis_with_width, try_random_window_kernel_basis_with_width, try_rank,
        try_select_independent_rows, BitPackedRow, PackedReducedRows, RandomWindowKernelWorkspace,
    };

    fn dot(lhs: &[u8], rhs: &[u8]) -> u8 {
        lhs.iter()
            .zip(rhs)
            .fold(0, |parity, (left, right)| parity ^ (*left & *right))
    }

    fn patterned_row(width: usize, salt: usize) -> Vec<u8> {
        (0..width)
            .map(|index| u8::from(((index * salt + index / 3 + salt) % 5) < 2))
            .collect()
    }

    fn dense_weight(row: &[u8]) -> usize {
        row.iter().map(|bit| usize::from(*bit)).sum()
    }

    fn row_with_ones(width: usize, ones: &[usize]) -> Vec<u8> {
        let mut row = vec![0; width];
        for &index in ones {
            row[index] = 1;
        }
        row
    }

    #[test]
    fn gf2_bitpacked_rows_match_dense_binary_rows() {
        for width in [0, 1, 63, 64, 65, 144] {
            let dense = patterned_row(width, 7);
            let packed = BitPackedRow::try_from_dense(&dense, width).unwrap();

            assert_eq!(packed.width(), width);
            assert_eq!(packed.to_dense(), dense);
        }
    }

    #[test]
    fn gf2_bitpacked_row_ops_match_dense_ops() {
        let lhs_dense = patterned_row(144, 3);
        let rhs_dense = patterned_row(144, 5);
        let mut expected_xor = lhs_dense.clone();
        for (left, right) in expected_xor.iter_mut().zip(&rhs_dense) {
            *left ^= *right;
        }

        let mut lhs = BitPackedRow::try_from_dense(&lhs_dense, 144).unwrap();
        let rhs = BitPackedRow::try_from_dense(&rhs_dense, 144).unwrap();

        assert_eq!(lhs.dot_parity(&rhs).unwrap(), dot(&lhs_dense, &rhs_dense));
        assert_eq!(lhs.weight(), dense_weight(&lhs_dense));
        assert!(!lhs.eq_logical(&rhs).unwrap());
        assert!(!lhs.is_zero());

        lhs.xor_assign(&rhs).unwrap();
        assert_eq!(lhs.to_dense(), expected_xor);
        assert_eq!(lhs.weight(), dense_weight(&expected_xor));
        assert_eq!(
            lhs.dot_parity(&rhs).unwrap(),
            dot(&expected_xor, &rhs_dense)
        );
        assert!(BitPackedRow::zeros(144).is_zero());
        assert!(BitPackedRow::zeros(144)
            .eq_logical(&BitPackedRow::try_from_dense(&vec![0; 144], 144).unwrap())
            .unwrap());
    }

    #[test]
    fn gf2_bitpacked_row_ops_handle_tail_bits() {
        for width in [1, 63, 65, 144] {
            let dense = patterned_row(width, 11);
            let mut clean = BitPackedRow::try_from_dense(&dense, width).unwrap();
            let mut dirty = BitPackedRow::try_from_dense(&dense, width).unwrap();
            dirty.set_storage_padding_for_test();

            assert_eq!(dirty.to_dense(), dense);
            assert_eq!(dirty.weight(), dense_weight(&dense));
            assert_eq!(dirty.dot_parity(&clean).unwrap(), dot(&dense, &dense));
            assert!(dirty.eq_logical(&clean).unwrap());
            assert_eq!(dirty.is_zero(), dense.iter().all(|bit| *bit == 0));

            clean.xor_assign(&dirty).unwrap();
            assert!(clean.is_zero());
            assert_eq!(clean.to_dense(), vec![0; width]);
        }
    }

    #[test]
    fn gf2_bitpacked_rows_reject_invalid_binary_inputs() {
        assert_eq!(
            BitPackedRow::try_from_dense(&[1, 2, 0], 3),
            Err(QecError::InvalidBinaryEntry {
                row: 0,
                col: 1,
                value: 2,
            })
        );
        assert_eq!(
            BitPackedRow::try_from_dense(&[1, 0], 3),
            Err(QecError::RowWidthMismatch {
                expected: 3,
                actual: 2,
            })
        );

        let width_three = BitPackedRow::try_from_dense(&[1, 0, 1], 3).unwrap();
        let width_four = BitPackedRow::try_from_dense(&[1, 0, 1, 0], 4).unwrap();
        assert_eq!(
            width_three.dot_parity(&width_four),
            Err(QecError::RowWidthMismatch {
                expected: 3,
                actual: 4,
            })
        );
        assert_eq!(
            width_three.eq_logical(&width_four),
            Err(QecError::RowWidthMismatch {
                expected: 3,
                actual: 4,
            })
        );
        let mut width_three_copy = width_three.clone();
        assert_eq!(
            width_three_copy.xor_assign(&width_four),
            Err(QecError::RowWidthMismatch {
                expected: 3,
                actual: 4,
            })
        );
    }

    #[test]
    fn packed_reduced_row_span_membership_matches_dense_membership() {
        let reduced = super::try_rref_with_width(
            &[
                row_with_ones(65, &[0, 1, 64]),
                row_with_ones(65, &[1, 2, 64]),
            ],
            65,
        )
        .unwrap();
        let packed = PackedReducedRows::try_from_reduced_rows(&reduced).unwrap();
        let member = BitPackedRow::try_from_dense(&row_with_ones(65, &[0, 2]), 65).unwrap();
        let nonmember = BitPackedRow::try_from_dense(&row_with_ones(65, &[3]), 65).unwrap();

        assert_eq!(try_in_packed_reduced_row_span(&packed, &member), Ok(true));
        assert_eq!(
            try_in_packed_reduced_row_span(&packed, &nonmember),
            Ok(false)
        );
    }

    #[test]
    fn packed_reduced_row_span_membership_ignores_target_padding_bits() {
        let reduced = super::try_rref_with_width(&[vec![1, 0, 0]], 3).unwrap();
        let packed = PackedReducedRows::try_from_reduced_rows(&reduced).unwrap();
        let mut zero = BitPackedRow::zeros(3);
        zero.set_storage_padding_for_test();

        assert_eq!(try_in_packed_reduced_row_span(&packed, &zero), Ok(true));
    }

    #[test]
    fn packed_reduced_row_span_membership_reports_width_mismatches() {
        let reduced = super::try_rref_with_width(&[vec![1, 0, 0]], 3).unwrap();
        let packed = PackedReducedRows::try_from_reduced_rows(&reduced).unwrap();
        let target = BitPackedRow::zeros(2);
        let row = BitPackedRow::zeros(3);

        assert_eq!(
            row.try_bit(3),
            Err(QecError::RowWidthMismatch {
                expected: 3,
                actual: 4,
            })
        );
        assert_eq!(
            try_in_packed_reduced_row_span(&packed, &target),
            Err(QecError::RowWidthMismatch {
                expected: 3,
                actual: 2,
            })
        );
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

    fn reference_random_window_kernel_basis_with_width(
        matrix: &[Vec<u8>],
        width: usize,
        column_permutation: &[usize],
    ) -> Vec<Vec<u8>> {
        let permuted = matrix
            .iter()
            .map(|row| {
                column_permutation
                    .iter()
                    .map(|&original_col| row[original_col])
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let permuted_basis = try_nullspace_basis_with_width(&permuted, width).unwrap();
        let mut original_basis = Vec::with_capacity(permuted_basis.len());
        for permuted_vector in permuted_basis {
            let mut original_vector = vec![0; width];
            for (permuted_col, &original_col) in column_permutation.iter().enumerate() {
                original_vector[original_col] = permuted_vector[permuted_col];
            }
            original_basis.push(original_vector);
        }

        original_basis
    }

    fn permutation_by_stride(width: usize, stride: usize) -> Vec<usize> {
        (0..width).map(|index| (index * stride) % width).collect()
    }

    fn assert_workspace_active_rows_are_packed_width(
        workspace: &RandomWindowKernelWorkspace,
        width: usize,
    ) {
        assert!(
            workspace.permuted_rows[..workspace.permuted_len]
                .iter()
                .all(|row| row.width() == width),
            "active packed workspace rows should use logical width {width}"
        );
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
    fn gf2_bitpacked_random_window_kernel_basis_matches_dense_workspace() {
        let width_144 = 144;
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
                vec![
                    vec![0, 1, 2, 3, 4],
                    vec![4, 2, 0, 3, 1],
                    vec![1, 3, 4, 0, 2],
                ],
            ),
            (
                vec![
                    patterned_row(width_144, 3),
                    patterned_row(width_144, 5),
                    patterned_row(width_144, 7),
                    patterned_row(width_144, 11),
                    patterned_row(width_144, 13),
                    patterned_row(width_144, 17),
                ],
                width_144,
                vec![
                    (0..width_144).collect::<Vec<_>>(),
                    (0..width_144).rev().collect::<Vec<_>>(),
                    permutation_by_stride(width_144, 5),
                ],
            ),
        ];
        let mut workspace = RandomWindowKernelWorkspace::new();

        for (matrix, width, permutations) in cases {
            for permutation in permutations {
                let reference =
                    reference_random_window_kernel_basis_with_width(&matrix, width, &permutation);
                let expected =
                    try_random_window_kernel_basis_with_width(&matrix, width, &permutation)
                        .unwrap();
                let actual = workspace
                    .try_kernel_basis_with_width(&matrix, width, &permutation)
                    .unwrap()
                    .to_vec();
                assert_workspace_active_rows_are_packed_width(&workspace, width);

                assert_eq!(
                    actual, reference,
                    "width {width} permutation {permutation:?}"
                );
                assert_eq!(
                    expected, reference,
                    "width {width} permutation {permutation:?}"
                );
                assert!(actual.iter().all(|row| row.len() == width));
                for vector in &actual {
                    assert_kernel_vector(&matrix, vector);
                }
            }
        }
    }

    #[test]
    fn gf2_bitpacked_random_window_kernel_workspace_reuse_resets_state() {
        let mut workspace = RandomWindowKernelWorkspace::new();

        let wide_width = 65;
        let wide = vec![
            patterned_row(wide_width, 3),
            patterned_row(wide_width, 7),
            patterned_row(wide_width, 11),
        ];
        let wide_permutation = permutation_by_stride(wide_width, 2);
        let expected_wide =
            reference_random_window_kernel_basis_with_width(&wide, wide_width, &wide_permutation);
        let helper_wide =
            try_random_window_kernel_basis_with_width(&wide, wide_width, &wide_permutation)
                .unwrap();
        assert_eq!(helper_wide, expected_wide);
        assert_eq!(
            workspace
                .try_kernel_basis_with_width(&wide, wide_width, &wide_permutation)
                .unwrap(),
            expected_wide.as_slice()
        );
        assert_workspace_active_rows_are_packed_width(&workspace, wide_width);

        let narrow = vec![vec![1, 1], vec![0, 0]];
        let narrow_permutation = vec![1, 0];
        let expected_narrow =
            reference_random_window_kernel_basis_with_width(&narrow, 2, &narrow_permutation);
        let helper_narrow =
            try_random_window_kernel_basis_with_width(&narrow, 2, &narrow_permutation).unwrap();
        assert_eq!(helper_narrow, expected_narrow);
        let actual_narrow = workspace
            .try_kernel_basis_with_width(&narrow, 2, &narrow_permutation)
            .unwrap()
            .to_vec();
        assert_workspace_active_rows_are_packed_width(&workspace, 2);
        assert_eq!(actual_narrow, expected_narrow);
        assert!(actual_narrow.iter().all(|row| row.len() == 2));
        for vector in &actual_narrow {
            assert_kernel_vector(&narrow, vector);
        }

        let larger = vec![vec![1, 0, 0, 1], vec![0, 1, 1, 0]];
        let larger_permutation = vec![2, 0, 3, 1];
        let expected_larger =
            reference_random_window_kernel_basis_with_width(&larger, 4, &larger_permutation);
        let helper_larger =
            try_random_window_kernel_basis_with_width(&larger, 4, &larger_permutation).unwrap();
        assert_eq!(helper_larger, expected_larger);
        let actual_larger = workspace
            .try_kernel_basis_with_width(&larger, 4, &larger_permutation)
            .unwrap()
            .to_vec();
        assert_workspace_active_rows_are_packed_width(&workspace, 4);
        assert_eq!(actual_larger, expected_larger);
        assert!(actual_larger.iter().all(|row| row.len() == 4));
        for vector in &actual_larger {
            assert_kernel_vector(&larger, vector);
        }
    }

    #[test]
    fn gf2_bitpacked_random_window_kernel_basis_rejects_invalid_inputs() {
        let mut workspace = RandomWindowKernelWorkspace::new();
        let previous_wide = vec![vec![1, 0, 1, 0], vec![0, 1, 1, 1]];
        let previous_permutation = vec![3, 0, 2, 1];
        workspace
            .try_kernel_basis_with_width(&previous_wide, 4, &previous_permutation)
            .unwrap();
        assert_workspace_active_rows_are_packed_width(&workspace, 4);

        let duplicate_permutation = vec![0, 1, 1, 3];
        assert_eq!(
            workspace
                .try_kernel_basis_with_width(&previous_wide, 4, &duplicate_permutation)
                .unwrap_err(),
            QecError::InvalidColumnPermutation {
                reason: "duplicate column 1".to_owned(),
            }
        );
        assert_eq!(
            workspace
                .try_kernel_basis_with_width(&previous_wide, 4, &[0, 1, 2])
                .unwrap_err(),
            QecError::InvalidColumnPermutation {
                reason: "expected length 4, got 3".to_owned(),
            }
        );
        assert_eq!(
            workspace
                .try_kernel_basis_with_width(&previous_wide, 4, &[0, 1, 2, 4])
                .unwrap_err(),
            QecError::InvalidColumnPermutation {
                reason: "column 4 out of range for width 4".to_owned(),
            }
        );

        let invalid_binary = vec![vec![1, 2, 0, 0]];
        assert_eq!(
            workspace
                .try_kernel_basis_with_width(&invalid_binary, 4, &[0, 1, 2, 3])
                .unwrap_err(),
            QecError::InvalidBinaryEntry {
                row: 0,
                col: 1,
                value: 2,
            }
        );

        let mismatched_width = vec![vec![1, 0, 0, 1], vec![1, 0]];
        assert_eq!(
            workspace
                .try_kernel_basis_with_width(&mismatched_width, 4, &[0, 1, 2, 3])
                .unwrap_err(),
            QecError::RowWidthMismatch {
                expected: 4,
                actual: 2,
            }
        );

        let narrow = vec![vec![1, 1]];
        let narrow_permutation = vec![1, 0];
        let expected_narrow =
            reference_random_window_kernel_basis_with_width(&narrow, 2, &narrow_permutation);
        let helper_narrow =
            try_random_window_kernel_basis_with_width(&narrow, 2, &narrow_permutation).unwrap();
        assert_eq!(helper_narrow, expected_narrow);
        let actual_narrow = workspace
            .try_kernel_basis_with_width(&narrow, 2, &narrow_permutation)
            .unwrap()
            .to_vec();
        assert_workspace_active_rows_are_packed_width(&workspace, 2);
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
