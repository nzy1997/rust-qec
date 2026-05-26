# Performance Parity Foundation Design

Date: 2026-05-25
Status: Proposed
Scope: `rstim` crate performance architecture, benchmarking, and staged rollout

## Summary

This design defines the next major project for the repository:
`performance parity foundation`.

The goal is not to keep adding isolated Stim features. The goal is to build a
shared performance foundation that lets `rstim` move meaningfully closer to
Stim on the two workloads where the current architecture is weakest:

- high-shot `sample` and `detect`
- large-`REPEAT` `analyze_errors`

The design is benchmark-first. Phase completion is defined by measured gains on
fixed workloads, not by landing a large new abstraction.

The project will introduce:

- a reproducible benchmark harness
- a shared compiled intermediate layer between `StimInstr` and hot-path
  execution or analysis
- a compiled sampler path for `sample` and `detect`
- a loop-aware analyzer path that avoids flattening large `REPEAT` structures

The design explicitly preserves the current semantic AST as the source of
truth, and it explicitly allows conservative fallback to existing slower paths
for workflows such as atom loss, tracked provenance, and sample traces.

## Goals

- Establish a benchmark suite that compares current `rstim`, improved `rstim`,
  and upstream Stim on fixed workloads.
- Improve `sample` and `detect` throughput by compiling once and reusing the
  compiled representation across many shots.
- Improve `analyze_errors` on large repeated circuits by avoiding eager global
  flattening of `REPEAT` blocks.
- Share structural compilation work between the sampling and analysis paths
  without forcing both paths into one executor implementation.
- Preserve current semantics and correctness across existing parity and
  regression tests.
- Preserve current differentiated workflows such as atom loss, QP101 export,
  sample trace annotations, and tracked DEM provenance by allowing explicit
  fallback to conservative paths.

## Non-Goals

- Do not promise full Stim performance parity in one phase.
- Do not rewrite the parser or replace `StimInstr` as the source semantic
  representation.
- Do not force atom-loss-aware sampling, tracked provenance, and QP101-focused
  workflows into the first compiled path.
- Do not require that the first loop-aware analyzer replicate every detail of
  Stim's loop folding implementation.
- Do not broaden this project into general CLI feature parity such as `diagram`
  or `repl`.

## Current State

The current codebase already has partial ingredients for a faster design, but
the hot paths are still dominated by interpretation and eager expansion.

- [`rstim/src/sampler.rs`](/Users/nzy/rcode/rstim/rstim/src/sampler.rs)
  already uses a reference-sample approach and routes non-loss circuits into
  the frame simulator. However, the frame simulator is built fresh per call and
  the circuit itself is not compiled into a reusable hot-path representation.
- [`rstim/src/sim/frame.rs`](/Users/nzy/rcode/rstim/rstim/src/sim/frame.rs)
  performs batch-oriented frame simulation, but still walks the instruction
  tree directly and handles `REPEAT` by recursively re-running the body for
  each iteration.
- [`rstim/src/error_analyzer.rs`](/Users/nzy/rcode/rstim/rstim/src/error_analyzer.rs)
  currently flattens any circuit containing `REPEAT` before the main DEM
  analysis path, which makes large repeated circuits structurally expensive.
- The repository already contains parity-oriented tests and comparisons against
  Stim, including:
  [`rstim/tests/stim_parity_showcase.rs`](/Users/nzy/rcode/rstim/rstim/tests/stim_parity_showcase.rs),
  Stim-derived regression suites under
  [`rstim/tests/`](/Users/nzy/rcode/rstim/rstim/tests),
  and DEM cross-validation coverage.

The result is that `rstim` has strong semantic momentum but lacks the compile
and loop-reuse layers that define Stim's strongest performance characteristics.

## Reference Target

The performance comparison target for this design is current upstream Stim,
specifically the 1.x line represented by Stim `v1.16.0`, released on
2026-05-22.

This does not imply complete command or feature parity. It defines the external
reference point for benchmark expectations and architectural direction.

## Decision Summary

The chosen direction is:

- prioritize performance work over additional feature matching
- treat the project as a large engineering effort, not a quick optimization
- cover both hot paths:
  `sample`/`detect` and `analyze_errors`
- lead with benchmark discipline instead of architecture-first implementation
- build one shared compile layer, then let sampler and analyzer consume it in
  different ways

## Alternatives Considered

### 1. Sampling-only project

This option focuses entirely on a compiled sampler for `sample` and `detect`.

Benefits:

- fastest path to visible throughput gains
- aligns directly with Stim's most famous strength

Costs:

- leaves `analyze_errors` structurally behind
- risks creating a second performance project later with a separate repeat
  model

### 2. Analyzer-only project

This option focuses entirely on loop-aware `analyze_errors`.

Benefits:

- isolates the flattening bottleneck cleanly
- gives a narrower first implementation target

Costs:

- does not improve the most common high-shot workflows
- does not establish a shared base for sampling

