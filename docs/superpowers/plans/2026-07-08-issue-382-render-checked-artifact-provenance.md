# Issue 382 Checked Artifact Provenance Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render canonical manifest provenance on checked benchmark result cards and reject built-site `app.js` output that drops the provenance rendering hook.

**Architecture:** Keep all provenance values manifest-backed by adding one `renderProvenance(provenance)` helper in `site/app.js` and calling it from `renderCheckedBenchmarkResults()` with `item.provenance`. Extend `tools/check_site_manifest.py` only with string-marker validation for provenance renderer wiring; schema and hash validation remain in the existing manifest validator. Update test fixtures that build checked-artifact manifests so they include the required provenance object introduced by #380 and #381.

**Tech Stack:** Vanilla JavaScript static site, Python `unittest` checker tests, Rust integration contract test, Cargo workspace.

## Global Constraints

- Render provenance status for checked result cards so readers can see recorded values, `not_recorded` reasons, and artifact hash status from the manifest.
- In: `item.provenance` from `data/benchmark-site.json`.
- Out: checked result cards for `surface-decoder-full` and `bb-circuit-full` display provenance fields from the manifest.
- Keep all values manifest-backed; do not hard-code surface or BB provenance values in `site/index.html`.
- Add a small `renderProvenance` helper in `site/app.js`, and call it from the checked-result card renderer with `item.provenance`.
- Display field name, `recorded` or `not_recorded`, compact recorded value, `not_recorded.reason`, and artifact hash count or path/hash rows for checked artifacts.
- Extend `tools/check_site_manifest.py` built-site validation only enough to reject missing provenance renderer wiring, such as removing the `renderProvenance(item.provenance)` path.
- Do not duplicate the full field-by-field schema or hash validation from #380 and #381.
- Preserve checked result card artifact links, commands, caveats, and provenance source links.
- Out of scope: site redesign, new charts, new benchmark claims, browser automation.

---

### Task 1: Render And Validate Checked Provenance Wiring

**Files:**
- Modify: `site/app.js`
- Modify: `tools/check_site_manifest.py`
- Modify: `tools/test_check_site_manifest.py`
- Modify: `tools/check_site_build.py`
- Modify: `rstim/tests/site_contract.rs`

**Interfaces:**
- Consumes: checked evidence `item.provenance` values already validated by `tools/check_site_manifest.py`.
- Produces: `renderProvenance(provenance)` in `site/app.js`, called as `renderProvenance(item.provenance)` from `renderCheckedBenchmarkResults()`.
- Produces: built-site marker validation requiring `renderProvenance`, `renderProvenance(item.provenance)`, and `item.provenance` in `_site/app.js`.

- [ ] **Step 1: Write the failing Python site-root wiring test**

In `tools/test_check_site_manifest.py`, update the `_site/app.js` fixture strings in `write_fixture_manifest()` and `test_accepts_built_site_manifest_when_site_root_is_wired()` so valid fixtures include provenance markers:

```python
(root / "_site/app.js").write_text(
    'fetch("data/benchmark-site.json"); renderBenchmarkManifest(manifest); '
    'renderCheckedBenchmarkResults(manifest); checkedBenchmarkItems; '
    'family.status; family.claims_limit; item.status; item.claims_limit; '
    'item.artifacts; item.commands; item.caveats; item.provenance; renderProvenance; '
    'renderProvenance(item.provenance); artifact.checked; artifact.kind === "image";\n',
    encoding="utf-8",
)
```

Add this test next to `test_rejects_built_site_without_checked_result_wiring()`:

```python
def test_rejects_built_site_without_provenance_renderer_wiring(self) -> None:
    repo, _, built_manifest_path = self.write_fixture_manifest()
    (repo / "_site/app.js").write_text(
        'fetch("data/benchmark-site.json"); renderBenchmarkManifest(manifest); '
        'renderCheckedBenchmarkResults(manifest); checkedBenchmarkItems; '
        'family.status; family.claims_limit; item.status; item.claims_limit; '
        'item.artifacts; item.commands; item.caveats; artifact.checked; artifact.kind === "image";\n',
        encoding="utf-8",
    )

    errors = check_site_manifest.validate_site_root(repo / "_site", built_manifest_path)

    self.assertTrue(
        any("provenance wiring" in error and "item.provenance" in error for error in errors),
        errors,
    )
```

- [ ] **Step 2: Write the failing Rust site-source contract**

In `rstim/tests/site_contract.rs`, extend the `checked_benchmark_artifacts_are_linked()` app marker list with:

```rust
"renderProvenance",
"renderProvenance(item.provenance)",
"item.provenance",
"recorded",
"not_recorded",
"artifact_hashes",
```

Then add provenance assertions for both checked manifest items:

```rust
for (item_id, item) in [
    ("surface-decoder-full", surface_item),
    ("bb-circuit-full", bb_item),
] {
    let provenance = item["provenance"]
        .as_object()
        .unwrap_or_else(|| panic!("{item_id} must carry canonical provenance"));
    for field in [
        "schema_version",
        "artifact_date",
        "source_commit",
        "commands",
        "cpu_model",
        "artifact_hashes",
    ] {
        assert!(
            provenance.contains_key(field),
            "{item_id} provenance is missing field {field}"
        );
    }
    assert_eq!(
        provenance["artifact_hashes"]["status"].as_str(),
        Some("recorded"),
        "{item_id} artifact hashes must be recorded"
    );
}
```

- [ ] **Step 3: Run focused tests to verify RED**

Run:

```sh
python3 -m unittest tools.test_check_site_manifest.SiteManifestValidatorTest.test_rejects_built_site_without_provenance_renderer_wiring -q
cargo test -p rstim --test site_contract checked_benchmark_artifacts_are_linked -q
```

Expected before implementation: Python fails because `validate_site_root()` does not yet name provenance wiring, and Rust fails because `site/app.js` lacks the new provenance renderer markers.

- [ ] **Step 4: Fix the built-site fixture provenance baseline**

`tools/test_check_site_build.py` currently exposes a baseline failure from #380/#381 because `tools/check_site_build.py::make_fixture_site()` creates checked artifacts without `provenance`. In `tools/check_site_build.py`, add a local helper before the fixture manifest:

```python
    provenance_reason = "historical fixture predates canonical provenance capture"

    def fixture_provenance(commands: list[str], artifact_hashes: dict[str, dict[str, str]]) -> dict[str, object]:
        return {
            "schema_version": 1,
            "artifact_date": {"status": "not_recorded", "reason": provenance_reason},
            "source_commit": {"status": "not_recorded", "reason": provenance_reason},
            "commands": {"status": "recorded", "value": commands},
            "os": {"status": "not_recorded", "reason": provenance_reason},
            "cpu_model": {"status": "not_recorded", "reason": provenance_reason},
            "rust_version": {"status": "not_recorded", "reason": provenance_reason},
            "python_version": {"status": "not_recorded", "reason": provenance_reason},
            "dependency_versions": {"status": "not_recorded", "reason": provenance_reason},
            "external_repository_commits": {"status": "not_recorded", "reason": provenance_reason},
            "seed_policy": {"status": "not_recorded", "reason": provenance_reason},
            "build_profile": {"status": "not_recorded", "reason": provenance_reason},
            "shots_or_error_budget": {"status": "not_recorded", "reason": provenance_reason},
            "artifact_hashes": {"status": "recorded", "value": artifact_hashes},
        }
```

For the fixture file contents already written in `make_fixture_site()`, use these hashes:

```python
surface_hashes = {
    "benchmarks/surface_decoder_compare/results/full/results.csv": {
        "sha256": "5f99836718375eb522c7113382a65ebba0256e8ead0fe2c8c1f0a0aea86ff891"
    },
    "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png": {
        "sha256": "33d8344a7135c42aa3876706b908f95b702d83ff53e05e4aaff17c07bf67a98e"
    },
}
bb_hashes = {
    "benchmarks/bb_circuit_bposd_compare/results/full/results.csv": {
        "sha256": "5f99836718375eb522c7113382a65ebba0256e8ead0fe2c8c1f0a0aea86ff891"
    },
    "benchmarks/bb_circuit_bposd_compare/results/full/summary.md": {
        "sha256": "88501c2f5e6660af97d9cafe49c86afa7adff4dc92cbe9e27141b4ef45642ee8"
    },
    "benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png": {
        "sha256": "33d8344a7135c42aa3876706b908f95b702d83ff53e05e4aaff17c07bf67a98e"
    },
    "benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md": {
        "sha256": "f999c0097163daf1dbd5fc72414fa2f5ec0fd43a13ef088f4c7fd32372dc61d4"
    },
}
```

Add `provenance` to the two checked fixture items:

```python
"provenance": fixture_provenance(["make surface-decoder-compare-full"], surface_hashes),
```

and:

```python
"provenance": fixture_provenance(["make bb-circuit-bposd-compare-full"], bb_hashes),
```

Also update the fixture `_site/app.js` string to include:

```javascript
function renderProvenance(provenance) { return provenance; }
const provenanceMarkers = ["item.provenance", "renderProvenance(item.provenance)", "artifact_hashes"];
```

- [ ] **Step 5: Implement `renderProvenance()` in `site/app.js`**

Add these helpers after `renderSourceLinks()`:

```javascript
  function provenanceFieldLabel(name) {
    return String(name || "")
      .replace(/_/g, " ")
      .replace(/^./, (char) => char.toUpperCase());
  }

  function renderCompactValue(value) {
    if (value === null || value === undefined) {
      return "";
    }
    if (Array.isArray(value)) {
      if (!value.length) {
        return '<span class="provenance-muted">empty</span>';
      }
      return `<ul class="provenance-value-list">${value.map((item) => `<li>${escapeHtml(item)}</li>`).join("")}</ul>`;
    }
    if (typeof value === "object") {
      const entries = Object.entries(value);
      if (!entries.length) {
        return '<span class="provenance-muted">empty</span>';
      }
      return `<ul class="provenance-value-list">${entries
        .map(([key, entryValue]) => `<li><code>${escapeHtml(key)}</code>: ${escapeHtml(JSON.stringify(entryValue))}</li>`)
        .join("")}</ul>`;
    }
    return `<span>${escapeHtml(value)}</span>`;
  }

  function renderArtifactHashes(entry) {
    if (!entry || entry.status !== "recorded" || !entry.value || typeof entry.value !== "object") {
      return renderCompactValue(entry && entry.value);
    }
    const rows = Object.entries(entry.value)
      .map(([path, hashEntry]) => {
        const sha = hashEntry && typeof hashEntry === "object" ? hashEntry.sha256 : "";
        return `
          <li>
            <code>${escapeHtml(path)}</code>
            <span class="provenance-hash">${escapeHtml(sha || "sha256 not recorded")}</span>
          </li>
        `;
      })
      .join("");
    return `
      <p class="provenance-muted">${Object.keys(entry.value).length} checked artifact hashes recorded</p>
      <ul class="provenance-hash-list">${rows}</ul>
    `;
  }

  function renderProvenance(provenance) {
    if (!provenance || typeof provenance !== "object") {
      return "<p>No canonical provenance is recorded for this checked result.</p>";
    }
    const rows = Object.entries(provenance)
      .map(([field, entry]) => {
        if (field === "schema_version") {
          return `
            <li class="provenance-row">
              <div class="provenance-row-heading">
                <code>${escapeHtml(field)}</code>
                <span class="badge">recorded</span>
              </div>
              ${renderCompactValue(entry)}
            </li>
          `;
        }
        const status = entry && typeof entry === "object" ? entry.status : "unspecified";
        const body =
          field === "artifact_hashes"
            ? renderArtifactHashes(entry)
            : status === "not_recorded"
              ? `<p class="provenance-muted">${escapeHtml(entry.reason || "reason not recorded")}</p>`
              : renderCompactValue(entry && entry.value);
        return `
          <li class="provenance-row">
            <div class="provenance-row-heading">
              <code>${escapeHtml(field)}</code>
              <span class="badge">${escapeHtml(status)}</span>
            </div>
            ${body}
          </li>
        `;
      })
      .join("");
    return `<ul class="provenance-card-list">${rows}</ul>`;
  }
```

Then call it in the checked-result card after reproduction commands:

```javascript
            <h4>Provenance</h4>
            ${renderProvenance(item.provenance)}
```

- [ ] **Step 6: Implement site-root provenance marker validation**

In `tools/check_site_manifest.py`, extend `checked_result_markers` with:

```python
        "item.provenance",
        "renderProvenance",
        "renderProvenance(item.provenance)",
```

Split provenance marker reporting from general checked-result reporting:

```python
    provenance_markers = [
        "item.provenance",
        "renderProvenance",
        "renderProvenance(item.provenance)",
    ]
    missing_provenance_markers = [marker for marker in provenance_markers if marker not in app]
    if missing_provenance_markers:
        add_error(errors, scope, f"app.js missing provenance wiring: {missing_provenance_markers}")
```

Keep the broader checked-result marker error unchanged for artifacts, commands,
caveats, and image handling.

- [ ] **Step 7: Run focused tests to verify GREEN**

Run:

```sh
python3 -m unittest tools.test_check_site_manifest.SiteManifestValidatorTest.test_rejects_built_site_without_provenance_renderer_wiring -q
cargo test -p rstim --test site_contract checked_benchmark_artifacts_are_linked -q
python3 -m unittest tools.test_check_site_build -q
```

Expected: all commands exit 0.

- [ ] **Step 8: Run issue verification and negative control**

Run:

```sh
make build-site
python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json
python3 tools/check_site_build.py _site
```

For the negative control, after a fresh `make build-site`, mutate `_site/app.js` only:

```sh
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

Expected: the final command exits nonzero and names provenance wiring.

Restore the built site afterward with:

```sh
make build-site
```

Finally run:

```sh
cargo test
```

Expected: all commands exit 0 except the intentional negative control.

- [ ] **Step 9: Commit**

Commit implementation changes:

```sh
git add site/app.js tools/check_site_manifest.py tools/test_check_site_manifest.py tools/check_site_build.py rstim/tests/site_contract.rs docs/superpowers/plans/2026-07-08-issue-382-render-checked-artifact-provenance.md
git commit -m "feat: render checked provenance on benchmark cards"
```
