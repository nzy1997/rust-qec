# QP101 SVG Source Labels Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render built-in QP101 SVG detector and observable labels with safe, resolved measurement-anchor source text.

**Architecture:** Extend the internal SVG render state from #168 so measurement anchor assignment also records individual measurement anchors and host qubit lanes. Resolve detector and observable `sources` against that state at render time, falling back to raw textual target refs whenever a source cannot be resolved.

**Tech Stack:** Rust 2024, existing `rstim::qp101` data model, hand-written SVG string output, integration tests in `rstim/tests/qp101_svg.rs`.

## Global Constraints

- Do not change the QP101 JSON schema or exporter semantics.
- Do not mutate the input `Qp101Document`.
- Resolve valid negative `Qp101TargetRef::Rec` sources only when the referenced measurement exists in renderer measurement history.
- Keep unresolved `rec` sources visible as raw text such as `rec[-99]`.
- Keep non-`rec` sources visible textually, including sweep and Pauli refs.
- Detector indices are renderer-only and monotonic in visual order, starting at `D0`.
- Observable labels preserve the QP101 observable index, such as `L2`.
- Choose the minimum resolved measurement-source qubit for host-lane placement; otherwise choose the minimum explicit source qubit; otherwise use `q0`.
- Do not draw connector lines, add coordinate-layout rendering, or add sample-shot detector flip highlighting.

---

### Task 1: Add Detector and Observable Source Regression Coverage

**Files:**
- Modify: `rstim/tests/qp101_svg.rs`

**Interfaces:**
- Consumes: `rstim::qp101_svg::render_svg(doc: &Qp101Document) -> Result<String, String>`
- Produces: Failing integration test `svg_renderer_resolves_detector_observable_sources`

- [ ] **Step 1: Add the failing integration test**

Add this test before `svg_renderer_labels_measurements_with_global_anchors`:

```rust
#[test]
fn svg_renderer_resolves_detector_observable_sources() {
    let detector_doc = export_qp101(
        &parse_lines("M 0\nDETECTOR rec[-1]\n").expect("detector source fixture should parse"),
    )
    .expect("detector source fixture should export");
    let detector_svg = render_svg(&detector_doc).expect("detector source fixture should render");

    for marker in [">m1</text>", ">DETECTOR</text>", ">D0 = m1</text>"] {
        assert!(
            detector_svg.contains(marker),
            "detector SVG should contain {marker}: {detector_svg}"
        );
    }
    assert!(
        !detector_svg.contains(">D0 = rec[-1]</text>"),
        "detector source should resolve to the existing measurement anchor: {detector_svg}"
    );

    let observable_doc = export_qp101(
        &parse_lines("M 0\nOBSERVABLE_INCLUDE(2) rec[-1]\n")
            .expect("observable source fixture should parse"),
    )
    .expect("observable source fixture should export");
    let observable_svg =
        render_svg(&observable_doc).expect("observable source fixture should render");

    for marker in [">OBS_INCLUDE(2)</text>", ">L2 *= m1</text>"] {
        assert!(
            observable_svg.contains(marker),
            "observable SVG should contain {marker}: {observable_svg}"
        );
    }

    let malformed_doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 1,
        operations: vec![
            Qp101Operation::Gate {
                gate: "M".to_string(),
                targets: vec![0],
                controls: Vec::new(),
                control_configs: None,
                params: Vec::new(),
                raw_targets: None,
                display: None,
                tags: Vec::new(),
                annotations: Vec::new(),
            },
            Qp101Operation::Detector {
                coords: Vec::new(),
                sources: vec![Qp101TargetRef::Rec { offset: -99 }],
                annotations: Vec::new(),
            },
        ],
        metadata: None,
        extensions: None,
    };
    let malformed_svg = render_svg(&malformed_doc).expect("malformed source should render");

    assert!(
        malformed_svg.contains(">D0 = rec[-99]</text>"),
        "unavailable rec source should remain visible as raw text: {malformed_svg}"
    );
    assert!(
        !malformed_svg.contains(">D0 = m1</text>"),
        "unavailable rec source must not invent the nearest anchor: {malformed_svg}"
    );

    let hand_built_doc = Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 2,
        operations: vec![
            Qp101Operation::Detector {
                coords: Vec::new(),
                sources: vec![
                    Qp101TargetRef::Sweep { index: 0 },
                    Qp101TargetRef::Pauli {
                        basis: Qp101PauliBasis::X,
                        qubit: 1,
                        inverted: Some(true),
                    },
                ],
                annotations: Vec::new(),
            },
            Qp101Operation::ObservableInclude {
                index: 3,
                sources: vec![Qp101TargetRef::Qubit {
                    index: 1,
                    inverted: Some(true),
                }],
                annotations: Vec::new(),
            },
        ],
        metadata: None,
        extensions: None,
    };
    let hand_built_svg = render_svg(&hand_built_doc).expect("hand-built sources should render");

    for marker in [
        ">D0 = sweep[0]*!X1</text>",
        ">OBS_INCLUDE(3)</text>",
        ">L3 *= !q1</text>",
    ] {
        assert!(
            hand_built_svg.contains(marker),
            "hand-built source SVG should contain {marker}: {hand_built_svg}"
        );
    }
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_resolves_detector_observable_sources -q
```

