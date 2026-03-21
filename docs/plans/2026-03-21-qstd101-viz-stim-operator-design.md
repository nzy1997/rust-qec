# QSTD101 Viz Stim-Style Operator Design

**Date:** 2026-03-21

## Goal

Refine the local `qstd101-viz` timeline renderer so it reads more like Stim's own timeline diagrams: only real qubit wires remain, unused metadata stops occupying circuit space, and `detector` / `observable_include` become inline circuit operators instead of bottom-lane annotations.

## Confirmed Decisions

- Remove the fake `meta` / `ann` wires from the circuit.
- Restore the left-side wire labels so they map directly to real qubits (`q0`, `q1`, ...).
- Do not render currently-unused metadata such as `qubit_coords` and `shift_coords` in the timeline.
- Keep the existing measurement-anchor semantics (`m1`, `m2`, ...) and reuse them in detector / observable text.
- Render `detector` and `observable_include` as single-wire operators on the circuit, following Stim-style timeline behavior.
- Match Stim's host-wire rule: mount the operator on the minimum qubit index referenced by the resolved sources.

## Visual Direction

The circuit body should return to a pure qubit-wire view. Measurement gates keep their existing anchor badges. `detector` and `observable_include` should render as lightweight operator boxes in their original moment column, matching Stim's timeline wording as closely as practical:

- box label `DETECTOR` with source label `D0 = m2*m1`
- box label `OBS_INCLUDE(0)` with source label `L0 *= m7`

Source resolution stays anchor-based, so raw `rec[-k]` is not the normal display path. Multiple sources use `*` separators instead of spaces. Anchor range compression such as `m7-m8` remains allowed.

The operator is not a spanning gate and does not draw connector lines. It occupies one host wire only. This keeps the timeline compact and aligned with the user's request to follow Stim instead of inventing a heavier custom notation.

## Host-Wire Semantics

When rendering a detector or observable:

1. Resolve each source into an internal representation.
2. For resolved `rec[-k]`, carry both the anchor text and the measured qubit.
3. Derive the host wire from the minimum qubit among all resolved measurement sources.
4. If no measurement source resolves to a qubit, fall back to any qubit-bearing non-`rec` source.
5. If no qubit can be inferred, fall back to `q0` while preserving unresolved text.

This matches the behavior observed from Stim's `timeline-text` and `timeline-svg` diagrams, where detector / observable operators are attached to the minimum involved qubit wire and observables use `*=` instead of `=`.

## Render-Model Changes

The current render model still separates `moment.main` and `moment.bottom`. That is no longer the right split once detector / observable items become true circuit operators.

The next refinement should move these items into `moment.main` as structured operator entries, for example:

- measurement gate entries
- ordinary gate entries
- detector operator entries
- observable operator entries

Bottom-lane text rendering should then disappear entirely. The timeline builder should compute the operator text and host wire during normalization, so the render path can stay simple and only worry about how to draw the operator.

## Scope Boundaries

This change intentionally does not:

- reintroduce geometric placement from coordinates
- draw semantic connector lines
- preserve metadata-only labels on hidden extra wires
- change the public Typst API

The focus is narrow and visual: make the timeline look more like Stim, make qubit labels correct again, and move detector / observable semantics into the circuit itself.
