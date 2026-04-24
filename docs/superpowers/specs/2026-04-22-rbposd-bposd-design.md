# Rust BPOSD Design

Date: 2026-04-22
Status: Proposed
Scope: `rstim` workspace, new `rbposd` crate

## Summary

This design adds a Rust implementation of BPOSD to the current workspace as a
new independent crate named `rbposd`.

The first version is intentionally narrow:

- ship an algorithm-focused crate instead of wiring directly into `rstim` or
  `rsinter`
- model decoding as a binary linear-code problem:
  `parity_check_matrix + syndrome -> correction`
- implement only the MVP algorithm path:
  `minimum-sum BP + parallel schedule + OSD_0`
- keep the core API free of detector-error-model, detector, and observable
  semantics

The roadmap explicitly includes later phases for:

- a thin CSS-oriented helper layer on top of the core matrix decoder
- a bridge into `rsinter::decode` for end-to-end decoding experiments in this
  workspace

This keeps the first version aligned with the BPOSD core from Python `ldpc`
while avoiding a premature collapse of the algorithm layer into the current
quantum workflow.

## Goals

- Add a standalone Rust crate in this workspace that provides a usable BPOSD
  decoder.
- Match the first meaningful slice of Python `ldpc` BPOSD functionality in a
  way that is easy to test against reference behavior.
- Center the core public API on matrix decoding inputs and outputs instead of
  quantum-specific artifacts.
- Make decoder compilation reusable so the same compiled decoder can process
  many syndromes efficiently.
- Leave clean extension points for later CSS helpers and `rsinter`
  integration.

## Non-Goals

- Do not make the first version consume `DetectorErrorModel` directly.
- Do not make detector and observable prediction first-class core API
  concepts.
- Do not implement `product_sum`, serial scheduling, or `OSD_CS` in the first
  version.
- Do not optimize for every sparse-matrix workload before the MVP behavior is
  correct and testable.
- Do not introduce a dependency from `rbposd` back into `rstim` or `rsinter`.

## Current State

The current workspace already has a natural integration target but does not yet
contain a real decoder implementation:

- [`rsinter/src/decode.rs`](/Users/nzy/rcode/rstim/rsinter/src/decode.rs)
  defines a minimal decoder abstraction with `Decoder` and
  `CompiledDecoder` traits, but only includes `VacuousDecoder`.
- [`rstim/src/dem.rs`](/Users/nzy/rcode/rstim/rstim/src/dem.rs) and
  [`rstim/src/m2d.rs`](/Users/nzy/rcode/rstim/rstim/src/m2d.rs) already cover
  detector error models and measurement-to-detection conversion, which means
  there is a clear later path from the workspace into a real decoder.
- [`rstim/doc/getting_started.md`](/Users/nzy/rcode/rstim/rstim/doc/getting_started.md)
  currently points users to external decoders such as `rmatching` rather than
  an in-workspace solution.

This creates a useful separation of concerns for the new work:

- `rbposd` can focus on correct and reusable decoding machinery
- later phases can bridge the existing workspace data flow into `rbposd`

## Constraints And Design Decisions

### Chosen Direction

The design follows this explicit direction:

- first build an independent crate
- design the core around binary linear-code decoding
- keep quantum-specific semantics outside the core API
- include later phases for CSS convenience and `rsinter` integration

### Alternatives Considered

#### 1. Pure algorithm crate only

This is the smallest scope and the easiest to align with Python `ldpc`, but it
pushes all workspace integration to a later design and provides less immediate
value to current `rstim` workflows.

#### 2. Layered approach

Build an independent algorithm crate first, then add a thin CSS helper layer,
then add `rsinter` integration through a dedicated adapter.

This is the recommended option because it preserves a clean core while still
committing the roadmap to useful local integration.

#### 3. Workflow-first integration

Start from `rsinter` and `DetectorErrorModel` integration, then build the
algorithm beneath it.

This would produce an end-to-end demo earlier, but it would also expand the
scope from "implement BPOSD" to "define a new quantum decoding workflow" and
would make it harder to stay faithful to the `ldpc` core.

