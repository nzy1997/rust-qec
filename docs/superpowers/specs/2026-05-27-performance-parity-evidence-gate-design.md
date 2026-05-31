# Performance Parity Evidence Gate Design

Date: 2026-05-27
Status: Proposed
Scope: `rstim` crate performance evidence, CI gating, and benchmark reporting

## Summary

This design defines the next project after `#35`:
`performance parity evidence gate`.

The goal is not to add another round of performance optimizations. The goal is
to turn the performance work introduced by the performance parity foundation
into a stable engineering evidence system that can answer two questions on
every future change:

- did a change regress the key compiled-path performance signals
- what evidence should a reviewer inspect when the answer is unclear

The system should make performance evidence first-class in the repository
instead of leaving it as a one-off example rerun workflow.

The project will introduce:

- layered benchmark artifacts:
  raw JSONL, machine-readable summary JSON, and human-readable Markdown report
- an explicit split between `gating` benchmark cases and `report_only` cases
- CI automation that runs the benchmark workflow, uploads artifacts, and fails
  on stable regression signals
- a local command flow that reproduces the same judgments as CI

The design explicitly avoids turning cross-machine absolute timing into a hard
PR gate. Instead, it gates on stable relative comparisons measured within the
same CI run.

## Goals

- Make performance evidence a maintained repository workflow instead of a
  manual example rerun.
- Add a benchmark artifact pipeline that preserves raw data and also produces
  reviewer-friendly summaries.
- Fail PRs when stable compiled-path performance signals clearly regress.
- Preserve explicit fallback guarantees for non-compiled workflows such as
  atom-loss sampling.
- Keep Stim comparison results visible in reports without making them the main
  source of CI flakiness.
- Ensure contributors can reproduce the CI decision locally with a documented
  command path.

## Non-Goals

- Do not promise new compiled-path optimizations in this project.
- Do not treat cross-run or cross-machine absolute wall time as a hard
  correctness signal.
- Do not fail PRs directly on `stim-cli` versus `rstim` absolute comparisons.
- Do not broaden the project into a general benchmark platform for every crate
  in the workspace.
- Do not move performance verdicts into the normal `cargo test --workspace`
  path.

## Current State

The repository already contains the first layer of performance infrastructure,
but it does not yet provide durable evidence or automated regression gating.

- `rstim/src/perf.rs` defines benchmark cases, variants, records, and routing
  helpers for the performance parity foundation.
- `rstim/examples/performance_parity_foundation.rs` runs those cases and emits
  one JSON line per `(case, variant)` result.
- `rstim/tests/perf_harness.rs` checks case coverage, variant labeling, and a
  few path-selection properties.
- `rstim/doc/performance_parity.md` documents how to rerun the harness and what
  to inspect manually.
- `.github/workflows/ci.yml` currently runs workspace tests and coverage, but
  it does not yet run a dedicated performance evidence job, publish benchmark
  artifacts, or enforce regression thresholds.

This means the repository can already collect performance records, but it still
depends on manual interpretation. There is no built-in distinction between
reporting data and gating data, no contract validation over the produced
records, and no CI-visible explanation of why a run should pass or fail.

## Decision Summary

The chosen direction is:

- keep the existing benchmark harness as the execution core
- add a second layer for summary, gating, and reporting instead of pushing all
  logic into the runner
- gate only on stable relative comparisons measured inside one run
- separate benchmark cases into `gating` and `report_only` tiers
- keep Stim in the evidence chain, but use it as report context instead of a
  hard gate
- roll out the system in stages:
  report first, contract gate second, regression gate last

## Alternatives Considered

### 1. Absolute baseline gate

This option would commit a baseline performance snapshot and fail CI whenever a
future run exceeds absolute thresholds.

Benefits:

- simple mental model
- easy to explain in one sentence

Costs:

- too sensitive to GitHub runner noise
- fragile across machine changes and background load
- likely to create noisy failures unrelated to the change under review

### 2. Report-only workflow

This option would automate artifact generation but leave regression judgment
entirely to human review.

Benefits:

- very low flake risk
- easier first implementation

Costs:

- does not satisfy the need for strong automatic regression protection
- invites inconsistent review discipline across PRs

### 3. Stable relative gate with full reports

This option measures all variants, keeps the raw evidence, and gates only on
the relative comparisons that are stable within one CI run.

Benefits:

- strong automation without relying on unstable absolute timing
- preserves enough context for manual diagnosis when a gate fails
- aligns with the current `rstim` benchmark harness structure

Costs:

- requires more design than a raw benchmark rerun
- needs a separate summary and gating layer

This is the recommended option.

## Success Criteria

This project is complete only if all of the following are true:

- CI has a dedicated performance evidence job separate from the normal test job
- the benchmark workflow emits `raw.jsonl`, `summary.json`, and `report.md`
- the summary layer validates artifact completeness and routing contracts before
  any regression judgment
- at least one compiled sampler comparison and one compiled analyzer comparison
  are enforced as hard regression checks
- at least one fallback protection case is enforced as a hard routing contract
- contributors can run one documented local command that reproduces the CI
  verdict
