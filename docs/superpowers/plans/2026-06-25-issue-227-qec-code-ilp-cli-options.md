# Issue 227 qec-code ILP CLI Options Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose exact CSS distance ILP backend and solver run-control options through `qec-code code css-distance exact` and report auditable JSON provenance.

**Architecture:** Add solver status metadata to `qec-ilp-core::ModelSolution` so qec-code can tell certified optimal solves from time-limited incumbents. Add qec-code distance solver option/provenance types, thread them from CLI parsing into the existing exact distance computation, and serialize requested/used backend plus solver limits in JSON. Keep `compute_distance(code)` as the compatibility wrapper over default options.

**Tech Stack:** Rust 2024, clap derive, serde/serde_json, qec-code, qec-ilp-core, Cargo integration tests.

## Global Constraints

- Add `--backend auto|highs|gurobi` to `code css-distance exact`; default is `auto`.
- Add `--time-limit-seconds <float-or-integer>` and forward it to `BinaryIlpConfig.backend.time_limit_seconds`.
- Add optional pass-through flags: `--mip-gap <float>`, `--threads <usize>`, and `--verbose-solver`.
- Add a qec-code feature that enables `qec-ilp-core/gurobi` for the CLI.
- Fail clearly when `--backend gurobi` is requested without the Gurobi feature.
- Include requested and used backend provenance plus solver option values in exact-distance JSON output.
- Report `status: "completed"` and `bound_type: "exact"` only for solver-certified optimal results.
- Report `status: "timeout"` and avoid `bound_type: "exact"` when the solver returns a time-limited incumbent.
- Do not change CSS code construction, logical basis extraction, or ILP lowering.
- Do not add a new exact distance algorithm.
- Do not make Gurobi a default dependency.
- Do not guarantee JSON output for solver failures that occur before any incumbent exists; those remain CLI errors.
- Required final verification command: `cargo test`.

---

### Task 1: Exact CSS Distance Solver Options And Provenance

**Files:**
- Modify: `qec-ilp-core/src/model.rs`
- Modify: `qec-ilp-core/src/backend/mod.rs`
- Modify: `qec-ilp-core/src/backend/highs.rs`
- Modify: `qec-ilp-core/src/backend/gurobi.rs`
- Modify: `qec-code/Cargo.toml`
- Modify: `qec-code/src/distance.rs`
- Modify: `qec-code/src/distance_exact.rs`
- Modify: `qec-code/src/cli.rs`
- Test: `qec-code/tests/distance_exact.rs`
- Test: `qec-code/tests/cli.rs`
- Test: `qec-ilp-core/src/backend/highs.rs`
- Test: `qec-ilp-core/src/backend/gurobi.rs`

**Interfaces:**
- Produces: `qec_ilp_core::ModelSolutionStatus::{Optimal, TimeLimit, SolutionLimit, SubOptimal}`.
- Produces: `qec_ilp_core::ModelSolution { binary_values: Vec<bool>, status: ModelSolutionStatus }`.
- Produces: `qec_ilp_core::backend::BinaryBackend::kind(&self) -> BackendKind`.
- Produces: qec-code exact-distance solver option types:
  - `ExactCssDistanceBackend::{Auto, Highs, Gurobi}`
  - `ExactCssDistanceSolverOptions { backend, time_limit_seconds, mip_gap, threads, verbose_solver }`
  - `ExactCssDistanceSolverStatus::{Optimal, TimeLimit, SolutionLimit, SubOptimal}`
  - `ExactCssDistanceSolverReport { backend, status }`
- Produces: `compute_distance_with_solver_options(code: &StabilizerCode, solver: ExactCssDistanceSolverOptions) -> Result<ExactCssDistanceComputation>`.
- Consumes: existing `compute_distance(code: &StabilizerCode) -> Result<DistanceResult>` callers continue to work unchanged.

- [ ] **Step 1: Write RED tests for JSON solver option serialization**

