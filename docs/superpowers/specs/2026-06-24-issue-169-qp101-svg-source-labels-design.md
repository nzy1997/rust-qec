# Issue 169 QP101 SVG Source Labels Design

## Context

Issue #168 added renderer-only measurement anchors to `rstim::qp101_svg`. The renderer now walks operations in expanded visual order, assigns global anchors such as `m1` and `m2-m3`, and keeps that state internal to rendering. Issue #169 uses the same derived state to make detector and observable labels readable without changing the QP101 JSON model.

The existing built-in SVG renderer currently shows `detector` and `L<index>` top notes but does not show source details. QP101 detector and observable sources remain raw `Qp101TargetRef` values, usually `rec[-k]`. Rendering must resolve valid `rec[-k]` references to the visual anchors that already appear in the same SVG, and must keep unresolved or non-`rec` sources visible as text.

## Approved Approach

Extend the internal `RenderState` in `rstim/src/qp101_svg.rs` with measurement history and a monotonic detector index. Each measurement-producing gate will append one `MeasurementRecord` per emitted measurement result as anchors are assigned. Detector and observable rendering will resolve their `sources` against the current history at render time, then render the operation label and a second source label on the chosen host wire.

The alternatives considered were:

1. Resolve sources by scanning SVG text for anchor labels. Rejected because it is brittle and can invent matches unrelated to measurement semantics.
2. Add resolved labels to `Qp101Document`. Rejected because the issue requires renderer-only behavior and no schema mutation.
3. Build a separate full Typst-style moment model. Rejected because connector lines, coordinate layout, and other Typst behavior are out of scope.

The selected approach reuses the #168 traversal and keeps all new behavior local to the built-in SVG renderer.

## Rendering Behavior

For detectors, render the operation label as `DETECTOR` and a source label like `D0 = m1`. The detector index is assigned in visual order and increments only for `detector` operations. For observables, render the operation label as `OBS_INCLUDE(<index>)` and a source label like `L2 *= m1`, preserving the QP101 observable index.

For `Qp101TargetRef::Rec { offset }`, calculate the referenced measurement index as `current_measurement_count + offset + 1`. A valid negative `rec` offset that maps to an existing measurement record resolves to that record's anchor, such as `m1`. Invalid offsets, positive offsets, missing measurement records, and hand-built malformed documents remain visible as raw text such as `rec[-99]`; the renderer must not panic or guess another anchor.

Non-`rec` source refs render textually. Use Stim-style spellings: qubits as `q0` or `!q0`, Pauli refs as `X0` or `!X0`, sweep refs as `sweep[0]`, and combiners as `*`. Multiple detector or observable source pieces are joined with `*`, matching the Typst source-label style.

## Host Wire Choice

Detector and observable labels should be placed on the best available qubit lane. If any source resolves to measurements, use the minimum qubit lane among those resolved measurements. Otherwise, use the minimum explicit source qubit from qubit or Pauli target refs. If no source provides a qubit, use `q0`. This follows the issue's Stim-style host-wire recommendation while remaining safe for unsupported and future source refs.

## Implementation Units

- `rstim/src/qp101_svg.rs`
  - Extend `RenderState` with `measurements: Vec<MeasurementRecord>` and `next_detector_index`.
  - Add target-ref formatting and source-resolution helpers.
  - Store individual measurement records when measurement anchors are assigned.
  - Render detector and observable labels with resolved source text and host-lane placement.
  - Include source-label baselines in SVG height calculation so labels are not clipped.

- `rstim/tests/qp101_svg.rs`
  - Add `svg_renderer_resolves_detector_observable_sources`.
  - Cover parser-created detector and observable fixtures, detector index display, unresolved `rec[-99]` fallback, and hand-built non-`rec` source refs.

## Testing

Use test-first development:

1. Add `svg_renderer_resolves_detector_observable_sources` and verify it fails because source labels are absent or unresolved.
2. Implement source resolution and host-lane rendering.
3. Run the required focused verification:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_resolves_detector_observable_sources -q
```

4. Run broader verification:

```sh
cargo test
```

## Scope Boundaries

Do not draw connector lines from detectors or observables back to measurement gates. Do not add coordinate-layout rendering. Do not add sample-shot detector flip highlighting. Do not change QP101 JSON schema, exporter semantics, fixtures, or Typst rendering because this issue changes only built-in SVG output.
