# Sparse GF(2) Composition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add canonical sparse GF(2) matrix composition primitives to `qec-code` for identity, transpose, horizontal concatenation, and Kronecker products.

**Architecture:** Add a new public `qec_code::sparse_gf2` module with a validated `SparseGf2Matrix` shape-and-rows type. Keep the existing `css::SparseRowsMatrix` JSON contract unchanged. Add typed `QecError` variants for sparse GF(2) validation and checked dimension arithmetic.

**Tech Stack:** Rust 2024, `qec-code`, `thiserror`, Cargo integration tests.

## Global Constraints

- No external dependencies.
- Keep all operations pure Rust.
- Keep row supports canonical: sorted, duplicate-free, and reduced by GF(2) parity.
- Shapes are explicit and validated with `num_rows` and `num_cols`.
- Empty but well-shaped matrices are valid.
- Public APIs return `QecError` values instead of panicking.
- Required verification commands:
  - `cargo test -p qec-code --test sparse_gf2 sparse_gf2_composition_matches_known_answers -- --exact`
  - `cargo test -p qec-code --test sparse_gf2 sparse_gf2_composition_rejects_invalid_shapes -- --exact`
  - `cargo test -p qec-code`
  - `cargo test`

---

### Task 1: Add Sparse GF(2) Contract Tests

**Files:**
- Create: `qec-code/tests/sparse_gf2.rs`

**Interfaces:**
- Consumes: planned `qec_code::sparse_gf2::{hconcat, identity, kron, transpose, SparseGf2Matrix}`
- Produces: exact issue-required tests that drive the public API and typed errors

- [ ] **Step 1: Write the failing integration test**

Create `qec-code/tests/sparse_gf2.rs`:

```rust
use qec_code::QecError;
use qec_code::sparse_gf2::{SparseGf2Matrix, hconcat, identity, kron, transpose};

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

#[test]
fn sparse_gf2_composition_matches_known_answers() {
    let a = SparseGf2Matrix::new(2, 2, vec![vec![0], vec![0, 1]]).unwrap();
    let b = SparseGf2Matrix::new(2, 2, vec![vec![1], vec![0]]).unwrap();

    assert_shape_and_rows(&identity(2).unwrap(), 2, 2, &[vec![0], vec![1]]);
    assert_shape_and_rows(&transpose(&a).unwrap(), 2, 2, &[vec![0, 1], vec![1]]);
    assert_shape_and_rows(&hconcat(&a, &b).unwrap(), 2, 4, &[vec![0, 3], vec![0, 1, 2]]);
    assert_shape_and_rows(&kron(&a, &b).unwrap(), 4, 4, &[vec![1], vec![0], vec![1, 3], vec![0, 2]]);

    let canonicalized =
        SparseGf2Matrix::new(2, 4, vec![vec![3, 1, 3, 2, 1], vec![2, 2]]).unwrap();
    assert_shape_and_rows(&canonicalized, 2, 4, &[vec![2], vec![]]);

    assert_shape_and_rows(&identity(0).unwrap(), 0, 0, &[]);

    let empty_wide = SparseGf2Matrix::new(0, 3, vec![]).unwrap();
    assert_shape_and_rows(&transpose(&empty_wide).unwrap(), 3, 0, &[vec![], vec![], vec![]]);

    let empty_rows_left = SparseGf2Matrix::new(0, 2, vec![]).unwrap();
    let empty_rows_right = SparseGf2Matrix::new(0, 5, vec![]).unwrap();
    assert_shape_and_rows(&hconcat(&empty_rows_left, &empty_rows_right).unwrap(), 0, 7, &[]);
    assert_shape_and_rows(&kron(&empty_rows_left, &empty_rows_right).unwrap(), 0, 10, &[]);
}

#[test]
fn sparse_gf2_composition_rejects_invalid_shapes() {
    assert_eq!(
        SparseGf2Matrix::new(1, 2, vec![vec![2]]),
        Err(QecError::SparseGf2SupportOutOfRange {
            row: 0,
            support: 2,
            num_cols: 2,
        })
    );

    assert_eq!(
        SparseGf2Matrix::new(2, 2, vec![vec![]]),
        Err(QecError::SparseGf2RowCountMismatch {
            expected: 2,
            actual: 1,
        })
    );

    let one_row = SparseGf2Matrix::new(1, 2, vec![vec![0]]).unwrap();
    let two_rows = SparseGf2Matrix::new(2, 2, vec![vec![0], vec![1]]).unwrap();
    assert_eq!(
        hconcat(&one_row, &two_rows),
        Err(QecError::SparseGf2HorizontalRowMismatch {
            left_rows: 1,
            right_rows: 2,
        })
    );

    let max_width = SparseGf2Matrix::new(0, usize::MAX, vec![]).unwrap();
    let one_col = SparseGf2Matrix::new(0, 1, vec![]).unwrap();
    assert_eq!(
        hconcat(&max_width, &one_col),
        Err(QecError::SparseGf2DimensionOverflow {
            operation: "hconcat",
        })
    );

    let two_cols = SparseGf2Matrix::new(0, 2, vec![]).unwrap();
    assert_eq!(
        kron(&max_width, &two_cols),
        Err(QecError::SparseGf2DimensionOverflow { operation: "kron" })
    );
}
```

