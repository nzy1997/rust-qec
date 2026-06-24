# Issue 171 QP101 SVG Noise Boxes Design

Date: 2026-06-24
Status: Design approved by Agent Desk standing policy
Scope: GitHub issue #171, compact QEC noise rendering in `rstim::qp101_svg`

## Summary

Issue #171 extends the Rust-side QP101 SVG renderer from issue #166 so common
QEC noise operations are visible as compact timeline boxes instead of large
generic gate spans. The renderer keeps the existing public interface:

```rust
pub fn render_svg(doc: &Qp101Document) -> Result<String, String>;
```

The change stays inside the SVG renderer and its integration tests. It consumes
`Qp101Operation::Noise` values with canonical `gate`, `params`, and
`raw_targets`, derives renderer-side presentation labels, and leaves sample-shot
annotation overlays out of scope.

## Current State

`rstim/src/qp101_svg.rs` already renders wires, ticks, simple gates, controlled
pairs, swaps, generic fallback boxes, top notes, and annotations. Noise
operations currently enter the generic fallback path:

- qubit targets are extracted from `raw_targets`
- one generic box uses the full gate name
- annotations render separately below the operation

The Typst reference renderer in `qp101-viz/lib.typ` already has noise-specific
policy helpers. Its broad labels are `XE`, `ZE`, `D1`, `D2`, with `LOSS`
remaining visible as `LOSS`. It treats `X_ERROR`, `Z_ERROR`, `DEPOLARIZE1`, and
`LOSS` as single-target noise, treats `DEPOLARIZE2` as pair-target noise when
the qubit target count is even, and falls back generically otherwise.

The repository has no `AGENTS.md`, `CLAUDE.md`, or `CONVENTIONS.md` in this
checkout.

## Goals

- Render `X_ERROR` as compact per-target boxes labeled `XE`.
- Render `Z_ERROR` as compact per-target boxes labeled `ZE`.
- Render `DEPOLARIZE1` as compact per-target boxes labeled `D1`.
- Render `DEPOLARIZE2` as compact paired target groups labeled `D2`.
- Render base `LOSS` as compact per-target boxes labeled `LOSS`.
- Preserve at least one visible probability or parameter note when noise
  parameters exist.
- Keep unknown `noise` operations visible through a generic fallback box.
- For odd or malformed `DEPOLARIZE2` target groups, visibly fall back to a
  generic `DEPOLARIZE2` noise box instead of panicking or dropping the op.
- Keep annotation rendering separate from base noise rendering.
- Add focused integration coverage for the required positive and negative
  cases.

## Non-Goals

- Do not add sample-shot fired-branch annotations.
- Do not add DEM-origin highlight markers.
- Do not add exhaustive support for every future noise instruction.
- Do not change QP101 JSON schema or exporter behavior.
- Do not add dependencies or CLI integration.
- Do not chase pixel-perfect parity with `qp101-viz`.

## Approaches Considered

### 1. Add local noise helpers to `rstim::qp101_svg`

Classify noise operations inside the renderer, map known canonical names to
short labels, format parameter notes, group raw qubit targets by single-target
or pair-target policy, and reuse the existing SVG primitives with small helper
functions.

Benefits:

- matches issue #171 and the existing #166 renderer architecture
- keeps schema and exporter semantic instead of display-oriented
- reuses current validation, XML escaping, and fallback behavior
- keeps the change easy to test with string-level semantic assertions

Costs:

- adds a small amount of renderer-side policy that future phases may evolve

This is the chosen approach.

### 2. Put short display labels into QP101 export

Change `export_qp101` so noise operations carry display labels such as `XE` or
`D1`.

Benefits:

- SVG rendering could stay close to the generic gate path

Costs:

- pollutes QP101 circuit data with renderer policy
- conflicts with the `qp101-viz` design assessment that the renderer should own
  compact labels
- risks affecting JSON consumers beyond SVG rendering

This is rejected.

### 3. Return errors for malformed `DEPOLARIZE2`

Reject odd-count `DEPOLARIZE2` target groups with a renderer error.

Benefits:

- makes malformed paired-target data explicit

Costs:

- existing #166 paired-gate behavior visibly falls back for unmatched operands
- issue #171 permits either clear errors or visible fallback
- fallback is more useful for debugging diagrams because the operation remains
  visible

This is rejected in favor of visible fallback.

## Renderer Behavior

Noise rendering should keep using the existing operation column model. Every
noise operation gets one column, just as it does today.

Label policy:

- `X_ERROR` -> `XE`
- `Z_ERROR` -> `ZE`
- `DEPOLARIZE1` -> `D1`
- `DEPOLARIZE2` -> `D2`
- `LOSS` -> `LOSS`
- any other noise gate -> canonical gate name

Parameter policy:

- if `params` is empty, render no parameter note
- if `params` has one value, render that value as a compact note near the noise
  operation
- if `params` has multiple values, join them in source order so the parameter
  text remains visible without changing the QP101 model

Target policy:

- extract qubit lanes from `raw_targets`, accepting `qubit` and `pauli` target
  references through the existing lane validation rules
- ignore non-qubit target references for lane grouping, as the current generic
  fallback does
- render single-target noise (`X_ERROR`, `Z_ERROR`, `DEPOLARIZE1`, `LOSS`) as
  one compact box per extracted lane
- render `DEPOLARIZE2` as paired lane groups in raw target order when the
  extracted lane count is non-zero and even
- when `DEPOLARIZE2` has zero or odd extracted lanes, render a generic fallback
  box labeled `DEPOLARIZE2`
- when unknown noise has extracted lanes, render a generic fallback box labeled
  with the canonical gate name
- when any noise operation has no extracted lanes, render a top note with the
  visible label so the operation is not silently dropped

Annotation rendering remains the existing `render_annotations` path. The base
noise boxes do not inspect sample-shot or DEM annotation contents.

## Testing

Add tests to `rstim/tests/qp101_svg.rs`.

The positive test named `svg_renderer_draws_noise_boxes` should build a
two-qubit QP101 document through `parse_lines` and `export_qp101` using:

```stim
H 0
X_ERROR(0.1) 0
Z_ERROR(0.2) 1
DEPOLARIZE1(0.3) 0
DEPOLARIZE2(0.4) 0 1
LOSS(0.5) 1
M 0
```

It should call `render_svg` and assert:

- the SVG contains `q0` and `q1`
- the SVG contains ordinary neighboring gate labels such as `H` and `M`
- the SVG contains visible noise labels `XE`, `ZE`, `D1`, `D2`, and `LOSS`
- the SVG contains at least one probability or parameter value such as `0.1`
- noise output uses compact per-target or paired boxes rather than one silent
  drop

The negative test should construct a manual `Qp101Document` containing
`Qp101Operation::Noise { gate: "DEPOLARIZE2", raw_targets: [q0, q1, q0], ... }`
and assert that `render_svg` does not panic, returns `Ok`, and includes a
visible generic `DEPOLARIZE2` box. This validates the chosen fallback path.

Run:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_draws_noise_boxes -q
cargo test -p rstim --test qp101_svg -q
cargo test
git diff --check
```
