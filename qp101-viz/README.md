# qp101-viz

`qp101-viz` is a local Typst package prototype for rendering QP101-ZY circuit
JSON as a timeline view. For normal static SVG output, prefer the built-in
`rstim render_svg` CLI:

```sh
rstim render_svg --in circuit.stim --out circuit.svg
```

For a committed atom-loss SVG workflow reference, see
[`rstim Render SVG Atom-Loss showcase`](docs/showcases/rstim-render-svg-atom-loss.md).

This package remains useful as optional legacy/prototype infrastructure for
Typst-specific workflows and direct QP101 JSON experiments.

## Scope

This first prototype focuses on:

- reading QP101-ZY JSON directly with `json(...)`
- rendering the ordered `operations` stream as a quill-based quantum circuit
- preserving `repeat`, `tick`, detectors, observables, and noise as distinct visual events
- assigning renderer-only global measurement anchors such as `m1`, `m2`, `m3`, ... in expanded visual order
- resolving `detector` and `observable_include` `rec[-k]` sources into those measurement anchors when possible

It renders a timeline view rather than a geometry-based layout.

## Public API

- `timeline-theme(...)`
- `qp101-timeline(doc, theme: timeline-theme())`
- `qp101-timeline-file(path, theme: timeline-theme())`

## Example

```typst
#import "../lib.typ": qp101-timeline-file, timeline-theme
#set page(width: auto, height: auto, margin: 10pt)

#qp101-timeline-file(
  "examples/repeat-detector.qp101.json",
  theme: timeline-theme(step_width: 5.6em),
)
```

For a sample-trace render that highlights atom loss, measurement outcomes, and
detector flips from one seeded shot, compile:

```sh
typst compile --root qp101-viz qp101-viz/examples/atom-loss-sample.typ /tmp/atom-loss-sample.pdf
```

The source circuit is
[`qp101-viz/examples/atom-loss-sample.stim`](qp101-viz/examples/atom-loss-sample.stim),
and the exported sample result is
[`qp101-viz/examples/atom-loss-sample.qp101.json`](qp101-viz/examples/atom-loss-sample.qp101.json).

For a denser end-to-end example, compile:

```sh
typst compile --root qp101-viz qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.typ /tmp/surface-code-rotated-memory-x-d3-r3-atom-loss.pdf
```

That file renders both the source circuit and a seeded sample shot for a
rotated surface-code memory-X circuit with `d=3`, `r=3`, sparse mixed noise
(`LOSS`, `X_ERROR`, `Z_ERROR`, `DEPOLARIZE1`, `DEPOLARIZE2`), and fixed sample
seed `7`.

To regenerate the bundled source `.stim` plus both JSON artifacts for that
showcase, run from the repository root:

```sh
cargo run -p rstim --example mixed_noise_showcase
```

## Repository Layout

- `examples/` keeps a small set of human-facing rendered demos.
- examples that are meant to render after copying the package are self-contained and read JSON from files bundled inside `examples/`.
- `checks/` holds metadata/query fixtures used to verify renderer behavior.

## Notes

- `repeat` blocks are expanded for display and rendered as dashed grouped regions labeled `repeat xN`.
- repeated iterations are separated with dashed slice markers inside the grouped region.
- `tick` stays explicit as a dedicated separator moment.
- the visible circuit now only uses real qubit wires labeled `q0`, `q1`, ...
- `qubit_coords` and `shift_coords` stay available in the JSON model but are intentionally hidden from the timeline view.
- the main gate track is drawn with Typst's `quill` package, following the same broad rendering model as the `yao-rs/visualization` reference.
- `R` and `RX` are rendered as lightweight reset boxes.
- `X_ERROR`, `Z_ERROR`, and `DEPOLARIZE1` render as compact single-qubit noise boxes with short labels such as `XE`, `ZE`, and `D1`, even when one op targets many qubits.
- `DEPOLARIZE2` renders as connected two-box noise gates, and each rendered pair carries its own parameter note above it.
- single-qubit measurement families such as `M`, `MX`, `MY`, `MR`, `ML`, and `MRL` are rendered as compact measurement boxes with plain-text anchors such as `m1` above the gate.
- pair/product/padding/herald measurement families (`MXX`, `MYY`, `MZZ`, `MPP`, `MPAD`, `HERALDED_ERASE`, and `HERALDED_PAULI_CHANNEL_1`) advance the same global measurement history used by Rust, so later `rec[-k]` sources resolve to the correct anchors.
- sample-trace annotations render inline on supported single-qubit measurements, including loss markers such as `1[L]` and `L=1 | M=1[L]`.
- measurement and detector/observable operators reserve extra horizontal space for their labels so dense timelines do not collide as easily.
- circuit-top measurement and Stim-style operator labels now share a single theme clearance value so they stay above the wire instead of drifting into gate bodies.
- `detector` and `observable_include` render inline on the circuit in a Stim-like single-wire box style.
- detector boxes use `DETECTOR` with a top label such as `D0 = m2*m1`.
- observable boxes use `OBS_INCLUDE(k)` with a top label such as `L0 *= m7`.
- detector and observable host wires follow Stim's rule: use the minimum resolved measurement-source qubit, otherwise fall back to the best available source qubit, otherwise `q0`.
- detector and `observable_include` display resolved anchors instead of raw `rec[-k]` when the referenced measurements exist in the current expanded history.
- detector and observable source text now lists each resolved anchor explicitly instead of compressing consecutive runs into `m7-m8` style ranges.
- non-`rec` sources remain textual, and unresolved `rec[...]` sources stay explicit as raw `rec[...]`.
- operation-local `annotations` now render per-target query markers on matched noise boxes, using `target_slots` plus `context.repeat_iterations` to disambiguate repeated and multi-source inputs.

## Verification

Previously verified locally with `typst 0.14.2` by compiling and querying:

- `examples/timeline.typ`
- `examples/rstim-fixture.typ`
- `examples/anchor-basic.typ`
- `checks/repeat-groups.typ`
- `checks/stim-operator-host-render.typ`
- local `examples/circuits/` renders such as `steane_x_basis_with_flags.json` and `surface_code_d3_with_flags.json` when those ignored local fixtures are available in the workspace
- query-based fixtures for:
  - measurement-history structure
  - wire layout and hidden metadata
  - detector / observable promotion into the main track
  - detector / observable source resolution