Add these assertions to `qec-code/tests/distance_exact.rs`. Keep the existing helper `sample_distance_result()` and update existing `ExactCssDistanceOptions` literals to include `solver: ExactCssDistanceSolverOptions::default()`.

```rust
use qec_code::distance_exact::{
    ExactCssDistanceBackend, ExactCssDistanceSolverOptions,
    ExactCssDistanceSolverReport, ExactCssDistanceSolverStatus,
};

#[test]
fn exact_css_distance_result_serializes_solver_provenance_for_completed_runs() {
    let result = ExactCssDistanceResult::completed_with_solver_report(
        sample_distance_result(),
        ExactCssDistanceOptions {
            input: ExactCssDistanceInput::CodeId {
                code_id: "steane".to_owned(),
            },
            solver: ExactCssDistanceSolverOptions {
                backend: ExactCssDistanceBackend::Highs,
                time_limit_seconds: Some(300.0),
                mip_gap: Some(0.001),
                threads: Some(2),
                verbose_solver: true,
            },
        },
        Some(ExactCssDistanceSolverReport {
            backend: ExactCssDistanceBackend::Highs,
            status: ExactCssDistanceSolverStatus::Optimal,
        }),
    );

    let json = serde_json::to_value(&result).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["bound_type"], "exact");
    assert_eq!(json["requested_backend"], "highs");
    assert_eq!(json["backend"], "highs");
    assert_eq!(json["solver_status"], "optimal");
    assert_eq!(json["time_limit_seconds"], 300.0);
    assert_eq!(json["mip_gap"], 0.001);
    assert_eq!(json["threads"], 2);
    assert_eq!(json["verbose_solver"], true);
    assert_eq!(json["options"]["backend"], "highs");
    assert_eq!(json["options"]["time_limit_seconds"], 300.0);
    assert_eq!(json["options"]["mip_gap"], 0.001);
    assert_eq!(json["options"]["threads"], 2);
    assert_eq!(json["options"]["verbose_solver"], true);
}

#[test]
fn exact_css_distance_result_serializes_time_limited_incumbent_as_upper_bound() {
    let result = ExactCssDistanceResult::completed_with_solver_report(
        sample_distance_result(),
        ExactCssDistanceOptions {
            input: ExactCssDistanceInput::CodeId {
                code_id: "steane".to_owned(),
            },
            solver: ExactCssDistanceSolverOptions {
                backend: ExactCssDistanceBackend::Highs,
                time_limit_seconds: Some(0.001),
                mip_gap: None,
                threads: None,
                verbose_solver: false,
            },
        },
        Some(ExactCssDistanceSolverReport {
            backend: ExactCssDistanceBackend::Highs,
            status: ExactCssDistanceSolverStatus::TimeLimit,
        }),
    );

    let json = serde_json::to_value(&result).unwrap();

    assert_eq!(json["status"], "timeout");
    assert_eq!(json["bound_type"], "upper");
    assert_eq!(json["requested_backend"], "highs");
    assert_eq!(json["backend"], "highs");
    assert_eq!(json["solver_status"], "time_limit");
    assert_eq!(json["time_limit_seconds"], 0.001);
}
```

- [ ] **Step 2: Run RED serialization tests**

Run:

```sh
cargo test -p qec-code --test distance_exact exact_css_distance_result_serializes
```

Expected: FAIL to compile because `ExactCssDistanceBackend`, solver option/report/status types, and `completed_with_solver_report` do not exist yet.

- [ ] **Step 3: Implement exact result and solver option types**

In `qec-code/src/distance_exact.rs`, add the solver option and solver report types. Update `ExactDistanceBoundType` to include `Upper`, `ExactCssDistanceStatus` to include `Timeout` and `Incomplete`, and `ExactCssDistanceOptions` to include `pub solver: ExactCssDistanceSolverOptions`.

Use this shape for result construction:

```rust
impl ExactCssDistanceResult {
    pub fn completed(distance: DistanceResult, options: ExactCssDistanceOptions) -> Self {
        Self::completed_with_solver_report(distance, options, None)
    }

    pub fn completed_with_solver_report(
        distance: DistanceResult,
        options: ExactCssDistanceOptions,
        solver_report: Option<ExactCssDistanceSolverReport>,
    ) -> Self {
        let solver_status = solver_report.map(|report| report.status);
        let backend = solver_report.map(|report| report.backend);
        let is_exact = solver_status
            .map(ExactCssDistanceSolverStatus::is_exact)
            .unwrap_or(true);
        let status = match solver_status {
            Some(ExactCssDistanceSolverStatus::TimeLimit) => ExactCssDistanceStatus::Timeout,
            Some(status) if !status.is_exact() => ExactCssDistanceStatus::Incomplete,
            _ => ExactCssDistanceStatus::Completed,
        };
        let bound_type = if is_exact {
            ExactDistanceBoundType::Exact
        } else {
            ExactDistanceBoundType::Upper
        };

        Self {
            status,
            distance: distance.distance,
            method: ExactCssDistanceMethod::RstimIlpExact,
            bound_type,
            logical_class: distance.logical_class,
            witness: DistanceBoundWitness::from_pauli(&distance.witness),
            requested_backend: options.solver.backend,
            backend,
            solver_status,
            time_limit_seconds: options.solver.time_limit_seconds,
            mip_gap: options.solver.mip_gap,
            threads: options.solver.threads,
            verbose_solver: options.solver.verbose_solver,
            options,
            provenance: ExactCssDistanceProvenance::current(),
        }
    }
}
```

The `ExactCssDistanceResult` fields should include:

```rust
pub requested_backend: ExactCssDistanceBackend,
#[serde(skip_serializing_if = "Option::is_none")]
pub backend: Option<ExactCssDistanceBackend>,
#[serde(skip_serializing_if = "Option::is_none")]
pub solver_status: Option<ExactCssDistanceSolverStatus>,
#[serde(skip_serializing_if = "Option::is_none")]
pub time_limit_seconds: Option<f64>,
#[serde(skip_serializing_if = "Option::is_none")]
pub mip_gap: Option<f64>,
#[serde(skip_serializing_if = "Option::is_none")]
pub threads: Option<u32>,
#[serde(default, skip_serializing_if = "is_false")]
pub verbose_solver: bool,
```

Add a private helper:

```rust
fn is_false(value: &bool) -> bool {
    !*value
}
```

- [ ] **Step 4: Run GREEN serialization tests**

Run:

```sh
cargo test -p qec-code --test distance_exact
```

Expected: PASS.

- [ ] **Step 5: Write RED qec-ilp-core status mapping tests**

Update `qec-ilp-core/src/backend/highs.rs` tests so the helper returns statuses:

```rust
#[test]
fn maps_optimal_solution_status() {
    assert_eq!(
        accepted_model_solution_status(
            HighsModelStatus::Optimal,
            HighsSolutionStatus::Feasible,
        ),
        Some(ModelSolutionStatus::Optimal),
    );
}

#[test]
fn maps_time_limited_feasible_solution_status() {
    assert_eq!(
        accepted_model_solution_status(
            HighsModelStatus::ReachedTimeLimit,
            HighsSolutionStatus::Feasible,
        ),
        Some(ModelSolutionStatus::TimeLimit),
    );
}
```

Update `qec-ilp-core/src/backend/gurobi.rs` tests:

```rust
#[test]
fn maps_gurobi_time_limit_with_incumbent() {
    assert_eq!(
        accepted_gurobi_solution_status(Status::TimeLimit, 1),
        Some(ModelSolutionStatus::TimeLimit),
    );
}
```

- [ ] **Step 6: Run RED qec-ilp-core status tests**

Run:

```sh
cargo test -p qec-ilp-core maps_
```

Expected: FAIL to compile because `ModelSolutionStatus` and the new helper names do not exist yet.

- [ ] **Step 7: Implement qec-ilp-core solver status and backend kind**

In `qec-ilp-core/src/model.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSolutionStatus {
    Optimal,
    TimeLimit,
    SolutionLimit,
    SubOptimal,
}
```

