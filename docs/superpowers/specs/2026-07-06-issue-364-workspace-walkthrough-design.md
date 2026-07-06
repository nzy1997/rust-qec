# Issue 364 Workspace Overview And Feature Walkthrough Design

## Goal

Extend the static `rstim` documentation site with compact sections that explain
how the workspace crates relate, then point readers to the existing runnable
showcase pages and workflow entry points before benchmark evidence.

## Context

The root `README.md` already maps the workspace and links the current showcase
pages. The site created by #363 is a single static page under `site/` with a
QP101 area, gallery, examples, and a focused contract test in
`rstim/tests/site_contract.rs`. Issue #364 depends on that shell and on the
site-level benchmark-evidence direction from #360: readers should see workspace
capabilities and feature walkthrough links before deeper evidence.

GitHub issue context beyond the Agent Desk issue body could not be fetched in
this sandbox because `gh` was blocked by the configured local proxy. The
provided issue body, local README/showcase docs, and merged #363 site state are
the authoritative context for this design.

## Approaches Considered

1. Compact single-page sections in `site/index.html`.
   - Pros: preserves existing QP101 resource URLs and `site/app.js`, keeps the
     static build unchanged, and matches the existing docs-shell pattern.
   - Cons: the landing page carries one more documentation area.

2. Add standalone overview and walkthrough HTML pages.
   - Pros: each topic could grow independently later.
   - Cons: adds routing/build surface not required by the issue and risks
     splitting readers away from the current shell.

3. Generate site cards from a manifest.
   - Pros: reusable if many showcase pages are added.
   - Cons: unnecessary for a small fixed set of links and adds another source
     of truth.

Chosen approach: option 1. It satisfies the issue with the smallest static-site
change and keeps the source of detailed commands in the existing Markdown
showcases.

## Site Design

`site/index.html` remains the only page. The top navigation adds links to:

- `#workspace-overview`
- `#feature-walkthroughs`
- the existing QP101, gallery, examples, and benchmark-evidence areas

Near the top of the page, add `section id="workspace-overview"` with compact
cards naming every workspace crate required by the issue:

- `rstim`: parser, sampler, detector-event/DEM extraction, SVG, and QP101
  front door.
- `rsinter`: benchmark orchestration and decoder experiment harness.
- `rmatching`: Rust MWPM decoder for detector-error-model workflows.
- `rbposd`: BP-OSD decoder component used by comparison flows.
- `rilpqec`: ILP-style decoder component used by comparison flows.
- `qec-code`: CSS construction and distance-search CLI workflows.
- `qec-ilp-core`: ILP-backed core checks used by QEC-code distance paths.

Add `section id="feature-walkthroughs"` immediately after the workspace
overview. It uses compact technical cards that link or name runnable entry
points for these workflow categories:

- circuit parsing and sampling
- detection and DEM extraction
- SVG/QP101 export
- decoder experiments
- CSS construction
- distance-search workflows

The section links the existing source pages instead of duplicating long command
blocks:

- `docs/showcases/rstim-cli-dem-pipeline.md`
- `docs/showcases/rstim-render-svg-atom-loss.md`
- `docs/showcases/qec-code-css-construction.md`
- `docs/showcases/benchmark-evidence.md`
- `docs/showcases/qec-code-random-window-benchmark.md`
- `docs/showcases/README.md`
- `rstim/doc/cli.md`

Add `section id="benchmark-evidence"` with compact links to benchmark evidence
and random-window evidence, satisfying the dependency direction that benchmark
material follows the workspace and walkthrough introduction. This section is a
navigation surface only; it does not add new benchmark claims or runs.

## CSS Design

Extend `site/styles.css` with reusable compact-card styles for overview,
walkthrough, and evidence sections. Reuse existing variables, 8px card radius,
grid layouts, and responsive breakpoints. Keep the page restrained and
documentation-oriented; no new framework, build dependency, images, or
JavaScript behavior is added.

## Contract Test

Extend `rstim/tests/site_contract.rs` with
`workspace_feature_walkthroughs_are_linked`. The test reads `site/index.html`
directly, matching the issue's focused contract target, and verifies:

- all required workspace crate names appear
- the workspace overview and feature walkthrough sections are present
- the required showcase links appear
- benchmark evidence or random-window evidence is linked
- each required workflow category is represented by section copy or link text
- representative runnable entry points are named, including `stats`,
  `sample`, `detect`, `analyze_errors`, `render_svg`, `export_json`,
  decoder/benchmark commands, `code css`, and random-window distance search

Negative controls become meaningful because removing `qec-code`, removing all
walkthrough links, or dropping one required workflow category fails the focused
test.

## Out Of Scope

- New feature implementations.
- New benchmark runs or generated benchmark artifacts.
- Changing QP101 schema/browser behavior.
- Adding a site generator, router, manifest, or JavaScript data model.
- Duplicating long showcase command blocks in HTML.

## Verification

Run:

```sh
make build-site
python3 tools/check_showcase_docs.py docs/showcases
cargo test -p rstim --test site_contract workspace_feature_walkthroughs_are_linked -q
cargo test
```

Expected result: all commands exit 0. The focused site contract fails if the
site omits any required workspace crate, removes the feature walkthrough links,
or omits one of the required workflow categories.

## Self Review

- Placeholder scan: no unresolved markers or incomplete sections.
- Consistency check: the chosen single-page design matches the existing site
  build and #363 contract.
- Scope check: the design is a focused static-site content update plus one
  contract test.
- Ambiguity check: required crates, links, workflow categories, and negative
  controls are named explicitly.
