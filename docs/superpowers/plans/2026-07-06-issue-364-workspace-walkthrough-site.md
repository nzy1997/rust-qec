# Issue 364 Workspace Walkthrough Site Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add workspace overview and feature walkthrough sections to the static site, with a focused contract proving the required crate names, showcase links, and workflow categories are present.

**Architecture:** Keep the existing single-page static site. Add a source-based Rust integration test in `rstim/tests/site_contract.rs`, then update `site/index.html` and `site/styles.css` with compact technical sections that link to existing Markdown showcases and benchmark evidence.

**Tech Stack:** Plain HTML, CSS, existing `make build-site`, Rust integration tests under `rstim/tests/`, `cargo test`.

## Global Constraints

- Keep `site/app.js` fetching exactly `qp101.schema.json`.
- Keep existing QP101 IDs and resource links: `id="qp101"`, `id="schema-browser"`, `id="operations"`, `id="gallery"`, `id="examples"`, `qp101.schema.json`, `QP101-ZY.md`, selected `examples/*.qp101.json`, and `gallery/*.svg`.
- Name every required workspace crate in the site source: `rstim`, `rsinter`, `rmatching`, `rbposd`, `rilpqec`, `qec-code`, and `qec-ilp-core`.
- Link or name runnable entry points for circuit parsing, sampling, detection, DEM extraction, SVG/QP101 export, decoder experiments, CSS construction, and distance-search workflows.
- Link `docs/showcases/rstim-cli-dem-pipeline.md`, `docs/showcases/rstim-render-svg-atom-loss.md`, `docs/showcases/qec-code-css-construction.md`, and benchmark evidence or the qec-code random-window showcase.
- Reuse existing showcase pages as source links instead of duplicating long command blocks in HTML.
- Do not add new feature implementations, benchmark runs, generated benchmark artifacts, a frontend framework, a router, or a new build dependency.
- Verification commands:
  - `make build-site`
  - `python3 tools/check_showcase_docs.py docs/showcases`
  - `cargo test -p rstim --test site_contract workspace_feature_walkthroughs_are_linked -q`
  - `cargo test`

---

## File Structure

- Modify `rstim/tests/site_contract.rs`: add `workspace_feature_walkthroughs_are_linked` and local helper assertions for readable negative-control failures.
- Modify `site/index.html`: add `workspace-overview`, `feature-walkthroughs`, and `benchmark-evidence` sections; update navigation and docs-home card links.
- Modify `site/styles.css`: add compact overview/walkthrough/evidence card grids and responsive behavior using existing variables.
- Leave `site/app.js`, `Makefile`, showcase Markdown, and benchmark artifacts unchanged.

### Task 1: Add Failing Workspace Walkthrough Contract

**Files:**
- Modify: `rstim/tests/site_contract.rs`

**Interfaces:**
- Consumes: `site/index.html` source HTML.
- Produces: Rust integration test `workspace_feature_walkthroughs_are_linked`.

- [ ] **Step 1: Write the failing test**

Add these helper functions after `assert_repo_file_exists`:

```rust
fn assert_contains_all(haystack: &str, markers: &[&str], context: &str) {
    for marker in markers {
        assert!(
            haystack.contains(marker),
            "{context} is missing marker {marker}"
        );
    }
}

fn assert_contains_all_case_insensitive(haystack: &str, markers: &[&str], context: &str) {
    let lower = haystack.to_lowercase();
    for marker in markers {
        assert!(
            lower.contains(&marker.to_lowercase()),
            "{context} is missing marker {marker}"
        );
    }
}
```

Add this test after `qp101_browser_resources_are_preserved`:

