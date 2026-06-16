# Randomized CSS Distance Upper Bound Design

Date: 2026-06-16
Status: Draft accepted in-session, written for review
Scope: `qec-code` API and CLI support for randomized CSS distance upper bounds

## Summary

Issue 76 adds a `randomized-upper-bound` distance method for CSS codes. The
method searches for low-weight nontrivial logical operators and reports the best
weight found as an upper bound on code distance. It must never be represented as
an exact distance.

The chosen design adds a typed upper-bound path in `qec-code` beside the
existing exact `compute_distance` API:

- exact distance keeps returning `DistanceResult`
- randomized upper-bound search returns a distinct `DistanceBoundResult`
- JSON output always includes `method: "randomized-upper-bound"` and
  `bound_type: "upper"`
- the CLI supports both built-in CSS code IDs and `--hx`/`--hz` JSON matrix
  inputs
- the first algorithm is conservative and deterministic under a fixed seed

## Goals

This milestone should:

1. Add a reusable library API for randomized CSS upper-bound distance search.
2. Add a `qec-code` CLI path that emits machine-readable JSON.
3. Support built-in CSS code IDs and external `hx`/`hz` JSON files.
4. Make randomized results impossible to confuse with exact distance results.
5. Make runs reproducible for the same input and seed.
6. Fail before producing a completed result for malformed options or malformed
   inputs.
7. Provide validation helpers for downstream consumers and tests.

## Non-Goals

This milestone should not:

1. Add certified lower bounds.
2. Add another exact distance backend.
3. Support non-CSS codes through this randomized method.
4. Add AutoQEC-side ingestion, report rendering, or provenance persistence.
5. Implement annealing, local improvement, or other advanced heuristic search in
   the first version.
6. Change the semantics of the existing exact `compute_distance` API.

## Current State

`qec-code` already owns the stabilizer and CSS algebra needed for the feature:

- `CssCode::from_hx_hz` validates CSS orthogonality and builds a
  `StabilizerCode`.
- `StabilizerCode` exposes stabilizer rows and canonical logical basis
  extraction.
- `distance::compute_distance` returns exact `DistanceResult` values, using ILP
  when `distance-ilp-highs` is enabled and exhaustive search otherwise.
- the CLI currently supports `code steane distance` for exact distance and
  `code css <code_id> hx|hz` for built-in CSS matrix export.

The missing pieces are a bound-specific result type, a randomized search path, a
CSS matrix JSON input path, and validation that keeps upper-bound results
separate from exact distance results.

## Alternatives Considered

### 1. Typed upper-bound backend inside `qec-code`

Add a new randomized distance module, options type, result type, validator, and
CLI command. Keep exact and randomized results distinct.

Benefits:

- best fit for AutoQEC and future library consumers
- avoids confusing exact distance and upper bounds
- allows JSON validation to be shared by CLI tests and downstream tools
- keeps future search improvements behind the same API

Costs:

- introduces a few new public types
- requires CLI parsing for CSS matrix files in addition to built-in IDs

This is the chosen approach.

### 2. Extend the existing exact `DistanceResult`

Add fields such as `method` and `bound_type` to the current exact result and use
the same type for randomized results.

Benefits:

- smaller short-term diff
- fewer CLI result shapes

Costs:

- makes it easier for consumers to treat an upper bound as exact
- forces exact distance callers to carry bound-specific fields
- weakens the safety property that issue 76 explicitly asks for

This approach is rejected.

### 3. CLI-only helper

Add a command that computes randomized JSON output without a clean library API.

Benefits:

- fastest command-line demo
- minimal public API surface

Costs:

- weak integration story for AutoQEC
- test and validation logic would be harder to reuse
- likely to require another refactor immediately after the issue is closed

This approach is rejected.

## Architecture

The implementation should add a new distance-bound path in `qec-code` without
changing exact distance semantics.

Recommended module shape:

- `qec-code/src/distance_bound.rs`
  - public result types
  - public options types
  - `randomized_css_upper_bound`
  - result validation helpers
- `qec-code/src/cli.rs`
  - new CSS distance command
  - JSON matrix loading
  - JSON result output
- `qec-code/src/css.rs`
  - minimal helpers for converting sparse JSON rows to dense `hx`/`hz`, if that
    keeps CLI parsing small and testable

The existing `distance::compute_distance` function remains an exact-distance
entry point and should not return randomized results.

## Public API

Add option and result types with a bound-specific shape:

```rust
pub struct RandomizedUpperBoundOptions {
    pub iterations: usize,
    pub restarts: usize,
    pub seed: u64,
    pub target_weight: Option<usize>,
}

pub struct DistanceBoundResult {
    pub status: BoundSearchStatus,
    pub method: DistanceBoundMethod,
    pub bound_type: BoundType,
    pub upper_bound: usize,
    pub logical_class: LogicalClass,
    pub witness: Pauli,
    pub options: RandomizedUpperBoundOptions,
    pub provenance: DistanceBoundProvenance,
}
```

