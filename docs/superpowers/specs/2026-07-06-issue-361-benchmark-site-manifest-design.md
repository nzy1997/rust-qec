# Issue 361 Benchmark Site Manifest Design

Issue: #361 Add a benchmark documentation site data manifest

Date: 2026-07-06

## Context

The repository already has a plain static site under `site/`, checked QP101
assets, benchmark evidence documents, and several benchmark artifact policies.
Issue #360 designs a broader benchmarked documentation site that keeps the
QP101 schema browser but adds feature walkthroughs, benchmark methodology,
benchmark results, limitations, and claims policy. Issue #359 classifies the
current benchmark directions and calls out that broad external benchmark claims
need stronger provenance before new campaigns are run.

Relevant local evidence for this manifest:

- `docs/showcases/benchmark-evidence.md`
- `docs/showcases/qec-code-random-window-benchmark.md`
- `benchmarks/surface_decoder_compare/results/full/`
- `benchmarks/bb_circuit_bposd_compare/results/full/`
- `benchmarks/qec_code_random_window/README.md`
- `.github/workflows/ci.yml`
- `.github/workflows/rbposd-parity.yml`
- `rstim/doc/performance_parity.md`

## Automatic Answers

This Agent Desk run is non-interactive, so the standing answer policy resolves
the normal Superpowers approval gates:

- No visual companion is needed because the deliverable is a machine-readable
  JSON manifest and validator.
- The design is approved from issues #359, #360, and #361 plus the local
  evidence documents.
- Use JSON in `site/benchmark-site.json`, with one top-level family per
  site-facing benchmark result family from #360.
- Use a Python standard-library validator at `tools/check_site_manifest.py`.
- Include focused Python unit tests for TDD and keep the issue-required
  negative controls inside the validator self-test.
- Do not run or regenerate benchmark campaigns. The manifest describes existing
  checked artifacts, local-only commands, future work, and claim limits.

## Approaches Considered

1. Add a small hand-authored JSON manifest plus a strict validator. This is the
   recommended approach because it matches #361, keeps site rendering out of
   scope, and makes later page rendering consume a stable data shape.
2. Generate the manifest by scraping Markdown pages and directories. This would
   reduce duplication but would make claims policy implicit and fragile.
3. Store the data in Python instead of JSON. This would simplify validation but
   would be less suitable as static-site data.

## Design

Create `site/benchmark-site.json` with:

- `schema_version`, fixed to `1`.
- `families`, a list of benchmark family objects.
- Family fields: `id`, `title`, `status`, `source_docs`, `claims_limit`, and
  `evidence_items`.
- Evidence item fields: `id`, `title`, `status`, `tier`, `artifacts`,
  `commands`, `provenance_requirements`, `provenance_sources`, and
  `claims_limit`.

The family `status` and evidence item `status` values are one of `existing`,
`partial`, `future`, or `local-only`. `tier` is descriptive site data, such as
`smoke`, `full`, `reference-gap`, `regression-gate`, or `future`.

The first manifest covers the #360 benchmark result families:

- `surface-decoder-comparison`: `existing`; checked full-tier CSV and PNG
  artifacts, plus local smoke and `rsinter` framework commands.
- `bb-circuit-bposd-comparison`: `partial`; checked BB72/BB144 full-tier CSV,
  Markdown, PNG, and reference-gap report, plus readiness and diagnostic local
  commands for broader BB coverage.
- `qec-code-random-window`: `local-only`; source docs and commands for smoke,
  full, no-target, multi-seed, ladder, and issue-225 readiness flows. It has no
  checked generated outputs because `benchmarks/out/` is ignored.
- `rstim-vs-stim-simulator`: `future`; planned simulator-level benchmarks
  against Stim for sampling, detection, DEM extraction, conversion, repeat-heavy
  circuits, atom-loss fallback paths, and memory use.
- `internal-regression-evidence`: `partial`; existing `rstim perf ci` and
  `rbposd` parity gates as regression evidence, not broad speed claims.

Artifact entries are checked artifacts only. Each artifact object has `path`,
`kind`, and `checked: true`. Local-only and future items use empty `artifacts`
arrays and record generated-output locations only in `claims_limit` or
provenance text, so ignored outputs are not represented as site-facing checked
artifacts.

## Validator

`tools/check_site_manifest.py` validates:

- JSON shape and duplicate IDs.
- Required family and evidence item fields, including `claims_limit`.
- Allowed statuses.
- `source_docs` exist, are git-tracked, and are not ignored.
- Checked artifact paths exist, are git-tracked, and are not ignored.
- Local-only or future evidence items do not list checked artifacts.
- Every required #360 family ID is present.
- Each evidence item has at least one command or checked artifact.
- Each evidence item lists provenance requirements and provenance sources.

The CLI supports:

```sh
python3 tools/check_site_manifest.py --self-test
python3 tools/check_site_manifest.py --repo-root . site/benchmark-site.json
```

The self-test builds a temporary git repository with a valid fixture manifest,
then confirms the required negative controls fail:

- A checked artifact changed to `benchmarks/missing/results.csv`.
- A manifest entry with omitted `claims_limit`.
- A checked artifact changed to an ignored `benchmarks/out/` path.

Errors name the bad entry ID and the violated rule.

## Error Handling

Validation accumulates errors and reports them to stderr. Each error begins
with the entry ID where possible, for example
`surface-decoder-full: checked artifact path does not exist:
benchmarks/missing/results.csv`. The command exits nonzero on any error and
prints one `ok: family ...` line per accepted family on success.

## Testing

Use TDD with `tools/test_check_site_manifest.py`:

1. A fixture manifest with checked artifacts validates successfully.
2. Missing required family IDs, missing `claims_limit`, missing checked
   artifacts, and ignored checked artifact paths are rejected with errors naming
   the offending entry ID.
3. The validator self-test exits 0 only after those negative controls are
   rejected.

Required verification:

```sh
python3 -m unittest tools.test_check_site_manifest -q
python3 tools/check_site_manifest.py --self-test
python3 tools/check_site_manifest.py --repo-root . site/benchmark-site.json
cargo test
```

Out of scope: site rendering, benchmark campaign execution, generated
benchmark output commits, and new performance ranking claims.
