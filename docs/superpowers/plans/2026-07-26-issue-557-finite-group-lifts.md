# Finite Group Algebra Lifts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add validated finite-group algebra types and left/right regular sparse GF(2) lifts for issue #557.

**Architecture:** Add a focused `qec_code::finite_group` module that owns group-table validation, canonical group-algebra elements, deterministic JSON serialization, and typed left/right lift operations. The lifts return the existing `SparseGf2Matrix` from issue #554, so row canonicalization and GF(2) duplicate cancellation reuse the established sparse-matrix boundary.

**Tech Stack:** Rust 2024, `serde`/`serde_json` already present in `qec-code`, `thiserror`-backed `QecError`, Cargo integration tests.

## Global Constraints

- Use pure Rust only; do not add an external computer-algebra dependency.
- Expose the new API as `qec_code::finite_group`.
- Define `MAX_FINITE_GROUP_ORDER` as `256` and document that it bounds associativity validation to 16,777,216 triples and table size to 65,536 entries.
- `FiniteGroupSpec::new` must return `QecError::GroupOrderLimitExceeded { order, max_order: MAX_FINITE_GROUP_ORDER }` before table cloning, inverse allocation, or associativity work when `order > MAX_FINITE_GROUP_ORDER`.
- Validate positive group order, identity range, exact square table shape, in-range products, unique two-sided identity matching the declared identity, one two-sided inverse per element, and associativity.
- `GroupAlgebraElement::new` must validate support bounds, sort supports, and cancel even multiplicities over GF(2).
- Canonical serialization must use deterministic compact JSON with group field order `order`, `identity`, `multiplication_table` and element field order `group_order`, `support`.
- Left regular lift semantics: support element `g` contributes `matrix_col * group.order() + group.multiply(group.inverse(g)?, x)`. This inverse-indexed left action is fixed by the exact issue #557 `C3` fixture.
- Right regular lift semantics: support element `h` contributes `matrix_col * group.order() + group.multiply(x, h)`.
- Lift output shape is `(matrix_rows * group.order()) x (matrix_cols * group.order())` with checked multiplication and checked column offset arithmetic.
- Public APIs return typed `QecError` values instead of panicking.
- Do not change the existing quantum Tanner parser or CSS sparse-row JSON behavior.

---

### Task 1: Finite Group Algebra Module And Lift Tests

**Files:**
- Create: `qec-code/src/finite_group.rs`
- Modify: `qec-code/src/lib.rs`
- Modify: `qec-code/src/error.rs`
- Create: `qec-code/tests/finite_group_lifts.rs`

**Interfaces:**
- Consumes: `qec_code::sparse_gf2::SparseGf2Matrix`, `qec_code::error::{QecError, Result}`.
- Produces: `qec_code::finite_group::{MAX_FINITE_GROUP_ORDER, FiniteGroupSpec, GroupAlgebraElement, LeftRegularLift, RightRegularLift, left_regular_lift, right_regular_lift}`.

- [ ] **Step 1: Write the failing integration tests**

Create `qec-code/tests/finite_group_lifts.rs` with tests that compile against the target API:

```rust
use qec_code::QecError;
use qec_code::finite_group::{
    FiniteGroupSpec, GroupAlgebraElement, LeftRegularLift, RightRegularLift,
    MAX_FINITE_GROUP_ORDER, left_regular_lift, right_regular_lift,
};
use qec_code::sparse_gf2::SparseGf2Matrix;

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
    FiniteGroupSpec::new(3, 0, vec![vec![0, 1, 2], vec![1, 2, 0], vec![2, 0, 1]])
        .unwrap()
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
    assert_eq!(
        group.to_json_string(),
        r#"{"order":3,"identity":0,"multiplication_table":[[0,1,2],[1,2,0],[2,0,1]]}"#
    );
    assert_eq!(
        ga(&group, vec![2, 1, 2, 2]).to_json_string(),
        r#"{"group_order":3,"support":[1,2]}"#
    );

    let matrix = vec![
        vec![ga(&group, vec![1, 2]), ga(&group, vec![0]), ga(&group, vec![])],
        vec![ga(&group, vec![]), ga(&group, vec![0, 1]), ga(&group, vec![1])],
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
        Err(QecError::InvalidGroupAlgebraElementSupport { support: 3, order: 3 })
    );

    let wrong_group_element = GroupAlgebraElement::new(&c2_group(), vec![1]).unwrap();
    assert_eq!(
        left_regular_lift(&group, &[vec![wrong_group_element]]),
        Err(QecError::GroupAlgebraOrderMismatch { expected: 3, actual: 2 })
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
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p qec-code --test finite_group_lifts finite_group_left_lift_matches_c3_fixture -- --exact
```

