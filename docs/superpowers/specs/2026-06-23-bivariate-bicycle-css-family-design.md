# Bivariate-Bicycle CSS Constructor Design

Date: 2026-06-23
Status: Approved design reference for issue #126
Scope: Typed Rust constructor for bivariate-bicycle CSS check matrices in `qec-code`

## Summary

Issue #126 implements the constructor slice of the broader bivariate-bicycle CSS
family design. The change turns the existing private `bb72` matrix helper into
a reusable Rust API that builds sparse-row CSS checks from typed lattice and
shift parameters.

CLI parsing, catalog text, benchmark integration, circuit-level schedules, and
logical observable generation remain out of scope for this issue.

## Current State

`qec-code/src/codes/built_in_css.rs` already owns:

- `BuiltInCssChecks`
- `built_in_css_checks(...)`
- fixed `steane` and `bb72` entries
- parameterized repetition, rotated-surface, and toric CSS families
- a private `bivariate_bicycle_checks(lx, ly, a_terms, b_terms)` helper used by
  `bb72`

The private helper already emits the intended matrix layout:

- `H_X = [A, B]`
- `H_Z = [B^T, A^T]`

over a periodic `lx * ly` lattice.

## Public Rust API

Add these items to `qec-code/src/codes/built_in_css.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BivariateBicycleParams {
    pub lx: usize,
    pub ly: usize,
    pub a_terms: Vec<(usize, usize)>,
    pub b_terms: Vec<(usize, usize)>,
}

pub fn bivariate_bicycle_css_checks(
    params: BivariateBicycleParams,
) -> Result<BuiltInCssChecks>;
```

For valid parameters, the constructor returns:

- `code_id = "bb"`
- `num_cols = 2 * lx * ly`
- `hx.len() = lx * ly`
- `hz.len() = lx * ly`

The existing fixed `bb72` branch should call this constructor with:

```text
lx = 6
ly = 6
A = {(3,0), (0,1), (0,2)}
B = {(0,3), (1,0), (2,0)}
```

and then expose `code_id = "bb72"` for the fixed alias.

## Validation

The constructor validates before matrix construction:

- `lx > 0`
- `ly > 0`
- `a_terms` is non-empty
- `b_terms` is non-empty
- no duplicate normalized shift exists within `a_terms`
- no duplicate normalized shift exists within `b_terms`

Normalized duplicate detection uses `(dx % lx, dy % ly)`. For example,
`a_terms = [(0, 0), (6, 0)]` is invalid when `lx = 6`.

Terms are `usize` pairs, so negative shifts cannot enter the typed API.
Duplicate detection is intentionally per polynomial: the same normalized shift
may appear once in `A` and once in `B`, because those supports land in different
column blocks.

## Matrix Semantics

Let `block = lx * ly`. The constructor emits `block` X-check rows and `block`
Z-check rows over `2 * block` columns. The first block is the left data block,
and the second block is the right data block.

For each lattice coordinate `(x, y)`:

- X row left-block entries use `(x + dx, y + dy)` for each `A` shift.
- X row right-block entries use `(x + dx, y + dy)` for each `B` shift, offset by
  `block`.
- Z row left-block entries use transposed `B` shifts, equivalent to
  `(x - dx, y - dy)` modulo the lattice.
- Z row right-block entries use transposed `A` shifts, equivalent to
  `(x - dx, y - dy)` modulo the lattice, offset by `block`.

Rows are sorted. The validation rules prevent duplicate row supports caused by
repeated or modulo-equivalent shifts inside the same polynomial.

## Testing

Add focused tests in `qec-code/tests/code.rs` whose names include
`bivariate_bicycle`, so this command exercises the full issue target:

```bash
cargo test -p qec-code --test code bivariate_bicycle
```

The tests must prove:

- the public constructor builds the BB144 sparse checks with `num_cols == 144`
- `hx.len() == 72`
- `hz.len() == 72`
- every row has weight 6
- every row is sorted, duplicate-free, and in range
- `CssCode::from_hx_hz(...)` accepts the generated matrices
- `lx = 0` is rejected
- modulo-duplicate terms such as `[(0, 0), (6, 0)]` with `lx = 6` are rejected
- the fixed `bb72` alias still matches the constructor with the BB72 parameters
