# Issue #212 BB BP-OSD Decode Profiling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add structured decode-phase timing and OSD/GF(2) counters for the Rust BB circuit BP-OSD path.

**Architecture:** Add `DecodeStats` to `rbposd` decode results, count OSD candidate and GF(2) work where it occurs, and aggregate those stats in `rsinter::bb_circuit_memory`. Expose the BB profile through `SimulationResult` plus a focused result-row helper/validator for tests.

**Tech Stack:** Rust 2024, `std::time::Instant`, existing `rbposd` and `rsinter` crates, existing `BenchmarkResultRow` JSON-compatible metrics.

## Global Constraints

- Keep instrumentation small and opt-in by surface; do not add profiler dependencies.
- Stable metric names must include `setup_seconds`, `sample_seconds`, `decode_seconds`, `bp_seconds`, `osd_seconds`, `decode_call_count`, `z_decode_call_count`, `x_decode_call_count`, `bp_iteration_count`, `osd_use_count`, `osd_candidate_count`, `gf2_solve_count`, and `gf2_full_elimination_count`.
- Count only actual decoder calls. When Z decoding predicts failure, X decoding is skipped and contributes zero to `x_decode_call_count`.
- For completed BB BP-OSD rows, `decode_call_count == z_decode_call_count + x_decode_call_count`.
- `osd_candidate_count` is zero for OSD-0 control runs and positive for the BB90 hard fixture diagnostic path that reaches OSD candidate search.
- Current GF(2) counters describe existing behavior only; do not optimize or change OSD semantics in this issue.
- Tests must inspect structured profile/result-row data, not human CLI text.
- Preserve the existing `rsinter bb-circuit-bposd-memory` four-column CLI output.
- Required verification commands:
  - `cargo test -p rsinter bb_circuit_bposd_timing_counters_partition_decode_work -- --nocapture`
  - `cargo test -p rsinter bb90_hard_syndrome_reports_osd_profile_counters -- --nocapture`
  - `cargo test -p rsinter bb_circuit_bposd_timing_counters_reject_incomplete_rows -q`
  - `cargo test`

---

## File Structure

- Modify: `rbposd/src/decoder.rs`
  - Add public `DecodeStats`, attach it to `DecodeResult`, and time BP/OSD phases in `BpOsdDecoder`.
- Modify: `rbposd/src/osd.rs`
  - Return OSD counter stats from `decode_osd_with_workspace`.
- Modify: `rbposd/src/gf2.rs`
  - Add per-call GF(2) solve/full-elimination counters for detailed solves.
- Modify: `rbposd/src/lsd_decoder.rs`
  - Populate `DecodeResult.stats` for LSD decodes with BP/decode counters and zero OSD counters.
- Modify: `rbposd/src/lib.rs`
  - Re-export `DecodeStats`.
- Modify: `rbposd/tests/osd.rs`
  - Add direct tests for OSD-0 and OSD candidate/GF(2) counters.
- Modify: `rsinter/src/bb_circuit_memory.rs`
  - Add `BbCircuitBposdProfile`, aggregate Z/X stats, profile replay helper, result-row helper, and completed-row validator.
- Modify: `rsinter/tests/bb_circuit_memory.rs`
  - Add the tiny-run profile and negative-control tests requested by #212.
- Modify: `rsinter/tests/bb90_hard_syndrome_fixture.rs`
  - Add the hard fixture profile diagnostic test requested by #212.

## Task 1: rbposd Per-Decode Stats

**Files:**
- Modify: `rbposd/src/decoder.rs`
- Modify: `rbposd/src/osd.rs`
- Modify: `rbposd/src/gf2.rs`
- Modify: `rbposd/src/lsd_decoder.rs`
- Modify: `rbposd/src/lib.rs`
- Modify: `rbposd/tests/osd.rs`

**Interfaces:**
- Produces: `rbposd::DecodeStats`.
- Updates: `rbposd::DecodeResult { stats: DecodeStats, .. }`.
- Produces internally: OSD helper stats with `osd_candidate_count`, `gf2_solve_count`, and `gf2_full_elimination_count`.

