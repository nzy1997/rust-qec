# Issue 365 Benchmark Methodology And Claims Policy Design

Issue: #365 Add benchmark methodology and claims-policy content to the site

Date: 2026-07-07

## Context

Issue #361 added `site/benchmark-site.json` as the site-facing benchmark
manifest. It already requires each benchmark family and evidence item to carry
`status` and `claims_limit`, and it distinguishes checked artifacts from
local-only or future evidence. Issue #365 adds the human-readable methodology
that explains how those fields constrain benchmark claims.

The current static site keeps the QP101 schema browser in `site/index.html`,
styles in `site/styles.css`, and browser behavior in `site/app.js`. The site
build target copies QP101 assets into `_site/`, but it does not yet copy or
render the benchmark manifest.

This Agent Desk run is non-interactive. The standing answer policy resolves the
normal Superpowers gates:

- No visual companion is needed because the work is a documentation/data-flow
  contract, not a visual layout choice.
- The design is approved from issue #365, the merged #361 manifest, and the
  local benchmark evidence documents.
- Use the existing static site instead of introducing a generated-site
  framework.
- Do not change benchmark runner schemas or regenerate benchmark campaigns.

## Approaches Considered

1. Add a benchmark methodology section to the existing static site and have
   `site/app.js` fetch the copied manifest to render each family and evidence
   item with its `status`, `tier`, and `claims_limit`. This is the chosen
   approach because it keeps the site simple while making status and claims
   limits manifest-backed.
2. Generate fully static HTML from the manifest during `make build-site`. This
   would make the built HTML self-contained, but it adds a site generation path
   for a small policy section and duplicates the existing client-side schema
   browser pattern.
3. Document the policy only in `docs/showcases/benchmark-evidence.md`. This is
   insufficient because the issue asks for a site section or page and for
   benchmark sections to expose manifest status and claims limits.

## Design

Add a `#benchmarks` section to `site/index.html` and link it from the primary
navigation. The section will cover:

- benchmark tiers: `smoke`, `full`, `extended`, and `reference reproduction`
- required provenance fields: OS, CPU, Rust version, Python version,
  dependency versions, external repository commits, command line, seeds, build
  profile, shots or error budgets, and date
- publishable evidence versus local-only evidence
- broad-claim limits, including the distinction that smoke checks verify wiring
  and local execution paths while full evidence can describe the committed
  checked run only within its recorded provenance

Add a manifest-backed table/card area with `id="benchmark-manifest"`. Static
HTML owns the methodology prose. `site/app.js` owns the rendered benchmark
status inventory and must fetch `data/benchmark-site.json`, then display each
family and evidence item with `status`, `tier`, and `claims_limit` from the
manifest. If the manifest cannot load, the page will show a concise error and
link to the JSON file.

Update `make build-site` to create `_site/data/` and copy
`site/benchmark-site.json` to `_site/data/benchmark-site.json`. This keeps the
site data path stable and matches the issue verification command.

Extend `tools/check_site_manifest.py` with an optional `--site-root` argument.
When supplied, the validator will also confirm that the built site has
`index.html`, `app.js`, and `data/benchmark-site.json`, and that the app is
wired to fetch and render manifest `status` and `claims_limit` fields. The
existing manifest validation remains the authority for required fields and
checked artifact policy.

Extend `rstim/tests/site_contract.rs` with
`benchmark_methodology_lists_required_provenance`. The test will inspect
`site/index.html`, `site/app.js`, and `site/benchmark-site.json`. It will fail
if the methodology omits required provenance fields, omits the named tiers, or
loses the manifest-backed status/claims-limit rendering hooks.

## Error Handling

The client-side manifest renderer will handle missing or invalid manifest
fetches by replacing the status inventory with an error panel. The manifest
validator will accumulate built-site errors alongside existing manifest errors
and exit nonzero when any required built-site file or wiring marker is missing.

## Testing

Use TDD for the Rust site contract and validator CLI extension:

1. Add the focused failing Rust contract test for methodology/provenance,
   manifest-backed status/claims-limit rendering, and smoke/full distinction.
2. Run the focused test and observe the expected failure.
3. Add the site section, app renderer, build-site copy step, and validator
   `--site-root` support.
4. Re-run the focused test and required verification commands.

Required verification:

```sh
make build-site
python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json
cargo test -p rstim --test site_contract benchmark_methodology_lists_required_provenance -q
cargo test
```

Out of scope: benchmark runner schema changes, new benchmark campaigns,
regenerating checked results, or making broad decoder performance claims.
