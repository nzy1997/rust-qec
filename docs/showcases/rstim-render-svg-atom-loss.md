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

The DEM-highlight render contains the base circuit labels `q0`, `XE`, `M`, and
`DETECTOR`, then adds highlighted query-result annotations such as `marker: X`,
`marker: D0`, `data-annotation-tags="dem-origin query-result"`, and
`data-annotation-tags="dem-symptom query-result"`.

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

## Limits

This showcase covers the built-in static SVG renderer, the currently supported
sample-shot annotation path, and single detector-error-model error highlighting.
Sample-shot rendering supports the sampled events exercised by the focused CLI
tests, including fired noise branches, loss-caused measurement information,
measurement outcomes, and detector flips.

`--seed` is only supported with `--sample_shot`; it is not a standalone render
option. `--sample_shot` and `--highlight_dem_error` are mutually exclusive, so
render sample annotations and DEM-origin highlights in separate commands.

This page does not promise a full Pages gallery migration, a full Typst
replacement, or coverage for every historical `qp101-viz` fixture.
