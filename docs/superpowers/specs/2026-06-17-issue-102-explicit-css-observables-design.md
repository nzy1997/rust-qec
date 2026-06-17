# Issue 102 Explicit CSS Observables Design

Date: 2026-06-17
Status: Design approved in-session, written for review
Scope: GitHub issue #102, explicit logical observables for general CSS memory benchmarks

## Summary

Issue #102 makes the existing general CSS memory benchmark path safe for
paper-facing `k > 1` CSS codes, especially the published bivariate-bicycle
`[[72,12,6]]` instance needed by AutoQEC issue nzy1997/AutoQEC#18.

PR #51 already added `rstim::codegen::css`, `rstim gen --code css --task
memory`, and `rsinter input_type = "css"` with an optional `observables` file.
The remaining gap is semantic: explicit observable rows are currently checked
as data-qubit supports, but they are not validated as logical observables of
the selected memory basis. For a `k = 12` BB memory benchmark, that is too
implicit to compare against published logical-memory curves.

The chosen design keeps the existing observable file format and upgrades its
contract. For `basis = "x"`, the file contains X-like logical supports. For
`basis = "z"`, it contains Z-like logical supports. Each row is validated
against the CSS code before circuit construction completes, and `rsinter`
records that completed CSS benchmark rows use any-logical failure aggregation.

This work is independent of issue #101 / PR #104. The exact CSS distance CLI
is useful for AutoQEC but not required for this observable-semantics change.

## Goals

This milestone should:

1. Define stable selected-basis semantics for explicit CSS observable files.
2. Validate explicit observable rows before DEM generation, decoding, or
   sampling.
3. Support `basis = "x"` and `basis = "z"` with clear logical selection rules.
4. Require explicit rows to define exactly `k` independent logical observables
   modulo the relevant CSS stabilizer span.
5. Make the existing `rsinter` logical error metric explicit as any-logical
   failure per shot.
6. Add BB72 explicit observable fixtures and smoke coverage for `k = 12`.
7. Preserve existing surface-code benchmark behavior and the current CSS
   matrix/observable JSON wrapper format.

## Non-Goals

This milestone should not:

1. Add per-logical failure-rate metrics.
2. Introduce a new versioned observable schema with separate `logical_x` and
   `logical_z` fields.
3. Change decoder interfaces or DEM observable-bit handling.
4. Require issue #101's exact-distance CLI implementation.
5. Change AutoQEC campaign, report, or promotion code.
6. Remove canonical fallback for small or exploratory CSS runs.

Per-logical metrics may be useful later, but issue #102 only needs a stable
paper-facing definition for the logical-memory benchmark rows currently emitted
by `rsinter`.

## Current State

The relevant implementation is already in place:

- `rstim::codegen::css::parse_css_matrix_json(...)` and
  `parse_css_observable_json(...)` parse dense and sparse-row JSON wrappers.
- `CssMemoryConfig` accepts `CssObservableSource::Explicit(rows)`,
  `CanonicalFallback`, and `ExplicitOrCanonical(rows)`.
- `css_memory(...)` validates matrix supports and CSS orthogonality, then emits
  selected-basis final data measurements and `OBSERVABLE_INCLUDE` instructions.
- `rsinter::bench::registry` expands `input_type = "css"` TOML specs with
  `hx`, `hz`, `basis`, optional `schedule`, optional `observables`, and
  optional `code_id`.
- `rsinter::bench::circuit_source::build_css(...)` reads those files, passes
  explicit rows to `css_memory(...)`, and records file paths in result params.
- `rsinter::bench::runners` compares the decoder-predicted observable bitset
  to the sampled observable-flip bitset. If the bitsets differ for a shot, that
  shot contributes one logical error.

The gap is that explicit rows are not currently checked to be logical
operators. The generator rejects wrong widths and out-of-range supports, but a
stabilizer row, a dependent duplicate logical, or a row that anticommutes with
the opposite CSS checks can still be accepted until downstream behavior becomes
ambiguous or silently wrong.

## Alternatives Considered

### 1. Metadata-only clarification

Document the current `observables` file behavior and add result metadata for
source and aggregation semantics.

Benefits:

- very small change
- minimal risk to the circuit generator

Costs:

- invalid logical rows still pass
- does not meet issue #102's pre-sampling validation requirement
- not strong enough for BB72 published-reference checks

This option is insufficient.

### 2. Selected-basis logical rows

Keep the existing observable file shape:

```json
{
  "format": "sparse_rows",
  "num_cols": 72,
  "rows": [[0, 5, 17], [2, 8, 41]]
}
```

Interpret rows according to the benchmark basis:

- `basis = "x"` means the rows are X-like logical supports.
- `basis = "z"` means the rows are Z-like logical supports.

Validate rows against `hx` and `hz`, require exactly `k` independent logical
classes, and record any-logical aggregation metadata.

Benefits:

- preserves PR #51's interface
- directly satisfies issue #102
- gives AutoQEC a stable format it can generate today
- keeps code-theory validation near the existing CSS generator
- avoids a new schema before there is a second observable-file consumer

Costs:

- file meaning depends on the surrounding `basis` parameter
- a future consumer needing both X and Z logicals in one file may want a richer
  schema

This is the chosen approach.

### 3. Versioned X/Z observable schema

Introduce a new schema such as:

```json
{
  "format": "css_logical_observables_v1",
  "num_cols": 72,
  "logical_x": [[...]],
  "logical_z": [[...]]
}
```

Benefits:

- most explicit standalone artifact
- can carry both X and Z logical sets at once

Costs:

- larger parser and compatibility surface
- existing CLI/TOML examples need more churn
- not necessary for current AutoQEC generation

This can be added later if AutoQEC or another tool needs a standalone logical
bundle.

## Decision

Use selected-basis logical rows with the existing `sparse_rows` and `dense`
wrappers.

The observable file itself remains only a matrix wrapper. The memory experiment
defines how to interpret it:

```toml
[runner.params]
input_type = "css"
code_id = "bivariate-bicycle-code-m6-n6"
hx = "input/hx.css.json"
hz = "input/hz.css.json"
observables = "input/logicals_x.css.json"
basis = "x"
schedule = "greedy"
rounds = [3]
p = [0.01]
```

For a CSS code with `n` columns and `k` logical qubits:

- X-basis observables are binary supports of X-like Pauli operators.
- Z-basis observables are binary supports of Z-like Pauli operators.
- Explicit rows must be non-empty as a set, within width, duplicate-free, in
  the selected-basis normalizer, not in the selected-basis stabilizer span, and
  independent modulo that stabilizer span.
- The explicit row count and quotient rank must both be `k`.

The current canonical fallback remains available. It should continue to derive
canonical logicals from `qec-code` and select X-like representatives for
memory-X or Z-like representatives for memory-Z.

## Logical Validation

Validation belongs in `rstim::codegen::css`, close to `resolve_observables`.
That function already has the matrix supports, memory basis, and selected
observable source. It should construct or reuse a `qec_code::css::CssCode` once
after support validation and orthogonality checks.

For `basis = "x"`, each explicit row must satisfy:

- it has width `n` after conversion to a dense binary vector
- it commutes with every Z check, equivalently it lies in `nullspace(hz)`
- it is not in `rowspan(hx)`
- the set of rows increases rank modulo `rowspan(hx)` until rank `k`

For `basis = "z"`, use the dual conditions:

- it commutes with every X check, equivalently it lies in `nullspace(hx)`
- it is not in `rowspan(hz)`
- the set of rows increases rank modulo `rowspan(hz)` until rank `k`

The implementation should reuse existing public
`qec_code::binary::try_in_row_span(...)` and `try_binary_rank(...)` utilities.
Keeping the high-level observable validation in `rstim::codegen::css` avoids
expanding `qec-code`'s public API for this milestone.

`CssCodegenError` should gain variants for logical-observable failures, with
messages that name the row and basis:

- `observable 2 is not an X logical: anticommutes with hz row 7`
- `observable 0 is an X stabilizer, not a logical`
- `explicit X observables define rank 10, expected k = 12`

Exact wording can follow local style, but the messages must identify whether
the failure is width/support, normalizer membership, stabilizer membership, or
logical-rank mismatch.

## Result Semantics

`rsinter` already aggregates logical failures as any-logical per shot:

```text
logical_errors += 1 if predicted_observable_bitset != sampled_observable_bitset
```

Issue #102 should make this explicit in result metadata instead of changing the
metric. Completed CSS result rows should include:

- `params.logical_observable_source`: `"explicit"` when `observables` is
  provided, otherwise `"canonical_fallback"`
- `params.logical_observable_basis`: `"x"` or `"z"`
- `params.logical_failure_aggregation`: `"any_logical"`
- `case_summary.num_obs`: already present through the runner path
- `case_summary.logical_observable_count`: equal to `num_obs`

The existing `params.observables` path can remain for provenance. If no
observable file is provided, it can continue to record `"canonical_fallback"`.

No per-logical metrics are added in this issue.

## CLI And Benchmark Flow

`rstim gen --code css --task memory` keeps the current arguments:

```text
--hx <path>
--hz <path>
--basis x|z
--schedule greedy|sequential
--observables <path>
```

If `--observables` is present, `run_css_gen(...)` should pass
`CssObservableSource::Explicit(rows)` exactly as it does now; `css_memory(...)`
performs the stronger validation. If validation fails and `--out` was provided,
the output file must not be overwritten. Existing tests already cover this
pattern for other CSS validation errors and should be extended for invalid
observable semantics.

`rsinter` keeps the same TOML interface. `build_css(...)` still reads the file,
checks width, and calls `css_memory(...)`. Logical validation errors should
surface as benchmark setup errors before the runner writes a completed row.

## Fixtures

Add BB72 explicit observable fixtures under `rsinter/tests/fixtures/css`:

- `bb72_hx.json`
- `bb72_hz.json`
- `bb72_logicals_x.json`

`qec-code/tests/fixtures/css` already contains `bb72_hx.json` and
`bb72_hz.json`; implementation should copy those JSON payloads into the
`rsinter` fixture area so the benchmark tests are self-contained. The new
`bb72_logicals_x.json` should define 12 independent X-like logical rows.

Do not add a large BB72 Z-logical fixture in this milestone. The library tests
should cover the Z-basis validator on a smaller code such as Steane.

## Testing

Add focused tests in `rstim/tests/css_codegen.rs`:

- valid explicit X logicals pass for Steane
- valid explicit Z logicals pass for Steane
- a row that anticommutes with the opposite checks fails
- a selected-basis stabilizer row fails
- duplicate or dependent logical rows fail the quotient-rank check
- an explicit set with too few rows fails with expected `k`
- BB72 explicit X logicals produce 12 observables and a DEM with 12
  observables

Extend `rstim/tests/cli_gen.rs`:

- invalid explicit observable semantics fail
- `--out` is not overwritten on that failure

Extend `rsinter` tests:

- a CSS benchmark row with explicit Steane observables records
  `logical_observable_source = "explicit"` and
  `logical_failure_aggregation = "any_logical"`
- a CSS benchmark without an observable file records
  `logical_observable_source = "canonical_fallback"`
- a BB72 CSS smoke fixture with explicit observables runs through a tiny
  `rmatching` benchmark and reports `num_obs = 12`
- a deliberately invalid observable fixture fails before a completed result is
  produced

Keep existing regression coverage passing:

- legacy surface benchmark fixtures
- Steane CSS smoke fixture
- rotated-surface CSS special-case smoke
- BB72 DEM smoke from PR #51

## Acceptance Criteria

Issue #102 is complete when:

1. The existing CSS observable JSON wrapper has documented selected-basis
   logical semantics.
2. Invalid explicit observables fail clearly before sampling.
3. `basis = "x"` and `basis = "z"` select X-like and Z-like logical supports
   respectively.
4. Explicit observable rows must define exactly `k` independent logical
   classes modulo stabilizers.
5. A rotated-surface `d = 3` CSS benchmark can run with explicit observables
   and preserves the existing general-CSS smoke behavior.
6. The BB72 `[[72,12,6]]` instance can run through `rsinter input_type =
   "css"` with explicit observables and `num_obs = 12`.
7. Completed result rows identify explicit versus fallback observable source
   and record `logical_failure_aggregation = "any_logical"`.

## Open Follow-Up

A future issue can add a richer logical-observable artifact format carrying
both X and Z logical sets plus labels or citations. That format would be useful
for standalone code-library interchange, but it is not required for the
selected-basis memory benchmark contract in issue #102.
