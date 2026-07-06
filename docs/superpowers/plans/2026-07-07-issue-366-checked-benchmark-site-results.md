# Issue 366 Checked Benchmark Site Results Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add manifest-backed static-site result cards for the checked surface-decoder and BB72/BB144 BP-OSD comparison artifacts.

**Architecture:** Keep the existing static site and one manifest fetch. `site/index.html` adds only the checked-results shell, `site/app.js` renders result cards from `data/benchmark-site.json`, `site/benchmark-site.json` carries item caveats next to artifact paths, and existing Rust/Python contracts verify the manifest-backed links and negative controls.

**Tech Stack:** Static HTML, CSS, browser JavaScript, JSON manifest data, Python standard-library validation, Rust integration tests with `serde_json`.

## Global Constraints

- Do not run new full benchmark campaigns.
- Do not change the checked result CSVs.
- Keep checked artifact paths sourced from `site/benchmark-site.json`; do not hard-code checked artifact paths in `site/index.html` or `site/app.js`.
- Preserve the wording that the BB72/BB144 full rows are batched, error-budget-stopped paired comparison rows and are not a fixed-shot reproduction of the pinned Bravyi reference curve.
- Result cards must link `benchmarks/surface_decoder_compare/results/full/results.csv`.
- Result cards must show/link `benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png`.
- Result cards must link `benchmarks/bb_circuit_bposd_compare/results/full/results.csv`.
- Result cards must show/link `benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png`.
- Result cards must link `benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md`.
- Result cards must render reproduction commands from the manifest.
- Result cards must render manifest-backed status and `claims_limit` text.
- `python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json` must reject checked artifact references in built `index.html` or `app.js` that are not listed as checked artifacts in the manifest.
- Focused Rust test name: `checked_benchmark_artifacts_are_linked`.

---

### Task 1: Add Checked Result Cards And Contracts

**Files:**
- Modify: `rstim/tests/site_contract.rs`
- Modify: `tools/check_site_manifest.py`
- Modify: `tools/test_check_site_manifest.py`
- Modify: `site/benchmark-site.json`
- Modify: `site/index.html`
- Modify: `site/app.js`
- Modify: `site/styles.css`
- Modify: `docs/superpowers/plans/2026-07-07-issue-366-checked-benchmark-site-results.md`

**Interfaces:**
- Consumes: `site/benchmark-site.json` schema version 1.
- Consumes: checked evidence item ids `surface-decoder-full` and `bb-circuit-full`.
- Produces: optional evidence item field `caveats: list[str]`.
- Produces: browser function `renderCheckedBenchmarkResults(manifest: object)`.
- Produces: built-site validator behavior that rejects unlisted checked artifact references.
- Produces: Rust contract test `checked_benchmark_artifacts_are_linked`.

- [ ] **Step 1: Write the failing Rust site contract test**

Add this helper after `assert_contains_all_case_insensitive` in `rstim/tests/site_contract.rs`:

```rust
fn find_evidence_item<'a>(manifest: &'a Value, item_id: &str) -> (&'a Value, &'a Value) {
    let families = manifest["families"]
        .as_array()
        .expect("manifest families must be an array");
    for family in families {
        let items = family["evidence_items"]
            .as_array()
            .expect("family evidence_items must be an array");
        for item in items {
            if item["id"].as_str() == Some(item_id) {
                return (family, item);
            }
        }
    }
    panic!("missing evidence item {item_id}");
}

fn assert_checked_artifacts(item: &Value, expected: &[(&str, &str)]) {
    let artifacts = item["artifacts"]
        .as_array()
        .expect("evidence item artifacts must be an array");
    for (path, kind) in expected {
        let artifact = artifacts
            .iter()
            .find(|artifact| artifact["path"].as_str() == Some(*path))
            .unwrap_or_else(|| panic!("missing checked artifact {path}"));
        assert_eq!(
            artifact["kind"].as_str(),
            Some(*kind),
            "artifact {path} must have kind {kind}"
        );
        assert_eq!(
            artifact["checked"].as_bool(),
            Some(true),
            "artifact {path} must be checked"
        );
        assert_repo_file_exists(path);
    }
}

fn assert_item_has_text_list_marker(item: &Value, field: &str, marker: &str) {
    let values = item[field]
        .as_array()
        .unwrap_or_else(|| panic!("evidence item field {field} must be an array"));
    assert!(
        values
            .iter()
            .filter_map(Value::as_str)
            .any(|value| value.contains(marker)),
        "evidence item field {field} is missing marker {marker}"
    );
}
```