Expected: the test fails because current SVG output does not contain `DETECTOR`, `D0 = m1`, `OBS_INCLUDE(2)`, or textual non-`rec` source labels.

### Task 2: Implement Safe Source Resolution in the SVG Renderer

**Files:**
- Modify: `rstim/src/qp101_svg.rs`
- Modify: `rstim/tests/qp101_svg.rs`

**Interfaces:**
- Consumes: Internal `RenderState`, `MeasurementTarget`, and `Qp101TargetRef`
- Produces: Internal `MeasurementRecord`, source-resolution helpers, and detector/observable SVG labels

- [ ] **Step 1: Add measurement history state**

In `rstim/src/qp101_svg.rs`, add this struct after `MeasurementTarget`:

```rust
#[derive(Debug, Clone)]
struct MeasurementRecord {
    index: usize,
    lane: usize,
}

impl MeasurementRecord {
    fn anchor(&self) -> String {
        format!("m{}", self.index)
    }
}
```

Then change `RenderState` to:

```rust
#[derive(Debug, Default)]
struct RenderState {
    next_measurement_index: usize,
    next_detector_index: usize,
    measurements: Vec<MeasurementRecord>,
}
```

- [ ] **Step 2: Store individual measurement records**

In `measurement_targets`, after `state.next_measurement_index += output_count;`, append each emitted measurement record:

```rust
for output_offset in 0..output_count {
    state.measurements.push(MeasurementRecord {
        index: first_index + output_offset,
        lane,
    });
}
```

- [ ] **Step 3: Add target-ref source formatting helpers**

Add these helpers near `escape_xml`:

```rust
#[derive(Debug)]
struct SourceLabel {
    text: String,
    host_lane: usize,
}

fn source_label(
    sources: &[Qp101TargetRef],
    measurements: &[MeasurementRecord],
    num_qubits: usize,
) -> SourceLabel {
    let mut pieces = Vec::new();
    let mut resolved_lanes = Vec::new();
    let mut fallback_lanes = Vec::new();

    for source in sources {
        let resolved = resolve_source_ref(source, measurements);
        pieces.push(resolved.text);
        if let Some(lane) = resolved.resolved_lane {
            resolved_lanes.push(lane);
        }
        if let Some(lane) = target_ref_lane(source, num_qubits) {
            fallback_lanes.push(lane);
        }
    }

    let host_lane = resolved_lanes
        .into_iter()
        .min()
        .or_else(|| fallback_lanes.into_iter().min())
        .unwrap_or(0);
    let text = if pieces.is_empty() {
        "-".to_string()
    } else {
        pieces.join("*")
    };

    SourceLabel { text, host_lane }
}

#[derive(Debug)]
struct ResolvedSourceRef {
    text: String,
    resolved_lane: Option<usize>,
}

fn resolve_source_ref(
    source: &Qp101TargetRef,
    measurements: &[MeasurementRecord],
) -> ResolvedSourceRef {
    if let Qp101TargetRef::Rec { offset } = source {
        if *offset < 0 {
            let resolved_index = measurements.len() as i64 + i64::from(*offset) + 1;
            if resolved_index >= 1 && resolved_index <= measurements.len() as i64 {
                if let Some(measurement) = measurements
                    .iter()
                    .find(|measurement| measurement.index == resolved_index as usize)
                {
                    return ResolvedSourceRef {
                        text: measurement.anchor(),
                        resolved_lane: Some(measurement.lane),
                    };
                }
            }
        }
    }

    ResolvedSourceRef {
        text: target_ref_text(source),
        resolved_lane: None,
    }
}

fn target_ref_lane(source: &Qp101TargetRef, num_qubits: usize) -> Option<usize> {
    let index = match source {
        Qp101TargetRef::Qubit { index, .. } => *index,
        Qp101TargetRef::Pauli { qubit, .. } => *qubit,
        Qp101TargetRef::Rec { .. } | Qp101TargetRef::Combiner | Qp101TargetRef::Sweep { .. } => {
            return None;
        }
    };
    let lane = usize::try_from(index).ok()?;
    (lane < num_qubits).then_some(lane)
}

fn target_ref_text(source: &Qp101TargetRef) -> String {
    match source {
        Qp101TargetRef::Qubit { index, inverted } => {
            format!("{}q{index}", inverted_prefix(*inverted))
        }
        Qp101TargetRef::Rec { offset } => format!("rec[{offset}]"),
        Qp101TargetRef::Pauli {
            basis,
            qubit,
            inverted,
        } => format!("{}{}{qubit}", inverted_prefix(*inverted), pauli_basis_text(basis)),
        Qp101TargetRef::Combiner => "*".to_string(),
        Qp101TargetRef::Sweep { index } => format!("sweep[{index}]"),
    }
}

fn inverted_prefix(inverted: Option<bool>) -> &'static str {
    if inverted.unwrap_or(false) { "!" } else { "" }
}

fn pauli_basis_text(basis: &Qp101PauliBasis) -> &'static str {
    match basis {
        Qp101PauliBasis::X => "X",
        Qp101PauliBasis::Y => "Y",
        Qp101PauliBasis::Z => "Z",
    }
}
```