- [ ] **Step 2: Run the known-answer test and confirm it fails at compile time**

Run:

```bash
cargo test -p qec-code --test sparse_gf2 sparse_gf2_composition_matches_known_answers -- --exact
```

Expected: FAIL because `qec_code::sparse_gf2` and its API do not exist yet.

- [ ] **Step 3: Run the negative-control test and confirm it fails at compile time**

Run:

```bash
cargo test -p qec-code --test sparse_gf2 sparse_gf2_composition_rejects_invalid_shapes -- --exact
```

Expected: FAIL because the new module and error variants do not exist yet.

- [ ] **Step 4: Commit the failing test scaffold**

Run:

```bash
git add qec-code/tests/sparse_gf2.rs
git commit -m "test: add sparse gf2 composition coverage"
```

### Task 2: Add Typed Sparse GF(2) Errors

**Files:**
- Modify: `qec-code/src/error.rs`

**Interfaces:**
- Consumes: test expectations from `qec-code/tests/sparse_gf2.rs`
- Produces: `QecError::SparseGf2RowCountMismatch`, `QecError::SparseGf2SupportOutOfRange`, `QecError::SparseGf2HorizontalRowMismatch`, and `QecError::SparseGf2DimensionOverflow`

- [ ] **Step 1: Add sparse GF(2) error variants**

In `qec-code/src/error.rs`, add these variants near the existing sparse-row
variants:

```rust
    #[error("sparse GF(2) row count mismatch: expected {expected}, got {actual}")]
    SparseGf2RowCountMismatch { expected: usize, actual: usize },
    #[error("out-of-range sparse GF(2) support {support} in row {row} for width {num_cols}")]
    SparseGf2SupportOutOfRange {
        row: usize,
        support: usize,
        num_cols: usize,
    },
    #[error("sparse GF(2) horizontal concatenation row mismatch: left has {left_rows}, right has {right_rows}")]
    SparseGf2HorizontalRowMismatch {
        left_rows: usize,
        right_rows: usize,
    },
    #[error("sparse GF(2) dimension overflow during {operation}")]
    SparseGf2DimensionOverflow { operation: &'static str },
```

- [ ] **Step 2: Run the negative-control test**

Run:

```bash
cargo test -p qec-code --test sparse_gf2 sparse_gf2_composition_rejects_invalid_shapes -- --exact
```

Expected: FAIL only because `qec_code::sparse_gf2` is still missing.

- [ ] **Step 3: Commit the typed errors**

Run:

```bash
git add qec-code/src/error.rs
git commit -m "feat: add sparse gf2 error variants"
```

### Task 3: Implement the Sparse GF(2) Module

