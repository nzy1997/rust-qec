# QP101 Circuit Gallery Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add three `qp101-viz` circuit renders to the QP101 website and generate them automatically during `make build-site`.

**Architecture:** Reuse `qp101-viz` Typst entrypoints to render SVGs into `_site/gallery/`, then add a static gallery section in the site HTML that references those generated assets and their matching example JSON files.

**Tech Stack:** Typst SVG export, HTML, CSS, Make, GitHub Actions Pages.

---

### Task 1: Add Missing Typst Wrappers

**Files:**
- Create: `qp101-viz/examples/basic-site.typ`
- Create: `qp101-viz/examples/repeat-detector-site.typ`

**Step 1: Write the failing check**

Run: `test -f qp101-viz/examples/basic-site.typ`

Expected: FAIL because the site-specific wrapper does not exist yet.

**Step 2: Add minimal wrappers**

Create two tiny Typst files that import `../lib.typ`, set auto-sized page output, and render:

- `examples/basic.qp101.json`
- `examples/repeat-detector.qp101.json`

**Step 3: Verify the wrappers compile**

Run:

- `typst compile --format svg --root qp101-viz qp101-viz/examples/basic-site.typ /tmp/basic-site.svg`
- `typst compile --format svg --root qp101-viz qp101-viz/examples/repeat-detector-site.typ /tmp/repeat-detector-site.svg`

Expected: both commands exit 0.

### Task 2: Extend Site Build Output

**Files:**
- Modify: `Makefile`
- Modify: `.github/workflows/deploy-pages.yml`

**Step 1: Write the failing check**

Run:

- `make build-site`
- `test -f _site/gallery/basic-site.svg`

Expected: first command succeeds, second command fails because gallery assets are not built.

**Step 2: Implement build-site gallery generation**

Update `build-site` to:

- create `_site/gallery`
- copy `qp101-viz/examples/atom-loss-sample.qp101.json` to `_site/examples/`
- run `typst compile --format svg` for:
  - `qp101-viz/examples/basic-site.typ`
  - `qp101-viz/examples/repeat-detector-site.typ`
  - `qp101-viz/examples/atom-loss-sample.typ`
- write outputs to `_site/gallery/basic-site.svg`, `_site/gallery/repeat-detector-site.svg`, and `_site/gallery/atom-loss-sample.svg`

**Step 3: Install Typst in Pages workflow**

Add a workflow step before `make build-site` that installs Typst in GitHub Actions.

**Step 4: Verify the build passes**

Run:

- `make build-site`
- `find _site/gallery -maxdepth 1 -type f -print`

Expected: the three SVG files exist.

### Task 3: Add The Gallery Section To The Website

**Files:**
- Modify: `site/index.html`
- Modify: `site/styles.css`

**Step 1: Write the failing check**

Run: `rg -n "Circuit Gallery|gallery/basic-site.svg|gallery/repeat-detector-site.svg|gallery/atom-loss-sample.svg" site/index.html`

Expected: FAIL because the gallery markup is not present yet.

**Step 2: Add gallery markup**

Insert a `Circuit Gallery` section before `Examples` with three cards:

- basic
- repeat-detector
- atom-loss-sample

Each card should include preview image, short explanation, and links to example JSON plus Typst source.

**Step 3: Add gallery styling**

Add CSS for:

- responsive gallery layout
- framed SVG preview surface
- image sizing that preserves aspect ratio and prevents overflow
- compact metadata links

**Step 4: Verify the source now references the gallery**

Run: `rg -n "Circuit Gallery|gallery/basic-site.svg|gallery/repeat-detector-site.svg|gallery/atom-loss-sample.svg" site/index.html`

Expected: PASS.

### Task 4: Verify End-To-End Site Output

**Files:**
- Test generated `_site/`

**Step 1: Build the site**

Run: `make build-site`

Expected: exit 0.

**Step 2: Confirm generated assets**

Run:

- `find _site/gallery -maxdepth 1 -type f -print`
- `python3 tools/validate_qp101_schema.py _site/qp101.schema.json _site/examples/basic.qp101.json _site/examples/repeat-detector.qp101.json _site/examples/atom-loss-sample.qp101.json`

Expected: the three SVGs exist and the example JSON files still validate.

**Step 3: Smoke test local serving**

Run: `python3 -m http.server 8765 --directory _site`

In separate commands:

- `curl -fsSL http://127.0.0.1:8765/ >/tmp/qp101-site.html`
- `curl -fsSL http://127.0.0.1:8765/gallery/basic-site.svg >/tmp/basic-site.svg`
- `curl -fsSL http://127.0.0.1:8765/gallery/repeat-detector-site.svg >/tmp/repeat-detector-site.svg`
- `curl -fsSL http://127.0.0.1:8765/gallery/atom-loss-sample.svg >/tmp/atom-loss-sample.svg`

Expected: all commands exit 0.
