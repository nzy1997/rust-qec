# Built-In CSS Code Spec Parser Design

Date: 2026-06-16
Status: Draft accepted in-session, written for review
Scope: GitHub issue #57, built-in CSS code-spec grammar in `qec-code`

## Summary

Issue #57 asks `qec-code` to generalize built-in CSS code ids from singleton
names such as `steane` to a stable code-spec grammar that can later support
parameterized families such as `repetition_x:d=5` and `repetition_z:d=5`.

This milestone is grammar-only. It should add a parser that returns a validated
selector for either a fixed built-in or a parameterized family, but it should
not add any new matrix families or change CLI listing, help text, JSON output,
or benchmark metadata.

The parser should live next to the existing built-in CSS registry so future
registry dispatch can parse once, validate once, and then call the appropriate
fixed-code lookup or family generator.

## Goals

- Add a small public parser for built-in CSS code specs.
- Preserve existing bare-id support for fixed built-ins such as `steane`.
- Parse parameterized family specs shaped like `family:key=value`.
- Recognize the future repetition families `repetition_x` and `repetition_z`
  without generating their matrices in this issue.
- Reject malformed specs with typed `QecError` variants.
- Keep the existing `built_in_css_checks("steane")`, `Steane::new()`, and
  `qec-code code css steane hx|hz` behavior unchanged.

## Non-Goals

- Do not add repetition-family matrix constructors. That belongs to issue #58.
- Do not add `bb72` matrices. That belongs to issue #59.
- Do not add a general bicycle-code parameter surface.
- Do not change sparse-row JSON serialization.
- Do not change CLI help, listing, or output shape.
- Do not change `rstim` or `rsinter` behavior.

## Current State

The existing built-in CSS registry is intentionally small:

- `qec-code/src/codes/built_in_css.rs` exposes
  `built_in_css_checks(code_id: &str) -> Result<BuiltInCssChecks>`.
- `built_in_css_checks("steane")` returns canonical Steane `hx` and `hz` row
  supports.
- unknown bare ids return `QecError::UnknownBuiltInCssCode`.
- `qec-code/src/cli.rs` routes `qec-code code css <code-id> <hx|hz>` through
  the registry and sparse-row serializer.

That shape works for singleton ids, but future families need one grammar that
can represent both fixed built-ins and parameterized constructors without
adding one hard-coded CLI branch per family.

## Alternatives Considered

### 1. Parser type inside `codes::built_in_css`

Add a built-in CSS code-spec parser and selector types in the same module as
the built-in CSS registry. Keep `built_in_css_checks` behavior unchanged for
this issue.

Benefits:

- matches issue #57 directly
- keeps parser and registry in the same domain
- avoids introducing broader code-spec abstractions before they are needed
- gives issues #58 and #59 a clear selector type to dispatch on later

Costs:

- issue #58 still needs to wire the selector into actual family generators

This is the recommended approach.

### 2. Parser plus immediate registry internal use

Make `built_in_css_checks` call the new parser immediately, while still only
generating fixed `steane` matrices.

Benefits:

- the CLI path would start using the new grammar immediately

Costs:

- valid repetition specs would parse successfully but still have no generator
- error semantics become awkward until issue #58 lands
- the current issue asks for grammar only, not registry dispatch changes

This is not recommended for issue #57.

### 3. New general `codes::code_spec` module

Create a broader code-spec module independent of built-in CSS.

Benefits:

- leaves room for future non-CSS built-in specs

Costs:

- adds abstraction before a second consumer exists
- pulls this narrow milestone away from the built-in CSS registry it serves

This is not recommended yet.

## Decision

Add the parser and selector types to `qec-code/src/codes/built_in_css.rs`.
Expose them publicly from that module, but leave the current registry lookup
function unchanged except for sharing any small helper names if useful.

The implementation should make parsing and validation explicit now, while
leaving matrix generation to later issues.

## Public API

The parser should return a selector with two cases:

```rust
pub enum BuiltInCssCodeSpec {
    Fixed {
        code_id: &'static str,
    },
    Family {
        family: BuiltInCssFamily,
        params: BuiltInCssParams,
    },
}

pub enum BuiltInCssFamily {
    RepetitionX,
    RepetitionZ,
}

pub struct BuiltInCssParams {
    pub distance: usize,
}

pub fn parse_built_in_css_code_spec(input: &str) -> Result<BuiltInCssCodeSpec>;
```