Add this test after `benchmark_methodology_lists_required_provenance`:

```rust
#[test]
fn checked_benchmark_artifacts_are_linked() {
    let index = read_repo_file("site/index.html");
    let app = read_repo_file("site/app.js");
    let manifest_text = read_repo_file("site/benchmark-site.json");
    let manifest: Value = serde_json::from_str(&manifest_text)
        .expect("site benchmark manifest must be valid JSON");

    assert_contains_all(
        &index,
        &[
            "id=\"checked-benchmark-results\"",
            "id=\"checked-benchmark-result-cards\"",
            "data-checked-items=\"surface-decoder-full bb-circuit-full\"",
            "Checked Benchmark Results",
        ],
        "checked benchmark result section",
    );

    assert_contains_all(
        &app,
        &[
            "const checkedBenchmarkItems",
            "renderCheckedBenchmarkResults",
            "findEvidenceItem",
            "item.artifacts",
            "artifact.checked",
            "artifact.kind === \"image\"",
            "item.commands",
            "item.caveats",
            "renderArtifactLinks",
            "renderCommandList",
            "renderTextList",
        ],
        "checked benchmark result renderer",
    );

    for hardcoded_path in [
        "benchmarks/surface_decoder_compare/results/full/results.csv",
        "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png",
        "benchmarks/bb_circuit_bposd_compare/results/full/results.csv",
        "benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png",
        "benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md",
    ] {
        assert!(
            !index.contains(hardcoded_path),
            "checked artifact path {hardcoded_path} must come from the manifest, not index.html"
        );
        assert!(
            !app.contains(hardcoded_path),
            "checked artifact path {hardcoded_path} must come from the manifest, not app.js"
        );
    }

    let (surface_family, surface_item) = find_evidence_item(&manifest, "surface-decoder-full");
    assert_eq!(surface_family["status"].as_str(), Some("existing"));
    assert_eq!(surface_item["status"].as_str(), Some("existing"));
    assert_eq!(surface_item["tier"].as_str(), Some("full"));
    assert_checked_artifacts(
        surface_item,
        &[
            (
                "benchmarks/surface_decoder_compare/results/full/results.csv",
                "csv",
            ),
            (
                "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png",
                "image",
            ),
        ],
    );
    assert_item_has_text_list_marker(surface_item, "commands", "make surface-decoder-compare-full");
    assert_item_has_text_list_marker(surface_item, "caveats", "committed run");
    assert!(
        surface_item["claims_limit"]
            .as_str()
            .is_some_and(|value| value.contains("committed-run evidence")),
        "surface checked item must keep its manifest claims limit"
    );

    let (bb_family, bb_item) = find_evidence_item(&manifest, "bb-circuit-full");
    assert_eq!(bb_family["status"].as_str(), Some("partial"));
    assert_eq!(bb_item["status"].as_str(), Some("existing"));
    assert_eq!(bb_item["tier"].as_str(), Some("full"));
    assert_checked_artifacts(
        bb_item,
        &[
            (
                "benchmarks/bb_circuit_bposd_compare/results/full/results.csv",
                "csv",
            ),
            (
                "benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png",
                "image",
            ),
            (
                "benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md",
                "report",
            ),
        ],
    );
    assert_item_has_text_list_marker(bb_item, "commands", "make bb-circuit-bposd-compare-full");
    assert_item_has_text_list_marker(
        bb_item,
        "caveats",
        "batched, error-budget-stopped paired comparison rows",
    );
    assert_item_has_text_list_marker(
        bb_item,
        "caveats",
        "not a fixed-shot reproduction",
    );
    assert!(
        bb_item["claims_limit"]
            .as_str()
            .is_some_and(|value| value.contains("reference-gap report only")),
        "BB checked item must keep its manifest claims limit"
    );
}
```

