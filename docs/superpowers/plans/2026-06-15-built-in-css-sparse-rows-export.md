# Built-in CSS Sparse Rows Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a validated `sparse_rows` export path in `qec-code` so built-in Steane `hx` and `hz` matrices serialize to the existing workspace JSON fixtures exactly, while malformed sparse-row supports fail with typed errors.

**Architecture:** Keep sparse-row validation and JSON serialization in `qec-code/src/css.rs` as a small dedicated wrapper type instead of pushing format logic into the built-in registry. Reuse the existing `built_in_css_checks("steane")` registry data as the positive export source, add sparse-row-specific `QecError` variants in `qec-code/src/error.rs`, and verify the behavior with one focused integration test file `qec-code/tests/css_export.rs`.

**Tech Stack:** Rust 2024, `qec-code`, `serde`, `serde_json`, `thiserror`, workspace fixture reads with `std::fs`, `cargo test`

---

### Task 1: Add the failing sparse-row export tests first

**Files:**
- Create: `qec-code/tests/css_export.rs`

- [ ] **Step 1: Write the new integration test file with exact positive and negative coverage**

```rust
use std::path::PathBuf;

use qec_code::codes::built_in_css::built_in_css_checks;
use qec_code::css::SparseRowsMatrix;
use qec_code::QecError;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read_fixture(rel_path: &str) -> String {
    std::fs::read_to_string(workspace_root().join(rel_path))
        .unwrap_or_else(|err| panic!("failed to read fixture {rel_path}: {err}"))
}

#[test]
fn steane_sparse_rows_json_matches_workspace_fixtures() {
    let checks = built_in_css_checks("steane").unwrap();

    let hx = SparseRowsMatrix::new(checks.num_cols, checks.hx.clone())
        .unwrap()
        .to_json_string();
    let hz = SparseRowsMatrix::new(checks.num_cols, checks.hz.clone())
        .unwrap()
        .to_json_string();

    let expected_hx = read_fixture("rsinter/tests/fixtures/css/steane_hx.json");
    let expected_hz = read_fixture("rsinter/tests/fixtures/css/steane_hz.json");

    assert_eq!(hx, expected_hx);
    assert_eq!(hz, expected_hz);
}

#[test]
fn sparse_rows_matrix_rejects_duplicate_or_out_of_range_supports() {
    assert_eq!(
        SparseRowsMatrix::new(3, vec![vec![0, 0]]),
        Err(QecError::DuplicateSparseRowSupport { row: 0, support: 0 })
    );

    assert_eq!(
        SparseRowsMatrix::new(3, vec![vec![3]]),
        Err(QecError::SparseRowSupportOutOfRange {
            row: 0,
            support: 3,
            num_cols: 3,
        })
    );
}
```

- [ ] **Step 2: Run the new test target and confirm it fails because `SparseRowsMatrix` does not exist yet**

Run:

```bash
cargo test -p qec-code --test css_export
```

Expected: compile fails with unresolved import / missing type errors for `qec_code::css::SparseRowsMatrix`, while the test names and fixture paths resolve cleanly.

- [ ] **Step 3: Commit the failing-test scaffold**

```bash
git add qec-code/tests/css_export.rs
git commit -m "test: add sparse rows export coverage"
```

### Task 2: Add sparse-row-specific typed errors

**Files:**
- Modify: `qec-code/src/error.rs`

- [ ] **Step 1: Extend `QecError` with duplicate and out-of-range sparse-row variants**

```rust
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum QecError {
    #[error("row width mismatch: expected {expected}, got {actual}")]
    RowWidthMismatch { expected: usize, actual: usize },
    #[error("invalid symplectic row width: expected even width, got {width}")]
    InvalidSymplecticRowWidth { width: usize },
    #[error("non-binary matrix entry {value} at row {row}, column {col}")]
    InvalidBinaryEntry { row: usize, col: usize, value: u8 },
    #[error("duplicate sparse-row support {support} in row {row}")]
    DuplicateSparseRowSupport { row: usize, support: usize },
    #[error(
        "out-of-range sparse-row support {support} in row {row} for width {num_cols}"
    )]
    SparseRowSupportOutOfRange {
        row: usize,
        support: usize,
        num_cols: usize,
    },
    #[error("invalid Pauli width: x has {x_width} bits, z has {z_width}")]
    InvalidPauliWidth { x_width: usize, z_width: usize },
    #[error("non-binary Pauli bit {value} in {which} support at index {index}")]
    InvalidPauliBit {
        which: &'static str,
        index: usize,
        value: u8,
    },
    #[error("stabilizers do not mutually commute")]
    NonCommutingStabilizers,
    #[error("stabilizers are linearly dependent")]
    DependentStabilizers,
    #[error("CSS X/Z checks are not orthogonal")]
    InvalidCssOrthogonality,
    #[error("logical basis extraction is unsupported for {k} logical qubits")]
    UnsupportedLogicalBasis { k: usize },
    #[error("exhaustive Pauli enumeration is unsupported for {n} qubits on this target")]
    UnsupportedExhaustiveEnumeration { n: usize },
    #[error("logical basis not found")]
    LogicalBasisNotFound,
    #[error("distance witness not found")]
    DistanceWitnessNotFound,
    #[error("unknown built-in CSS code: {code_id}")]
    UnknownBuiltInCssCode { code_id: String },
}
```