The result should be serializable to JSON where completed randomized results
include:

- `status: "completed"`
- `method: "randomized-upper-bound"`
- `bound_type: "upper"`
- `upper_bound`
- `logical_class`
- `witness`
- `options`
- `provenance`

`DistanceBoundResult` is a completed-result type: `status` is serialized as
`"completed"` and `upper_bound` is always numeric. If no witness is found, the
API returns an explicit error and the CLI writes no stdout.

## CLI

Add a CLI command for CSS distance bounds. The exact spelling can follow local
`clap` patterns, but the intended user shape is:

```sh
qec-code code css-distance randomized-upper-bound --code-id steane --iterations 1000 --seed 7 --json
qec-code code css-distance randomized-upper-bound --hx hx.json --hz hz.json --iterations 1000 --seed 7 --json
```

Rules:

- exactly one input mode is required: `--code-id` or `--hx` plus `--hz`
- `--json` is required for the first version, so the command has one stable
  machine-readable output contract
- `--iterations` and `--seed` are required
- `--restarts` defaults to `1`
- `--target-weight` is optional
- successful completed runs write JSON to stdout and nothing to stderr
- invalid options or malformed inputs write a clear error to stderr and nothing
  to stdout

The external matrix format should accept the existing sparse-rows JSON fixture
shape:

```json
{
  "format": "sparse_rows",
  "num_cols": 7,
  "rows": [[0, 3, 5, 6]]
}
```

Sparse rows are the only required external matrix file format for the first
implementation because existing fixtures already use them. Dense binary matrix
JSON is a future extension and should be rejected with an explicit unsupported
format error until it is deliberately added with tests.

## Algorithm

The first implementation should use a conservative seeded randomized search:

1. Validate options before starting search:
   - `iterations > 0`
   - `restarts > 0`
   - `target_weight`, when present, is greater than zero
2. Build `CssCode` from `hx` and `hz`, then use the underlying `StabilizerCode`.
3. Extract the canonical logical basis and stabilizer rows.
4. Initialize a small deterministic PRNG from `seed`.
5. For each restart and iteration:
   - sample a nonzero logical coefficient vector
   - sample stabilizer coefficients
   - combine logical and stabilizer rows over GF(2)
   - convert the candidate row to a `Pauli`
   - validate the witness commutes with stabilizers and is not in the stabilizer
     span
   - update the best witness if the candidate has lower weight
6. Stop early if `target_weight` is reached or beaten.
7. Return a completed upper-bound result only when a witness is found.

The search should not claim optimality. It only reports the best nontrivial
logical operator found.

## Error Handling

The following should fail before producing a completed result:

- malformed JSON
- unsupported matrix JSON format
- dense matrix JSON until that format is deliberately added
- sparse supports out of range
- duplicate sparse supports
- mismatched `hx`/`hz` widths
- non-orthogonal CSS checks
- `iterations = 0`
- `restarts = 0`
- `target_weight = 0`
- no logical qubits
- no witness found after the configured search

No-witness completion must not invent a numeric value.

## Validation

Add validation helpers that can be used by tests and downstream consumers:

- randomized results must have `method: "randomized-upper-bound"`
- randomized results must have `bound_type: "upper"`
- completed randomized results must have a positive numeric `upper_bound`
- `upper_bound` must equal the witness weight
- the witness must be nontrivial for the input code
- optional known-distance validation rejects `upper_bound < known_exact_distance`

These helpers give the negative controls from issue 76 a single enforcement
point.

## Testing

Add focused tests for:

1. API reproducibility: same input, options, and seed gives the same upper bound
   and witness.
2. API validity: different seeds may differ, but every completed result validates
   as an upper bound.
3. Pinned Steane run recovers upper bound `3`.
4. Small CSS fixtures with known distances recover equality under pinned
   options.
5. CLI built-in mode emits completed JSON for `--code-id steane`.
6. CLI file mode accepts sparse-rows `hx`/`hz` JSON fixtures.
7. Invalid options fail before stdout is written.
8. Validator rejects:
   - `bound_type: "exact"`
   - missing or wrong method
   - a mocked upper bound below a known exact distance

The BB `[[72,12,6]]` fixture should be added as a test target when that fixture
is available to the distance test suite. It is not required to block the first
implementation if the fixture is absent.

## Rollout

The first implementation should land in small steps:

1. Add bound result/options/validation types.
2. Add CSS matrix JSON parsing for sparse rows.
3. Add seeded randomized search API.
4. Add CLI command and JSON output.
5. Add known-fixture and negative-control tests.
6. Run `cargo test -p qec-code` and any feature-gated distance tests relevant to
   the touched code.

Future improvements can add stronger search strategies under the same method
or under a versioned strategy field, but they should preserve the result
contract that randomized distance is an upper bound, not exact.
