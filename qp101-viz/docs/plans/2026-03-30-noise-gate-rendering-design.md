# Noise Gate Rendering Design

Date: 2026-03-30

## Context

The current timeline renderer treats every `noise` operation as a generic multi-qubit gate. In practice, this causes three visible problems:

- `X_ERROR` and `DEPOLARIZE1` are rendered as large boxes spanning multiple wires, even though they are single-qubit noise channels.
- `DEPOLARIZE2` is rendered as one large spanning box instead of a two-qubit connected gate.
- Full gate names plus parameters, such as `DEPOLARIZE1(0.001)`, make noise gates visually wider than necessary.

The JSON samples in this repository already preserve enough semantic information to render these operations correctly. The main issue is in the renderer, not in the schema.

## JSON Design Assessment

The current JSON design is acceptable and should remain unchanged.

- `gate` preserves the canonical operation name.
- `params` preserves the physical parameter values.
- `raw_targets` preserves the source-level target references.
- optional `display.label` already exists as a presentation override and can remain as an escape hatch.

The renderer should own the mapping from canonical gate name to visual form. That keeps the schema semantic and avoids polluting circuit data with display-only abbreviations.

## Rendering Semantics

Noise gates should be classified by renderer-side policy before drawing:

- `X_ERROR`, `Z_ERROR`, `DEPOLARIZE1`: single-qubit noise
- `DEPOLARIZE2`: two-qubit noise
- unknown noise gates: fallback to the current generic multi-qubit rendering

Single-qubit noise should render as one compact box per target qubit in the same moment. This applies both to one-target ops and to batched ops where a single JSON operation references many qubits.

Two-qubit noise should render as a pair of compact boxes connected by a vertical line. If a future `DEPOLARIZE2` op contains an even-length target list, the renderer should expand it pairwise in order. Odd or malformed target sets should fall back to the generic rendering instead of guessing.

## Labels And Notes

Default short labels should be provided by the renderer:

- `X_ERROR` -> `XE`
- `DEPOLARIZE1` -> `D1`
- `DEPOLARIZE2` -> `D2`

These are presentation defaults only. If `display.label` is present, it should continue to override the default.

Noise parameters should not remain inside the gate body. Instead, the renderer should place a compact note such as `p=0.001` above the rendered noise group. For batched single-qubit noise and paired two-qubit noise, the parameter note should appear once per logical group, attached to the topmost rendered gate.

## Implementation Structure

The change should stay localized to `lib.typ` by extracting dedicated noise helpers:

- `noise-short-label(op)`
- `noise-note-label(op)`
- `noise-qubit-groups(op)`
- `render-noise-op(op, theme)`

`render-main-op` should delegate all `type == "noise"` operations to `render-noise-op`.

This keeps policy separate from drawing and avoids expanding the existing generic gate path further.

## Error Handling

The renderer should remain tolerant of malformed or unfamiliar data.

- If a supposed single-qubit noise op cannot resolve clean qubit targets, use the current generic fallback.
- If `DEPOLARIZE2` does not resolve to clean pairs, use the current generic fallback.
- Unknown noise gate kinds should keep the current behavior.

The goal is to improve common known cases without reducing compatibility.

## Verification Plan

Verification should include both metadata-style checks and visual compilation.

Add targeted checks to cover:

- single `X_ERROR` renders as a single boxed gate with short label
- batched `X_ERROR` and `DEPOLARIZE1` render as multiple single-wire gates in one moment
- `DEPOLARIZE2` renders as a two-wire connected structure rather than a spanning multigate
- parameter notes appear once above the rendered noise group

Then recompile representative examples, especially the circuit demos that currently show the issue, to confirm the updated appearance is correct.

## Out Of Scope

This change does not redesign the timeline layout system, change the JSON schema, or introduce a geometry-based renderer. It only fixes noise gate semantics and presentation within the existing timeline view.
