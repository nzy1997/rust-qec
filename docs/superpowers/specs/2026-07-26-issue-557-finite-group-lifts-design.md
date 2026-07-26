# Issue 557 Finite Group Algebra Lifts Design

Date: 2026-07-26
Status: Approved by non-interactive Agent Desk standing policy
Scope: GitHub issue #557, Roadmap ID M2-01

## Summary

Add pure-Rust finite-group algebra primitives to `qec-code` for generalized
bicycle, lifted-product, and two-block constructor work. The implementation
introduces a validated `FiniteGroupSpec`, a canonical `GroupAlgebraElement`,
and distinct left- and right-regular lift operations that produce the existing
`SparseGf2Matrix` representation.

The issue references `docs/design/2026-07-26-qec-code-family-support.md`.
That file is not present in this worker branch. This design is grounded in the
issue body, the merged issue #554 sparse GF(2) API, and current `qec-code`
error and serialization conventions.

## Goals

- Validate explicit finite-group multiplication tables for positive order,
  declared identity, square shape, in-range products, two-sided identity,
  two-sided inverses, closure, and associativity.
- Reject declarations above a documented maximum group order before cloning the
  multiplication table, allocating validation tables, or running associativity.
- Represent group-algebra elements over GF(2), canonicalizing supports by
  sorting and canceling even multiplicities.
- Provide deterministic canonical serialization for validated group specs and
  algebra elements.
- Provide typed left-regular and right-regular lift operations.
- Produce `SparseGf2Matrix` rows with checked size and index arithmetic.
- Cover the exact `C3` left-regular fixture and all 36 `S3` left/right
  commutation checks.
- Add negative controls for missing identity, non-associativity, out-of-range
  products, out-of-range algebra support, lift-size overflow, and group-order
  limit rejection.

## Non-Goals

- Do not add generalized bicycle, lifted-product, or two-block constructors in
  this issue.
- Do not introduce an external computer-algebra dependency.
- Do not refactor the existing quantum Tanner JSON parser or change its
  identity-0 v1 input contract.
- Do not implement dense matrix multiplication or row reduction for group
  algebra matrices.

## Approaches Considered

### 1. Extend `codes::quantum_tanner::ValidatedFiniteGroup`

This reuses existing validation code, but the type also carries quantum Tanner
generator sets and is tied to a JSON parser contract that requires identity 0.
Making it a general constructor primitive would either expose unrelated fields
or risk changing existing quantum Tanner behavior. This is not selected.

### 2. Add free functions over raw multiplication tables and supports

This has a small public surface, but every caller would need to track whether
tables and supports were already validated and canonicalized. It also makes
deterministic serialization a convention instead of a type invariant. This is
not selected.

### 3. Add a dedicated `finite_group` module

Create `qec-code/src/finite_group.rs` with validated `FiniteGroupSpec`,
canonical `GroupAlgebraElement`, marker types for `LeftRegularLift` and
`RightRegularLift`, and lift functions that emit `SparseGf2Matrix`. This keeps
the new public boundary focused, composes directly with issue #554, and avoids
behavior changes in current constructors. This is the selected approach.

## Public API

Expose `pub mod finite_group` from `qec-code/src/lib.rs`.

The module defines:

```rust
pub const MAX_FINITE_GROUP_ORDER: usize = 256;

pub struct FiniteGroupSpec { ... }
pub struct GroupAlgebraElement { ... }
pub struct LeftRegularLift;
pub struct RightRegularLift;

impl FiniteGroupSpec {
    pub fn new(order: usize, identity: usize, multiplication_table: Vec<Vec<usize>>) -> Result<Self>;
    pub fn order(&self) -> usize;
    pub fn identity(&self) -> usize;
    pub fn multiplication_table(&self) -> &[Vec<usize>];
    pub fn inverse_table(&self) -> &[usize];
    pub fn multiply(&self, left: usize, right: usize) -> Result<usize>;
    pub fn inverse(&self, element: usize) -> Result<usize>;
    pub fn to_json_string(&self) -> String;
}

impl GroupAlgebraElement {
    pub fn new(group: &FiniteGroupSpec, support: Vec<usize>) -> Result<Self>;
    pub fn group_order(&self) -> usize;
    pub fn support(&self) -> &[usize];
    pub fn to_json_string(&self) -> String;
}

impl LeftRegularLift {
    pub fn checked_output_shape(&self, group: &FiniteGroupSpec, matrix_rows: usize, matrix_cols: usize) -> Result<(usize, usize)>;
    pub fn lift(&self, group: &FiniteGroupSpec, matrix: &[Vec<GroupAlgebraElement>]) -> Result<SparseGf2Matrix>;
}

impl RightRegularLift {
    pub fn checked_output_shape(&self, group: &FiniteGroupSpec, matrix_rows: usize, matrix_cols: usize) -> Result<(usize, usize)>;
    pub fn lift(&self, group: &FiniteGroupSpec, matrix: &[Vec<GroupAlgebraElement>]) -> Result<SparseGf2Matrix>;
}

pub fn left_regular_lift(group: &FiniteGroupSpec, matrix: &[Vec<GroupAlgebraElement>]) -> Result<SparseGf2Matrix>;
pub fn right_regular_lift(group: &FiniteGroupSpec, matrix: &[Vec<GroupAlgebraElement>]) -> Result<SparseGf2Matrix>;
```