and add `pub status: ModelSolutionStatus` to `ModelSolution`.

In `qec-ilp-core/src/backend/mod.rs`, add `fn kind(&self) -> BackendKind;` to `BinaryBackend`.

In `qec-ilp-core/src/backend/highs.rs`:

- import `BackendKind` and `ModelSolutionStatus`
- implement `fn kind(&self) -> BackendKind { BackendKind::Highs }`
- replace the bool status helper with:

```rust
fn accepted_model_solution_status(
    model_status: HighsModelStatus,
    primal_status: HighsSolutionStatus,
) -> Option<ModelSolutionStatus> {
    match model_status {
        HighsModelStatus::Optimal => Some(ModelSolutionStatus::Optimal),
        HighsModelStatus::ReachedTimeLimit if primal_status == HighsSolutionStatus::Feasible => {
            Some(ModelSolutionStatus::TimeLimit)
        }
        _ => None,
    }
}
```

- set `ModelSolution { binary_values, status: solution_status }`.

In `qec-ilp-core/src/backend/gurobi.rs`:

- import `BackendKind` and `ModelSolutionStatus`
- implement `fn kind(&self) -> BackendKind { BackendKind::Gurobi }`
- replace the bool status helper with:

```rust
fn accepted_gurobi_solution_status(status: Status, sol_count: i32) -> Option<ModelSolutionStatus> {
    match status {
        Status::Optimal => Some(ModelSolutionStatus::Optimal),
        Status::TimeLimit if sol_count > 0 => Some(ModelSolutionStatus::TimeLimit),
        Status::SolutionLimit if sol_count > 0 => Some(ModelSolutionStatus::SolutionLimit),
        Status::SubOptimal if sol_count > 0 => Some(ModelSolutionStatus::SubOptimal),
        _ => None,
    }
}
```

- set `ModelSolution { binary_values, status: solution_status }`.

- [ ] **Step 8: Run GREEN qec-ilp-core tests**

Run:

```sh
cargo test -p qec-ilp-core
```

Expected: PASS.

- [ ] **Step 9: Write RED qec-code CLI and distance tests**

Add focused tests to `qec-code/tests/cli.rs`:

```rust
#[cfg(feature = "distance-ilp-highs")]
#[test]
fn code_css_distance_exact_accepts_highs_backend_and_solver_limits() {
    let output = run_qec_code(&[
        "code",
        "css-distance",
        "exact",
        "--code-id",
        "steane",
        "--backend",
        "highs",
        "--time-limit-seconds",
        "300",
        "--mip-gap",
        "0.001",
        "--threads",
        "1",
        "--verbose-solver",
        "--json",
    ]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["status"], "completed");
    assert_eq!(json["bound_type"], "exact");
    assert_eq!(json["requested_backend"], "highs");
    assert_eq!(json["backend"], "highs");
    assert_eq!(json["solver_status"], "optimal");
    assert_eq!(json["time_limit_seconds"], 300.0);
    assert_eq!(json["mip_gap"], 0.001);
    assert_eq!(json["threads"], 1);
    assert_eq!(json["verbose_solver"], true);
    assert_eq!(json["options"]["backend"], "highs");
}

#[test]
fn code_css_distance_exact_rejects_gurobi_backend_without_feature() {
    let output = run_qec_code(&[
        "code",
        "css-distance",
        "exact",
        "--code-id",
        "steane",
        "--backend",
        "gurobi",
        "--json",
    ]);

    if cfg!(feature = "distance-ilp-gurobi") {
        assert!(output.status.success());
    } else {
        assert!(!output.status.success());
        assert_eq!(output.stdout, b"");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("ILP backend is unavailable"), "stderr was: {stderr}");
        assert!(stderr.contains("Gurobi"), "stderr was: {stderr}");
    }
}

#[test]
fn run_code_css_distance_exact_rejects_invalid_solver_options() {
    for (flag, value, expected) in [
        ("--time-limit-seconds", "0", "time_limit_seconds"),
        ("--time-limit-seconds", "NaN", "time_limit_seconds"),
        ("--mip-gap", "-0.1", "mip_gap"),
        ("--mip-gap", "NaN", "mip_gap"),
        ("--threads", "0", "threads"),
    ] {
        let result = run_qec_code_in_process(&[
            "code",
            "css-distance",
            "exact",
            "--code-id",
            "steane",
            flag,
            value,
            "--json",
        ]);
        assert!(
            matches!(
                result,
                Err(QecError::InvalidCssDistanceInput(message)) if message.contains(expected)
            ),
            "expected invalid {expected} error for {flag} {value}, got {result:?}",
        );
    }
}
```