```rust
#[test]
fn workspace_feature_walkthroughs_are_linked() {
    let index = read_repo_file("site/index.html");

    assert_contains_all(
        &index,
        &[
            "id=\"workspace-overview\"",
            "id=\"feature-walkthroughs\"",
            "id=\"benchmark-evidence\"",
            "rstim",
            "rsinter",
            "rmatching",
            "rbposd",
            "rilpqec",
            "qec-code",
            "qec-ilp-core",
            "docs/showcases/rstim-cli-dem-pipeline.md",
            "docs/showcases/rstim-render-svg-atom-loss.md",
            "docs/showcases/qec-code-css-construction.md",
            "docs/showcases/benchmark-evidence.md",
            "docs/showcases/qec-code-random-window-benchmark.md",
            "docs/showcases/README.md",
            "rstim/doc/cli.md",
            "rstim stats",
            "rstim sample",
            "rstim detect",
            "rstim analyze_errors",
            "rstim render_svg",
            "rstim export_json",
            "rsinter bench",
            "code css",
            "random-window-upper-bound",
        ],
        "workspace walkthrough site source",
    );

    assert_contains_all_case_insensitive(
        &index,
        &[
            "circuit parsing",
            "sampling",
            "detection",
            "dem extraction",
            "svg/qp101 export",
            "decoder experiments",
            "css construction",
            "distance-search workflows",
        ],
        "workspace walkthrough copy",
    );
}
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```sh
cargo test -p rstim --test site_contract workspace_feature_walkthroughs_are_linked -q
```

Expected: FAIL because `site/index.html` does not yet contain `id="workspace-overview"`, `id="feature-walkthroughs"`, or the required workflow links.

- [ ] **Step 3: Commit the failing contract**

Run:

```sh
git add rstim/tests/site_contract.rs
git commit -m "test: require workspace feature walkthroughs"
```

Expected: commit succeeds with only `rstim/tests/site_contract.rs` staged.

### Task 2: Add Site Sections And Compact Styling

**Files:**
- Modify: `site/index.html`
- Modify: `site/styles.css`

**Interfaces:**
- Consumes: the Task 1 test markers.
- Produces: static site content that satisfies `workspace_feature_walkthroughs_are_linked` while preserving the QP101 contract.

- [ ] **Step 1: Update primary navigation**

In `site/index.html`, extend `.nav-links` so it contains these links before the existing QP101/gallery/example links:

```html
<a href="#workspace-overview">Workspace</a>
<a href="#feature-walkthroughs">Walkthroughs</a>
<a href="#benchmark-evidence">Evidence</a>
```

Keep the existing QP101, Operations, Gallery, and Examples links.

- [ ] **Step 2: Add a workspace card to the docs home grid**

In the `#docs-home .docs-grid`, add this card before the QP101 card:

```html
<article class="docs-card">
  <h3>Workspace Overview</h3>
  <p>
    Map <code>rstim</code>, decoder crates, CSS construction helpers, and
    ILP-backed checks before jumping into runnable workflows.
  </p>
  <div class="docs-card-links">
    <a href="#workspace-overview">Crates</a>
    <a href="#feature-walkthroughs">Walkthroughs</a>
    <a href="#benchmark-evidence">Evidence</a>
  </div>
</article>
```

- [ ] **Step 3: Add the workspace overview section**

Add this section after `</section>` for `docs-home` and before `<section id="qp101"`:

```html
<section id="workspace-overview" class="workspace-overview" aria-labelledby="workspace-overview-title">
  <div class="section-heading">
    <p class="eyebrow">Workspace overview</p>
    <h2 id="workspace-overview-title">Crates by workflow boundary</h2>
    <p class="section-copy">
      The workspace is split around circuit production, detector-error-model
      data paths, decoder experiments, and CSS-code construction. The cards
      below name the crate that owns each boundary before the walkthroughs
      link to runnable examples.
    </p>
  </div>
  <div class="workspace-grid">
    <article class="compact-card">
      <h3><code>rstim</code></h3>
      <p>
        Simulator crate and CLI for circuit parsing, sampling, detection,
        DEM extraction, SVG rendering, and QP101 export.
      </p>
    </article>
    <article class="compact-card">
      <h3><code>rsinter</code></h3>
      <p>
        Benchmark orchestration and decoder experiment harness for surface-code
        and BB-circuit comparison flows.
      </p>
    </article>
    <article class="compact-card">
      <h3><code>rmatching</code></h3>
      <p>
        Rust MWPM decoder entry point for detector-error-model workflows.
      </p>
    </article>
    <article class="compact-card">
      <h3><code>rbposd</code></h3>
      <p>
        BP-OSD decoder component used by the workspace comparison harnesses.
      </p>
    </article>
    <article class="compact-card">
      <h3><code>rilpqec</code></h3>
      <p>
        ILP-style decoder component used in comparison and reproduction flows.
      </p>
    </article>
    <article class="compact-card">
      <h3><code>qec-code</code> and <code>qec-ilp-core</code></h3>
      <p>
        CSS construction helpers, code-family exports, and ILP-backed distance
        checks used by exact and randomized distance-search workflows.
      </p>
    </article>
  </div>
</section>
```

