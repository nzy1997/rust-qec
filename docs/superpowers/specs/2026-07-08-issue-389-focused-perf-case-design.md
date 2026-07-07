# Issue 389 Focused Perf Case Runner Design

## Objective

Add `--case <label>` to `rstim perf run` and `rstim perf ci` so reviewers can
rerun only the Stim-style simulator comparison case:

```text
stim-style-surface-sample-d11-r100-b1024
```

The focused run must emit raw JSONL evidence for that selected case even when a
tool variant fails. Default no-`--case` full-suite behavior stays conservative:
it still runs every registered case and may abort on infrastructure or tool
failures.

## Selected Approach

Add optional case selection to the perf runner options and validate the label
before opening or running any benchmark output. The selected-case path reuses
the existing case source loader, compiled-path variant discovery, warmup
rounds, and measured-round loop, but records per-variant outcomes instead of
returning early on the first variant failure.

This is the recommended approach over two alternatives:

- Filtering raw output after a full-suite run would still execute unrelated
  cases and would not solve selected-case tool failure evidence.
- Adding a separate one-off command would duplicate perf-runner behavior and
  make future perf evidence modes harder to keep consistent.

## Raw Record Semantics

`PerfMeasurementRecord` will gain explicit selected-run outcome fields:

- `status`: one of `completed`, `tool_failed`, `timed_out`, or
  `missing_variant`.
- `failure_reason`: optional reviewer-readable text for non-completed records.
- `stderr`: optional captured standard error when a tool process fails.

Completed records keep the existing timing and metadata fields. Failure records
use the same case metadata, variant label, `measurement_index`, and `warmup`
position that a completed measurement would have used, with `wall_time_ns = 0`
and the current peak-memory sample if available. That keeps the raw JSONL shape
parseable by the existing summary layer while still making failures explicit.

The first implementation records `tool_failed` for spawn failures, stdin write
failures, wait failures, and nonzero process exits from `stim-cli`. It records
`missing_variant` when the selected case expects a variant that discovery cannot
run for that circuit. `timed_out` is part of the public status vocabulary for a
future runner timeout mechanism, but this issue does not add timeout controls.

## CLI Contract

`rstim perf run` accepts:

```text
--case <label>
```

When omitted, behavior is unchanged. When provided:

- unknown labels fail before any benchmarks run with text containing
  `unknown benchmark case`;
- output contains only the selected case label;
- warmup and measured rounds preserve the existing ordering and counts;
- each discovered variant emits either completed measurement records or
  failure records.

`rstim perf ci` accepts the same `--case <label>` and writes `raw.jsonl`,
`summary.json`, and `report.md` for only that case. Focused CI skips the
full-suite gate because report-only selected evidence must remain reproducible
even when unrelated gating cases are absent from the raw artifact.

## Summary And Report Behavior

The summary parser remains backward-compatible with existing raw JSONL that
does not include `status`, `failure_reason`, or `stderr`; omitted status is
treated as `completed`.

Variant summaries count only completed measured records for medians and
comparisons. Variants with only failed measured records remain present and carry
status/failure summaries instead of causing the summary parser to fail. The
report renders variant status and failure reasons so selected evidence is
reviewer-readable.

For selected-case summaries, missing data for unrelated benchmark cases is not
reported. Full-suite summaries keep the current missing-case issue behavior.

## Tests

Add TDD coverage in the existing perf tests:

- a CLI negative control for `perf run --case no-such-case` that exits nonzero,
  prints `unknown benchmark case`, and does not create the requested output;
- a focused runner test that simulates a failing selected `stim-cli` variant and
  asserts raw JSONL includes `status: "tool_failed"` and a failure reason;
- a focused summary/report test proving selected-case raw records do not
  produce missing-case issues for unrelated cases;
- a focused CI test proving `raw.jsonl`, `summary.json`, and `report.md` contain
  only the selected case.

Run the issue's two verification commands, the unknown-case negative control,
focused Rust tests while developing, and the repository `cargo test` gate before
opening the PR.

## Scope Limits

Do not optimize the selected benchmark. Do not add a hard Stim-vs-`rstim`
speed gate. Do not change default full-suite behavior unless compatibility with
the new status fields requires the summary parser to understand both old and
new raw record shapes.

## Self-Review

- No placeholder text remains.
- The design validates unknown labels before running benchmarks.
- The default no-`--case` path stays conservative.
- Selected runs preserve warmup and measured-round behavior.
- Failed selected tool variants become explicit raw records with status and
  reviewer-readable failure context.
- The design does not optimize sampler performance or introduce a cross-tool
  speed threshold.
