# QEC Code ILP Distance Design

Date: 2026-06-15
Status: Draft accepted in-session, written for review
Scope: `qec-code` exact distance computation for general stabilizer codes

## Summary

This design upgrades `qec-code` distance computation from exhaustive Pauli
enumeration to an ILP-backed exact solver for general stabilizer codes.

The chosen direction is:

- keep `qec-code::distance::compute_distance` as the main API
- prefer ILP when an ILP feature is enabled at compile time
- otherwise keep exhaustive search for small codes and return an explicit error
  for larger codes
- support general stabilizer codes, not only CSS codes
- support general `k >= 1`, returning the globally minimum-weight nontrivial
  logical witness
- extract solver-agnostic ILP infrastructure into a new small shared workspace
  crate so `qec-code` and `rilpqec` can reuse backend code without taking on
  each other's domain semantics

The recommended formulation parameterizes candidate logical operators using the
existing canonical logical basis:

- stabilizer coefficients choose an element of the stabilizer span
- logical coefficients choose a nonzero element of the logical quotient
- binary per-qubit activity variables encode Pauli weight

This turns exact distance into one binary optimization problem instead of
logical-class enumeration or raw symplectic search.

## Goals

This milestone should:

1. Replace the current primary distance path with an ILP-backed exact solver
   when an ILP feature is enabled.
2. Preserve the current `compute_distance` API.
3. Support general stabilizer codes and general `k >= 1`.
4. Return one globally minimum-weight nontrivial logical witness.
5. Keep a correctness-oriented fallback path for small codes when ILP is not
   enabled.
6. Reuse ILP solver plumbing across `qec-code` and `rilpqec` without coupling
   the two crates' lowering logic.

## Non-Goals

This milestone should not:

1. Promise scalable performance on large surface codes, color codes, or other
   highly structured large instances.
2. Expose a large runtime solver configuration API in the first version.
3. Return all minimum-weight witnesses or all minimum logical classes.
4. Move stabilizer-code semantics into the shared ILP crate.
5. Move DEM semantics into the shared ILP crate.
6. Replace the existing small-code exhaustive implementation as a validation
   oracle.

## Current State

`qec-code` already has the algebraic pieces needed to support a stronger
distance algorithm:

- `StabilizerCode`
- canonical logical basis extraction for general `k`
- symplectic and GF(2) utilities

The current distance path in
[`qec-code/src/distance.rs`](/Users/nzy/rcode/temprstim/rstim/qec-code/src/distance.rs:1)
still exhaustively enumerates all non-identity Pauli operators, filters for
commuting non-stabilizers, and keeps the minimum-weight witness.

That approach is acceptable only for very small codes. It fails to scale and
already has an explicit unsupported-size error path.

Separately, `rilpqec` already contains reusable solver-facing structure:

- backend selection
- `HiGHS` and optional `Gurobi` glue
- compiled solve boundaries

However, `rilpqec` is semantically a DEM decoder crate. `qec-code` should not
depend directly on DEM-specific lowering or result types just to reuse a solver
backend.

## Decision Summary

The chosen design is:

1. keep `compute_distance(code)` as the public entry point
2. add ILP-backed distance solving behind narrow `qec-code` features such as
   `distance-ilp-highs`
3. create a new small workspace crate, referred to in this design as
   `qec-ilp-core`, that owns:
   - solver-agnostic ILP model representation
   - backend traits
   - backend configuration
   - backend errors
   - concrete `HiGHS` and optional `Gurobi` integration
4. keep lowering logic domain-specific:
   - `qec-code` lowers stabilizer-code distance problems
   - `rilpqec` lowers DEM decoding problems
5. formulate distance as one binary optimization problem over stabilizer and
   logical basis coefficients

## Alternatives Considered

### 1. Make `qec-code` directly depend on a default open-source backend

This would make ILP the default behavior with no feature gating.

Benefits:

- simplest end-user experience
- one obvious production path

Costs:

- makes the algebraic core crate heavier by default
- increases build and portability burden
- weakens the crate boundary established in earlier `qec-code` design work

This is not the recommended first step.

### 2. Use compile-time ILP features and keep exhaustive fallback

This keeps the public API stable, uses ILP when explicitly enabled, and retains
small-code exhaustive search as a reference path.

Benefits:

- preserves the existing API surface
- keeps default builds lighter
- enables direct ILP vs exhaustive cross-checks on small instances
- allows backend rollout without committing to a permanently heavy default

Costs:

- introduces multiple internal execution paths
- requires precise documentation of feature-dependent behavior

This is the recommended option.

### 3. Keep `qec-code` solver-free and move all distance solving elsewhere

This would preserve a pure algebraic crate and put exact distance solving in a
separate crate or external tool.

Benefits:

- cleanest dependency boundary

Costs:

- weak user experience for the main `qec-code` distance API
- unnecessarily broadens the multi-crate workflow for the first useful release
- delays integration of a feature the crate is already expected to own

This is not the recommended option.

### 4. Enumerate logical classes and solve one ILP per class

