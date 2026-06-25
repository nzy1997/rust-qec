# Issue 171 QP101 SVG Noise Boxes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render common QEC `noise` operations in the built-in QP101 SVG renderer as compact timeline boxes with visible labels and parameter notes.

**Architecture:** Keep the existing `rstim::qp101_svg::render_svg(&Qp101Document) -> Result<String, String>` interface and add renderer-local noise policy helpers. Noise rendering stays separate from annotation rendering and reuses the current raw-target validation, SVG escaping, and generic fallback behavior.

**Tech Stack:** Rust 2024, existing `rstim::qp101` data model, hand-written SVG string writer, existing `rstim::parser::parse_lines` and `rstim::qp101::export_qp101` in tests.

## Global Constraints

- The public renderer interface remains `pub fn render_svg(doc: &Qp101Document) -> Result<String, String>`.
- The renderer must consume QP101 `noise` operations with `gate`, `params`, and `raw_targets`.
- Render `X_ERROR` as compact per-target boxes labeled `XE`.
- Render `Z_ERROR` as compact per-target boxes labeled `ZE`.
- Render `DEPOLARIZE1` as compact per-target boxes labeled `D1`.
- Render `DEPOLARIZE2` as compact paired target groups labeled `D2` when the target list is well formed.
- Render base `LOSS` visibly as `LOSS`.
- Render generic fallback boxes for other `noise` operations.
- Keep probability notes or parameter text visible when `params` exists.
- Malformed paired `DEPOLARIZE2` target groups must not panic or silently drop the operation; this plan uses visible generic fallback labeled `DEPOLARIZE2`.
- Keep annotation rendering separate; do not add sample-shot fired-branch annotations or DEM-origin highlight markers.
- Use a small hand-written SVG writer instead of introducing a dependency.
- Escape SVG text content.
- Keep layout constants local and deterministic.
- Do not add CLI integration or change QP101 export/schema behavior.

---

### Task 1: Compact Noise Rendering

**Files:**
- Modify: `rstim/tests/qp101_svg.rs`
- Modify: `rstim/src/qp101_svg.rs`

**Interfaces:**
- Consumes: `Qp101Operation::Noise { gate: String, params: Vec<f64>, raw_targets: Vec<Qp101TargetRef>, annotations: Vec<Qp101Annotation> }`.
- Produces: SVG text containing compact known-noise labels, parameter notes, existing wire labels, and ordinary neighboring gate labels.

- [ ] **Step 1: Write failing integration tests**

Append these tests to `rstim/tests/qp101_svg.rs` before the existing `annotation` helper:

```rust
#[test]
fn svg_renderer_draws_noise_boxes() {
    let instrs = parse_lines(
        "H 0\n\
         X_ERROR(0.1) 0\n\
         Z_ERROR(0.2) 1\n\
         DEPOLARIZE1(0.3) 0\n\
         DEPOLARIZE2(0.4) 0 1\n\
         LOSS(0.5) 1\n\
         M 0\n",
    )
    .expect("test circuit should parse");
    let doc = export_qp101(&instrs).expect("test circuit should export to QP101");

    let svg = render_svg(&doc).expect("renderer should produce SVG");

    for marker in ["q0", "q1", "H", "M"] {
        assert!(
            svg.contains(marker),
            "SVG should preserve neighboring timeline marker {marker}: {svg}"
        );
    }
    for label in ["XE", "ZE", "D1", "D2", "LOSS"] {
        let marker = format!(">{label}</text>");
        assert!(
            svg.contains(&marker),
            "SVG missing compact noise label {label}: {svg}"
        );
    }
    assert!(
        svg.contains("p=0.1"),
        "noise parameter note should remain visible for X_ERROR: {svg}"
    );
    assert!(
        svg.matches("class=\"noise-box\"").count() >= 6,
        "known noise should render as compact per-target or paired boxes: {svg}"
    );
}

#[test]
fn svg_renderer_falls_back_for_odd_depolarize2_targets() {
    let doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 2,
        operations: vec![Qp101Operation::Noise {
            gate: "DEPOLARIZE2".to_string(),
            params: vec![0.4],
            raw_targets: vec![
                Qp101TargetRef::Qubit {
                    index: 0,
                    inverted: None,
                },
                Qp101TargetRef::Qubit {
                    index: 1,
                    inverted: None,
                },
                Qp101TargetRef::Qubit {
                    index: 0,
                    inverted: None,
                },
            ],
            annotations: Vec::new(),
        }],
        metadata: None,
        extensions: None,
    };

    let svg = render_svg(&doc).expect("odd DEPOLARIZE2 target groups should visibly fall back");

    assert!(
        svg.contains(">DEPOLARIZE2</text>"),
        "odd paired noise should keep a visible generic DEPOLARIZE2 label: {svg}"
    );
    assert!(
        svg.contains("p=0.4"),
        "fallback noise should still show parameter text: {svg}"
    );
    assert!(
        svg.contains("class=\"gate-box\""),
        "odd paired noise should use the generic fallback box: {svg}"
    );
}
```

- [ ] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_draws_noise_boxes -q
```

Expected: FAIL because current noise rendering uses full labels such as `X_ERROR` and does not emit `XE`, `D1`, `D2`, `LOSS` compact noise boxes or `p=...` notes.

- [ ] **Step 3: Route noise operations through a dedicated renderer**

In `rstim/src/qp101_svg.rs`, change the `Qp101Operation::Noise` match arm from:

```rust
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
```

to:

```rust
Qp101Operation::Noise {
    gate,
    params,
    raw_targets,
    annotations,
} => {
    let x = x_for_column(*column);
    render_noise(out, x, num_qubits, gate, params, raw_targets, annotations)?;
    *column += 1;
}
```

- [ ] **Step 4: Add noise policy and drawing helpers**

Add these helpers after `is_simple_single_qubit_gate` and before `render_single_qubit_boxes` in `rstim/src/qp101_svg.rs`:

```rust
enum NoisePolicy {
    Single,
    Pair,
    Fallback,
}