### 3. Shared compile layer with staged dual-track rollout

This option introduces a compile layer shared by the sampler and analyzer,
while still allowing each consumer to evolve independently.

Benefits:

- creates one place to encode repeat structure, spans, and feature flags
- keeps the sampler and analyzer aligned on circuit structure
- lets Phase 1 target the sampler first without orphaning analyzer work

Costs:

- larger up-front design burden
- requires stricter scope control to avoid over-generalizing the compile layer

This is the recommended option.

## Success Criteria

This section defines success for the first complete performance milestone, not
for the internal numbering of the rollout phases below.

The first complete performance milestone is defined by benchmarked improvement,
not by abstraction count.

The benchmark suite must cover:

- commands:
  `sample`, `detect`, `analyze_errors`
- circuit families:
  repetition code, surface code, and color code generator outputs
- structural stress:
  at least one high-shot case and at least one large-`REPEAT` case
- protection cases:
  at least one atom-loss or QP101-adjacent circuit to ensure the new routing
  logic does not break differentiated workflows

Each benchmark record must capture:

- circuit label
- qubit count
- measurement count
- detector count
- observable count
- repeat depth
- repeat count or effective iteration count
- shot count when applicable
- wall time
- peak memory
- tool variant:
  current `rstim`, new-path `rstim`, and Stim

The first delivery is successful only if all of the following are true:

- `sample` or `detect` shows a clear multi-case throughput improvement over the
  current `rstim` baseline on fixed large-shot workloads
- `analyze_errors` no longer scales by eager global repeat expansion on at
  least one large-`REPEAT` benchmark
- existing semantic parity and regression tests remain green
- differentiated workflows still behave correctly through explicit fallback or
  through unchanged existing paths

## Recommended Architecture

The architecture should be split into three layers.

### Layer 1: Source Semantics

The existing `StimInstr` AST remains the semantic source of truth.

This layer continues to own:

- parsing
- round-trip formatting
- current semantic validation
- QP101 export
- sample trace integration
- atom loss semantics
- tracked provenance semantics

This avoids the highest-risk failure mode of the project: redefining semantics
inside the performance layer.

### Layer 2: Shared Compile Layer

Add a new internal module, for example `rstim/src/compiled/`, responsible for
converting `StimInstr` into a reusable structural representation.

Suggested core types:

- `CompiledCircuit`
- `CompiledBlock`
- `CompiledRepeatRegion`
- `CompiledOpBatch` or an equivalent linear instruction segment type
- `CompiledFeatureFlags`

The compile layer should precompute information such as:

- operation batches suitable for hot-path execution
- repeat region boundaries and nesting
- qubit, measurement, detector, and observable spans
- feature flags such as `has_loss`, `has_feedback`, and
  `needs_tracked_provenance`
- metadata needed by path routing and benchmark labeling

The compile layer is not itself an executor or analyzer. Its job is to turn the
semantic tree into a reusable performance-oriented structural form.

### Layer 3: Consumer Paths

There are two primary consumers of the compiled layer.

#### Compiled Sampler

The compiled sampler serves `sample` and `detect`.

Responsibilities:

- consume `CompiledCircuit`
- reuse compiled structure across many shots
- reduce opcode dispatch and recursive AST traversal overhead
- reuse repeat-body structure instead of interpreting each loop iteration from
  the original AST
- preserve the current reference-sample framing model where applicable

The first implementation does not need to fully replicate Stim's final compiled
sampler design. It must, however, establish compile-once and execute-many
behavior.

#### Loop-Aware Analyzer

The loop-aware analyzer serves `analyze_errors`.

Responsibilities:

- consume compiled repeat structure instead of flattening the full circuit
- analyze repeat bodies in structured form
- compose safe summaries across repeated regions
- avoid cost proportional to full eager repeat expansion where possible

The first implementation should optimize for safety and correctness before it
optimizes for the most aggressive possible loop folding.

## Fast Path And Fallback Strategy

The project must not force every workflow into the new performance paths on day
one.

Routing should be explicit:

- common circuits without advanced semantic requirements use compiled paths
- circuits that require unsupported compiled behavior fall back to the current
  implementations

The conservative path should remain the default fallback for:

- atom-loss-aware sampling
- sample trace generation
- tracked DEM provenance
- any workflow that depends on semantics not yet modeled by the compiled layer

Fallback must be correct and intentional. Silent wrong behavior is not allowed.

## Data Flow

The steady-state data flow should be:

`parse_lines -> StimInstr -> compile() -> CompiledCircuit -> consumer`

Sampler path:

`StimInstr -> CompiledCircuit -> CompiledSampler::run(shots)`

Analyzer path:

`StimInstr -> CompiledCircuit -> LoopAwareAnalyzer::analyze()`

This keeps the high-level source stable while giving hot paths reusable,
structured input.

## Phased Rollout

### Phase 0: Benchmark Harness