- [ ] **Step 10: Run RED qec-code CLI tests**

Run:

```sh
cargo test -p qec-code --test cli code_css_distance_exact_rejects_gurobi_backend_without_feature
cargo test -p qec-code --test cli run_code_css_distance_exact_rejects_invalid_solver_options
```

Expected: FAIL because the CLI flags and validation do not exist yet.

- [ ] **Step 11: Implement qec-code distance solver options and CLI flags**

In `qec-code/Cargo.toml`, add:

```toml
distance-ilp-gurobi = ["distance-ilp-highs", "qec-ilp-core/gurobi"]
```

In `qec-code/src/distance.rs`:

- add `ExactCssDistanceComputation { distance: DistanceResult, solver_report: Option<ExactCssDistanceSolverReport> }`
- add `compute_distance_with_solver_options`
- have `compute_distance` call the new function with `ExactCssDistanceSolverOptions::default()` and return `.distance`
- under `distance-ilp-highs`, build `BinaryIlpConfig` from solver options and record `backend.kind()` before `solve()`
- convert `qec_ilp_core::ModelSolutionStatus` to `ExactCssDistanceSolverStatus`
- under non-ILP builds, accept only default solver options and otherwise return `QecError::DistanceComputationUnsupported`.

In `qec-code/src/cli.rs`:

- add `#[arg(long, value_enum, default_value_t = ExactCssDistanceBackend::Auto)] backend: ExactCssDistanceBackend`
- add `time_limit_seconds`, `mip_gap`, `threads`, and `verbose_solver` fields
- validate all solver options before solving
- build `ExactCssDistanceOptions { input, solver }`
- call `compute_distance_with_solver_options(css.code(), options.solver)` and construct `ExactCssDistanceResult::completed_with_solver_report(computation.distance, options, computation.solver_report)`.

Validation helper messages should include these option names:

- `time_limit_seconds`
- `mip_gap`
- `threads`

- [ ] **Step 12: Run GREEN qec-code focused tests**

Run:

```sh
cargo test -p qec-code --test distance_exact
cargo test -p qec-code --test cli code_css_distance_exact_rejects_gurobi_backend_without_feature
cargo test -p qec-code --test cli run_code_css_distance_exact_rejects_invalid_solver_options
```

Expected: PASS.

- [ ] **Step 13: Run ILP-enabled focused test**

Run:

```sh
cargo test -p qec-code --features distance-ilp-highs --test cli code_css_distance_exact_accepts_highs_backend_and_solver_limits
```

Expected: PASS.

- [ ] **Step 14: Run formatting and final verification**

Run:

```sh
cargo fmt --all
cargo test
```

Expected: PASS. Existing pre-existing warnings in unrelated rmatching tests may still appear; no new warnings should be introduced by qec-code or qec-ilp-core.

- [ ] **Step 15: Commit**

Run:

```sh
git status --short
git add qec-ilp-core/src/model.rs qec-ilp-core/src/backend/mod.rs qec-ilp-core/src/backend/highs.rs qec-ilp-core/src/backend/gurobi.rs qec-code/Cargo.toml qec-code/src/distance.rs qec-code/src/distance_exact.rs qec-code/src/cli.rs qec-code/tests/distance_exact.rs qec-code/tests/cli.rs docs/superpowers/plans/2026-06-25-issue-227-qec-code-ilp-cli-options.md
git commit -m "feat: expose qec-code exact distance ILP options"
```
