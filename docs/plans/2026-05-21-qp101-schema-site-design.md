# QP101 Schema Site Design

## Goal

Publish QP101-ZY as a small, downloadable, and browsable web asset without changing the current Rust exporter shape.

## Scope

This work updates the QP101 documentation layer only. It adds a JSON Schema for the existing draft format, links that schema from the protocol document, and creates a static GitHub Pages site that introduces QP101-ZY and visualizes the schema.

It does not change `rstim/src/qp101.rs`, the CLI `export_json` output, or the Typst renderer behavior.

## Architecture

The repository will carry three publishable assets:

- `rstim/doc/qp101.schema.json`: structural JSON Schema for QP101-ZY draft v1.0.
- `site/`: static source files for the GitHub Pages page.
- `_site/`: generated build output produced by `make build-site`.

The site is plain HTML, CSS, and JavaScript. It uses relative URLs so it works under a GitHub Pages project path such as `/rstim/`. The JavaScript fetches the schema and renders a compact browser over the top-level document, operation definitions, target-reference definitions, and annotation definitions.

## Schema Boundary

The schema validates the stable draft surface described in `rstim/doc/QP101-ZY.md`:

- top-level fields: `standard`, `version`, `num_qubits`, `operations`, `metadata`, and `extensions`
- operation types: `gate`, `repeat`, `tick`, `qubit_coords`, `shift_coords`, `detector`, `observable_include`, `noise`, and `annotation`
- target-reference kinds: `qubit`, `rec`, `pauli`, `combiner`, and `sweep`
- operation-local annotations and annotation styles

The schema remains extensible. It allows additional fields on protocol objects and treats `metadata`, `extensions`, `annotation.context`, and other tool-specific payloads as ordinary JSON objects. It captures structural requirements but does not attempt semantic validation that JSON Schema cannot express cleanly, such as checking a qubit index against top-level `num_qubits`.

## Web Page

The page is a single tool-style document, not a marketing landing page. The first viewport introduces QP101-ZY, gives the schema download action, and links to the protocol document. The main page sections are:

- schema browser with a navigation list and field details
- operation type table with concise usage descriptions
- example JSON snippets for a minimal gate circuit and a Stim-style repeat/detector circuit

The schema browser should remain useful if JavaScript loads successfully and graceful if it fails: show an error message and keep the schema download link visible.

## Build And Deploy

`make build-site` generates `_site/` by copying static files from `site/`, the schema from `rstim/doc/`, the protocol markdown, and selected QP101 examples. The GitHub Actions workflow mirrors the TDAA-Go Pages approach: build `_site/`, upload it with `actions/upload-pages-artifact`, and publish it with `actions/deploy-pages`.

## Verification

Verification should include:

- `make build-site`
- schema validation against at least one committed `.qp101.json` example
- local static serving smoke check for `_site/index.html`
- confirmation that `_site/qp101.schema.json` exists and can be downloaded as a standalone asset
