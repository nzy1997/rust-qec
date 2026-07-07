# Issue 382 Checked Result Provenance Rendering Design

Issue: #382 Render checked artifact provenance on result cards

Date: 2026-07-08

## Context

Issue #380 added canonical provenance schema validation for checked benchmark
evidence, and issue #381 added SHA-256 validation for checked artifact hashes.
The manifest now carries `item.provenance` for `surface-decoder-full` and
`bb-circuit-full`, but `site/app.js` only renders artifact links, commands,
caveats, and provenance source links on checked result cards.

This Agent Desk run is non-interactive. The standing answer policy resolves the
normal Superpowers gates:

- No visual companion is needed because the display shape is textual and the
  issue already specifies the card content.
- The design is approved from issue #382, parent issue #379, dependency issues
  #380 and #381, and the existing checked-result renderer patterns.
- Use the recommended Superpowers execution option, Subagent-Driven
  Development, because the writing-plans skill marks it recommended.
- Keep all displayed values manifest-backed; do not hard-code surface-decoder
  or BB-circuit provenance values in `site/index.html`, `site/app.js`, or test
  fixtures except as fixture manifest data.

## Approaches Considered

1. Add a focused `renderProvenance(provenance)` helper in `site/app.js`, call it
   from `renderCheckedBenchmarkResults()` with `item.provenance`, and validate
   that built `app.js` keeps both the helper and the call. This is the chosen
   approach because it matches the issue recommendation and keeps rendering in
   the existing manifest-backed card path.
2. Render only a compact artifact-hash summary and link to the raw manifest for
   other fields. This is rejected because issue #382 asks readers to see
   recorded values and `not_recorded` reasons directly on checked result cards.
3. Add static provenance copy to `site/index.html`. This is rejected because
   the values must come from `data/benchmark-site.json`, not hand-written page
   content.

## Design

Add `renderProvenance(provenance)` near the checked-result helpers in
`site/app.js`. The helper returns a small fallback paragraph when provenance is
missing or malformed, although checked manifest validation should prevent that
for promoted checked artifacts.

For each provenance key, render:

- the field name in `code` text;
- a status badge for `recorded` or `not_recorded`;
- the recorded value when it can be represented compactly;
- the `not_recorded.reason` text for historical metadata gaps;
- for `artifact_hashes`, one path/hash row per recorded artifact hash when the
  hash map is available, with the artifact path and SHA-256 escaped and
  manifest-backed.

For compact recorded values:

- strings, numbers, and booleans render inline;
- arrays render as list items;
- simple objects render as key/value list items;
- nested objects fall back to JSON text only when compact enough for the card.

Call `renderProvenance(item.provenance)` inside each checked result card after
reproduction commands and before caveats. This keeps artifact links, commands,
caveats, and source links in place while adding canonical provenance status.

Extend `tools/check_site_manifest.py` built-site validation narrowly. The
`validate_site_root()` app marker check should reject built `app.js` when it no
longer contains the provenance rendering hook:

- `renderProvenance`
- `renderProvenance(item.provenance)`
- `item.provenance`

This is intentionally not a schema or hash validator. Canonical provenance
shape remains owned by the manifest validation added in #380, and hash content
validation remains owned by #381.

## Error Handling

`renderProvenance()` escapes every displayed value with the existing
`escapeHtml()` helper. Unknown recorded shapes render as compact JSON when
short enough; otherwise they render a count-oriented summary to avoid growing
cards with large manifest objects. Malformed provenance entries render a
reviewer-visible status of `unspecified` instead of throwing in the browser.

`validate_site_root()` should produce an error message naming provenance wiring
when a fixture removes `item.provenance` or the `renderProvenance()` call. The
negative control can then distinguish this issue's site wiring check from the
manifest schema and hash checks.

## Testing

Use TDD around the existing site-source contract and Python built-site
validation tests:

1. Add Rust contract markers in `rstim/tests/site_contract.rs` requiring
   `renderProvenance`, `renderProvenance(item.provenance)`, `item.provenance`,
   and provenance status strings in `site/app.js`.
2. Add Python fixture coverage in `tools/test_check_site_manifest.py` so a valid
   built `app.js` contains the provenance markers and a mutation that removes
   the hook fails with an error naming provenance wiring.
3. Run those focused tests and observe the expected failures before production
   changes.
4. Add the renderer helper and marker validation.
5. Update `tools/check_site_build.py` fixture provenance data only as needed so
   built-site validation remains consistent with #380 and #381.
6. Run the issue verification commands and the required Rust test suite.

Required verification:

```sh
make build-site
python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json
python3 tools/check_site_build.py _site
cargo test
```

Negative control:

```sh
make build-site
python3 - <<'PY'
from pathlib import Path
app = Path("_site/app.js")
text = app.read_text(encoding="utf-8")
text = text.replace("${renderProvenance(item.provenance)}", "")
text = text.replace("function renderProvenance(provenance)", "function removedProvenance(provenance)")
text = text.replace("item.provenance", "item.removed_provenance")
app.write_text(text, encoding="utf-8")
PY
python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json
```

The final command must exit nonzero and name provenance wiring.

Out of scope: site redesign, new charts, new benchmark claims, browser
automation, and duplicating the field-by-field provenance schema or hash
validation from #380 and #381.

## Self Review

- Marker scan: no unresolved open markers.
- Consistency check: rendering and validation both use `item.provenance` from
  the checked-result card path.
- Scope check: the design only touches checked card rendering, built-site
  provenance wiring validation, fixtures/tests, and Superpowers workflow
  artifacts.
- Ambiguity check: renderer placement, compact value handling, artifact-hash
  rows, and negative-control behavior are explicit.
