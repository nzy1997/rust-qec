# Issue 129 Bivariate-Bicycle CSS Export Design

Date: 2026-06-23
Status: Approved for implementation under the Agent Desk standing policy
Scope: Wire parsed bivariate-bicycle built-in CSS specs through existing CSS export and catalog paths

## Summary

Issue #129 completes the narrow integration left by issues #126 and #128. Parsed
`bb:lx=...,ly=...,a=...,b=...` specs should flow through the existing
`qec-code code css <code-id-or-family-spec> <hx|hz>` command and emit compact
`sparse_rows` JSON using the existing typed bivariate-bicycle constructor.

No new CLI subcommand, fixture generation, benchmark integration, circuit
integration, or logical observable work is in scope.

## Current State

`qec-code/src/codes/built_in_css.rs` already has:

- `BivariateBicycleParams`
- `BuiltInCssFamily::BivariateBicycle`
- `BuiltInCssParams::BivariateBicycle(...)`
- `parse_built_in_css_code_spec("bb:...")`
- `bivariate_bicycle_css_checks(...)`

The remaining gap is in `built_in_css_checks(...)`, which still rejects parsed
bivariate-bicycle specs as parser-only before matrix generation. The catalog
also lists `bb72` but not the parameterized `bb` family shape.

## Chosen Approach

Keep the existing positional CLI grammar and wire the parsed family inside
`built_in_css_checks(...)`. When the parsed spec is
`BuiltInCssFamily::BivariateBicycle` with
`BuiltInCssParams::BivariateBicycle(params)`, call
`bivariate_bicycle_css_checks(params)`.

Add a one-line catalog entry:

```text
bb:lx=<period-x>,ly=<period-y>,a=<dx>:<dy>|...,b=<dx>:<dy>|...
```

This is the smallest change that uses the parser and constructor already merged
from the dependency issues, keeps all user-facing export behavior under the
existing `code css` path, and avoids duplicating any BB72 fixtures.

## Alternatives Considered

1. Add a separate `code css bb ...` subcommand. This was rejected because the
   issue explicitly requires the existing positional `code css <spec> hx|hz`
   path.
2. Generate new parameterized BB fixtures. This was rejected because the issue
   explicitly limits verification to comparing parameterized BB72 output against
   existing `bb72` fixtures.
3. Keep `built_in_css_checks(...)` parser-only and add a new internal helper.
   This was rejected because it would miss the issue objective and create an
   extra dispatch path.

## Data Flow

For:

```bash
qec-code code css "bb:lx=6,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0" hx
```

the flow is:

1. Clap keeps the quoted spec as the existing `CODE_ID` positional value.
2. `run_css(...)` calls `built_in_css_checks(spec)`.
3. `parse_built_in_css_code_spec(...)` returns
   `BuiltInCssCodeSpec::Family { family: BivariateBicycle, params:
   BivariateBicycle(...) }`.
4. `built_in_css_checks(...)` calls `bivariate_bicycle_css_checks(...)`.
5. `run_css(...)` selects `hx` or `hz`, wraps it in `SparseRowsMatrix`, and
   emits compact `sparse_rows` JSON.

## Error Handling

Invalid BB parameters should continue to fail during parsing or constructor
validation with existing built-in CSS parameter errors. For example:

```bash
qec-code code css "bb:lx=0,ly=6,a=3:0,b=0:3" hx
```

must exit non-zero, print no stdout, and include an
`out-of-range built-in CSS integer parameter lx for family bb` error on stderr.

## Testing

Add CLI tests in `qec-code/tests/cli.rs` whose names include `bb`, so this
command exercises the issue target:

```bash
cargo test -p qec-code --test cli bb
```

The tests must prove:

- parameterized BB72 `hx` output equals `qec-code/tests/fixtures/css/bb72_hx.json`
- parameterized BB72 `hz` output equals `qec-code/tests/fixtures/css/bb72_hz.json`
- `qec-code code css list` contains the `bb:lx=<period-x>,ly=<period-y>,a=...,b=...` family entry
- invalid `lx=0` fails non-zero, writes empty stdout, and reports a built-in CSS parameter error
