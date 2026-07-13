# Website Redesign (Zola Multi-Page Site) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single-page `site/` with a Zola-built four-page site: a user-facing pitch landing page plus `/guide/`, `/benchmarks/`, and `/qp101/` subpages, keeping all existing content and claims discipline.

**Architecture:** Zola (static site generator, single Rust-ecosystem binary) renders four section pages from custom Tera templates that share one base template (nav + footer). The existing hand-written HTML sections move verbatim into per-page templates; `app.js` splits into two page-scoped scripts. `make build-site` runs `zola build` then the existing Python asset-copy steps into `_site`, and `tools/check_site_build.py` is reworked to validate the multi-page tree.

**Tech Stack:** Zola (Tera templates), plain CSS/JS (no framework), Python 3 checker tools, GitHub Pages via `deploy-pages.yml`.

**Spec:** `docs/superpowers/specs/2026-07-13-website-redesign-design.md`

## Global Constraints

- **Relative URLs only.** The site is served at `https://nzy1997.github.io/rstim/` (a subpath). Never emit root-absolute (`/...`) hrefs/srcs in rendered HTML. Subpages reference root assets with `../`.
- **Claims-policy phrases move verbatim.** These exact strings must appear in the built `benchmarks/index.html` (they are checker-enforced): `Claims Policy`, `Publishable Evidence`, `Local-Only Evidence`, `smoke checks verify wiring`, `full evidence can describe the committed checked run`, `committed-run evidence`, `not a general decoder ordering claim`.
- **No new performance claims.** Landing-page benchmark copy may only restate existing claims-limit wording.
- **Old anchor ids preserved** on the pages their sections move to (`workspace-overview`, `feature-walkthroughs`, `operations`, `benchmark-evidence`, `checked-benchmark-results`, `benchmarks`, `qp101`, `schema-browser`, `gallery`, `examples`).
- **Source line references** in this plan refer to `site/index.html` and `site/app.js` at commit `0af7e38` (unchanged since `47ffef3`). Do not delete these files until Task 5.
- **Zola version pinned.** Task 1 records the locally installed `zola --version`; Task 7 pins the same version in CI. This plan writes `v0.19.2` — substitute the recorded version everywhere if it differs.
- Reuse existing CSS classes from `site/styles.css` (`hero`, `eyebrow`, `section-heading`, `section-copy`, `docs-grid`, `docs-card`, `docs-card-links`, `compact-card`, `actions`, `button`, `panel`, `result-plot`, `table-wrap`, `gallery-grid`, `site-footer`). Do not restyle; visual polish is out of scope.

## File Structure

```
site/
  config.toml                       NEW    Zola config
  benchmark-site.json               KEEP   source manifest (unchanged)
  content/_index.md                 NEW    landing section stub
  content/guide/_index.md           NEW
  content/benchmarks/_index.md      NEW
  content/qp101/_index.md           NEW
  templates/base.html               NEW    shared head/nav/footer
  templates/index.html              NEW    landing page (new content)
  templates/guide.html              NEW    workspace map + walkthroughs + operations
  templates/benchmarks.html         NEW    evidence + results + methodology
  templates/qp101.html              NEW    schema browser + gallery + examples
  static/styles.css                 NEW    copy of site/styles.css
  static/js/qp101-browser.js        NEW    split from app.js
  static/js/benchmarks.js           NEW    split from app.js
  index.html                        DELETE in Task 5
  app.js                            DELETE in Task 5
  styles.css                        DELETE in Task 5
Makefile                            MODIFY build-site target (Task 5)
tools/check_site_build.py           MODIFY multi-page rework (Task 6)
tools/test_check_site_build.py      MODIFY (Task 6)
tools/test_site_app_rendering.py    MODIFY (Task 4)
.github/workflows/deploy-pages.yml  MODIFY Zola install + checker gate (Task 7)
```

Local preview after Task 5: `make build-site && python3 -m http.server 8080 --directory _site` (do not use `zola serve` for final checks — copied assets like `data/benchmark-site.json` only exist in `_site`).

---

### Task 1: Zola toolchain, scaffold, base template, and landing page

**Files:**
- Create: `site/config.toml`, `site/templates/base.html`, `site/templates/index.html`, `site/content/_index.md`, `site/static/styles.css` (copy)

**Interfaces:**
- Produces: `base.html` with Tera blocks `title`, `description`, `hero`, `main`, `scripts`, and a `root` variable read from `section.extra.root` (`"."` on the landing page, `".."` on subpages). `<body data-root="{{ root }}">` is the contract the JS files (Tasks 3–4) use to prefix fetch/asset paths.

- [ ] **Step 1: Install Zola and record the version**

Run: `brew install zola && zola --version`
Expected: prints a version (e.g. `zola 0.19.2`). Record it; it is the pin for CI in Task 7 and for the `## Global Constraints` substitution.

- [ ] **Step 2: Create `site/config.toml`**

```toml
base_url = "https://nzy1997.github.io/rstim"
title = "rstim"
compile_sass = false
build_search_index = false

[markdown]
highlight_code = false
```

- [ ] **Step 3: Copy the stylesheet into Zola's static tree**

Run: `mkdir -p site/static/js && cp site/styles.css site/static/styles.css`
(Copy, not move — the old build must keep working until Task 5.)

- [ ] **Step 4: Create `site/templates/base.html`**

```html
{% set root = section.extra.root | default(value=".") %}<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{% block title %}rstim{% endblock title %}</title>
    <meta
      name="description"
      content="{% block description %}rstim — a quantum error correction workspace in Rust.{% endblock description %}"
    >
    <link rel="stylesheet" href="{{ root }}/styles.css">
  </head>
  <body data-root="{{ root }}">
    <header class="site-header">
      <nav class="top-nav" aria-label="Primary">
        <a class="brand" href="{{ root }}/" aria-label="rstim home">
          <span class="brand-mark">r</span>
          <span>rstim</span>
        </a>
        <div class="nav-links">
          <a href="{{ root }}/">Home</a>
          <a href="{{ root }}/guide/">Guide</a>
          <a href="{{ root }}/benchmarks/">Benchmarks</a>
          <a href="{{ root }}/qp101/">QP101</a>
        </div>
      </nav>
      {% block hero %}{% endblock hero %}
    </header>

    <main>
    {% block main %}{% endblock main %}
    </main>

    <footer class="site-footer">
      <span>rstim — a quantum error correction workspace in Rust</span>
      <a href="https://github.com/nzy1997/rstim">Repository</a>
    </footer>
    {% block scripts %}{% endblock scripts %}
  </body>
</html>
```