This fixes a logical class and solves for the lightest representative in that
coset, then takes the global minimum across all nonzero classes.

Benefits:

- smaller per-solve model
- easy to reason about for very small `k`

Costs:

- requires `2^(2k) - 1` solves
- scales poorly in `k`
- is less aligned with the goal of general `k >= 1`

This is useful as a small-instance validation strategy, not as the primary
production path.

### 5. Build a raw symplectic ILP without logical-basis parameterization

This would optimize directly over a candidate symplectic row, add commutation
constraints, and separately encode non-membership in the stabilizer span.

Benefits:

- low-level and mathematically direct

Costs:

- non-membership in the stabilizer span is the hardest part to encode cleanly
- higher implementation risk
- weaker fit for the algebra already present in `qec-code`

This is not the recommended first implementation.

## Architecture

The implementation should be split into three layers.

### Layer 1: `qec-code`

`qec-code` remains the owner of:

- `StabilizerCode`
- `DistanceResult`
- `compute_distance`
- stabilizer-code distance lowering
- witness reconstruction and post-solve validation

It should add an internal distance path that:

1. computes or reuses the canonical logical basis
2. short-circuits `k = 0` codes with the existing no-witness error semantics
3. lowers the code into an ILP model
4. solves through `qec-ilp-core`
5. reconstructs the witness `Pauli`
6. validates the result before returning `DistanceResult`

It should also own the feature-dependent dispatch logic:

- if an ILP distance feature is enabled, use ILP
- otherwise use the current exhaustive implementation
- if exhaustive search is unsupported for the code size, return an explicit
  unsupported-distance error

### Layer 2: `qec-ilp-core`

This new workspace crate should be intentionally small and domain-agnostic.

It should own:

- a binary/integer linear model representation
- backend traits and compiled-model boundaries
- backend configuration types
- backend error types
- concrete backend implementations such as `HiGHS`
- optional `Gurobi` support behind feature flags

It should not know about:

- stabilizer codes
- logical operators
- detector error models
- observables

This crate is solver infrastructure only.

### Layer 3: `rilpqec`

`rilpqec` should keep its existing DEM decoder semantics and lowering logic,
but its backend glue and generic model handling should move to or depend on
`qec-ilp-core`.

This preserves reuse while keeping domain boundaries clean:

- `qec-code` reuses backend plumbing, not DEM semantics
- `rilpqec` reuses backend plumbing, not stabilizer-code semantics

## Data Flow

The distance solve should follow this flow:

1. start from `StabilizerCode`
2. if `num_logical_qubits() == 0`, return the existing no-witness error
3. compute `canonical_logical_basis()`
4. lower the code into an ILP model
5. compile and solve through `qec-ilp-core`
6. recover the symplectic witness row from the binary solution
7. convert the row back into `Pauli`
8. validate:
   - witness commutes with all stabilizers
   - witness is not in the stabilizer span
   - witness weight matches the optimization objective
9. return `DistanceResult`

## ILP Formulation

The primary formulation should parameterize a candidate witness using the
stabilizer span plus a nonzero logical quotient element.

Let:

- `s_j` be the symplectic rows of the stabilizer generators
- `lx_i` and `lz_i` be the canonical logical basis rows

Introduce binary variables:

- `lambda_j` for stabilizer-generator coefficients
- `a_i` for logical `X_i` coefficients
- `b_i` for logical `Z_i` coefficients

Define the candidate symplectic row:

`p = sum_j lambda_j s_j + sum_i a_i lx_i + sum_i b_i lz_i (mod 2)`

The crucial property is that the logical rows form a basis of the logical
quotient modulo stabilizers. Therefore, if at least one logical coefficient is
nonzero, the candidate cannot lie purely in the stabilizer span.

### Nontrivial Logical Constraint

Require the logical coefficient vector to be nonzero:

`sum_i a_i + sum_i b_i >= 1`

This enforces that the witness is a nontrivial logical operator without having
to encode stabilizer-span non-membership directly.

### Mod-2 Coordinate Constraints

For each symplectic coordinate `c`, introduce:

- binary variable `p_c`
- integer auxiliary variable `t_c`

Add the equality:

`p_c + 2 t_c = sum_j S[c,j] lambda_j + sum_i LX[c,i] a_i + sum_i LZ[c,i] b_i`

This enforces that `p_c` equals the parity of the chosen generators at that
coordinate.

### Weight Variables

For each physical qubit `q`, introduce a binary variable `y_q` indicating
whether the witness acts nontrivially on that qubit.

Let `p_x(q)` and `p_z(q)` be the `X` and `Z` coordinates for qubit `q`.

Add:

- `p_x(q) <= y_q`
- `p_z(q) <= y_q`
- `y_q <= p_x(q) + p_z(q)`

This makes `y_q = 1` exactly when the Pauli action on qubit `q` is `X`, `Y`,
or `Z`, and `y_q = 0` only for identity. Therefore:

`weight(p) = sum_q y_q`

### Objective

Minimize:

`sum_q y_q`

This objective exactly matches Pauli weight and counts `Y` on one qubit as
weight one, not two.

