# qstd101-viz

`qstd101-viz` is a local Typst package prototype for rendering QSTD101-ZY circuit JSON as a timeline view.

## Scope

This first prototype focuses on:

- reading QSTD101-ZY JSON directly with `json(...)`
- rendering the ordered `operations` stream as a quill-based quantum circuit
- preserving `repeat`, `tick`, detectors, observables, and noise as distinct visual events
- assigning renderer-only global measurement anchors such as `m1`, `m2`, `m3`, ... in expanded visual order
- resolving `detector` and `observable_include` `rec[-k]` sources into those measurement anchors when possible

It does not yet implement a geometry-based layout renderer. The schema and renderer shape are designed so that a future coordinate/layout view can consume the same JSON.

## Public API

- `timeline-theme(...)`
- `qstd101-timeline(doc, theme: timeline-theme())`
- `qstd101-timeline-file(path, theme: timeline-theme())`

## Example

```typst
#import "../lib.typ": qstd101-timeline-file, timeline-theme
#set page(width: auto, height: auto, margin: 10pt)

#qstd101-timeline-file(
  "examples/repeat-detector.qstd101.json",
  theme: timeline-theme(step_width: 5.6em),
)
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
- `M`, `MX`, and `MR` are rendered as compact measurement boxes with anchor badges above the gate.
- measurement-producing gates currently recognized for semantic anchors are `M`, `MX`, and `MR`.
- measurement and detector/observable operators reserve extra horizontal space for their labels so dense timelines do not collide as easily.
- `detector` and `observable_include` render inline on the circuit in a Stim-like single-wire box style.
- detector boxes use `DETECTOR` with a top label such as `D0 = m2*m1`.
- observable boxes use `OBS_INCLUDE(k)` with a top label such as `L0 *= m7`.
- detector and observable host wires follow Stim's rule: use the minimum resolved measurement-source qubit, otherwise fall back to the best available source qubit, otherwise `q0`.
- detector and `observable_include` display resolved anchors instead of raw `rec[-k]` when the referenced measurements exist in the current expanded history.
- detector and observable source text now lists each resolved anchor explicitly instead of compressing consecutive runs into `m7-m8` style ranges.
- non-`rec` sources remain textual, and unresolved `rec[...]` sources stay explicit as raw `rec[...]`.

## Verification

Verified in this session with local `typst 0.14.2` by compiling and querying:

- `examples/timeline.typ`
- `examples/rstim-fixture.typ`
- `examples/anchor-basic.typ`
- `checks/repeat-groups.typ`
- `checks/stim-operator-host-render.typ`
- query-based fixtures for:
  - measurement-history structure
  - wire layout and hidden metadata
  - detector / observable promotion into the main track
  - detector / observable source resolution
