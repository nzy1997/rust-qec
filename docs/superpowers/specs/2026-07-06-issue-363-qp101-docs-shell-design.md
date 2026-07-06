# Issue 363 QP101 Docs Shell Design

## Goal

Add a broader `rstim` documentation-site shell while preserving the existing
QP101-ZY schema browser, resource URLs, examples, and gallery as a first-class
site area.

## Context

The current site is a single static QP101 page under `site/`. `make build-site`
copies `site/index.html`, `site/styles.css`, `site/app.js`,
`rstim/doc/qp101.schema.json`, `rstim/doc/QP101-ZY.md`, and three selected
QP101 examples into `_site/`, then generates gallery SVGs. `site/app.js`
fetches `qp101.schema.json` directly and renders the schema navigation.

Issue #363 requires the broader documentation shell to absorb this browser, not
replace it. The old direct URLs must remain valid:

- `qp101.schema.json`
- `QP101-ZY.md`
- `examples/basic.qp101.json`
- `examples/repeat-detector.qp101.json`
- `examples/atom-loss-sample.qp101.json`
- `gallery/basic-site.svg`
- `gallery/repeat-detector-site.svg`
- `gallery/atom-loss-sample.svg`

GitHub issue context beyond the Agent Desk issue body could not be fetched in
this sandbox because `gh` was blocked by the configured local proxy. The design
therefore treats the supplied issue body and local repository state as the
authoritative context.

## Approaches Considered

1. Single-page documentation shell with QP101 preserved in-page.
   - Pros: keeps `qp101.schema.json` fetch and existing resource URLs
     unchanged, avoids redirects, preserves the current schema browser code,
     and is enough for later site issues to add focused sections.
   - Cons: the page still carries all shell and QP101 content in one HTML file.

2. Multi-page shell with a dedicated QP101 page.
   - Pros: clearer future routing if the site grows substantially.
   - Cons: higher risk for this issue because old direct links and the browser
     fetch path need compatibility redirects or duplicate resource handling.

3. Data-driven site manifest.
   - Pros: reusable for a larger documentation portal.
   - Cons: outside this issue's preservation objective and likely to overlap
     sibling site issues.

The chosen approach is option 1. It is the smallest change that satisfies the
objective while making QP101 visibly first-class in the broader site shell.

## Site Design

`site/index.html` remains the only page. The document title and header become
`rstim Documentation`, with top-level navigation for docs home, QP101, CLI, and
benchmark evidence. The first viewport introduces `rstim` as a documentation
hub and includes compact resource cards for getting started, CLI workflows,
benchmark evidence, and QP101-ZY.

The QP101 area is an explicit `<section id="qp101">` and remains near the top of
the page. It keeps:

- schema download link to `qp101.schema.json`
- protocol draft link to `QP101-ZY.md`
- example links under `examples/*.qp101.json`
- schema browser section with `id="schema-browser"`
- operation table
- gallery examples and SVG image paths
- example download cards

The JavaScript continues to call `fetch("qp101.schema.json")`; no schema path or
export behavior changes.

## CSS Design

`site/styles.css` extends the existing plain CSS approach. It adds shell
layout, doc cards, and compact section grouping while keeping the browser,
operation table, gallery, and examples readable on desktop and mobile.

The design stays restrained and documentation-oriented: no frontend framework,
no new build step, no client-side router, and no dependency on external assets.

## Contract Test

Add `rstim/tests/site_contract.rs` with
`qp101_browser_resources_are_preserved`. The test assumes `make build-site` has
already produced `_site/`, matching the issue verification command. It checks:

- `_site/index.html` exists and exposes QP101 as a first-class area via
  `id="qp101"` and a `href="#qp101"` navigation link.
- `_site/index.html` still links to `qp101.schema.json`, `QP101-ZY.md`, the
  selected examples, and gallery SVGs.
- `_site/app.js` still fetches `qp101.schema.json`.
- copied resource files exist in `_site/`.

This makes the issue's negative control meaningful: removing the schema link
from the page or changing the browser fetch path fails the focused test.

## Out Of Scope

- Changing `rstim export_json` behavior.
- Changing `rstim/doc/qp101.schema.json`.
- Changing selected QP101 example JSON.
- Adding redirects, a router, or a multi-page build.
- Regenerating benchmark artifacts.

## Verification

Run:

```sh
make build-site
cargo test -p rstim --test site_gallery -q
cargo test -p rstim --test site_contract qp101_browser_resources_are_preserved -q
cargo test
```

Expected result: all commands exit 0, and `_site/` contains the QP101 schema,
protocol draft, selected examples, and gallery SVGs.

## Self Review

- Placeholder scan: no unresolved markers or incomplete sections.
- Consistency check: the chosen single-page architecture matches the preserved
  resource paths and the planned contract test.
- Scope check: the design is a focused static-site change plus one contract
  test.
- Ambiguity check: QP101 first-class status is defined by an explicit
  `id="qp101"` page area, a primary navigation link, and preserved schema
  links/fetches.
