# Issue 365 Benchmark Methodology And Claims Policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add site-facing benchmark methodology and claims-policy content that renders manifest-backed benchmark statuses and claims limits.

**Architecture:** The existing static site remains the only site surface. `site/index.html` carries the methodology prose, `site/app.js` renders the benchmark status inventory from `data/benchmark-site.json`, `Makefile` copies the manifest into `_site/data/`, and `tools/check_site_manifest.py` validates the built-site wiring when `--site-root` is provided.

**Tech Stack:** Static HTML, CSS, browser JavaScript, Makefile, Python standard library, Rust integration tests with `serde_json`.

## Global Constraints

- Do not change benchmark runner schemas.
- Do not run or regenerate benchmark campaigns.
- Do not commit generated benchmark outputs under `benchmarks/out/`.
- Keep the existing QP101 schema browser resources and markers intact.
- The site must name benchmark tiers `smoke`, `full`, `extended`, and `reference reproduction`.
- The methodology must list OS, CPU, Rust version, Python version, dependency versions, external repository commits, command line, seeds, build profile, shots or error budgets, and date.
- The policy must explain publishable evidence versus local-only evidence.
- The policy must explain the distinction between `smoke` and `full`.
- Benchmark family and evidence item status and `claims_limit` text must come from `site/benchmark-site.json`, not from hand-authored duplicate status text.
- `make build-site` must produce `_site/data/benchmark-site.json`.
- `python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json` must exit 0 after `make build-site`.
- Focused test name: `benchmark_methodology_lists_required_provenance`.

---

### Task 1: Add Benchmark Methodology, Manifest Rendering, And Contracts

**Files:**
- Modify: `Makefile`
- Modify: `rstim/tests/site_contract.rs`
- Modify: `site/index.html`
- Modify: `site/styles.css`
- Modify: `site/app.js`
- Modify: `tools/check_site_manifest.py`
- Modify: `tools/test_check_site_manifest.py`
- Modify: `docs/superpowers/plans/2026-07-07-issue-365-benchmark-methodology-claims-policy.md`

**Interfaces:**
- Consumes: `site/benchmark-site.json` schema version 1 with top-level `families`.
- Consumes: existing `validate_manifest(repo_root: Path, manifest_path: Path) -> list[str]`.
- Produces: optional `validate_site_root(site_root: Path, manifest_path: Path) -> list[str]`.
- Produces: CLI option `--site-root PATH`.
- Produces: built file `_site/data/benchmark-site.json`.
- Produces: browser function `renderBenchmarkManifest(manifest: object)`.
- Produces: Rust test `benchmark_methodology_lists_required_provenance`.

- [x] **Step 1: Write the failing Rust site contract test**

Add these imports and helper structs near the top of `rstim/tests/site_contract.rs`:

```rust
use serde_json::Value;
```

Add this test after `qp101_browser_resources_are_preserved`:

```rust
#[test]
fn benchmark_methodology_lists_required_provenance() {
    let index = read_repo_file("site/index.html");
    let app = read_repo_file("site/app.js");
    let manifest_text = read_repo_file("site/benchmark-site.json");
    let manifest: Value = serde_json::from_str(&manifest_text)
        .expect("site benchmark manifest must be valid JSON");

    for marker in [
        "id=\"benchmarks\"",
        "Benchmark Methodology",
        "Claims Policy",
        "smoke",
        "full",
        "extended",
        "reference reproduction",
        "Publishable Evidence",
        "Local-Only Evidence",
        "smoke checks verify wiring",
        "full evidence can describe the committed checked run",
    ] {
        assert!(index.contains(marker), "benchmark methodology is missing marker {marker}");
    }

    for field in [
        "OS",
        "CPU",
        "Rust version",
        "Python version",
        "dependency versions",
        "external repository commits",
        "command line",
        "seeds",
        "build profile",
        "shots or error budgets",
        "date",
    ] {
        assert!(index.contains(field), "benchmark methodology is missing provenance field {field}");
    }

    for marker in [
        "id=\"benchmark-manifest\"",
        "fetch(\"data/benchmark-site.json\")",
        "renderBenchmarkManifest",
        "family.status",
        "family.claims_limit",
        "item.status",
        "item.claims_limit",
    ] {
        let source = if marker.starts_with("id=") { &index } else { &app };
        assert!(source.contains(marker), "manifest-backed benchmark rendering is missing marker {marker}");
    }

    let families = manifest["families"]
        .as_array()
        .expect("manifest families must be an array");
    assert!(!families.is_empty(), "manifest must list benchmark families");
    for family in families {
        assert!(family["status"].as_str().is_some(), "family is missing status: {family:?}");
        assert!(
            family["claims_limit"].as_str().is_some(),
            "family is missing claims_limit: {family:?}"
        );
        let items = family["evidence_items"]
            .as_array()
            .expect("family evidence_items must be an array");
        assert!(!items.is_empty(), "family must list evidence items: {family:?}");
        for item in items {
            assert!(item["status"].as_str().is_some(), "item is missing status: {item:?}");
            assert!(
                item["claims_limit"].as_str().is_some(),
                "item is missing claims_limit: {item:?}"
            );
        }
    }
}
```