## Recommended Architecture

Use a layered design with three levels:

1. `rbposd` core
2. optional thin helpers above the core
3. workspace integration adapters outside the core

### Layer 1: `rbposd` Core

The core owns:

- parity-check matrix representation
- syndrome and correction vector representation
- channel priors
- BP implementation
- OSD implementation
- compiled decoder lifecycle

The core does not own:

- DEM parsing
- detector graph semantics
- measurement-to-detection conversion
- observable prediction semantics

### Layer 2: Thin Helper Layer

After the MVP is stable, add helpers for two-matrix CSS-style use cases such as
separate `Hx` and `Hz` decode flows. These helpers should remain convenience
wrappers over the same matrix-based core.

### Layer 3: Workspace Integration

`rsinter` integration should be implemented as an adapter that translates a
workspace-specific decoding problem into the core `rbposd` matrix problem. That
adapter may depend on `rstim` and `rsinter`, but `rbposd` itself must remain
independent.

## Public API Shape

The public API should be centered on a compiled decoder object, not on free
functions. The compiled form can hold the structural data that is expensive to
rebuild on every decode call.

### Core Types

#### `ParityCheckMatrix`

Represents a binary parity-check matrix.

Requirements:

- support construction from sparse rows at minimum
- support validation of dimensions and index bounds
- expose row and column structure needed by BP and OSD
- permit algorithm-specific derived structures without changing the public
  input model

The stable public type should represent the decoding problem. Internal
algorithm-specific adjacency structures may be derived from it.

#### `Syndrome`

Represents the right-hand side of the decoding equation over GF(2).

Requirements:

- length must match the number of matrix rows
- easy construction from `Vec<bool>` in the MVP
- packed representations are a later optimization, not part of the first public
  surface

#### `Correction`

Represents the decoded binary error or recovery vector.

Requirements:

- length must match the number of matrix columns
- be easy to verify against the syndrome using GF(2) multiplication

#### `ChannelModel`

Describes bit prior information for decoding.

The MVP should support:

- a uniform binary symmetric channel case
- per-bit flip probabilities

This covers the minimum needed to model the common `ldpc` BPOSD entry points
without overspecifying future channel abstractions.

#### `DecoderConfig`

Collects algorithm configuration.

The MVP should include fields for:

- maximum BP iterations
- convergence or stopping policy

It may contain enums for future options, but the first implemented path is:

- BP rule: `minimum_sum`
- schedule: `parallel`
- OSD mode: `OSD_0`

The MVP should not expose minimum-sum scaling, damping, or "always run OSD"
switches in the public config. The fixed behavior is:

- run BP up to the configured iteration limit
- return the BP hard decision immediately if it satisfies the parity checks
- run `OSD_0` only when the BP hard decision fails

#### `BpOsdDecoder`

Compiled decoder built from:

- `ParityCheckMatrix`
- `ChannelModel`
- `DecoderConfig`

Responsibilities:

- validate compatibility of inputs
- build row and column adjacency
- precompute reusable working structures
- decode many syndromes through `decode(&syndrome)`

#### `DecodeResult`

The result type should include both the correction and enough diagnostic data
to support tests, benchmarks, and later regression analysis.

Required fields:

- `correction`
- `converged`
- `bp_iterations`
- `used_osd`
- `residual_syndrome_weight`

Optional future fields may be added later, but the MVP should avoid exposing
large internal traces by default.

### Example API Sketch

```rust
let pcm = ParityCheckMatrix::from_sparse_rows(num_rows, num_cols, rows)?;
let channel = ChannelModel::Bsc { error_rate: 0.05 };
let config = DecoderConfig::default();

let decoder = BpOsdDecoder::new(pcm, channel, config)?;
let result = decoder.decode(&syndrome)?;
```

This shape supports both current MVP goals and later adapters that compile a
decoder once and reuse it across many shots.

## Crate Layout

The initial crate layout should stay focused:

```text
rbposd/src/
  lib.rs
  matrix.rs
  vector.rs
  config.rs
  decoder.rs
  bp.rs
  osd.rs
  error.rs
```

