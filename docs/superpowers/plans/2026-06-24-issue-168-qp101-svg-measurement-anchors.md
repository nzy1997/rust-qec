# QP101 SVG Measurement Anchors Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render global renderer-only measurement anchor labels such as `m1`, `m2`, and `m3` in built-in QP101 SVG output.

**Architecture:** Keep the public `render_svg(&Qp101Document)` interface unchanged. Add a small derived render state inside `rstim/src/qp101_svg.rs` that tracks global measurement indices while the existing recursive operation traversal emits SVG. Measurement metadata is internal and never written back to the input document.

**Tech Stack:** Rust 2024, existing `rstim::qp101` data model, hand-written SVG string output, integration tests in `rstim/tests/qp101_svg.rs`.

## Global Constraints

- Do not change the QP101 JSON schema.
- Do not mutate the input `Qp101Document`.
- Measurement anchors are renderer-only SVG labels.
- Anchor numbering is global across the full rendered document and does not reset per moment, repeat body, or qubit.
- Supported one-output families are `M`, `MX`, `MY`, `MZ`, `MR`, `MRX`, `MRY`, `MRZ`.
- Supported two-output families are `ML`, `MXL`, `MYL`, `MZL`, `MRL`, `MRXL`, `MRYL`, `MRZL`.
- Reset-only gates such as `R` and `RX` must not receive measurement anchors.
- Detector and observable source resolution, repeat group decorations, and sample-shot annotation rendering are out of scope.

---

### Task 1: Add Renderer-Only Measurement Anchors

**Files:**
- Modify: `rstim/tests/qp101_svg.rs`
- Modify: `rstim/src/qp101_svg.rs`

**Interfaces:**
- Consumes: `rstim::qp101_svg::render_svg(doc: &Qp101Document) -> Result<String, String>`
- Produces: unchanged public `render_svg` interface; internal `RenderState` and `MeasurementTarget` only inside `rstim/src/qp101_svg.rs`

- [ ] **Step 1: Write the failing integration test**

Add this test to `rstim/tests/qp101_svg.rs` before the helper `annotation` function:

```rust
#[test]
fn svg_renderer_labels_measurements_with_global_anchors() {
    let instrs = parse_lines("M 0\nMRL 1\nMX 0\n")
        .expect("measurement anchor fixture should parse");
    let doc = export_qp101(&instrs).expect("measurement anchor fixture should export");
    let original_doc = doc.clone();

    let svg = render_svg(&doc).expect("measurement anchor fixture should render");

    for marker in [">M</text>", ">MRL</text>", ">MX</text>"] {
        assert!(
            svg.contains(marker),
            "SVG should keep original measurement gate label {marker}: {svg}"
        );
    }
    for anchor in [">m1</text>", ">m2-m3</text>", ">m4</text>"] {
        assert!(
            svg.contains(anchor),
            "SVG should contain measurement anchor {anchor}: {svg}"
        );
    }
    assert!(
        svg.find(">m1</text>").expect("m1 should be present")
            < svg.find(">m2-m3</text>").expect("m2-m3 should be present"),
        "m1 should appear before the MRL span: {svg}"
    );
    assert!(
        svg.find(">m2-m3</text>").expect("m2-m3 should be present")
            < svg.find(">m4</text>").expect("m4 should be present"),
        "MRL should reserve m2 and m3 before MX receives m4: {svg}"
    );
    assert_eq!(
        doc, original_doc,
        "SVG rendering must not mutate the QP101 document"
    );

    let reset_only = export_qp101(
        &parse_lines("R 0\nRX 1\n").expect("reset-only fixture should parse"),
    )
    .expect("reset-only fixture should export");
    let reset_svg = render_svg(&reset_only).expect("reset-only fixture should render");

    assert!(
        !reset_svg.contains(">m1</text>"),
        "reset-only gates must not receive measurement anchors: {reset_svg}"
    );
}
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_labels_measurements_with_global_anchors -q --offline
```

Expected: the test fails because `m1`, `m2-m3`, and `m4` are not present.

- [ ] **Step 3: Add internal render state and measurement helpers**

In `rstim/src/qp101_svg.rs`, add these structs after the layout constants:

```rust
#[derive(Debug, Clone)]
struct MeasurementTarget {
    lane: usize,
    first_index: usize,
    output_count: usize,
}

impl MeasurementTarget {
    fn anchor(&self) -> String {
        if self.output_count == 1 {
            format!("m{}", self.first_index)
        } else {
            format!(
                "m{}-m{}",
                self.first_index,
                self.first_index + self.output_count - 1
            )
        }
    }
}

#[derive(Debug, Default)]
struct RenderState {
    next_measurement_index: usize,
}
```

Add this helper near `is_simple_single_qubit_gate`:

```rust
fn measurement_output_count(gate: &str) -> Option<usize> {
    match gate {
        "M" | "MX" | "MY" | "MZ" | "MR" | "MRX" | "MRY" | "MRZ" => Some(1),
        "ML" | "MXL" | "MYL" | "MZL" | "MRL" | "MRXL" | "MRYL" | "MRZL" => Some(2),
        _ => None,
    }
}
```

Add this helper near `raw_target_lanes`:

```rust
fn measurement_targets(
    gate: &str,
    targets: &[u32],
    num_qubits: usize,
    state: &mut RenderState,
) -> Result<Vec<MeasurementTarget>, String> {
    let Some(output_count) = measurement_output_count(gate) else {
        return Ok(Vec::new());
    };

    let mut measurement_targets = Vec::with_capacity(targets.len());
    for &target in targets {
        let lane = validate_lane(target, num_qubits, gate)?;
        let first_index = state.next_measurement_index + 1;
        state.next_measurement_index += output_count;
        measurement_targets.push(MeasurementTarget {
            lane,
            first_index,
            output_count,
        });
    }
    Ok(measurement_targets)
}
```

- [ ] **Step 4: Thread render state through operation rendering**

Change `render_svg` to create a mutable state and pass it to `render_operations`:

```rust
let mut column = 0usize;
let mut state = RenderState::default();
render_operations(
    &mut out,
    &doc.operations,
    doc.num_qubits,
    &mut column,
    &mut state,
)?;
```

Change `render_operations` to accept `state: &mut RenderState`. In the `Gate` match arm, pass `state` into `render_gate`. In the `Repeat` match arm, pass the same `state` into the recursive `render_operations` call so measurement numbering remains global.

Change `render_gate` to accept `state: &mut RenderState`. After computing `lanes`, derive:

```rust
let measurement_targets = measurement_targets(gate, targets, num_qubits, state)?;
```

After `render_annotations(out, x, &lanes, annotations);`, add:

```rust
render_measurement_anchors(out, x, &measurement_targets);
```

- [ ] **Step 5: Render measurement anchor labels**

Add this helper near `render_annotations`:

```rust
fn render_measurement_anchors(out: &mut String, x: i32, targets: &[MeasurementTarget]) {
    for target in targets {
        out.push_str(&format!(
            "<text class=\"measurement-anchor\" x=\"{x}\" y=\"{}\" fill=\"#2563eb\" text-anchor=\"middle\" font-size=\"11\">{}</text>\n",
            lane_y(target.lane) + GATE_HEIGHT / 2 + 14,
            escape_xml(&target.anchor())
        ));
    }
}
```

- [ ] **Step 6: Run the focused test and verify GREEN**

Run:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_labels_measurements_with_global_anchors -q --offline
```

Expected: the test passes.

- [ ] **Step 7: Run the full QP101 SVG integration test file**

Run:

```sh
cargo test -p rstim --test qp101_svg -q --offline
```

Expected: all tests in `rstim/tests/qp101_svg.rs` pass.

- [ ] **Step 8: Format the changed Rust files**

Run:

```sh
cargo fmt -- rstim/src/qp101_svg.rs rstim/tests/qp101_svg.rs
```

Expected: command exits 0 and leaves only intentional formatting changes.

- [ ] **Step 9: Commit the implementation**

Run:

```sh
git add rstim/src/qp101_svg.rs rstim/tests/qp101_svg.rs docs/superpowers/plans/2026-06-24-issue-168-qp101-svg-measurement-anchors.md
git commit -m "feat: render qp101 svg measurement anchors"
```

Expected: one implementation commit containing the renderer change, the regression test, and this implementation plan.
