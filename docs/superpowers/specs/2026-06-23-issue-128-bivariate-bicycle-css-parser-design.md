# Issue 128 Bivariate-Bicycle CSS Parser Design

Date: 2026-06-23
Status: Approved for implementation under the Agent Desk standing policy
Scope: Parse and validate bivariate-bicycle built-in CSS family specs in `qec-code`

## Summary

Issue #128 adds parser support for bivariate-bicycle CSS family specs such as:

```text
bb:lx=12,ly=6,a=3:0|0:1|0:2,b=0:3|1:0|2:0
```

The parser returns a typed `BuiltInCssCodeSpec::Family` payload containing
`BivariateBicycleParams`. Matrix construction from a parsed `bb:...` spec remains
out of scope for this issue.

## Current State

`qec-code/src/codes/built_in_css.rs` already parses fixed ids such as `bb72` and
distance-based families such as `toric:d=3`. Issue #126 has already added the
typed `BivariateBicycleParams` struct plus `bivariate_bicycle_css_checks(...)`
and validation helpers for positive dimensions and normalized duplicate shifts.

The current `BuiltInCssParams` shape only stores `distance`, so it cannot carry
the `lx`, `ly`, `a_terms`, and `b_terms` payload needed by `bb`.

## Chosen Approach

Use a family-specific parameter enum:

```rust
pub enum BuiltInCssParams {
    Distance { distance: usize },
    BivariateBicycle(BivariateBicycleParams),
}
```

Add `BuiltInCssFamily::BivariateBicycle` and route `bb:...` parser input to a
new bivariate-bicycle parameter parser. Keep the existing distance parser for
`repetition_x`, `repetition_z`, `surface_rotated`, and `toric`.

This keeps the public parse result typed, avoids optional-field ambiguity, and
preserves existing behavior for distance-based families.

## Parser Rules

The `bb` family accepts exactly these keys:

- `lx`: positive `usize`
- `ly`: positive `usize`
- `a`: non-empty `|`-separated list of `dx:dy` `usize` terms
- `b`: non-empty `|`-separated list of `dx:dy` `usize` terms

Keys may appear in any order. Duplicate keys are rejected. Unknown keys are
rejected. Missing keys are rejected with the parser-facing names `lx`, `ly`,
`a`, and `b`.

Parsed terms are validated using the issue #126 normalized duplicate rules:
within each polynomial, `(dx % lx, dy % ly)` must be unique. The same normalized
shift may appear once in `a` and once in `b`.

Malformed term syntax such as `a=3` is rejected as an invalid integer parameter
for `a`, because a complete term must contain both coordinates as `dx:dy`.

## Out Of Scope

This issue does not add catalog text, CLI help text, benchmark integration, or
logical observable support. It also does not make `built_in_css_checks("bb:...")`
generate matrices from the parsed spec.

## Testing

Add focused parser tests in `qec-code/tests/code.rs` whose names include
`built_in_css_code_spec`, so this command exercises the issue target:

```bash
cargo test -p qec-code --test code built_in_css_code_spec
```

The tests must prove that the parser accepts the BB144 payload with:

- `lx = 12`
- `ly = 6`
- `a_terms = [(3, 0), (0, 1), (0, 2)]`
- `b_terms = [(0, 3), (1, 0), (2, 0)]`

The same filtered target must reject:

- missing `a`
- `lx=0`
- duplicate `lx`
- unknown `foo=1`
- malformed `a=3`
- modulo-duplicate terms such as `a=0:0|6:0` when `lx=6`

