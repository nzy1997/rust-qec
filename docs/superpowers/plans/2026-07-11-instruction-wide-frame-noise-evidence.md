# Instruction-Wide Frame Noise Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add release-build frame-noise telemetry and publish a checked instruction-wide evidence bundle for issue #463.

**Architecture:** The Rust CLI emits actual runtime telemetry only when built with `benchmark-telemetry`. A Python runner consumes that telemetry, independently inspects the fixture, runs a separate correctness comparison, writes deterministic artifacts, and a checker recomputes every semantic claim before validating hashes.

**Tech Stack:** Rust 2024, clap, serde/serde_json, Python 3 stdlib, Stim CLI/Python `1.15.0`, existing benchmark manifest helpers.

## Global Constraints

- Base branch is `master`; worker branch is `agent/issue-463-publish-instruction-wide-frame-noise-evidence-run-1`.
- Add Cargo feature `benchmark-telemetry`.
- Release binary built with `--features benchmark-telemetry` must support `--benchmark-telemetry-json <path>`.
- Canonical case is `stim_surface_d11_r100` from `benchmarks/rstim_vs_stim_simulator/cases.full.toml`.
- Fixture SHA-256 must be `a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229`.
- Manifest SHA-256 must be `9fc35393f362f709e90bfd64ab08eda5140844974a7e685fd1e5614f67e0c921`.
- Stim version must be `1.15.0`.
- Measurements use `shots=1024`, `seed=7`, `warmup-rounds=0`, and `measure-rounds=1`.
- Required runtime totals are `X_ERROR` builds `203` attempts `24946688`, `DEPOLARIZE1` builds `200` attempts `12288000`, `DEPOLARIZE2` builds `400` attempts `45056000`, total builds `803`, total attempts `82290688`.
- Independent fixture load totals are `X_ERROR` targets `24362`, `DEPOLARIZE1` targets `12000`, `DEPOLARIZE2` pairs `44000`, total legacy setups `80362`.
- Measurement timer scope is process spawn through complete stdout/stderr drain and observed exit.
- Measurement output is `12121` bits, `1516` bytes per shot, and `1552384` bytes for 1024 shots.
- Correctness output must be a passing `detect` comparison for `12000` detectors plus one observable.
- `artifact-sha256.json` hashes the other six files only.
- Out of scope: earlier release artifacts, site provenance, DEM sampling, SIMD/alignment requirements, and timing thresholds.

---

### Task 1: Rust Telemetry Feature

**Files:**
- Modify: `rstim/Cargo.toml`
- Modify: `rstim/src/sim/frame.rs`
- Modify: `rstim/src/cli.rs`
- Test: `rstim/tests/frame_instruction_wide_one_qubit_noise.rs`
- Test: `rstim/tests/frame_instruction_wide_depolarize2.rs`

**Interfaces:**
- Produces: CLI flag `--benchmark-telemetry-json <path>` and JSON object `{"operations":[...]}` with per-instruction telemetry records.
- Consumes: existing frame simulator instruction-wide telemetry and sparse iterator counters.

- [ ] **Step 1: Write failing tests**

Add assertions in existing frame instruction-wide tests that accumulated telemetry contains per-instruction records for `X_ERROR`, `DEPOLARIZE1`, and `DEPOLARIZE2` without changing the existing last-instruction telemetry assertions.

- [ ] **Step 2: Run red tests**

Run `cargo test -p rstim --test frame_instruction_wide_one_qubit_noise -- --nocapture` and `cargo test -p rstim --test frame_instruction_wide_depolarize2 -- --nocapture`. Expected failure: accumulated telemetry API is missing.

- [ ] **Step 3: Implement telemetry**

Define a `telemetry_enabled` cfg pattern using `any(debug_assertions, feature = "benchmark-telemetry")`, add `FrameNoiseTelemetryRecord`, reset/take helpers, record one row per executed sparse/dense operation, and preserve existing debug helper behavior.

- [ ] **Step 4: Wire the CLI**

Add a global `benchmark_telemetry_json` CLI option. Reset telemetry before executing commands that can sample, and after a successful command write `{"operations":[...]}` to the requested path. If the feature is absent and the flag is supplied, return a clear error.

- [ ] **Step 5: Verify**

Run both focused Rust tests again. Expected result: pass.

### Task 2: Runner

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/run_frame_instruction_wide_benchmark.py`
- Create: `benchmarks/rstim_vs_stim_simulator/tests/test_run_frame_instruction_wide_benchmark.py`

**Interfaces:**
- Consumes: CLI specified in issue #463.
- Produces: `raw.jsonl`, `summary.json`, `report.md`, `environment.json`, `fixture-load.json`, `correctness-summary.json`, and `artifact-sha256.json`.

- [ ] **Step 1: Write failing tests**

Create fake rstim/stim binaries. Test that the runner rejects a fake rstim that emits valid b8 sample bytes but omits telemetry, and that a fake telemetry-emitting binary produces all seven artifacts with hashes over the six non-manifest files.

- [ ] **Step 2: Run red tests**

Run `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_frame_instruction_wide_benchmark -q`. Expected failure: module missing.

- [ ] **Step 3: Implement runner**

Resolve and validate the manifest case, invoke the existing fixture inspector, run measured `rstim sample --out_format b8`, require telemetry JSON, aggregate operation rows, run separate `stim detect` and `rstim detect` correctness comparisons, collect environment/provenance, and write artifact hashes.

- [ ] **Step 4: Verify**

Run the runner unit test again. Expected result: pass.

### Task 3: Checker

**Files:**
- Create: `tools/check_rstim_vs_stim_instruction_wide_noise_evidence.py`
- Create: `tools/test_check_rstim_vs_stim_instruction_wide_noise_evidence.py`

**Interfaces:**
- Consumes: a bundle directory via `--dir`.
- Produces: `PASS instruction-wide frame-noise evidence builds=803 attempts=82290688 legacy_setups=80362` on success.

- [ ] **Step 1: Write failing tests**

Cover raw-summary-report recomputation and the required negative controls: raw iterator builds changed to `80362`, failed or sample-mode correctness, mismatched fixture/manifest/binary/artifact hash, and missing `artifact-sha256.json`.

- [ ] **Step 2: Run red tests**

Run `python3 -m unittest tools.test_check_rstim_vs_stim_instruction_wide_noise_evidence -q`. Expected failure: checker missing.

- [ ] **Step 3: Implement checker**

Validate required files, raw rows, summary derivation, report rendering, fixture-load semantics, correctness semantics, environment provenance and artifact digests, then validate mandatory canonical hashes.

- [ ] **Step 4: Verify**

Run checker unit tests. Expected result: pass.

### Task 4: Publish Evidence And Final Verification

**Files:**
- Create directory: `benchmarks/rstim_vs_stim_simulator/results/frame-instruction-wide-release/`
- Create artifacts: `raw.jsonl`, `summary.json`, `report.md`, `environment.json`, `fixture-load.json`, `correctness-summary.json`, `artifact-sha256.json`

**Interfaces:**
- Consumes: release `target/release/rstim` with `benchmark-telemetry`.
- Produces: committed checked evidence bundle.

- [ ] **Step 1: Run issue verification commands**

Run the exact build, runner, checker, published checker, and unittest commands from issue #463.

- [ ] **Step 2: Run broad verification**

Run `cargo test`.

- [ ] **Step 3: Commit and PR**

Commit scoped changes, push the worker branch, and create a pull request against `master`.
