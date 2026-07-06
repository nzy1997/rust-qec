# Issue 363 QP101 Docs Shell Preservation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a broader static documentation shell while preserving the QP101 schema browser and its direct resource paths.

**Architecture:** Keep the site single-page and static. Add a documentation shell to `site/index.html`, extend `site/styles.css`, leave `site/app.js` fetching `qp101.schema.json`, and protect the contract with a focused Rust integration test that inspects `_site/` after `make build-site`.

**Tech Stack:** Plain HTML, CSS, JavaScript, existing `make build-site`, Rust integration tests under `rstim/tests/`, `cargo test`.

## Global Constraints

- Keep `site/app.js` fetching exactly `qp101.schema.json`.
- Keep direct links to `qp101.schema.json`, `QP101-ZY.md`, and selected `examples/*.qp101.json`.
- Keep gallery image paths under `gallery/*.svg`.
- Do not change `rstim/doc/qp101.schema.json`.
- Do not change `rstim export_json` behavior.
- Do not add a client-side router, frontend framework, or new build dependency.
- The page must expose QP101 as a first-class area with `id="qp101"` and a primary navigation link to `#qp101`.
- Verification commands:
  - `make build-site`
  - `cargo test -p rstim --test site_gallery -q`
  - `cargo test -p rstim --test site_contract qp101_browser_resources_are_preserved -q`
  - `cargo test`

---

## File Structure

- Create `rstim/tests/site_contract.rs`: focused static-site contract test for QP101 resource preservation.
- Modify `site/index.html`: broader docs shell, primary docs navigation, explicit QP101 section, preserved browser/resources/gallery/examples.
- Modify `site/styles.css`: docs shell, cards, QP101 section layout, and responsive styles using existing CSS conventions.
- Leave `site/app.js` unchanged unless verification reveals a formatting-only need; the fetch path must remain `fetch("qp101.schema.json")`.

### Task 1: Add Failing QP101 Site Contract Test

**Files:**
- Create: `rstim/tests/site_contract.rs`

**Interfaces:**
- Consumes: `_site/index.html`, `_site/app.js`, and copied `_site/` resources produced by `make build-site`.
- Produces: integration test `qp101_browser_resources_are_preserved`.

- [ ] **Step 1: Write the failing test**

Create `rstim/tests/site_contract.rs` with this content:

```rust
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn read_site_file(relative: &str) -> String {
    let path = repo_root().join("_site").join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn assert_site_file_exists(relative: &str) {
    let path = repo_root().join("_site").join(relative);
    assert!(Path::new(&path).is_file(), "missing built site file {}", path.display());
}

#[test]
fn qp101_browser_resources_are_preserved() {
    let index = read_site_file("index.html");
    let app = read_site_file("app.js");

    for marker in [
        "id=\"qp101\"",
        "href=\"#qp101\"",
        "href=\"qp101.schema.json\"",
        "href=\"QP101-ZY.md\"",
        "href=\"examples/basic.qp101.json\"",
        "href=\"examples/repeat-detector.qp101.json\"",
        "href=\"examples/atom-loss-sample.qp101.json\"",
        "id=\"schema-browser\"",
        "id=\"operations\"",
        "id=\"gallery\"",
        "id=\"examples\"",
        "src=\"gallery/basic-site.svg\"",
        "src=\"gallery/repeat-detector-site.svg\"",
        "src=\"gallery/atom-loss-sample.svg\"",
    ] {
        assert!(index.contains(marker), "built index is missing marker {marker}");
    }

    assert!(
        app.contains("fetch(\"qp101.schema.json\")"),
        "schema browser must keep fetching qp101.schema.json"
    );

    for relative in [
        "qp101.schema.json",
        "QP101-ZY.md",
        "examples/basic.qp101.json",
        "examples/repeat-detector.qp101.json",
        "examples/atom-loss-sample.qp101.json",
        "gallery/basic-site.svg",
        "gallery/repeat-detector-site.svg",
        "gallery/atom-loss-sample.svg",
    ] {
        assert_site_file_exists(relative);
    }
}
```

- [ ] **Step 2: Build the existing site**

Run:

```sh
make build-site
```

Expected: command exits 0 and refreshes `_site/`.

- [ ] **Step 3: Run the focused contract test and verify it fails**

Run:

```sh
cargo test -p rstim --test site_contract qp101_browser_resources_are_preserved -q
```

Expected: FAIL because the current built page does not yet expose a first-class `id="qp101"` area and `href="#qp101"` primary navigation link.

- [ ] **Step 4: Commit the failing test**

Run:

```sh
git add rstim/tests/site_contract.rs
git commit -m "test: preserve qp101 site browser resources"
```

