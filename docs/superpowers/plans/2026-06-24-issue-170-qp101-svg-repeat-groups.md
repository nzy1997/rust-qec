# QP101 SVG Repeat Groups Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render built-in QP101 SVG repeat blocks with visible repeat group regions and iteration boundary markers while preserving expanded-order measurement/source semantics.

**Architecture:** Keep the public `render_svg(&Qp101Document)` API unchanged. Extend the private SVG render state with repeat group spans collected during the existing recursive expanded traversal, then draw plain SVG dashed group rectangles and iteration boundaries behind operation SVG.

**Tech Stack:** Rust 2024, existing `rstim::qp101` data model, hand-written SVG string output, integration tests in `rstim/tests/qp101_svg.rs`.

## Global Constraints

- Do not change the QP101 JSON schema or exporter semantics.
- Do not mutate the input `Qp101Document`.
- Repeat bodies render in expanded visual order.
- Measurement anchors remain global across repeat iterations and must not reset inside repeat bodies.
- Detector and observable source labels resolve against the expanded measurement history from #169.
- Repeat groups use plain SVG shapes and text, not Typst `gategroup` or `slice` concepts.
- Render repeat labels as group decorations over the expanded body columns, not as separate operation columns.
- Support nested repeats with the same span mechanism; no pixel-perfect Typst parity is required.
- Do not collapse large repeat counts, add coordinate-layout rendering, or add sample-shot annotation rendering.

---

### Task 1: Add Repeat Group Regression Coverage

**Files:**
- Modify: `rstim/tests/qp101_svg.rs`

**Interfaces:**
- Consumes: `rstim::qp101_svg::render_svg(doc: &Qp101Document) -> Result<String, String>`
- Produces: Failing integration test `svg_renderer_draws_repeat_groups_and_iteration_boundaries`

- [ ] **Step 1: Add the failing integration test**

Add this test before `svg_renderer_assigns_measurement_anchors_in_expanded_repeat_order`:

```rust
#[test]
fn svg_renderer_draws_repeat_groups_and_iteration_boundaries() {
    let instrs = parse_lines(
        "REPEAT 2 {\n  M 0\n  DETECTOR rec[-1]\n  TICK\n}\n",
    )
    .expect("repeat group fixture should parse");
    let doc = export_qp101(&instrs).expect("repeat group fixture should export");

    let svg = render_svg(&doc).expect("repeat group fixture should render");

    for marker in [
        "class=\"repeat-group\"",
        ">repeat x2</text>",
        "class=\"repeat-iteration-boundary\"",
        ">iter 2</text>",
        ">m1</text>",
        ">m2</text>",
        ">D0 = m1</text>",
        ">D1 = m2</text>",
    ] {
        assert!(
            svg.contains(marker),
            "repeat SVG should contain {marker}: {svg}"
        );
    }
    assert_eq!(
        svg.matches(">m1</text>").count(),
        1,
        "first repeat iteration should contain exactly one m1 anchor: {svg}"
    );
    assert_eq!(
        svg.matches(">m2</text>").count(),
        1,
        "second repeat iteration should continue to m2 instead of resetting to m1: {svg}"
    );
    assert!(
        !svg.contains(">D1 = m1</text>"),
        "second detector source must not resolve to the first iteration anchor: {svg}"
    );
    assert!(
        svg.find(">m1</text>").expect("m1 should be present")
            < svg.find(">m2</text>").expect("m2 should be present"),
        "measurement anchors should appear in expanded repeat order: {svg}"
    );
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_draws_repeat_groups_and_iteration_boundaries -q
```

Expected: the test fails because the current renderer does not emit `class="repeat-group"`, `class="repeat-iteration-boundary"`, or `iter 2`.

### Task 2: Implement Repeat Span Decorations

**Files:**
- Modify: `rstim/src/qp101_svg.rs`
- Modify: `rstim/tests/qp101_svg.rs`

**Interfaces:**
- Consumes: private `RenderState`, `render_operations`, `count_visible_columns`, `x_for_column`, `lane_y`
- Produces: private `RepeatGroupSpan`, repeat span recording during expanded traversal, and plain SVG repeat decorations

- [ ] **Step 1: Add repeat span state**

In `rstim/src/qp101_svg.rs`, add this struct after `MeasurementRecord`:

```rust
#[derive(Debug, Clone)]
struct RepeatGroupSpan {
    count: u64,
    start_column: usize,
    end_column: usize,
    iteration_starts: Vec<usize>,
}
```

Then add a field to `RenderState`:

```rust
repeat_groups: Vec<RepeatGroupSpan>,
```

- [ ] **Step 2: Stop counting repeats as standalone columns**

Change the `Qp101Operation::Repeat` arm in `count_visible_columns` to:

```rust
Qp101Operation::Repeat { count, body, .. } => {
    let count = usize::try_from(*count).unwrap_or(usize::MAX);
    count_visible_columns(body).saturating_mul(count)
}
```

- [ ] **Step 3: Render operation SVG into a buffer and draw decorations behind it**

In `render_svg`, replace the direct wire/render sequence with this order:

```rust
let mut operation_out = String::new();
let mut column = 0usize;
let mut state = RenderState::default();
render_operations(
    &mut operation_out,
    &doc.operations,
    doc.num_qubits,
    &mut column,
    &mut state,
)?;
render_repeat_decorations(&mut out, &state.repeat_groups, doc.num_qubits);
render_wires(&mut out, doc.num_qubits, width);
out.push_str(&operation_out);
```

This keeps repeat fills behind wires, gates, anchors, source labels, and annotations.

- [ ] **Step 4: Record repeat spans while recursively rendering expanded body columns**

