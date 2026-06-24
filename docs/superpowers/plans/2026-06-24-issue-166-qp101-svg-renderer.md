# Issue 166 QP101 SVG Renderer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a minimal Rust QP101-to-SVG renderer module with focused tests for wires, gates, ticks, escaping, and zero-qubit rejection.

**Architecture:** Add a new `rstim::qp101_svg` module that consumes `rstim::qp101::Qp101Document` and writes deterministic SVG strings with local layout constants. The renderer stays independent of Stim parsing and QP101 export internals; tests may use `export_qp101` only to build realistic QP101 input.

**Tech Stack:** Rust 2024, existing `rstim::qp101` data model, hand-written SVG string writer, existing `rstim::parser::parse_lines` and `rstim::qp101::export_qp101` in tests.

## Global Constraints

- The public renderer interface is `pub fn render_svg(doc: &Qp101Document) -> Result<String, String>`.
- The renderer must consume `&rstim::qp101::Qp101Document`, not raw Stim instructions.
- Success returns a complete SVG document string.
- Failure returns a clear renderer error.
- Return an error naming `num_qubits` or qubits when `doc.num_qubits == 0`.
- The SVG document must include `width`, `height`, and `viewBox`.
- Render qubit wires and `qN` labels.
- Render simple single-qubit gates such as `H`, `X`, `Y`, `Z`, `S`, `T`, `R`, and `RX`.
- Render common controlled or two-qubit gates such as `CX`, `CZ`, and `SWAP` where practical.
- Render generic fallback gate boxes for unsupported but valid operations.
- Render tick separators.
- Escape SVG text content so gate labels, display labels, and annotations cannot break XML.
- Use a small hand-written SVG writer instead of introducing a heavy dependency.
- Keep layout constants local and deterministic.
- Do not add CLI integration.
- Do not implement measurement anchors, detector or observable source resolution, repeat group decorations, sample-shot overlays, DEM-highlight overlays, or pixel-perfect Typst parity.

---

### Task 1: QP101 SVG Renderer Module

**Files:**
- Create: `rstim/src/qp101_svg.rs`
- Modify: `rstim/src/lib.rs`
- Create: `rstim/tests/qp101_svg.rs`

**Interfaces:**
- Consumes: `rstim::qp101::{Qp101Annotation, Qp101Display, Qp101Document, Qp101Operation, Qp101TargetRef}`.
- Produces: `rstim::qp101_svg::render_svg(doc: &Qp101Document) -> Result<String, String>`.

- [ ] **Step 1: Write the failing integration test**

Create `rstim/tests/qp101_svg.rs`:

```rust
use rstim::parser::parse_lines;
use rstim::qp101::{
    export_qp101, Qp101Display, Qp101Document, Qp101Operation, Qp101TargetRef,
};
use rstim::qp101_svg::render_svg;

#[test]
fn svg_renderer_draws_wires_gates_and_ticks() {
    let instrs = parse_lines(
        "QUBIT_COORDS(0, 0) 0\nQUBIT_COORDS(1, 0) 1\nH 0\nCX 0 1\nTICK\nM 0 1\n",
    )
    .expect("test circuit should parse");
    let mut doc = export_qp101(&instrs).expect("test circuit should export to QP101");
    doc.operations.push(Qp101Operation::Gate {
        gate: "CUSTOM".to_string(),
        targets: vec![1],
        controls: Vec::new(),
        control_configs: None,
        params: Vec::new(),
        raw_targets: Some(vec![Qp101TargetRef::Qubit {
            index: 1,
            inverted: None,
        }]),
        display: Some(Qp101Display {
            label: Some("A&B<test>".to_string()),
        }),
        tags: Vec::new(),
        annotations: Vec::new(),
    });

    let svg = render_svg(&doc).expect("renderer should produce SVG");

    assert!(svg.starts_with("<svg"), "SVG should start with <svg: {svg}");
    for marker in ["q0", "q1", "H", "CX", "tick"] {
        assert!(svg.contains(marker), "SVG missing semantic marker {marker}: {svg}");
    }
    for element in ["<line", "<rect", "<circle"] {
        assert!(svg.contains(element), "SVG missing visible element {element}: {svg}");
    }
    assert!(
        svg.contains("A&amp;B&lt;test&gt;"),
        "display label should be XML-escaped: {svg}"
    );
    assert!(
        !svg.contains("A&B<test>"),
        "display label must not appear as raw XML-sensitive text: {svg}"
    );
}

#[test]
fn svg_renderer_rejects_zero_qubits() {
    let doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 0,
        operations: Vec::new(),
        metadata: None,
        extensions: None,
    };

    let err = render_svg(&doc).expect_err("zero-qubit document should fail layout");
    assert!(
        err.contains("num_qubits") || err.contains("qubits"),
        "error should name num_qubits or qubits, got {err}"
    );
}
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_draws_wires_gates_and_ticks -q --offline
```