- [ ] **Step 2: Write the failing Python validator tests**

In `tools/test_check_site_manifest.py`, update the `_site/index.html` fixture text in `write_fixture_manifest` so it includes the checked-results section:

```python
(root / "_site/index.html").write_text(
    '<section id="benchmarks">Benchmark Methodology Claims Policy<div id="benchmark-manifest"></div></section>\n'
    '<section id="checked-benchmark-results"><div id="checked-benchmark-result-cards" '
    'data-checked-items="surface-decoder-full bb-circuit-full"></div></section>\n',
    encoding="utf-8",
)
```

Update the `_site/app.js` fixture text so it includes the new renderer markers:

```python
(root / "_site/app.js").write_text(
    'fetch("data/benchmark-site.json"); renderBenchmarkManifest(manifest); '
    'renderCheckedBenchmarkResults(manifest); checkedBenchmarkItems; '
    'family.status; family.claims_limit; item.status; item.claims_limit; '
    'item.artifacts; item.commands; item.caveats; artifact.checked; artifact.kind === "image";\n',
    encoding="utf-8",
)
```

Add these tests before `test_rejects_built_site_without_manifest_status_wiring`:

```python
def test_rejects_built_site_without_checked_result_wiring(self) -> None:
    repo, _, built_manifest_path = self.write_fixture_manifest()
    (repo / "_site/app.js").write_text(
        'fetch("data/benchmark-site.json"); renderBenchmarkManifest(manifest); '
        'family.status; family.claims_limit; item.status; item.claims_limit;\n',
        encoding="utf-8",
    )
    errors = check_site_manifest.validate_site_root(repo / "_site", built_manifest_path)
    self.assertTrue(any("checked result" in error for error in errors), errors)

def test_rejects_built_site_artifact_reference_not_listed_in_manifest(self) -> None:
    repo, _, built_manifest_path = self.write_fixture_manifest()
    index = repo / "_site/index.html"
    index.write_text(
        index.read_text(encoding="utf-8")
        + '<a href="benchmarks/surface_decoder_compare/results/full/not-in-manifest.csv">bad</a>\n',
        encoding="utf-8",
    )
    errors = check_site_manifest.validate_site_root(repo / "_site", built_manifest_path)
    self.assertTrue(
        any("not listed as a checked manifest artifact" in error for error in errors),
        errors,
    )
```

- [ ] **Step 3: Run tests to verify RED**

Run:

```sh
cargo test -p rstim --test site_contract checked_benchmark_artifacts_are_linked -q
python3 -m unittest tools.test_check_site_manifest -q
```

Expected: the Rust test fails because the checked-results section and renderer do not exist yet. The Python test fails because built-site validation does not yet require checked-result renderer markers or reject unlisted checked artifact references.

- [ ] **Step 4: Add manifest caveats to checked evidence items**

In `site/benchmark-site.json`, add this field to evidence item `surface-decoder-full` after `claims_limit`:

```json
"caveats": [
  "The checked-in surface-decoder full-tier artifacts are evidence for the committed run, not a promise about current local machine speed or a general decoder ordering.",
  "Surface-decoder smoke commands are local implementation checks and are not a replacement for the full comparison campaign."
]
```

Add this field to evidence item `bb-circuit-full` after `claims_limit`:

```json
"caveats": [
  "The BB72/BB144 full rows are batched, error-budget-stopped paired comparison rows.",
  "They are not a fixed-shot reproduction of the pinned Bravyi reference curve; the reference-gap report records that interpretation boundary."
]
```

- [ ] **Step 5: Add the checked-results shell to the site HTML**

In `site/index.html`, add a primary nav link after Evidence:

```html
<a href="#checked-benchmark-results">Results</a>
```

Add this section after `</section>` for `id="benchmark-evidence"` and before `id="benchmarks"`:

```html
      <section id="checked-benchmark-results" class="checked-results-section" aria-labelledby="checked-benchmark-results-title">
        <div class="section-heading">
          <p class="eyebrow">Checked result artifacts</p>
          <h2 id="checked-benchmark-results-title">Checked Benchmark Results</h2>
          <p class="section-copy">
            These cards are rendered from the benchmark manifest so artifact
            links, commands, status, claims limits, and caveats stay tied to the
            checked evidence inventory.
          </p>
        </div>
        <div
          id="checked-benchmark-result-cards"
          class="checked-results-grid"
          data-checked-items="surface-decoder-full bb-circuit-full"
          aria-live="polite"
        >
          <p>Loading checked benchmark results.</p>
        </div>
      </section>
```

- [ ] **Step 6: Render checked result cards from the manifest**

In `site/app.js`, add these constants after `const benchmarkManifest = document.getElementById("benchmark-manifest");`:

```js
const checkedBenchmarkResults = document.getElementById("checked-benchmark-result-cards");
const checkedBenchmarkItems = ["surface-decoder-full", "bb-circuit-full"];
```

Add these helper functions after `renderBenchmarkManifest`:

```js
function findEvidenceItem(manifest, itemId) {
  const families = Array.isArray(manifest.families) ? manifest.families : [];
  for (const family of families) {
    const items = Array.isArray(family.evidence_items) ? family.evidence_items : [];
    const item = items.find((candidate) => candidate && candidate.id === itemId);
    if (item) {
      return { family, item };
    }
  }
  return null;
}

function fileName(path) {
  return String(path || "").split("/").pop() || String(path || "artifact");
}

function renderArtifactLinks(item) {
  const artifacts = Array.isArray(item.artifacts) ? item.artifacts : [];
  const checkedArtifacts = artifacts.filter(
    (artifact) => artifact && artifact.checked === true && artifact.path,
  );
  if (!checkedArtifacts.length) {
    return "<p>No checked artifacts are listed for this item.</p>";
  }
  const links = checkedArtifacts
    .map(
      (artifact) => `
        <li>
          <a href="${escapeHtml(artifact.path)}">${escapeHtml(fileName(artifact.path))}</a>
          <span class="badge">${escapeHtml(artifact.kind || "artifact")}</span>
        </li>
      `,
    )
    .join("");
  return `<ul class="result-link-list">${links}</ul>`;
}

function renderImageArtifacts(item) {
  const artifacts = Array.isArray(item.artifacts) ? item.artifacts : [];
  const images = artifacts.filter(
    (artifact) => artifact && artifact.checked === true && artifact.kind === "image" && artifact.path,
  );
  if (!images.length) {
    return "";
  }
  return images
    .map(
      (image) => `
        <figure class="result-plot">
          <a href="${escapeHtml(image.path)}">
            <img src="${escapeHtml(image.path)}" alt="${escapeHtml(item.title || "Checked benchmark plot")}">
          </a>
        </figure>
      `,
    )
    .join("");
}

function renderCommandList(commands) {
  if (!Array.isArray(commands) || !commands.length) {
    return "<p>No reproduction command is listed.</p>";
  }
  const commandText = commands.map((command) => `$ ${command}`).join("\n");
  return `<pre class="result-commands"><code>${escapeHtml(commandText)}</code></pre>`;
}

function renderTextList(values) {
  if (!Array.isArray(values) || !values.length) {
    return "";
  }
  return `<ul class="result-note-list">${values.map((value) => `<li>${escapeHtml(value)}</li>`).join("")}</ul>`;
}

function renderSourceLinks(paths) {
  if (!Array.isArray(paths) || !paths.length) {
    return "";
  }
  const links = paths
    .map((path) => `<li><a href="${escapeHtml(path)}">${escapeHtml(path)}</a></li>`)
    .join("");
  return `<ul class="result-link-list source-links">${links}</ul>`;
}

function renderCheckedBenchmarkResults(manifest) {
  if (!checkedBenchmarkResults) {
    return;
  }
  checkedBenchmarkResults.innerHTML = checkedBenchmarkItems
    .map((itemId) => {
      const found = findEvidenceItem(manifest, itemId);
      if (!found) {
        return `<article class="result-card error"><h3>${escapeHtml(itemId)}</h3><p>Missing checked benchmark manifest item.</p></article>`;
      }
      const { family, item } = found;
      return `
        <article class="result-card">
          <div class="result-card-copy">
            <div class="manifest-heading">
              <div>
                <p class="eyebrow">${escapeHtml(family.title || family.id || "Benchmark family")}</p>
                <h3>${escapeHtml(item.title || item.id || "Checked benchmark result")}</h3>
              </div>
              <div class="schema-meta">
                ${renderBadge("family", family.status)}
                ${renderBadge("status", item.status)}
                ${renderBadge("tier", item.tier)}
              </div>
            </div>
            <p><strong>Claims limit:</strong> ${escapeHtml(item.claims_limit || family.claims_limit || "No claims limit recorded.")}</p>
            <h4>Artifacts</h4>
            ${renderArtifactLinks(item)}
            <h4>Reproduction</h4>
            ${renderCommandList(item.commands)}
            <h4>Caveats</h4>
            ${renderTextList(item.caveats)}
            <h4>Provenance Sources</h4>
            ${renderSourceLinks(item.provenance_sources || family.source_docs)}
          </div>
          <div class="result-card-plot">
            ${renderImageArtifacts(item)}
          </div>
        </article>
      `;
    })
    .join("");
}
```