- [ ] **Step 4: Add feature walkthrough and evidence sections**

Immediately after the workspace overview section, add:

```html
<section id="feature-walkthroughs" class="feature-walkthroughs" aria-labelledby="feature-walkthroughs-title">
  <div class="section-heading">
    <p class="eyebrow">Feature walkthroughs</p>
    <h2 id="feature-walkthroughs-title">Runnable entry points by task</h2>
    <p class="section-copy">
      These cards link the existing showcase pages and CLI references instead
      of copying their long command blocks into the site.
    </p>
  </div>
  <div class="walkthrough-grid">
    <article class="compact-card">
      <h3>Circuit parsing and sampling</h3>
      <p>
        Start with <code>rstim stats</code> for circuit parsing and structure,
        then use <code>rstim sample</code> for measurement shots.
      </p>
      <div class="docs-card-links">
        <a href="https://github.com/nzy1997/rstim/blob/master/docs/showcases/rstim-cli-dem-pipeline.md">CLI DEM pipeline</a>
        <a href="https://github.com/nzy1997/rstim/blob/master/rstim/doc/cli.md">CLI reference</a>
      </div>
    </article>
    <article class="compact-card">
      <h3>Detection and DEM extraction</h3>
      <p>
        Use <code>rstim detect</code>, <code>rstim analyze_errors</code>, and
        <code>rstim sample_dem</code> to move from circuit shots to detector
        events and detector error models.
      </p>
      <div class="docs-card-links">
        <a href="https://github.com/nzy1997/rstim/blob/master/docs/showcases/rstim-cli-dem-pipeline.md">DEM walkthrough</a>
        <a href="#operations">Operation semantics</a>
      </div>
    </article>
    <article class="compact-card">
      <h3>SVG/QP101 export</h3>
      <p>
        Render static diagrams with <code>rstim render_svg</code> and export
        structured interchange data with <code>rstim export_json</code>.
      </p>
      <div class="docs-card-links">
        <a href="https://github.com/nzy1997/rstim/blob/master/docs/showcases/rstim-render-svg-atom-loss.md">SVG atom-loss showcase</a>
        <a href="#qp101">QP101 browser</a>
      </div>
    </article>
    <article class="compact-card">
      <h3>Decoder experiments</h3>
      <p>
        Follow <code>rsinter bench</code> and the decoder crates through the
        checked-in surface, MWPM, BP-OSD, and ILP comparison evidence.
      </p>
      <div class="docs-card-links">
        <a href="https://github.com/nzy1997/rstim/blob/master/docs/showcases/benchmark-evidence.md">Benchmark evidence</a>
        <a href="https://github.com/nzy1997/rstim/blob/master/benchmarks/surface_decoder_compare/README.md">Surface decoder comparison</a>
      </div>
    </article>
    <article class="compact-card">
      <h3>CSS construction</h3>
      <p>
        Use <code>qec-code code css</code> to list built-ins, export sparse
        rows, and construct CSS matrices for fixed and parameterized families.
      </p>
      <div class="docs-card-links">
        <a href="https://github.com/nzy1997/rstim/blob/master/docs/showcases/qec-code-css-construction.md">CSS construction showcase</a>
        <a href="https://github.com/nzy1997/rstim/blob/master/docs/showcases/README.md">Showcase index</a>
      </div>
    </article>
    <article class="compact-card">
      <h3>Distance-search workflows</h3>
      <p>
        Run exact checks and randomized upper-bound searches, including
        <code>random-window-upper-bound</code>, through <code>qec-code</code>
        and <code>qec-ilp-core</code>.
      </p>
      <div class="docs-card-links">
        <a href="https://github.com/nzy1997/rstim/blob/master/docs/showcases/qec-code-css-construction.md">Exact distance examples</a>
        <a href="https://github.com/nzy1997/rstim/blob/master/docs/showcases/qec-code-random-window-benchmark.md">Random-window benchmark</a>
      </div>
    </article>
  </div>
</section>

<section id="benchmark-evidence" class="benchmark-evidence" aria-labelledby="benchmark-evidence-title">
  <div class="section-heading">
    <p class="eyebrow">Evidence after walkthroughs</p>
    <h2 id="benchmark-evidence-title">Benchmark and reproduction evidence</h2>
    <p class="section-copy">
      The site links the checked-in benchmark evidence after the workspace map
      and feature walkthroughs so performance material is grounded in runnable
      entry points.
    </p>
  </div>
  <div class="evidence-grid">
    <article class="compact-card">
      <h3>Benchmark evidence</h3>
      <p>
        Surface-code, BB-circuit, and decoder comparison artifacts with smoke
        and full-campaign entry points.
      </p>
      <div class="docs-card-links">
        <a href="https://github.com/nzy1997/rstim/blob/master/docs/showcases/benchmark-evidence.md">Evidence showcase</a>
      </div>
    </article>
    <article class="compact-card">
      <h3>Random-window distance search</h3>
      <p>
        QEC-code random-window upper-bound evidence, no-target smoke profiles,
        and paper-baseline comparison wiring.
      </p>
      <div class="docs-card-links">
        <a href="https://github.com/nzy1997/rstim/blob/master/docs/showcases/qec-code-random-window-benchmark.md">Random-window showcase</a>
      </div>
    </article>
  </div>
</section>
```

