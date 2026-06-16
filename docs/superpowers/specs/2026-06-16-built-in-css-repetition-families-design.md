# Built-In CSS Repetition Families Design

Date: 2026-06-16
Status: Draft accepted in-session, written for review
Scope: GitHub issue #58, built-in CSS repetition-family matrix generation in `qec-code`

## Summary

Issue #58 asks `qec-code` to turn the tiny one-sided CSS chain used in
`rstim/tests/css_codegen.rs` into explicit built-in CSS code specs:

```text
repetition_x:d=<distance>
repetition_z:d=<distance>
```

The grammar for these specs already exists from issue #57. This issue should
wire parsed repetition-family selectors into the built-in CSS registry and
generate canonical nearest-neighbor chain supports. It should not add
observables, logical operators, circuit generation, non-chain variants, or the
future `bb72` built-in from issue #59.

## Goals

- Make `built_in_css_checks("repetition_x:d=5")` return a width-5 CSS check
  source with chain checks in `hx` and no `hz` rows.
- Make `built_in_css_checks("repetition_z:d=5")` return the same chain checks
  in `hz` and no `hx` rows.
- Keep `built_in_css_checks("steane")` and existing CLI export behavior
  working unchanged.
- Build both repetition families through one shared chain-support helper.
- Reject `distance < 2` at generation time so the registry cannot return a
  degenerate repetition-family code with no checks.
- Add focused tests for the exact issue-requested shapes and invalid distance.

## Non-Goals

- Do not add logical observables or logical operators.
- Do not add circuit generation.
- Do not add non-chain repetition variants.
- Do not add `bb72`; that belongs to issue #59.
- Do not add a general parameter system beyond the existing issue #57 parser.
- Do not change sparse-row JSON serialization.
- Do not change the CLI command shape.

## Current State

`qec-code/src/codes/built_in_css.rs` currently exposes:

- `BuiltInCssChecks`, the owned raw CSS matrix result type
- `parse_built_in_css_code_spec`, which parses fixed ids and repetition-family
  specs
- `BuiltInCssCodeSpec`, `BuiltInCssFamily`, and `BuiltInCssParams`
- `built_in_css_checks(code_id: &str)`, which currently recognizes only
  `steane`

The parser already accepts:

- `steane`
- `repetition_x:d=5`
- `repetition_z:d=5`

It rejects unknown families, missing parameters, duplicate parameter keys,
non-integer distances, unexpected parameters, and `d=0`. It intentionally does
not reject `d=1`, because issue #57 was grammar-only and treated positive
integers as syntactically valid.

The current CLI path already routes:

```text
qec-code code css <code-id> <hx|hz>
```

through `built_in_css_checks` and `SparseRowsMatrix`. Therefore no CLI enum or
serialization changes are needed for repetition families; widening the registry
lookup is enough for CLI export to work naturally.

## Alternatives Considered

### 1. Extend `built_in_css_checks` to parse and dispatch code specs

Make the existing registry entry point call `parse_built_in_css_code_spec`.
Fixed selectors dispatch to existing fixed-code constructors. Repetition-family
selectors dispatch to a new repetition generator.

Benefits:

- matches the direction established by issue #57
- keeps one public lookup path for fixed ids and family specs
- makes the existing CLI export path support repetition specs without a CLI
  redesign
- keeps the implementation small and local to the built-in CSS registry

Costs:

- `built_in_css_checks` becomes a code-spec lookup rather than a bare-id-only
  lookup, but that is the intended evolution of the API.

This is the recommended approach.

### 2. Add a separate `built_in_css_checks_from_spec` function

Leave `built_in_css_checks` as a bare-id lookup and add a new API for parsed
or parameterized specs.

Benefits:

- makes the widened behavior explicit in the function name

Costs:

- forces callers and the CLI to choose between two registry paths
- keeps parameterized specs out of the existing generic CSS export command
- adds API surface before a second lookup behavior is needed

This is not recommended for issue #58.

### 3. Add per-family CLI branches

Add dedicated CLI handling for `repetition_x` and `repetition_z` outside the
registry.

Benefits:

- could special-case CLI behavior quickly

Costs:

- duplicates registry dispatch logic in the CLI
- conflicts with the existing generic `code css <code-id> <hx|hz>` command
- would make later built-ins such as `bb72` harder to add consistently

This is not recommended.

## Decision