- [ ] **Step 5: Create `site/content/_index.md`**

```markdown
+++
title = "rstim"
template = "index.html"

[extra]
root = "."
+++
```

- [ ] **Step 6: Create `site/templates/index.html`** (the landing page — all new content)

```html
{% extends "base.html" %}

{% block title %}rstim — a quantum error correction workspace in Rust{% endblock title %}
{% block description %}rstim combines a Stim-style circuit simulator, MWPM/BP-OSD/ILP decoders, and a parallel benchmark harness in one Rust workspace.{% endblock description %}

{% block hero %}
<div class="hero">
  <p class="eyebrow">Rust QEC workspace</p>
  <h1>A complete quantum error correction workspace in Rust</h1>
  <p class="hero-copy">
    <code>rstim</code> combines a Stim-style circuit simulator, MWPM, BP-OSD,
    and ILP decoders, and a parallel benchmark harness in one Cargo
    workspace — one <code>cargo build</code> away.
  </p>
  <div class="actions" aria-label="Getting started">
    <a class="button primary" href="#quick-start">Get started</a>
    <a class="button" href="benchmarks/">See the benchmarks</a>
  </div>
</div>
{% endblock hero %}

{% block main %}
<section id="capabilities" class="docs-home" aria-label="What is in the box">
  <div class="section-heading">
    <p class="eyebrow">What's in the box</p>
    <h2>One workspace, the whole decoder-experiment loop</h2>
    <p class="section-copy">
      Simulate circuits, extract detector error models, decode with three
      decoder families, and benchmark the results — without leaving the
      workspace.
    </p>
  </div>
  <div class="docs-grid">
    <article class="docs-card">
      <h3>Simulate and sample circuits</h3>
      <p>
        Parse Stim-format circuits, inspect structure with
        <code>rstim stats</code>, and sample measurement or detector shots.
      </p>
      <div class="docs-card-links">
        <a href="guide/#feature-walkthroughs">Walkthroughs</a>
      </div>
    </article>
    <article class="docs-card">
      <h3>Extract detector error models</h3>
      <p>
        Derive DEMs with <code>rstim analyze_errors</code> and sample them
        directly with <code>rstim sample_dem</code>.
      </p>
      <div class="docs-card-links">
        <a href="guide/#feature-walkthroughs">DEM pipeline</a>
      </div>
    </article>
    <article class="docs-card">
      <h3>Decode with three decoder families</h3>
      <p>
        MWPM (<code>rmatching</code>), BP-OSD (<code>rbposd</code>), and ILP
        (<code>rilpqec</code>) decoders share one workspace and harness.
      </p>
      <div class="docs-card-links">
        <a href="guide/#workspace-overview">Workspace map</a>
      </div>
    </article>
    <article class="docs-card">
      <h3>Run benchmark campaigns</h3>
      <p>
        <code>rsinter</code> orchestrates decoder experiments and keeps
        checked artifacts tied to recorded provenance and claims limits.
      </p>
      <div class="docs-card-links">
        <a href="benchmarks/">Benchmarks</a>
      </div>
    </article>
    <article class="docs-card">
      <h3>Render circuit diagrams</h3>
      <p>
        Render SVG diagrams with <code>rstim render_svg</code>, including
        seeded atom-loss sample-shot overlays, and export QP101 JSON.
      </p>
      <div class="docs-card-links">
        <a href="qp101/#gallery">Gallery</a>
        <a href="qp101/">QP101 format</a>
      </div>
    </article>
    <article class="docs-card">
      <h3>Construct CSS codes</h3>
      <p>
        Build CSS code matrices with <code>qec-code</code> and run exact or
        randomized distance checks backed by <code>qec-ilp-core</code>.
      </p>
      <div class="docs-card-links">
        <a href="guide/#feature-walkthroughs">CSS construction</a>
      </div>
    </article>
  </div>
</section>

<section id="quick-start" class="panel" aria-labelledby="quick-start-title">
  <div class="section-heading">
    <p class="eyebrow">Quick start</p>
    <h2 id="quick-start-title">From clone to first circuit</h2>
  </div>
  <pre><code>git clone https://github.com/nzy1997/rstim
cd rstim
cargo build --workspace
printf 'H 0\nREPEAT 2 {\n  M 0\n  DETECTOR rec[-1]\n  TICK\n}\n' | cargo run -p rstim --bin rstim -- stats</code></pre>
  <p class="section-copy">
    The last command prints circuit statistics such as
    <code>num_qubits</code>, <code>num_measurements</code>, and
    <code>num_detectors</code>. From there, the <a href="guide/">guide</a>
    walks through sampling, DEM extraction, rendering, and decoder
    experiments.
  </p>
</section>

<section id="headline-benchmark" aria-labelledby="headline-benchmark-title">
  <div class="section-heading">
    <p class="eyebrow">Checked evidence</p>
    <h2 id="headline-benchmark-title">Benchmarked, with claims limits</h2>
    <p class="section-copy">
      The committed surface-code decoder comparison below is checked
      full-tier evidence: committed-run evidence, not a general decoder
      ordering claim. Methodology, provenance, and every claims limit live
      on the <a href="benchmarks/">benchmarks page</a>.
    </p>
  </div>
  <figure class="result-plot">
    <a href="benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png">
      <img
        src="benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png"
        alt="Surface-code decoder comparison plot from the checked full-tier benchmark run"
      >
    </a>
  </figure>
</section>
{% endblock main %}
```

- [ ] **Step 7: Build and verify**

Run: `rm -rf _site && zola --root site build --output-dir "$(pwd)/_site" && grep -cE 'quick-start|headline-benchmark|capabilities' _site/index.html`
Expected: build succeeds; grep prints a count ≥ 3. Also confirm `_site/styles.css` exists.

