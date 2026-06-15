# rsinter Failure Taxonomy Design

## Context

Issue 49 asks `rsinter` benchmark results to expose a structured failure
taxonomy. Today `BenchmarkResultRow` has only `status: String` and
`error: Option<String>`, so downstream consumers have to parse strings to
distinguish ordinary logical failures from crashes, solver backend failures, or
unsupported decoder configurations.

AutoQEC needs this distinction for its keep/discard/crash loop. A candidate with
logical errors is a normal discard. A decoder crash, unsupported backend, sampler
failure, or timeout needs separate handling.

The lower-level `collect` path also only records logical error counts in
`TaskStats`. This design extends both benchmark JSONL rows and `TaskStats` so
they use the same taxonomy.

## Goals

- Add a typed `FailureKind` model shared by benchmark rows and task stats.
- Serialize benchmark `failure_kind` values as stable snake_case strings:
  `ok`, `logical_failure`, `timeout`, `solver_failure`, `unsupported`, and
  `sampler_error`.
- Preserve existing human-readable `status` and `error` fields for compatibility
  and diagnostics.
- Record per-point benchmark failures as result rows instead of panicking or
  producing an empty artifact when a run-time decoder/backend/sampler failure
  occurs.
- Add `failure_kind` to `TaskStats` and CSV resume data while keeping old CSV
  files readable.
- Keep configuration/spec errors as regular `Err` results that fail the
  benchmark before artifact writes.

## Non-Goals

- Do not redesign benchmark specs, plotting, or the benchmark artifact layout.
- Do not add cancellation inside a decoder call; wall-clock timeout remains a
  stop condition checked between batches.
- Do not change downstream AutoQEC code in this repository.
- Do not make every decoder backend expose a rich typed error hierarchy in this
  issue. Use typed `FailureKind` at the `rsinter` boundary.

## Data Model

Add a public enum in a new shared module, `rsinter/src/failure.rs`, and re-export
it from `rsinter/src/lib.rs` so both benchmark rows and task stats can use one
type:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    Ok,
    LogicalFailure,
    Timeout,
    SolverFailure,
    Unsupported,
    SamplerError,
}
```

`BenchmarkResultRow` gains:

```rust
pub failure_kind: FailureKind,
```

`TaskStats` gains the same field.

`status` stays for legacy consumers. The intended mapping is:

- `status = "ok"` when `failure_kind` is `ok`, `logical_failure`, or `timeout`
  and the run completed without a run-time error.
- `status = "error"` when `failure_kind` is `solver_failure`, `unsupported`, or
  `sampler_error`.

`error` stays as optional detail. It is `None` for ordinary `ok`,
`logical_failure`, and `timeout` outcomes unless the implementation has useful
non-fatal detail to report.

## Compatibility

`read_results_jsonl` should accept older rows with no `failure_kind`. When the
field is absent, infer:

- `status == "error"`: classify from `error` if possible, otherwise
  `solver_failure`.
- `metrics.logical_errors > 0`: `logical_failure`.
- Otherwise: `ok`.

CSV output adds a `failure_kind` column. `read_csv` should detect whether the
column is present by header name. If an old CSV lacks the column, infer:

- `errors > 0`: `logical_failure`.
- Otherwise: `ok`.

`TaskStats::add` should combine compatible stats conservatively. Use this
priority order, from strongest to weakest:

```text
unsupported > solver_failure > sampler_error > timeout > logical_failure > ok
```

This keeps resumed aggregate stats from hiding the most important failure or
stop condition.

## Run-Time Data Flow

Change the `rsinter` decoder traits from panic-oriented adapters to explicit
results:

```rust
pub trait Decoder: Send + Sync {
    fn compile_for_dem(&self, dem: &DetectorErrorModel)
        -> Result<Box<dyn CompiledDecoder>, String>;
}

