# Issue 175 QP101 Gallery Built-In SVG Design

Date: 2026-06-25
Status: Design approved by Agent Desk standing policy
Scope: GitHub issue #175, switch the QP101 site gallery build from Typst to `rstim render_svg`

## Summary

Issue #175 migrates the generated QP101 site gallery from the legacy
`qp101-viz` Typst renderer to the supported Rust CLI renderer:

```sh
rstim render_svg --in circuit.stim --out circuit.svg
```

The site should still publish the existing QP101 JSON example downloads for
the basic, repeat-detector, and atom-loss sample examples. The gallery SVGs
should be generated from committed Stim source circuits, with the atom-loss
example using the seeded sample-shot renderer flags so the rendered SVG keeps
sample annotations visible.

## Current State

The dependency issues for #175 are merged into `master`:

- issue #167 added `rstim render_svg --in/--out`
- issue #170 added repeat group rendering
- issue #171 added compact QEC noise rendering
- issue #172 added `--sample_shot --seed <n>` rendering
- issue #174 documented `render_svg` as the primary static SVG workflow

Relevant current files:

- `Makefile` builds `_site/`, copies static files and JSON examples, then runs
  three `typst compile --format svg` commands for gallery SVGs.
- `.github/workflows/deploy-pages.yml` installs Typst before running
  `make build-site`.
- `qp101-viz/examples/atom-loss-sample.stim` already backs the sampled
  atom-loss example.
- `qp101-viz/examples/basic.qp101.json` and
  `qp101-viz/examples/repeat-detector.qp101.json` do not yet have committed
  Stim source files beside them.
- `site/index.html` still labels the gallery as rendered with `qp101-viz` and
  links each gallery card to Typst source.

No `AGENTS.md`, `CLAUDE.md`, or `GEMINI.md` repository instructions are present
in this checkout.

## Goals

- Make `make build-site` generate gallery SVGs through `cargo run -p rstim --bin
  rstim -- render_svg`.
- Add committed `.stim` source fixtures for the basic and repeat-detector
  gallery examples.
- Keep the existing QP101 JSON files available for site downloads.
- Update the Pages workflow so it installs Rust for the renderer path and no
  longer installs Typst.
- Update the gallery copy and links so the public site demonstrates the
  built-in renderer, not the Typst prototype path.
- Add a repository-level negative-control test that masks `typst` while leaving
  `cargo` available and verifies the gallery build path succeeds.
- Add invalid-fixture coverage proving the gallery build fails before replacing
  an existing SVG with misleading output.

## Non-Goals

- Do not delete or rewrite `qp101-viz/`.
- Do not add a JSON-input mode to `render_svg`.
- Do not change QP101 JSON schema or example-download names.
- Do not add interactive gallery behavior or coordinate-layout rendering.

## Approaches Considered

### 1. Add a small gallery build script and call `render_svg` per Stim source

Create `tools/build_qp101_gallery.py` as the explicit gallery-rendering entry
point. The script validates that each source fixture exists, runs the expected
`rstim render_svg` command for each gallery SVG, and relies on the CLI's
existing safe-output behavior so invalid input does not replace an existing
target. `Makefile` calls this script after copying static assets and JSON
examples.

Benefits:

- keeps the Makefile readable while preserving loud failures
- gives tests a narrow gallery build path to exercise without rebuilding the
  whole site each time
- avoids shell quoting drift for sample-shot flags
- makes the exact source-to-output mapping auditable

Costs:

- adds one small Python helper, but the repository already uses Python tools
  for QP101 validation

This is the chosen approach.

### 2. Inline all `cargo run ... render_svg` commands in `Makefile`

Replace each `typst compile` line with a direct `cargo run -p rstim --bin rstim
-- render_svg ...` command.

Benefits:

- smallest code diff
- easy to inspect in the Makefile

Costs:

- duplicates command shape and flags
- makes invalid-fixture testing awkward
- grows brittle as gallery variants add more flags