Expected: FAIL to compile because `rstim::qp101_svg` does not exist.

- [ ] **Step 3: Expose the module**

Modify `rstim/src/lib.rs` by adding the new module next to `qp101`:

```rust
pub mod qp101;
pub mod qp101_svg;
```

- [ ] **Step 4: Implement the renderer**

Create `rstim/src/qp101_svg.rs` with a small hand-written SVG writer. The final file should follow this structure and behavior:

```rust
use crate::qp101::{Qp101Annotation, Qp101Display, Qp101Document, Qp101Operation, Qp101TargetRef};

const LEFT_MARGIN: i32 = 56;
const RIGHT_MARGIN: i32 = 24;
const TOP_MARGIN: i32 = 32;
const BOTTOM_MARGIN: i32 = 32;
const LANE_GAP: i32 = 48;
const COLUMN_GAP: i32 = 72;
const GATE_WIDTH: i32 = 38;
const GATE_HEIGHT: i32 = 28;

pub fn render_svg(doc: &Qp101Document) -> Result<String, String> {
    if doc.num_qubits == 0 {
        return Err("cannot render QP101 SVG with num_qubits = 0".to_string());
    }

    let visible_columns = count_visible_columns(&doc.operations).max(1);
    let width = LEFT_MARGIN + RIGHT_MARGIN + (visible_columns as i32 + 1) * COLUMN_GAP;
    let height = TOP_MARGIN + BOTTOM_MARGIN + (doc.num_qubits.saturating_sub(1) as i32) * LANE_GAP;
    let mut out = String::new();

    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n"
    ));
    out.push_str("<g class=\"qp101-svg\" fill=\"none\" font-family=\"monospace\" font-size=\"14\">\n");
    render_wires(&mut out, doc.num_qubits, width);
    let mut column = 0usize;
    render_operations(&mut out, &doc.operations, doc.num_qubits, &mut column)?;
    out.push_str("</g>\n</svg>\n");
    Ok(out)
}

fn count_visible_columns(ops: &[Qp101Operation]) -> usize {
    ops.iter()
        .map(|op| match op {
            Qp101Operation::QubitCoords { .. } | Qp101Operation::ShiftCoords { .. } => 0,
            Qp101Operation::Repeat { body, .. } => 1 + count_visible_columns(body),
            _ => 1,
        })
        .sum()
}

fn render_wires(out: &mut String, num_qubits: usize, width: i32) {
    for q in 0..num_qubits {
        let y = lane_y(q);
        out.push_str(&format!(
            "<text x=\"8\" y=\"{}\" fill=\"#20252d\" dominant-baseline=\"middle\">q{q}</text>\n",
            y
        ));
        out.push_str(&format!(
            "<line class=\"wire\" x1=\"{LEFT_MARGIN}\" y1=\"{y}\" x2=\"{}\" y2=\"{y}\" stroke=\"#667085\" stroke-width=\"1\" />\n",
            width - RIGHT_MARGIN
        ));
    }
}

fn render_operations(
    out: &mut String,
    ops: &[Qp101Operation],
    num_qubits: usize,
    column: &mut usize,
) -> Result<(), String> {
    for op in ops {
        match op {
            Qp101Operation::QubitCoords { .. } | Qp101Operation::ShiftCoords { .. } => {}
            Qp101Operation::Tick { annotations } => {
                render_tick(out, x_for_column(*column), num_qubits, annotations);
                *column += 1;
            }
            Qp101Operation::Gate {
                gate,
                targets,
                controls,
                raw_targets,
                display,
                annotations,
                ..
            } => {
                let x = x_for_column(*column);
                render_gate(out, x, num_qubits, gate, targets, controls, raw_targets.as_deref(), display.as_ref(), annotations)?;
                *column += 1;
            }
            Qp101Operation::Noise {
                gate,
                raw_targets,
                annotations,
                ..
            } => {
                let x = x_for_column(*column);
                let lanes = raw_target_lanes(raw_targets, num_qubits, gate)?;
                render_generic_box(out, x, num_qubits, gate, &lanes, "#fff7ed")?;
                render_annotations(out, x, &lanes, annotations);
                *column += 1;
            }
            Qp101Operation::Repeat {
                count,
                body,
                annotations,
            } => {
                let x = x_for_column(*column);
                let label = format!("repeat x{count}");
                render_top_note(out, x, &label);
                render_annotations(out, x, &[0], annotations);
                *column += 1;
                render_operations(out, body, num_qubits, column)?;
            }
            Qp101Operation::Detector { annotations, .. } => {
                let x = x_for_column(*column);
                render_top_note(out, x, "detector");
                render_annotations(out, x, &[0], annotations);
                *column += 1;
            }
            Qp101Operation::ObservableInclude {
                index,
                annotations,
                ..
            } => {
                let x = x_for_column(*column);
                let label = format!("L{index}");
                render_top_note(out, x, &label);
                render_annotations(out, x, &[0], annotations);
                *column += 1;
            }
            Qp101Operation::Annotation {
                kind,
                text,
                annotations,
            } => {
                let x = x_for_column(*column);
                let label = format!("{kind}: {text}");
                render_top_note(out, x, &label);
                render_annotations(out, x, &[0], annotations);
                *column += 1;
            }
        }
    }
    Ok(())
}
```

