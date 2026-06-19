# Issue 93 Rbposd LSD Benchmark Integration Design

Date: 2026-06-19
Status: Approved by non-interactive Standing Answer Policy
Scope: GitHub issue #93, `rsinter` result-row normalization and deterministic LSD benchmark coverage

## Summary

Issue #93 needs reviewer-checkable proof that LSD-backed `rbposd` benchmark
runs record the normalized LSD decoder settings they actually use, write normal
benchmark artifacts, reject invalid LSD params before artifacts are created,
and expose a deterministic behavior difference when `lsd_order` changes.

The dependency work from #91 and #92 is already present on `master`: the
`rbposd` runner parses typed LSD params, records `lsd_method` and `lsd_order`
in its normalized flat param map, and routes LSD runs through
`RbposdLsdDemDecoder` with the parsed BP config. The remaining issue #93 work
should therefore add focused tests around that behavior and only change
production code if those tests reveal a gap.

## Goals

- Add the issue-named verification tests:
  - `rbposd_lsd_benchmark_records_normalized_decoder_params`
  - `rbposd_lsd_benchmark_run_writes_results_jsonl`
  - `rbposd_lsd_order_changes_logical_error_rate`
  - `rbposd_lsd_benchmark_rejects_unknown_decoder_param_without_results`
- Prove LSD result rows keep the same flat `params` shape as existing benchmark
  rows.
- Prove the recorded LSD params are normalized effective values, including
  defaults such as `lsd_method = "localized_statistics"` when only
  `lsd_order` is supplied.
- Prove a valid LSD benchmark run writes `results.jsonl` under the expected
  artifact directory.
- Prove unknown LSD-facing params fail during preflight and leave no stale
  result artifact directory.
- Prove a small exact DEM case where `lsd_order = 1` changes the measured
  logical error rate compared with `lsd_order = 0`.

## Non-Goals

- Do not update benchmark spec fixtures or plot rendering.
- Do not expand supported LSD methods or orders.
- Do not change `BenchmarkResultRow` shape.
- Do not add broad borrowed differential fixture coverage.
- Do not alter `rbposd` core decoding behavior.

## Current Context

`rsinter/src/bench/runners/rbposd.rs` builds `RbposdRunnerParams` from the
flat TOML map. For LSD params it currently normalizes:

- `bp_algorithm`
- `bp_iters`
- `early_stop`
- `lsd_method`
- `lsd_order`

The benchmark runner then merges these decoder params with generic case params
inside `run_decoder_point_with_dem_mode`, adds `decoder_impl` and `seed`, and
writes the row through the normal benchmark artifact path.

`rsinter/tests/bench_run.rs` already contains the issue #91 LSD surface helper
and the successful issue #92 LSD artifact smoke test. These are the right
integration-test building blocks for #93.

`rsinter/tests/decode_rbposd.rs` already contains
`rbposd_osd_order_changes_ler`, an exact small-DEM logical-error-rate test. The
new LSD order test should follow that style rather than using a random
benchmark sweep.

## Alternatives Considered

### 1. Focused Tests Around Existing Normalization

Add the required issue-named tests using the current runner and adapter APIs.
Use a small TOML benchmark spec for artifact and param assertions, and use a
small exact DEM for deterministic LSD-order behavior.

Benefits:

- Matches the issue verification commands exactly.
- Keeps the result-row shape flat and unchanged.
- Avoids unrelated benchmark fixture churn.
- Gives reviewers direct evidence for the dependency integration.

Cost:

- The production diff may be test-only if the existing normalized path already
  satisfies the issue.

This is the chosen approach.

### 2. Refactor Rbposd Param Normalization Into A Public Helper

Expose a new helper that returns normalized decoder params and test that helper
directly.

Benefits:

- Makes unit tests smaller.

Costs:

- Expands public or crate-visible surface just for tests.
- Does not prove artifact writing through the normal benchmark workflow.

This is rejected for issue #93.

### 3. Add Benchmark Fixture TOML Files

Create durable fixture specs for LSD benchmark runs.

Benefits:

- Fixtures could be reused by future CLI tests.

Costs:

- The issue lists spec updates as out of scope.
- Adds broader maintenance surface than needed for the requested verification.

This is rejected for issue #93.

## Design

### Normalized Param Test

Add `rbposd_lsd_benchmark_records_normalized_decoder_params` in
`rsinter/tests/bench_run.rs`. Build a small LSD-backed spec with
`lsd_order = 1` and without an explicit `lsd_method`; this proves the row records
the normalized default method rather than raw TOML text. Run the benchmark with
`max_shots = 0` so the test is deterministic and cheap. Assert that the single
row is `ok` and includes generic case params plus normalized decoder params:
`distance`, `rounds`, `p`, `bp_algorithm`, `bp_iters`, `early_stop`,
`lsd_method`, `lsd_order`, `decoder_impl`, and `seed`.

### Results Jsonl Test

Add `rbposd_lsd_benchmark_run_writes_results_jsonl` in
`rsinter/tests/bench_run.rs`. Use a small valid LSD spec, run the normal
benchmark workflow, assert `run_manifest.json` and `results.jsonl` exist under
`rbposd_lsd/test-run`, read the JSONL row, and assert the runner, language,
status, path-facing decoder identity, and normalized LSD params.

### Negative Control

Add `rbposd_lsd_benchmark_rejects_unknown_decoder_param_without_results` in
`rsinter/tests/bench_run.rs`. Feed the issue #91 LSD helper an unknown
LSD-facing param, assert the existing `unknown rbposd runner param: <key>` error,
and assert the runner artifact directory was not created.

### Deterministic LSD Order Behavior

Add `rbposd_lsd_order_changes_logical_error_rate` in
`rsinter/tests/decode_rbposd.rs`. Use the small matrix shape from the existing
`rbposd` LSD fixture where order 0 and order 1 choose different corrections for
the same syndrome. Represent it as a DEM with three independent error
mechanisms:

- `D0`
- `D1`
- `D1 L0`

with probability `0.3775406687981454` for each. Enumerate all eight error
events exactly, compute actual logical flips from the DEM's observable column,
decode each syndrome with `RbposdLsdDemDecoder` for order 0 and order 1, and sum
logical-error probabilities. Assert the rates differ and that order 1 is lower.

## Error Handling

The negative-control test relies on existing benchmark preflight behavior:
unknown `rbposd` params fail before any runner artifact directory is created.
No new error strings are introduced unless tests expose a missing validation
path.

## Testing

Run the issue commands:

```bash
cargo test -p rsinter rbposd_lsd_benchmark_records_normalized_decoder_params
cargo test -p rsinter rbposd_lsd_benchmark_run_writes_results_jsonl
cargo test -p rsinter rbposd_lsd_order_changes_logical_error_rate
cargo test -p rsinter rbposd_lsd_benchmark_rejects_unknown_decoder_param_without_results
```

Also run:

```bash
cargo test -p rsinter
cargo test
git diff --check
```

Use `--offline` only if Cargo tries to access the network in this Agent Desk
workspace.

## Self-Review

- Placeholder scan: no unfinished markers remain.
- Scope check: the plan is limited to issue #93 and avoids fixture or plotting
  changes.
- Consistency check: all new test names match the issue verification commands.
- Ambiguity check: the normalized-param test explicitly uses omitted
  `lsd_method` to distinguish effective normalization from raw TOML echoing.
