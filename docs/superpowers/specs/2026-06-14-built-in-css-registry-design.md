Date: 2026-06-14
Status: Proposed
Scope: `qec-code` built-in CSS registry for raw `Hx`/`Hz` access, with `Steane::new()` lowered from the same source data

## Summary

Issue #54 asks `qec-code` to expose built-in CSS parity-check data by code id
instead of only exposing the lowered `StabilizerCode`.

The change should add a small built-in CSS registry in `qec-code/src/codes/`
that returns canonical row-support data:

- lookup by built-in CSS code id such as `steane`
- stable result shape `BuiltInCssChecks`
- `hx` and `hz` represented as sorted, duplicate-free row supports
- rejection of unknown ids via a dedicated error
- `Steane::new()` rebuilt from the same registry data instead of maintaining
  a separate copy of the parity checks

This task is intentionally narrow. It does not expand the CLI, logical-basis
APIs, distance APIs, or non-CSS built-in code support.

## Goals

- Add a stable library lookup for built-in CSS code checks by code id.
- Return raw CSS check data in a reusable form before lowering to
  `StabilizerCode`.
- Keep the API extensible so future CSS families can be added without changing
  the public function shape.
- Make `Steane::new()` consume the same raw matrix source as the registry.
- Verify that built-in row supports are canonical: sorted and duplicate-free.

## Non-Goals

- Do not add non-CSS built-in code registries.
- Do not change CLI output or add CLI subcommands.
- Do not extend `CssCode` with a new public constructor that accepts row
  supports directly.
- Do not add user-facing import/export formats for parity-check data.
- Do not change logical-basis extraction or distance computation behavior.

## Current State

`qec-code` already has:

- a general `StabilizerCode` core
- `CssCode::from_hx_hz(hx, hz)` for dense binary check matrices
- `Steane::new()` as the first built-in code constructor

What is missing is a library path that exposes the raw built-in CSS check data
itself. Today `Steane::new()` hard-codes the dense Hamming-style check rows
inside `qec-code/src/codes/steane.rs`, and callers cannot reuse those checks
without duplicating constants or extracting them from the lowered code.

That is the wrong ownership boundary. Built-in code data should live with the
built-in code registry, and the convenience constructor should lower from that
shared source.

## Alternatives Considered

### 1. Built-in CSS registry under `codes/`

Add a dedicated module next to `qec-code/src/codes/mod.rs` that owns built-in
CSS metadata and lookup.

Benefits:

- matches the issue request directly
- keeps built-in code lookup separate from CSS validation logic
- gives future CSS families one obvious place to land
- lets `Steane::new()` and downstream callers share the same source of truth

Costs:

- requires one small lowering helper from row supports to dense bit rows

This is the recommended option.

### 2. Put built-in lookup into `css.rs`

Make the CSS convenience layer also own built-in code ids and registry lookup.

Benefits:

- slightly fewer modules in the short term

Costs:

- mixes built-in code catalog concerns with generic CSS construction
- makes `css.rs` responsible for both validation semantics and built-in naming
- blurs the separation between reusable constructors and shipped examples

This is not the recommended option.

### 3. Keep per-code constants and add a thin forwarding registry

Leave `steane.rs` as the owner of its check constants and make a registry that
forwards to those constants.

Benefits:

- smallest code diff

Costs:

- preserves duplicate ownership of built-in data
- does not satisfy the design goal that `Steane::new()` and registry lookup use
  one shared raw matrix source

This is not the recommended option.

## Decision

Add a built-in CSS registry module under `qec-code/src/codes/`, expose a
stable lookup function, and move built-in Steane check ownership there.

The first implementation should only ship `steane`, but the API must be shaped
so that future built-in CSS codes can be added without changing the lookup
contract.

## Module Structure

The `codes` module should grow from:

- `codes::steane`

to:

- `codes::built_in_css`
- `codes::steane`

Responsibilities:

- `codes::built_in_css`
  - defines the stable `BuiltInCssChecks` result type
  - owns built-in CSS row-support data
  - exposes lookup by `code_id`
- `codes::steane`
  - remains the ergonomic built-in constructor
  - no longer owns its own parity-check constants
  - lowers registry data through `CssCode::from_hx_hz(...)`