`MAX_FINITE_GROUP_ORDER = 256` bounds validation work to at most 16,777,216
associativity triples and 65,536 table entries. That is large enough for the
small algebraic constructor fixtures in scope while avoiding unbounded
order-cubed work in a library call.

## Validation And Canonicalization

`FiniteGroupSpec::new` checks `order > MAX_FINITE_GROUP_ORDER` before it
inspects or clones the supplied table and before it allocates inverse data. It
then validates positive order, identity range, exact square shape, in-range
products, unique two-sided identity matching the declared identity, one
two-sided inverse per element, and associativity. Closure is represented by the
in-range product check over the entire table.

`GroupAlgebraElement::new` validates every support index against the supplied
group order, sorts supports, and keeps an element only when it appears an odd
number of times. The stored support is deterministic, sorted, duplicate-free,
and reduced over GF(2).

Serialization uses `serde_json::to_string` over small internal structs whose
field order is fixed by the struct definition. A group serializes as:

```json
{"order":3,"identity":0,"multiplication_table":[[0,1,2],[1,2,0],[2,0,1]]}
```

An algebra element serializes as:

```json
{"group_order":3,"support":[1,2]}
```

## Lift Semantics

For a group-algebra matrix with `m` rows and `n` columns over a group of order
`k`, both lifts return an `(m * k) x (n * k)` sparse GF(2) matrix. Output row
`matrix_row * k + x` expands each algebra element in that row.

For left-regular lift, a support element `g` contributes column
`matrix_col * k + group.multiply(group.inverse(g)?, x)`. This inverse-indexed
left action is the convention fixed by the issue #557 exact `C3` fixture: for
the second row block, support `1` in `C3` maps row offsets `0, 1, 2` to
`2, 0, 1`.

For right-regular lift, a support element `h` contributes column
`matrix_col * k + group.multiply(x, h)`.

Each output row is constructed through `SparseGf2Matrix::new`, so overlapping
support contributions cancel over GF(2) and the final rows are canonical.
The typed lift operations expose `checked_output_shape` so callers and tests can
exercise dimension overflow checks without constructing huge matrices.

## Errors

Add typed `QecError` variants:

```rust
InvalidFiniteGroupTable { reason: String }
GroupOrderLimitExceeded { order: usize, max_order: usize }
InvalidFiniteGroupElement { element: usize, order: usize }
InvalidGroupAlgebraElementSupport { support: usize, order: usize }
GroupAlgebraOrderMismatch { expected: usize, actual: usize }
GroupAlgebraMatrixRowWidthMismatch { expected: usize, actual: usize }
GroupAlgebraDimensionOverflow { operation: &'static str }
```

## Testing

Add `qec-code/tests/finite_group_lifts.rs` with the required exact tests:

- `finite_group_left_lift_matches_c3_fixture`
- `left_and_right_regular_s3_actions_commute`
- `finite_group_lifts_reject_invalid_tables`
- `finite_group_lifts_reject_group_order_limit_before_allocation`

The invalid-table test covers no identity, non-associativity, out-of-range
product, support outside the group, and lift-size overflow. The order-limit
test passes an oversized declaration with an empty table and expects
`GroupOrderLimitExceeded`, proving the limit check precedes shape validation,
table cloning, inverse allocation, and associativity work.

Verification:

```text
cargo test -p qec-code --test finite_group_lifts finite_group_left_lift_matches_c3_fixture -- --exact
cargo test -p qec-code --test finite_group_lifts left_and_right_regular_s3_actions_commute -- --exact
cargo test -p qec-code --test finite_group_lifts finite_group_lifts_reject_invalid_tables -- --exact
cargo test -p qec-code --test finite_group_lifts finite_group_lifts_reject_group_order_limit_before_allocation -- --exact
cargo test -p qec-code
cargo test
```
