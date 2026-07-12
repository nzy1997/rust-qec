# Paired Frame-Noise Evidence Design

## Context

Issue #493 refreshes the existing `frame-instruction-wide-release` evidence slot. The slot already contains deterministic operation counters, fixture-load evidence, correctness evidence, portable provenance, and artifact hashes. Issue #492, already merged into `master`, added the paired baseline/candidate runner pinned to baseline revision `f10d1ed024d3519318ed244c9095724074519595`.

## Chosen Approach

Use the paired runner as the source of timing evidence, but publish it beside the existing frame-noise artifacts instead of replacing the current `raw.jsonl`, `summary.json`, or `report.md`. The new artifacts are:

- `paired-raw.jsonl`
- `paired-summary.json`
- `paired-report.md`

The checker validates those paired files in the same semantic pass as the existing telemetry and correctness files, before artifact hash validation. It derives `candidate_over_baseline = candidate_median / baseline_median`, validates the stored ratio and outcome, and rejects ratios above `1.05`.

## Alternatives Considered

1. Extend the runner summary and checker, then publish paired artifacts beside the existing files. This preserves the old evidence surface while making the same-run timing gate explicit. This is the selected approach.
2. Only add checker-side derivation without changing the runner summary. This would leave `paired-summary.json` less self-describing than the issue requests.
3. Replace the original `raw.jsonl`, `summary.json`, and `report.md` with paired timing output. This would discard the deterministic instruction-wide telemetry that the issue says to preserve.

## Data Contract

The paired summary records:

- `baseline_revision = "f10d1ed024d3519318ed244c9095724074519595"`
- a distinct 40-character candidate revision
- two variants, `baseline-rstim-frame-noise-b8` and `candidate-rstim-frame-noise-b8`
- seven measured records per variant
- median elapsed time per variant
- `candidate_over_baseline`
- `outcome`, classified as `improved` for ratio `<= 0.95`, `neutral` for `0.95 < ratio <= 1.05`, and `regressed` above `1.05`

The paired raw file must contain the runner's warmup and measured records from one same-run invocation. Each record keeps the canonical `--skip_reference_sample` frame-noise command, 1024 shots, b8 output, 1,552,384 stdout bytes, and the shared timer scope `process_spawn_stdout_stderr_drain_exit`.

## Validation

The instruction-wide checker keeps the existing requirements:

- 803 candidate iterator builds
- 80,362 legacy setups
- 82,290,688 attempts
- correctness status `pass`
- complete 1,552,384-byte sampling output
- runtime identity matching the catalog

It adds paired evidence validation:

- required paired artifact files exist
- paired raw semantics match the runner contract
- paired summary and report match derivations from `paired-raw.jsonl`
- baseline revision is pinned to `f10d1ed024d3519318ed244c9095724074519595`
- stored outcome matches the derived ratio
- derived ratio is at most `1.05`

Artifact hash validation remains last so semantic failures are reported before stale hash failures.

## Tests

Focused unit tests cover valid paired evidence plus the issue's negative controls:

- `improved` stored for a `1.02` ratio is rejected as a classification mismatch
- a `1.10` ratio is rejected with `candidate frame-noise path exceeds 1.05 non-regression limit`
- failed correctness status is rejected before artifact hash validation

The aggregate portable checker expected output is updated to include the paired timing outcome and ratio.
