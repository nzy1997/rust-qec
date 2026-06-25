# Issue 227 qec-code ILP CLI Options Design

Date: 2026-06-25
Status: Design approved by Agent Desk standing policy
Scope: GitHub issue #227, exposing exact CSS distance ILP backend and run-control options

## Summary

Issue #227 should let benchmark scripts configure the existing exact CSS
distance ILP path from the `qec-code` CLI. The command should accept a requested
backend, a per-instance solver time limit, and the already-supported optional
solver knobs for MIP gap, threads, and verbosity. JSON output should make the
requested options auditable and should only report an exact bound when the ILP
backend proves optimality.

The implementation should not add a new distance algorithm. It should thread
the existing `qec-ilp-core::BinaryIlpConfig` through `qec-code`, add a small
qec-code-facing solver option type, and extend qec-ilp-core solution metadata so
callers can distinguish optimal solutions from time-limited incumbents.

## Current State

`code css-distance exact` currently parses only an input source and `--json`.
It calls `compute_distance(css.code())`, and the ILP implementation constructs
`qec_ilp_core::BinaryIlpConfig::default()` internally.

`qec-ilp-core` already supports:

- `BackendKind::{Auto, Highs, Gurobi}`
- `BackendConfig.time_limit_seconds`
- `BackendConfig.mip_gap`
- `BackendConfig.threads`
- `BackendConfig.verbose`
- Gurobi support behind the `qec-ilp-core/gurobi` feature

The backends currently accept time-limited incumbent solutions as successful
`ModelSolution` values without exposing whether optimality was certified. That
is not enough for the exact distance CLI, because an incumbent is only an upper
bound until the backend proves optimality.

There is no existing PR for issue #227.

## Goals

- Add `--backend auto|highs|gurobi` to `code css-distance exact`; default is
  `auto`.
- Add `--time-limit-seconds <float-or-integer>` and forward it to
  `BinaryIlpConfig.backend.time_limit_seconds`.
- Add optional pass-through flags:
  - `--mip-gap <float>`
  - `--threads <usize>`
  - `--verbose-solver`
- Add a qec-code feature that enables `qec-ilp-core/gurobi` for the CLI.
- Fail clearly when `--backend gurobi` is requested without the Gurobi feature.
- Include requested and used backend provenance plus solver option values in
  exact-distance JSON output.
- Report `status: "completed"` and `bound_type: "exact"` only for
  solver-certified optimal results.
- Report `status: "timeout"` and avoid `bound_type: "exact"` when the solver
  returns a time-limited incumbent.

## Non-Goals

- Do not change CSS code construction, logical basis extraction, or ILP
  lowering.
- Do not add a new exact distance algorithm.
- Do not make Gurobi a default dependency.
- Do not require benchmark harness changes in this PR.
- Do not guarantee JSON output for solver failures that occur before any
  incumbent exists; those remain CLI errors.

## Approaches Considered

### 1. Thread an explicit ILP config through qec-code and expose solver status

Add a qec-code distance configuration type, convert it to
`qec_ilp_core::BinaryIlpConfig` under the ILP feature, and add solution metadata
to `qec-ilp-core::ModelSolution`. The CLI result can then serialize requested
options, the backend actually built, and whether the solution is exact or
time-limited.

Benefits:

- satisfies the benchmark CLI use case directly
- keeps qec-ilp-core as the single owner of backend status
- avoids ad hoc parsing of backend error strings
- preserves the existing default command behavior

Costs:

- touches both `qec-code` and `qec-ilp-core`
- requires test updates for solution struct literals

This is the chosen approach.

### 2. Add CLI flags but keep using `compute_distance`

Parse the new flags into the JSON output, but continue using the default
`compute_distance` path.

Benefits:

- smallest qec-code diff

Costs:

- backend and time-limit options would not affect the solver
- benchmark provenance would be misleading
- timeout incumbents would still be reported as exact

This is rejected.

### 3. Build the ILP backend directly in the CLI

Move exact-distance ILP construction into `cli.rs` so the command can call
`build_binary_backend` directly.

Benefits:

- exposes the lower-level configuration quickly

Costs:

- duplicates distance computation responsibilities in the CLI
- makes post-validation and exhaustive fallback harder to keep consistent
- weakens testable boundaries between CLI parsing and distance solving

This is rejected.

## Design

### qec-ilp-core

Add solver provenance to the backend abstraction:

- `BinaryBackend::kind(&self) -> BackendKind`
- `ModelSolutionStatus::{Optimal, TimeLimit, SolutionLimit, SubOptimal}`
- `ModelSolution { binary_values, status }`

HiGHS maps `Optimal` to `ModelSolutionStatus::Optimal` and
`ReachedTimeLimit` with a feasible solution to `ModelSolutionStatus::TimeLimit`.
HiGHS time-limit runs without a feasible solution remain errors.

Gurobi maps `Optimal` to `Optimal`, `TimeLimit` with an incumbent to
`TimeLimit`, `SolutionLimit` with an incumbent to `SolutionLimit`, and
`SubOptimal` with an incumbent to `SubOptimal`. The Gurobi backend still exists
only behind `qec-ilp-core/gurobi`.

### qec-code Distance API

Keep `compute_distance(code)` as the default compatibility entry point. Add a
new exact CSS distance run path that accepts solver options and returns an enum:

- completed exact result when solver status is `Optimal`
- timeout/incomplete result when solver status is not certified exact

Under non-ILP builds, default options continue to use the existing exhaustive
search path. Non-default solver options should fail with a clear unsupported
configuration error because exhaustive search cannot honor backend settings.

### CLI

`ExactCssDistanceCli` gains:

- `--backend`, defaulting to `auto`
- `--time-limit-seconds`
- `--mip-gap`
- `--threads`
- `--verbose-solver`

CLI parsing should reject non-finite or non-positive time limits, negative or
non-finite MIP gaps, and zero threads before solving.

### JSON Contract

Completed exact runs keep existing fields and add provenance fields:

```json
{
  "status": "completed",
  "distance": 3,
  "method": "rstim-ilp-exact",
  "bound_type": "exact",
  "backend": "highs",
  "requested_backend": "highs",
  "time_limit_seconds": 300
}
```

Timeout/incomplete incumbent runs should serialize with a non-exact bound:

```json
{
  "status": "timeout",
  "distance": 5,
  "method": "rstim-ilp-exact",
  "bound_type": "upper",
  "backend": "highs",
  "requested_backend": "highs",
  "time_limit_seconds": 0.001
}
```

The existing `options` object should also include the solver options so old
consumers that group input/configuration under `options` can audit the command
line without reading top-level fields.

## Testing Design

Use TDD with focused tests first:

- qec-code exact JSON serialization includes default and explicit solver
  options.
- CLI accepts `--backend highs --time-limit-seconds 300` and reports requested
  and used backend provenance.
- CLI rejects `--backend gurobi` without the Gurobi feature with an unavailable
  backend error.
- CLI rejects invalid time limit, MIP gap, and threads values.
- qec-ilp-core unit tests verify HiGHS and Gurobi status mapping helpers.
- qec-code distance unit tests verify non-optimal solution metadata maps to
  timeout/incomplete JSON rather than exact JSON.

Required final verification:

```sh
cargo test
```
