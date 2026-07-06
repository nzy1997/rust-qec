# Issue 368 Static Site Build Checker Design

Issue: #368 Add a reviewer-readable static site build contract

Date: 2026-07-07

## Context

The static site now spans the QP101 documentation shell, schema browser,
gallery assets, copied benchmark manifest data, copied checked benchmark
artifacts, methodology copy, checked result cards, and local-only or future
benchmark classifications. The existing `tools/check_site_manifest.py`
validates the manifest shape and copied checked artifacts, but reviewers still
need one command that validates the whole built `_site/` tree and prints a
human-readable PASS/WARN/FAIL summary.

Relevant local inputs:

- `make build-site`, which refreshes `_site/`, copies QP101 schema/protocol and
  examples, builds gallery SVGs, and copies benchmark data.
- `tools/check_site_manifest.py`, which owns the manifest and checked-artifact
  rules and must stay the manifest authority.
- `site/index.html`, `site/app.js`, and `site/benchmark-site.json`, which define
  the source site contract.
- `rstim/tests/site_contract.rs`, which documents the site markers added by
  issues #363 through #367.
- Sibling Superpowers specs for issues #361 through #367, which establish the
  manifest, QP101 preservation, workspace walkthrough, benchmark methodology,
  checked result, qec-code local-only, and future benchmark constraints.

GitHub issue and PR context could not be fetched with `gh` in this sandbox
because the configured local proxy is blocked. The provided Agent Desk issue
body, merged local issue artifacts, and repository state are therefore the
available authoritative context.

## Automatic Answers

This Agent Desk run is non-interactive, so the standing answer policy resolves
the normal Superpowers gates:

- No visual companion is needed because the deliverable is a command-line
  static-site checker and text summary.
- The design is approved from issue #368, the issue #361-#367 local context,
  and the existing site/manifest contracts.
- Reuse `tools.check_site_manifest` for manifest and copied-artifact checks.
- Use Python standard library only; do not add browser automation or external
  HTTP link checks.
- Add self-test mutation coverage for the issue's required negative controls.
- Use the recommended Superpowers execution option, Subagent-Driven
  Development, because the writing-plans skill marks it recommended.

## Approaches Considered

1. Add `tools/check_site_build.py` as a wrapper-level checker that calls
   `tools.check_site_manifest`, validates site-local HTML links/assets and
   required copy/caveat/classification contracts, then prints a PASS/WARN/FAIL
   summary. This is the chosen approach because it gives reviewers one command
   while keeping manifest rules centralized.
2. Extend `tools/check_site_manifest.py` to validate every site concern. This
   would avoid a second CLI, but it would overload a manifest-specific tool with
   page, link, QP101, and classification checks.
3. Add Rust contract tests only. This would preserve developer coverage but
   would not give reviewers the requested `_site/` checker or readable summary.

## Build Checker Design

Create `tools/check_site_build.py`. Its CLI supports:

```sh
python3 tools/check_site_build.py --self-test
python3 tools/check_site_build.py _site
```

The built-site check accepts a site root path. It derives `repo_root` from the
current working directory by default and derives the manifest path as
`<site_root>/data/benchmark-site.json`. It validates:

- Required built files: `index.html`, `styles.css`, `app.js`,
  `qp101.schema.json`, `QP101-ZY.md`, `data/benchmark-site.json`, the three
  copied QP101 example JSON files, and the three generated gallery SVGs.
- Local links and media references in built HTML and JS. The checker should
  parse `href`, `src`, and string-literal paths using standard-library HTML and
  regex helpers, ignore anchors, downloads, `mailto:`, and external
  `http(s)://` links, and verify same-site paths exist. It should also verify
  same-page anchors in `href="#..."` resolve to IDs in the built HTML.
- Required anchors and sections: `docs-home`, `workspace-overview`,
  `feature-walkthroughs`, `benchmark-evidence`, `checked-benchmark-results`,
  `benchmarks`, `benchmark-manifest`, `checked-benchmark-result-cards`,
  `qp101`, `schema-browser`, `operations`, `gallery`, and `examples`.
