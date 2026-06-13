# General CSS Codegen Design

Date: 2026-06-13
Status: Draft accepted in-session, written for review
Scope: GitHub issue #46, general CSS syndrome-extraction circuit generation and benchmark integration

## Summary

Add a general CSS-code memory experiment generator that accepts parity-check
matrices `hx` and `hz`, produces an `rstim` circuit, and lets the existing
sampler, detector-error-model analyzer, and decoder benchmark stack consume
that circuit. The feature should support both explicit logical observables and
automatic canonical-logical fallback, expose a CLI path for generated circuits,
and let `rsinter` benchmarks use CSS matrix inputs instead of only rotated
surface-code parameters.

The implementation should be layered: `qec-code` validates CSS code-theory data
and derives canonical logicals, `rstim::codegen::css` turns normalized data into
Stim IR, and `rsinter` selects a circuit source before running existing decoder
logic.

## Current State

`rsinter` currently builds benchmark circuits through
`rstim::codegen::surface_code::rotated_memory_x(distance, rounds, p)` and then
uses `ErrorAnalyzer::circuit_to_dem_decomposed` plus `sample_batch`. That path
works for surface-code benchmarks but cannot evaluate arbitrary CSS candidates
from parity-check matrices.

The workspace already has useful pieces:

- `qec-code::css::CssCode::from_hx_hz` validates dense binary CSS checks and
  rejects non-orthogonal `hx * hz^T`.
- `qec-code` can derive canonical logical bases for stabilizer codes.
- `rstim::codegen` already has surface, color, and repetition generators.
- `rstim` has CLI generation through `rstim gen`.
- `rsinter` has benchmark specs and decoder runners, but runner points are
  surface-specific.

## Goals

This feature should:

1. Add `rstim::codegen::css::css_memory(...) -> Result<Vec<StimInstr>, _>`.
2. Accept CSS checks in dense or sparse-row JSON wrappers.
3. Support memory-X and memory-Z experiments.
4. Support explicit logical observable supports and canonical fallback.
5. Support both sequential and greedy CNOT schedules, defaulting to greedy for
   user-facing paths.
6. Add `rstim gen --code css --task memory ...` for CSS circuit generation.
7. Add `rsinter` benchmark input support for CSS matrix files.
8. Preserve existing surface-code generator and benchmark behavior.

## Non-Goals

This v1 should not:

1. Optimize physical layout or produce geometry-aware coordinates.
2. Promise surface-code circuit equivalence at the exact instruction ordering
   level.
3. Add a full public matrix framework to `rstim`.
4. Add new decoder algorithms.
5. Solve logical selection heuristics beyond explicit supports and canonical
   `qec-code` fallback.
6. Require existing surface benchmark TOML files to change.

## Architecture

The design uses three ownership boundaries.

### `qec-code`

`qec-code` remains the authority for code-theory validation and canonical
logical derivation. `rstim` should add a workspace dependency on `qec-code`
rather than duplicating orthogonality validation or canonical-logical logic.

The CSS codegen path should use `qec-code::css::CssCode::from_hx_hz` after
normalizing matrix-file inputs into dense binary matrices. If canonical
fallback is requested, it should call the existing logical-basis API and select
logical `X` operators for memory-X or logical `Z` operators for memory-Z.
Fallback should accept only representatives that are pure in the measured
basis: memory-X observables must have empty Z support, and memory-Z observables
must have empty X support. If canonical derivation returns mixed
representatives for a CSS input, the generator should return a clear error and
ask the caller to supply explicit observables.

### `rstim::codegen::css`

`rstim::codegen::css` owns the library generator:

- matrix-file normalization types
- public CSS memory configuration
- scheduler selection
- circuit emission
- detector construction
- observable construction
- CSS-specific errors

It should return `Result<Vec<StimInstr>, CssCodegenError>` instead of panicking
on user input.

### `rsinter`

`rsinter` should introduce a circuit-source abstraction so decoder runners can
operate on a generated circuit without knowing whether it came from a rotated
surface generator or CSS matrix files. The existing surface benchmark behavior
should remain the default when `input_type` is absent.

## Public API

The concrete names may be adjusted during implementation to match local style,
but the API should have these concepts:

```rust
pub enum MemoryBasis {
    X,
    Z,
}

pub enum CssSchedule {
    Sequential,
    Greedy,
}

pub struct CssCheckMatrices {
    pub hx: Vec<Vec<usize>>,
    pub hz: Vec<Vec<usize>>,
    pub num_data_qubits: usize,
}

pub enum CssObservableSource {
    Explicit(Vec<Vec<usize>>),
    CanonicalFallback,
    ExplicitOrCanonical(Vec<Vec<usize>>),
}

pub struct CssMemoryConfig {
    pub checks: CssCheckMatrices,
    pub rounds: usize,
    pub noise: NoiseParams,
    pub basis: MemoryBasis,
    pub schedule: CssSchedule,
    pub observables: CssObservableSource,
}

pub fn css_memory(config: CssMemoryConfig) -> Result<Vec<StimInstr>, CssCodegenError>;
```

`CssCheckMatrices` stores sparse supports internally because circuit emission
uses supports directly. Validation still converts to dense matrices for
`qec-code::CssCode`.

## JSON Input

Matrix files should use explicit wrappers so dense and sparse encodings are
unambiguous.

Dense:

```json
{
  "format": "dense",
  "rows": [[1, 0, 1], [0, 1, 1]]
}
```

Sparse row supports:

```json
{
  "format": "sparse_rows",
  "num_cols": 3,
  "rows": [[0, 2], [1, 2]]
}
```

The parser should reject:

- unknown formats
- dense rows with non-binary entries
- ragged dense rows
- sparse rows with repeated indices
- sparse rows with out-of-range indices
- `hx` and `hz` with different widths
- empty-width inputs

Logical observable files should use the same sparse-row support convention:

```json
{
  "format": "sparse_rows",
  "num_cols": 72,
  "rows": [[0, 5, 17], [2, 8, 41]]
}
```

For `k > 1`, each row becomes one observable index in the output circuit.

## Circuit Semantics

The generated memory circuit uses:

- one data qubit per matrix column
- one ancilla per `hx` row
- one ancilla per `hz` row
- stable qubit ordering: data first, then X-check ancillas, then Z-check
  ancillas
- stable check ordering: all X checks in input order, then all Z checks in
  input order

All checks are measured every round. X-check ancillas are measured through the
standard basis-change pattern:

1. apply `H` to X-check ancillas
2. apply CNOT interactions for X checks and Z checks
3. apply `H` to X-check ancillas
4. measure-reset ancillas with `MR`

For X checks, CNOT direction is ancilla to data. For Z checks, CNOT direction is
data to ancilla. This matches the existing surface-code convention.

Noise should follow the existing `NoiseParams` semantics:

- `before_round_data_depolarization`: `DEPOLARIZE1` on data at the start of a
  round
- `after_clifford_depolarization`: `DEPOLARIZE1` after ancilla H layers and
  `DEPOLARIZE2` after CNOT layers
- `before_measure_flip_probability`: `X_ERROR` before ancilla and final data
  measurement
- `after_reset_flip_probability`: `X_ERROR` after data reset and ancilla `MR`
  reset

Data reset and final data measurement use the selected memory basis:

- memory-X: `RX` at the start and `MX` at the end
- memory-Z: `R` at the start and `M` at the end

## Detectors And Observables

First-round detectors are emitted only for checks deterministic under the
chosen memory basis:

- memory-X: X-check detectors
- memory-Z: Z-check detectors

Later rounds emit detectors for every check, comparing current and previous
ancilla measurements.

Tail detectors are emitted for the selected-basis checks. Each tail detector is
the parity of:

- final data measurements on that check support
- the last ancilla measurement for that check

Final data measurement records on each selected logical support are emitted as
`OBSERVABLE_INCLUDE` targets. Explicit logical rows are preferred when present.
When canonical fallback is used, the generator selects all canonical logical-X
supports for memory-X and all canonical logical-Z supports for memory-Z.

## Scheduling

Two schedules should be supported.

`Sequential` emits one CNOT interaction per layer with a `TICK` between
layers. It is deterministic and useful for debugging.

`Greedy` packs non-conflicting CNOT interactions into deterministic layers. It
walks checks and support indices in stable order, placing each interaction into
the first layer that does not already use either qubit. This reduces depth for
LDPC inputs while keeping scheduling logic simple and testable.

The default for CLI and `rsinter` should be `Greedy`. Tests should exercise both
schedules.

## CLI

The existing common generator path should remain unchanged:

```sh
rstim gen --code surface_code --task rotated_memory_x --distance 3 --rounds 3
```

CSS generation should be a parallel path under the same subcommand:

```sh
rstim gen \
  --code css \
  --task memory \
  --hx hx.json \
  --hz hz.json \
  --basis x \
  --rounds 3 \
  --after_clifford_depolarization 0.001 \
  --schedule greedy \
  --observables logicals.json \
  --out circuit.stim
```

CSS mode does not use `--distance`. During implementation, the CLI argument
shape should be adjusted so legacy generators still require distance while CSS
generation requires `--hx`, `--hz`, `--basis`, and `--rounds`.

If `--observables` is absent, the CLI should use canonical fallback. The CLI
should return clear errors for invalid matrices, non-orthogonal checks,
out-of-range logical supports, invalid basis values, invalid schedules, and
logical fallback failures.

## `rsinter` Integration

Runner params should gain `input_type`.

Existing specs omit `input_type` and continue to mean rotated surface
memory-X:

```toml
[runner.params]
distance = [3]
rounds = [3]
p = [0.002, 0.005, 0.010]
max_shots = 2000
max_errors = 20
batch_size = 256
```

CSS specs use matrix paths:

```toml
[runner.params]
input_type = "css"
code_id = "bb72"
hx = "codes/bb72/hx.json"
hz = "codes/bb72/hz.json"
basis = "x"
rounds = [3]
p = [0.002]
schedule = "greedy"
observables = "codes/bb72/logicals_x.json"
max_shots = 2000
max_errors = 20
batch_size = 256
```

The benchmark runner should build the circuit once per point, derive the DEM
with `ErrorAnalyzer::circuit_to_dem_decomposed`, and then run the existing
sampling and decoder flow unchanged.

Result rows should include enough params or case summary metadata to identify
CSS inputs:

- `input_type`
- `code_id` when supplied
- `basis`
- `schedule`
- `rounds`
- `p`
- matrix path strings or stable matrix identifiers

## Error Handling

The CSS path should use structured internal errors and convert to strings at
CLI and benchmark boundaries. Error messages should name the failing field when
possible, for example:

- `hx row 2 has width 5, expected 7`
- `hz row 0 contains out-of-range column 72 for width 72`
- `CSS X/Z checks are not orthogonal`
- `observable 3 references data qubit 91, but width is 72`
- `canonical logical 4 is not pure in memory-X basis`
- `canonical logical fallback produced no observables`

Library APIs should not panic on malformed CSS input.

## Verification

### Library Tests

Add focused `rstim` tests for:

- dense and sparse JSON wrappers normalizing to the same supports
- non-orthogonal CSS checks returning a clear error
- explicit observables producing the expected observable count
- canonical fallback working on Steane
- sequential and greedy schedules both compiling through
  `ErrorAnalyzer::circuit_to_dem_decomposed`

### Surface Special Case

Add test helpers or fixtures that construct rotated-surface-style `hx/hz` for
`d = 3` and `d = 5`. The generated CSS memory circuit does not need exact
instruction equality with `rotated_memory_x`, but it should match the known
special case at the behavioral level:

- same effective detector count
- same observable count
- DEM generation succeeds
- a small fixed-seed `rmatching` smoke comparison gives logical error rates
  compatible with the rotated baseline under a smoke-sized binomial tolerance

### BB Smoke

Add a BB `[[72,12,6]]` fixture or deterministic generator. Verify:

- CSS codegen succeeds
- DEM generation succeeds
- detector count matches the expected circuit construction
- observable count is 12 when canonical or explicit 12-logical input is used

### CLI And Benchmark Tests

Add tests that:

- `rstim gen --code css --task memory ...` emits parseable circuit text
- a tiny `rsinter` CSS benchmark spec runs through one decoder smoke case
- the legacy minimal surface benchmark fixture still runs unchanged

## Acceptance Criteria

The issue is complete when:

1. `rstim::codegen::css::css_memory` can generate DEM-compatible circuits from
   valid CSS `hx/hz` inputs.
2. Non-commuting CSS checks return a clear error.
3. Explicit and canonical logical-observable paths are both tested.
4. `rstim gen --code css --task memory` works with dense and sparse JSON
   matrix files.
5. `rsinter` can benchmark a CSS input from a TOML spec.
6. Existing surface-code generator and benchmark tests still pass.
7. The `d = 3, 5` surface special case and BB `[[72,12,6]]` smoke checks pass.

## Known Limitations

The generated CSS coordinates are generic and index-based. They are intended to
make circuits inspectable and deterministic, not to represent a physical
embedding or planar layout.

Greedy scheduling is a deterministic depth reduction, not a hardware-aware
scheduler. More advanced scheduling can be added later behind the same
`CssSchedule` boundary.