**Files:**
- Create: `qec-code/src/sparse_gf2.rs`
- Modify: `qec-code/src/lib.rs`

**Interfaces:**
- Consumes: `QecError` variants from Task 2
- Produces: public `SparseGf2Matrix` and composition functions

- [ ] **Step 1: Expose the new module**

Add this line to `qec-code/src/lib.rs`:

```rust
pub mod sparse_gf2;
```

- [ ] **Step 2: Implement `qec-code/src/sparse_gf2.rs`**

Create `qec-code/src/sparse_gf2.rs`:

```rust
use crate::error::{QecError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseGf2Matrix {
    num_rows: usize,
    num_cols: usize,
    rows: Vec<Vec<usize>>,
}

impl SparseGf2Matrix {
    pub fn new(num_rows: usize, num_cols: usize, rows: Vec<Vec<usize>>) -> Result<Self> {
        if rows.len() != num_rows {
            return Err(QecError::SparseGf2RowCountMismatch {
                expected: num_rows,
                actual: rows.len(),
            });
        }

        let rows = rows
            .into_iter()
            .enumerate()
            .map(|(row_index, row)| canonicalize_row(num_cols, row_index, row))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            num_rows,
            num_cols,
            rows,
        })
    }

    pub fn identity(size: usize) -> Result<Self> {
        identity(size)
    }

    pub fn transpose(&self) -> Result<Self> {
        transpose(self)
    }

    pub fn hconcat(&self, rhs: &Self) -> Result<Self> {
        hconcat(self, rhs)
    }

    pub fn kron(&self, rhs: &Self) -> Result<Self> {
        kron(self, rhs)
    }

    pub fn num_rows(&self) -> usize {
        self.num_rows
    }

    pub fn num_cols(&self) -> usize {
        self.num_cols
    }

    pub fn rows(&self) -> &[Vec<usize>] {
        &self.rows
    }
}

pub fn identity(size: usize) -> Result<SparseGf2Matrix> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(size)
        .map_err(|_| QecError::SparseGf2DimensionOverflow {
            operation: "identity",
        })?;
    for index in 0..size {
        rows.push(vec![index]);
    }
    SparseGf2Matrix::new(size, size, rows)
}

pub fn transpose(matrix: &SparseGf2Matrix) -> Result<SparseGf2Matrix> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(matrix.num_cols)
        .map_err(|_| QecError::SparseGf2DimensionOverflow {
            operation: "transpose",
        })?;
    rows.resize_with(matrix.num_cols, Vec::new);

    for (row_index, row) in matrix.rows.iter().enumerate() {
        for &support in row {
            rows[support].push(row_index);
        }
    }

    SparseGf2Matrix::new(matrix.num_cols, matrix.num_rows, rows)
}

pub fn hconcat(
    left: &SparseGf2Matrix,
    right: &SparseGf2Matrix,
) -> Result<SparseGf2Matrix> {
    if left.num_rows != right.num_rows {
        return Err(QecError::SparseGf2HorizontalRowMismatch {
            left_rows: left.num_rows,
            right_rows: right.num_rows,
        });
    }

    let num_cols =
        left.num_cols
            .checked_add(right.num_cols)
            .ok_or(QecError::SparseGf2DimensionOverflow {
                operation: "hconcat",
            })?;

    let mut rows = Vec::new();
    rows.try_reserve_exact(left.num_rows)
        .map_err(|_| QecError::SparseGf2DimensionOverflow {
            operation: "hconcat",
        })?;

    for (left_row, right_row) in left.rows.iter().zip(&right.rows) {
        let mut row = Vec::new();
        row.try_reserve_exact(left_row.len().saturating_add(right_row.len()))
            .map_err(|_| QecError::SparseGf2DimensionOverflow {
                operation: "hconcat",
            })?;
        row.extend(left_row.iter().copied());
        for &support in right_row {
            row.push(
                left.num_cols.checked_add(support).ok_or(
                    QecError::SparseGf2DimensionOverflow {
                        operation: "hconcat",
                    },
                )?,
            );
        }
        rows.push(row);
    }

    SparseGf2Matrix::new(left.num_rows, num_cols, rows)
}

pub fn kron(left: &SparseGf2Matrix, right: &SparseGf2Matrix) -> Result<SparseGf2Matrix> {
    let num_rows =
        left.num_rows
            .checked_mul(right.num_rows)
            .ok_or(QecError::SparseGf2DimensionOverflow { operation: "kron" })?;
    let num_cols =
        left.num_cols
            .checked_mul(right.num_cols)
            .ok_or(QecError::SparseGf2DimensionOverflow { operation: "kron" })?;

    let mut rows = Vec::new();
    rows.try_reserve_exact(num_rows)
        .map_err(|_| QecError::SparseGf2DimensionOverflow { operation: "kron" })?;

    for left_row in &left.rows {
        for right_row in &right.rows {
            let mut row = Vec::new();
            for &left_support in left_row {
                let block_start = left_support.checked_mul(right.num_cols).ok_or(
                    QecError::SparseGf2DimensionOverflow { operation: "kron" },
                )?;
                for &right_support in right_row {
                    row.push(block_start.checked_add(right_support).ok_or(
                        QecError::SparseGf2DimensionOverflow { operation: "kron" },
                    )?);
                }
            }
            rows.push(row);
        }
    }

    SparseGf2Matrix::new(num_rows, num_cols, rows)
}

fn canonicalize_row(num_cols: usize, row_index: usize, mut row: Vec<usize>) -> Result<Vec<usize>> {
    for &support in &row {
        if support >= num_cols {
            return Err(QecError::SparseGf2SupportOutOfRange {
                row: row_index,
                support,
                num_cols,
            });
        }
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

    Ok(canonical)
}
```

