# Issue 392 Publish rstim-vs-Stim Checked Evidence Design

## Objective

Promote the `rstim-vs-stim-simulator` benchmark-site family from future work to
partial checked evidence. The site must expose the checked speed summary, speed
report, correctness summary, fixture manifest, canonical `.stim` fixture, and
showcase page with provenance and a narrow claim boundary.

## Context

Issue #391 is complete and merged, so
`docs/showcases/rstim-vs-stim-simulator.md` already documents the workload,
commands, interpretation, and limits. The current site manifest still lists the
family as `future`, and the build checker still hard-codes that future status.

The current checkout does not contain
`benchmarks/rstim_vs_stim_simulator/results/full/` artifacts yet. The checked
artifacts for this issue will be generated from the documented commands and
committed under that directory:

- `speed-summary.json` from focused `rstim perf ci --case`;
- `speed-report.md` from the same focused run;
- `correctness-summary.json` from the smoke correctness verifier.

The canonical fixture manifest and `.stim` input are already checked under
`benchmarks/rstim_vs_stim_simulator/`.

## Selected Approach

Use a manifest-first promotion with policy checks. Add one checked evidence item
for `rstim-vs-stim-simulator` with family and item status `partial`, list the
five checked artifacts plus the showcase page, and add canonical provenance to
the item. Update the site app's checked-result card list and static site copy so
reviewers see the family as partial checked evidence instead of future-only
planning.

This is preferred over keeping a future placeholder because the issue explicitly
requires checked/partial evidence. It is also preferred over rerunning or tuning
a broader benchmark campaign because the issue is publication work, not
optimization work.

## Artifact Set

Create or keep these checked files:

- `benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json`
- `benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md`
- `benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json`
- `benchmarks/rstim_vs_stim_simulator/cases.full.toml`
- `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim`
- `docs/showcases/rstim-vs-stim-simulator.md`

The manifest artifact kinds will distinguish `speed-summary`,
`speed-report`, `correctness-summary`, `fixture-manifest`, `stim-fixture`, and
`showcase`. All entries are `checked: true` and must stay tracked by git.

## Provenance

The item will use the existing canonical provenance schema with
`schema_version = 1`. The provenance records the checked run date, source
commit, commands, OS, CPU model, Rust version, Stim version, build profile,
seeds, shot counts, and SHA-256 hashes for every checked artifact listed on the
item.

Some values fit existing canonical fields:

- Stim version is recorded under `dependency_versions`.
- Seeds are recorded under `seed_policy`.
- Shot counts are recorded under `shots_or_error_budget`.

The `provenance_requirements` list remains reviewer-facing text and explicitly
includes OS, CPU model, Rust version, Stim version, command line, seeds, build
profile, shot counts, and date.

## Site And Checker Policy

`site/benchmark-site.json` will become the source of truth. The static site app
will include `rstim-vs-stim-full` in checked result cards, while the manifest
inventory already renders all families.

`tools/check_site_manifest.py` and `tools/check_site_build.py` will accept
`rstim-vs-stim-simulator` status `partial` only when it lists checked artifacts.
The build checker must fail if checked artifacts are missing from `_site`, point
under `benchmarks/out/`, are ignored, are untracked, or have copied hashes that
do not match the manifest. It must no longer require the family to remain
`future`.

The checked-artifact reference regular expressions will include
`benchmarks/rstim_vs_stim_simulator/results/full/` so hard-coded site links are
validated against the manifest.

## Claim Boundary

The family claim remains limited to the recorded workload and recorded
environment. The site must not claim broad `rstim`/Stim parity, broad
performance parity, or generator equivalence. Slow, bad, failed, or incomplete
results remain publishable evidence only when the status and provenance are
visible.

## Tests

Use test-first changes around the policy transition:

- Python manifest tests accept a fixture where `rstim-vs-stim-simulator` is
  `partial` with checked artifacts and reject `partial` without checked
  artifacts.
- Python build tests accept the partial fixture and reject a missing
  `correctness-summary.json` copied artifact.
- Rust site contract tests require the manifest and static copy to show partial
  checked evidence rather than future-only planning.
- Issue verification runs `make build-site` and
  `python3 tools/check_site_build.py _site`.
- Negative control removes
  `_site/benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json`
  and verifies `python3 tools/check_site_build.py _site` exits nonzero with a
  missing checked artifact.

## Self-Review

- No placeholder text remains.
- The artifact list matches issue #392 and avoids `benchmarks/out/`.
- The status is `partial`, not `future`.
- The claim boundary is limited to checked workload and checked environment.
- The design does not optimize or broaden benchmark results.