- [ ] **Step 8: Commit**

```bash
git add site/config.toml site/templates site/content site/static
git commit -m "feat(site): scaffold Zola site with pitch landing page"
```

---

### Task 2: Guide page

**Files:**
- Create: `site/templates/guide.html`, `site/content/guide/_index.md`
- Read (source of moved HTML): `site/index.html`

**Interfaces:**
- Consumes: `base.html` blocks and `root` contract from Task 1 (`root = ".."` here).
- Produces: built page `guide/index.html` with anchor ids `workspace-overview`, `feature-walkthroughs`, `operations`.

- [ ] **Step 1: Create `site/content/guide/_index.md`**

```markdown
+++
title = "Guide"
template = "guide.html"

[extra]
root = ".."
+++
```

- [ ] **Step 2: Create `site/templates/guide.html`**

```html
{% extends "base.html" %}

{% block title %}Guide — rstim{% endblock title %}
{% block description %}rstim workspace map, runnable walkthroughs, and the QP101 operation reference.{% endblock description %}

{% block hero %}
<div class="hero">
  <p class="eyebrow">Guide</p>
  <h1>Workspace guide</h1>
  <p class="hero-copy">
    The workspace map, runnable walkthroughs, and the QP101 operation
    reference.
  </p>
</div>
{% endblock hero %}

{% block main %}
<!-- PASTE 1: site/index.html lines 109-242 verbatim
     (the two <section> blocks: id="workspace-overview" and
      id="feature-walkthroughs") -->
<!-- PASTE 2: site/index.html lines 517-580 verbatim
     (the <section id="operations"> block with the operation-types table) -->
{% endblock main %}
```

Then replace the two PASTE comments with the referenced line ranges, and apply exactly one href edit inside the pasted HTML:

- Old (from line 204): `<a href="#qp101">QP101 browser</a>`
- New: `<a href="../qp101/">QP101 browser</a>`

All `https://github.com/...` links and the `<a href="#operations">Operation semantics</a>` link (line 193) stay unchanged — `#operations` is now on this same page.

- [ ] **Step 3: Build and verify**

Run: `rm -rf _site && zola --root site build --output-dir "$(pwd)/_site" && grep -oE 'id="workspace-overview"|id="feature-walkthroughs"|id="operations"|href="\.\./qp101/"' _site/guide/index.html | sort -u`
Expected: all four strings printed; no `href="#qp101"` remains (`! grep -q 'href="#qp101"' _site/guide/index.html`).

- [ ] **Step 4: Commit**

```bash
git add site/templates/guide.html site/content/guide
git commit -m "feat(site): add guide page with workspace map, walkthroughs, operations"
```

---

### Task 3: QP101 page and qp101-browser.js

**Files:**
- Create: `site/templates/qp101.html`, `site/content/qp101/_index.md`, `site/static/js/qp101-browser.js`
- Read (sources): `site/index.html`, `site/app.js`

**Interfaces:**
- Consumes: `root`/`data-root` contract from Task 1.
- Produces: built page `qp101/index.html` with ids `qp101`, `schema-browser`, `gallery`, `examples`, loading `js/qp101-browser.js`, which fetches `ROOT + "/qp101.schema.json"`.

- [ ] **Step 1: Create `site/content/qp101/_index.md`**

```markdown
+++
title = "QP101"
template = "qp101.html"

[extra]
root = ".."
+++
```

- [ ] **Step 2: Create `site/templates/qp101.html`**

```html
{% extends "base.html" %}

{% block title %}QP101-ZY — rstim{% endblock title %}
{% block description %}QP101-ZY schema browser, protocol draft, examples, and rendered circuit gallery.{% endblock description %}

{% block hero %}
<div class="hero">
  <div class="qp101-kicker">
    <p class="eyebrow">Quantum circuit interchange</p>
    <span class="status">JSON Schema + protocol draft</span>
  </div>
  <h1>QP101-ZY schema browser</h1>
  <p class="hero-copy">
    A JSON format for ordered quantum circuits, repeat blocks, detector
    annotations, noise events, and visualization metadata used by
    <code>rstim</code> and <code>qp101-viz</code>.
  </p>
  <div class="actions" aria-label="QP101 resources">
    <a class="button primary" href="#schema-browser">Open the schema browser</a>
    <a class="button" href="../qp101.schema.json" download>Download schema</a>
    <a class="button" href="../QP101-ZY.md">Read protocol draft</a>
  </div>
</div>
{% endblock hero %}

{% block main %}
<section id="qp101" class="qp101-section" aria-label="QP101 reference">
  <!-- PASTE 1: site/index.html lines 468-495 verbatim (intro-grid) -->
  <!-- PASTE 2: site/index.html lines 497-515 verbatim (schema-browser panel) -->
  <!-- PASTE 3: site/index.html lines 582-655 verbatim (gallery) -->
  <!-- PASTE 4: site/index.html lines 657-709 verbatim (examples) -->
</section>
{% endblock main %}

{% block scripts %}
<script src="{{ root }}/js/qp101-browser.js"></script>
{% endblock scripts %}
```

Replace the PASTE comments with the referenced ranges, then apply these href/src edits inside the pasted HTML (old → new; every occurrence):

| Old | New | Occurrences (source lines) |
| --- | --- | --- |
| `href="examples/basic.qp101.json"` | `href="../examples/basic.qp101.json"` | 592, 599, 666 |
| `href="examples/repeat-detector.qp101.json"` | `href="../examples/repeat-detector.qp101.json"` | 614, 621, 689 |
| `href="examples/atom-loss-sample.qp101.json"` | `href="../examples/atom-loss-sample.qp101.json"` | 636, 643 |
| `src="gallery/basic-site.svg"` | `src="../gallery/basic-site.svg"` | 606 |
| `src="gallery/repeat-detector-site.svg"` | `src="../gallery/repeat-detector-site.svg"` | 627 |
| `src="gallery/atom-loss-sample.svg"` | `src="../gallery/atom-loss-sample.svg"` | 649 |

GitHub `https://...` links stay unchanged. The operations table (lines 517-580) is NOT pasted here — it moved to the guide page in Task 2.

