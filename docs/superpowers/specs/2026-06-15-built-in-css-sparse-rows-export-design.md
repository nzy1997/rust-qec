Date: 2026-06-15
Status: Draft accepted in-session, written for review
Scope: GitHub issue #55, sparse-row JSON export for built-in CSS checks in `qec-code`

## Summary

Issue #55 asks `qec-code` to export built-in CSS parity-check matrices through
the workspace's existing `sparse_rows` JSON contract instead of inventing a new
format.

The change should stay narrow:

- add a validated `sparse_rows` wrapper in `qec-code`
- serialize one matrix at a time (`hx` or `hz`)
- make built-in Steane export match the existing workspace fixtures byte-for-byte
- reject malformed sparse-row supports with dedicated typed errors

This task does not add CLI output, new code families, or cross-crate plumbing.
It is a library-only export path plus tests.

## Goals

- Add a reusable `sparse_rows` matrix representation in `qec-code`.
- Keep validation and JSON serialization in the generic CSS layer instead of
  the built-in registry layer.
- Export built-in Steane `hx` and `hz` matrices as standalone JSON documents
  matching the existing `rsinter` fixtures exactly.
- Reject duplicate and out-of-range support entries with specific `QecError`
  variants.
- Add issue-teeth tests that prove the positive and negative paths.

## Non-Goals

- Do not add a new top-level `{hx, hz}` JSON shape.
- Do not add CLI commands or CLI flags.
- Do not change `rstim`, `rsinter`, or fixture consumers.
- Do not add new built-in CSS code families.
- Do not widen this work into generic CSS import/export workflows beyond the
  one `sparse_rows` contract already used in the workspace.
- Do not add cross-row CSS-code validation such as orthogonality checks inside
  the sparse-row wrapper.

## Current State

The workspace already has a canonical sparse-row JSON format in
`rsinter/tests/fixtures/css/steane_hx.json` and
`rsinter/tests/fixtures/css/steane_hz.json`:

```json
{"format":"sparse_rows","num_cols":7,"rows":[[0,3,5,6],[1,3,4,6],[2,4,5,6]]}
```

`qec-code` now exposes built-in Steane checks through
`built_in_css_checks("steane")`, returning canonical row supports:

- `num_cols = 7`
- `hx = [[0, 3, 5, 6], [1, 3, 4, 6], [2, 4, 5, 6]]`
- `hz = [[0, 3, 5, 6], [1, 3, 4, 6], [2, 4, 5, 6]]`

What is missing is a typed library boundary that says "this is a validated
`sparse_rows` matrix" and can serialize it back to the exact JSON contract.
Without that boundary, callers would either hand-roll JSON or push JSON logic
down into the built-in registry, which is the wrong ownership split.

## Alternatives Considered

### 1. Free function in `css.rs`

Add a helper such as:

```rust
pub fn serialize_sparse_rows_json(num_cols: usize, rows: Vec<Vec<usize>>) -> Result<String>
```

Benefits:

- smallest API surface
- smallest code diff

Costs:

- validation and serialization are only loosely coupled
- negative-path tests must target a helper instead of a stable value type
- future reuse becomes a pile of ad hoc helpers

This is workable, but not the recommended option.

### 2. Small wrapper type in `css.rs`

Add a dedicated sparse-row matrix wrapper such as:

```rust
pub struct SparseRowsMatrix {
    num_cols: usize,
    rows: Vec<Vec<usize>>,
}
```

Benefits:

- gives the contract a clear ownership boundary
- binds validation to construction
- makes positive and negative tests direct and natural
- keeps built-in registry concerns separate from JSON contract concerns
- leaves room for future reuse without widening this issue now

Costs:

- one extra type to carry around
- one small amount of owned data copying when exporting built-ins

This is the recommended option.

### 3. Serialize directly in `built_in_css.rs`

Make the built-in registry itself return JSON strings or own the sparse-row
serialization logic.

Benefits:

- minimal short-term diff

Costs:

- mixes built-in code catalog responsibilities with format-contract logic
- makes malformed-input tests unnatural because built-in Steane data is already
  valid
- reduces reuse for non-built-in CSS matrices

This is not the recommended option.

## Decision

Add a small `SparseRowsMatrix` type under `qec-code/src/css.rs`, make it the
single owner of sparse-row validation and JSON serialization, and keep the
built-in registry as a thin source of canonical row-support data.

For issue #55, built-in export can stay explicit at the call site:

1. fetch built-in checks with `built_in_css_checks("steane")`
2. build `SparseRowsMatrix::new(checks.num_cols, checks.hx.clone())`
3. call `to_json_string()`

No extra convenience API on `BuiltInCssChecks` is required for this issue.

## Module Structure

The implementation should stay within the existing crate boundaries:

- `qec-code/src/css.rs`
  - add `SparseRowsMatrix`
  - add sparse-row validation
  - add JSON serialization
