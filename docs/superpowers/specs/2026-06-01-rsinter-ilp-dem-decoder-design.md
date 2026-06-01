# Rsinter ILP DEM Decoder Design

Date: 2026-06-01
Status: Proposed
Scope: `rstim` workspace, new ILP decoder crate plus `rsinter` integration

## Summary

This design adds an integer-programming decoder for detector error models to
the current workspace.

The first production integration target is not `rstim` CLI code. It is
[`rsinter/src/decode.rs`](/Users/nzy/rcode/rstim/rsinter/src/decode.rs:1),
which already defines the repository's reusable decoder boundary for
DEM-driven shot decoding.

The recommended implementation is:

- add a new workspace crate that owns DEM-to-ILP lowering and solver backends
- add a thin `rsinter` adapter that implements `Decoder`
- make `Gurobi` the preferred backend when available
- fall back automatically to `HiGHS` when `Gurobi` is unavailable
- support general multi-detector and multi-observable `error(...)` terms
- treat `Separator` / `^` as a plain separator and ignore its alternative-path
  semantics in the first version

The goal is to ship a real end-to-end decoder path for existing `rsinter`
workflows without welding solver-specific logic directly into `rsinter`.

## Goals

- Add a real ILP-based DEM decoder to the workspace.
- Integrate the first usable version through the existing
  `rsinter::decode::Decoder` trait.
- Keep solver selection explicit:
  prefer `Gurobi`, otherwise use `HiGHS`.
- Separate three concerns cleanly:
  DEM lowering, solver backend, and `rsinter` adapter.
- Reuse a compiled model across many shots instead of rebuilding the full ILP
  for every syndrome.
- Preserve the current `rstim::dem::DetectorErrorModel` traversal semantics for
  `Repeat` and `ShiftDetectors`.
- Cover the current repository's existing DEM edge cases such as observable-only
  terms and exact-probability terms.

## Non-Goals

- Do not support the full meaning of Stim `^` separator segments in the first
  version. The first version intentionally discards that distinction.
- Do not promise competitive performance against MWPM-based decoders in the
  first phase.
- Do not change `rstim`'s DEM representation or parser.
- Do not make the first delivery a general-purpose public optimization crate.
- Do not block the open-source development path on commercial solver
  availability.
- Do not route `rstim` CLI commands directly through the new decoder in the
  first phase.

## Current State

The workspace already contains the boundaries needed for a clean decoder
integration, but not an ILP decoder implementation.

- [`rsinter/src/decode.rs`](/Users/nzy/rcode/rstim/rsinter/src/decode.rs:1)
  defines the decoding boundary:
  compile a decoder for a DEM, then decode bit-packed detector shots into
  bit-packed observable predictions.
- [`rsinter/src/rbposd_adapter.rs`](/Users/nzy/rcode/rstim/rsinter/src/rbposd_adapter.rs:1)
  already lowers `DetectorErrorModel` into a matrix-like decoding problem for a
  different decoder family. This file demonstrates the right integration layer
  and already contains repository-specific handling for:
  `Repeat`, `ShiftDetectors`, `Separator`, observable-only terms, and exact
  probabilities.
- [`rstim/src/dem.rs`](/Users/nzy/rcode/rstim/rstim/src/dem.rs:1)
  already provides the repository-owned DEM tree and traversal semantics.
- [`rmatching/src/decoder.rs`](/Users/nzy/rcode/rstim/rmatching/src/decoder.rs:1)
  shows the other existing `rsinter` decoder integration pattern.

This gives the project a stable target:

- the new decoder should plug into `rsinter`
- the solver and modeling logic should live outside `rsinter`
- the lowering semantics should stay aligned with the current DEM model instead
  of re-parsing DEM text in a second place

## Decision Summary

The chosen direction is:

- add a new workspace crate dedicated to ILP DEM decoding
- make that crate own the compiled problem representation and solver backends
- keep `rsinter` integration as a thin adapter layer
- use `Gurobi` first when the environment supports it
- fall back automatically to `HiGHS` on machines without `Gurobi`
- treat every DEM `error(...)` instruction as one ILP column after ignoring
  `Separator` markers

This is intentionally not the smallest possible patch. The extra structure is
worth it because the first version already needs two solver backends and a
compiled-model lifecycle.

## Alternatives Considered

### 1. Implement directly inside `rsinter`

This would be the shortest path to a passing demo:

- add `IlpDemDecoder` next to the existing decoder code
- put DEM lowering, solver setup, and shot decoding in the same crate

Benefits:

- smallest initial diff
- fastest route to end-to-end tests

Costs:

- mixes repository integration code with solver-specific modeling code
- makes `Gurobi` / `HiGHS` fallback logic harder to isolate and test
- makes later standalone benchmarks or backend reuse awkward

### 2. Add a new ILP decoder crate and keep `rsinter` thin

This option creates a new workspace crate, for example `rilpqec`, and keeps
`rsinter` focused on its existing adapter boundary.

Benefits:

- clean separation between lowering, solving, and integration
- natural home for backend-specific code and feature flags
- easier to benchmark and test independently

