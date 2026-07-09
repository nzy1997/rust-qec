# Issue 416 Post-Optimization Speed Evidence Design

Issue: #416

## Context

Issue #406 and the checked artifact at
`benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json` document a
debug-profile selected-case gap where Stim CLI is about 261x faster than
`rstim-compiled`. That artifact must remain historically true. Issues #407 and
#408 added the release/debug selected-case runner and the semantic guard for the
old artifact. Issues #409 through #415 have since landed the sampler
optimization work that this evidence should follow.

This issue publishes later post-optimization evidence without overwriting the
old #406 output. The new evidence is checked into a stable release directory
and is referenced separately by docs and site metadata.

## Approaches Considered

1. Checked release directory under
   `benchmarks/rstim_vs_stim_simulator/results/release/` (selected). This meets
   the publish requirement, keeps local runner output separate from checked
   evidence, and gives docs/site metadata stable paths and hashes.

2. Local-only output under `benchmarks/out/`. This is useful for staging but
   fails the issue's publish objective because ignored local artifacts are not
   discoverable checked evidence.

3. Replace the existing `results/full/` artifacts with release output. This is
   rejected because it would hide or overwrite the historical #406 result that
   #408 explicitly protects.

## Design

The checked post-optimization evidence lives in
`benchmarks/rstim_vs_stim_simulator/results/release/` with exactly these
published files:

- `summary.json`, promoted from the #407 release runner output;
- `report.md`, promoted from the #407 release runner output;
- `environment.json`, promoted from the #407 release runner output and carrying
  the release profile, selected case label, `rstim` binary path, Rust/Cargo
  versions, and Stim CLI probe result.

The evidence is one selected workload only:
`stim-style-surface-sample-d11-r100-b1024` with `--profile release`,
`--warmup-rounds 0`, and `--measure-rounds 1`. The docs and site wording must
describe it as a recorded post-optimization release-profile run, not broad
`rstim`/Stim parity.

## Checker

Add `tools/check_rstim_vs_stim_post_optimization_evidence.py` with this
interface:

```sh
python3 tools/check_rstim_vs_stim_post_optimization_evidence.py \
  --old benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json \
  --new-dir benchmarks/rstim_vs_stim_simulator/results/release
```

The checker validates five boundaries:

- the old summary still passes the #408 gap-artifact guard;
- the new directory contains `summary.json`, `report.md`, and
  `environment.json`;
- the new summary is not the same file/content as the old summary and contains
  only the selected case with release-run timing metadata;
- `environment.json` marks the run as release/post-optimization evidence and
  records `rstim_binary_path`, `rustc_version`, `cargo_version`, and Stim CLI
  probe metadata;
- `docs/showcases/rstim-vs-stim-simulator.md` and `site/benchmark-site.json`
  link old and new artifacts separately while preserving narrow claim limits.

The checker exits nonzero when the old summary is reused as the new release
summary, even if an `environment.json` and `report.md` are present.

## Site And Docs

`docs/showcases/rstim-vs-stim-simulator.md` gains a short checked-artifacts
section that names both the historical #406 artifact and the release
post-optimization evidence directory. The Limits section keeps the no-broad
parity claim.

`site/benchmark-site.json` gains a separate evidence item for
`rstim-vs-stim-release` with the three release artifacts and recorded SHA-256
hashes. Existing `rstim-vs-stim-full` remains unchanged except for any hash
updates forced by documentation references. The site checked-results list adds
the new release item so the checked artifact cards remain discoverable.

## Tests

Add focused Python unit tests for the new checker:

- default checked artifacts pass;
- a release directory that reuses the old #406 summary fails;
- missing release files fail;
- missing release environment metadata fails;
- docs or site metadata that omit the separate new item fail;
- docs with broad all-workload parity wording fail.

Run the issue's checker command, the negative control command, the existing
manifest/showcase checkers, the focused Python unit tests, `git diff --check`,
and `cargo test`.

## Scope Limits

Do not overwrite files under `benchmarks/rstim_vs_stim_simulator/results/full/`.
Do not add CI wall-clock gates based on Stim ratios. Do not claim performance
parity beyond the recorded selected release-profile run.
