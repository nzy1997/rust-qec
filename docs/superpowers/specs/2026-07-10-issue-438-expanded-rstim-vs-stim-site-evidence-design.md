# Issue 438 Expanded rstim-vs-Stim Site Evidence Design

Issue: #438

## Context

Issue #437 is merged on `master` and validates the complete checked evidence
pack without applying timing thresholds. The public benchmark site still
exposes only the original full-fixture evidence and the first release sample
run. Its checked-artifact regular expressions and required-artifact map do not
recognize the expanded distribution, repetition sample, surface detect, or DEM
sample directories.

The existing site card also mixes historical debug-profile speed evidence with
correctness evidence. Issue #438 must make the evidence areas separately
reviewable while preserving the issue #406 artifact as a historical gap record.

## Approaches Considered

1. Split the family into correctness, historical debug-gap, and case-scoped
   release speed cards. Back the cards with an explicit checked-path policy,
   exact required artifacts, copied-file hashes, and a narrow-claim scanner.
   This is selected because it makes the evidence boundaries visible to readers
   and keeps the site copy boundary intentional.

2. Append the new paths to the existing combined card and extend only the path
   regular expressions. This is smaller, but correctness and speed remain
   conflated and an accidental broad claim has no local guardrail.

3. Accept every path below
   `benchmarks/rstim_vs_stim_simulator/results/`. This lowers maintenance, but
   it permits unreviewed future output directories to become checked site
   artifacts and weakens the current allow-list model.

## Documentation and Site Cards

Update `docs/showcases/rstim-vs-stim-simulator.md` with an expanded-evidence
section that links the checked correctness rollup and each release-profile
speed directory. The prose will report only recorded cases and environments:

- distribution correctness covers the eight catalogued small-circuit cases
  plus the existing d11/r100 full-fixture summary;
- release sample evidence identifies the selected surface sample and repetition
  sample workloads separately;
- release detect evidence identifies `surface-detect-d13-r13`;
- release DEM evidence identifies
  `stim-style-surface-dem-sample-d11-r100-b1024`;
- the old full-directory speed summary and report remain explicitly labeled as
  issue #406 debug-profile gap evidence.

Restructure the `rstim-vs-stim-simulator` manifest family into separate checked
items for expanded correctness, the historical debug gap, and each release
speed workload. Each item lists only its own artifacts, commands, provenance,
claim limit, and caveats. The release cards may state recorded wall time or
throughput relationships only when the case label, profile, and environment
scope are present. The family remains `partial` because the artifacts cover a
finite workload set rather than every Stim behavior.

Every checked artifact remains a direct site link and has a recorded SHA-256
entry. The DEM card includes its checked `raw.jsonl`; the other expanded
directories include every committed summary, report, and environment or
rollup file used by issue #437.

## Checked-Artifact Policy

Extend the checked-artifact reference expression in both site validators to
recognize these directories deliberately:

- `results/distributions/`;
- `results/release/`;
- `results/release-repetition-sample/`;
- `results/release-surface-detect/`;
- `results/release-dem-sample/`.

Keep an exact path-to-kind map for required `rstim`-vs-Stim site artifacts.
Also validate every checked artifact in this family against that map. A
manifest entry outside the approved set fails before
`copy_site_benchmark_data.py` copies it, with an error that includes the full
rejected path. This gives the copy helper a single policy source instead of a
second drifting allow-list.

The required map includes the legacy full correctness, debug speed, fixture
manifest, canonical Stim fixture, and showcase paths as well as:

- all three files in `results/distributions/`;
- `summary.json`, `report.md`, and `environment.json` for each circuit release
  speed directory;
- `raw.jsonl`, `summary.json`, `report.md`, and `environment.json` for the DEM
  speed directory.

## Claim Guardrail

Add one case-insensitive scanner shared by manifest validation and showcase
validation for these unqualified forms, allowing Markdown backticks around
tool names:

- `rstim is faster than Stim`;
- `rstim beats Stim`;
- `full Stim parity`.

The scanner checks every string in `site/benchmark-site.json` and the showcase
body. A match produces the stable diagnostic:

```text
broad rstim-vs-Stim claim is not allowed
```

The checker intentionally rejects those phrases even when embedded in a longer
sentence. Approved wording must instead name the recorded case and profile or
describe a bounded evidence area.

## Tests and Verification

Extend the manifest and build fixtures with the expanded files, separate card
IDs, hashes, and copied-site paths. Add negative controls that:

- inject each forbidden broad form into the manifest or showcase and require
  the stable claim diagnostic;
- simulate a stale checked-path policy that omits the DEM release directory and
  require the rejected or missing path in the error;
- remove a new copied artifact and require the build checker to name it.

Run the focused Python unit tests first, then the issue commands:

```sh
make build-site
python3 tools/check_site_manifest.py --repo-root . --site-root _site site/benchmark-site.json
python3 tools/check_site_build.py --repo-root . _site
python3 tools/check_rstim_vs_stim_expanded_evidence.py \
  --correctness-dir benchmarks/rstim_vs_stim_simulator/results/distributions \
  --full-correctness benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json \
  --speed-dirs benchmarks/rstim_vs_stim_simulator/results/release,benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample,benchmarks/rstim_vs_stim_simulator/results/release-surface-detect \
  --dem-speed-dir benchmarks/rstim_vs_stim_simulator/results/release-dem-sample
cargo test --workspace
```

## Scope Limits

- Do not create or modify benchmark result artifacts.
- Do not add timing thresholds or cross-machine gates.
- Do not change simulator, detector, or DEM behavior.
- Do not redesign the site renderer; use its existing manifest-driven cards.

## Self-Review

- The design separates correctness and speed evidence and preserves issue #406.
- Every expanded evidence directory maps to required, hash-recorded site files.
- The copy helper and both site validators share one checked-path policy.
- The claim diagnostic and rejected-path diagnostic satisfy both negative
  controls.
- No benchmark generation, speed threshold, placeholder, or unrelated site
  redesign remains.