Exact names may follow the surrounding code style, but the API should expose a
typed selector rather than forcing callers to inspect raw strings after parsing.

For issue #57, `BuiltInCssParams` can contain only `distance`. If future
families need different parameter sets, the enum can grow family-specific
payloads then.

## Grammar

The parser should accept exactly two shapes:

```text
fixed-id
family:key=value[,key=value...]
```

Accepted fixed ids:

- `steane`

Accepted family names:

- `repetition_x`
- `repetition_z`

Accepted repetition-family parameters:

- required: `d=<integer>`
- rejected: any key other than `d`
- rejected: duplicate `d`

The parser should use `d` as the external spelling because that is the issue
request and the expected future CLI/metadata shape. The typed API can expose it
as `distance`.

## Validation Rules

The parser should reject:

1. unknown bare ids, reusing `QecError::UnknownBuiltInCssCode`
2. unknown family names
3. missing required `d`
4. duplicate parameter keys
5. non-integer `d` values
6. empty parameter keys or values
7. unexpected parameter keys
8. out-of-range integer values

For issue #57, `d` should be a positive integer, so `d=0` is out of range. The
family generator in issue #58 can tighten the semantic rule to `d >= 2` when it
actually constructs nearest-neighbor chain checks.

## Error Handling

Add typed variants to `QecError` for code-spec parse failures that are not
already covered by `UnknownBuiltInCssCode`:

```rust
UnknownBuiltInCssFamily { family: String }
MissingBuiltInCssParameter { family: String, parameter: String }
DuplicateBuiltInCssParameter { family: String, parameter: String }
InvalidBuiltInCssIntegerParameter {
    family: String,
    parameter: String,
    value: String,
}
UnexpectedBuiltInCssParameter { family: String, parameter: String }
OutOfRangeBuiltInCssIntegerParameter {
    family: String,
    parameter: String,
    value: usize,
}
```

The display strings should be clear enough for CLI stderr if a future CLI path
surfaces these errors directly. Tests should assert the typed variants rather
than string matching.

## Data Flow

For this issue, data flow is limited to parsing:

```text
input &str
  -> parse_built_in_css_code_spec
  -> BuiltInCssCodeSpec selector
  -> future registry or generator dispatch
```

Existing matrix data flow remains unchanged:

```text
built_in_css_checks("steane")
  -> BuiltInCssChecks
  -> SparseRowsMatrix
  -> sparse_rows JSON
```

## Testing

Add the issue-requested tests to `qec-code/tests/code.rs`:

- `built_in_css_code_spec_parses_fixed_and_parameterized_ids`
- `built_in_css_code_spec_rejects_unknown_family_missing_distance_and_bad_integers`

The positive test should cover:

- `steane` parses as a fixed selector
- `repetition_x:d=5` parses as family `RepetitionX` with distance `5`
- `repetition_z:d=5` parses as family `RepetitionZ` with distance `5`

The negative test should cover at least:

- `unknown:d=5` rejects an unknown family
- `repetition_x` rejects missing `d`
- `repetition_x:d=nope` rejects a bad integer
- `repetition_x:d=5,d=7` rejects duplicate `d`
- `repetition_x:d=0` rejects out-of-range distance
- `repetition_x:d=5,foo=1` rejects unexpected parameters

Run the verification command requested by the issue:

```text
cargo test -p qec-code --test code built_in_css_code_spec_parses_fixed_and_parameterized_ids built_in_css_code_spec_rejects_unknown_family_missing_distance_and_bad_integers
```

Because the registry and CLI behavior should stay unchanged, a final local
check should also run:

```text
cargo test -p qec-code
```

## Implementation Notes

- Keep parsing code small and deterministic; the grammar does not need a parser
  crate.
- Split once on `:` to distinguish fixed ids from family specs.
- Split parameter lists on `,` and each pair on `=`.
- Track seen parameter keys in a small local set or option field so duplicate
  detection is explicit.
- Do not normalize unknown spellings into valid ids.
- Do not silently ignore extra parameters.
- Do not call matrix constructors from the parser.

## Acceptance Criteria

- The parser distinguishes fixed ids from parameterized family specs.
- Valid repetition-family specs produce a typed selector with distance.
- malformed specs return typed errors instead of being silently misparsed.
- Existing built-in Steane registry and CLI export tests continue to pass.
- The issue-requested targeted test command passes.