fn noise_policy(gate: &str) -> NoisePolicy {
    match gate {
        "X_ERROR" | "Z_ERROR" | "DEPOLARIZE1" | "LOSS" => NoisePolicy::Single,
        "DEPOLARIZE2" => NoisePolicy::Pair,
        _ => NoisePolicy::Fallback,
    }
}

fn noise_label(gate: &str) -> &str {
    match gate {
        "X_ERROR" => "XE",
        "Z_ERROR" => "ZE",
        "DEPOLARIZE1" => "D1",
        "DEPOLARIZE2" => "D2",
        "LOSS" => "LOSS",
        _ => gate,
    }
}

fn noise_param_note(params: &[f64]) -> Option<String> {
    if params.is_empty() {
        return None;
    }
    let values = params
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!("p={values}"))
}

fn render_noise(
    out: &mut String,
    x: i32,
    num_qubits: usize,
    gate: &str,
    params: &[f64],
    raw_targets: &[Qp101TargetRef],
    annotations: &[Qp101Annotation],
) -> Result<(), String> {
    let lanes = raw_target_lanes(raw_targets, num_qubits, gate)?;
    let note = noise_param_note(params);

    match noise_policy(gate) {
        NoisePolicy::Single if !lanes.is_empty() => {
            if let Some(note) = note.as_deref() {
                render_param_note(out, x, &lanes, note);
            }
            for &lane in &lanes {
                render_noise_box(out, x, lane_y(lane), noise_label(gate));
            }
        }
        NoisePolicy::Pair if !lanes.is_empty() && lanes.len() % 2 == 0 => {
            if let Some(note) = note.as_deref() {
                render_param_note(out, x, &lanes, note);
            }
            for pair in lanes.chunks_exact(2) {
                render_noise_pair(out, x, pair[0], pair[1], noise_label(gate));
            }
        }
        _ => {
            if let Some(note) = note.as_deref() {
                render_param_note(out, x, &lanes, note);
            }
            render_generic_box(out, x, num_qubits, gate, &lanes, "#fff7ed")?;
        }
    }

    render_annotations(out, x, &lanes, annotations);
    Ok(())
}

fn render_param_note(out: &mut String, x: i32, lanes: &[usize], note: &str) {
    let y = lanes
        .iter()
        .min()
        .map(|lane| lane_y(*lane) - GATE_HEIGHT / 2 - 6)
        .unwrap_or(TOP_MARGIN - 4);
    out.push_str(&format!(
        "<text class=\"param-note\" x=\"{x}\" y=\"{y}\" fill=\"#475467\" text-anchor=\"middle\" font-size=\"11\">{}</text>\n",
        escape_xml(note)
    ));
}

fn render_noise_box(out: &mut String, x: i32, y: i32, label: &str) {
    out.push_str(&format!(
        "<rect class=\"noise-box\" x=\"{}\" y=\"{}\" width=\"{GATE_WIDTH}\" height=\"{GATE_HEIGHT}\" rx=\"4\" ry=\"4\" stroke=\"#9a3412\" fill=\"#fff7ed\" />\n",
        x - GATE_WIDTH / 2,
        y - GATE_HEIGHT / 2
    ));
    out.push_str(&format!(
        "<text x=\"{x}\" y=\"{y}\" fill=\"#111827\" text-anchor=\"middle\" dominant-baseline=\"middle\">{}</text>\n",
        escape_xml(label)
    ));
}

fn render_noise_pair(out: &mut String, x: i32, lane_a: usize, lane_b: usize, label: &str) {
    let y1 = lane_y(lane_a);
    let y2 = lane_y(lane_b);
    out.push_str(&format!(
        "<line class=\"noise-pair\" x1=\"{x}\" y1=\"{y1}\" x2=\"{x}\" y2=\"{y2}\" stroke=\"#9a3412\" stroke-width=\"1.5\" />\n"
    ));
    render_noise_box(out, x, y1, label);
    render_noise_box(out, x, y2, label);
}
```

- [ ] **Step 5: Run focused tests to verify GREEN**

Run:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_draws_noise_boxes -q
cargo test -p rstim --test qp101_svg svg_renderer_falls_back_for_odd_depolarize2_targets -q
```

Expected: both tests PASS.

- [ ] **Step 6: Run renderer test module and formatting**

Run:

```sh
cargo test -p rstim --test qp101_svg -q
cargo fmt --check -p rstim
```

Expected: `qp101_svg` tests PASS. If `cargo fmt --check -p rstim` reports pre-existing formatting drift outside the touched files, run `rustfmt --check rstim/src/qp101_svg.rs rstim/tests/qp101_svg.rs` and record the drift.

- [ ] **Step 7: Run full verification**

Run:

```sh
cargo test
git diff --check
```

Expected: both commands PASS. Existing warnings are acceptable if tests exit 0.

- [ ] **Step 8: Commit implementation**

Run:

```sh
git add rstim/src/qp101_svg.rs rstim/tests/qp101_svg.rs docs/superpowers/plans/2026-06-24-issue-171-qp101-svg-noise-boxes.md
git commit -m "feat: render qp101 svg noise boxes"
```

Expected: commit succeeds with only the renderer, test, and plan changes staged.