- [ ] **Step 1: Write failing rbposd stats tests**

Append tests to `rbposd/tests/osd.rs`:

```rust
#[test]
fn osd0_decode_reports_zero_candidate_and_one_gf2_solve() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 2], vec![1, 2]]).unwrap();
    let decoder = BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.1, 0.9]),
        DecoderConfig {
            max_bp_iterations: 1,
            osd_order: 0,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let result = decoder.decode(&Syndrome::from(vec![true, true])).unwrap();

    assert_eq!(result.stats.decode_call_count, 1);
    assert_eq!(result.stats.osd_use_count, usize::from(result.used_osd));
    assert_eq!(result.stats.osd_candidate_count, 0);
    if result.used_osd {
        assert_eq!(result.stats.gf2_solve_count, 1);
        assert_eq!(result.stats.gf2_full_elimination_count, 1);
    }
}

#[test]
fn osd_order_two_decode_reports_candidate_and_gf2_counters() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap();
    let decoder = BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.49, 0.48, 0.47]),
        DecoderConfig {
            max_bp_iterations: 1,
            osd_order: 2,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let result = decoder.decode(&Syndrome::from(vec![true])).unwrap();

    assert_eq!(result.stats.decode_call_count, 1);
    assert!(result.used_osd);
    assert_eq!(result.stats.osd_use_count, 1);
    assert!(result.stats.osd_candidate_count > 0);
    assert_eq!(
        result.stats.gf2_solve_count,
        result.stats.gf2_full_elimination_count
    );
    assert!(result.stats.gf2_solve_count >= result.stats.osd_candidate_count + 1);
}
```

- [ ] **Step 2: Run RED**

Run:

```sh
cargo test -p rbposd osd0_decode_reports_zero_candidate_and_one_gf2_solve osd_order_two_decode_reports_candidate_and_gf2_counters -- --nocapture
```

Expected: FAIL because `DecodeResult.stats` does not exist.

- [ ] **Step 3: Implement `DecodeStats`**

In `rbposd/src/decoder.rs`, add:

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DecodeStats {
    pub bp_seconds: f64,
    pub osd_seconds: f64,
    pub decode_call_count: usize,
    pub bp_iteration_count: usize,
    pub osd_use_count: usize,
    pub osd_candidate_count: usize,
    pub gf2_solve_count: usize,
    pub gf2_full_elimination_count: usize,
}
```

Add `pub stats: DecodeStats` to `DecodeResult`. Re-export it from
`rbposd/src/lib.rs`.

- [ ] **Step 4: Add GF(2) and OSD counter plumbing**

In `rbposd/src/gf2.rs`, add an internal stats struct and make
`solve_with_column_order_detailed` increment solve/full-elimination counters
once at the start of each call. Keep the existing public/internal return
behavior unchanged by returning the stats alongside `DetailedSolution` only from
a new detailed-with-stats helper, and keep existing callers working through
wrappers.

In `rbposd/src/osd.rs`, return an internal `OsdDecodeOutcome { correction,
stats }` from `decode_osd_with_workspace`. Increment `osd_candidate_count` once
per forced free-column combination visited for `osd_order > 0`.

- [ ] **Step 5: Time BP/OSD phases in decoders**

In `BpOsdDecoder::decode`, use `Instant::now()` around BP and OSD work. Populate
`DecodeStats` for all successful branches. In `BpLsdDecoder::decode`, populate
the new `stats` field with decode count, BP timing, and BP iteration count while
leaving OSD/GF(2) counters at zero.

- [ ] **Step 6: Run GREEN**

Run:

```sh
cargo test -p rbposd osd0_decode_reports_zero_candidate_and_one_gf2_solve osd_order_two_decode_reports_candidate_and_gf2_counters -- --nocapture
cargo test -p rbposd -q
```

Expected: PASS.

## Task 2: BB Circuit Profile Aggregation And Row Validation

**Files:**
- Modify: `rsinter/src/bb_circuit_memory.rs`
- Modify: `rsinter/tests/bb_circuit_memory.rs`

**Interfaces:**
- Produces: `BbCircuitBposdProfile`.
- Updates: `SimulationResult { profile: BbCircuitBposdProfile, .. }`.
- Produces: `bb_circuit_bposd_result_row(code_id: &str, result: &SimulationResult) -> BenchmarkResultRow`.
- Produces: `validate_bposd_profile_result_row(row: &BenchmarkResultRow) -> Result<(), String>`.

- [ ] **Step 1: Write RED rsinter tests**

Append tests to `rsinter/tests/bb_circuit_memory.rs` that call
`run_simulation_for_code`, inspect `SimulationResult.profile`, convert it to a
result row, and validate the row. Use these concrete assertions:

```rust
#[test]
fn bb_circuit_bposd_timing_counters_partition_decode_work() {
    let result = run_simulation_for_code(
        "bb90",
        SimulationConfig {
            physical_error_rate: 1.0e-12,
            num_cycles: 1,
            num_trials: 1,
            seed: Some(1),
            max_bp_iterations: 10,
            osd_order: 0,
        },
    )
    .unwrap();

    let profile = &result.profile;
    assert!(profile.setup_seconds.is_finite());
    assert!(profile.sample_seconds.is_finite());
    assert!(profile.decode_seconds.is_finite());
    assert!(profile.decode_call_count > 0);
    assert_eq!(
        profile.decode_call_count,
        profile.z_decode_call_count + profile.x_decode_call_count
    );
    assert_eq!(profile.osd_candidate_count, 0);
    assert!(profile.bp_iteration_count >= profile.decode_call_count);

    let row = bb_circuit_bposd_result_row("bb90", &result);
    validate_bposd_profile_result_row(&row).unwrap();
    for key in [
        "setup_seconds",
        "sample_seconds",
        "decode_seconds",
        "bp_seconds",
        "osd_seconds",
        "decode_call_count",
        "bp_iteration_count",
        "osd_use_count",
        "osd_candidate_count",
        "gf2_solve_count",
        "gf2_full_elimination_count",
    ] {
        assert!(row.metrics.contains_key(key), "missing metric {key}");
    }
}