- [ ] **Step 2: Run the export test again and confirm the failure has narrowed to the missing `SparseRowsMatrix` implementation**

Run:

```bash
cargo test -p qec-code --test css_export
```

Expected: compile still fails, but the new `QecError` variants resolve and the only remaining unresolved symbols are the sparse-row wrapper methods/types.

- [ ] **Step 3: Commit the error additions**

```bash
git add qec-code/src/error.rs
git commit -m "feat: add sparse rows export errors"
```

### Task 3: Implement `SparseRowsMatrix` and JSON serialization in `css.rs`

**Files:**
- Modify: `qec-code/src/css.rs`

- [ ] **Step 1: Add the new wrapper type and its public API near the top of `css.rs`**

```rust
use crate::Pauli;
use crate::code::StabilizerCode;
use crate::error::{QecError, Result};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseRowsMatrix {
    num_cols: usize,
    rows: Vec<Vec<usize>>,
}

impl SparseRowsMatrix {
    pub fn new(num_cols: usize, rows: Vec<Vec<usize>>) -> Result<Self> {
        validate_sparse_rows(num_cols, &rows)?;
        Ok(Self { num_cols, rows })
    }

    pub fn to_json_string(&self) -> String {
        #[derive(Serialize)]
        struct SparseRowsMatrixJson<'a> {
            format: &'static str,
            num_cols: usize,
            rows: &'a [Vec<usize>],
        }

        serde_json::to_string(&SparseRowsMatrixJson {
            format: "sparse_rows",
            num_cols: self.num_cols,
            rows: &self.rows,
        })
        .expect("validated sparse rows matrix should always serialize")
    }
}
```

- [ ] **Step 2: Add the sparse-row validator without mutating caller input**

```rust
fn validate_sparse_rows(num_cols: usize, rows: &[Vec<usize>]) -> Result<()> {
    for (row_index, row) in rows.iter().enumerate() {
        let mut seen = std::collections::BTreeSet::new();
        for &support in row {
            if support >= num_cols {
                return Err(QecError::SparseRowSupportOutOfRange {
                    row: row_index,
                    support,
                    num_cols,
                });
            }
            if !seen.insert(support) {
                return Err(QecError::DuplicateSparseRowSupport {
                    row: row_index,
                    support,
                });
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Run the export test target and confirm both issue-55 tests now pass**

Run:

```bash
cargo test -p qec-code --test css_export
```

Expected: both `steane_sparse_rows_json_matches_workspace_fixtures` and `sparse_rows_matrix_rejects_duplicate_or_out_of_range_supports` pass.

- [ ] **Step 4: Commit the sparse-row wrapper implementation**

```bash
git add qec-code/src/css.rs
git commit -m "feat: add sparse rows matrix export"
```

### Task 4: Run focused regression coverage for existing `qec-code` behavior

**Files:**
- Modify: `qec-code/tests/code.rs`

- [ ] **Step 1: Add one library-level smoke test that `SparseRowsMatrix` can round-trip the built-in Steane shape without affecting existing registry tests**

```rust
use qec_code::codes::built_in_css::built_in_css_checks;
use qec_code::codes::steane::Steane;
use qec_code::css::{CssCode, SparseRowsMatrix};
use qec_code::{Pauli, QecError, StabilizerCode};

#[test]
fn sparse_rows_matrix_serializes_steane_supports() {
    let checks = built_in_css_checks("steane").unwrap();
    let text = SparseRowsMatrix::new(checks.num_cols, checks.hx.clone())
        .unwrap()
        .to_json_string();

    assert_eq!(
        text,
        r#"{"format":"sparse_rows","num_cols":7,"rows":[[0,3,5,6],[1,3,4,6],[2,4,5,6]]}"#
    );
}
```

- [ ] **Step 2: Run the focused code test file**

Run:

```bash
cargo test -p qec-code --test code
```

Expected: the existing `code` integration tests still pass, including the new Steane sparse-row serialization smoke test.

- [ ] **Step 3: Commit the regression smoke test**

```bash
git add qec-code/tests/code.rs
git commit -m "test: cover sparse rows matrix from built-in checks"
```

### Task 5: Run the full `qec-code` verification suite and close the issue scope

**Files:**
- Modify: none
- Test: `qec-code/tests/css_export.rs`
- Test: `qec-code/tests/code.rs`
- Test: `qec-code/tests/cli.rs`

- [ ] **Step 1: Run the exact issue verification command**

Run:

```bash
cargo test -p qec-code --test css_export steane_sparse_rows_json_matches_workspace_fixtures sparse_rows_matrix_rejects_duplicate_or_out_of_range_supports
```

Expected: both named tests pass.

- [ ] **Step 2: Run the full crate test suite**

Run:

```bash
cargo test -p qec-code
```

Expected: all `qec-code` unit, integration, and CLI tests pass.

- [ ] **Step 3: Inspect the final diff before shipping**

Run:

```bash
git diff --stat HEAD~4..HEAD
git status --short
```

Expected: only `qec-code/src/css.rs`, `qec-code/src/error.rs`, `qec-code/tests/code.rs`, and `qec-code/tests/css_export.rs` are changed for issue #55, with no unrelated workspace churn.

- [ ] **Step 4: Commit the final verification checkpoint**

```bash
git add qec-code/src/css.rs qec-code/src/error.rs qec-code/tests/code.rs qec-code/tests/css_export.rs
git commit -m "test: verify sparse rows export coverage"
```
