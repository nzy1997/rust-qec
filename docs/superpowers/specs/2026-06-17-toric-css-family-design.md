# Toric CSS Family Design

Date: 2026-06-17
Status: Design approved in-session, written for review
Scope: GitHub issue #71, built-in toric CSS family in `qec-code`

## Summary

Issue #71 adds a parameterized built-in CSS family:

```text
toric:d=<distance>
```

The family should expose canonical periodic square-lattice toric-code
parity-check supports as `BuiltInCssChecks`. It stays inside `qec-code`; it
does not add logical observables, decoding, circuit generation, or changes to
`rstim` and `rsinter`.

The dependency chain is already present. Issue #57 added the code-spec parser,
issue #58 wired parameterized families through `built_in_css_checks(...)`,
issue #68 added `surface_rotated:d=<distance>`, issue #60 added
`qec-code code css list`, and issue #61 added a representative fixture
manifest. Issue #71 should follow the same parser-backed registry pattern and
also update the built-in CSS catalog so the new family is discoverable.

## Goals

- Make `built_in_css_checks("toric:d=3")` return the exact `d=3` `hx` and
  `hz` rows requested by issue #71.
- Make `built_in_css_checks("toric:d=4")` return 32 columns, 16 X-check rows,
  and 16 Z-check rows, with every row weight equal to 4.
- Keep all returned rows canonical: sorted, duplicate-free, deterministic, and
  non-empty.
- Reject `distance < 2` using the existing typed built-in CSS integer parameter
  error.
- Update `built_in_css_catalog()` and `qec-code code css list` output to
  include `toric:d=<distance>`.
- Keep the implementation local to the existing `qec-code` built-in CSS
  registry and tests.

## Non-Goals

- Do not add logical observable generation.
- Do not add decoder integration.
- Do not add toric circuit generation.
- Do not add a second toric indexing convention.
- Do not refactor `surface_rotated` into a shared lattice abstraction.
- Do not change sparse-row JSON serialization.
- Do not add toric entries to the representative built-in CSS fixture
  manifest in this issue.
- Do not change `rstim` or `rsinter`.

## Current State

`qec-code/src/codes/built_in_css.rs` owns the built-in CSS registry:

- `BuiltInCssChecks`
- `BuiltInCssCatalogEntry`
- `BuiltInCssCodeSpec`
- `BuiltInCssFamily`
- `BuiltInCssParams`
- `built_in_css_catalog()`
- `parse_built_in_css_code_spec(...)`
- `built_in_css_checks(...)`

The registry already supports:

- fixed ids: `steane`, `bb72`
- parameterized families: `repetition_x:d=<distance>`,
  `repetition_z:d=<distance>`, and `surface_rotated:d=<distance>`

`built_in_css_checks(...)` parses once, then dispatches either to fixed-code
constructors or family constructors. That is the right path for
`toric:d=<distance>` as well.

The CLI list path renders `built_in_css_catalog()` as stable human-readable
text. Any new supported built-in family should be added to the catalog so users
can discover it without reading Rust source.

## Alternatives Considered

### 1. Extend the existing selector, registry dispatch, and catalog

Add `BuiltInCssFamily::Toric`, parse `toric:d=<distance>`, dispatch from
`built_in_css_checks(...)` to a private `toric_css_checks(distance)` helper,
and add a catalog entry for `toric:d=<distance>`.

Benefits:

- follows the code-spec architecture used by the existing families
- keeps one registry entry point for fixed ids and parameterized families
- makes the existing CLI export path work naturally
- keeps `qec-code code css list` aligned with supported specs
- keeps the issue narrowly scoped to `qec-code`

Costs:

- the built-in CSS catalog and its exact CLI list snapshot must be updated

This is the recommended approach.

### 2. Add the toric registry family without updating the catalog

Make `built_in_css_checks("toric:d=3")` work, but leave
`qec-code code css list` unchanged.

Benefits:

- slightly smaller change set

Costs:

- the family would be usable but not discoverable through the list command
- it conflicts with the purpose of issue #60, which added the catalog as the
  user-facing registry summary

This is not recommended.

### 3. Extract a shared square-lattice family framework

Create a general helper for two-dimensional CSS lattice families and use it for
toric and possibly `surface_rotated`.

Benefits:

- could reduce future duplication if many lattice families are added

Costs:

