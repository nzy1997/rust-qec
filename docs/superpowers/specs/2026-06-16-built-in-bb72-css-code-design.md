# Built-In BB72 CSS Code Design

Date: 2026-06-16
Status: Design approved in-session, written for review
Scope: GitHub issue #59, fixed built-in `bb72` CSS parity-check source in `qec-code`

## Summary

Issue #59 adds a fixed built-in CSS code id, `bb72`, for the existing
`[[72,12,6]]` bivariate-bicycle benchmark code. The code should return the raw
`hx` and `hz` parity-check row supports from `qec-code` without requiring
external matrix files.

This feature is independent of issue #58. The repetition families in #58 are
parameterized specs such as `repetition_x:d=5`; `bb72` is a bare fixed id like
`steane`. The only required dependency is issue #57, which already added the
code-spec parser and typed validation path.

## Goals

- Add `built_in_css_checks("bb72")`.
- Return `BuiltInCssChecks` with `num_cols = 72`.
- Return 36 `hx` rows and 36 `hz` rows.
- Ensure every returned row has weight 6.
- Ensure the returned rows satisfy CSS orthogonality through
  `CssCode::from_hx_hz(...)`.
- Keep `bb72` as a fixed id, not a parameterized bicycle family.
- Let the existing `qec-code code css bb72 hx|hz` CLI path work naturally
  through the registry.

## Non-Goals

- Do not add a general bivariate-bicycle family interface.
- Do not add logical observable generation.
- Do not change `rsinter` benchmark configuration or runners.
- Do not depend on issue #58 or implement repetition-family matrices.
- Do not add new public error variants.

## Current State

`qec-code/src/codes/built_in_css.rs` currently owns:

- `BuiltInCssChecks`
- `BuiltInCssCodeSpec`
- `parse_built_in_css_code_spec(...)`
- `built_in_css_checks(...)`
- the existing `steane` fixed built-in source

The parser already accepts fixed ids and parameterized repetition-family specs,
but `built_in_css_checks(...)` still dispatches only `steane`.

`rsinter/tests/css_surface_special.rs` already contains a local
bivariate-bicycle helper used by the `bb72_css_smoke_builds_dem_with_twelve_observables`
test. That helper is the correct source to lift into `qec-code` for the fixed
`bb72` matrix construction.

## Approach

Add `bb72` as a second fixed built-in branch beside `steane` in
`built_in_css_checks(...)`.

The matrix construction should be private to `qec-code/src/codes/built_in_css.rs`.
It should use a small internal bivariate-bicycle helper with the fixed issue
#59 term sets:

```text
lx = 6
ly = 6
A = {(3,0), (0,1), (0,2)}
B = {(0,3), (1,0), (2,0)}
H_X = [A, B]
H_Z = [B^T, A^T]
```

The constructor returns sparse row supports over 72 columns. The first 36
columns represent the first block and the second 36 columns represent the
second block. For each lattice coordinate `(x, y)`, it emits one `hx` row and
one `hz` row, matching the existing `rsinter` helper.

The helper stays private because issue #59 asks for one stable benchmark code,
not a public parameter surface. Future bicycle families can reuse or reshape
the helper later if a separate issue asks for that API.

## Public Behavior

`built_in_css_checks("bb72")` returns:

```rust
BuiltInCssChecks {
    code_id: "bb72",
    num_cols: 72,
    hx: /* 36 weight-6 rows */,
    hz: /* 36 weight-6 rows */,
}
```

`parse_built_in_css_code_spec("bb72")` should parse as:

```rust
BuiltInCssCodeSpec::Fixed { code_id: "bb72" }
```

`parse_built_in_css_code_spec("bb72:d=3")` should reject the input. This keeps
the fixed id from accidentally becoming a parameterized family.

The existing CLI flow remains unchanged:

```text
qec-code code css bb72 hx
qec-code code css bb72 hz
```

Both commands should serialize the selected sparse-row matrix through the
existing `SparseRowsMatrix` wrapper.

## Data Flow

The library and CLI share the same registry path:

```text
qec-code code css bb72 hx|hz
  -> cli::run_css(...)
  -> built_in_css_checks("bb72")
  -> private bb72_checks()
  -> private bivariate_bicycle_checks(...)
  -> SparseRowsMatrix::new(72, rows)
  -> JSON output
```

Direct library callers enter at `built_in_css_checks("bb72")` and receive the
same owned sparse row supports. Dense conversion is only needed in tests to
validate CSS orthogonality through `CssCode::from_hx_hz(...)`.

## Error Handling

No new error variants are needed.

- Unknown bare ids continue returning `QecError::UnknownBuiltInCssCode`.
- Unknown parameterized families continue returning
  `QecError::UnknownBuiltInCssFamily`.
- Unexpected parameters continue using the issue #57 parser validation errors.

Because `bb72` has no parameters, `bb72:d=3` should not be accepted.

## Testing

Add focused tests in `qec-code/tests/code.rs`:

1. `bb72_has_expected_shape_and_css_orthogonality`
   - calls `built_in_css_checks("bb72")`
   - asserts `code_id == "bb72"`
   - asserts `num_cols == 72`
   - asserts `hx.len() == 36`
   - asserts `hz.len() == 36`
   - asserts every `hx` and `hz` row has weight 6
   - asserts rows are canonical and in range
   - converts sparse supports to dense binary rows
   - validates with `CssCode::from_hx_hz(...)`

2. `bb72_code_spec_rejects_unexpected_parameters`
   - asserts `parse_built_in_css_code_spec("bb72")` returns a fixed selector
   - asserts `parse_built_in_css_code_spec("bb72:d=3")` is rejected

Add a small CLI smoke test in `qec-code/tests/cli.rs` if it stays inexpensive:

- `qec-code code css bb72 hx` succeeds
- stdout parses as sparse-row JSON
- the JSON reports `num_cols = 72` and 36 rows

Run the issue verification command:

```bash
cargo test -p qec-code --test code bb72_has_expected_shape_and_css_orthogonality bb72_code_spec_rejects_unexpected_parameters
```

Also run the broader `qec-code` test target if the focused tests pass.

## Interaction With Issue #58

This design intentionally does not wait for #58. The only likely interaction is
a small merge conflict if #58 changes `qec-code/src/codes/built_in_css.rs` or
`qec-code/tests/code.rs` at the same time. The features do not depend on each
other semantically:

- #58 adds parameterized repetition-family constructors.
- #59 adds one fixed `bb72` built-in source.

Keeping `bb72` fixed and private minimizes the conflict surface and preserves a
clear path for both issues to land independently.