Build the benchmark harness before major architectural changes.

Deliverables:

- reproducible benchmark cases
- machine-readable output format
- comparison support for current `rstim`, improved `rstim`, and Stim
- fixed seeds and fixed workload parameters
- documentation for how to rerun the benchmark suite

This phase exists to prevent architecture work from drifting away from measured
benefit.

### Phase 1: Shared Compile Layer

Introduce `CompiledCircuit` and related structural types.

Deliverables:

- compile entry point from `StimInstr`
- repeat-region representation
- feature flags for path selection
- metadata used by benchmark reporting and routing
- tests proving compiled structure preserves source ordering and structure

This phase does not need to provide the full speedup yet. It establishes the
base that later phases consume.

### Phase 2: Compiled Sampler And Detect Path

Add a compiled execution path for `sample` and `detect`.

Deliverables:

- compile-once execution path for non-fallback circuits
- repeat-aware execution reuse
- benchmarked throughput improvements on fixed high-shot workloads
- path routing and fallback tests

The first version should optimize away repeated AST interpretation and repeated
loop-tree traversal before attempting more ambitious sampling tricks.

### Phase 3: Loop-Aware Analyze Errors

Replace eager repeat flattening in the main analysis path with compiled repeat
structure and safe summary reuse.

Deliverables:

- no mandatory full flattening for supported repeated circuits
- benchmarked improvement on large-`REPEAT` analyzer workloads
- dedicated repeat-boundary correctness tests
- documented unsupported structures that still require fallback

This phase should first eliminate the structural flattening bottleneck safely.
It does not need to implement every detail of Stim-style loop folding on the
first pass.

## Benchmark Design

The benchmark suite should be stored in-repo and should be runnable on demand.

The benchmark set should include:

- generated repetition-code memory circuits
- generated rotated and unrotated surface-code memory circuits
- generated color-code memory circuits
- one or more large-shot sampling cases
- one or more large-repeat analyzer cases
- one protection case that exercises differentiated semantics

The benchmark output should be designed for later regression checks and for
human-readable summaries in docs or release notes.

At minimum, the benchmark system should support:

- raw records
- stable labels
- grouped summary tables
- comparison against previous `rstim` baseline runs

## Risks

### Dual-Path Semantic Drift

If the compiled path and the conservative path encode semantics differently,
the project will create a second implementation that slowly diverges.

Mitigation:

- keep `StimInstr` as the source semantic layer
- keep compile as structural lowering, not semantic reinterpretation
- require parity-style regression coverage across both paths

### Incorrect Repeat Summaries

`analyze_errors` is especially sensitive to detector and observable influence
that crosses repeat boundaries.

Mitigation:

- make the first analyzer summary model intentionally conservative
- prefer partial reuse over unsafe aggressive folding
- add repeat-boundary correctness tests before broadening optimization scope

### Benchmark Noise

Poorly controlled benchmark inputs can turn the project into a sequence of
inconclusive runs.

Mitigation:

- fixed inputs
- fixed seeds
- fixed shot and repeat parameters
- persistent result files
- summary by median or similarly stable aggregate

### Scope Collapse Into Differentiated Features

If atom loss, tracked provenance, and QP101-focused workflows are pulled into
the first compiled path, the project will likely stall.

Mitigation:

- explicit fast-path and fallback-path routing
- no requirement that differentiated workflows use the compiled path in the
  first performance phase

## Testing Strategy

### 1. Semantic Regression

Reuse and extend the existing Stim-derived test suites and parity tests to
ensure the new paths do not change accepted semantics.

### 2. Routing Tests

Add direct tests proving that:

- supported common circuits take the compiled path
- unsupported or high-risk circuits take the fallback path
- fallback behavior is explicit and deterministic

### 3. Repeat-Specific Correctness

Add tests focused on:

- deep repeat nesting
- large repeat counts
- detector propagation across repeat boundaries
- observable propagation across repeat boundaries
- equality of analyzer outputs between old and new safe paths on supported
  repeat workloads

### 4. Benchmark Regression

Treat benchmark output as part of the engineering evidence for phase
completion.

Performance changes should be evaluated against:

- current `rstim`
- the in-progress compiled path
- Stim where relevant

### 5. Acceptance Rule

A phase is complete only when:

- semantic tests are green
- routing tests are green
- benchmark evidence shows the intended gain for that phase

Structural cleanup without benchmark evidence is not a valid completion
condition.

## Implementation Notes

The most important design constraint is that the compile layer must stay
narrow. It should describe structure, spans, and reusable regions. It should
not become a second universal semantics engine.

The most important product constraint is that benchmark evidence decides
priority. If a compiled abstraction grows but does not move the fixed
workloads, it should be reconsidered immediately.

## Recommended Next Step

The next step after this design is to write an implementation plan that starts
with the benchmark harness and then sequences the compile layer, compiled
sampler path, and loop-aware analyzer path with explicit review checkpoints.