- expands issue #71 beyond the requested toric family
- `surface_rotated` and toric use different boundary and indexing rules
- adds abstraction before the repository has enough repeated code to justify it

This is not recommended for issue #71.

## Decision

Use the existing parser-backed registry path. Add `Toric` to
`BuiltInCssFamily`, teach the parser that `toric` is a parameterized family
requiring `d`, add a private toric generator in
`qec-code/src/codes/built_in_css.rs`, and add a catalog entry.

The public behavior should be:

```rust
BuiltInCssChecks {
    code_id: "toric",
    num_cols: 2 * distance * distance,
    hx,
    hz,
}
```

`code_id` should be the static family name, not the full input spec string,
matching the existing parameterized-family behavior.

## Indexing

Use the issue #71 data-qubit indexing convention.

Horizontal edges are listed first, row-major:

```text
h(x, y), for 0 <= x,y < d
horizontal_index(x, y) = x * d + y
```

Vertical edges are listed second, also row-major:

```text
v(x, y), for 0 <= x,y < d
vertical_index(x, y) = d * d + x * d + y
```

All coordinate arithmetic for neighboring checks uses periodic wraparound on
both axes. In code, wraparound can be handled by helper functions such as:

```rust
fn wrap_prev(value: usize, distance: usize) -> usize {
    (value + distance - 1) % distance
}

fn wrap_next(value: usize, distance: usize) -> usize {
    (value + 1) % distance
}
```

Because `distance < 2` is rejected before generation, these helpers never need
to handle `distance == 0`.

## Check Generation

The generator should produce one X-check row and one Z-check row for each
periodic lattice site or plaquette coordinate `(x, y)`, scanning row-major:

```text
for x in 0..d {
    for y in 0..d {
        ...
    }
}
```

### X Checks

`hx` represents vertex X-checks. For each `(x, y)`, emit the four incident
edges:

```text
h(x, y)
h(x, y - 1 mod d)
v(x, y)
v(x - 1 mod d, y)
```

After translating edges to column indices, sort the row before appending it.
For `d=3`, this produces the issue #71 `hx` golden rows:

```rust
vec![
    vec![0, 2, 9, 15],
    vec![0, 1, 10, 16],
    vec![1, 2, 11, 17],
    vec![3, 5, 9, 12],
    vec![3, 4, 10, 13],
    vec![4, 5, 11, 14],
    vec![6, 8, 12, 15],
    vec![6, 7, 13, 16],
    vec![7, 8, 14, 17],
]
```

### Z Checks

`hz` represents plaquette Z-checks. For each `(x, y)`, emit the four boundary
edges:

```text
h(x, y)
h(x + 1 mod d, y)
v(x, y)
v(x, y + 1 mod d)
```

After translating edges to column indices, sort the row before appending it.
For `d=3`, this produces the issue #71 `hz` golden rows:

```rust
vec![
    vec![0, 3, 9, 10],
    vec![1, 4, 10, 11],
    vec![2, 5, 9, 11],
    vec![3, 6, 12, 13],
    vec![4, 7, 13, 14],
    vec![5, 8, 12, 14],
    vec![0, 6, 15, 16],
    vec![1, 7, 16, 17],
    vec![2, 8, 15, 17],
]
```

### Canonical Rows

Every emitted support row should be:

- sorted
- duplicate-free
- non-empty
- deterministic
- in range for `num_cols`

For all accepted distances, each `hx` and `hz` row should have weight 4.

## Error Handling

Reuse the existing typed parser and generation errors:

- `toric` without parameters returns
  `QecError::MissingBuiltInCssParameter`.
- `toric:d=0` is rejected by the parser as
  `QecError::OutOfRangeBuiltInCssIntegerParameter`.
- `toric:d=1` parses as a positive integer and is then rejected by the family
  generator as `QecError::OutOfRangeBuiltInCssIntegerParameter`.
- `toric:d=nope` returns `QecError::InvalidBuiltInCssIntegerParameter`.
- `toric:d=3,d=4` returns `QecError::DuplicateBuiltInCssParameter`.
- `toric:d=3,foo=1` returns `QecError::UnexpectedBuiltInCssParameter`.

No new `QecError` variant is needed.

## Data Flow

Library callers and the existing CLI share the same registry path:

```text
input code spec string
  -> parse_built_in_css_code_spec(...)
  -> BuiltInCssCodeSpec::Family {
         family: BuiltInCssFamily::Toric,
         params: BuiltInCssParams { distance },
     }
  -> family_css_checks(...)
  -> toric_css_checks(distance)
  -> BuiltInCssChecks { code_id: "toric", num_cols, hx, hz }
```