- [ ] **Step 4: Render source operators on their host lane**

Add this helper near `render_gate_box`:

```rust
fn render_source_operation(out: &mut String, x: i32, lane: usize, label: &str, source: &str) {
    render_gate_box(out, x, lane_y(lane), label, "#f8fafc");
    out.push_str(&format!(
        "<text class=\"source-label\" x=\"{x}\" y=\"{}\" fill=\"#475467\" text-anchor=\"middle\" font-size=\"11\">{}</text>\n",
        below_gate_text_y(lane),
        escape_xml(source)
    ));
}
```

- [ ] **Step 5: Use source labels for detector and observable operations**

Replace the `Qp101Operation::Detector` and `Qp101Operation::ObservableInclude` match arms in `render_operations` with source-aware rendering:

```rust
Qp101Operation::Detector {
    sources,
    annotations,
    ..
} => {
    let x = x_for_column(*column);
    let detector_index = state.next_detector_index;
    state.next_detector_index += 1;
    let source = source_label(sources, &state.measurements, num_qubits);
    render_source_operation(
        out,
        x,
        source.host_lane,
        "DETECTOR",
        &format!("D{detector_index} = {}", source.text),
    );
    render_annotations_with_line_offset(out, x, &[source.host_lane], annotations, 1);
    *column += 1;
}
Qp101Operation::ObservableInclude {
    index,
    sources,
    annotations,
    ..
} => {
    let x = x_for_column(*column);
    let source = source_label(sources, &state.measurements, num_qubits);
    render_source_operation(
        out,
        x,
        source.host_lane,
        &format!("OBS_INCLUDE({index})"),
        &format!("L{index} *= {}", source.text),
    );
    render_annotations_with_line_offset(out, x, &[source.host_lane], annotations, 1);
    *column += 1;
}
```

- [ ] **Step 6: Include detector and observable source labels in height calculation**

Replace `max_rendered_below_gate_text_baseline` with this stateful version:

