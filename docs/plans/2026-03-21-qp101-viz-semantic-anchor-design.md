# QP101 Viz Semantic Anchor Design

**Date:** 2026-03-21

## Goal

Improve the local `qp101-viz` prototype so detector and observable annotations are visually tied to the measurements they depend on. The chosen direction is to keep the main circuit drawing close to the `yao-rs/visualization` quill-based style while adding a renderer-only semantic anchor layer.

The optimization target is not general prettification. It is semantic readability: when a user sees a detector or observable annotation, they should be able to tell which measurement results it depends on without manually decoding raw `rec[-k]` references.

## Chosen Direction

The renderer should assign a global, monotonically increasing measurement anchor to each measurement result that appears in the fully expanded visual execution order. These anchors are renderer-only labels such as `m1`, `m2`, `m3`, and they do not change the QP101 JSON schema.

`detector` and `observable_include` operations currently preserve their true source semantics through `rec[-k]`. The renderer should continue to respect those sources, but instead of displaying only raw relative references, it should resolve them against the visualized measurement history and render the resolved anchors. For example:

- `det rec[-1]` becomes `det m12`
- `obs[0] rec[-3] rec[-1]` becomes `obs[0] m18 m20`

This is intentionally a visualization interpretation layer, not a protocol change. The JSON stays generic and lossless; the Typst renderer adds a stable human-facing index for static diagrams.

## Render Model

The current renderer already normalizes ordered `operations` into visual moments. The next refinement should extend that normalization into a two-part render model:

1. `moments`: the fully expanded display order after handling `tick` and `repeat`
2. `measurement_history`: a global list of measurement outputs in visual order

During moment construction, the renderer should identify operations that emit measurement records and append anchor entries to `measurement_history`. Each entry should minimally preserve:

- anchor id, e.g. `m17`
- moment index
- qubit lane
- source operation kind, e.g. `M`, `MX`, `MR`

When the renderer later encounters `detector` or `observable_include`, each `rec[-k]` source should be resolved relative to the current measurement history length. This keeps the interpretation faithful to the original Stim-style semantics while giving the output a stable global numbering scheme.

## Visual Encoding

Measurement anchors should be rendered as small labels attached directly to the measurement gate they annotate. The main gate remains visually dominant; the anchor behaves like a corner badge or lightweight adjacent label. The objective is that a user first sees the circuit operation and only then reads the semantic anchor if needed.

Detectors and observables should continue to live on the lower annotation wire. However, instead of raw `rec[-k]`, their labels should show resolved measurement anchors. Continuous anchor ranges may be compressed for readability, e.g. `m12-m14`, while non-contiguous anchors remain space-separated.

This design explicitly rejects drawing explicit connector lines in this phase. Connectors are more visually direct, but on large QEC circuits they create clutter quickly. Global numbered anchors keep the diagram compact and robust in static PDF output.

## Supported Scope

The first implementation pass should only assign anchors to measurement-producing operations that are already common in the current renderer and fixture set. At minimum:

- `M`
- `MX`
- `MR`

The implementation should be structured so additional measurement-like operations can be added later, for example `MY`, `MZ`, `MRX`, `MRY`, and `MRZ`.

Reset-only operations such as `R` and `RX` should not receive anchors unless they actually emit a measurement record.

## Failure Behavior

If a `detector` or `observable_include` references a `rec[-k]` that cannot be resolved within the current visualized history, the renderer should show that explicitly instead of silently dropping it. A visible fallback such as `unresolved rec[-2]` is preferable to an incorrect anchor mapping.

Likewise, if a source reference is not a `rec` item, the renderer should preserve it textually instead of forcing it into the anchor model.

## Verification Strategy

Verification should happen at three levels.

1. A minimal hand-written example proving that a single measurement resolves to `m1`.
2. A repeat-based example proving that anchors are global across expanded rounds and do not reset inside `repeat`.
3. A real `rstim` fixture compile proving the renderer still works on large exported circuits.

For exact semantic checks, SVG output is preferable to PDF because the resulting text can be inspected directly. PDF compilation remains useful as the final smoke test for diagram production quality.

## Non-Goals

This refinement does not:

- change the QP101 JSON schema
- add geometric layout rendering from `coords`
- draw explicit semantic connector lines
- attempt to support every possible future measurement-producing operation in the first pass

The focus is narrow: make detector and observable dependencies legible in the existing timeline renderer without destabilizing the protocol or overcomplicating the first visualization pipeline.
