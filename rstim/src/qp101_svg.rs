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
    out.push_str(
        "<g class=\"qp101-svg\" fill=\"none\" font-family=\"monospace\" font-size=\"14\">\n",
    );
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
                render_gate(
                    out,
                    x,
                    num_qubits,
                    gate,
                    targets,
                    controls,
                    raw_targets.as_deref(),
                    display.as_ref(),
                    annotations,
                )?;
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
                index, annotations, ..
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

fn lane_y(q: usize) -> i32 {
    TOP_MARGIN + q as i32 * LANE_GAP
}

fn x_for_column(column: usize) -> i32 {
    LEFT_MARGIN + COLUMN_GAP + column as i32 * COLUMN_GAP
}

fn escape_xml(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn gate_label(gate: &str, display: Option<&Qp101Display>) -> String {
    display
        .and_then(|display| display.label.as_deref())
        .unwrap_or(gate)
        .to_string()
}

fn gate_lanes(
    targets: &[u32],
    controls: &[u32],
    num_qubits: usize,
    gate: &str,
) -> Result<Vec<usize>, String> {
    let mut lanes = Vec::with_capacity(controls.len() + targets.len());
    for &control in controls {
        lanes.push(validate_lane(control, num_qubits, gate)?);
    }
    for &target in targets {
        lanes.push(validate_lane(target, num_qubits, gate)?);
    }
    Ok(lanes)
}

fn raw_target_lanes(
    raw_targets: &[Qp101TargetRef],
    num_qubits: usize,
    gate: &str,
) -> Result<Vec<usize>, String> {
    let mut lanes = Vec::new();
    for target in raw_targets {
        match target {
            Qp101TargetRef::Qubit { index, .. } => {
                lanes.push(validate_lane(*index, num_qubits, gate)?);
            }
            Qp101TargetRef::Pauli { qubit, .. } => {
                lanes.push(validate_lane(*qubit, num_qubits, gate)?);
            }
            Qp101TargetRef::Rec { .. }
            | Qp101TargetRef::Combiner
            | Qp101TargetRef::Sweep { .. } => {}
        }
    }
    Ok(lanes)
}

fn validate_lane(index: u32, num_qubits: usize, gate: &str) -> Result<usize, String> {
    let lane = usize::try_from(index)
        .map_err(|_| format!("gate {gate} uses invalid qubit index {index}"))?;
    if lane >= num_qubits {
        return Err(format!(
            "gate {gate} references qubit {index}, but num_qubits is {num_qubits}"
        ));
    }
    Ok(lane)
}

fn render_gate(
    out: &mut String,
    x: i32,
    num_qubits: usize,
    gate: &str,
    targets: &[u32],
    controls: &[u32],
    raw_targets: Option<&[Qp101TargetRef]>,
    display: Option<&Qp101Display>,
    annotations: &[Qp101Annotation],
) -> Result<(), String> {
    let label = gate_label(gate, display);
    let lanes = if let Some(raw_targets) = raw_targets {
        raw_target_lanes(raw_targets, num_qubits, gate)?
    } else {
        gate_lanes(targets, controls, num_qubits, gate)?
    };

    match gate {
        "CX" | "CZ" => {
            if let Some(pairs) = controlled_pairs(targets, controls, num_qubits, gate)? {
                for (control_lane, target_lane) in &pairs {
                    render_controlled_pair(out, x, *control_lane, *target_lane, gate);
                }
            } else {
                render_generic_box(out, x, num_qubits, &label, &lanes, "#eef2ff")?;
            }
        }
        "SWAP" => {
            if let Some(pairs) = target_pairs(targets, num_qubits, gate)? {
                for (lane_a, lane_b) in &pairs {
                    render_swap_pair(out, x, *lane_a, *lane_b);
                }
            } else {
                render_generic_box(out, x, num_qubits, &label, &lanes, "#ecfeff")?;
            }
        }
        _ => {
            if controls.is_empty() && !targets.is_empty() && raw_targets.is_none() {
                render_single_qubit_boxes(out, x, gate, display, targets, num_qubits)?;
            } else if controls.is_empty() && !lanes.is_empty() && lanes.len() == targets.len() {
                render_single_qubit_boxes(out, x, gate, display, targets, num_qubits)?;
            } else {
                render_generic_box(out, x, num_qubits, &label, &lanes, "#ffffff")?;
            }
        }
    }

    render_annotations(out, x, &lanes, annotations);
    Ok(())
}

fn controlled_pairs(
    targets: &[u32],
    controls: &[u32],
    num_qubits: usize,
    gate: &str,
) -> Result<Option<Vec<(usize, usize)>>, String> {
    if !controls.is_empty() {
        if controls.len() != targets.len() {
            return Ok(None);
        }
        let mut pairs = Vec::with_capacity(controls.len());
        for (&control, &target) in controls.iter().zip(targets.iter()) {
            pairs.push((
                validate_lane(control, num_qubits, gate)?,
                validate_lane(target, num_qubits, gate)?,
            ));
        }
        return Ok(Some(pairs));
    }
    target_pairs(targets, num_qubits, gate)
}

fn target_pairs(
    targets: &[u32],
    num_qubits: usize,
    gate: &str,
) -> Result<Option<Vec<(usize, usize)>>, String> {
    if targets.len() % 2 != 0 {
        return Ok(None);
    }
    let mut pairs = Vec::with_capacity(targets.len() / 2);
    for chunk in targets.chunks_exact(2) {
        pairs.push((
            validate_lane(chunk[0], num_qubits, gate)?,
            validate_lane(chunk[1], num_qubits, gate)?,
        ));
    }
    Ok(Some(pairs))
}

fn render_single_qubit_boxes(
    out: &mut String,
    x: i32,
    gate: &str,
    display: Option<&Qp101Display>,
    targets: &[u32],
    num_qubits: usize,
) -> Result<(), String> {
    let label = gate_label(gate, display);
    for &target in targets {
        let lane = validate_lane(target, num_qubits, gate)?;
        render_gate_box(out, x, lane_y(lane), &label, "#ffffff");
    }
    Ok(())
}

fn render_controlled_pair(
    out: &mut String,
    x: i32,
    control_lane: usize,
    target_lane: usize,
    gate: &str,
) {
    let control_y = lane_y(control_lane);
    let target_y = lane_y(target_lane);
    out.push_str(&format!(
        "<line class=\"{gate}\" x1=\"{x}\" y1=\"{control_y}\" x2=\"{x}\" y2=\"{target_y}\" stroke=\"#111827\" stroke-width=\"1.5\" />\n"
    ));
    out.push_str(&format!(
        "<circle class=\"control\" cx=\"{x}\" cy=\"{control_y}\" r=\"6\" fill=\"#111827\" />\n"
    ));
    out.push_str(&format!(
        "<rect class=\"target {gate}\" x=\"{}\" y=\"{}\" width=\"{GATE_WIDTH}\" height=\"{GATE_HEIGHT}\" rx=\"4\" ry=\"4\" stroke=\"#111827\" fill=\"#eef2ff\" />\n",
        x - GATE_WIDTH / 2,
        target_y - GATE_HEIGHT / 2
    ));
    out.push_str(&format!(
        "<text x=\"{x}\" y=\"{target_y}\" fill=\"#111827\" text-anchor=\"middle\" dominant-baseline=\"middle\">{}</text>\n",
        escape_xml(gate)
    ));
}

fn render_swap_pair(out: &mut String, x: i32, lane_a: usize, lane_b: usize) {
    let y1 = lane_y(lane_a);
    let y2 = lane_y(lane_b);
    out.push_str(&format!(
        "<line class=\"SWAP\" x1=\"{x}\" y1=\"{y1}\" x2=\"{x}\" y2=\"{y2}\" stroke=\"#0f172a\" stroke-width=\"1.5\" />\n"
    ));
    for y in [y1, y2] {
        out.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#0f172a\" stroke-width=\"1.5\" />\n",
            x - 7,
            y - 7,
            x + 7,
            y + 7
        ));
        out.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#0f172a\" stroke-width=\"1.5\" />\n",
            x - 7,
            y + 7,
            x + 7,
            y - 7
        ));
    }
    render_top_note(out, x, "SWAP");
}

