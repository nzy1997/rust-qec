# Issue 166 QP101 SVG Renderer Design

Date: 2026-06-24
Status: Design approved by Agent Desk standing policy
Scope: GitHub issue #166, minimal QP101-to-SVG renderer module for `rstim`

## Summary

Issue #166 adds the first Rust-side SVG renderer for QP101 documents. The
renderer should live in `rstim`, consume `rstim::qp101::Qp101Document`, and
return a complete SVG document string through a small public function:

```rust
pub fn render_svg(doc: &Qp101Document) -> Result<String, String>;
```

This PR intentionally stops before CLI integration, measurement anchoring,
detector source resolution, repeat decorations, sample overlays, DEM highlight
overlays, or Typst parity. The objective is a deterministic SVG timeline that
proves the built-in path can draw wires, labels, gates, and ticks directly from
QP101.

## Current State

`rstim` already has:

- `rstim/src/qp101.rs` with `Qp101Document`, `Qp101Operation`, display labels,
  target references, annotations, and export helpers.
- `rstim/src/lib.rs` as the crate module surface.
- `rstim/tests/qp101_export.rs` and related fixtures that show how tests build
  QP101 documents.
- `rstim/tests/fixtures/qp101_svg/manifest.json` from issue #165, with semantic
  marker expectations for later renderer tests.
- `qp101-viz/` as the Typst reference renderer. Its README documents broad
  timeline semantics: wires labeled `q0`, `q1`, ticks as separators, coordinate
  metadata hidden from the timeline, simple gates, reset boxes, and fallback
  behavior for more complex operations.

The repository has no `AGENTS.md` in this checkout.

## Goals

- Add `rstim/src/qp101_svg.rs`.
- Export the module from `rstim/src/lib.rs`.
- Implement `render_svg(&Qp101Document) -> Result<String, String>`.
- Return an error naming `num_qubits` or qubits when `doc.num_qubits == 0`.
- Render a complete `<svg>` document with `xmlns`, `width`, `height`, and
  `viewBox`.
- Render qubit wires and `qN` labels for each qubit.
- Render simple single-qubit gate boxes for labels such as `H`, `X`, `Y`, `Z`,
  `S`, `T`, `R`, and `RX`.
- Render practical two-qubit forms for `CX`, `CZ`, and `SWAP`.
- Render unsupported but valid operations as visible fallback gate boxes or
  notes when possible.
- Render tick separators as visible vertical markers with a semantic `tick`
  label.
- Escape XML-sensitive user-visible labels from gate display labels and
  annotations.
- Add an integration test named
  `svg_renderer_draws_wires_gates_and_ticks` plus a zero-qubit negative control.

## Non-Goals

- Do not add CLI integration.
- Do not resolve measurement anchors, detector sources, or observable sources.
- Do not implement repeat group decorations or expanded repeat semantics.
- Do not implement sample-shot overlays or DEM-highlight overlays.
- Do not chase pixel-perfect Typst parity.
- Do not add a new SVG or XML dependency.

## Approaches Considered

### 1. Small hand-written SVG writer

Create a focused `qp101_svg` module with local layout constants and helper
functions for XML escaping, qubit validation, target extraction, and common
gate drawing.

Benefits:

- matches the issue recommendation
- keeps output deterministic for text-based tests
- avoids dependency churn
- keeps renderer behavior separate from QP101 export

Costs:

- future renderer phases may need to evolve the writer into a richer layout
  model

This is the chosen approach.

### 2. Serialize QP101 as text inside an SVG wrapper

Emit a basic SVG that contains QP101 JSON or debug text.

Benefits:

- very small implementation

Costs:

- fails the issue's requirement that the SVG contain real wire and gate
  elements
- does not prove the renderer path

This is rejected.

### 3. Add a third-party SVG builder

Use a Rust SVG/XML builder crate to construct the document.

Benefits:

- structured API for XML writing

Costs:

- unnecessary dependency for the minimal renderer
- conflicts with the issue recommendation to use a small hand-written writer

This is rejected.

## Renderer Behavior

The renderer should use deterministic local layout constants:

- left label margin
- right margin
- top margin
- lane gap
- column gap
- fixed gate box width and height

Every rendered operation that needs horizontal space gets a column. Metadata
operations such as `qubit_coords` and `shift_coords` do not draw timeline
columns. Ticks draw dashed vertical separators across all wires. Gates draw
visible boxes, connectors, controls, or swap marks on the target qubit lanes.

For gate labels, use `display.label` when present, otherwise use the QP101 gate
name. That ensures display labels are exercised by the escape path. Annotation
labels and text may be rendered as small escaped text near the operation.

The minimal common-gate rules are:

- single-qubit gates with one or more targets render as one labeled box per
  target
- `CX` and `CZ` with explicit `controls` and `targets` render a connector from
  the first control to the first target
- `CX` and `CZ` with only `targets` render adjacent target pairs as
  control-target pairs, matching current `export_qp101` output for `CX 0 1`
- `SWAP` with target pairs renders a connector and cross marks
- any other operation with visible qubit targets renders a generic fallback box
  spanning or marking the referenced lanes
- operations with no qubit lane render a top note instead of failing

Target references beyond `doc.num_qubits` are structurally invalid for layout
and should return a clear renderer error naming the invalid qubit and operation.

## Testing

Add `rstim/tests/qp101_svg.rs`.

The positive test should:

- build a small two-qubit QP101 document through `parse_lines` and
  `export_qp101`
- include `H`, `CX`, and `TICK`
- add or include a display label containing `A&B<test>`
- call `rstim::qp101_svg::render_svg`
- assert the SVG starts with `<svg`
- assert it contains `q0`, `q1`, `H`, `CX`, and `tick`
- assert it contains real SVG elements such as `<line`, `<rect`, and
  `<circle`
- assert `A&B<test>` appears only escaped as `A&amp;B&lt;test&gt;`

The negative control should construct a `Qp101Document` with `num_qubits = 0`
and assert `render_svg` returns an error mentioning `num_qubits` or qubits.

Run:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_draws_wires_gates_and_ticks -q
cargo test
git diff --check
```

If the sandbox blocks online crates.io index access, use `--offline` for Rust
verification and record the exact network failure.