pub trait CompiledDecoder: Send {
    fn decode_shots_bit_packed(
        &self,
        dets: &[u8],
        num_shots: usize,
        num_dets: usize,
        num_obs: usize,
    ) -> Result<Vec<u8>, String>;
}
```

`rilpqec` should stop using `expect` in its adapter for compile/decode failures.
It should return the underlying error string. This lets `BackendUnavailable`
become `unsupported`, and other ILP backend failures become `solver_failure`.

`rmatching` and `rbposd` should wrap compile/decode failures in `Err(String)`
where the underlying API can fail. Internal invariants that truly indicate a
programmer bug can remain asserts or expects, but normal backend and decode
failures should cross the boundary as `Result`.

## Benchmark Semantics

`run_decoder_point` should construct `BenchmarkResultRow` for both normal and
run-time failure outcomes.

Normal outcomes:

- No logical errors and no timeout: `failure_kind = ok`.
- Logical errors and no timeout: `failure_kind = logical_failure`.
- Wall-clock budget reached: `failure_kind = timeout`, even if logical errors
  were also observed. The logical error count remains in metrics.

Run-time failures:

- Circuit construction or DEM analysis failure: classify as `unsupported` when
  the input/code family is unsupported; otherwise classify as `solver_failure`.
- Decoder compile failure: `unsupported` for unavailable/unsupported backend
  messages, otherwise `solver_failure`.
- Decoder decode failure: `solver_failure`.
- `sample_batch`, `write_shots_b8`, or observable buffer mismatch:
  `sampler_error`.

When a run-time failure happens after partial progress, the row should include
the partial `shots_used`, `logical_errors`, timing metrics, params, and case
summary available so far. The `error` field should contain the detail string.

`run_rust_benchmark` should keep preflight behavior for configuration/spec
errors. Unknown runner params, invalid decoder params like `mip_gap = 1.0`, and
invalid benchmark point definitions should still fail the whole planned run
before writing artifacts. Those are authoring errors, not evaluated candidate
failures.

## Collect Semantics

`collect` should continue to reject global configuration errors with `Err`, such
as invalid `max_wall_seconds`.

Per-task run-time failures should become `TaskStats` rows:

- Missing decoder in the supplied decoder map is still a caller error and should
  remain `Err`.
- Decoder compile failure: return stats for that task with zero or resumed
  progress and `failure_kind = unsupported` or `solver_failure`.
- Decode failure during a batch: return stats for that task with progress up to
  the completed previous batch and `failure_kind = solver_failure`.
- Sampler or packing failure: return stats for that task with
  `failure_kind = sampler_error`.
- Normal completion uses the same `ok`, `logical_failure`, and `timeout`
  semantics as benchmark rows.

The current parallel collection structure can still return a `Vec<TaskStats>`.
Only global setup and caller errors need to abort the entire collection.

## Classification Helper

Use a small helper instead of spreading string checks throughout the code. It
can start simple:

- `classify_error(message: &str, phase: FailurePhase) -> FailureKind`
- `classify_completed(logical_errors: u64, timed_out: bool) -> FailureKind`
- `combine_failure_kind(a: FailureKind, b: FailureKind) -> FailureKind`

The first implementation classifies `BackendUnavailable`, `backend is
unavailable`, and `no ILP backend is available` as `unsupported`. Solver-specific
errors such as HiGHS, Gurobi, compile, decode, or width mismatch map to
`solver_failure` unless they clearly originate from sampling/packing.

## Testing

Add focused tests in `rsinter`.

`rsinter/tests/bench_result.rs`:

- Round-trip JSON serialization includes `failure_kind`.
- Old JSONL rows without `failure_kind` still deserialize and infer `ok` or
  `logical_failure`.

`rsinter/tests/csv_io.rs`:

- CSV round trip preserves `TaskStats.failure_kind`.
- Old CSV without a `failure_kind` column remains readable.

`rsinter/src/bench/runners/mod.rs` or `rsinter/tests/bench_runner_wrappers.rs`:

- `failure_kind_is_structured` covers clean `ok`, normal
  `logical_failure`, and wall-clock `timeout`.
- A fake decoder compile/decode failure yields a row with `status = "error"`,
  a structured failure kind, and a non-empty `error`.

`rsinter/tests/bench_run.rs`:

- A `rilpqec` benchmark point with `backend = "gurobi"` and no `gurobi`
  feature writes a result row with `failure_kind = "unsupported"` rather than
  panicking or leaving no artifact. Gate this test with
  `#[cfg(not(feature = "gurobi"))]`.

`rsinter/tests/collect.rs`:

- A normal clean collect returns `ok`.
- A logical-error collect returns `logical_failure`.
- A wall-clock-limited collect returns `timeout`.
- A fake decoder failure returns `TaskStats.failure_kind = solver_failure`
  without aborting the whole collection.

Run narrow checks first:

```text
cargo test -p rsinter failure_kind_is_structured
cargo test -p rsinter bench_result
cargo test -p rsinter csv_io
cargo test -p rsinter collect
```

Then run:

```text
cargo test -p rsinter
```
