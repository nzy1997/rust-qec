# Website Redesign: Zola Multi-Page Site

**Date:** 2026-07-13
**Status:** Approved design, pending implementation plan

## Problem

The current Pages site (`site/index.html` + `styles.css` + `app.js`, deployed to
<https://nzy1997.github.io/rstim/>) is a single page with ten anchor sections.
Three problems motivate the redesign:

1. **Structure/navigation** — one long page with a 10-link anchor nav is hard
   to navigate.
2. **Audience/messaging** — the page opens with "Static reference site" and
   gives top billing to the QP101 schema browser, which is insider material.
   The primary audience is *potential users*: QEC researchers and engineers
   deciding whether to adopt rstim.
3. **Content bloat** — sections have accreted (QP101, gallery, operations,
   evidence) with no tiering.

Visual polish is explicitly **not** a goal: the existing visual language
(styles.css) is kept.

## Decision Summary

- **Approach:** migrate to a static site generator, multi-page.
- **Generator:** Zola (single Rust binary, Tera templates, trivial CI install).
- **Old content:** nothing is cut; everything is demoted from the landing page
  to subpages.
- **Positioning:** the landing page leads with the "complete QEC workspace"
  story — simulator + decoders + benchmark harness in one Rust workspace.

## Page Map

```
/                    Landing page (the pitch)
/benchmarks/         Evidence + checked results + methodology + claims limits
/qp101/              QP101 schema browser + protocol draft + examples + gallery
/guide/              Workspace map + feature walkthroughs + operations reference
```

Shared top nav on every page: **Home · Guide · Benchmarks · QP101**.

### Landing page (`/`)

1. **Hero** — "A complete quantum error correction workspace in Rust":
   Stim-style simulator + MWPM/BP-OSD/ILP decoders + parallel benchmark
   harness, one `cargo build` away. Primary CTA "Get started" (anchors to the
   quick-start section); secondary CTA "See the benchmarks" (`/benchmarks/`).
2. **What's in the box** — one card per capability (simulate & sample
   circuits, extract DEMs, decode with three decoder families, run benchmark
   campaigns, render SVG diagrams), each linking into `/guide/` or `/qp101/`.
3. **Quick start** — minimal install + first-circuit snippet, sourced from the
   existing README/showcase material.
4. **Headline benchmark figure** — the surface-code decoder comparison plot
   with one carefully-worded sentence and a link to `/benchmarks/`.
5. **Footer** — GitHub, docs subpages.

### Subpages

Content moves over reorganized, not rewritten:

- `/benchmarks/` absorbs the current benchmark-evidence, checked-results,
  methodology, and claims-limits sections, including the dynamic manifest and
  result cards rendered from `benchmark-site.json`.
- `/qp101/` absorbs the interactive schema browser, protocol draft link,
  example QP101 files, and the rendered gallery.
- `/guide/` absorbs the workspace overview, feature-walkthrough links, and the
  operations reference.

## Claims Discipline (constraint)

The site's benchmark claims have been deliberately narrowed and audited (see
`git log -- site`). This wording discipline carries over untouched:

- The landing page may only state what `/benchmarks/` substantiates.
- The methodology and claims-limits content moves verbatim (modulo layout)
  to `/benchmarks/`.
- No new performance claims are introduced during migration.

## Technical Architecture

### Zola layout

```
site/
  config.toml
  content/
    _index.md
    benchmarks/_index.md
    qp101/_index.md
    guide/_index.md
  templates/
    base.html          shared nav + footer
    index.html         landing page
    (per-section templates as needed)
  static/
    styles.css         ported from current site
    qp101-browser.js   split from app.js (schema browser part)
    benchmarks.js      split from app.js (benchmark cards part)
```

- **Custom templates, no third-party theme.** The existing CSS is ported;
  themes would fight the existing look and the interactive pages.
- `app.js` is split along its two existing element-guarded halves: the QP101
  schema browser (fetches `qp101.schema.json`) and the benchmark manifest /
  checked-results cards (fetches `data/benchmark-site.json`). Fetch paths
  must be revisited to work from subpage URLs (root-absolute paths).

### Build pipeline

`make build-site` becomes:

1. `zola build` with output to `_site`.
2. Copy static payloads into `_site` as today: `qp101.schema.json`,
   `QP101-ZY.md`, the three example `.qp101.json` files.
3. Run the two existing Python asset steps against `_site`:
   `tools/build_qp101_gallery.py` and `tools/copy_site_benchmark_data.py`.

(Generated assets land in `_site` after the Zola build rather than being
staged into `site/static/`, so no gitignore changes are needed and the
existing script CLIs are reused unchanged.)

`tools/check_site_build.py` remains the CI gate, updated to assert:

- the four pages exist at their new paths;
- the JSON payloads fetched by JS exist at the paths the JS uses;
- nav links between pages resolve.

### CI / deployment

`deploy-pages.yml` gains one step: install a **pinned** Zola release binary.
Everything else (`make build-site` → upload `_site` → `deploy-pages`) is
unchanged.

### URL compatibility

Old deep links of the form `/#qp101` will break as content moves to
`/qp101/`. Accepted risk (docs site, low inbound-link exposure). Mitigation:
the new pages keep the old section anchor IDs, so within-page deep links
(`/benchmarks/#checked-benchmark-results` etc.) still work.

## Out of Scope

- Visual redesign / new styling beyond what page restructuring requires.
- Rewriting walkthrough or methodology content.
- Cutting any existing content.
- Redirects from old anchor URLs.

## Verification

- Local: `make build-site && python3 tools/check_site_build.py _site`;
  `zola serve` for manual inspection of all four pages, the interactive
  schema browser, and the benchmark cards.
- CI: existing Pages workflow must pass with the Zola install step; the
  check script gates the build as before.