#[test]
fn bb_circuit_bposd_timing_counters_reject_incomplete_rows() {
    let mut result = run_simulation_for_code(
        "bb90",
        SimulationConfig {
            physical_error_rate: 1.0e-12,
            num_cycles: 1,
            num_trials: 1,
            seed: Some(1),
            max_bp_iterations: 10,
            osd_order: 0,
        },
    )
    .unwrap();

    let mut missing = bb_circuit_bposd_result_row("bb90", &result);
    missing.metrics.remove("decode_call_count");
    assert!(validate_bposd_profile_result_row(&missing).is_err());

    result.profile.x_decode_call_count += 1;
    let mismatched = bb_circuit_bposd_result_row("bb90", &result);
    assert!(validate_bposd_profile_result_row(&mismatched).is_err());
}
```

The first test must run a tiny OSD-0 control and assert `osd_candidate_count ==
0`, `decode_call_count > 0`, `decode_call_count == z_decode_call_count +
x_decode_call_count`, `bp_iteration_count >= decode_call_count` when
`max_bp_iterations > 0`, and required metric keys exist in the result row.

The negative-control test must reject a completed synthetic BB row missing a
required metric and reject a row where `decode_call_count` disagrees with
basis-specific counts.

- [ ] **Step 2: Run RED**

Run:

```sh
cargo test -p rsinter bb_circuit_bposd_timing_counters_partition_decode_work -- --nocapture
cargo test -p rsinter bb_circuit_bposd_timing_counters_reject_incomplete_rows -q
```

Expected: FAIL because the BB profile/result-row functions do not exist.

- [ ] **Step 3: Implement profile aggregation**

In `rsinter/src/bb_circuit_memory.rs`, add `BbCircuitBposdProfile` with the
metric fields from Global Constraints and helper methods to add Z/X
`rbposd::DecodeStats`. Measure setup around code/cycle/model/decoder creation,
sample time around `simulate_trial`, and decode time around actual decoder
calls. Preserve skipped-X semantics by only incrementing X counts when X decode
is invoked.

- [ ] **Step 4: Implement result-row helper and validator**

Use existing `bench::result::{BenchmarkResultRow, MetricMap, PairMapExt,
ParamMap}` and `failure::FailureKind`. The helper should emit a completed BB
row with required metrics. The validator should check only completed rows whose
benchmark and runner identify the BB BP-OSD profile helper; reject missing,
non-finite, or negative required metrics and reject mismatched decode counts.

- [ ] **Step 5: Run GREEN**

Run:

```sh
cargo test -p rsinter bb_circuit_bposd_timing_counters_partition_decode_work -- --nocapture
cargo test -p rsinter bb_circuit_bposd_timing_counters_reject_incomplete_rows -q
```

Expected: PASS.

## Task 3: BB90 Hard Fixture Profile Diagnostic

**Files:**
- Modify: `rsinter/src/bb_circuit_memory.rs`
- Modify: `rsinter/tests/bb90_hard_syndrome_fixture.rs`

**Interfaces:**
- Produces: `profile_syndrome_replay(model, syndrome_bits, max_bp_iterations, osd_order) -> Result<BbCircuitBposdProfile, String>`.

- [ ] **Step 1: Write RED hard-fixture test**

Add `bb90_hard_syndrome_reports_osd_profile_counters` to
`rsinter/tests/bb90_hard_syndrome_fixture.rs`. It should load the existing
fixture, compute the replay, call the profile helper on the selected fixture
basis, assert `decode_call_count > 0`, `osd_use_count > 0`,
`osd_candidate_count > 0`, and print the profile.

- [ ] **Step 2: Run RED**

Run:

```sh
cargo test -p rsinter bb90_hard_syndrome_reports_osd_profile_counters -- --nocapture
```

Expected: FAIL because the profile replay helper does not exist.

- [ ] **Step 3: Implement replay profiling helper**

In `rsinter/src/bb_circuit_memory.rs`, compile a `BpOsdDecoder` for the provided
model and decode the syndrome once while aggregating stats into a
`BbCircuitBposdProfile`. This helper does not sample or rebuild the model, so
`setup_seconds` and `sample_seconds` are zero and `decode_seconds` covers the
single replay decode.

- [ ] **Step 4: Run GREEN**

Run:

```sh
cargo test -p rsinter bb90_hard_syndrome_reports_osd_profile_counters -- --nocapture
```

Expected: PASS.

## Task 4: Final Verification And Cleanup

**Files:**
- Review all files touched by Tasks 1-3.

**Interfaces:**
- Produces: committed implementation and PR-ready branch.

- [ ] **Step 1: Format**

Run:

```sh
cargo fmt
```

Expected: exits 0.

- [ ] **Step 2: Run required focused checks**

Run:

```sh
cargo test -p rsinter bb_circuit_bposd_timing_counters_partition_decode_work -- --nocapture
cargo test -p rsinter bb90_hard_syndrome_reports_osd_profile_counters -- --nocapture
cargo test -p rsinter bb_circuit_bposd_timing_counters_reject_incomplete_rows -q
```

Expected: all pass.

- [ ] **Step 3: Run full verification**

Run:

```sh
cargo test
```

Expected: all pass.

- [ ] **Step 4: Commit**

Run:

```sh
git add docs/superpowers/specs/2026-06-25-issue-212-bb-bposd-profile-design.md docs/superpowers/plans/2026-06-25-issue-212-bb-bposd-profile.md rbposd/src/decoder.rs rbposd/src/osd.rs rbposd/src/gf2.rs rbposd/src/lsd_decoder.rs rbposd/src/lib.rs rbposd/tests/osd.rs rsinter/src/bb_circuit_memory.rs rsinter/tests/bb_circuit_memory.rs rsinter/tests/bb90_hard_syndrome_fixture.rs
git commit -m "feat: instrument bb bposd decode profiling"
```

Expected: commit succeeds.

- [ ] **Step 5: Finish branch**

Use `superpowers:finishing-a-development-branch`, choose "Push and create a
Pull Request" by standing policy, and stop after the PR is created.