This is rejected.

### 3. Generate gallery SVGs from existing QP101 JSON

Teach the gallery build to render from committed `.qp101.json` examples instead
of Stim sources.

Benefits:

- would reuse the current example JSON files directly

Costs:

- conflicts with #175's recommendation to avoid burying a new
  `render_svg --from_json` input mode in this migration
- bypasses the user-facing CLI path documented by #174

This is rejected.

## Build Design

Add `tools/build_qp101_gallery.py` with a fixed manifest of gallery entries:

- `qp101-viz/examples/basic.stim` ->
  `_site/gallery/basic-site.svg` via plain `render_svg`
- `qp101-viz/examples/repeat-detector.stim` ->
  `_site/gallery/repeat-detector-site.svg` via plain `render_svg`
- `qp101-viz/examples/atom-loss-sample.stim` ->
  `_site/gallery/atom-loss-sample.svg` via
  `render_svg --sample_shot --seed 7`

The script accepts `--repo-root`, `--out-dir`, and `--rstim-cmd` options. The
defaults support the normal repository build, while tests can point `--out-dir`
at a temporary directory and reuse the Cargo-built test binary as `--rstim-cmd`.

For each entry, the script should:

- fail if the source fixture is missing
- create the gallery output directory
- run the renderer command with `--in <source>` and `--out <target>`
- propagate nonzero exit status and stderr from `rstim`
- leave existing output preservation to `rstim render_svg`, which renders into
  memory before replacing file output

The `Makefile` `build-site` target should keep copying site static files,
schema/protocol docs, and QP101 JSON examples, then call:

```sh
python3 tools/build_qp101_gallery.py --repo-root . --out-dir _site/gallery
```

## Fixture Design

Add two committed Stim fixtures beside the existing QP101 examples:

- `qp101-viz/examples/basic.stim`:
  `H 0`, `CX 0 1`, `TICK`, `M 0 1`
- `qp101-viz/examples/repeat-detector.stim`:
  qubit coordinates for q0-q2, a `REPEAT 2` block with two CNOTs, a tick, one
  measurement, one detector, and a final observable include

These fixtures intentionally mirror the existing JSON downloads without making
the gallery build depend on JSON input.

## Site And Workflow Design

Update `site/index.html` so the gallery heading says it is rendered with
`rstim render_svg`. Replace each Typst source link with a Stim source link.

Update `.github/workflows/deploy-pages.yml`:

- remove the Typst setup step
- install the Rust stable toolchain using the same `rustup toolchain install
  stable --profile minimal && rustup default stable` pattern used by CI
- add `Swatinem/rust-cache@v2` to keep the Pages build from recompiling from
  scratch on every run

## Test Design

Add `rstim/tests/site_gallery.rs` with two tests:

- `qp101_gallery_builds_without_typst` copies the site inputs into a temporary
  repository-shaped tree, masks `typst` with a failing executable at the front
  of `PATH`, runs `tools/build_qp101_gallery.py` with
  `--rstim-cmd <CARGO_BIN_EXE_rstim>`, and verifies the three SVGs exist,
  start with `<svg`, and contain visible renderer text such as `q0`, `H`,
  `repeat x2`, `iter 2`, `LOSS`, or `DETECTOR`.
- `qp101_gallery_invalid_fixture_does_not_replace_existing_svg` copies the
  fixtures into a temporary tree, writes invalid Stim into one source, creates
  an existing target SVG, runs the script against that target directory, and
  verifies the script fails and the existing SVG text is unchanged.

Also keep the issue's manual verification:

```sh
make build-site
python3 tools/validate_qp101_schema.py _site/qp101.schema.json _site/examples/basic.qp101.json _site/examples/repeat-detector.qp101.json _site/examples/atom-loss-sample.qp101.json
find _site/gallery -maxdepth 1 -type f -name '*.svg' -print
cargo test
```