Costs:

- larger up-front patch
- requires one more crate in the workspace

This is the recommended option.

### 3. Use a generic modeling layer such as `good_lp` as the primary API

This option would standardize model construction through a generic solver
abstraction and dispatch to `Gurobi` or `HiGHS` beneath it.

Benefits:

- potentially shorter model-building code
- simpler backend abstraction on paper

Costs:

- weaker fit for a workflow that should compile once and then solve many
  syndromes by mutating the RHS
- less direct access to backend-specific capabilities and tuning
- likely to become a leaky abstraction if the project needs better reuse or
  performance later

This is not the recommended first implementation path.

## Success Criteria

The first delivery is successful only if all of the following are true:

- a new `rsinter` decoder implementation can compile from an existing
  `DetectorErrorModel`
- the decoder can consume bit-packed detector shots and return bit-packed
  observable predictions
- the compiled decoder chooses `Gurobi` when available and otherwise decodes
  using `HiGHS`
- existing repository edge cases are covered:
  multi-detector terms, multi-observable terms, observable-only terms,
  `probability = 0`, `probability = 1`, `Repeat`, `ShiftDetectors`, and
  `Separator` ignored as a plain separator
- repository tests prove the decoder works through `rsinter`, not only through
  unit tests of internal helpers

Performance is not the first-phase success gate. Model reuse across shots is
required, but no benchmark threshold is part of the first milestone.

## Recommended Architecture

The design is split into three layers.

### Layer 1: DEM Lowering

The new crate should expose a problem builder that converts
`rstim::dem::DetectorErrorModel` into a compiled ILP problem description.

Suggested internal output:

- `num_detectors`
- `num_observables`
- sparse detector columns, one column per error mechanism
- sparse observable columns, one column per error mechanism
- adjusted per-column probabilities or weights
- forced-syndrome bits induced by exact-probability columns
- baseline observable bits induced by exact-probability or observable-only
  columns

The lowering pass should follow the same repository-local interpretation
already established in the `rbposd` adapter:

- `DemTarget::Detector` toggles detector membership in the current column
- `DemTarget::Observable` toggles observable membership in the current column
- `DemTarget::Separator` is ignored
- `Repeat` recursively replays the body while carrying detector offsets
- `ShiftDetectors` mutates the current offset
- `Detector` and `LogicalObservable` annotation instructions do not generate
  columns

Each `error(probability)` instruction becomes one binary decision variable in
the ILP after target cancellation is applied.

### Layer 2: Solver Backend

The new crate should define a backend boundary around a compiled model.

Suggested shape:

- `BackendKind`:
  `Auto`, `Gurobi`, `Highs`
- `BackendConfig`:
  solver preference plus optional tuning such as time limit, MIP gap, threads,
  and verbosity
- `CompiledBackend` trait or enum-backed object:
  own a reusable compiled model and solve for a provided syndrome

The `Auto` path must implement:

- choose `Gurobi` if the feature is enabled and the environment is usable
- otherwise choose `HiGHS`

The backend boundary must hide solver-specific details from the `rsinter`
adapter. The adapter should not know whether the correction came from `Gurobi`
or `HiGHS`.

### Layer 3: `rsinter` Adapter

`rsinter` should add a new decoder adapter, for example `IlpDemDecoder`, that
implements `rsinter::decode::Decoder`.

Responsibilities:

- accept high-level decoder configuration
- compile the DEM once into the new crate's compiled decoder
- decode bit-packed detector shots by:
  unpacking to syndrome bits,
  applying forced syndrome toggles,
  solving for the correction,
  mapping the correction into observables,
  XORing baseline observable toggles,
  and packing the observable prediction bits back into bytes

This is the same outer workflow that the current `rbposd` adapter already
follows. The difference is that the correction is produced by an ILP solver
instead of BP+OSD.

## ILP Formulation

For a lowered DEM problem with `m` detectors and `n` error mechanisms:

- binary variables:
  `e_j in {0,1}` for each mechanism `j`
- integer auxiliary variables:
  `a_i >= 0` for each detector parity constraint `i`

Objective:

- minimize `sum_j w_j * e_j`

Constraints:

- `sum_j H[i,j] * e_j = s_i + 2 * a_i` for each detector `i`

Where:

- `H` is the binary detector-incidence matrix produced from the DEM
- `s` is the shot syndrome after applying any forced-syndrome toggles
- `w_j` is the column weight derived from the mechanism probability

Observable prediction is not part of the optimization variables. It is derived
after solving:

- `obs = baseline_obs XOR (O * e mod 2)`

where `O` is the observable-incidence matrix.

## Probability And Weight Handling

The first version should not minimize raw probabilities. It should minimize
log-likelihood-style weights derived from per-column probabilities.

Recommended mapping:

- for `0 < p < 0.5`:
  `w = ln((1 - p) / p)`
- for `p = 0`:
  drop the column entirely
- for `p = 1`:
  fold its detector and observable effect into forced baseline state