```rust
fn max_rendered_below_gate_text_baseline(
    ops: &[Qp101Operation],
    num_qubits: usize,
) -> Result<Option<i32>, String> {
    let mut state = RenderState::default();
    max_rendered_below_gate_text_baseline_with_state(ops, num_qubits, &mut state)
}

fn max_rendered_below_gate_text_baseline_with_state(
    ops: &[Qp101Operation],
    num_qubits: usize,
    state: &mut RenderState,
) -> Result<Option<i32>, String> {
    let mut max_baseline = None;
    for op in ops {
        match op {
            Qp101Operation::QubitCoords { .. } | Qp101Operation::ShiftCoords { .. } => {}
            Qp101Operation::Tick { annotations } => {
                update_max_baseline_from_annotations(&mut max_baseline, &[0usize], annotations, 0);
            }
            Qp101Operation::Gate {
                gate,
                targets,
                controls,
                raw_targets,
                annotations,
                ..
            } => {
                let lanes = if let Some(raw_targets) = raw_targets {
                    raw_target_lanes(raw_targets, num_qubits, gate)?
                } else {
                    gate_lanes(targets, controls, num_qubits, gate)?
                };
                let measurement_targets = measurement_targets(gate, targets, num_qubits, state)?;
                for target in &measurement_targets {
                    update_max_baseline(&mut max_baseline, below_gate_text_y(target.lane));
                }
                update_max_baseline_from_annotations(
                    &mut max_baseline,
                    &lanes,
                    annotations,
                    usize::from(!measurement_targets.is_empty()),
                );
            }
            Qp101Operation::Noise {
                gate,
                raw_targets,
                annotations,
                ..
            } => {
                let lanes = raw_target_lanes(raw_targets, num_qubits, gate)?;
                update_max_baseline_from_annotations(&mut max_baseline, &lanes, annotations, 0);
            }
            Qp101Operation::Repeat {
                count,
                body,
                annotations,
            } => {
                update_max_baseline_from_annotations(&mut max_baseline, &[0usize], annotations, 0);
                for _ in 0..*count {
                    if let Some(body_baseline) =
                        max_rendered_below_gate_text_baseline_with_state(body, num_qubits, state)?
                    {
                        update_max_baseline(&mut max_baseline, body_baseline);
                    }
                }
            }
            Qp101Operation::Detector {
                sources,
                annotations,
                ..
            } => {
                let source = source_label(sources, &state.measurements, num_qubits);
                update_max_baseline(&mut max_baseline, below_gate_text_y(source.host_lane));
                update_max_baseline_from_annotations(
                    &mut max_baseline,
                    &[source.host_lane],
                    annotations,
                    1,
                );
                state.next_detector_index += 1;
            }
            Qp101Operation::ObservableInclude {
                sources,
                annotations,
                ..
            } => {
                let source = source_label(sources, &state.measurements, num_qubits);
                update_max_baseline(&mut max_baseline, below_gate_text_y(source.host_lane));
                update_max_baseline_from_annotations(
                    &mut max_baseline,
                    &[source.host_lane],
                    annotations,
                    1,
                );
            }
            Qp101Operation::Annotation { annotations, .. } => {
                update_max_baseline_from_annotations(&mut max_baseline, &[0usize], annotations, 0);
            }
        }
    }
    Ok(max_baseline)
}
```

- [ ] **Step 7: Update existing fallback-renderer expectation**

In `svg_renderer_renders_qp101_fallback_operations_and_annotations`, update the marker list from `detector` and `L7` to `DETECTOR`, `D0 = m1`, `OBS_INCLUDE(7)`, and `L7 *= m1`.

- [ ] **Step 8: Run the focused test and verify GREEN**

Run:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_resolves_detector_observable_sources -q
```

Expected: the new test passes.

- [ ] **Step 9: Run the QP101 SVG integration tests**

Run:

```sh
cargo test -p rstim --test qp101_svg -q
```

Expected: all QP101 SVG integration tests pass.

- [ ] **Step 10: Format changed Rust files**

Run:

```sh
cargo fmt -- rstim/src/qp101_svg.rs rstim/tests/qp101_svg.rs
```

Expected: command exits 0 and leaves only intentional formatting changes.

### Task 3: Final Verification and Commit

**Files:**
- Modify: `docs/superpowers/plans/2026-06-24-issue-169-qp101-svg-source-labels.md`
- Modify: `rstim/src/qp101_svg.rs`
- Modify: `rstim/tests/qp101_svg.rs`

**Interfaces:**
- Consumes: Passing focused implementation from Task 2
- Produces: Committed implementation ready for PR

- [ ] **Step 1: Run required focused verification**

Run:

```sh
cargo test -p rstim --test qp101_svg svg_renderer_resolves_detector_observable_sources -q
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
git add rstim/src/qp101_svg.rs rstim/tests/qp101_svg.rs docs/superpowers/plans/2026-06-24-issue-169-qp101-svg-source-labels.md
git commit -m "feat: resolve qp101 svg detector sources"
```

Expected: one implementation commit containing source resolution, regression tests, and this plan.