Extend `built_in_css_checks` so it accepts the full built-in CSS code-spec
surface parsed by `parse_built_in_css_code_spec`.

The function should parse once, then dispatch:

```text
Fixed("steane") -> existing Steane raw checks
Family(RepetitionX, distance) -> repetition checks in hx
Family(RepetitionZ, distance) -> repetition checks in hz
```

Unknown fixed ids and malformed family specs should continue to use the typed
`QecError` variants introduced by issue #57.

## Data Model

The generated repetition checks should use row-support form, matching
`BuiltInCssChecks`:

```rust
BuiltInCssChecks {
    code_id: "repetition_x",
    num_cols: distance,
    hx: chain_supports(distance),
    hz: vec![],
}
```

and:

```rust
BuiltInCssChecks {
    code_id: "repetition_z",
    num_cols: distance,
    hx: vec![],
    hz: chain_supports(distance),
}
```

`code_id` can remain `&'static str`. It should identify the built-in family
name rather than trying to preserve the complete input spec string. The issue's
required observable behavior is the returned `num_cols`, `hx`, and `hz`, and
keeping `code_id` static avoids API churn.

The shared helper should generate canonical rows:

```text
chain_supports(5) = [[0,1], [1,2], [2,3], [3,4]]
```

Each row is sorted, duplicate-free, and in ascending row order.

## Validation And Error Handling

The parser remains responsible for syntax-level validation:

- family name is known
- `d` is present
- `d` parses as `usize`
- duplicate or unexpected parameters are rejected
- `d=0` is rejected as out of range

The repetition generator is responsible for the semantic rule from issue #58:

```text
distance >= 2
```

If `distance < 2`, return:

```rust
QecError::OutOfRangeBuiltInCssIntegerParameter {
    family: "repetition_x" or "repetition_z",
    parameter: "d",
    value: distance,
}
```

This reuses the existing typed error instead of adding a new variant for the
same parameter-range class.

## Data Flow

The library path should be:

1. caller invokes `built_in_css_checks("repetition_x:d=5")`
2. registry calls `parse_built_in_css_code_spec`
3. parser returns `BuiltInCssCodeSpec::Family`
4. registry validates `distance >= 2`
5. registry calls the shared chain-support helper
6. registry returns `BuiltInCssChecks { num_cols: 5, hx, hz }`

The existing CLI path should need no special handling:

1. Clap parses `qec-code code css repetition_x:d=5 hx`
2. `run_css` passes the spec string to `built_in_css_checks`
3. registry returns the generated checks
4. `SparseRowsMatrix::new` validates the selected matrix
5. `to_json_string` prints the existing compact sparse-row JSON format

## Testing And Verification

Add three focused tests to `qec-code/tests/code.rs`.

### `repetition_x_d5_matches_chain_checks`

Call `built_in_css_checks("repetition_x:d=5")` and assert:

- `num_cols == 5`
- `hx == [[0,1], [1,2], [2,3], [3,4]]`
- `hz == []`
- `hx` rows are strictly increasing

### `repetition_z_d5_matches_chain_checks`

Call `built_in_css_checks("repetition_z:d=5")` and assert:

- `num_cols == 5`
- `hx == []`
- `hz == [[0,1], [1,2], [2,3], [3,4]]`
- `hz` rows are strictly increasing

### `repetition_family_rejects_distance_below_two`

Call `built_in_css_checks("repetition_x:d=1")` and
`built_in_css_checks("repetition_z:d=1")`. Assert both return
`QecError::OutOfRangeBuiltInCssIntegerParameter` with parameter `d` and value
`1`.

Issue #58 lists this verification intent:

```text
cargo test -p qec-code --test code repetition_x_d5_matches_chain_checks repetition_z_d5_matches_chain_checks repetition_family_rejects_distance_below_two
```

Cargo accepts only one positional test filter, so the implementation plan
should run these as repeated exact test invocations or use an appropriate shared
substring filter. Before completion, run the focused tests and
`cargo test -p qec-code`.

## Implementation Notes

- Keep the helper private to `built_in_css.rs` unless a second module needs it.
- Prefer small internal functions such as `fixed_css_checks`,
  `repetition_css_checks`, and `chain_supports` if they make
  `built_in_css_checks` easier to scan.
- Avoid changing public parser types unless tests reveal an actual need.
- Leave `Steane::new()` unchanged except for any internal helper movement in
  the registry module.