- for `p > 0.5`:
  complement the column by toggling the same detector and observable support
  into the forced baseline state and then use `1 - p` as the effective column
  probability

This preserves the maximum-likelihood meaning of the objective while keeping
all optimized columns in the range `0 < p <= 0.5`.

The implementation should clamp only for numerical safety near the endpoints,
not as a semantic shortcut.

## Data Flow

The intended end-to-end flow is:

1. `rsinter` receives a `DetectorErrorModel`
2. the new adapter lowers it into a compiled ILP problem through the new crate
3. the new crate compiles a solver-specific reusable model
4. `rsinter` receives bit-packed detector outcomes for one or many shots
5. each shot is unpacked into syndrome bits
6. forced-syndrome bits are XORed in
7. the backend solves for the binary correction vector
8. observables are computed from the correction plus baseline toggles
9. observable bits are packed back into the output buffer

The reusable boundary is the compiled decoder, not a one-shot solve call.

## Error Handling

The first version should be strict about configuration and DEM shape, but it
should not reject the repository-defined first-phase DEM subset.

Expected error classes:

- backend selection failure:
  neither `Gurobi` nor `HiGHS` is available
- backend compile failure:
  solver model construction fails
- solve failure:
  solver reports infeasible, unbounded, or internal failure
- dimension mismatch:
  provided shot dimensions do not match the compiled DEM dimensions
- invalid probability:
  DEM contains a probability outside `[0, 1]`

The decoder should not fail merely because a DEM contains `Separator` targets.
Those targets are explicitly ignored in this design.

The new crate should preserve rich internal error types. However, the current
`rsinter::decode::Decoder` trait does not return `Result` from
`compile_for_dem` or `decode_shots_bit_packed`.

The first version should therefore keep the current `rsinter` trait shape and
narrow backend compile or solve failures at the adapter boundary with precise
error context, even if that currently means a fail-fast path instead of typed
error propagation through `rsinter`.

Changing the `rsinter` decoder trait to return `Result` is a valid future
cleanup, but it is out of scope for this first design.

## Crate And Feature Layout

Recommended workspace change:

- add a new member crate, tentatively named `rilpqec`

Recommended dependency structure:

- `rilpqec` depends on `rstim`
- `rsinter` depends on `rilpqec`
- `rilpqec` must not depend on `rsinter`

Recommended feature layout:

- default feature enables `HiGHS`
- optional `gurobi` feature enables the `Gurobi` backend

This keeps the open-source path buildable by default while still supporting
the preferred commercial backend on equipped machines.

## Testing Strategy

Testing should be layered so that failures localize cleanly.

### Lowering Tests

Add unit tests in the new crate for:

- multi-detector columns
- multi-observable columns
- `Separator` ignored while preserving one combined column
- `Repeat` and `ShiftDetectors`
- duplicate detector or observable targets cancelling mod 2
- observable-only terms
- exact-probability terms:
  `0`, `1`, and `> 0.5`

### Backend Tests

Add backend-focused tests for:

- `Auto` chooses `HiGHS` when `Gurobi` is unavailable
- explicit `Highs` selection works
- explicit `Gurobi` selection reports a clear error when unsupported in the
  current build or environment
- a small compiled model can solve repeated syndromes without recompiling

These tests should stay small and deterministic.

### `rsinter` Integration Tests

Mirror the current style of
[`rsinter/tests/decode_rbposd.rs`](/Users/nzy/rcode/rstim/rsinter/tests/decode_rbposd.rs:1).

Required cases:

- simple single-observable prediction
- multi-observable prediction
- observable-only baseline term
- zero-syndrome case with baseline toggle
- exact-probability forced syndrome case
- `collect` pipeline smoke test using the new decoder name

## Rollout Plan

The implementation should land in three short phases.

### Phase 1: New Crate And Lowering Core

- add the new workspace crate
- implement DEM lowering and compiled problem representation
- add lowering tests

### Phase 2: Solver Backends

- add `HiGHS` backend first because it is the guaranteed open path
- add `Gurobi` backend behind a feature flag
- implement `Auto` backend selection
- add backend tests

### Phase 3: `rsinter` Adapter

- add `IlpDemDecoder`
- wire it into integration tests and `collect`
- add repository-level smoke coverage

This ordering keeps repository integration late enough that the core solver and
lowering logic can stabilize first, while still preserving the agreed primary
goal of `rsinter` integration.

## Open Questions Resolved By This Design

- Should the first version integrate through `rsinter` or start as an isolated
  crate?
  Answer:
  integrate through `rsinter`, but keep the ILP machinery in a separate crate.

- Should the first version require a commercial solver?
  Answer:
  no. Prefer `Gurobi`, but support automatic `HiGHS` fallback.

- Should the first version preserve `Separator` / `^` alternative semantics?
  Answer:
  no. Ignore `Separator` targets and treat the whole `error(...)` line as one
  mechanism after mod-2 target cancellation.

- Should the first version support only fully decomposed DEMs?
  Answer:
  no. Support general multi-detector and multi-observable terms within the
  repository-defined simplified interpretation above.
