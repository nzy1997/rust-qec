# QP101 Circuit Gallery Design

## Goal

Add rendered circuit diagrams from `qp101-viz` to the QP101 website so the page shows concrete visual output in addition to schema and raw JSON examples.

## Scope

This change extends the static GitHub Pages site already added for QP101. It does not change the QP101 schema, the Rust exporter, or the `qp101-viz` renderer logic. It only reuses existing renderer entrypoints to generate static SVGs during site build.

## Chosen Direction

The site will embed three rendered examples:

- `basic`
- `repeat-detector`
- `atom-loss-sample`

The diagrams will be generated as SVG with Typst during `make build-site`. This keeps the website aligned with the current `qp101-viz` output instead of creating a second browser-side renderer.

## Asset Layout

Two tiny site-specific Typst wrappers will be added under `qp101-viz/examples/`:

- `basic-site.typ`
- `repeat-detector-site.typ`

`atom-loss-sample.typ` already exists and can be reused directly. The site build places the generated SVGs into `_site/gallery/`.

## Web Layout

The website will add a `Circuit Gallery` section before the JSON examples section. Each gallery item includes:

- title
- one-sentence explanation
- static SVG preview
- link to the corresponding `.qp101.json`
- link to the renderer source wrapper

This keeps the page narrative ordered: protocol summary, schema browser, rendered circuits, then downloadable example JSON.

## Build Impact

`make build-site` will now require Typst. The GitHub Pages workflow must therefore install Typst before running the build step.