The list path should use only catalog metadata:

```text
qec-code code css list
  -> built_in_css_catalog()
  -> stable human-readable list including toric:d=<distance>
```

Listing should not construct toric matrices.

## Testing

Add or update tests in `qec-code/tests/code.rs`.

### Parser coverage

Extend the existing positive parser test so:

```rust
parse_built_in_css_code_spec("toric:d=3")
```

returns:

```rust
Ok(BuiltInCssCodeSpec::Family {
    family: BuiltInCssFamily::Toric,
    params: BuiltInCssParams { distance: 3 },
})
```

Extend the existing negative parser test so bare `toric` returns:

```rust
Err(QecError::MissingBuiltInCssParameter {
    family: "toric".to_owned(),
    parameter: "d".to_owned(),
})
```

### `toric_d3_matches_expected_checks`

Call `built_in_css_checks("toric:d=3")` and assert:

- `code_id == "toric"`
- `num_cols == 18`
- exact `hx` rows match issue #71
- exact `hz` rows match issue #71
- rows are canonical and in range
- `CssCode::from_hx_hz(...)` accepts the dense matrices

### `toric_d4_has_expected_counts_and_weight_four_rows`

Call `built_in_css_checks("toric:d=4")` and assert:

- `code_id == "toric"`
- `num_cols == 32`
- `hx.len() == 16`
- `hz.len() == 16`
- every `hx` row has weight 4
- every `hz` row has weight 4
- rows are canonical and in range
- `CssCode::from_hx_hz(...)` accepts the dense matrices

### `toric_family_rejects_distance_below_two`

Call `built_in_css_checks("toric:d=1")` and assert:

```rust
Err(QecError::OutOfRangeBuiltInCssIntegerParameter {
    family: "toric".to_owned(),
    parameter: "d".to_owned(),
    value: 1,
})
```

### Catalog and CLI list coverage

Update `built_in_css_catalog_lists_supported_specs` so the catalog includes
`toric:d=<distance>` with a non-empty description containing `distance >= 2`.

Update the CLI list tests in `qec-code/tests/cli.rs`:

- `code_css_list_includes_supported_built_ins`
- `run_code_css_list_returns_catalog_without_newline`

The expected direct `run(...)` catalog output should become:

```text
Built-in CSS codes:
  steane                        fixed [[7,1,3]] CSS code
  bb72                          fixed [[72,12,6]] bivariate-bicycle CSS code
  repetition_x:d=<distance>     X-check chain, distance >= 2
  repetition_z:d=<distance>     Z-check chain, distance >= 2
  surface_rotated:d=<distance>  rotated surface CSS code, distance >= 2
  toric:d=<distance>            periodic square-lattice toric CSS code, distance >= 2
```

## Fixture Manifest

Do not extend `BUILT_IN_CSS_FIXTURE_CASES` in `qec-code/tests/cli.rs` for this
issue.

Issue #61 intentionally made the fixture manifest a small representative
regression sweep, not a second registry. The current manifest also does not
include `surface_rotated`, so adding toric there would be a broader manifest
policy change. Toric CLI export will still be covered indirectly through the
shared `built_in_css_checks(...)` path and can be pinned in a later fixture
manifest extension if needed.

## Verification

Run the focused issue #71 and nearby catalog/list tests:

```bash
cargo test -p qec-code --test code toric
cargo test -p qec-code --test code built_in_css_catalog_lists_supported_specs
cargo test -p qec-code --test cli code_css_list_
```

Before completion, also run:

```bash
cargo test -p qec-code
cargo fmt --check --package qec-code
```

## Acceptance Criteria

- `toric:d=<distance>` parses as a built-in CSS family spec.
- `distance < 2` is rejected.
- `d=3` exactly matches the issue #71 `hx` and `hz` rows.
- `d=4` matches the issue #71 count and weight constraints.
- All returned toric rows are sorted, duplicate-free, non-empty, and in range.
- `qec-code code css list` includes `toric:d=<distance>`.
- Existing built-in CSS families and fixed ids continue to work.
- No files outside `qec-code/src/codes/built_in_css.rs`,
  `qec-code/tests/code.rs`, `qec-code/tests/cli.rs`, and the
  superpowers plan/spec docs are needed for implementation.
