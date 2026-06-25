# Issue 174 Built-In SVG Renderer Documentation Design

Date: 2026-06-25
Status: Design approved by Agent Desk standing policy
Scope: GitHub issue #174, user-facing documentation for the built-in `rstim render_svg` workflow

## Summary

Issue #174 updates the documentation now that the built-in SVG renderer covers
plain circuits, seeded sample-shot overlays, and DEM-origin highlight overlays.
The documented front door for static circuit visualization should be:

```sh
rstim render_svg --in circuit.stim --out circuit.svg
```

`export_json` remains documented as the QP101 structured-data export path for
downstream tooling and the legacy/prototype Typst package. The documentation
must not imply that users need the older QP101 JSON -> Typst workflow for
normal static SVG diagrams.

## Current State

The dependency issues for #174 are merged into `master`:

- issue #167 added `rstim render_svg --in/--out` and stdin/stdout rendering
- issue #172 added `--sample_shot --seed <n>` rendering through QP101 sample
  annotations
- issue #173 added `--highlight_dem_error <index>` rendering through QP101 DEM
  highlight annotations

Relevant current files:

- `README.md` lists `export_json` in common workflows and points atom-loss
  readers directly to `qp101-viz` examples.
- `rstim/doc/cli.md` documents `export_json`, but does not yet document
  `render_svg` as a generation/export command.
- `qp101-viz/README.md` already calls the Typst package a prototype, but its
  example section still reads as the primary visualization path.
- `rstim/tests/cli_render_svg.rs` already has CLI behavior coverage for plain
  render, sample-shot annotations, DEM highlights, invalid Stim input, invalid
  sample options, and invalid DEM queries.

The `.AGENTS` QP101 synchronization rules do not require updating
`rstim/doc/QP101-ZY.md` because this issue changes documentation and tests only;
it does not change QP101 JSON shape or semantics.

## Goals

- Document `render_svg` in `README.md` as the primary static circuit
  visualization path.
- Document `render_svg` in `rstim/doc/cli.md` with:
  - one plain file-output command
  - stdin-to-stdout behavior
  - one seeded sample-shot command
  - one DEM-highlight command
  - the documented `--seed` without `--sample_shot` error behavior
- Keep `export_json` documented as QP101 structured-data export for downstream
  processing, fixture generation, and the optional legacy/prototype Typst path.
- Update `qp101-viz/README.md` to describe Typst as optional prototype
  infrastructure now that the committed renderer examples are covered by the
  built-in CLI.
- Add a documentation-tracking CLI test named
  `render_svg_documented_workflow_matches_cli`.
- Include a negative control in that test that rejects stale command spelling
  such as `rstim svg_render`.
- Avoid promising coordinate-layout rendering or interactive browser editing.

## Non-Goals

- Do not change CLI behavior.
- Do not change QP101 JSON output.
- Do not switch the QP101 gallery or website build.
- Do not remove `qp101-viz/`.
- Do not write a long visualization tutorial beyond README and CLI reference
  updates.

## Approaches Considered

### 1. Document `render_svg` as primary, keep `export_json` as data export

Add concise workflow examples to the README and CLI reference. Reframe
`qp101-viz` as legacy/prototype infrastructure for QP101 JSON consumers. Add a
focused integration test that reads the docs, mirrors a documented plain command
against a known small circuit, checks one documented failure case, and asserts
that stale command names are absent.

Benefits:

- directly matches issue #174
- keeps docs aligned with the real CLI
- preserves QP101 export docs without making JSON the visualization front door
- avoids unrelated gallery or Typst churn

Costs:

- the doc test mirrors a documented command rather than parsing arbitrary shell
  examples, but it still protects the exact command shape and required prose
  anchors

This is the chosen approach.

### 2. Move all visualization docs out of README into a new tutorial

Create a larger visualization guide and link it from README and CLI docs.

Benefits:

- leaves room for screenshots and extended examples later

Costs:

- exceeds the issue's requested scope
- makes the required front-door examples less discoverable
- adds a tutorial before the gallery migration issue has landed

This is rejected.

### 3. Leave Typst docs as the main path and only add a `render_svg` note

Add a short reference entry for `render_svg` while keeping the README atom-loss
section centered on `export_json` and `qp101-viz`.

Benefits:

- smallest diff

Costs:

- conflicts with #33 and #174
- continues to teach the older JSON -> Typst path as the default visualization
  route

This is rejected.

## Documentation Design

`README.md` should:

- change the workspace map entry for `qp101-viz/` to say it is optional
  legacy/prototype QP101 Typst infrastructure
- add `render_svg` alongside `export_json` in common workflows
- rename or broaden the atom-loss section so it documents built-in SVG
  rendering first
- include runnable commands for:
  - plain file-output rendering
  - seeded sample-shot SVG rendering
  - DEM-highlight SVG rendering
- keep links to QP101 JSON and Typst examples as related/legacy material

`rstim/doc/cli.md` should:

- list `render_svg` under generation/export commands
- add a `## Render SVG diagrams with render_svg` section before
  `export_json`
- document file and stdout behavior:
  - `--in <path>` reads Stim input
  - omitting `--in` reads stdin
  - `--out <path>` writes SVG to a file
  - omitting `--out` writes SVG to stdout
- include the three required command examples:
  - `rstim render_svg --in circuit.stim --out circuit.svg`
  - `rstim render_svg --sample_shot --seed 7 --in circuit.stim --out sample.svg`
  - `rstim render_svg --highlight_dem_error 0 --in circuit.stim --out highlight.svg`
- document `--seed is only supported with --sample_shot`
- document that `--sample_shot` and `--highlight_dem_error` are mutually
  exclusive
- keep `export_json` as structured QP101 data export for downstream tools and
  optional Typst/`qp101-viz` workflows

`qp101-viz/README.md` should:

- keep the package docs intact
- add an early note that `rstim render_svg` is now the primary built-in static
  SVG renderer
- describe `qp101-viz` as optional legacy/prototype Typst infrastructure for
  QP101 JSON files
- avoid deleting examples or changing fixtures

## Test Design

Extend `rstim/tests/cli_render_svg.rs` with
`render_svg_documented_workflow_matches_cli`.

The test should:

- read `README.md` and `rstim/doc/cli.md`
- assert both files mention `render_svg`
- assert both files still mention `export_json`
- assert the docs contain the documented plain command
  `rstim render_svg --in circuit.stim --out circuit.svg`
- assert the docs contain `--sample_shot --seed 7`
- assert the docs contain `--highlight_dem_error 0`
- assert neither doc contains stale `rstim svg_render`
- run the mirrored plain command shape against a temporary
  `H 0\nCX 0 1\nTICK\nM 0\n` circuit and assert the produced output starts
  with `<svg` and contains expected markers such as `q0`, `H`, and `M`
- run `rstim render_svg --seed 7 --out <existing>` with stdin input and assert
  the documented error contains `--seed is only supported with --sample_shot`
  and the existing output file is unchanged

Run the issue verification command:

```sh
cargo test -p rstim --test cli_render_svg render_svg_documented_workflow_matches_cli -q
```

Also run broad verification:

```sh
cargo test
git diff --check
```
