# Issue #212 BB BP-OSD Decode Profiling Design

Date: 2026-06-25
Status: Non-interactive Agent Desk design, auto-approved by standing policy
Scope: GitHub issue #212, minimal decode-phase timing and counters for the Rust BB circuit BP-OSD path

## Context

Issue #209 reports that the Rust `rsinter bb-circuit-bposd-memory` path is much
slower than upstream `ldpc`/`bposd` on hard BB circuit cases. Issue #210 is
already merged into this branch and provides the BB90 hard-syndrome fixture
needed to exercise the slow path. The next optimization issue, #214, needs
stable counters that prove whether full GF(2) elimination still scales with OSD
candidate enumeration.

The current code has:

- `rbposd::DecodeResult` with correction, BP iteration count, and an OSD-used
  flag.
- `rbposd::OsdPathDiagnostic` from #210, which reports the planned OSD frontier
  without enumerating order-7 candidates.
- `rsinter::bb_circuit_memory::run_simulation_for_code`, which builds effective
  Z/X models, samples trials, decodes Z first, and skips X when Z already
  predicts failure.
- No structured BB result row or validator for required timing/counter fields.

## Goals

- Add a lightweight `rbposd` stats object that reports per-decode timing and
  deterministic counters without adding profiler dependencies.
- Count BP time, OSD time, BP iterations, OSD use, OSD candidates attempted,
  GF(2) solve calls, and full GF(2) eliminations.
- Aggregate these stats in the BB circuit memory simulation across Z and X
  basis decodes.
- Expose setup, sampling, and total decode timing for the BB path.
- Provide a structured result-row helper and validator so tests can inspect
  fields without parsing CLI text.
- Document skipped-X aggregation semantics through the public profile: only
  actually invoked X decodes increment `x_decode_call_count`; skipped X decodes
  contribute zero.
- Keep the implementation scoped to instrumentation. Do not optimize OSD or
  change decoder semantics.

## Non-Goals

- Do not add a broad benchmark dashboard.
- Do not add a Python `ldpc` comparison runner.
- Do not make CI depend on timing thresholds.
- Do not implement the #214 OSD factorization optimization.
- Do not create a full `benchmarks/bb_circuit_bposd/*.toml` campaign surface
  unless it is required by existing code. It is not present in this checkout, so
  this issue adds only a minimal structured BB result-row hook.

## Approaches Considered

### 1. Minimal stats in `rbposd` plus BB aggregation

Add `DecodeStats` to `rbposd::DecodeResult`, collect timing around BP and OSD
inside `BpOsdDecoder::decode`, count GF(2) solve/full-elimination calls inside
`PreparedLinearSystem`, and aggregate the stats in `rsinter::bb_circuit_memory`.

This is the chosen approach. It is narrow, keeps the counter source close to
the work being counted, and gives #214 stable names without requiring a profiler
or benchmark product.

### 2. External timing wrapper around `rsinter`

Measure only setup, sampling, and whole-decode timing in the BB simulation
loop, while inferring BP/OSD split from existing result fields.

This is rejected because it cannot expose GF(2) solve counts or OSD candidate
counts, which are the counters #214 needs.

### 3. Add a generic profiling framework

Introduce a generic profiler layer with spans, events, or per-thread collectors.

This is rejected as overbuilt for the issue. Wall-clock `Instant` timing plus
local deterministic counters is enough.

## Design

### `rbposd` Decode Stats

Add:

```rust
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

`DecodeResult` gains `pub stats: DecodeStats`.

Each successful `BpOsdDecoder::decode` returns `decode_call_count = 1`.
Dimension errors return no `DecodeResult` and therefore no stats. Zero-syndrome
prior fast paths report zero BP/OSD time and zero BP iterations. BP-only
converged decodes report BP timing and iteration count, with zero OSD counters.
OSD decodes report BP timing, OSD timing, `osd_use_count = 1`, and the counters
returned by the OSD/GF(2) path.

`BpLsdDecoder` reuses the same `DecodeResult` type and returns zero OSD/GF(2)
stats except for BP timing/iteration/decode count.

### OSD And GF(2) Counters

`PreparedLinearSystem::solve_with_column_order_detailed` increments both
`gf2_solve_count` and `gf2_full_elimination_count` once per call. This reflects
current behavior before #214: every candidate solve performs a full dense
elimination.

`decode_osd_with_workspace` returns an OSD stats value alongside the correction.
For `osd_order = 0`, candidate count is zero. For order > 0, the base OSD-0
solve is counted as a GF(2) solve/full elimination but not as a candidate. Each
forced free-column combination visited increments `osd_candidate_count` whether
the solve succeeds or fails, because the counter measures attempted candidate
evaluation work.

### BB Circuit Aggregation

Add:

```rust
pub struct BbCircuitBposdProfile {
    pub setup_seconds: f64,
    pub sample_seconds: f64,
    pub decode_seconds: f64,
    pub bp_seconds: f64,
    pub osd_seconds: f64,
    pub decode_call_count: usize,
    pub z_decode_call_count: usize,
    pub x_decode_call_count: usize,
    pub bp_iteration_count: usize,
    pub osd_use_count: usize,
    pub osd_candidate_count: usize,
    pub gf2_solve_count: usize,
    pub gf2_full_elimination_count: usize,
}
```

`SimulationResult` gains `pub profile: BbCircuitBposdProfile`.

`setup_seconds` covers code construction, syndrome-cycle construction,
effective-model construction, and decoder construction. `sample_seconds`
covers trial sampling only. `decode_seconds` covers `BpOsdDecoder::decode`
calls only. The per-decode `bp_seconds` and `osd_seconds` are summed from
`DecodeStats`.

For each trial, Z decode is attempted first. If Z predicts a logical failure,
the trial is failed immediately and X decode is skipped. This means
`x_decode_call_count` counts only invoked X decodes, and
`decode_call_count == z_decode_call_count + x_decode_call_count`.

### Structured Result Row

Add a focused helper in `rsinter::bb_circuit_memory` that converts a
`SimulationResult` into `bench::result::BenchmarkResultRow` with all required
timing and counter metrics. Add a validator:

```rust
pub fn validate_bposd_profile_result_row(row: &BenchmarkResultRow) -> Result<(), String>
```

The validator rejects completed BB BP-OSD rows that omit required fields, use
non-finite or negative timing/counter values, or disagree on
`decode_call_count == z_decode_call_count + x_decode_call_count`.

The existing CLI keeps the four-column human output for compatibility. Tests use
the structured profile/result-row helpers directly.

## Testing

Add rsinter tests with the exact names requested by issue #212:

- `bb_circuit_bposd_timing_counters_partition_decode_work`
- `bb90_hard_syndrome_reports_osd_profile_counters`
- `bb_circuit_bposd_timing_counters_reject_incomplete_rows`

The first test runs a tiny BB memory configuration and inspects the structured
row/profile fields. It also runs an OSD-0 control and asserts candidate count is
zero.

The hard-fixture test replays the #210 BB90 fixture through a profiling helper,
asserts `decode_call_count > 0`, `osd_use_count > 0`, and prints the timing and
counter profile for diagnostic use.

The negative-control test constructs synthetic completed BB rows and verifies
that missing required metrics or mismatched basis decode counts are rejected.

Required verification commands:

```sh
cargo test -p rsinter bb_circuit_bposd_timing_counters_partition_decode_work -- --nocapture
cargo test -p rsinter bb90_hard_syndrome_reports_osd_profile_counters -- --nocapture
cargo test -p rsinter bb_circuit_bposd_timing_counters_reject_incomplete_rows -q
cargo test
```

## Approval

This Agent Desk run is non-interactive. The standing answer policy approves the
recommended minimal stats-plus-aggregation design because it matches the issue's
scope and avoids broad unrelated benchmark work.