fn render_generic_box(
    out: &mut String,
    x: i32,
    _num_qubits: usize,
    label: &str,
    lanes: &[usize],
    fill: &str,
) -> Result<(), String> {
    if lanes.is_empty() {
        render_top_note(out, x, label);
        return Ok(());
    }
    let min_lane = *lanes.iter().min().expect("checked non-empty");
    let max_lane = *lanes.iter().max().expect("checked non-empty");
    let top = lane_y(min_lane) - GATE_HEIGHT / 2;
    let bottom = lane_y(max_lane) + GATE_HEIGHT / 2;
    let height = bottom - top;
    out.push_str(&format!(
        "<rect class=\"gate-box\" x=\"{}\" y=\"{top}\" width=\"{GATE_WIDTH}\" height=\"{height}\" rx=\"4\" ry=\"4\" stroke=\"#111827\" fill=\"{fill}\" />\n",
        x - GATE_WIDTH / 2
    ));
    let text_y = (top + bottom) / 2;
    out.push_str(&format!(
        "<text x=\"{x}\" y=\"{text_y}\" fill=\"#111827\" text-anchor=\"middle\" dominant-baseline=\"middle\">{}</text>\n",
        escape_xml(label)
    ));
    Ok(())
}

fn render_gate_box(out: &mut String, x: i32, y: i32, label: &str, fill: &str) {
    out.push_str(&format!(
        "<rect class=\"gate-box\" x=\"{}\" y=\"{}\" width=\"{GATE_WIDTH}\" height=\"{GATE_HEIGHT}\" rx=\"4\" ry=\"4\" stroke=\"#111827\" fill=\"{fill}\" />\n",
        x - GATE_WIDTH / 2,
        y - GATE_HEIGHT / 2
    ));
    out.push_str(&format!(
        "<text x=\"{x}\" y=\"{y}\" fill=\"#111827\" text-anchor=\"middle\" dominant-baseline=\"middle\">{}</text>\n",
        escape_xml(label)
    ));
}

