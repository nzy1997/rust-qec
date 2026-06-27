# Rstim Render SVG Atom-Loss Showcase

This showcase demonstrates the built-in `rstim render_svg` path for static SVG
circuit diagrams, seeded atom-loss sample-shot annotations, and
detector-error-model source highlighting.

## What This Shows

`rstim render_svg` turns a Stim-like circuit into an SVG without requiring the
older Typst rendering path. The plain command is useful for a quick visible
circuit diagram, `--sample_shot --seed 7` overlays deterministic sampled events,
and `--highlight_dem_error 0` marks the source and symptom locations for one
detector-error-model term.

The atom-loss sample uses the committed
[`qp101-viz/examples/atom-loss-sample.stim`](qp101-viz/examples/atom-loss-sample.stim)
fixture because it is intentionally small and exercises loss, lost-data
measurement, and detector annotations. `qp101-viz` remains optional
legacy/prototype context for users who need the older Typst fixture path.

The larger surface-code preview uses the committed
[`qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.stim`](qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.stim)
fixture to show the same sample-shot overlay path on a distance-3, round-3
rotated-memory circuit.

## Run It

From the repository root, render a plain inline circuit:

```sh
printf 'H 0\nCX 0 1\nTICK\nM 0\n' | rstim render_svg > /tmp/rstim-plain.svg
```

Render the committed atom-loss sample with deterministic sample-shot
annotations:

```sh
rstim render_svg \
  --sample_shot \
  --seed 7 \
  --in qp101-viz/examples/atom-loss-sample.stim \
  --out /tmp/rstim-atom-loss-sample.svg
```

Render the distance-3, round-3 surface-code atom-loss sample with the same
deterministic seed:

```sh
rstim render_svg \
  --sample_shot \
  --seed 7 \
  --in qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.stim \
  --out /tmp/rstim-surface-code-d3-r3-atom-loss.svg
```

Render a compact DEM-origin highlight example:

```sh
printf 'X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n' | \
  rstim render_svg --highlight_dem_error 0 > /tmp/rstim-dem-highlight.svg
```

## Expected Result

Each command writes an SVG file whose text starts with `<svg`.

The plain render contains visible circuit labels such as `q0`, `H`, `CX`, and
`M`. The atom-loss sample render contains the compact noise and measurement
labels `>D1</text>`, `>LOSS</text>`, `>M</text>`, `>MRL</text>`, and
`>DETECTOR</text>`, plus seeded sample-shot annotation text such as `marker: X`,
`marker: L`, `marker: 1[L]`, `marker: L=1 | M=1[L]`, and `marker: D0`.

The surface-code atom-loss render is a wider SVG with 17 qubit wires, repeated
measurement rounds, `LOSS` operations, seeded sample markers, detector boxes,
and an `OBS_INCLUDE(0)` marker.

The DEM-highlight render contains the base circuit labels `q0`, `XE`, `M`, and
`DETECTOR`, then adds highlighted query-result annotations such as `marker: X`,
`marker: D0`, `data-annotation-tags="dem-origin query-result"`, and
`data-annotation-tags="dem-symptom query-result"`.

## Visual Preview

The compact checked-in preview below is generated from the committed atom-loss
sample with `--sample_shot --seed 7`.

![Seeded atom-loss sample-shot SVG render](assets/atom-loss-sample-seed7.svg)

The wider checked-in preview uses the same seeded sample-shot path on the
surface-code distance-3, round-3 atom-loss fixture.

![Seeded surface-code d=3 r=3 atom-loss sample-shot SVG render](assets/surface-code-d3-r3-atom-loss-seed7.svg)

## Code

- [`rstim/doc/cli.md`](rstim/doc/cli.md) documents `render_svg`,
  `--sample_shot --seed 7`, and `--highlight_dem_error 0`.
- [`rstim/tests/cli_render_svg.rs`](rstim/tests/cli_render_svg.rs) verifies the
  CLI paths, deterministic sample-shot SVG annotations, DEM-origin highlight
  markers, and the `--seed` negative control.
- [`rstim/tests/qp101_svg.rs`](rstim/tests/qp101_svg.rs) verifies renderer
  details such as SVG labels, noise boxes, annotation styles, detector source
  labels, and viewBox behavior.
- [`qp101-viz/examples/atom-loss-sample.stim`](qp101-viz/examples/atom-loss-sample.stim)
  is the small committed atom-loss input used by the sample-shot command.
- [`qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.stim`](qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.stim)
  is the committed surface-code atom-loss input used by the wide sample-shot
  preview.

## Verification

Validate this showcase page:

```sh
python3 tools/check_showcase_docs.py docs/showcases/rstim-render-svg-atom-loss.md
```

Run the focused CLI and renderer coverage:

```sh
cargo test -p rstim --test cli_render_svg --test qp101_svg -q
```

Expected success criteria: the checker prints an `ok:` line for this page, and
the focused Rust tests pass. Those Rust tests cover plain file/stdin SVG output,
`--sample_shot --seed 7`, `--highlight_dem_error 0`, mutual exclusion between
sample-shot and highlight modes, and the negative control where
`rstim render_svg --seed 7` without `--sample_shot` fails with
`--seed is only supported with --sample_shot`.

The preview SVG committed with this page can be regenerated with:

```sh
cargo run -q -p rstim --bin rstim -- render_svg \
  --sample_shot \
  --seed 7 \
  --in qp101-viz/examples/atom-loss-sample.stim \
  --out docs/showcases/assets/atom-loss-sample-seed7.svg

cargo run -q -p rstim --bin rstim -- render_svg \
  --sample_shot \
  --seed 7 \
  --in qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.stim \
  --out docs/showcases/assets/surface-code-d3-r3-atom-loss-seed7.svg
```

## Limits

This showcase covers the built-in static SVG renderer, the currently supported
sample-shot annotation path, and single detector-error-model error highlighting.
Sample-shot rendering supports the sampled events exercised by the focused CLI
tests, including fired noise branches, loss-caused measurement information,
measurement outcomes, and detector flips.

The surface-code distance-3, round-3 preview is intentionally wide. It is meant
as a visual artifact for inspection in Markdown or a browser, not as a compact
thumbnail.

`--seed` is only supported with `--sample_shot`; it is not a standalone render
option. `--sample_shot` and `--highlight_dem_error` are mutually exclusive, so
render sample annotations and DEM-origin highlights in separate commands.

This page does not promise a full Pages gallery migration, a full Typst
replacement, or coverage for every historical `qp101-viz` fixture.
