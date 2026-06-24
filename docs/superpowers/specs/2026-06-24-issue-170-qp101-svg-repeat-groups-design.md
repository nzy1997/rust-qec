# Issue 170 QP101 SVG Repeat Groups Design

## Context

Issues #168 and #169 moved the built-in `rstim::qp101_svg` renderer toward the QP101 timeline semantics used by `qp101-viz`. The current Rust renderer already expands `Qp101Operation::Repeat` bodies for visible columns, measurement-anchor numbering, detector indices, and detector/observable source resolution. It only shows the repeat operation as a top note, so users cannot see which expanded operations came from a repeat block or where later iterations begin.

Issue #170 adds the missing repeat structure display. The renderer must keep expanded-order semantics: measurement anchors and source labels continue globally across iterations, and `rec[-k]` labels resolve against the same expanded history.

## Approved Approach

Build a small internal repeat span model while rendering. When `render_operations` encounters a repeat block, it records the current visual column as the repeat start, recursively renders each expanded iteration into the same `RenderState`, records each iteration's start column, and then stores a `RepeatGroupSpan` covering the expanded body columns. After all operations are rendered, the renderer draws repeat decorations from these spans as plain SVG shapes.

The alternatives considered were:

1. Keep the existing top note only and rely on expanded columns. Rejected because it does not satisfy the visible repeat group and iteration-boundary requirements.
2. Port the Typst `gategroup`/`slice` rendering model directly. Rejected because the issue asks for plain SVG decorations and no Typst-specific concepts.
3. Pre-flatten the document into a separate public render model. Rejected for this issue because the existing renderer already has the needed expanded traversal and a private span list is the smallest stable step.

The selected approach keeps the public `render_svg(&Qp101Document) -> Result<String, String>` interface unchanged and keeps repeat metadata renderer-only.

## Rendering Behavior

For each repeat block with at least one rendered body column, draw a dashed rounded rectangle behind the expanded body columns. The rectangle spans all qubit wires, uses a light neutral fill, and has `class="repeat-group"` so tests and downstream users can identify it. Render a label such as `repeat x2` near the top-left of the rectangle.

For repeat counts greater than one, draw a vertical dashed boundary at the start of each iteration after the first. Each boundary uses `class="repeat-iteration-boundary"` and gets a label such as `iter 2`. The first iteration does not need an `iter 1` marker because the group label already marks the repeat block.

Nested repeats are supported by the same span list. Inner repeat groups are recorded while rendering the body and are drawn before or after outer groups using normal SVG order; no special nesting layout beyond the dashed boxes is required.

## Expanded Semantics

Repeat body operations remain expanded in visual order. A fixture like:

```stim
REPEAT 2 {
  M 0
  DETECTOR rec[-1]
  TICK
}
```

must render one repeat group labeled `repeat x2`, one `iter 2` boundary, measurement anchors `m1` and `m2`, and detector source labels `D0 = m1` and `D1 = m2`. It must not render two separate `m1` anchors for the two iterations.

## Implementation Units

- `rstim/src/qp101_svg.rs`
  - Add `RepeatGroupSpan` to the private renderer state.
  - Record repeat start/end columns and expanded iteration starts during recursive repeat rendering.
  - Draw repeat group rectangles and iteration boundaries before operation SVG so decorations sit behind gates and labels.
  - Keep height and width calculation based on expanded visible columns.

- `rstim/tests/qp101_svg.rs`
  - Add `svg_renderer_draws_repeat_groups_and_iteration_boundaries`.
  - Cover `repeat x2`, `iter 2`, one global `m1`, one global `m2`, per-iteration detector source labels, and no duplicate `m1` anchors.

## Testing

Use test-first development:

1. Add `svg_renderer_draws_repeat_groups_and_iteration_boundaries` and verify it fails because the current renderer has no `iter 2` boundary or repeat group rectangle.
2. Implement private repeat span recording and SVG drawing.
3. Run the required focused verification:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_draws_repeat_groups_and_iteration_boundaries -q
```

4. Run broader verification:

```sh
cargo test
```

## Scope Boundaries

Do not collapse large repeat counts into abbreviated rendering. Do not add coordinate-layout rendering. Do not attempt pixel-perfect parity with the Typst repeat group style. Do not change the QP101 schema, exporter semantics, or public SVG renderer API.