- [x] **Step 2: Write the failing Python validator tests**

In `tools/test_check_site_manifest.py`, extend the temporary fixture setup to create built-site files:

```python
(root / "_site/data").mkdir(parents=True)
(root / "_site/index.html").write_text(
    '<section id="benchmarks"><div id="benchmark-manifest"></div></section>\n',
    encoding="utf-8",
)
(root / "_site/app.js").write_text(
    'fetch("data/benchmark-site.json"); family.status; family.claims_limit; item.status; item.claims_limit;\n',
    encoding="utf-8",
)
```

After writing `site/benchmark-site.json`, also write the built copy:

```python
built_manifest_path = root / "_site/data/benchmark-site.json"
built_manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
```

Return `root, manifest_path, built_manifest_path` from `write_fixture_manifest`, then update existing callers to ignore the third value:

```python
repo, manifest_path, _ = self.write_fixture_manifest()
```

Add these test methods:

```python
def test_accepts_built_site_manifest_when_site_root_is_wired(self) -> None:
    repo, _, built_manifest_path = self.write_fixture_manifest()
    errors = check_site_manifest.validate_manifest(repo, built_manifest_path)
    errors.extend(check_site_manifest.validate_site_root(repo / "_site", built_manifest_path))
    self.assertEqual(errors, [])

def test_rejects_built_site_without_manifest_status_wiring(self) -> None:
    repo, _, built_manifest_path = self.write_fixture_manifest()
    (repo / "_site/app.js").write_text('fetch("data/benchmark-site.json");\n', encoding="utf-8")
    errors = check_site_manifest.validate_site_root(repo / "_site", built_manifest_path)
    self.assertTrue(any("status" in error and "claims_limit" in error for error in errors), errors)
```

- [x] **Step 3: Run tests to verify RED**

Run:

```sh
cargo test -p rstim --test site_contract benchmark_methodology_lists_required_provenance -q
python3 -m unittest tools.test_check_site_manifest -q
```

Expected: the Rust test fails because `site/index.html` has no `#benchmarks`
methodology section and `site/app.js` does not fetch `data/benchmark-site.json`.
The Python test fails because `validate_site_root` does not exist.

- [x] **Step 4: Add built manifest copy to the site build**

In `Makefile`, update the `build-site` target so it creates `_site/data` and copies the manifest:

```make
build-site:
	rm -rf _site
	mkdir -p _site/examples _site/gallery _site/data
	cp site/index.html site/styles.css site/app.js _site/
	cp site/benchmark-site.json _site/data/benchmark-site.json
```

Keep the existing QP101 copies and gallery command after these lines.

- [x] **Step 5: Add the benchmark methodology section**

In `site/index.html`, add `<a href="#benchmarks">Benchmarks</a>` to `.nav-links`.

Add this section between `docs-home` and `qp101`:

```html
      <section id="benchmarks" class="benchmark-section" aria-labelledby="benchmark-title">
        <div class="section-heading">
          <p class="eyebrow">Benchmark evidence</p>
          <h2 id="benchmark-title">Benchmark Methodology</h2>
          <p class="section-copy">
            Benchmark pages separate smoke checks, checked full artifacts,
            extended campaigns, and reference reproduction work so the site
            does not imply broader performance claims than the recorded
            evidence supports.
          </p>
        </div>

        <section class="tier-grid" aria-label="Benchmark tiers">
          <article>
            <h3><code>smoke</code></h3>
            <p>
              Local wiring checks with small cases. smoke checks verify wiring,
              command compatibility, and artifact shape; they are not
              statistical evidence.
            </p>
          </article>
          <article>
            <h3><code>full</code></h3>
            <p>
              Checked artifact campaigns with recorded provenance. full
              evidence can describe the committed checked run only within its
              recorded commands, hardware, versions, seeds, and budgets.
            </p>
          </article>
          <article>
            <h3><code>extended</code></h3>
            <p>
              Longer local or future campaigns that broaden case coverage but
              remain non-publishable until their artifacts and provenance are
              checked into the site manifest.
            </p>
          </article>
          <article>
            <h3><code>reference reproduction</code></h3>
            <p>
              Source-backed reproduction against an external paper, dataset,
              or repository commit. The reproduced scope and mismatches must be
              stated next to the result.
            </p>
          </article>
        </section>

        <section class="provenance-panel" aria-labelledby="provenance-title">
          <div>
            <p class="eyebrow">Required provenance</p>
            <h3 id="provenance-title">Every benchmark claim needs these fields</h3>
          </div>
          <ul class="provenance-list">
            <li>OS</li>
            <li>CPU</li>
            <li>Rust version</li>
            <li>Python version</li>
            <li>dependency versions</li>
            <li>external repository commits</li>
            <li>command line</li>
            <li>seeds</li>
            <li>build profile</li>
            <li>shots or error budgets</li>
            <li>date</li>
          </ul>
        </section>

        <section class="evidence-policy-grid" aria-label="Evidence policy">
          <article>
            <h3>Publishable Evidence</h3>
            <p>
              Publishable evidence is tracked by git, listed in the manifest,
              tied to source documents, and bounded by a manifest
              <code>claims_limit</code>. It may support statements about the
              specific committed artifact set, not broad decoder rankings.
            </p>
          </article>
          <article>
            <h3>Local-Only Evidence</h3>
            <p>
              Local-only evidence can validate commands and readiness, but
              ignored generated outputs and workstation-specific timings are
              not site-facing benchmark claims.
            </p>
          </article>
          <article>
            <h3>Claims Policy</h3>
            <p>
              Broad performance claims require full provenance, checked
              artifacts, source-backed interpretation, and an explicit
              manifest claims limit. Future benchmark families cannot appear
              without status and claims-limit fields.
            </p>
          </article>
        </section>

        <section class="manifest-panel" aria-labelledby="manifest-title">
          <div class="section-heading">
            <p class="eyebrow">Manifest-backed inventory</p>
            <h3 id="manifest-title">Benchmark Status And Claims Limits</h3>
          </div>
          <div id="benchmark-manifest" class="benchmark-manifest" aria-live="polite">
            <p>Loading benchmark manifest.</p>
          </div>
        </section>
      </section>
```

- [x] **Step 6: Style the benchmark section**

In `site/styles.css`, add `.tier-grid` to the existing grid selector:

```css
.docs-grid,
.intro-grid,
.example-grid,
.tier-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 1rem;
}
```

Then add benchmark-specific styles:

```css
.benchmark-section {
  display: flex;
  flex-direction: column;
}

.tier-grid {
  grid-template-columns: repeat(4, minmax(0, 1fr));
}

.tier-grid article,
.evidence-policy-grid article,
.manifest-family {
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface);
  box-shadow: var(--shadow);
  padding: 1.25rem;
}

.tier-grid p,
.evidence-policy-grid p,
.manifest-family p,
.manifest-item p {
  color: var(--muted);
}

.provenance-panel {
  display: grid;
  grid-template-columns: minmax(220px, 320px) minmax(0, 1fr);
  gap: 1rem;
  align-items: start;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface);
  box-shadow: var(--shadow);
  padding: 1.25rem;
}

.provenance-list,
.manifest-list {
  display: flex;
  flex-wrap: wrap;
  gap: 0.55rem;
  margin: 0;
  padding: 0;
  list-style: none;
}

.provenance-list li,
.manifest-list li {
  padding: 0.35rem 0.55rem;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: #fbfcfd;
}

.evidence-policy-grid,
.benchmark-manifest {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 1rem;
}

.manifest-family {
  min-width: 0;
}

.manifest-heading {
  display: flex;
  justify-content: space-between;
  gap: 0.75rem;
  align-items: start;
}

.manifest-items {
  display: grid;
  gap: 0.75rem;
  margin-top: 1rem;
}

.manifest-item {
  padding-top: 0.75rem;
  border-top: 1px solid var(--border);
}
```

In the `@media (max-width: 820px)` block, add `.tier-grid`, `.evidence-policy-grid`, `.benchmark-manifest`, and `.provenance-panel` to the one-column grid list.

- [x] **Step 7: Render manifest status and claims-limit fields in JavaScript**

In `site/app.js`, keep the existing QP101 schema browser code intact and add these helpers before the final `})();`:

