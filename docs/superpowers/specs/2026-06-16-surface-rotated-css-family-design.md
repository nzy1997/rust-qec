# Surface Rotated CSS Family Design

Date: 2026-06-16
Status: Design approved in-session, written for review
Scope: GitHub issue #68, built-in rotated-surface CSS family in `qec-code`

## Summary

Issue #68 adds a parameterized built-in CSS family:

```text
surface_rotated:d=<distance>
```

The family should expose canonical rotated-surface parity-check supports as
`BuiltInCssChecks`. It should stay strictly inside `qec-code`; it should not
add circuit generation, logical observables, decoder integration, or
`rstim`/`rsinter` refactoring.

The dependency chain is already in place. Issue #57 added the code-spec parser,
issue #58 wired parameterized families through `built_in_css_checks(...)`, and
issue #59 added the fixed `bb72` built-in. Issue #68 should follow the same
parser-backed registry pattern used by the repetition families.

## Goals

- Make `built_in_css_checks("surface_rotated:d=3")` return the exact `d=3`
  `hx` and `hz` rows requested by issue #68.
- Make `built_in_css_checks("surface_rotated:d=5")` return 25 columns, 12
  X-check rows, and 12 Z-check rows, with 4 weight-2 and 8 weight-4 rows on
  each side.
- Keep all returned rows canonical: sorted, duplicate-free, deterministic, and
  non-empty.
- Reject `distance < 2` using the existing typed built-in CSS integer parameter
  error.
- Keep the implementation local to `qec-code/src/codes/built_in_css.rs`.
- Add focused library tests in `qec-code/tests/code.rs`.

## Non-Goals

- Do not add logical observable generation.
- Do not add rotated-surface circuit generation.
- Do not add decoder integration.
- Do not change the CLI command shape.
- Do not add a CLI smoke test for this issue.
- Do not refactor or deduplicate `rstim` or `rsinter` helpers.
- Do not add toric, color-code, or unrotated surface-code families.

## Current State

`qec-code/src/codes/built_in_css.rs` currently owns the built-in CSS registry:

- `BuiltInCssChecks`
- `BuiltInCssCodeSpec`
- `BuiltInCssFamily`
- `BuiltInCssParams`
- `parse_built_in_css_code_spec(...)`
- `built_in_css_checks(...)`

The registry already supports:

- fixed ids: `steane`, `bb72`
- parameterized families: `repetition_x:d=<distance>`,
  `repetition_z:d=<distance>`

`built_in_css_checks(...)` parses once, then dispatches either to fixed-code
constructors or family constructors. That is the right path for
`surface_rotated:d=<distance>` as well.

The rotated-surface geometry needed for issue #68 is encoded in
`rstim/src/codegen/surface_code.rs`. `rsinter/tests/css_surface_special.rs` also
has a rotated-surface-style CSS smoke helper, but issue #68 explicitly warns not
to copy it as-is because registry outputs must stay canonical and non-empty.
That helper also uses a different row/index convention from the issue #68
`d=3` expectation.

## Alternatives Considered

### 1. Extend the existing selector and registry dispatch

Add `BuiltInCssFamily::SurfaceRotated`, parse
`surface_rotated:d=<distance>`, and dispatch from `built_in_css_checks(...)` to
a private `surface_rotated_css_checks(distance)` helper.

Benefits:

- follows the code-spec architecture from issues #57 and #58
- keeps one registry entry point for fixed ids and parameterized families
- preserves the existing CLI route without adding CLI-specific code
- keeps the issue narrowly scoped to `qec-code`

Costs:

- `BuiltInCssParams` remains a shared distance-only parameter payload, but this
  is sufficient for the existing families and `surface_rotated`.

This is the recommended approach.

### 2. Add a separate surface-specific lookup function

Add a function such as `surface_rotated_css_checks(...)` as a public API and
leave the generic selector unchanged.

Benefits:

- isolates the new family at first glance

Costs:

- bypasses the parser-backed family dispatch created by issues #57 and #58
- forces callers to know about multiple lookup paths
- does not make the existing generic CSS export path naturally support the new
  code spec

This is not recommended.

### 3. Extract a shared rotated-surface geometry module

Create a reusable geometry abstraction shared by `rstim`, `rsinter`, and
`qec-code`.

Benefits:

- could reduce future duplicated geometry logic

Costs:

- expands issue #68 beyond its registry-only scope
- risks changing circuit-generation or benchmark behavior
- adds an abstraction before this issue needs one

This is not recommended for issue #68.

## Decision

Use the existing parser-backed registry path. Add `SurfaceRotated` to
`BuiltInCssFamily`, teach the parser that `surface_rotated` is a parameterized
family requiring `d`, and add a private generator in
`qec-code/src/codes/built_in_css.rs`.

The public behavior should be:

```rust
BuiltInCssChecks {
    code_id: "surface_rotated",
    num_cols: distance * distance,
    hx,
    hz,
}
```

`code_id` should be the static family name, not the full input spec string,
matching the repetition-family behavior.

## Geometry And Row Supports

Use the rotated-surface data and measurement geometry from
`rstim/src/codegen/surface_code.rs`, reimplemented locally inside `qec-code` so
there is no dependency from `qec-code` back to `rstim`.

Data qubits use coordinates:

```text
(2*x + 1, 2*y + 1), for x,y in 0..d
```

Column indices use the issue #68 convention:

```text
data_index(x, y) = x * d + y
```

This makes `d=3` columns:

```text
(0,0)->0, (0,1)->1, (0,2)->2,
(1,0)->3, (1,1)->4, (1,2)->5,
(2,0)->6, (2,1)->7, (2,2)->8
```

Measurement candidates use coordinates:

```text
(2*ax, 2*ay), for ax,ay in 0..=d
```

For each measurement candidate:

- `on_boundary_1 = ax == 0 || ax == d`
- `on_boundary_2 = ay == 0 || ay == d`
- `parity = (ax % 2) != (ay % 2)`
- skip when `on_boundary_1 && parity`
- skip when `on_boundary_2 && !parity`
- `parity == true` emits an X check into `hx`
- `parity == false` emits a Z check into `hz`

The support of a measurement coordinate is the set of in-range data qubits at
the four diagonal offsets:

```text
(+1,+1), (+1,-1), (-1,+1), (-1,-1)
```

Each emitted support row should be sorted and non-empty. The coordinate scan
order should be deterministic. Empty support rows must not be returned.

For `d=3`, the exact expected output is:

```rust
num_cols = 9
hx = vec![
    vec![0, 3],
    vec![1, 2, 4, 5],
    vec![3, 4, 6, 7],
    vec![5, 8],
]
hz = vec![
    vec![1, 2],
    vec![0, 1, 3, 4],
    vec![4, 5, 7, 8],
    vec![6, 7],
]
```

For `d=5`, the generator should return:

- `num_cols = 25`
- `hx.len() = 12`
- `hz.len() = 12`
- 4 weight-2 and 8 weight-4 rows in `hx`
- 4 weight-2 and 8 weight-4 rows in `hz`

The family should accept all `distance >= 2`. Issue #68 does not restrict the
family to odd distances.

## Error Handling

Reuse the existing typed parser and generation errors:

- `surface_rotated` without parameters returns
  `QecError::MissingBuiltInCssParameter`.
- `surface_rotated:d=0` is rejected by the parser as
  `QecError::OutOfRangeBuiltInCssIntegerParameter`.
- `surface_rotated:d=1` parses as a positive integer and is then rejected by
  the family generator as `QecError::OutOfRangeBuiltInCssIntegerParameter`.
- `surface_rotated:d=nope` returns
  `QecError::InvalidBuiltInCssIntegerParameter`.
- `surface_rotated:d=3,foo=1` returns
  `QecError::UnexpectedBuiltInCssParameter`.

No new `QecError` variant is needed.

## Data Flow

Library callers and the existing CLI share the same registry path:

```text
input code spec string
  -> parse_built_in_css_code_spec(...)
  -> BuiltInCssCodeSpec::Family {
       family: BuiltInCssFamily::SurfaceRotated,
       params: BuiltInCssParams { distance },
     }
  -> surface_rotated_css_checks(distance)
  -> BuiltInCssChecks { code_id, num_cols, hx, hz }
```

The existing `qec-code code css <code-id> <hx|hz>` command should naturally work
through `built_in_css_checks(...)`, but this issue does not add a dedicated CLI
test.

## Testing

Add or update tests in `qec-code/tests/code.rs`.

### Parser coverage

Extend the existing positive parser test so:

```rust
parse_built_in_css_code_spec("surface_rotated:d=3")
```

returns:

```rust
Ok(BuiltInCssCodeSpec::Family {
    family: BuiltInCssFamily::SurfaceRotated,
    params: BuiltInCssParams { distance: 3 },
})
```

### `surface_rotated_d3_matches_expected_checks`

Call `built_in_css_checks("surface_rotated:d=3")` and assert:

- `code_id == "surface_rotated"`
- `num_cols == 9`
- exact `hx` rows match issue #68
- exact `hz` rows match issue #68
- rows are canonical

### `surface_rotated_d5_has_expected_check_counts_and_weights`

Call `built_in_css_checks("surface_rotated:d=5")` and assert:

- `code_id == "surface_rotated"`
- `num_cols == 25`
- `hx.len() == 12`
- `hz.len() == 12`
- each side has 4 weight-2 rows
- each side has 8 weight-4 rows
- rows are canonical and in range

### `surface_rotated_rejects_distance_below_two`

Call `built_in_css_checks("surface_rotated:d=1")` and assert:

```rust
Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
    family: "surface_rotated".to_owned(),
    parameter: "d".to_owned(),
    value: 1,
})
```

## Verification

Run the focused tests from issue #68. Cargo accepts only one positional test
filter, so the implementation plan should run each test individually or use an
appropriate shared filter:

```bash
cargo test -p qec-code --test code surface_rotated_d3_matches_expected_checks
cargo test -p qec-code --test code surface_rotated_d5_has_expected_check_counts_and_weights
cargo test -p qec-code --test code surface_rotated_rejects_distance_below_two
```

Before completion, also run:

```bash
cargo test -p qec-code
```

## Acceptance Criteria

- `surface_rotated:d=<distance>` parses as a built-in CSS family spec.
- `distance < 2` is rejected.
- `d=3` exactly matches the issue #68 `hx` and `hz` rows.
- `d=5` matches the issue #68 row-count and weight constraints.
- Existing built-in CSS families and fixed ids continue to work.
- No files outside `qec-code/src/codes/built_in_css.rs`,
  `qec-code/tests/code.rs`, and the superpowers plan/spec docs are needed for
  implementation.
