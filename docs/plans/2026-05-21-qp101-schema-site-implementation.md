# QP101 Schema Site Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a downloadable QP101-ZY JSON Schema and a static GitHub Pages site that explains and visualizes it.

**Architecture:** Keep the Rust exporter unchanged. Add a structural schema under `rstim/doc/`, a plain static site under `site/`, a `make build-site` target that emits `_site/`, and a GitHub Pages workflow that deploys `_site/`.

**Tech Stack:** JSON Schema draft 2020-12, HTML, CSS, vanilla JavaScript, Make, GitHub Actions Pages.

---

### Task 1: Add The QP101 JSON Schema

**Files:**
- Create: `rstim/doc/qp101.schema.json`

**Step 1: Write schema asset**

Create `rstim/doc/qp101.schema.json` with:

- `$schema`: `https://json-schema.org/draft/2020-12/schema`
- `$id`: `https://nzy1997.github.io/rstim/qp101.schema.json`
- top-level required fields: `standard`, `version`, `num_qubits`, `operations`
- `$defs` for `operation`, each operation type, `targetRef`, each target-ref kind, `annotation`, `annotationStyle`, `display`, `metadata`, and `jsonValue`
- `additionalProperties: true` on protocol objects so tool extensions remain legal

**Step 2: Verify JSON syntax**

Run: `python3 -m json.tool rstim/doc/qp101.schema.json >/tmp/qp101.schema.pretty.json`

Expected: exit 0.

**Step 3: Validate an example fixture**

Run: `python3 tools/validate_qp101_schema.py rstim/doc/qp101.schema.json qp101-viz/examples/basic.qp101.json`

Expected: exit 0.

### Task 2: Link The Schema From QP101 Documentation

**Files:**
- Modify: `rstim/doc/QP101-ZY.md`

**Step 1: Add schema availability text**

Add a short section after the top-level field list explaining:

- `qp101.schema.json` is the machine-readable structural schema for draft v1.0
- it is intended for download and automated validation
- semantic validation rules remain in the markdown specification

**Step 2: Re-read the edited section**

Run: `sed -n '40,120p' rstim/doc/QP101-ZY.md`

Expected: section mentions `qp101.schema.json` and does not claim the schema validates cross-document semantic rules.

### Task 3: Add Static Site Source

**Files:**
- Create: `site/index.html`
- Create: `site/styles.css`
- Create: `site/app.js`

**Step 1: Build the HTML**

Create a single-page document with:

- header title `QP101-ZY`
- schema download link to `qp101.schema.json`
- protocol link to `QP101-ZY.md`
- schema browser container
- operation type table
- two example blocks

**Step 2: Build the CSS**

Use a quiet documentation/tool style. Avoid heavy decorative gradients. Ensure:

- responsive two-column schema browser on desktop
- single-column layout on mobile
- text wraps inside controls and code blocks
- buttons use stable dimensions and readable contrast

**Step 3: Build the JavaScript**

Implement:

- fetch `qp101.schema.json`
- collect browser nodes from top-level `properties` and `$defs`
- render navigation buttons
- render selected node type, required fields, properties, enum/const values, and description
- show an inline error if schema loading fails

### Task 4: Add Site Build Target

**Files:**
- Modify: `Makefile`

**Step 1: Add phony target**

Add `build-site` to `.PHONY` and help output.

**Step 2: Implement build-site**

The target should:

- remove `_site`
- create `_site/examples`
- copy `site/index.html`, `site/styles.css`, and `site/app.js`
- copy `rstim/doc/qp101.schema.json` to `_site/qp101.schema.json`
- copy `rstim/doc/QP101-ZY.md` to `_site/QP101-ZY.md`
- copy `qp101-viz/examples/basic.qp101.json` and `qp101-viz/examples/repeat-detector.qp101.json` to `_site/examples/`

**Step 3: Run the target**

Run: `make build-site`

Expected: exit 0 and `_site/index.html`, `_site/qp101.schema.json`, `_site/QP101-ZY.md`, and `_site/examples/basic.qp101.json` exist.

### Task 5: Add GitHub Pages Deployment Workflow

**Files:**
- Create: `.github/workflows/deploy-pages.yml`

**Step 1: Add workflow**

Create a workflow triggered by:

- push to `master`
- manual `workflow_dispatch`

Use:

- `actions/checkout@v4`
- `actions/configure-pages@v5`
- `make build-site`
- `actions/upload-pages-artifact@v3` with path `_site`
- `actions/deploy-pages@v4`

Set permissions:

- `contents: read`
- `pages: write`
- `id-token: write`

**Step 2: Inspect workflow syntax**

Run: `sed -n '1,220p' .github/workflows/deploy-pages.yml`

Expected: workflow includes the build, upload, and deploy steps.

### Task 6: Verify The Site Locally

**Files:**
- Test generated output under `_site/`

**Step 1: Build the site**

Run: `make build-site`

Expected: exit 0.

**Step 2: Validate copied schema JSON**

Run: `python3 -m json.tool _site/qp101.schema.json >/tmp/qp101-site-schema.pretty.json`

Expected: exit 0.

**Step 3: Validate examples against copied schema**

Run: `python3 tools/validate_qp101_schema.py _site/qp101.schema.json _site/examples/basic.qp101.json _site/examples/repeat-detector.qp101.json`

Expected: exit 0.

**Step 4: Smoke test static serving**

Run: `python3 -m http.server 8765 --directory _site`

Expected: server starts. In a separate command run `curl -fsSL http://127.0.0.1:8765/ >/tmp/qp101-site.html` and `curl -fsSL http://127.0.0.1:8765/qp101.schema.json >/tmp/qp101-site.schema.json`, then stop the server.

Expected: both curl commands exit 0.
