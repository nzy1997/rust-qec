# Issue 554 Sparse GF(2) Composition Design

Date: 2026-07-26
Status: Accepted by non-interactive Agent Desk standing policy
Scope: GitHub issue #554, canonical sparse GF(2) matrix composition primitives in `qec-code`

## Summary

Add a small pure-Rust sparse GF(2) matrix module to `qec-code` for code-family
constructors that need checked composition before materializing CSS matrices.
The module carries row and column counts explicitly, canonicalizes row supports
with GF(2) parity semantics, and exposes identity, transpose, horizontal
concatenation, and Kronecker product primitives.

This design intentionally does not change `css::SparseRowsMatrix`. That type is
the existing JSON sparse-row contract and currently rejects duplicate supports
and zero-width matrices. The new composition layer has a different input
contract: accept raw sparse supports, reduce even multiplicities over GF(2), and
support empty but well-shaped matrices.

## Goals

- Provide a public sparse GF(2) matrix value with explicit `num_rows` and
  `num_cols`.
- Canonicalize every input and output row into sorted, duplicate-free supports.
- Remove even support multiplicities over GF(2).
- Check row counts, support bounds, hconcat compatibility, and dimension
  arithmetic without panics.
- Add known-answer tests matching the issue fixture exactly.
- Add negative controls for out-of-range support, incompatible hconcat row
  counts, and dimension overflow.

## Non-Goals

- Do not replace or loosen the existing CSS `sparse_rows` JSON parser.
- Do not add code-family constructors in this issue.
- Do not add dense matrix multiplication or row-reduction APIs.
- Do not add external dependencies.

## Alternatives Considered

### 1. Extend `css::SparseRowsMatrix`

This would keep all sparse rows in one type, but it would either change existing
CSS JSON semantics or require constructor flags that make the type ambiguous.
The existing tests assert duplicate rejection and zero-width rejection, while
this issue requires GF(2) canonicalization and empty well-shaped matrices. This
is not the recommended option.

### 2. Add free functions over `Vec<Vec<usize>>`

This has the smallest type surface, but shapes would have to travel as loose
parameters at every call site. It also makes validated canonical state hard to
distinguish from raw input rows. This is workable but weaker than a typed matrix
boundary.

### 3. Add a dedicated `sparse_gf2` module

Create `qec-code/src/sparse_gf2.rs` with a `SparseGf2Matrix` type and module
functions/methods for composition. This preserves CSS JSON behavior, gives
future constructors a reusable validated value, and keeps error handling typed.
This is the recommended option.

## Decision

Use a dedicated `qec_code::sparse_gf2` module:

```rust
pub struct SparseGf2Matrix {
    num_rows: usize,
    num_cols: usize,
    rows: Vec<Vec<usize>>,
}
```

Public API:

```rust
impl SparseGf2Matrix {
    pub fn new(num_rows: usize, num_cols: usize, rows: Vec<Vec<usize>>) -> Result<Self>;
    pub fn identity(size: usize) -> Result<Self>;
    pub fn transpose(&self) -> Result<Self>;
    pub fn hconcat(&self, rhs: &Self) -> Result<Self>;
    pub fn kron(&self, rhs: &Self) -> Result<Self>;
    pub fn num_rows(&self) -> usize;
    pub fn num_cols(&self) -> usize;
    pub fn rows(&self) -> &[Vec<usize>];
}

pub fn identity(size: usize) -> Result<SparseGf2Matrix>;
pub fn transpose(matrix: &SparseGf2Matrix) -> Result<SparseGf2Matrix>;
pub fn hconcat(left: &SparseGf2Matrix, right: &SparseGf2Matrix) -> Result<SparseGf2Matrix>;
pub fn kron(left: &SparseGf2Matrix, right: &SparseGf2Matrix) -> Result<SparseGf2Matrix>;
```

The free functions make the primitives easy to import in constructor code; the
methods keep chaining ergonomic for callers that already hold a matrix.

## Canonicalization

`SparseGf2Matrix::new` owns canonicalization. It validates that
`rows.len() == num_rows`, rejects any support `>= num_cols`, sorts each row, and
keeps a support only when it appears an odd number of times in that row. Empty
rows are valid. Zero rows and zero columns are valid when the row vector matches
the explicit row count.

All composition methods construct results through the same canonical path or
produce rows that are already canonical by construction. This gives callers one
invariant: every observed `rows()` value is sorted and duplicate-free.

## Operations

- Identity: return an `n x n` matrix whose row `i` contains `[i]`. `n = 0`
  returns an empty `0 x 0` matrix.
- Transpose: allocate one output row per input column and append the input row
  index to each transposed support. Since input rows are canonical and row
  iteration is ascending, output rows are sorted.
- Horizontal concatenation: require equal row counts, checked-add the column
  widths, and append right supports shifted by the left width.
- Kronecker product: checked-multiply row and column dimensions. For each
  nonzero `a[i, j]`, XOR the shifted support of every row of the right matrix
  into output row `i * rhs.num_rows + r`. The output width is
  `lhs.num_cols * rhs.num_cols`, and support indices are computed with checked
  multiplication and addition.

## Errors

Add typed `QecError` variants:

```rust
SparseGf2RowCountMismatch { expected: usize, actual: usize }
SparseGf2SupportOutOfRange { row: usize, support: usize, num_cols: usize }
SparseGf2HorizontalRowMismatch { left_rows: usize, right_rows: usize }
SparseGf2DimensionOverflow { operation: &'static str }
```

These cover the issue's negative controls while preserving existing CSS sparse
row errors.

## Testing

Add `qec-code/tests/sparse_gf2.rs` with the required exact tests:

- `sparse_gf2_composition_matches_known_answers`
- `sparse_gf2_composition_rejects_invalid_shapes`

The known-answer test checks the issue fixture for identity, transpose,
hconcat, and Kronecker product exactly. It also checks canonicalization with
even duplicate cancellation and zero-dimensional well-shaped matrices.

The negative test checks out-of-range support rejection, explicit row-count
mismatch, incompatible hconcat row counts, hconcat overflow, and Kronecker
dimension overflow.

Verification:

```text
cargo test -p qec-code --test sparse_gf2 sparse_gf2_composition_matches_known_answers -- --exact
cargo test -p qec-code --test sparse_gf2 sparse_gf2_composition_rejects_invalid_shapes -- --exact
cargo test -p qec-code
cargo test
```