fn render_tick(out: &mut String, x: i32, num_qubits: usize, annotations: &[Qp101Annotation]) {
    let y1 = lane_y(0) - GATE_HEIGHT / 2;
    let y2 = lane_y(num_qubits.saturating_sub(1)) + GATE_HEIGHT / 2;
    out.push_str(&format!(
        "<line class=\"tick\" x1=\"{x}\" y1=\"{y1}\" x2=\"{x}\" y2=\"{y2}\" stroke=\"#98a2b3\" stroke-width=\"1\" stroke-dasharray=\"4 4\" />\n"
    ));
    out.push_str(&format!(
        "<text x=\"{x}\" y=\"{}\" fill=\"#475467\" text-anchor=\"middle\">tick</text>\n",
        y1 - 8
    ));
    render_annotations(out, x, &[0], annotations);
}

fn render_top_note(out: &mut String, x: i32, label: &str) {
    out.push_str(&format!(
        "<text x=\"{x}\" y=\"{}\" fill=\"#475467\" text-anchor=\"middle\">{}</text>\n",
        TOP_MARGIN - 18,
        escape_xml(label)
    ));
}

fn render_annotations(out: &mut String, x: i32, lanes: &[usize], annotations: &[Qp101Annotation]) {
    let base_lane = lanes.first().copied().unwrap_or(0);
    let base_y = lane_y(base_lane) + GATE_HEIGHT / 2 + 14;
    for (idx, annotation) in annotations.iter().enumerate() {
        let mut parts = Vec::new();
        parts.push(annotation.kind.clone());
        if let Some(label) = annotation.label.as_deref() {
            parts.push(label.to_string());
        }
        if let Some(text) = annotation.text.as_deref() {
            parts.push(text.to_string());
        }
        let content = escape_xml(&parts.join(": "));
        out.push_str(&format!(
            "<text x=\"{x}\" y=\"{}\" fill=\"#7a5af8\" text-anchor=\"middle\" font-size=\"11\">{content}</text>\n",
            base_y + idx as i32 * 12
        ));
    }
}