Module responsibilities:

- `matrix.rs`: stable matrix input type and derived helpers
- `vector.rs`: syndrome and correction representations plus GF(2) utilities
- `config.rs`: channel and decoder configuration types
- `decoder.rs`: public compiled-decoder API and orchestration
- `bp.rs`: minimum-sum belief propagation
- `osd.rs`: OSD_0 implementation and GF(2) elimination helpers that are truly
  OSD-specific
- `error.rs`: construction and decode error types

If later implementation shows that GF(2) elimination deserves its own module,
that refactor is acceptable, but it is not required to start.

## Algorithm Scope

The MVP is a deliberately narrow algorithm slice:

- input: binary parity-check matrix, syndrome, and bit priors
- BP rule: minimum-sum
- schedule: parallel
- post-processing: OSD_0
- output: correction vector satisfying `H * e = syndrome` over GF(2)

### Explicitly Included

- reusable compiled decoder construction
- syndrome validation
- BP iterations with a clear stopping condition
- conversion from BP state to a hard decision plus reliability ordering
- OSD_0 fallback or refinement
- result diagnostics sufficient for testing and regression tracking

### Explicitly Excluded

- `product_sum`
- serial or layered scheduling
- higher-order OSD variants such as `OSD_CS`
- DEM-native input
- batch APIs as part of the first public surface
- quantum observable prediction in the core result type

## Decode Data Flow

### `BpOsdDecoder::new(...)`

Construction performs:

1. validate matrix dimensions and channel-model compatibility
2. build row and column adjacency structures
3. initialize reusable BP metadata and workspace plans
4. prepare OSD-related permutation and elimination workspace

The goal is to pay structural setup cost once.

### `decode(&syndrome)`

Decode performs:

1. validate syndrome length
2. initialize BP messages from priors
3. run minimum-sum BP until convergence or iteration limit
4. derive a hard decision and reliability ordering
5. if the hard decision already satisfies the syndrome, return it directly
6. otherwise run OSD_0
7. return correction plus diagnostics

### `OSD_0`

The `OSD_0` step performs:

1. sort columns by reliability
2. permute matrix columns accordingly
3. perform GF(2) elimination to identify a usable information set
4. construct a candidate correction satisfying the syndrome
5. invert the permutation back to the original column order

The implementation must make these substeps individually testable instead of
hiding all logic inside one large routine.

## Testing Strategy

Testing should be layered so failures are easy to localize.

### 1. Algebra Tests

Verify:

- GF(2) row operations
- column permutations
- system-form elimination
- `H * correction = syndrome` checks

These tests must not depend on BP or OSD.

### 2. Algorithm Unit Tests

Use small hand-constructed matrices and fixed syndromes to verify:

- BP iteration behavior
- hard-decision extraction
- OSD_0 result construction
- correctness of `DecodeResult` diagnostics

### 3. Reference Comparison Tests

Create a set of fixed fixtures and compare the Rust implementation against the
targeted MVP behavior from Python `ldpc`.

The comparison focus should be:

- correction validity
- convergence behavior when the compared configuration matches
- stable behavior on selected fixed examples

The goal is not to clone every `ldpc` feature immediately. The goal is to lock
the specific MVP path this design commits to.

### 4. Property Tests

On small random matrices, verify that every successful decode result satisfies
the syndrome equation. This is especially important for OSD, where subtle
mistakes can produce plausible but invalid outputs.

### 5. Regression Benchmarks

Track a few repeatable workloads to catch unexpected performance or iteration
regressions during refactors. The first version does not need aggressive
optimization, but it should guard against accidental complexity blowups.

## Risks And Mitigations

### Risk: Wrong Matrix Representation Boundary

If the public matrix type is tailored too early to one internal algorithm, BP
and OSD will fight over representation details.

Mitigation:

- keep `ParityCheckMatrix` stable as the problem definition
- derive algorithm-specific adjacency or temporary structures internally

### Risk: Behavior Drift From Reference Implementation