Expected: commit succeeds with only the new test file staged.

### Task 2: Add Documentation Shell While Preserving QP101 Browser

**Files:**
- Modify: `site/index.html`
- Modify: `site/styles.css`

**Interfaces:**
- Consumes: the existing static site and the Task 1 contract markers.
- Produces: a single-page docs shell with an explicit first-class QP101 section.

- [ ] **Step 1: Update the document header and primary navigation**

In `site/index.html`, replace the current `<title>`, meta description, top nav, and hero with a broader documentation shell. Preserve the same static file names and add a `href="#qp101"` primary nav link.

The resulting header must contain these markers:

```html
<title>rstim Documentation</title>
<meta
  name="description"
  content="Documentation, QP101 schema browser, examples, and benchmark evidence for rstim."
>
<a class="brand" href="#docs-home" aria-label="rstim documentation home">
  <span class="brand-mark">r</span>
  <span>rstim docs</span>
</a>
<a href="#qp101">QP101</a>
```

- [ ] **Step 2: Add docs-home resource cards**

Still in `site/index.html`, add a first section with `id="docs-home"` and cards for:

```html
<section id="docs-home" class="docs-home" aria-label="rstim documentation overview">
```

The QP101 card in this section must include direct links to:

```html
<a href="qp101.schema.json" download>Schema</a>
<a href="QP101-ZY.md">Protocol</a>
```

- [ ] **Step 3: Wrap the existing QP101 content in a first-class section**

Move the current QP101 summary, schema browser, operation table, gallery, and examples under:

```html
<section id="qp101" class="qp101-section" aria-labelledby="qp101-title">
```

Inside that section, keep a QP101 heading and resource buttons with the existing direct links:

```html
<h2 id="qp101-title">QP101-ZY schema browser</h2>
<a class="button primary" href="qp101.schema.json" download>Download JSON Schema</a>
<a class="button" href="QP101-ZY.md">Read Protocol Draft</a>
<a class="button" href="examples/basic.qp101.json">View Example</a>
```

Do not change the IDs `schema-browser`, `operations`, `gallery`, or `examples`.

- [ ] **Step 4: Extend CSS for the shell**

In `site/styles.css`, add or update styles for:

```css
.top-nav {
  align-items: center;
  justify-content: space-between;
}

.brand,
.nav-links,
.docs-card-links,
.docs-home,
.docs-grid,
.qp101-section,
.qp101-kicker {
  display: flex;
  gap: 1rem;
}
```

Use the existing CSS variables and card radius conventions. Keep mobile breakpoints working by stacking `.top-nav`, `.nav-links`, `.docs-grid`, `.intro-grid`, `.example-grid`, `.schema-shell`, and `.gallery-card` on narrow screens.

- [ ] **Step 5: Rebuild and run the contract test**

Run:

```sh
make build-site
cargo test -p rstim --test site_contract qp101_browser_resources_are_preserved -q
```

Expected: both commands exit 0.

- [ ] **Step 6: Commit the site shell**

Run:

```sh
git add site/index.html site/styles.css
git commit -m "feat: preserve qp101 browser inside docs shell"
```

Expected: commit succeeds with only the static site files staged.

### Task 3: Final Verification

**Files:**
- No new files.

**Interfaces:**
- Consumes: Task 1 and Task 2 commits.
- Produces: verified branch ready for review and PR.

- [ ] **Step 1: Run issue verification**

Run:

```sh
make build-site
cargo test -p rstim --test site_gallery -q
cargo test -p rstim --test site_contract qp101_browser_resources_are_preserved -q
```

Expected: all commands exit 0.

- [ ] **Step 2: Confirm built assets**

Run:

```sh
test -f _site/qp101.schema.json
test -f _site/QP101-ZY.md
test -f _site/examples/basic.qp101.json
test -f _site/gallery/basic-site.svg
test -f _site/gallery/repeat-detector-site.svg
test -f _site/gallery/atom-loss-sample.svg
```

Expected: all commands exit 0.

- [ ] **Step 3: Run full requested cargo verification**

Run:

```sh
cargo test
```

Expected: command exits 0.

- [ ] **Step 4: Inspect the branch diff**

Run:

```sh
git status --short
git diff --stat master...HEAD
git diff --check master...HEAD
```

Expected: only intentional files changed and `git diff --check` exits 0.

## Plan Self Review

- Spec coverage: Task 1 covers the contract test; Task 2 covers the shell and
  QP101 preservation; Task 3 covers the issue verification commands.
- Completion scan: no unresolved markers or delayed implementation notes.
- Type consistency: the only new Rust item is
  `qp101_browser_resources_are_preserved`, matching the issue command.
