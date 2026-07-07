# Issue 390 Sample Speed Evidence Design

## Objective

Extend perf summary JSON and Markdown reports for sample workloads so reviewer
evidence includes median shots per second, report-only `rstim`-vs-Stim context,
and explicit unavailable statuses when a comparison cannot be computed.

The change is evidence/reporting work only. It must not tune `rstim` speed, hide
poor results, or promote cross-machine Stim comparisons into a CI gate.

## Selected Approach

Add rate and comparison availability fields to the existing perf summary model,
then render those same fields in the Markdown report. Completed sample variants
with positive `shots` get:

- `median_shots_per_second`
- `rstim_compiled_vs_stim_cli_ratio` for report-only Stim context when both
  `rstim-compiled` and `stim-cli` completed

Unavailable Stim comparisons are represented as structured summary fields with
a status of `missing_variant`, `tool_failed`, or `timed_out` and the recorded
reason when one exists. The renderer prints the unavailable status instead of
fabricating a ratio.

This is the recommended approach over two alternatives:

- Encoding Stim availability only as generic summary issues would make JSON and
  Markdown evidence harder to line up and would blur report-only context with
  gating failures.
- Reusing the existing compiled-vs-interpreted comparison list for Stim ratios
  would make report-only cross-tool context look like the same kind of gate
  candidate as same-run `rstim` backend comparisons.

## Summary Model

`PerfVariantSummary` gains:

- `median_shots_per_second: Option<f64>`

Only sample workloads with completed measured records and positive `shots`
produce a rate. If a completed sample record has `shots = 0`, summarization
fails with:

```text
shots must be positive for sample rate
```

`PerfCaseSummary` gains:

- `rstim_compiled_vs_stim_cli_ratio: Option<PerfUnavailableComparisonSummary>`

The comparison summary carries:

- `kind = "rstim_compiled_vs_stim_cli"`
- `lhs_variant = "rstim-compiled"`
- `rhs_variant = "stim-cli"`
- `ratio: Option<f64>`
- `status: String`
- `failure_reason: Option<String>`

When both variants completed, `ratio` is
`rstim-compiled.median_wall_time_ns / stim-cli.median_wall_time_ns` and
`status = "completed"`. A ratio above 1 honestly means `rstim` was slower than
Stim. If either variant is absent or did not complete, `ratio = null` and
`status` names the unavailable condition: `missing_variant`, `tool_failed`, or
`timed_out`.

The existing `comparisons` array remains reserved for same-project `rstim`
compiled-vs-interpreted or analyzer comparisons. Gate evaluation keeps using
that path and ignores the Stim context field.

## Markdown Report

The report prints completed variant rates as `shots/s` next to median wall time
for sample workloads. It prints a separate line for the Stim context:

```text
- report-only Stim comparison: rstim-compiled / stim-cli = ...
```

If unavailable, that line includes the status and recorded reason rather than a
numeric ratio. The exact phrase `report-only Stim comparison` is intentional so
external readers can distinguish context from CI gating.

## Fixture Evidence

Add the issue verification fixture at:

```text
rstim/tests/fixtures/perf/stim_style_sample_raw.jsonl
```

The fixture uses the public report-only case label
`stim-style-surface-sample-d11-r100-b1024`, positive `shots`, completed
`stim-cli`, `rstim-interpreted`, and `rstim-compiled` rows, plus one
non-completed variant path in focused tests. It is small and deterministic; it
does not attempt to reproduce real machine timing.

## Tests

Add focused TDD coverage in existing perf tests:

- summary JSON includes `median_shots_per_second` for completed sample variants;
- summary JSON includes completed
  `rstim_compiled_vs_stim_cli_ratio` when both variants completed;
- failed or missing Stim comparisons carry explicit unavailable status and
  reason without synthetic ratios;
- zero-shot sample records fail summarization with
  `shots must be positive for sample rate`;
- Markdown contains `shots/s` and `report-only Stim comparison`;
- the CLI fixture path used by the issue verification commands works.

Run the issue's exact summarize/report commands, the zero-shot negative
control, focused Rust tests, and the repository `cargo test` gate.

## Scope Limits

Do not tune benchmark speed. Do not suppress slow ratios. Do not add a
cross-machine Stim threshold to `perf gate`. Do not remove or downgrade the
existing `rstim` compiled-vs-interpreted comparisons.

## Self-Review

- No placeholder text remains.
- The design keeps `rstim` compiled-vs-interpreted comparisons as the gating
  candidate.
- Stim comparisons are explicitly labeled report-only.
- Failed and missing variants are represented as status evidence, not fake
  ratios.
- The zero-shot negative control has a concrete error string.