Small choices in minimum-sum initialization, message updates, clipping, or
stopping criteria can produce large differences from the reference behavior.

Mitigation:

- define the MVP comparison scope before implementation
- lock fixed fixtures early
- compare behavior incrementally instead of only at the end

### Risk: Silent OSD Incorrectness

OSD can appear to work while returning invalid or unreliable outputs if column
ordering, elimination, or inverse permutation logic is wrong.

Mitigation:

- split OSD into individually testable helper steps
- add explicit validity checks that the produced correction satisfies the
  syndrome equation

### Risk: Integration Pressure Pollutes The Core

When `rsinter` integration begins, there will be pressure to move DEM,
detector, or observable logic into `rbposd`.

Mitigation:

- keep adaptation code outside the core crate
- treat quantum workflow inputs as adapter responsibilities, not core
  abstractions

## Phased Implementation Plan

The work should be executed in explicit phases.

### Phase 0: Reference Scope Lock

- inspect the relevant Python `ldpc` BPOSD MVP behavior in detail
- define the exact comparison surface for the first Rust version
- assemble a small fixture set covering simple repetition-style and generic
  sparse matrix cases

Exit criteria:

- the team has a written list of MVP behaviors to match
- fixed fixtures exist for reference comparisons

### Phase 1: Crate Skeleton And Algebra Infrastructure

- add `rbposd` as a new workspace member
- create core matrix, vector, config, and error types
- implement GF(2) utilities and matrix validation
- keep decoding logic separate from algebra infrastructure

Exit criteria:

- crate builds
- algebra tests pass
- matrix and vector invariants are enforced

### Phase 2: Minimum-Sum BP Core

- implement parallel-schedule minimum-sum BP
- support uniform and per-bit prior probabilities
- expose basic convergence diagnostics
- verify behavior on fixed fixtures

Exit criteria:

- BP-only decoding works on the chosen MVP fixtures
- diagnostics are stable enough for tests and benchmarks

### Phase 3: OSD_0 Integration

- implement reliability ordering
- implement column permutation and GF(2) elimination for OSD_0
- produce candidate corrections in original column order
- add regression cases where pure BP fails but BP+OSD succeeds

Exit criteria:

- successful decode results always satisfy the syndrome equation
- fixture coverage includes cases that require OSD_0

### Phase 4: Public API Consolidation

- finalize `BpOsdDecoder::new(...)` and `decode(...)`
- finalize `DecodeResult`
- add examples and crate-level documentation
- avoid exposing premature batch or quantum-specific APIs

Exit criteria:

- users can compile a decoder and decode syndromes with a small documented
  example
- public API is small and coherent

### Phase 5: Thin CSS Helper Layer

- add convenience wrappers for two-matrix CSS-style workflows
- keep helpers as wrappers over the same core decoder abstraction

Exit criteria:

- CSS-oriented callers can use a more natural entry point
- no quantum-specific semantics leak into the core matrix types

### Phase 6: `rsinter` Integration

- implement an adapter in `rsinter::decode`
- translate workspace-specific decoding problems into matrix decoding inputs
- provide an end-to-end path for `collect(...)` to use `rbposd`

Exit criteria:

- `rsinter` can use `rbposd` through an adapter
- `rbposd` still has no dependency on `rstim` or `rsinter`

## Acceptance Criteria

This design is satisfied when:

- a new workspace crate provides a usable Rust BPOSD MVP implementation
- the MVP public API is centered on compiled matrix decoding
- the first implemented path is `minimum-sum + parallel + OSD_0`
- test coverage proves algebra correctness, decode validity, and selected
  reference alignment
- later phases preserve a clean route to CSS helpers and `rsinter`
  integration without polluting the core API

## Open Questions Deferred To The Implementation Plan

These questions do not block the design, but they should be resolved during
planning:

- whether the public vector types should start as thin wrappers or aliases over
  compact bit-oriented storage
- whether damping or normalization controls are needed in the MVP config from
  day one or can be added in a follow-up
- what the minimal fixture set should be for reference comparison against
  Python `ldpc`