### Witness Reconstruction

After solving:

- collect the solved `p_c` bits into one symplectic row
- rebuild the `Pauli`
- classify the witness with the existing logical-class logic or its successor

### Why This Formulation

This formulation is the recommended production path because it:

- supports general `k >= 1`
- finds the globally lightest nontrivial logical witness in one solve
- reuses `qec-code`'s existing canonical logical basis infrastructure
- avoids explicit and awkward stabilizer-span non-membership encoding

## Public API

The public API should remain conservative.

Keep:

```rust
pub fn compute_distance(code: &StabilizerCode) -> Result<DistanceResult>
```

The first milestone should not require new mandatory public configuration
parameters.

The existing `DistanceResult` shape should remain sufficient:

- `distance`
- `witness`
- `logical_class`

If runtime solver configuration is needed later, it should be added as an
additional explicit API such as `compute_distance_with_config(...)`, not by
complicating the current primary entry point in the first implementation.

## Feature Design

`qec-code` should use narrow distance-specific features, for example:

- `distance-ilp-highs`
- later, if needed, `distance-ilp-gurobi`

Behavior should be:

1. if an ILP distance feature is enabled, prefer ILP
2. otherwise run exhaustive search
3. if exhaustive search is unsupported for the problem size, return a clear
   unsupported-distance error
4. for `k = 0` codes, return the existing no-witness error before either
   backend path is invoked

This is intentionally not a generic feature named `ilp`, because the feature is
about one specific high-level behavior: distance solving.

## Error Semantics

The current error surface is too narrow to describe solver-backed failures
clearly. The first implementation should add explicit distance-solver error
cases instead of overloading `DistanceWitnessNotFound`.

Recommended additions include variants equivalent to:

- ILP backend unavailable
- ILP solve failed
- ILP infeasible
- distance computation unsupported for the current configuration

Semantic expectations:

- `k = 0` code:
  existing no-witness error
- no ILP feature and unsupported exhaustive size:
  configuration/unsupported error
- requested backend missing:
  backend-unavailable error
- solver returns operational failure:
  solve-failed error
- solver returns infeasible for a code with `k >= 1`:
  explicit inconsistency error, treated as a likely bug or lowering mismatch

`DistanceWitnessNotFound` should remain reserved for genuine no-witness cases,
such as `k = 0` codes, not solver wiring failures.

## CLI Behavior

The existing CLI output format for `distance` should remain stable.

The first version should change only the internal solving path. Optional future
diagnostic or verbose output may expose whether ILP or exhaustive search was
used, but default user-facing output should not churn without need.

## Verification Strategy

Correctness should not rely only on the solver returning a binary answer. The
verification strategy should have four layers.

### 1. Lowering Unit Tests

Add solver-independent tests for the distance lowering itself:

- variable counts
- constraint counts or structural presence checks
- correct binding of `x/z` coordinates to per-qubit weight variables
- presence of the nonzero logical-coefficient constraint

These tests should catch indexing and wiring mistakes before solver invocation.

### 2. Small-Instance Truth Cross-Checks

On small codes, compare ILP and exhaustive results:

- same distance
- witness weight matches distance
- witness commutes with all stabilizers
- witness is not in the stabilizer span

The current exhaustive implementation remains the oracle for these tests.

### 3. General-`k` Coverage

The test matrix must include more than `k = 1`.

At minimum, include:

- `k = 0` code: expect no logical witness
- small `k = 1` code such as Steane or a compact hand-built example
- small `k > 1` general stabilizer code where the global minimum witness is not
  tied to a special-cased single logical qubit assumption

This is necessary because the requested scope explicitly includes general
`k >= 1`.

### 4. Feature and Backend Integration Tests

Test behavior across build modes:

- default `qec-code`: exhaustive on small instances, explicit unsupported error
  on larger instances
- `qec-code` with ILP feature: ILP preferred and small-instance results match
  exhaustive truth
- `qec-ilp-core`: backend availability and backend error propagation
- `rilpqec`: existing decoder behavior remains correct after backend
  extraction/refactoring

## Success Criteria

The first delivery is successful only if all of the following hold:

1. `compute_distance` remains the primary API.
2. With an ILP distance feature enabled, `compute_distance` prefers ILP.
3. Without ILP enabled, small-code exhaustive behavior remains correct.
4. Without ILP enabled, larger unsupported instances fail explicitly.
5. Small-code ILP results match exhaustive truth.
6. The implementation supports general `k >= 1`, not only `k = 1`.
7. The backend-sharing refactor does not break existing `rilpqec` decoding
   behavior.

## Implementation Notes for Planning

This design is intentionally structured to support a later implementation plan
with clear phases:

1. extract shared ILP infrastructure into `qec-ilp-core`
2. refit `rilpqec` onto that shared layer without changing semantics
3. add `qec-code` distance lowering
4. integrate feature-gated dispatch in `compute_distance`
5. add exhaustive-vs-ILP cross-check coverage

Those phases should be detailed in the implementation-planning step after the
user reviews this spec.