Expected: FAIL at compile time because `qec_code::finite_group` and the new `QecError` variants are not defined yet.

- [ ] **Step 3: Add typed errors**

Modify `qec-code/src/error.rs` by adding these variants to `QecError` near the existing sparse GF(2) errors:

```rust
    #[error("invalid finite group table: {reason}")]
    InvalidFiniteGroupTable { reason: String },
    #[error("finite group order {order} exceeds maximum supported order {max_order}")]
    GroupOrderLimitExceeded { order: usize, max_order: usize },
    #[error("invalid finite group element {element}: expected < {order}")]
    InvalidFiniteGroupElement { element: usize, order: usize },
    #[error("invalid group-algebra support {support}: expected < {order}")]
    InvalidGroupAlgebraElementSupport { support: usize, order: usize },
    #[error("group-algebra element order mismatch: expected {expected}, got {actual}")]
    GroupAlgebraOrderMismatch { expected: usize, actual: usize },
    #[error("group-algebra matrix row width mismatch: expected {expected}, got {actual}")]
    GroupAlgebraMatrixRowWidthMismatch { expected: usize, actual: usize },
    #[error("group-algebra dimension overflow during {operation}")]
    GroupAlgebraDimensionOverflow { operation: &'static str },
```

- [ ] **Step 4: Implement the module and export it**

Create `qec-code/src/finite_group.rs` with these implementation details:

```rust
use serde::Serialize;

use crate::error::{QecError, Result};
use crate::sparse_gf2::SparseGf2Matrix;

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
```

Implement the public methods and free functions listed in the design. Use private helpers named `validate_group_table_shape`, `find_unique_table_identity`, `build_inverse_table`, `validate_associativity`, `validate_group_element`, `canonicalize_support`, `regular_lift`, `regular_lift_shape`, `left_action`, and `right_action`. The shape helper must use `checked_mul` for both row and column dimensions and return `QecError::GroupAlgebraDimensionOverflow { operation: "regular lift shape" }` for overflow. The left action helper must use `group.multiply(group.inverse(g)?, x)` so the exact `C3` fixture matches. The right action helper must use `group.multiply(x, h)`. The regular lift helper must check rectangular input rows, check each element's `group_order`, compute output columns with checked multiplication/addition, and pass the output rows through `SparseGf2Matrix::new`.

Add `pub mod finite_group;` to `qec-code/src/lib.rs`.

- [ ] **Step 5: Run focused GREEN verification**

Run each exact command from the issue:

```bash
cargo test -p qec-code --test finite_group_lifts finite_group_left_lift_matches_c3_fixture -- --exact
cargo test -p qec-code --test finite_group_lifts left_and_right_regular_s3_actions_commute -- --exact
cargo test -p qec-code --test finite_group_lifts finite_group_lifts_reject_invalid_tables -- --exact
cargo test -p qec-code --test finite_group_lifts finite_group_lifts_reject_group_order_limit_before_allocation -- --exact
```

Expected: all four commands PASS with no warnings.

- [ ] **Step 6: Run broader verification**

Run:

```bash
cargo test -p qec-code
```

Expected: PASS with no warnings.

- [ ] **Step 7: Commit**

Run:

```bash
git add qec-code/src/error.rs qec-code/src/finite_group.rs qec-code/src/lib.rs qec-code/tests/finite_group_lifts.rs docs/superpowers/plans/2026-07-26-issue-557-finite-group-lifts.md
git commit -m "feat: add finite group algebra lifts"
```

Expected: commit succeeds and includes the implementation plan, production code, and integration tests.

## Plan Self-Review

- Spec coverage: Task 1 covers all issue #557 deliverables, acceptance criteria, negative controls, and verification commands.
- Placeholder scan: This plan contains no TBD markers or deferred implementation placeholders.
- Type consistency: Public names match the accepted design and are used consistently in the test code, production-module instructions, and verification commands.