- `qec-code/src/error.rs`
  - add sparse-row-specific error variants
- `qec-code/tests/css_export.rs`
  - add issue-specific export tests

`qec-code/src/codes/built_in_css.rs` remains the owner of built-in check data
only. It should not become a JSON-formatting module.

## Public API

The wrapper type should be public so library callers can use it without going
through built-in code lookup:

```rust
pub struct SparseRowsMatrix {
    num_cols: usize,
    rows: Vec<Vec<usize>>,
}
```

Recommended constructor and serializer:

```rust
impl SparseRowsMatrix {
    pub fn new(num_cols: usize, rows: Vec<Vec<usize>>) -> Result<Self>;
    pub fn to_json_string(&self) -> String;
}
```

API notes:

- `new(...)` owns validation and is the only fallible step.
- `to_json_string()` is infallible because a constructed value is already
  validated.
- the JSON output must be canonical and compact, without a trailing newline.
  Writers that need line-oriented output should append their own newline.
- the wrapper is intentionally narrow and does not yet expose file I/O helpers.

## Validation Rules

`SparseRowsMatrix::new(...)` should enforce only the invariants required by the
workspace `sparse_rows` contract for this issue:

1. `num_cols` must be positive.
2. Every support entry in every row must be `< num_cols`.
3. No support entry may appear more than once within a row.
4. Row order must be preserved exactly as provided; the wrapper must not sort,
   normalize, or otherwise repair input rows.

Chosen non-rules:

- no cross-row uniqueness checks
- no CSS orthogonality checks
- no logical-basis checks
- no runtime sorting or repair of malformed rows
- no extra row-order canonicalization rule beyond preserving the caller's input

If input rows are malformed, construction should fail. The type should not
quietly normalize bad inputs.

## Error Handling

Sparse-row validation should stay in the existing `QecError` enum. Add three new
variants with enough detail for precise tests and debugging:

```rust
QecError::InvalidSparseRowsWidth { num_cols: usize }
QecError::DuplicateSparseRowSupport { row: usize, support: usize }
QecError::SparseRowSupportOutOfRange {
    row: usize,
    support: usize,
    num_cols: usize,
}
```

Behavioral expectations:

- duplicate detection should report the row index and offending support
- out-of-range detection should report the row index, offending support, and
  declared width
- zero-width matrices should be rejected before checking rows
- the constructor should fail fast on the first invalid row entry

These errors are for user-provided or caller-provided row supports. They are
not specific to built-in Steane.

## Data Flow

The positive path for built-in export should be:

1. `built_in_css_checks("steane")` returns canonical row supports.
2. The caller selects exactly one matrix: `hx` or `hz`.
3. `SparseRowsMatrix::new(checks.num_cols, selected_rows)` validates the data.
4. `to_json_string()` emits the compact document text:

```json
{"format":"sparse_rows","num_cols":N,"rows":[...]}
```

The current workspace fixtures store that JSON document with a trailing newline.
Byte-for-byte fixture parity should append that newline at the test or writer
boundary, not bake it into `to_json_string()`.

This keeps the issue aligned with the required input/output contract:

- input: one built-in CSS matrix
- output: one JSON document

It also avoids widening the scope into an aggregate export format.

## Testing And Verification

Add `qec-code/tests/css_export.rs` with two top-level tests named exactly as
the issue requests.

### 1. `steane_sparse_rows_json_matches_workspace_fixtures`

This test should:

- call `built_in_css_checks("steane")`
- export `hx` through `SparseRowsMatrix`
- export `hz` through `SparseRowsMatrix`
- read `rsinter/tests/fixtures/css/steane_hx.json`
- read `rsinter/tests/fixtures/css/steane_hz.json`
- compare exported strings to fixture contents byte-for-byte

This is the shared baseline proving that `qec-code` matches the existing
workspace contract exactly.

### 2. `sparse_rows_matrix_rejects_duplicate_or_out_of_range_supports`

This test should construct two invalid matrices:

- one with a duplicate support within a row, such as `rows = vec![vec![0, 0]]`
- one with an out-of-range support, such as `rows = vec![vec![3]]` for
  `num_cols = 3`

It should assert the exact `QecError` variants for each case.

### Verification command

Acceptance is the issue-provided command:

```text
cargo test -p qec-code --test css_export steane_sparse_rows_json_matches_workspace_fixtures sparse_rows_matrix_rejects_duplicate_or_out_of_range_supports
```

Passing this command proves:

1. built-in Steane exports match the workspace fixtures exactly
2. malformed sparse-row supports are rejected, so the positive test cannot pass
   by accident on unchecked input

## Scope Check

This design is intentionally small enough for one implementation pass:

- one new wrapper type
- two new error variants
- one focused integration test file

It does not require CLI design, cross-crate API design, or a new serialization
framework.