Replace the `Qp101Operation::Repeat` arm in `render_operations` with:

```rust
Qp101Operation::Repeat { count, body, .. } => {
    let start_column = *column;
    let mut iteration_starts = Vec::new();
    for _ in 0..*count {
        iteration_starts.push(*column);
        render_operations(out, body, num_qubits, column, state)?;
    }
    if *column > start_column {
        state.repeat_groups.push(RepeatGroupSpan {
            count: *count,
            start_column,
            end_column: *column - 1,
            iteration_starts,
        });
    }
}
```

- [ ] **Step 5: Add plain SVG repeat decoration helpers**

Add these constants near the other layout constants:

```rust
const REPEAT_GROUP_TOP_PAD: i32 = 8;
const REPEAT_GROUP_BOTTOM_PAD: i32 = 8;
const REPEAT_GROUP_X_PAD: i32 = 4;
```

Add these helpers near `render_top_note`:

```rust
fn render_repeat_decorations(out: &mut String, groups: &[RepeatGroupSpan], num_qubits: usize) {
    for group in groups.iter().rev() {
        render_repeat_group(out, group, num_qubits);
        render_repeat_iteration_boundaries(out, group, num_qubits);
    }
}

fn render_repeat_group(out: &mut String, group: &RepeatGroupSpan, num_qubits: usize) {
    let x_start = x_for_column(group.start_column);
    let x_end = x_for_column(group.end_column);
    let left = x_start - COLUMN_GAP / 2 + REPEAT_GROUP_X_PAD;
    let right = x_end + COLUMN_GAP / 2 - REPEAT_GROUP_X_PAD;
    let top = lane_y(0) - GATE_HEIGHT / 2 - REPEAT_GROUP_TOP_PAD;
    let bottom = lane_y(num_qubits.saturating_sub(1)) + GATE_HEIGHT / 2 + REPEAT_GROUP_BOTTOM_PAD;
    let width = right - left;
    let height = bottom - top;
    out.push_str(&format!(
        "<rect class=\"repeat-group\" x=\"{left}\" y=\"{top}\" width=\"{width}\" height=\"{height}\" rx=\"6\" ry=\"6\" stroke=\"#98a2b3\" stroke-width=\"1\" stroke-dasharray=\"6 4\" fill=\"#f8fafc\" />\n"
    ));
    out.push_str(&format!(
        "<text class=\"repeat-group-label\" x=\"{}\" y=\"{}\" fill=\"#475467\" text-anchor=\"start\" font-size=\"12\">repeat x{}</text>\n",
        left + 8,
        top + 13,
        group.count
    ));
}

fn render_repeat_iteration_boundaries(out: &mut String, group: &RepeatGroupSpan, num_qubits: usize) {
    let top = lane_y(0) - GATE_HEIGHT / 2 - REPEAT_GROUP_TOP_PAD;
    let bottom = lane_y(num_qubits.saturating_sub(1)) + GATE_HEIGHT / 2 + REPEAT_GROUP_BOTTOM_PAD;
    for (iteration_offset, &start_column) in group.iteration_starts.iter().enumerate().skip(1) {
        let x = x_for_column(start_column) - COLUMN_GAP / 2;
        out.push_str(&format!(
            "<line class=\"repeat-iteration-boundary\" x1=\"{x}\" y1=\"{top}\" x2=\"{x}\" y2=\"{bottom}\" stroke=\"#98a2b3\" stroke-width=\"1\" stroke-dasharray=\"4 4\" />\n"
        ));
        out.push_str(&format!(
            "<text class=\"repeat-iteration-label\" x=\"{x}\" y=\"{}\" fill=\"#475467\" text-anchor=\"middle\" font-size=\"11\">iter {}</text>\n",
            top - 4,
            iteration_offset + 1
        ));
    }
}
```

- [ ] **Step 6: Run the focused test and verify GREEN**

Run:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_draws_repeat_groups_and_iteration_boundaries -q
```

Expected: the new repeat group test passes.

- [ ] **Step 7: Run the QP101 SVG integration test file**

Run:

```sh
cargo test -p rstim --test qp101_svg -q
```

Expected: all QP101 SVG integration tests pass.

- [ ] **Step 8: Format changed Rust files**

Run:

```sh
cargo fmt -- rstim/src/qp101_svg.rs rstim/tests/qp101_svg.rs
```

Expected: command exits 0 and leaves only intentional formatting changes.

### Task 3: Final Verification and Commit

**Files:**
- Modify: `docs/superpowers/plans/2026-06-24-issue-170-qp101-svg-repeat-groups.md`
- Modify: `rstim/src/qp101_svg.rs`
- Modify: `rstim/tests/qp101_svg.rs`

**Interfaces:**
- Consumes: passing repeat group implementation from Task 2
- Produces: committed implementation ready for PR

- [ ] **Step 1: Run required focused verification**

Run:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_draws_repeat_groups_and_iteration_boundaries -q
```

Expected: pass.

- [ ] **Step 2: Run required broader verification**

Run:

```sh
cargo test
```

Expected: pass.

- [ ] **Step 3: Run formatting and whitespace checks**

Run:

```sh
cargo fmt --check
git diff --check origin/master..HEAD
```

Expected: both commands exit 0.

- [ ] **Step 4: Commit the implementation**

Run:

```sh
git add rstim/src/qp101_svg.rs rstim/tests/qp101_svg.rs docs/superpowers/plans/2026-06-24-issue-170-qp101-svg-repeat-groups.md
git commit -m "feat: render qp101 svg repeat groups"
```

Expected: one implementation commit containing repeat group rendering, regression tests, and this plan.