Change the manifest fetch guard from `if (benchmarkManifest) {` to:

```js
if (benchmarkManifest || checkedBenchmarkResults) {
```

Then call both renderers in the success callback:

```js
renderBenchmarkManifest(manifest);
renderCheckedBenchmarkResults(manifest);
```

In the catch block, also set `checkedBenchmarkResults.innerHTML` when the checked-results container exists.

- [ ] **Step 7: Style the checked result cards**

In `site/styles.css`, add `.checked-results-section` to the existing flex-column section list:

```css
.checked-results-section,
```

Add `.checked-results-grid` to the grid display selector:

```css
.checked-results-grid,
```

Add these rules near the manifest styles:

```css
.checked-results-grid {
  grid-template-columns: 1fr;
}

.result-card {
  display: grid;
  grid-template-columns: minmax(280px, 1fr) minmax(320px, 0.9fr);
  gap: 1rem;
  align-items: start;
  padding: 1.25rem;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface);
  box-shadow: var(--shadow);
}

.result-card-copy,
.result-card-plot {
  min-width: 0;
}

.result-card h4 {
  margin: 1rem 0 0.45rem;
}

.result-card p,
.result-note-list {
  color: var(--muted);
}

.result-link-list,
.result-note-list {
  display: grid;
  gap: 0.45rem;
  margin: 0;
  padding-left: 1.1rem;
}

.result-link-list a {
  font-weight: 700;
  text-decoration: none;
}

.result-commands {
  border-radius: 8px;
}

.result-plot {
  margin: 0;
  overflow: auto;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: #fbfcfd;
  padding: 0.8rem;
}

.result-plot img {
  display: block;
  width: 100%;
  min-width: 360px;
  height: auto;
}
```

Add `.checked-results-grid` and `.result-card` to the responsive one-column rules:

```css
.checked-results-grid,
```

and:

```css
.result-card {
  grid-template-columns: 1fr;
}
```

- [ ] **Step 8: Extend the built-site manifest checker**

In `tools/check_site_manifest.py`, add `import re` with the imports.

Add this constant near `ITEM_REQUIRED_FIELDS`:

```python
CHECKED_ARTIFACT_REFERENCE_RE = re.compile(
    r"benchmarks/(?:surface_decoder_compare|bb_circuit_bposd_compare)/results/full/[A-Za-z0-9._/-]+"
)
```

In `validate_item`, after `validate_string_list` for `provenance_requirements`, add:

```python
if "caveats" in item:
    validate_string_list(scope, "caveats", item.get("caveats"), errors, allow_empty=False)
```

Add this helper before `validate_site_root`:

```python
def validate_site_artifact_references(site_root: Path, manifest: dict[str, Any], errors: list[str]) -> None:
    checked_paths = {artifact_path for _, artifact_path in iter_checked_artifact_paths(manifest)}
    for relative in ("index.html", "app.js"):
        path = site_root / relative
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for match in sorted(set(CHECKED_ARTIFACT_REFERENCE_RE.findall(text))):
            if match not in checked_paths:
                add_error(
                    errors,
                    "site root",
                    f"{relative} references checked artifact {match} that is not listed as a checked manifest artifact",
                )
```

In `validate_site_root`, add these required index markers:

```python
for marker in [
    'id="benchmarks"',
    'id="benchmark-manifest"',
    'id="checked-benchmark-results"',
    'id="checked-benchmark-result-cards"',
    "Benchmark Methodology",
    "Claims Policy",
]:
```

Replace the single `required_app_markers` check with two lists:

```python
required_app_markers = [
    'fetch("data/benchmark-site.json")',
    "renderBenchmarkManifest",
    "family.status",
    "family.claims_limit",
    "item.status",
    "item.claims_limit",
]
missing_app_markers = [marker for marker in required_app_markers if marker not in app]
if missing_app_markers:
    add_error(errors, scope, f"app.js missing manifest status and claims_limit wiring: {missing_app_markers}")

checked_result_markers = [
    "checkedBenchmarkItems",
    "renderCheckedBenchmarkResults",
    "item.artifacts",
    "item.commands",
    "item.caveats",
    "artifact.checked",
    'artifact.kind === "image"',
]
missing_checked_markers = [marker for marker in checked_result_markers if marker not in app]
if missing_checked_markers:
    add_error(errors, scope, f"app.js missing checked result rendering: {missing_checked_markers}")
```

At the end of `validate_site_root`, load the manifest when possible and call the artifact-reference validator:

```python
try:
    manifest = load_json(manifest_path)
except (FileNotFoundError, json.JSONDecodeError):
    manifest = None
if isinstance(manifest, dict):
    validate_site_artifact_references(site_root, manifest, errors)
```

- [ ] **Step 9: Run focused tests to verify GREEN**

Run:

```sh
cargo test -p rstim --test site_contract checked_benchmark_artifacts_are_linked -q
python3 -m unittest tools.test_check_site_manifest -q
```

Expected: both commands exit 0.

- [ ] **Step 10: Build the site and run issue verification**

Run:

```sh
make build-site
python3 -m unittest benchmarks.surface_decoder_compare.tests.test_docs_contract -q
python3 -m benchmarks.bb_circuit_bposd_compare.validate_reference_gap_report --results benchmarks/bb_circuit_bposd_compare/results/full/results.csv --report benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md
python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json
cargo test -p rstim --test site_contract checked_benchmark_artifacts_are_linked -q
```

Expected: all commands exit 0. `_site/benchmarks/...` contains the checked CSVs, PNGs, BB summary, and BB reference-gap report copied from manifest entries.

- [ ] **Step 11: Run cargo test**

Run:

```sh
cargo test
```

Expected: exits 0. If it hangs or fails in a pre-existing unrelated test, record the exact last output and still keep the issue-specific verification evidence from Step 10.

- [ ] **Step 12: Commit implementation**

Run:

```sh
git status --short
git add rstim/tests/site_contract.rs tools/check_site_manifest.py tools/test_check_site_manifest.py site/benchmark-site.json site/index.html site/app.js site/styles.css docs/superpowers/plans/2026-07-07-issue-366-checked-benchmark-site-results.md
git commit -m "feat: publish checked benchmark site results"
```

Expected: one implementation commit after the plan commit.

## Self Review

- Spec coverage: one task covers manifest caveats, site result cards, artifact links/images, reproduction commands, status/claims-limit rendering, caveats, validator negative controls, and required verification.
- Placeholder scan: no TODO/TBD placeholders.
- Type consistency: the produced names `renderCheckedBenchmarkResults`, `checkedBenchmarkItems`, and `checked_benchmark_artifacts_are_linked` are used consistently across tasks and tests.