- [ ] **Step 3: Create `site/static/js/qp101-browser.js`**

Assemble in this exact order:

1. Opening lines:

```js
(function () {
  const ROOT = (document.body && document.body.dataset && document.body.dataset.root) || ".";
```

2. `site/app.js` lines 2–199 verbatim (`navList`/`detail`/`status` consts, `groups`, `resolveRef`, `schemaType`, `titleFromKey`, `collectNodes`, `nodeSchema`, `renderMeta`, `renderVariantList`, `renderFields`, `escapeHtml`, `renderDetail`).
3. `site/app.js` lines 479–506 verbatim (`renderNav`).
4. The schema fetch block — `site/app.js` lines 538–557 with two edits (fetch URL and error link):

```js
  fetch(ROOT + "/qp101.schema.json")
    .then((response) => {
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }
      return response.json();
    })
    .then((schema) => {
      status.textContent = "Loaded";
      renderNav(schema, collectNodes(schema));
    })
    .catch((error) => {
      status.textContent = "Error";
      detail.classList.add("error");
      detail.innerHTML = `
        <h3>Schema could not be loaded</h3>
        <p>${escapeHtml(error.message)}</p>
        <p><a href="${ROOT}/qp101.schema.json" download>Download qp101.schema.json</a></p>
      `;
    });
})();
```

- [ ] **Step 4: Build and verify**

Run: `rm -rf _site && zola --root site build --output-dir "$(pwd)/_site" && grep -oE 'id="schema-browser"|id="gallery"|id="examples"|js/qp101-browser\.js|\.\./qp101\.schema\.json' _site/qp101/index.html | sort -u && node --check site/static/js/qp101-browser.js`
Expected: all five strings printed; `node --check` exits 0 (syntax valid). Also: `! grep -q 'src="gallery/' _site/qp101/index.html` (no un-prefixed gallery srcs).

- [ ] **Step 5: Commit**

```bash
git add site/templates/qp101.html site/content/qp101 site/static/js/qp101-browser.js
git commit -m "feat(site): add QP101 page with schema browser, gallery, examples"
```

---

### Task 4: Benchmarks page, benchmarks.js, and node rendering test

**Files:**
- Create: `site/templates/benchmarks.html`, `site/content/benchmarks/_index.md`, `site/static/js/benchmarks.js`
- Modify: `tools/test_site_app_rendering.py`
- Read (sources): `site/index.html`, `site/app.js`

**Interfaces:**
- Consumes: `root`/`data-root` contract from Task 1.
- Produces: built page `benchmarks/index.html` with ids `benchmark-evidence`, `checked-benchmark-results`, `checked-benchmark-result-cards`, `benchmarks`, `benchmark-manifest`; `js/benchmarks.js` fetches `ROOT + "/data/benchmark-site.json"` and prefixes every manifest artifact href/src with `ROOT + "/"`.

- [ ] **Step 1: Update the node rendering test to target the new file (failing first)**

In `tools/test_site_app_rendering.py` apply exactly these edits:

- Line 12 skip message: `"node is required to execute site/static/js/benchmarks.js"`
- Line 20: `const appJs = fs.readFileSync("site/static/js/benchmarks.js", "utf8");`
- Line 58 (inside the `document` stub object, before `getElementById`): add

```js
              body: { dataset: { root: "." } },
```

- Line 70 fetch matcher: `const fixture = url.endsWith("data/benchmark-site.json") ? manifestFixture : schemaFixture();`
- Line 78 filename label: `{ filename: "site/static/js/benchmarks.js" }`

- [ ] **Step 2: Run the test to verify it fails**

