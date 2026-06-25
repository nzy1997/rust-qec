# Issue 230 Random-Window GF(2) Kernel-Basis Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a deterministic GF(2) random-window kernel-basis helper that permutes columns, computes a nullspace basis, and maps candidate rows back to the original column order.

**Architecture:** Keep the helper crate-internal in `qec-code/src/gf2.rs` next to the existing RREF and nullspace helpers. Add one explicit `QecError` variant for invalid column permutations, then cover the helper with unit tests in the `gf2` module.

**Tech Stack:** Rust 2024, existing `qec-code` GF(2) row helpers, Cargo unit tests.

## Global Constraints

- Do not add CSS witness validation.
- Do not add CLI flags.
- Do not add bit-packed matrix storage.
- Do not add external GF(2) dependencies.
- Do not add an RNG dependency or seeded permutation generator in this issue.
- The helper must return rows in original, unpermuted column order.
- The helper must be deterministic for the same matrix and permutation.
- Invalid binary entries, row-width mismatches, and invalid permutations must return clear errors.

---

## File Structure

- Modify `qec-code/src/error.rs`
  - Add `QecError::InvalidColumnPermutation { reason: String }`.
- Modify `qec-code/src/gf2.rs`
  - Add `try_random_window_kernel_basis_with_width`.
  - Add a private `validate_column_permutation`.
  - Add focused unit tests named by the issue verification commands.

---

### Task 1: Crate-Internal GF(2) Random-Window Kernel Basis

**Files:**
- Modify: `qec-code/src/error.rs`
- Modify: `qec-code/src/gf2.rs`
- Test: `qec-code/src/gf2.rs`

**Interfaces:**
- Consumes: `try_nullspace_basis_with_width(matrix: &[BinaryRow], width: usize) -> Result<Vec<BinaryRow>>`.
- Produces:
  - `QecError::InvalidColumnPermutation { reason: String }`
  - `pub(crate) fn try_random_window_kernel_basis_with_width(matrix: &[BinaryRow], width: usize, column_permutation: &[usize]) -> Result<Vec<BinaryRow>>`

- [x] **Step 1: Write failing unit tests**

In `qec-code/src/gf2.rs`, update the test module import block from:

```rust
use super::{
    try_in_row_span_with_width, try_nullspace_basis_with_width, try_select_independent_rows,
};
```

to:

```rust
use super::{
    try_in_row_span_with_width, try_nullspace_basis_with_width,
    try_random_window_kernel_basis_with_width, try_rank, try_select_independent_rows,
};
```

Then add these helpers and tests inside the existing `#[cfg(test)] mod tests`:

```rust
fn assert_kernel_vector(matrix: &[Vec<u8>], vector: &[u8]) {
    for row in matrix {
        assert_eq!(dot(row, vector), 0);
    }
}

#[test]
fn gf2_random_window_kernel_basis_contract() {
    let matrix = vec![vec![1, 1, 0, 0], vec![0, 1, 1, 0]];
    let permutation = vec![3, 0, 2, 1];

    let basis =
        try_random_window_kernel_basis_with_width(&matrix, 4, &permutation).unwrap();
    let repeated =
        try_random_window_kernel_basis_with_width(&matrix, 4, &permutation).unwrap();
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
fn gf2_random_window_kernel_basis_rejects_bad_permutation() {
    let matrix = vec![vec![1, 0, 1, 0], vec![0, 1, 1, 0]];
    let error =
        try_random_window_kernel_basis_with_width(&matrix, 4, &[0, 1, 1, 3]).unwrap_err();

    assert_eq!(
        error,
        QecError::InvalidColumnPermutation {
            reason: "duplicate column 1".to_owned(),
        }
    );
    assert!(error.to_string().contains("invalid column permutation"));
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
```

- [x] **Step 2: Run the contract test to verify RED**

Run:

```bash
cargo test -p qec-code gf2_random_window_kernel_basis_contract -q
```

Expected: FAIL to compile because `try_random_window_kernel_basis_with_width` does not exist yet.

- [x] **Step 3: Add the invalid permutation error**

In `qec-code/src/error.rs`, add this variant after `InvalidBinaryEntry`:

```rust
#[error("invalid column permutation: {reason}")]
InvalidColumnPermutation { reason: String },
```

- [x] **Step 4: Add the random-window helper implementation**

In `qec-code/src/gf2.rs`, add this implementation after `try_nullspace_basis_with_width`:

```rust
pub(crate) fn try_random_window_kernel_basis_with_width(
    matrix: &[BinaryRow],
    width: usize,
    column_permutation: &[usize],
) -> Result<Vec<BinaryRow>> {
    validate_rows_with_width(matrix, width)?;
    validate_column_permutation(column_permutation, width)?;

    let permuted = matrix
        .iter()
        .map(|row| {
            column_permutation
                .iter()
                .map(|&original_col| row[original_col])
                .collect::<BinaryRow>()
        })
        .collect::<Vec<_>>();

    let permuted_basis = try_nullspace_basis_with_width(&permuted, width)?;
    let mut original_basis = Vec::with_capacity(permuted_basis.len());
    for permuted_vector in permuted_basis {
        let mut original_vector = vec![0; width];
        for (permuted_col, &original_col) in column_permutation.iter().enumerate() {
            original_vector[original_col] = permuted_vector[permuted_col];
        }
        original_basis.push(original_vector);
    }

    Ok(original_basis)
}

fn validate_column_permutation(column_permutation: &[usize], width: usize) -> Result<()> {
    if column_permutation.len() != width {
        return Err(QecError::InvalidColumnPermutation {
            reason: format!(
                "expected length {width}, got {}",
                column_permutation.len()
            ),
        });
    }

    let mut seen = vec![false; width];
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
```

- [x] **Step 5: Run focused GREEN tests**

Run:

```bash
cargo test -p qec-code gf2_random_window_kernel_basis_contract -q
cargo test -p qec-code gf2_random_window_kernel_basis_rejects_bad_permutation -q
```

Expected: both commands pass.

- [x] **Step 6: Run qec-code regression tests**

Run:

```bash
cargo test -p qec-code -q
```

Expected: all qec-code tests pass.

- [x] **Step 7: Commit the implementation**

Run:

```bash
git add qec-code/src/error.rs qec-code/src/gf2.rs docs/superpowers/plans/2026-06-25-issue-230-random-window-gf2-kernel-basis.md
git commit -m "feat: add gf2 random-window kernel basis"
```

Expected: commit succeeds with only the GF(2) helper, its error variant, unit tests, and this plan.
