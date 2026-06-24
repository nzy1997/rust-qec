# Issue 168 QP101 SVG Measurement Anchors Design

## Context

Issue #168 extends the built-in `rstim::qp101_svg` renderer added by #166. The renderer already consumes an immutable `Qp101Document`, writes deterministic SVG text, and keeps renderer behavior local to `rstim/src/qp101_svg.rs`. The QP101 JSON contract must not change for this issue.

The Typst renderer has the measurement semantics to port: single-output measurement families (`M`, `MX`, `MY`, `MZ`, `MR`, `MRX`, `MRY`, `MRZ`) emit one result per target, while loss-visible families (`ML`, `MXL`, `MYL`, `MZL`, `MRL`, `MRXL`, `MRYL`, `MRZL`) emit two results per target. The built-in renderer should display renderer-only anchors such as `m1` and `m2` in expanded visual order.

## Approved Approach

Use a small internal render model in `rstim/src/qp101_svg.rs`. During rendering, thread a `RenderState` through the existing operation traversal. For each gate, derive per-target `MeasurementTarget` metadata when the gate is a supported measurement family. Each metadata entry records the qubit lane, gate label, first measurement index, output count, and displayed anchor text. This state is derived only from the input document and is not written back into `Qp101Document`.

The chosen alternatives were:

1. Add anchor annotations to `Qp101Document`. Rejected because the issue explicitly forbids schema changes and mutation.
2. Count anchors inside SVG text emission only. Rejected because later detector and observable source resolution needs reusable metadata.
3. Build a full Typst-style moment model now. Rejected because repeat decorations and source resolution are out of scope.

The selected approach keeps the change small while still creating the reusable measurement metadata boundary required by later #33 work.

## Rendering Behavior

For measurement gates, render the existing gate label exactly as before and add a separate anchor label near each measurement target. Single-output targets display one anchor, for example `m1`. Multi-output targets display an inclusive span, for example `m2-m3`, while reserving both measurement indices internally. Anchor numbering is global across the full rendered document and follows expanded visual traversal, including each repeat-body iteration counted by its repeat count. Repeat group decorations remain out of scope.

Reset-only gates such as `R` and `RX` are not measurement-producing operations and must not display anchors.

## Implementation Units

- `rstim/src/qp101_svg.rs`
  - Add `RenderState` and `MeasurementTarget`.
  - Add `measurement_output_count(gate: &str) -> Option<usize>` matching the Typst table.
  - Derive measurement targets for gate operations before drawing.
  - Render anchor text without changing gate labels or document data.

- `rstim/tests/qp101_svg.rs`
  - Add `svg_renderer_labels_measurements_with_global_anchors`.
  - Cover single-output ordering, `MRL` output-span reservation, preservation of gate labels, document immutability, and reset-only negative control.

## Testing

Use test-first development:

1. Add the new integration test and verify it fails because anchors are missing.
2. Implement the renderer state and anchor drawing.
3. Run the required focused test:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_labels_measurements_with_global_anchors -q
```

4. Run the broader applicable verification:

```sh
cargo test
```

If the environment blocks online dependency resolution, use the same commands with `--offline` and record the workspace-level network limitation.

## Scope Boundaries

Do not resolve detector or observable sources. Do not add repeat group decorations. Do not add sample-shot annotation rendering. Do not update the QP101 JSON schema, fixture schema, or format documentation because this change is renderer-only.
