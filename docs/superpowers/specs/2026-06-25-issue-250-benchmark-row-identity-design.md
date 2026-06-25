# Issue 250 Benchmark Row Identity Design

Date: 2026-06-25
Status: Approved by non-interactive standing policy
Scope: `rsinter` benchmark result rows and merge behavior

## Summary

Benchmark result rows currently merge by concatenating and sorting. That is
usable for inspection, but it cannot safely combine repeated samples for the
same benchmark task or detect conflicting duplicate rows.

The change will add a deterministic machine identity for each benchmark result
row and make `rsinter bench merge` group rows by that identity. Compatible
rows with the same identity will have additive counters summed and derived
metrics recomputed. Rows with the same identity but conflicting non-additive
metadata will return a clear merge error.

## Goals

- Provide a stable `BenchmarkResultRow` identity helper based on canonical JSON
  serialization and a SHA-256 digest.
- Include the identity in serialized JSONL output for debugging and future
  resume workflows, while still accepting legacy JSONL rows without it.
- Make JSON object key order irrelevant to identity computation.
- Merge compatible rows with the same identity by summing additive counters:
  `shots_used`, `logical_errors`, `compile_us`, `total_decode_us`, and
  `wall_seconds`.
- Recompute `logical_error_rate` and `decode_us_per_shot` from the merged
  counters.
- Treat non-additive fields as compatibility checks: benchmark, runner,
  language, params, stable case summary fields, status, failure kind, error
  state, artifacts, and unknown non-additive metrics must not conflict.
- Preserve existing output ordering after merge: runner, distance, and `p`.

## Non-Goals

- Do not add resume behavior.
- Do not change benchmark runner scheduling or seed selection.
- Do not change benchmark plot behavior beyond accepting merged rows.
- Do not rewrite committed benchmark artifacts.

## Current State

`BenchmarkResultRow` stores benchmark, runner, language, status, failure kind,
params, case summary, metrics, artifacts, and error. Rows serialize directly
with derived `serde` behavior.

`merge_result_rows` currently flattens all input row sets and sorts them. It
does not identify duplicate benchmark tasks, sum counters, or detect conflicts.

## Design

### Row Identity

Add `BenchmarkResultRow::identity(&self) -> Result<String, String>`. The helper
will serialize a small identity input object with stable JSON map ordering and
hash those bytes with SHA-256. The identity string will be formatted as
`sha256:<hex>`.

The identity input will include:

- `schema`: `rsinter.benchmark_result_row.v1`
- `benchmark`
- `runner`
- `language`
- `params`
- stable case summary fields

The identity will exclude status, failure kind, metrics, artifacts, and error
because those are result state rather than task identity. Same-identity rows
with conflicting result state will be rejected by merge compatibility checks.

Case summary fields are split into stable identity fields and progress fields.
`num_shots_generated` is progress and will be merged additively when present;
other case summary keys are stable and must match.

### JSONL Serialization

Implement custom `Serialize` for `BenchmarkResultRow` that emits an `identity`
field before the existing row fields. Existing deserialization will ignore the
serialized identity, infer missing `failure_kind` as it does today, and compute
fresh identities from row content.

This keeps legacy JSONL input compatible while making new output easier to
inspect and use in future resume workflows.

### Merge Semantics

Change `merge_result_rows` to return `Result<Vec<BenchmarkResultRow>, String>`.
The CLI will propagate this error with its existing `Result<(), String>` flow.

Merge will group rows by `row.identity()?`. For each duplicate identity:

- Verify benchmark, runner, language, status, failure kind, params, error, and
  artifacts are equal.
- Verify all stable case summary fields are equal.
- Sum additive metric counters present in either row.
- Sum `case_summary.num_shots_generated` when present in either row.
- Recompute `logical_error_rate` when `shots_used` and `logical_errors` are
  present.
- Recompute `decode_us_per_shot` when `shots_used` and `total_decode_us` are
  present.
- Reject conflicting unknown non-additive metrics with a clear error message.

Rows with different identities will remain distinct. After grouping, merged
rows will be sorted with the existing runner, distance, and `p` ordering.

## Alternatives Considered

### 1. Store identity as a real struct field

Rejected for this issue because it would require every row constructor and
test helper to populate a value that is derived from the other fields. A helper
plus custom serialization keeps the identity authoritative and avoids stale
stored identities.

### 2. Use raw JSON row serialization as the identity input

Rejected because the raw row includes result metrics, status, error, and
artifacts. Those fields change as sampling progresses and would prevent resume
or merge workflows from recognizing the same task.

### 3. Keep merge as concatenate-and-sort

Rejected because the issue requires duplicate task rows to combine safely and
requires conflicts to fail clearly.

## Verification

Run the focused requested regression test:

```bash
cargo test -p rsinter --test bench_merge benchmark_merge_combines_rows_with_same_identity
```

Then run broader verification:

```bash
cargo test
```
