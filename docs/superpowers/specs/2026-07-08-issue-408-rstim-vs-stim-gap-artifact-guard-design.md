# Issue 408 rstim-vs-Stim Gap Artifact Guard Design

## Objective

Add a small repository checker that preserves the checked issue #406
performance-gap artifact for
`stim-style-surface-sample-d11-r100-b1024`. The checker must reject summaries
that merely keep a large ratio while changing the recorded selected-case
identity, measured sample counts, statuses, or `stim-cli` and
`rstim-compiled` rates.

## Context

Issue #406 records a narrow, intentionally bad checked result:
`stim-cli` is about `261.34x` faster than `rstim-compiled` on the checked
debug-profile sample workload. The committed summary currently reports:

- `stim-cli`: `5690.64878525516` shots/s, `sample_count = 1`,
  `status = completed`;
- `rstim-compiled`: `21.774891038227285` shots/s, `sample_count = 1`,
  `status = completed`;
- case metadata `workload = sample`, `tier = report_only`, and
  `present_variants = ["rstim-compiled", "rstim-interpreted", "stim-cli"]`;
- file SHA-256
  `97ae397e598fe447d206c6b07a26ceaa0a3336d1883a7f77bc194f7b4c491805`,
  matching the recorded hash in `site/benchmark-site.json`.

The guard protects only this checked artifact. It must not optimize sampler
code and must not require future optimized evidence to preserve this ratio.

## Selected Approach

Create `tools/check_rstim_vs_stim_gap_artifact.py` with a data-driven
fingerprint and focused Python tests. The checker reads
`benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json` by default
or an explicit summary path when supplied. It validates the selected case,
required variants, completed statuses, measured sample counts, exact selected
metadata, expected rates within a tight floating tolerance, and the ratio range
`200 <= stim-cli / rstim-compiled <= 300`.

When the default artifact path is checked, the script also loads
`site/benchmark-site.json` and compares the artifact SHA-256 to the manifest's
recorded provenance hash when the manifest entry is present. Explicit fixture
paths skip this manifest hash check so tests can write synthetic negative
controls under `/tmp`.

This is preferred over a ratio-only check because ratio-only synthetic data can
hide changed rates or regenerated summaries. It is preferred over expanding the
site manifest validator because this issue asks for a direct artifact-preserve
command with its own interface and negative controls.

## Interface

Command:

```sh
python3 tools/check_rstim_vs_stim_gap_artifact.py [speed-summary.json]
```

If no path is supplied, the checker reads:

```text
benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json
```

Success prints:

```text
PASS checked #406 gap is preserved: stim-cli is 261.34x faster than rstim-compiled
```

Failure exits nonzero and reports the rejection reason to stderr, including
cases such as missing selected case, missing variant, non-completed variant,
changed sample count, selected-case rate changed, ratio outside 200-300, or
checked artifact hash differs from site manifest.

## Fingerprint

The semantic fingerprint includes:

- selected `case_label`;
- `workload`;
- `tier`;
- `present_variants`;
- completed statuses for `stim-cli` and `rstim-compiled`;
- measured `sample_count` values for both variants;
- recorded `median_shots_per_second` values for both variants, checked with a
  tight absolute/relative tolerance.

`rstim-interpreted` remains part of the expected `present_variants` list because
it is present in the checked #406 artifact, but only `stim-cli` and
`rstim-compiled` are required as completed rate-bearing variants for the gap.

## Tests

Use test-first Python unit tests under `tools/`:

- default checked artifact passes and prints the expected `261.34x` message;
- equal-speed fixture is rejected with `ratio outside 200-300`;
- changed-large-gap fixture is rejected with `selected-case rate changed`;
- missing `rstim-compiled` fixture is rejected with `missing rstim-compiled`;
- a copied default-path fixture with content different from the manifest hash is
  rejected with `checked artifact hash differs from site manifest`.

Issue verification also runs the exact command from issue #408, the two shell
negative controls from the issue body, and `cargo test`.

## Self-Review

- No placeholder text remains.
- The guard is scoped only to the checked #406 artifact.
- The default path and PASS text match the requested interface.
- Fixture paths can skip manifest hash validation.
- The design does not optimize sampler code or rewrite checked artifacts.