- QP101 assets: copied schema, protocol draft, example JSON files, and gallery
  SVGs must exist and be non-empty.
- Manifest and copied checked artifacts by calling
  `tools.check_site_manifest.validate_manifest(repo_root, manifest_path,
  site_root=site_root)` and `validate_site_root(site_root, manifest_path)`.
- Checked benchmark artifacts copied from the manifest. The checker should name
  the checked benchmark artifact paths in the PASS summary.
- Claims-policy phrases in built HTML and/or manifest fields, including
  "Claims Policy", "Publishable Evidence", "Local-Only Evidence",
  "smoke checks verify wiring", "full evidence can describe the committed
  checked run", "committed-run evidence", and "not a general decoder ordering
  claim".
- Local-only and future classifications: qec-code random-window must remain
  `local-only` or `partial` without checked artifacts under `benchmarks/out/`;
  `rstim-vs-stim-simulator` must remain `future` with no checked artifacts.

## Summary Output

The checker accumulates `CheckResult` entries with a status of `PASS`, `WARN`,
or `FAIL`, a short area name, and concise detail. It prints one line per area,
then a final summary line:

```text
PASS QP101 assets: schema, protocol, examples, and gallery assets are present
PASS workspace overview: required anchors and walkthrough sections are present
PASS benchmark methodology: claims-policy phrases are present
PASS checked benchmark artifacts: <N> checked artifacts copied
PASS local-only/future classifications: qec-code local-only/partial and rstim-vs-stim future
SUMMARY: PASS (<N> checks, 0 warnings, 0 failures)
```

The process exits 0 only when there are no failures. Warnings are allowed but
still counted. Current expected verification should produce an all-PASS
summary.

## Self-Test And Negative Controls

The self-test builds a temporary fixture site with a tiny git repository,
minimal built HTML/JS, a valid manifest fixture, QP101 assets, gallery SVGs,
and checked benchmark artifacts. It then confirms the valid fixture passes and
the required mutations fail:

- Missing QP101 schema file.
- Missing copied checked benchmark plot.
- Site HTML missing the claims-policy caveat.
- Built site linking a checked artifact path that is not listed as a checked
  manifest artifact.

The self-test uses the same public validation functions as the CLI, so mutation
coverage exercises the actual checker contract rather than a parallel test
harness.

## Error Handling

All checks should accumulate failures instead of stopping at the first missing
file. File decoding uses UTF-8. Malformed JSON is reported through the manifest
checker. Link parsing should ignore external links because the issue explicitly
excludes browser automation and external HTTP checking.

## Testing

Use TDD with focused Python unit tests and the checker self-test:

1. Add tests for a valid fixture summary and the four required mutations.
2. Run the focused Python tests and confirm they fail before the checker exists.
3. Implement the checker with standard-library code and reuse
   `tools.check_site_manifest`.
4. Re-run the focused tests and the issue verification commands.

Required verification:

```sh
make build-site
python3 tools/check_site_build.py --self-test
python3 tools/check_site_build.py _site
cargo test
```

Expected result: all commands exit 0. The checker PASS summary names QP101,
workspace overview, benchmark methodology, checked benchmark artifacts, and
local-only/future benchmark classifications.

Out of scope: browser automation, external HTTP link checking, introducing a
site framework, regenerating benchmark campaigns, and changing benchmark
artifact contents.

## Self Review

- Placeholder scan: no unresolved markers or incomplete sections.
- Consistency check: the checker delegates manifest/artifact policy to
  `tools.check_site_manifest`, matching the technical recommendation.
- Scope check: the design only adds a reviewer-facing built-site checker and
  tests.
- Ambiguity check: required files, anchors, phrases, classifications, self-test
  mutations, CLI commands, and expected summary content are explicit.
