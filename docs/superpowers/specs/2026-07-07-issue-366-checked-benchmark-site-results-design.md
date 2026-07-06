# Issue 366 Checked Benchmark Site Results Design

Issue: #366 Publish checked surface-decoder and BB comparison evidence on the site

Date: 2026-07-07

## Context

The site already has a static documentation shell, benchmark methodology
content, and `site/benchmark-site.json`. The manifest lists checked full-tier
surface-decoder artifacts and checked BB72/BB144 BP-OSD artifacts. `make
build-site` copies checked artifacts under `_site/benchmarks/...` and copies the
manifest to `_site/data/benchmark-site.json`.

The missing surface is a result-oriented site section that makes the checked
artifact families visible without duplicating benchmark values in free-form
HTML. The page should link the CSV/report artifacts, show the copied plot
images, list reproduction commands, and display the status and `claims_limit`
text from the manifest.

This Agent Desk run is non-interactive. The standing answer policy resolves the
normal Superpowers gates:

- No visual companion is needed because the site already has the relevant card
  layout and the issue is primarily a manifest-backed evidence contract.
- The design is approved from issue #366, the merged manifest/artifact work from
  #361 and #362, and the claims policy from #365.
- Use the existing static site and manifest renderer instead of adding a site
  generator or copying benchmark values into HTML.
- Preserve checked result CSVs and images as committed artifacts. Do not run new
  full benchmark campaigns.
- Use the recommended Superpowers execution option, Subagent-Driven
  Development, because the writing-plans skill marks it recommended.

## Approaches Considered

1. Extend the existing client-side manifest renderer with a checked-results
   section keyed by the two checked evidence item ids. This is the chosen
   approach because artifact paths, commands, status, tier, and claims limits
   stay sourced from `site/benchmark-site.json`.
2. Generate static result cards during `make build-site`. This would work, but
   it adds a generation layer to a static site that already fetches JSON for the
   schema browser and benchmark status inventory.
3. Hand-author links and command blocks in `site/index.html`. This is rejected
   because the issue explicitly asks to render from manifest/artifact paths
   rather than copying values into free-form HTML.

## Design

Add a `#checked-benchmark-results` section after the existing benchmark
evidence overview and before benchmark methodology. The static HTML will carry
only the section heading, a stable container, and the two evidence item ids:
`surface-decoder-full` and `bb-circuit-full`.

Extend `site/app.js` so the single `data/benchmark-site.json` fetch renders both
the existing benchmark status inventory and the checked result cards. The new
renderer will:

- find the checked item in the manifest by evidence item id
- render family title, item title, family status, item status, and tier
- render item `claims_limit` from the manifest
- render checked artifact links from `item.artifacts`
- render image artifacts as plot previews using the copied artifact path
- render reproduction commands from `item.commands`
- render provenance source links and caveats from manifest fields
- show a concise error if a configured checked item is missing

Add `caveats` arrays to the two checked evidence items in
`site/benchmark-site.json`. The BB caveat must include the preserved wording:
the BB72/BB144 full rows are batched, error-budget-stopped paired comparison
rows and are not a fixed-shot reproduction of the pinned Bravyi reference
curve. Rendering caveats from the manifest keeps the limitation adjacent to the
artifact links and lets source tests fail if it is removed.

Extend `rstim/tests/site_contract.rs` with
`checked_benchmark_artifacts_are_linked`. The test will parse the manifest,
confirm the two checked evidence items contain the expected checked artifacts,
commands, claims limits, and caveats, and confirm the site JavaScript renders
links/images/commands/caveats from manifest fields. It will also reject
hard-coded checked artifact paths in `site/index.html` and `site/app.js`.

Extend `tools/check_site_manifest.py` built-site validation so it recognizes the
checked-results renderer hooks and rejects checked benchmark artifact references
in built `index.html` or `app.js` that are not listed as checked artifacts in
the manifest. This keeps the negative control for nonexistent or unlisted
checked artifact paths in the manifest/build contract.

## Error Handling

If a configured checked item is absent from the manifest, the page will render a
small error panel in the checked-results section naming the missing item id.
If the manifest fetch fails, the existing benchmark manifest error path will
continue to report the fetch failure and link the JSON path.

The manifest checker will accumulate built-site wiring and artifact-reference
errors with the existing manifest validation errors and exit nonzero when any
checked-result hook or checked artifact copy is missing.

## Testing

Use TDD around the new site contract and manifest checker behavior:

1. Add the focused Rust test `checked_benchmark_artifacts_are_linked` and run it
   to observe the expected failure before implementation.
2. Add Python validator coverage for built-site checked-result wiring and
   unlisted checked artifact references, then run it to observe the expected
   failure before implementation.
3. Implement the manifest caveats, site section, renderer, CSS, and validator
   checks.
4. Re-run the focused tests and the issue verification commands.

Required verification:

```sh
make build-site
python3 -m unittest benchmarks.surface_decoder_compare.tests.test_docs_contract -q
python3 -m benchmarks.bb_circuit_bposd_compare.validate_reference_gap_report --results benchmarks/bb_circuit_bposd_compare/results/full/results.csv --report benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md
python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json
cargo test -p rstim --test site_contract checked_benchmark_artifacts_are_linked -q
cargo test
```

Out of scope: regenerating benchmark campaigns, changing checked result CSVs,
claiming broad decoder rankings, or redesigning the documentation site.

## Self Review

- Placeholder scan: no unresolved placeholders or TODOs.
- Consistency check: the design keeps artifact paths and result-card data in
  the manifest and uses static HTML only for the section shell.
- Scope check: the design is limited to site rendering, manifest metadata,
  validator wiring, and contracts.
- Ambiguity check: the two checked evidence item ids and required BB caveat
  wording are explicit.