The implementation must also include concrete helpers for:

```rust
fn lane_y(q: usize) -> i32 { TOP_MARGIN + q as i32 * LANE_GAP }
fn x_for_column(column: usize) -> i32 { LEFT_MARGIN + COLUMN_GAP + column as i32 * COLUMN_GAP }
```

Add `escape_xml`, `gate_label`, `gate_lanes`, `raw_target_lanes`, `validate_lane`, `render_gate`, `render_single_qubit_boxes`, `render_controlled_pair`, `render_swap_pair`, `render_generic_box`, `render_gate_box`, `render_tick`, `render_top_note`, and `render_annotations`.

The helper behavior must be exact:

- `escape_xml` replaces `&`, `<`, `>`, `"`, and `'` with XML entities.
- `gate_label` returns `display.label` when present, otherwise `gate`.
- `gate_lanes` validates every target and control qubit is `< num_qubits`.
- `raw_target_lanes` extracts qubits from `Qp101TargetRef::Qubit` and `Qp101TargetRef::Pauli`.
- `render_gate` special-cases `CX`, `CZ`, and `SWAP`; all other gates call `render_single_qubit_boxes` when each target is independent or `render_generic_box` for multi-lane fallback.
- `CX` and `CZ` use explicit `controls` when available; otherwise adjacent target pairs from `targets`.
- `render_controlled_pair` draws a vertical `<line>`, a filled control `<circle>`, a target `<rect>`, and text containing `CX` or `CZ`.
- `render_swap_pair` draws a vertical `<line>`, two cross marks, and escaped `SWAP` text.
- `render_tick` draws a dashed vertical `<line>` spanning all lanes and a `tick` text label.
- Every text label written to SVG goes through `escape_xml`.

- [ ] **Step 5: Run the focused test to verify GREEN**

Run:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_draws_wires_gates_and_ticks -q --offline
```

Expected: PASS.

- [ ] **Step 6: Run the zero-qubit negative control**

Run:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_rejects_zero_qubits -q --offline
```

Expected: PASS.

- [ ] **Step 7: Run the full renderer integration test**

Run:

```sh
cargo test -p rstim --test qp101_svg -q --offline
```

Expected: PASS.

- [ ] **Step 8: Run the issue verification command**

Run:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_draws_wires_gates_and_ticks -q
```

Expected: PASS when online registry access is available. If the sandbox blocks crates.io index access, rerun the same command with `--offline` and record the online failure.

- [ ] **Step 9: Run broad checks**

Run:

```sh
cargo test -p rstim --offline
git diff --check
```

Expected: PASS.

- [ ] **Step 10: Commit**

```sh
git add rstim/src/lib.rs rstim/src/qp101_svg.rs rstim/tests/qp101_svg.rs docs/superpowers/plans/2026-06-24-issue-166-qp101-svg-renderer.md
git commit -m "feat: add minimal qp101 svg renderer"
```