- [ ] **Step 3: Run the focused tests**

Run:

```bash
cargo test -p qec-code --test sparse_gf2 sparse_gf2_composition_matches_known_answers -- --exact
cargo test -p qec-code --test sparse_gf2 sparse_gf2_composition_rejects_invalid_shapes -- --exact
```

Expected: both tests PASS.

- [ ] **Step 4: Commit the implementation**

Run:

```bash
git add qec-code/src/error.rs qec-code/src/lib.rs qec-code/src/sparse_gf2.rs qec-code/tests/sparse_gf2.rs
git commit -m "feat: add sparse gf2 composition primitives"
```

### Task 4: Format and Run Full Verification

**Files:**
- Verify: `qec-code/src/error.rs`
- Verify: `qec-code/src/lib.rs`
- Verify: `qec-code/src/sparse_gf2.rs`
- Verify: `qec-code/tests/sparse_gf2.rs`

**Interfaces:**
- Consumes: complete implementation from Tasks 1-3
- Produces: rustfmt-clean code and full test evidence

- [ ] **Step 1: Format touched Rust files**

Run:

```bash
cargo fmt --check
```

Expected: PASS. If it fails, run `cargo fmt` and rerun `cargo fmt --check`.

- [ ] **Step 2: Run the issue-required exact tests**

Run:

```bash
cargo test -p qec-code --test sparse_gf2 sparse_gf2_composition_matches_known_answers -- --exact
cargo test -p qec-code --test sparse_gf2 sparse_gf2_composition_rejects_invalid_shapes -- --exact
```

Expected: both commands PASS.

- [ ] **Step 3: Run crate-level verification**

Run:

```bash
cargo test -p qec-code
```

Expected: PASS.

- [ ] **Step 4: Run repository verification requested by Agent Desk**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 5: Commit any formatting-only changes**

If `cargo fmt` modified files after the implementation commit, run:

```bash
git add qec-code/src/error.rs qec-code/src/lib.rs qec-code/src/sparse_gf2.rs qec-code/tests/sparse_gf2.rs
git commit -m "style: format sparse gf2 composition"
```

Expected: either a small formatting commit is created or there are no changes to commit.