- [ ] **Step 5: Add compact CSS grids**

In `site/styles.css`, update the combined display selector:

```css
.brand,
.nav-links,
.docs-card-links,
.docs-home,
.workspace-overview,
.feature-walkthroughs,
.benchmark-evidence,
.docs-grid,
.qp101-section,
.qp101-kicker {
  display: flex;
  gap: 1rem;
}
```

Add these rules near the existing card/grid rules:

```css
.workspace-overview,
.feature-walkthroughs,
.benchmark-evidence {
  flex-direction: column;
}

.workspace-grid,
.walkthrough-grid,
.evidence-grid {
  display: grid;
  gap: 1rem;
}

.workspace-grid,
.walkthrough-grid {
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.evidence-grid {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.compact-card {
  min-width: 0;
  padding: 1.15rem;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface);
  box-shadow: var(--shadow);
}

.compact-card p {
  margin: 0.65rem 0 0;
  color: var(--muted);
}

.compact-card .docs-card-links {
  flex-wrap: wrap;
  margin-top: 1rem;
}
```

Update the `@media (max-width: 820px)` grid stack selector to include:

```css
.workspace-grid,
.walkthrough-grid,
.evidence-grid,
```

- [ ] **Step 6: Run the focused test to verify GREEN**

Run:

```sh
cargo test -p rstim --test site_contract workspace_feature_walkthroughs_are_linked -q
```

Expected: PASS.

- [ ] **Step 7: Rebuild the site and preserve the QP101 contract**

Run:

```sh
make build-site
cargo test -p rstim --test site_contract qp101_browser_resources_are_preserved -q
```

Expected: both commands exit 0.

- [ ] **Step 8: Commit the site update**

Run:

```sh
git add site/index.html site/styles.css
git commit -m "feat: add workspace walkthrough site sections"
```

Expected: commit succeeds with only the static site files staged.

### Task 3: Documentation Checker And Final Verification

**Files:**
- No new files.

**Interfaces:**
- Consumes: Task 1 and Task 2 commits.
- Produces: verified branch ready for review and PR creation.

- [ ] **Step 1: Run required verification**

Run:

```sh
make build-site
python3 tools/check_showcase_docs.py docs/showcases
cargo test -p rstim --test site_contract workspace_feature_walkthroughs_are_linked -q
cargo test
```

Expected: all commands exit 0.

- [ ] **Step 2: Inspect branch diff**

Run:

```sh
git diff --stat origin/master..HEAD
git status --short
```

Expected: diff only contains the design spec, implementation plan, site contract test, and static site files. Working tree is clean except ignored generated `_site/` output if present.

## Plan Self Review

- Spec coverage: Task 1 covers the negative-control contract. Task 2 covers all required crates, workflow categories, showcase links, benchmark/random-window evidence, and compact site styling. Task 3 covers the issue verification commands and required `cargo test`.
- Placeholder scan: no unresolved markers or incomplete steps remain.
- Type consistency: helper names and test names in Task 1 match the verification command and later references.