Run: `python3 -m pytest tools/test_site_app_rendering.py -v`
Expected: FAIL (or ERROR) — `site/static/js/benchmarks.js` does not exist yet. (If node is not installed the test skips; then rely on Step 6's check instead.)

- [ ] **Step 3: Create `site/content/benchmarks/_index.md`**

```markdown
+++
title = "Benchmarks"
template = "benchmarks.html"

[extra]
root = ".."
+++
```

- [ ] **Step 4: Create `site/templates/benchmarks.html`**

```html
{% extends "base.html" %}

{% block title %}Benchmarks — rstim{% endblock title %}
{% block description %}Checked benchmark evidence, results, methodology, and claims limits for rstim.{% endblock description %}

{% block hero %}
<div class="hero">
  <p class="eyebrow">Checked evidence</p>
  <h1>Benchmarks</h1>
  <p class="hero-copy">
    Benchmark and reproduction evidence, checked result artifacts, the
    methodology behind them, and the claims limits they carry.
  </p>
</div>
{% endblock hero %}

{% block main %}
<!-- PASTE: site/index.html lines 244-445 verbatim (the three <section>
     blocks: id="benchmark-evidence", id="checked-benchmark-results",
     id="benchmarks") -->
{% endblock main %}

{% block scripts %}
<script src="{{ root }}/js/benchmarks.js"></script>
{% endblock scripts %}
```

Replace the PASTE comment with the referenced range. No href edits — every link in that range is an absolute GitHub URL, and the claims-policy phrases must land byte-identical.

- [ ] **Step 5: Create `site/static/js/benchmarks.js`**

Assemble in this exact order:

1. Opening lines:

```js
(function () {
  const ROOT = (document.body && document.body.dataset && document.body.dataset.root) || ".";
```

2. `escapeHtml` — copy of `site/app.js` lines 181–187:

```js
  function escapeHtml(value) {
    return String(value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }
```

3. `site/app.js` lines 200–477 verbatim (benchmark manifest consts and every render function through `renderCheckedBenchmarkResults`), with exactly these three edits to prefix artifact URLs:

In `renderArtifactLinks` (source lines 285–290), the link becomes:

```js
        (artifact) => `
        <li>
          <a href="${ROOT}/${escapeHtml(artifact.path)}">${escapeHtml(fileName(artifact.path))}</a>
          <span class="badge">${escapeHtml(artifact.kind || "artifact")}</span>
        </li>
      `,
```

In `renderImageArtifacts` (source lines 306–312), the figure becomes:

```js
        (image) => `
        <figure class="result-plot">
          <a href="${ROOT}/${escapeHtml(image.path)}">
            <img src="${ROOT}/${escapeHtml(image.path)}" alt="${escapeHtml(item.title || "Checked benchmark plot")}">
          </a>
        </figure>
      `,
```

(`repoSourceHref` and `renderSourceLinks` stay unchanged — they emit absolute GitHub URLs.)

4. The manifest fetch block — `site/app.js` lines 508–536 with the fetch URL and both error links edited:

```js
  if (benchmarkManifest || checkedBenchmarkResults) {
    fetch(ROOT + "/data/benchmark-site.json")
      .then((response) => {
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        return response.json();
      })
      .then((manifest) => {
        renderBenchmarkManifest(manifest);
        renderCheckedBenchmarkResults(manifest);
      })
      .catch((error) => {
        if (benchmarkManifest) {
          benchmarkManifest.classList.add("error");
          benchmarkManifest.innerHTML = `
            <p>Benchmark manifest could not be loaded: ${escapeHtml(error.message)}</p>
            <p><a href="${ROOT}/data/benchmark-site.json">Open benchmark-site.json</a></p>
          `;
        }
        if (checkedBenchmarkResults) {
          checkedBenchmarkResults.classList.add("error");
          checkedBenchmarkResults.innerHTML = `
            <p>Checked benchmark results could not be loaded: ${escapeHtml(error.message)}</p>
            <p><a href="${ROOT}/data/benchmark-site.json">Open benchmark-site.json</a></p>
          `;
        }
      });
  }
})();
```

- [ ] **Step 6: Run tests and build to verify they pass**

Run: `python3 -m pytest tools/test_site_app_rendering.py -v && node --check site/static/js/benchmarks.js && rm -rf _site && zola --root site build --output-dir "$(pwd)/_site"`
Expected: pytest PASS (or SKIP without node, in which case `node --check` is also skipped), zola build succeeds.
Then: `grep -cE 'Claims Policy|smoke checks verify wiring|not a general decoder ordering claim' _site/benchmarks/index.html` — expected ≥ 3, and `grep -oE 'id="benchmark-manifest"|id="checked-benchmark-result-cards"' _site/benchmarks/index.html | sort -u` prints both ids.

- [ ] **Step 7: Commit**

```bash
git add site/templates/benchmarks.html site/content/benchmarks site/static/js/benchmarks.js tools/test_site_app_rendering.py
git commit -m "feat(site): add benchmarks page with manifest-backed result cards"
```

---

### Task 5: Build switchover — Makefile and old-file removal

**Files:**
- Modify: `Makefile:47-58` (the `build-site` target)
- Delete: `site/index.html`, `site/app.js`, `site/styles.css`

**Interfaces:**
- Consumes: the Zola tree from Tasks 1–4; existing `tools/build_qp101_gallery.py` (`--out-dir`) and `tools/copy_site_benchmark_data.py` (`--site-root`) CLIs, unchanged.
- Produces: `make build-site` emits the complete multi-page `_site` (pages + styles + js + schema/protocol/examples/gallery/data/benchmark artifacts). NOTE: `tools/check_site_build.py` will FAIL against this tree until Task 6 — expected mid-branch state.

- [ ] **Step 1: Replace the `build-site` target in `Makefile`**

```make
build-site:
	rm -rf _site
	zola --root site build --output-dir $(CURDIR)/_site
	mkdir -p _site/examples _site/data
	cp rstim/doc/qp101.schema.json _site/qp101.schema.json
	cp rstim/doc/QP101-ZY.md _site/QP101-ZY.md
	cp qp101-viz/examples/basic.qp101.json _site/examples/basic.qp101.json
	cp qp101-viz/examples/repeat-detector.qp101.json _site/examples/repeat-detector.qp101.json
	cp qp101-viz/examples/atom-loss-sample.qp101.json _site/examples/atom-loss-sample.qp101.json
	python3 tools/build_qp101_gallery.py --repo-root . --out-dir _site/gallery
	python3 tools/copy_site_benchmark_data.py --repo-root . --site-root _site site/benchmark-site.json
```

(The `mkdir -p _site/gallery` from the old target is dropped — the gallery script creates its own out-dir; keep it if the script errors without it.)

- [ ] **Step 2: Delete the superseded single-page files**

Run: `git rm site/index.html site/app.js site/styles.css`

- [ ] **Step 3: Build and verify the full tree**

Run: `make build-site && ls _site/index.html _site/guide/index.html _site/benchmarks/index.html _site/qp101/index.html _site/styles.css _site/js/benchmarks.js _site/js/qp101-browser.js _site/qp101.schema.json _site/QP101-ZY.md _site/data/benchmark-site.json _site/gallery/basic-site.svg _site/examples/basic.qp101.json`
Expected: every path listed exists. Then spot-check an artifact copy: `ls _site/benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png` — exists (page dir and artifact dir coexist under `_site/benchmarks/`).

- [ ] **Step 4: Manual smoke of the served site**

Run: `python3 -m http.server 8080 --directory _site` and open `http://localhost:8080/`. Verify: nav works across all four pages, the QP101 schema browser loads definitions, the benchmarks page renders manifest cards with plot images, gallery SVGs render. Stop the server.

- [ ] **Step 5: Commit**

```bash
git add Makefile
git commit -m "feat(site): build site with Zola, retire single-page sources"
```

---

### Task 6: Rework tools/check_site_build.py for the multi-page site

**Files:**
- Modify: `tools/check_site_build.py`, `tools/test_check_site_build.py`

**Interfaces:**
- Consumes: the `_site` layout produced by Task 5.
- Produces: `check_site_build(site_root, repo_root)` validating four pages; helper `check_pages(site_root, page_data, js_texts)` (area name `"site pages"` replaces `"workspace overview"`); `resolve_site_reference(site_root, base_dir, value)` gains a `base_dir` parameter; `read_text_file(path, label=None)` gains a label for error messages. `run_self_test()` / `--self-test` still the harness.

- [ ] **Step 1: Update the unit tests first (failing)**

In `tools/test_check_site_build.py` apply these edits:

- `test_valid_fixture_prints_required_pass_summary_areas`: replace marker `"PASS workspace overview"` with `"PASS site pages"`.
- `test_invalid_app_js_blocks_provenance_pass`: replace both `"app.js"` occurrences with `"js/benchmarks.js"` (the unlink path and the detail assertion).
- `test_rejects_missing_claims_policy_caveat` and `test_rejects_missing_claims_policy_phrase_even_if_manifest_keeps_it`: operate on `fixture.site_root / "benchmarks/index.html"` instead of `"index.html"`.
- `test_rejects_html_reference_that_escapes_site_root`: replace the mutation with `index.read_text(...).replace('href="guide/"', 'href="../outside.txt"', 1)` and assert area `"site pages"`.
- `test_rejects_js_reference_that_escapes_site_root`: target `fixture.site_root / "js/benchmarks.js"`, append `'\nconst escaped = "../../outside.txt";\n'`, assert `"../../outside.txt"` in detail and area `"site pages"`.
- `test_missing_index_or_app_returns_fail_summary_instead_of_raising`: loop over `("index.html", "guide/index.html", "js/benchmarks.js")`.
- `test_invalid_utf8_returns_fail_summary_instead_of_raising`: loop over `("index.html", "js/benchmarks.js", "data/benchmark-site.json")`.
- `test_rejects_unmanifested_checked_artifact_link` and `test_rejects_unmanifested_rstim_dem_artifact_link`: append the bad link to `fixture.site_root / "benchmarks/index.html"` with an `../` prefix, e.g. `'<a href="../benchmarks/surface_decoder_compare/results/full/not-in-manifest.csv">bad</a>\n'` (the detail assertion strings stay without the prefix — the regex extracts the `benchmarks/...` tail).
- Add one new test:

```python
    def test_rejects_broken_cross_page_anchor(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        index = fixture.site_root / "index.html"
        index.write_text(
            index.read_text(encoding="utf-8").replace(
                'href="guide/#feature-walkthroughs"', 'href="guide/#missing-anchor"', 1
            ),
            encoding="utf-8",
        )

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any(
                result.status == "FAIL"
                and result.area == "site pages"
                and "missing-anchor" in result.detail
                for result in results
            ),
            check_site_build.format_summary(results),
        )
```

Run: `python3 -m pytest tools/test_check_site_build.py -x -q`
Expected: FAIL (checker still single-page).

- [ ] **Step 2: Replace the layout constants in `tools/check_site_build.py`**

Replace `REQUIRED_FILES`, `REQUIRED_ANCHORS`, and `ROOT_LEVEL_SITE_FILES` (lines 92–145 region) with:

```python
PAGE_FILES = (
    "index.html",
    "guide/index.html",
    "benchmarks/index.html",
    "qp101/index.html",
)
JS_FILES = ("js/qp101-browser.js", "js/benchmarks.js")
PAGE_REQUIRED_ANCHORS = {
    "index.html": ("capabilities", "quick-start", "headline-benchmark"),
    "guide/index.html": ("workspace-overview", "feature-walkthroughs", "operations"),
    "benchmarks/index.html": (
        "benchmark-evidence",
        "checked-benchmark-results",
        "checked-benchmark-result-cards",
        "benchmarks",
        "benchmark-manifest",
    ),
    "qp101/index.html": ("qp101", "schema-browser", "gallery", "examples"),
}
REQUIRED_FILES = PAGE_FILES + JS_FILES + (
    "styles.css",
    "data/benchmark-site.json",
    "QP101-ZY.md",
    "qp101.schema.json",
    "examples/basic.qp101.json",
    "examples/repeat-detector.qp101.json",
    "examples/atom-loss-sample.qp101.json",
    "gallery/basic-site.svg",
    "gallery/repeat-detector-site.svg",
    "gallery/atom-loss-sample.svg",
)
ROOT_LEVEL_SITE_FILES = {"qp101.schema.json", "QP101-ZY.md", "styles.css"}
```

`QP101_REQUIRED_FILES`, `CLAIMS_POLICY_PHRASES`, `CHECKED_ARTIFACT_REFERENCE_RE`, and `STRING_LITERAL_PATH_RE` stay unchanged.

- [ ] **Step 3: Make reference resolution page-relative**

Replace `resolve_site_reference` with:

```python
def resolve_site_reference(site_root: Path, base_dir: Path, value: str) -> tuple[str | None, Path | None, str | None]:
    normalized = normalize_local_reference(value)
    if normalized is None:
        return None, None, None
    origin = site_root if value.startswith("/") else base_dir
    candidate = (origin / normalized).resolve(strict=False)
    try:
        candidate.relative_to(site_root)
    except ValueError:
        return normalized, None, f"path escape outside built site: {value}"
    return normalized, candidate, None
```

Give `read_text_file` a label for error messages:

```python
def read_text_file(path: Path, label: str | None = None) -> tuple[str | None, str | None]:
    name = label or path.name
    try:
        return path.read_text(encoding="utf-8"), None
    except UnicodeDecodeError as exc:
        return None, f"{name}: invalid UTF-8 ({exc})"
    except OSError as exc:
        return None, f"{name}: {exc.strerror or exc}"
```

- [ ] **Step 4: Replace `check_workspace_overview` with `check_pages`**

Delete `check_workspace_overview` and add:

```python
def check_page_reference(
    site_root: Path,
    page: str,
    page_dir: Path,
    value: str,
    ids_by_page: dict[str, set[str]],
) -> str | None:
    if value.startswith("#"):
        anchor = value[1:]
        if anchor and anchor not in ids_by_page.get(page, set()):
            return f"{page}: missing same-page anchor #{anchor}"
        return None
    if is_external_link(value):
        return None
    split = urlsplit(value)
    if split.scheme or split.netloc or not split.path:
        return None
    normalized, candidate, error = resolve_site_reference(site_root, page_dir, split.path)
    if error is not None:
        return f"{page}: {error}"
    if normalized is None or candidate is None:
        return None
    target = candidate / "index.html" if candidate.is_dir() else candidate
    if not target.exists():
        return f"{page}: missing local reference {value}"
    if split.fragment:
        try:
            target_page = target.relative_to(site_root).as_posix()
        except ValueError:
            return None
        target_ids = ids_by_page.get(target_page)
        if target_ids is not None and split.fragment not in target_ids:
            return f"{page}: missing anchor #{split.fragment} in {target_page}"
    return None


def check_pages(
    site_root: Path,
    page_data: dict[str, tuple[str, HtmlCollector]],
    js_texts: dict[str, str],
) -> CheckResult:
    problems: list[str] = []
    ids_by_page = {page: collector.ids for page, (_, collector) in page_data.items()}

    for page, (text, collector) in page_data.items():
        page_dir = (site_root / page).parent
        missing_anchors = [
            anchor for anchor in PAGE_REQUIRED_ANCHORS.get(page, ()) if anchor not in collector.ids
        ]
        if missing_anchors:
            problems.append(f"{page}: missing required anchors: {', '.join(missing_anchors)}")

        for value in collector.hrefs + collector.srcs:
            problem = check_page_reference(site_root, page, page_dir, value, ids_by_page)
            if problem is not None:
                problems.append(problem)

        script_texts: list[str] = []
        for src in collector.srcs:
            if not src.endswith(".js"):
                continue
            _, candidate, _ = resolve_site_reference(site_root, page_dir, src)
            if candidate is not None and candidate.is_file():
                rel = candidate.relative_to(site_root).as_posix()
                if rel in js_texts:
                    script_texts.append(js_texts[rel])

        for path in sorted(collect_local_string_paths(text, *script_texts)):
            normalized, candidate, error = resolve_site_reference(site_root, page_dir, path)
            if error is not None:
                problems.append(f"{page}: {error}")
            elif normalized is not None and candidate is not None and not candidate.exists():
                problems.append(f"{page}: missing local reference {path}")

    if problems:
        return fail("site pages", "; ".join(problems))
    return pass_("site pages", "required anchors, cross-page links, and local references are present")
```

- [ ] **Step 5: Rewire `check_site_build` and the methodology/artifact checks**

Change `check_checked_artifacts` to take one combined text: signature `check_checked_artifacts(site_root, combined_text, manifest)`; inside, replace `index_text + "\n" + app_text` with `combined_text`. `check_benchmark_methodology` is unchanged but will be fed the benchmarks page text. Replace the body of `check_site_build` after the two `check_non_empty_files` results with:

```python
    page_data: dict[str, tuple[str, HtmlCollector]] = {}
    js_texts: dict[str, str] = {}
    read_errors: list[str] = []
    for page in PAGE_FILES:
        text, error = read_text_file(site_root / page, label=page)
        if error is not None:
            read_errors.append(error)
            continue
        collector = HtmlCollector()
        collector.feed(text)
        page_data[page] = (text, collector)
    for js_file in JS_FILES:
        text, error = read_text_file(site_root / js_file, label=js_file)
        if error is not None:
            read_errors.append(error)
        else:
            js_texts[js_file] = text

    if read_errors:
        results.append(fail("site pages", "could not read built-site files: " + "; ".join(read_errors)))
    else:
        results.append(check_pages(site_root, page_data, js_texts))

    manifest_results, manifest, manifest_site_errors = check_manifest_and_artifacts(site_root, repo_root)
    results.extend(manifest_results)
    results.append(check_checked_provenance(manifest, manifest_site_errors + read_errors))

    benchmarks_entry = page_data.get("benchmarks/index.html")
    if benchmarks_entry is None:
        results.append(fail("benchmark methodology", "could not read built-site files: benchmarks/index.html"))
    else:
        results.append(check_benchmark_methodology(benchmarks_entry[0]))

    if read_errors:
        results.append(fail("checked benchmark artifacts", "could not read built-site files: " + "; ".join(read_errors)))
    else:
        combined_text = "\n".join(
            [text for text, _ in page_data.values()] + [js_texts[js_file] for js_file in JS_FILES]
        )
        results.append(check_checked_artifacts(site_root, combined_text, manifest))
    results.append(check_local_only_future(site_root, manifest))

    return results
```

- [ ] **Step 6: Rewrite the self-test fixture pages**

In `make_fixture_site`, keep everything about the manifest, artifact files, git init, and asset files (`styles.css`, `QP101-ZY.md`, `qp101.schema.json`, `examples/*`, `gallery/*`, `benchmarks/*` writes) unchanged, but replace the single `index.html` and `app.js` writes with the four pages and two JS files:

```python
    write_text(
        site_root / "index.html",
        """<!doctype html>
<html lang="en">
<body data-root=".">
  <nav>
    <a href="./">Home</a>
    <a href="guide/">Guide</a>
    <a href="benchmarks/">Benchmarks</a>
    <a href="qp101/">QP101</a>
  </nav>
  <section id="capabilities"><a href="guide/#feature-walkthroughs">walkthroughs</a></section>
  <section id="quick-start"></section>
  <section id="headline-benchmark">
    <a href="benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png">
      <img src="benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png" alt="surface">
    </a>
  </section>
</body>
</html>
""",
    )
    write_text(
        site_root / "guide/index.html",
        """<!doctype html>
<html lang="en">
<body data-root="..">
  <section id="workspace-overview"></section>
  <section id="feature-walkthroughs">
    <a href="../qp101/">qp101</a>
    <a href="#operations">ops</a>
  </section>
  <section id="operations"></section>
</body>
</html>
""",
    )
    write_text(
        site_root / "benchmarks/index.html",
        """<!doctype html>
<html lang="en">
<body data-root="..">
  <section id="benchmark-evidence"></section>
  <section id="checked-benchmark-results">
    <div id="checked-benchmark-result-cards"></div>
    <a href="../benchmarks/surface_decoder_compare/results/full/results.csv">surface csv</a>
  </section>
  <section id="benchmarks">
    <h2>Benchmark Methodology</h2>
    <p>smoke checks verify wiring</p>
    <p>full evidence can describe the committed checked run only.</p>
    <p>checked full-tier artifacts are committed-run evidence.</p>
    <p>not a general decoder ordering claim.</p>
    <article><h3>Publishable Evidence</h3></article>
    <article><h3>Local-Only Evidence</h3></article>
    <article><h3>Claims Policy</h3></article>
    <div id="benchmark-manifest"></div>
  </section>
  <script src="../js/benchmarks.js"></script>
</body>
</html>
""",
    )
    write_text(
        site_root / "qp101/index.html",
        """<!doctype html>
<html lang="en">
<body data-root="..">
  <section id="qp101">
    <a href="../qp101.schema.json">schema</a>
    <a href="../QP101-ZY.md">protocol</a>
    <a href="../examples/basic.qp101.json">basic</a>
  </section>
  <section id="schema-browser"></section>
  <section id="gallery">
    <img src="../gallery/basic-site.svg" alt="basic">
    <img src="../gallery/repeat-detector-site.svg" alt="repeat">
    <img src="../gallery/atom-loss-sample.svg" alt="atom">
  </section>
  <section id="examples">
    <a href="../examples/repeat-detector.qp101.json">repeat</a>
    <a href="../examples/atom-loss-sample.qp101.json">atom loss</a>
  </section>
  <script src="../js/qp101-browser.js"></script>
</body>
</html>
""",
    )
    write_text(
        site_root / "js/benchmarks.js",
        """const checkedBenchmarkItems = ["surface-decoder-full", "bb-circuit-full", "rstim-vs-stim-full"];
function renderBenchmarkManifest(manifest) { return manifest; }
function renderCheckedBenchmarkResults(manifest) { return manifest; }
function renderProvenance(provenance) { return provenance; }
const manifestMarkers = ["family.status", "family.claims_limit", "item.status", "item.claims_limit"];
const checkedMarkers = ["item.artifacts", "item.commands", "item.caveats", "artifact.checked"];
const provenanceMarkers = ["item.provenance", "renderProvenance(item.provenance)", "artifact_hashes"];
fetch("/data/benchmark-site.json");
const localRefs = [
  "/benchmarks/bb_circuit_bposd_compare/results/full/results.csv",
  "/benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json",
];
artifact.kind === "image";
""",
    )
    write_text(
        site_root / "js/qp101-browser.js",
        """const status = document.getElementById("schema-status");
fetch("/qp101.schema.json");
const localRefs = [
  "/examples/basic.qp101.json",
  "/gallery/basic-site.svg",
];
""",
    )
```

- [ ] **Step 7: Update the self-test mutations**

In `run_self_test`, update the `mutations` list entries:

- `missing_claims_policy` and `unmanifested_checked_link`: target `f.site_root / "benchmarks/index.html"`; the unmanifested link becomes `'<a href="../benchmarks/surface_decoder_compare/results/full/not-in-manifest.csv">bad</a>\n'` (marker unchanged).
- `html_escape_outside_site`: mutation becomes `.replace('href="guide/"', 'href="../outside.txt"', 1)` on `index.html`; marker `"path escape outside built site: ../outside.txt"`.
- `js_escape_outside_site`: append `'\nconst escaped = "../../outside.txt";\n'` to `f.site_root / "js/benchmarks.js"`; marker `"path escape outside built site: ../../outside.txt"`.
- `invalid_utf8_app` → rename to `invalid_utf8_benchmarks_js`: write bytes to `f.site_root / "js/benchmarks.js"`; marker `"js/benchmarks.js: invalid UTF-8"`.
- Add a new mutation:

```python
            (
                "broken_cross_page_anchor",
                lambda f: (f.site_root / "index.html").write_text(
                    (f.site_root / "index.html").read_text(encoding="utf-8").replace(
                        'href="guide/#feature-walkthroughs"', 'href="guide/#missing-anchor"', 1
                    ),
                    encoding="utf-8",
                ),
                "missing anchor #missing-anchor",
            ),
```

All other mutations are unchanged.

- [ ] **Step 8: Run the self-test and unit tests**

Run: `python3 tools/check_site_build.py --self-test && python3 -m pytest tools/test_check_site_build.py -q`
Expected: `PASS self-test: ...` and all pytest tests pass.

- [ ] **Step 9: Validate the real build**

Run: `make build-site && python3 tools/check_site_build.py _site`
Expected: `SUMMARY: PASS (…, 0 failures)`. If any reference fails, fix the template href (not the checker) unless the checker logic is provably wrong.

- [ ] **Step 10: Commit**

```bash
git add tools/check_site_build.py tools/test_check_site_build.py
git commit -m "feat(tools): validate multi-page Zola site in check_site_build"
```

---

### Task 7: CI workflow and end-to-end verification

**Files:**
- Modify: `.github/workflows/deploy-pages.yml`

**Interfaces:**
- Consumes: `make build-site` (Task 5), `tools/check_site_build.py` (Task 6), the Zola version recorded in Task 1.

- [ ] **Step 1: Add Zola install and checker gate to the build job**

In `.github/workflows/deploy-pages.yml`, after the `Swatinem/rust-cache@v2` step and before `Build benchmarked documentation site`, insert (substituting the Task 1 version for `v0.19.2`):

```yaml
      - name: Install Zola
        run: |
          curl -fsSL https://github.com/getzola/zola/releases/download/v0.19.2/zola-v0.19.2-x86_64-unknown-linux-gnu.tar.gz \
            | sudo tar -xz -C /usr/local/bin zola
          zola --version
```

After the `Build benchmarked documentation site` step and before `Upload Pages artifact`, insert:

```yaml
      - name: Check built site
        run: python3 tools/check_site_build.py _site
```

- [ ] **Step 2: Full local verification pass**

Run each and confirm the expected result:

```bash
make build-site                                    # succeeds
python3 tools/check_site_build.py _site           # SUMMARY: PASS
python3 tools/check_site_build.py --self-test     # PASS self-test
python3 -m pytest tools/test_check_site_build.py tools/test_site_app_rendering.py -q   # all pass
```

Then serve and click through once more: `python3 -m http.server 8080 --directory _site` — all four pages, schema browser loads, benchmark cards render with images, no 404s in the browser console.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/deploy-pages.yml
git commit -m "ci: install Zola and gate Pages deploy on site checker"
```

- [ ] **Step 4: Finish the branch**

Use the superpowers:finishing-a-development-branch skill to merge or open a PR. After the Pages deploy runs on master, verify https://nzy1997.github.io/rstim/ serves the new landing page and subpages.