```javascript
  const benchmarkManifest = document.getElementById("benchmark-manifest");

  function renderBadge(label, value) {
    return `<span class="badge">${escapeHtml(label)}: ${escapeHtml(value || "unspecified")}</span>`;
  }

  function renderBenchmarkManifest(manifest) {
    if (!benchmarkManifest) {
      return;
    }
    const families = Array.isArray(manifest.families) ? manifest.families : [];
    if (!families.length) {
      benchmarkManifest.innerHTML = "<p>No benchmark families are listed.</p>";
      return;
    }
    benchmarkManifest.innerHTML = families
      .map((family) => {
        const items = Array.isArray(family.evidence_items) ? family.evidence_items : [];
        const itemHtml = items
          .map((item) => `
            <article class="manifest-item">
              <div class="manifest-heading">
                <h4>${escapeHtml(item.title || item.id || "Evidence item")}</h4>
                <div class="schema-meta">
                  ${renderBadge("status", item.status)}
                  ${renderBadge("tier", item.tier)}
                </div>
              </div>
              <p><strong>Claims limit:</strong> ${escapeHtml(item.claims_limit || "No claims limit recorded.")}</p>
            </article>
          `)
          .join("");
        return `
          <article class="manifest-family">
            <div class="manifest-heading">
              <h3>${escapeHtml(family.title || family.id || "Benchmark family")}</h3>
              <div class="schema-meta">
                ${renderBadge("status", family.status)}
              </div>
            </div>
            <p><strong>Claims limit:</strong> ${escapeHtml(family.claims_limit || "No claims limit recorded.")}</p>
            <div class="manifest-items">${itemHtml}</div>
          </article>
        `;
      })
      .join("");
  }

  if (benchmarkManifest) {
    fetch("data/benchmark-site.json")
      .then((response) => {
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        return response.json();
      })
      .then((manifest) => {
        renderBenchmarkManifest(manifest);
      })
      .catch((error) => {
        benchmarkManifest.classList.add("error");
        benchmarkManifest.innerHTML = `
          <p>Benchmark manifest could not be loaded: ${escapeHtml(error.message)}</p>
          <p><a href="data/benchmark-site.json">Open benchmark-site.json</a></p>
        `;
      });
  }
```

- [x] **Step 8: Add built-site validator support**

In `tools/check_site_manifest.py`, add this function after `validate_manifest`:

```python
def validate_site_root(site_root: Path, manifest_path: Path) -> list[str]:
    errors: list[str] = []
    scope = "site root"
    index_path = site_root / "index.html"
    app_path = site_root / "app.js"
    expected_manifest = site_root / "data/benchmark-site.json"

    for path, label in [
        (index_path, "index.html"),
        (app_path, "app.js"),
        (expected_manifest, "data/benchmark-site.json"),
    ]:
        if not path.is_file():
            add_error(errors, scope, f"missing built site file {label}")

    if manifest_path.resolve() != expected_manifest.resolve():
        add_error(errors, scope, "manifest path must be _site/data/benchmark-site.json when --site-root is used")

    index = index_path.read_text(encoding="utf-8") if index_path.is_file() else ""
    app = app_path.read_text(encoding="utf-8") if app_path.is_file() else ""

    for marker in ['id="benchmarks"', 'id="benchmark-manifest"', "Benchmark Methodology", "Claims Policy"]:
        if marker not in index:
            add_error(errors, scope, f"index.html missing benchmark marker {marker}")

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

    return errors
```

Update `parse_args`:

```python
parser.add_argument("--site-root", type=Path, help="Built site root for status and claims-limit wiring checks")
```

Update `main` after `errors = validate_manifest(args.repo_root, args.manifest)`:

```python
if args.site_root is not None:
    errors.extend(validate_site_root(args.site_root, args.manifest))
```

- [x] **Step 9: Run focused GREEN verification**

Run:

```sh
python3 -m unittest tools.test_check_site_manifest -q
cargo test -p rstim --test site_contract benchmark_methodology_lists_required_provenance -q
make build-site
python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json
```

Expected: all commands exit 0. The validator prints one `ok: family ...` line
for each manifest family.

- [x] **Step 10: Run repository verification**

Run:

```sh
cargo test
git diff --check
```

Expected: both commands exit 0.

- [x] **Step 11: Commit the implementation**

Run:

```sh
git add Makefile rstim/tests/site_contract.rs site/index.html site/styles.css site/app.js tools/check_site_manifest.py tools/test_check_site_manifest.py docs/superpowers/plans/2026-07-07-issue-365-benchmark-methodology-claims-policy.md
git commit -m "docs: add benchmark methodology policy"
```

Expected: commit succeeds with the site policy, manifest renderer, validator, tests, and implementation plan.