`css.rs` remains a constructor/validation layer for dense `Hx`/`Hz` matrices.
It does not become the home for built-in code ids.

## Public API

The registry should expose one public lookup function:

```rust
pub fn built_in_css_checks(code_id: &str) -> Result<BuiltInCssChecks>
```

The stable result shape should be:

```rust
pub struct BuiltInCssChecks {
    pub code_id: &'static str,
    pub num_cols: usize,
    pub hx: Vec<Vec<usize>>,
    pub hz: Vec<Vec<usize>>,
}
```

Design notes:

- `code_id` should be `&'static str` because built-in ids are compile-time
  constants and do not require allocation in the returned value.
- `hx` and `hz` should use row-support form instead of dense `Vec<Vec<u8>>`.
- each returned support row is canonical: sorted ascending and duplicate-free.
- the first implementation only recognizes `steane`.

The registry result should be owning data. Callers should not need to care
whether the implementation started from static slices or other internal
storage.

## Data Model and Lowering

The built-in registry should store canonical row-support constants for Steane.

Representative shape:

- `num_cols = 7`
- `hx = [[0, 3, 5, 6], [1, 3, 4, 6], [2, 4, 5, 6]]`
- `hz = [[0, 3, 5, 6], [1, 3, 4, 6], [2, 4, 5, 6]]`

The runtime data flow should be:

1. `built_in_css_checks("steane")` returns canonical support rows.
2. `Steane::new()` calls the registry function.
3. `Steane::new()` lowers support rows into dense binary matrices of width
   `num_cols`.
4. `Steane::new()` calls `CssCode::from_hx_hz(...)`.
5. `CssCode` lowers to the existing general `StabilizerCode`.

This keeps the public surface for this issue tight:

- built-in registry exposes reusable raw CSS data
- `CssCode` continues to accept dense matrices only
- `Steane::new()` becomes a consumer of the registry instead of a second source
  of truth

This design deliberately does not add a new public `CssCode` constructor for
row supports. That would widen the task from "add a built-in registry" to
"expand the CSS construction API", which issue #54 does not require.

## Error Handling

Unknown built-in ids should be rejected with a dedicated public error:

```rust
QecError::UnknownBuiltInCssCode { code_id: String }
```

Behavioral requirements:

- preserve the original user-provided id string in the error
- do not silently map unknown ids to a default code
- do not return a generic formatting or parse error for this case

No additional public error variant is needed for malformed built-in row
supports. Those supports are crate-owned constants, not user input. If the
registry constants are wrong, that is an implementation bug to be caught by
tests, not a runtime-recovery API surface.

## Canonicalization Policy

The registry must return canonical data, but it should not canonicalize at
lookup time.

Chosen policy:

- built-in row-support constants are authored in canonical form
- tests assert the canonical invariants
- lookup returns the stored canonical data unchanged

Rejected policy:

- sort rows on every lookup
- deduplicate entries on every lookup
- quietly repair malformed built-in rows at runtime

Runtime repair would hide bugs in crate-maintained constants and make tests
less meaningful.

## Testing and Verification

Acceptance should be anchored by the two issue-specified tests:

```text
cargo test -p qec-code --test code built_in_css_registry_exposes_steane_checks built_in_css_registry_rejects_unknown_code_id
```

The first test should verify:

- `built_in_css_checks("steane")` succeeds
- `code_id == "steane"`
- `num_cols == 7`
- `hx` equals the expected Steane support rows
- `hz` equals the expected Steane support rows
- each support row is sorted
- each support row has no duplicates

The second test should verify:

- unknown ids return `QecError::UnknownBuiltInCssCode { .. }`
- the error preserves the attempted id
- no default built-in code is returned

Existing Steane invariants should remain covered:

- `Steane::new()` still produces `n = 7`
- stabilizer rank remains `6`
- the code still has `k = 1`
- stabilizer rows still lower to the expected binary symplectic width

This preserves confidence in both the new lookup API and the existing built-in
constructor path.

## Out of Scope Follow-Ups

This design intentionally leaves these as later work:

- additional built-in CSS codes beyond `steane`
- a public helper for lowering support rows to dense matrices
- a public `CssCode::from_supports(...)` constructor
- non-CSS built-in code registries
- CLI inspection commands for built-in check data

Those extensions can be added later if real consumers appear, without changing
the core lookup contract established here.