- failed CI runs explain whether the failure is infrastructure, contract, or
  regression related

## Recommended Architecture

The system should be split into five focused layers under a `perf` module tree.

### Layer 1: Case Definitions

This layer owns benchmark case metadata.

Suggested responsibilities:

- case label
- workload type
- circuit source
- shot count
- case tier:
  `gating` or `report_only`
- whether the case requires a compiled path
- whether the case requires fallback away from the compiled path
- which variant comparisons are expected and meaningful

This is the layer that defines the contract for the rest of the system. The
runner should not infer policy on its own.

### Layer 2: Runner

This layer executes the benchmark cases and writes raw records.

Responsibilities:

- execute each configured `(case, variant)` workload
- support one warmup round and five measured rounds per `(case, variant)` pair
  in the first version
- capture raw timing and memory observations
- preserve case and variant metadata in the raw output
- write raw JSONL records without interpreting pass or fail

The existing `performance_parity_foundation` flow should remain the natural
starting point for this layer. The main change is that it should collect enough
repeated measurements to support stable median-based summary logic.

### Layer 3: Summary

This layer reads raw JSONL and produces machine-readable normalized output.

Responsibilities:

- validate schema and record completeness
- group repeated measurements by `(case, variant)`
- compute stable aggregates such as median wall time
- compute relative ratios for allowed comparisons
- preserve enough metadata for the gate and report layers

Suggested summary metrics:

- `median_wall_time_ns`
- `median_peak_memory_bytes` when available
- `compiled_vs_interpreted_ratio`
- `compiled_analyzer_vs_flattened_ratio`
- Stim comparison ratios for reporting only

### Layer 4: Gate

This layer consumes the summary output and decides whether CI should pass.

Responsibilities:

- distinguish infrastructure failure from contract failure from regression
  failure
- apply per-comparison thresholds to `gating` cases only
- enforce fallback and compiled-path routing expectations
- produce concise machine-readable and human-readable failure reasons

The gate must be strict about contract problems before it ever reasons about
performance. Untrusted data must not produce a misleading regression verdict.

### Layer 5: Report

This layer renders a human-readable report from the same summary data.

Responsibilities:

- show the measured cases and variants
- show median metrics and comparison ratios
- mark which cases were gating and which were report-only
- include Stim comparison rows for context
- explain any failures or skipped comparisons

The report should be suitable for CI job summaries and uploaded artifacts.

## Artifact Model

The workflow should produce three first-class artifacts.

### Raw JSONL

This is the lossless execution record. It should keep one line per measured
sample, not one line per summarized result.

Suggested fields:

- `case_label`
- `tool_variant`
- `workload`
- `tier`
- `measurement_index`
- `warmup`
- `qubits`
- `measurements`
- `detectors`
- `observables`
- `repeat_depth`
- `repeat_count`
- `shots`
- `wall_time_ns`
- `peak_memory_bytes`

### Summary JSON

This is the contract-checked and comparison-ready artifact.

Suggested contents:

- normalized case metadata
- variant presence map
- aggregate timing and memory metrics
- computed comparison ratios
- contract warnings or failures
- gate-oriented verdict inputs

### Markdown Report

This is the reviewer-facing artifact.

Suggested sections:

- run overview
- gating cases
- report-only cases
- comparison tables
- routing/fallback notes
- failure summary

## Data Flow

The data flow should stay linear and explicit:

`benchmark run -> raw jsonl -> summary json -> gate verdict + markdown report`

Concretely:

1. the runner executes all selected case and variant pairs
2. the runner emits raw JSONL records
3. the summary step validates and aggregates those records
4. the gate step applies contract checks and regression thresholds
5. the report step renders the final human-readable evidence

This split is intentional. It prevents the benchmark runner from becoming a
mixed execution, policy, and rendering layer.

## Case Tiering And Comparison Rules

The benchmark set should be split into two tiers.

### `gating`

These cases must be stable enough to support hard CI decisions.

They should include:

- at least one high-shot sampling case where compiled sampling is expected
- at least one detection-event case where compiled sampling is expected
- at least one large-`REPEAT` analyzer case where the compiled analyzer path is
  expected
- at least one fallback protection case where compiled routing must not occur

### `report_only`

These cases add breadth or diagnostic value but are not stable enough, or not
yet mature enough, for hard gating.

They may include:

- larger or more expensive stress cases
- additional code families
- exploratory Stim comparisons
- future cases that are informative but not yet stable under CI noise

### Allowed Hard Comparisons

The first version should gate only on:

- `rstim-compiled / rstim-interpreted` for compiled sampler-capable
  `sample` and `detect` cases
- `rstim-analyzer-compiled / rstim-analyzer-flattened` for compiled
  analyzer-capable `analyze_errors` cases

Stim comparisons should remain visible in reports but should not fail PRs in
the first version.

## Failure Model

The gate should classify failures into three categories.

### Infrastructure Failure

Examples:

- Stim binary missing or not executable
- benchmark subprocess crash
- raw artifact not written
- summary parse failure

This means the evidence run did not complete successfully.

### Contract Failure

Examples:

- required variant missing for a gating case
- duplicated records for the same measurement slot
- conflicting case metadata under the same label
- case marked as compiled-capable did not produce the compiled variant
- case marked as fallback-only incorrectly produced the compiled variant

This means the data cannot be trusted enough to judge performance.

### Regression Failure

Examples:

- compiled sampler ratio exceeds the allowed threshold
- compiled analyzer ratio exceeds the allowed threshold
- future stable regression metrics exceed configured bounds

This means the data is trustworthy and indicates a real CI-visible regression.

## Threshold Strategy

The gate should use same-run relative thresholds derived from summary metrics.

Recommended first version:

- use median wall-clock timing over multiple measured rounds
- use an initial hard threshold of `1.10` for both sampler and analyzer
  comparisons
- configure thresholds per comparison type instead of globally

Example first-pass rules:

- sampler gate:
  `rstim-compiled / rstim-interpreted <= 1.10`
- analyzer gate:
  `rstim-analyzer-compiled / rstim-analyzer-flattened <= 1.10`
- fallback gate:
  compiled variant must be absent and interpreted execution must succeed

These thresholds should start loose enough to avoid noise-driven failures. The
initial goal is to catch obvious regressions, not to measure tiny wins.

## Testing Strategy

Testing should be layered so logic correctness and benchmark execution concerns
do not collapse into one flaky suite.

### Unit Tests

Add pure logic tests for:

- raw record parsing
- summary aggregation
- median computation
- ratio computation
- contract validation
- gate verdict classification
- report rendering of key sections

These tests should use small fixtures and should not depend on real timing
behavior.

### Schema And Contract Regression Tests

Add tests that pin:

- required summary JSON keys
- stable report section headings
- expected failure wording for common gate failures

These tests protect the artifact contract consumed by CI and reviewers.

### Focused Integration Tests

Extend the current `perf_harness` coverage with integration-style tests that
exercise:

- tiered case definitions
- repeated-measurement grouping
- compiled-required case validation
- fallback-required case validation

These tests should verify control flow and data shape, not real performance.

### CI Performance Gate

Keep real performance verdicts in a dedicated CI job instead of the workspace
test suite. This isolates timing-sensitive logic from normal correctness tests.

## CI Design

Add a new workflow job, for example `perf-gate`, separate from the existing
test and coverage jobs.

Recommended steps:

1. check out the repository
2. install Rust and Stim
3. run the performance evidence command flow
4. upload `raw.jsonl`, `summary.json`, and `report.md` as artifacts
5. publish `report.md` into the job summary
6. fail the job if the gate verdict is infrastructure, contract, or regression
   failure

The CI job should not be merged into `cargo test --workspace`. It serves a
different purpose and should stay independently diagnosable.

## Local Command Surface

The repository should expose one coherent command surface for local reruns.

The first version should expose these as `rstim` CLI subcommands:

- `perf run`
- `perf summarize`
- `perf gate`
- `perf report`
- `perf ci`

In CLI form, the local reproduction path should be:

`cargo run -p rstim --bin rstim -- perf ci`

The `perf ci` entry point should execute the same pipeline as the CI job so a
developer can reproduce the verdict locally with one command.

## Rollout Plan

The rollout should be staged to avoid shipping a noisy hard gate before the
data contract is proven stable.

### Phase A: Report-Only

Deliver:

- raw JSONL generation
- summary generation
- Markdown report generation
- CI artifact upload

Do not fail PRs yet on performance evidence.

Goal:
prove that the artifact pipeline and command surface are stable.

### Phase B: Contract Gate

Deliver hard failures for:

- missing variants
- duplicate or malformed records
- inconsistent case metadata
- compiled-required and fallback-required routing violations

Do not yet fail PRs on performance ratio thresholds.

Goal:
prove that the data is trustworthy.

### Phase C: Regression Gate

Deliver hard failures for:

- compiled sampler regressions on selected gating cases
- compiled analyzer regressions on selected gating cases

Keep thresholds conservative at first.

Goal:
enforce stable performance evidence without turning CI into a noise source.

## Risks And Mitigations

### Risk: CI Noise Produces False Regressions

Mitigation:

- compare variants within one run
- use repeated measurements and medians
- start with conservative thresholds
- keep absolute Stim comparisons out of the hard gate

### Risk: The Runner Becomes A God Object

Mitigation:

- keep execution, summary, gate, and report layers separate
- keep policy in case definitions and gate logic, not in the runner

### Risk: Artifact Contracts Drift Quietly

Mitigation:

- add schema and wording regression tests
- keep summary output structured and explicit

### Risk: Contributors Cannot Reproduce CI Locally

Mitigation:

- provide one documented local command path
- keep the CI workflow aligned with the same command flow

## Completion Criteria

This project should be considered complete when:

- the repository can produce all three benchmark artifacts in CI and locally
- contract validation failures are explicit and readable
- at least one compiled sampler comparison is hard-gated
- at least one compiled analyzer comparison is hard-gated
- at least one fallback protection case is hard-gated
- the CI job publishes a readable report artifact and summary
- the documented local command reproduces the same gate verdict as CI

At that point, future performance work can build on a stable evidence system
instead of redefining how performance claims are justified on each PR.
