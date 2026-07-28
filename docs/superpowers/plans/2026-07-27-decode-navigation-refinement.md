# Decode and Navigation Refinement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Merge decoder benchmarks into Decode, enlarge the two comparison plots, simplify the home hero, and show the active page in the top navigation.

**Architecture:** Keep the existing Zola section-template structure. The shared base template reads a `section.extra.nav` key from each section's `_index.md`; Decode owns both full benchmark plots and campaign evidence, while the obsolete benchmark-campaign route is removed from content, templates, validators, and fixtures.

**Tech Stack:** Zola/Tera templates, static CSS and JavaScript, Rust site-contract tests, Python build/manifest validators.

## Global Constraints

- Delete `/benchmark-campaigns/`; do not add a redirect.
- Keep benchmark evidence manifest-backed through `site/static/js/benchmarks.js`.
- Show each decoder comparison plot at full content width on its own row.
- Remove both home hero buttons.
- Preserve keyboard-visible navigation and add `aria-current="page"` to the active item.
- Keep the local preview at `http://127.0.0.1:8000/`.

---

### Task 1: Define the merged-site contract

**Files:**
- Modify: `rstim/tests/site_contract.rs`
- Modify: `tools/test_check_site_build.py`
- Modify: `tools/test_check_site_manifest.py`

**Interfaces:**
- Consumes: existing static template and built-site contract helpers.
- Produces: failing tests for one Decode route, no Bench route, no home buttons, active navigation, and full-width decoder evidence.

- [ ] **Step 1: Update the Rust source contract before production templates**

Require the combined site source to contain decoder and campaign anchors in `site/templates/decoding.html`, require `class="decoder-evidence"`, and require active-navigation markers in `site/templates/base.html`. Assert that `site/templates/index.html` does not contain `Explore the workspace`, `RSMP v1 showcase`, or `href="benchmark-campaigns/"`, and that the base template does not contain `>Bench</a>`.

- [ ] **Step 2: Update Python fixture expectations before production validators**

Remove `benchmark-campaigns/index.html` from valid fixture creation. Assign `surface-decoder-local-smoke` and `bb-circuit-local-readiness` to the fixture's Decode page together with full decoder evidence. Add fixture assertions for active navigation and absence of the deleted route.

- [ ] **Step 3: Run focused tests and verify RED**

Run:

```bash
cargo test -p rstim --test site_contract
python3 -m unittest tools.test_check_site_build tools.test_check_site_manifest
```

Expected: failures identify the still-present Bench route/card/buttons, missing Decode campaign anchor, missing active navigation, or missing full-width decoder evidence class.

---

### Task 2: Merge the user-facing pages and add active navigation

**Files:**
- Modify: `site/templates/index.html`
- Modify: `site/templates/base.html`
- Modify: `site/templates/decoding.html`
- Modify: `site/static/styles.css`
- Modify: `site/content/_index.md`
- Modify: `site/content/simulator/_index.md`
- Modify: `site/content/detector-models/_index.md`
- Modify: `site/content/decoding/_index.md`
- Modify: `site/content/css-codes/_index.md`
- Modify: `site/content/rsmp-v1-showcase/_index.md`
- Modify: `site/content/qp101/_index.md`
- Delete: `site/content/benchmark-campaigns/_index.md`
- Delete: `site/templates/benchmark-campaigns.html`

**Interfaces:**
- Consumes: `section.extra.root` and manifest item IDs rendered by `benchmarks.js`.
- Produces: `section.extra.nav` values `home`, `simulate`, `dems`, `decode`, `css`, `rsmp`, and `qp101`; `.nav-link.current`; `.decoder-evidence`.

- [ ] **Step 1: Remove duplicated home and navigation UI**

Delete the home hero `<div class="actions">` and the Run benchmark campaigns card. Remove the Bench link from `base.html`.

- [ ] **Step 2: Add deterministic active-navigation state**

Add `nav = "..."` to each section's `[extra]` block. In `base.html`, compute:

```tera
{% set current_nav = section.extra.nav | default(value="home") %}
```

Give each navigation link `class="nav-link{% if current_nav == "decode" %} current{% endif %}"` and conditionally add `aria-current="page"` for the matching key.

- [ ] **Step 3: Merge campaign content into Decode**

Move the campaign description, smoke commands, campaign table, and evidence IDs `surface-decoder-local-smoke bb-circuit-local-readiness` into `decoding.html`. Preserve `id="decoder-families"` and add `id="benchmark-campaigns"` on the merged page.

- [ ] **Step 4: Make decoder plots full-width**

Add `decoder-evidence` to the full-results evidence container and add:

```css
.decoder-evidence .result-card.has-plot {
  grid-template-columns: minmax(0, 1fr);
}

.decoder-evidence .result-card-plot,
.decoder-evidence .result-plot img {
  width: 100%;
}

.nav-link.current {
  color: var(--accent-dark);
  text-decoration: underline;
  text-decoration-thickness: 2px;
  text-underline-offset: 0.35rem;
}
```

- [ ] **Step 5: Delete the obsolete route files**

Delete `site/content/benchmark-campaigns/_index.md` and `site/templates/benchmark-campaigns.html` without adding a redirect.

---

### Task 3: Align validators with the merged architecture

**Files:**
- Modify: `tools/check_site_build.py`
- Modify: `tools/check_site_manifest.py`
- Modify: `tools/test_check_site_build.py`
- Modify: `tools/test_check_site_manifest.py`
- Modify: `rstim/tests/site_contract.rs`

**Interfaces:**
- Consumes: the seven-route source layout and Decode evidence assignments.
- Produces: validators that reject reintroducing the deleted route and ensure every manifest evidence item remains assigned.

- [ ] **Step 1: Remove the benchmark route from required page lists**

Delete `benchmark-campaigns/index.html` from `PAGE_FILES`, `PAGE_REQUIRED_ANCHORS`, evidence-page lists, artifact-reference scans, and test fixture directories.

- [ ] **Step 2: Move campaign expectations to Decode**

Require both `decoder-families` and `benchmark-campaigns` anchors on `decoding/index.html`. Move local smoke/readiness assignments and methodology phrases into the Decode fixture.

- [ ] **Step 3: Run focused tests and verify GREEN**

Run:

```bash
cargo test -p rstim --test site_contract
python3 -m unittest tools.test_check_site_build tools.test_check_site_manifest
```

Expected: 9 Rust site-contract tests and all Python checker tests pass with zero failures.

---

### Task 4: Rebuild and refresh the local preview

**Files:**
- Rebuild: `_site/`

**Interfaces:**
- Consumes: validated Zola source, QP101 assets, and `site/benchmark-site.json`.
- Produces: a checked static site served at `http://127.0.0.1:8000/`.

- [ ] **Step 1: Build a fresh staging site**

Run Zola into `_site-next`, copy QP101 resources, render the gallery, and copy benchmark artifacts using the repository's existing build commands.

- [ ] **Step 2: Validate the complete build**

Run:

```bash
python3 tools/check_site_build.py _site-next
```

Expected: `SUMMARY: PASS` with zero failures.

- [ ] **Step 3: Replace the served build and verify HTTP**

Swap `_site-next` into `_site`, then run:

```bash
curl -fsS -o /dev/null -w 'HTTP %{http_code}\n' http://127.0.0.1:8000/
```

Expected: `HTTP 200`.

- [ ] **Step 4: Refresh the local preview**

Reload the existing local preview if browser policy permits; otherwise leave the verified server running and provide the exact local URL for manual refresh.
