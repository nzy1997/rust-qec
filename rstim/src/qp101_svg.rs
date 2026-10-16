use crate::qp101::{
    Qp101Annotation, Qp101Display, Qp101Document, Qp101Operation, Qp101PauliBasis, Qp101TargetRef,
};

const LEFT_MARGIN: i32 = 56;
const RIGHT_MARGIN: i32 = 24;
const TOP_MARGIN: i32 = 56;
const BOTTOM_MARGIN: i32 = 32;
const LANE_GAP: i32 = 88;
const COLUMN_GAP: i32 = 72;
const GATE_WIDTH: i32 = 38;
const GATE_HEIGHT: i32 = 28;
const ANNOTATION_LINE_GAP: i32 = 12;
const BELOW_GATE_TEXT_BOTTOM_PAD: i32 = 4;
const ABOVE_GATE_TEXT_GAP: i32 = 8;
const SOURCE_GATE_MIN_WIDTH: i32 = 64;
const SOURCE_GATE_TEXT_PAD: i32 = 24;
const SOURCE_GATE_CHAR_WIDTH: i32 = 6;
const SOURCE_OPERATION_MIN_COLUMN_SPAN: usize = 2;
const REPEAT_GROUP_TOP_PAD: i32 = 8;
const REPEAT_GROUP_BOTTOM_PAD: i32 = 8;
const REPEAT_GROUP_X_PAD: i32 = 4;
const REPEAT_GROUP_LABEL_X_PAD: i32 = 8;
const REPEAT_GROUP_LABEL_DEPTH_STAGGER: i32 = 16;
const REPEAT_GROUP_LABEL_GATE_GAP: i32 = 4;
const REPEAT_LABEL_TOP_RESERVE: i32 = 16;
const REPEAT_ANNOTATION_LINE_OFFSET: usize = 1;

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

#[derive(Debug, Clone)]
struct RepeatGroupSpan {
    count: u64,
    start_column: usize,
    end_column: usize,
    iteration_starts: Vec<usize>,
    depth: usize,
}

#[derive(Debug, Default)]
struct RenderState {
    next_measurement_index: usize,
    next_detector_index: usize,
    measurements: Vec<MeasurementRecord>,
    repeat_groups: Vec<RepeatGroupSpan>,
    repeat_depth: usize,
}

#[derive(Debug, Clone, Copy)]
struct LaneSpan {
    min: usize,
    max: usize,
}

impl LaneSpan {
    fn from_lanes(lanes: &[usize], num_qubits: usize) -> Self {
        if lanes.is_empty() {
            return LaneSpan {
                min: 0,
                max: num_qubits.saturating_sub(1),
            };
        }
        LaneSpan {
            min: *lanes.iter().min().expect("checked non-empty"),
            max: *lanes.iter().max().expect("checked non-empty"),
        }
    }

    fn conflicts(self, other: LaneSpan) -> bool {
        self.min <= other.max && other.min <= self.max
    }
}

enum LayerItem<'a> {
    Operation(&'a Qp101Operation),
    ControlledPair {
        gate: &'a str,
        control_lane: usize,
        target_lane: usize,
    },
    SwapPair {
        lane_a: usize,
        lane_b: usize,
    },
    NoiseBox {
        gate: &'a str,
        params: &'a [f64],
        lane: usize,
        annotations: Vec<&'a Qp101Annotation>,
    },
    NoisePair {
        gate: &'a str,
        params: &'a [f64],
        lane_a: usize,
        lane_b: usize,
        annotations: Vec<&'a Qp101Annotation>,
    },
}

struct SourceLayerItem<'a> {
    lane: usize,
    label: String,
    source: String,
    annotations: &'a [Qp101Annotation],
    highlighted: bool,
    column_span: usize,
}

impl LayerItem<'_> {
    fn span(&self, num_qubits: usize) -> Result<LaneSpan, String> {
        match self {
            LayerItem::Operation(op) => operation_lane_span(op, num_qubits),
            LayerItem::ControlledPair {
                control_lane,
                target_lane,
                ..
            } => Ok(LaneSpan {
                min: (*control_lane).min(*target_lane),
                max: (*control_lane).max(*target_lane),
            }),
            LayerItem::SwapPair { lane_a, lane_b } => Ok(LaneSpan {
                min: (*lane_a).min(*lane_b),
                max: (*lane_a).max(*lane_b),
            }),
            LayerItem::NoiseBox { lane, .. } => Ok(LaneSpan {
                min: *lane,
                max: *lane,
            }),
            LayerItem::NoisePair { lane_a, lane_b, .. } => Ok(LaneSpan {
                min: (*lane_a).min(*lane_b),
                max: (*lane_a).max(*lane_b),
            }),
        }
    }
}

pub fn render_svg(doc: &Qp101Document) -> Result<String, String> {
    if doc.num_qubits == 0 {
        return Err("cannot render QP101 SVG with num_qubits = 0".to_string());
    }

    let visible_columns = count_visible_columns(&doc.operations, doc.num_qubits)?.max(1);
    let width = LEFT_MARGIN + RIGHT_MARGIN + (visible_columns as i32 + 1) * COLUMN_GAP;
    let top_reserve = repeat_label_top_reserve(&doc.operations);
    let height = svg_height(doc)? + top_reserve;
    let mut out = String::new();

    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n"
    ));
    out.push_str(
        "<g class=\"qp101-svg\" fill=\"none\" font-family=\"monospace\" font-size=\"14\">\n",
    );
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
    if top_reserve > 0 {
        out.push_str(&format!(
            "<g class=\"qp101-content\" transform=\"translate(0 {top_reserve})\">\n"
        ));
    }
    render_repeat_backgrounds(&mut out, &state.repeat_groups, doc.num_qubits);
    render_wires(&mut out, doc.num_qubits, width);
    out.push_str(&operation_out);
    if top_reserve > 0 {
        out.push_str("</g>\n");
    }
    render_repeat_labels(&mut out, &state.repeat_groups, doc.num_qubits, top_reserve);
    out.push_str("</g>\n</svg>\n");
    Ok(out)
}

fn repeat_label_top_reserve(ops: &[Qp101Operation]) -> i32 {
    if ops
        .iter()
        .any(|op| matches!(op, Qp101Operation::Repeat { .. }))
    {
        REPEAT_LABEL_TOP_RESERVE
    } else {
        0
    }
}

fn count_visible_columns(ops: &[Qp101Operation], num_qubits: usize) -> Result<usize, String> {
    let mut state = RenderState::default();
    count_visible_columns_with_state(ops, num_qubits, &mut state)
}

fn count_visible_columns_with_state(
    ops: &[Qp101Operation],
    num_qubits: usize,
    state: &mut RenderState,
) -> Result<usize, String> {
    let mut total = 0usize;
    let mut layer = Vec::new();
    let mut source_layer = Vec::new();
    for op in ops {
        match op {
            Qp101Operation::QubitCoords { .. } | Qp101Operation::ShiftCoords { .. } => {}
            Qp101Operation::Tick { .. } | Qp101Operation::Annotation { .. } => {
                count_and_clear_operation_layer(&mut total, &mut layer, num_qubits, state)?;
                count_and_clear_source_layer(&mut total, &mut source_layer, num_qubits, state)?;
                total = total.saturating_add(1);
            }
            Qp101Operation::Detector { .. } | Qp101Operation::ObservableInclude { .. } => {
                count_and_clear_operation_layer(&mut total, &mut layer, num_qubits, state)?;
                source_layer.push(op);
            }
            Qp101Operation::Gate { .. } | Qp101Operation::Noise { .. } => {
                count_and_clear_source_layer(&mut total, &mut source_layer, num_qubits, state)?;
                layer.push(op);
            }
            Qp101Operation::Repeat { count, body, .. } => {
                count_and_clear_operation_layer(&mut total, &mut layer, num_qubits, state)?;
                count_and_clear_source_layer(&mut total, &mut source_layer, num_qubits, state)?;
                for _ in 0..*count {
                    total = total
                        .saturating_add(count_visible_columns_with_state(body, num_qubits, state)?);
                }
            }
        }
    }
    count_and_clear_operation_layer(&mut total, &mut layer, num_qubits, state)?;
    count_and_clear_source_layer(&mut total, &mut source_layer, num_qubits, state)?;
    Ok(total)
}

fn count_and_clear_operation_layer(
    total: &mut usize,
    layer: &mut Vec<&Qp101Operation>,
    num_qubits: usize,
    state: &mut RenderState,
) -> Result<(), String> {
    *total = total.saturating_add(count_operation_layer_columns(layer, num_qubits)?);
    for op in layer.drain(..) {
        advance_measurement_state_for_count(op, num_qubits, state)?;
    }
    Ok(())
}

fn count_and_clear_source_layer(
    total: &mut usize,
    layer: &mut Vec<&Qp101Operation>,
    num_qubits: usize,
    state: &mut RenderState,
) -> Result<(), String> {
    *total = total.saturating_add(count_source_layer_columns(layer, num_qubits, state)?);
    layer.clear();
    Ok(())
}

fn count_source_layer_columns(
    layer: &[&Qp101Operation],
    num_qubits: usize,
    state: &mut RenderState,
) -> Result<usize, String> {
    let mut column_spans: Vec<Vec<LaneSpan>> = Vec::new();
    for op in layer {
        let item = source_layer_item(op, num_qubits, state)?;
        let span = LaneSpan {
            min: item.lane,
            max: item.lane,
        };
        let assigned_column =
            first_non_conflicting_column_with_width(&column_spans, span, item.column_span);
        reserve_column_span(&mut column_spans, assigned_column, item.column_span, span);
    }
    Ok(column_spans.len())
}

fn advance_measurement_state_for_count(
    op: &Qp101Operation,
    num_qubits: usize,
    state: &mut RenderState,
) -> Result<(), String> {
    match op {
        Qp101Operation::Gate {
            gate,
            targets,
            raw_targets,
            ..
        } => {
            let _ = measurement_targets(gate, targets, raw_targets.as_deref(), num_qubits, state)?;
        }
        Qp101Operation::Noise {
            gate, raw_targets, ..
        } => {
            let _ = measurement_targets(gate, &[], Some(raw_targets), num_qubits, state)?;
        }
        _ => {}
    }
    Ok(())
}

fn count_operation_layer_columns(
    layer: &[&Qp101Operation],
    num_qubits: usize,
) -> Result<usize, String> {
    let mut column_spans: Vec<Vec<LaneSpan>> = Vec::new();
    for op in layer {
        for item in operation_layer_items(op, num_qubits)? {
            let span = item.span(num_qubits)?;
            let assigned_column = first_non_conflicting_column(&column_spans, span);
            if assigned_column == column_spans.len() {
                column_spans.push(Vec::new());
            }
            column_spans[assigned_column].push(span);
        }
    }
    Ok(column_spans.len())
}

fn svg_height(doc: &Qp101Document) -> Result<i32, String> {
    let base_height = base_svg_height(doc.num_qubits);
    let Some(max_text_baseline) =
        max_rendered_below_gate_text_baseline(&doc.operations, doc.num_qubits)?
    else {
        return Ok(base_height);
    };

    Ok(base_height.max(max_text_baseline + BELOW_GATE_TEXT_BOTTOM_PAD))
}

fn base_svg_height(num_qubits: usize) -> i32 {
    TOP_MARGIN + BOTTOM_MARGIN + (num_qubits.saturating_sub(1) as i32) * LANE_GAP
}

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
                let measurement_targets =
                    measurement_targets(gate, targets, raw_targets.as_deref(), num_qubits, state)?;
                for target in &measurement_targets {
                    update_max_baseline(&mut max_baseline, below_gate_text_y(target.lane));
                }
                update_max_baseline_from_below_annotations(
                    &mut max_baseline,
                    &lanes,
                    annotations,
                    usize::from(!measurement_targets.is_empty()),
                );
            }
            Qp101Operation::Noise {
                gate,
                params,
                raw_targets,
                annotations,
                ..
            } => {
                let lanes = raw_target_lanes(raw_targets, num_qubits, gate)?;
                let measurement_targets =
                    measurement_targets(gate, &[], Some(raw_targets), num_qubits, state)?;
                for target in &measurement_targets {
                    update_max_baseline(&mut max_baseline, below_gate_text_y(target.lane));
                }
                let param_line_offset = usize::from(!measurement_targets.is_empty());
                if !params.is_empty() {
                    update_max_baseline(
                        &mut max_baseline,
                        below_gate_text_y_with_offset(
                            lanes.first().copied().unwrap_or(0),
                            param_line_offset,
                        ),
                    );
                }
                update_max_baseline_from_below_annotations(
                    &mut max_baseline,
                    &lanes,
                    annotations,
                    param_line_offset + usize::from(!params.is_empty()),
                );
            }
            Qp101Operation::Repeat {
                count,
                body,
                annotations,
            } => {
                update_max_baseline_from_annotations(
                    &mut max_baseline,
                    &[0usize],
                    annotations,
                    REPEAT_ANNOTATION_LINE_OFFSET,
                );
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
                update_max_baseline_from_below_annotations(
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
                update_max_baseline_from_below_annotations(
                    &mut max_baseline,
                    &[source.host_lane],
                    annotations,
                    1,
                );
            }
            Qp101Operation::Annotation { annotations, .. } => {
                update_max_baseline_from_below_annotations(
                    &mut max_baseline,
                    &[0usize],
                    annotations,
                    0,
                );
            }
        }
    }
    Ok(max_baseline)
}

fn update_max_baseline_from_annotations(
    max_baseline: &mut Option<i32>,
    lanes: &[usize],
    annotations: &[Qp101Annotation],
    line_offset: usize,
) {
    if let Some(baseline) = max_annotation_baseline(lanes, annotations, line_offset) {
        update_max_baseline(max_baseline, baseline);
    }
}

fn update_max_baseline_from_below_annotations(
    max_baseline: &mut Option<i32>,
    lanes: &[usize],
    annotations: &[Qp101Annotation],
    line_offset: usize,
) {
    let below_count = annotations
        .iter()
        .filter(|annotation| !is_sample_annotation(annotation))
        .count();
    if below_count == 0 {
        return;
    }
    let base_lane = lanes.first().copied().unwrap_or(0);
    let baseline =
        below_gate_text_y(base_lane) + (line_offset + below_count - 1) as i32 * ANNOTATION_LINE_GAP;
    update_max_baseline(max_baseline, baseline);
}

fn max_annotation_baseline(
    lanes: &[usize],
    annotations: &[Qp101Annotation],
    line_offset: usize,
) -> Option<i32> {
    if annotations.is_empty() {
        return None;
    }
    let base_lane = lanes.first().copied().unwrap_or(0);
    Some(
        below_gate_text_y(base_lane)
            + (line_offset + annotations.len() - 1) as i32 * ANNOTATION_LINE_GAP,
    )
}

fn update_max_baseline(max_baseline: &mut Option<i32>, baseline: i32) {
    *max_baseline = Some(max_baseline.map_or(baseline, |current| current.max(baseline)));
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

fn render_operations<'a>(
    out: &mut String,
    ops: &'a [Qp101Operation],
    num_qubits: usize,
    column: &mut usize,
    state: &mut RenderState,
) -> Result<(), String> {
    let mut layer = Vec::new();
    let mut source_layer = Vec::new();
    for op in ops {
        match op {
            Qp101Operation::QubitCoords { .. } | Qp101Operation::ShiftCoords { .. } => {}
            Qp101Operation::Tick { annotations } => {
                flush_operation_layer(out, &mut layer, num_qubits, column, state)?;
                flush_source_layer(out, &mut source_layer, num_qubits, column, state)?;
                render_tick(out, x_for_column(*column), num_qubits, annotations);
                *column += 1;
            }
            Qp101Operation::Gate { .. } | Qp101Operation::Noise { .. } => {
                flush_source_layer(out, &mut source_layer, num_qubits, column, state)?;
                layer.push(op);
            }
            Qp101Operation::Repeat {
                count,
                body,
                annotations,
            } => {
                flush_operation_layer(out, &mut layer, num_qubits, column, state)?;
                flush_source_layer(out, &mut source_layer, num_qubits, column, state)?;
                let start_column = *column;
                let mut iteration_starts = Vec::new();
                let depth = state.repeat_depth;
                state.repeat_depth += 1;
                for _ in 0..*count {
                    iteration_starts.push(*column);
                    render_operations(out, body, num_qubits, column, state)?;
                }
                state.repeat_depth = depth;
                if *column > start_column {
                    let span = RepeatGroupSpan {
                        count: *count,
                        start_column,
                        end_column: *column - 1,
                        iteration_starts,
                        depth,
                    };
                    render_repeat_annotations(out, &span, annotations);
                    state.repeat_groups.push(span);
                }
            }
            Qp101Operation::Detector { .. } | Qp101Operation::ObservableInclude { .. } => {
                flush_operation_layer(out, &mut layer, num_qubits, column, state)?;
                source_layer.push(op);
            }
            Qp101Operation::Annotation {
                kind,
                text,
                annotations,
            } => {
                flush_operation_layer(out, &mut layer, num_qubits, column, state)?;
                flush_source_layer(out, &mut source_layer, num_qubits, column, state)?;
                let x = x_for_column(*column);
                let label = format!("{kind}: {text}");
                render_top_note(out, x, &label);
                render_annotations(out, x, &[0], annotations);
                *column += 1;
            }
        }
    }
    flush_operation_layer(out, &mut layer, num_qubits, column, state)?;
    flush_source_layer(out, &mut source_layer, num_qubits, column, state)?;
    Ok(())
}

fn flush_operation_layer(
    out: &mut String,
    layer: &mut Vec<&Qp101Operation>,
    num_qubits: usize,
    column: &mut usize,
    state: &mut RenderState,
) -> Result<(), String> {
    if layer.is_empty() {
        return Ok(());
    }

    let mut column_spans: Vec<Vec<LaneSpan>> = Vec::new();
    for op in layer.drain(..) {
        for item in operation_layer_items(op, num_qubits)? {
            let span = item.span(num_qubits)?;
            let assigned_column = first_non_conflicting_column(&column_spans, span);
            if assigned_column == column_spans.len() {
                column_spans.push(Vec::new());
            }
            column_spans[assigned_column].push(span);
            render_layer_item(
                out,
                x_for_column(*column + assigned_column),
                num_qubits,
                &item,
                state,
            )?;
        }
    }
    *column += column_spans.len();
    Ok(())
}

fn flush_source_layer<'a>(
    out: &mut String,
    layer: &mut Vec<&'a Qp101Operation>,
    num_qubits: usize,
    column: &mut usize,
    state: &mut RenderState,
) -> Result<(), String> {
    if layer.is_empty() {
        return Ok(());
    }

    let mut column_spans: Vec<Vec<LaneSpan>> = Vec::new();
    for op in layer.drain(..) {
        let item = source_layer_item(op, num_qubits, state)?;
        let span = LaneSpan {
            min: item.lane,
            max: item.lane,
        };
        let assigned_column =
            first_non_conflicting_column_with_width(&column_spans, span, item.column_span);
        reserve_column_span(&mut column_spans, assigned_column, item.column_span, span);
        let x = x_for_column(*column + assigned_column);
        render_source_operation(
            out,
            x,
            item.lane,
            &item.label,
            &item.source,
            item.highlighted,
        );
        render_source_annotations_with_line_offset(out, x, &[item.lane], item.annotations, 1);
    }
    *column += column_spans.len();
    Ok(())
}

fn source_layer_item<'a>(
    op: &'a Qp101Operation,
    num_qubits: usize,
    state: &mut RenderState,
) -> Result<SourceLayerItem<'a>, String> {
    match op {
        Qp101Operation::Detector {
            sources,
            annotations,
            ..
        } => {
            let detector_index = state.next_detector_index;
            state.next_detector_index += 1;
            let source = source_label(sources, &state.measurements, num_qubits);
            let source_text = format!("D{detector_index} = {}", source.text);
            Ok(SourceLayerItem {
                lane: source.host_lane,
                label: "DETECTOR".to_string(),
                column_span: source_operation_column_span_for_item("DETECTOR", &source_text),
                source: source_text,
                annotations,
                highlighted: source_block_highlighted(annotations),
            })
        }
        Qp101Operation::ObservableInclude {
            index,
            sources,
            annotations,
            ..
        } => {
            let source = source_label(sources, &state.measurements, num_qubits);
            let label = format!("OBS_INCLUDE({index})");
            let source_text = format!("L{index} *= {}", source.text);
            Ok(SourceLayerItem {
                lane: source.host_lane,
                column_span: source_operation_column_span_for_item(&label, &source_text),
                source: source_text,
                annotations,
                highlighted: source_block_highlighted(annotations),
                label,
            })
        }
        other => Err(format!(
            "internal error: non-source operation in source layer: {other:?}"
        )),
    }
}

fn first_non_conflicting_column(column_spans: &[Vec<LaneSpan>], span: LaneSpan) -> usize {
    column_spans
        .iter()
        .position(|spans| spans.iter().all(|existing| !existing.conflicts(span)))
        .unwrap_or(column_spans.len())
}

fn first_non_conflicting_column_with_width(
    column_spans: &[Vec<LaneSpan>],
    span: LaneSpan,
    column_span: usize,
) -> usize {
    let column_span = column_span.max(1);
    for start in 0..=column_spans.len() {
        let fits = (start..start + column_span).all(|column| {
            column >= column_spans.len()
                || column_spans[column]
                    .iter()
                    .all(|existing| !existing.conflicts(span))
        });
        if fits {
            return start;
        }
    }
    column_spans.len()
}

fn reserve_column_span(
    column_spans: &mut Vec<Vec<LaneSpan>>,
    start: usize,
    column_span: usize,
    span: LaneSpan,
) {
    let end = start + column_span.max(1);
    while column_spans.len() < end {
        column_spans.push(Vec::new());
    }
    for spans in &mut column_spans[start..end] {
        spans.push(span);
    }
}

fn operation_lane_span(op: &Qp101Operation, num_qubits: usize) -> Result<LaneSpan, String> {
    let lanes = match op {
        Qp101Operation::Gate {
            gate,
            targets,
            controls,
            raw_targets,
            ..
        } => {
            if let Some(raw_targets) = raw_targets {
                raw_target_lanes(raw_targets, num_qubits, gate)?
            } else {
                gate_lanes(targets, controls, num_qubits, gate)?
            }
        }
        Qp101Operation::Noise {
            gate, raw_targets, ..
        } => raw_target_lanes(raw_targets, num_qubits, gate)?,
        _ => Vec::new(),
    };
    Ok(LaneSpan::from_lanes(&lanes, num_qubits))
}

fn operation_layer_items<'a>(
    op: &'a Qp101Operation,
    num_qubits: usize,
) -> Result<Vec<LayerItem<'a>>, String> {
    match op {
        Qp101Operation::Gate {
            gate,
            targets,
            controls,
            ..
        } if gate == "CX" || gate == "CZ" => {
            if let Some(pairs) = controlled_pairs(targets, controls, num_qubits, gate)? {
                return Ok(pairs
                    .into_iter()
                    .map(|(control_lane, target_lane)| LayerItem::ControlledPair {
                        gate,
                        control_lane,
                        target_lane,
                    })
                    .collect());
            }
        }
        Qp101Operation::Gate { gate, targets, .. } if gate == "SWAP" => {
            if let Some(pairs) = target_pairs(targets, num_qubits, gate)? {
                return Ok(pairs
                    .into_iter()
                    .map(|(lane_a, lane_b)| LayerItem::SwapPair { lane_a, lane_b })
                    .collect());
            }
        }
        Qp101Operation::Noise {
            gate,
            params,
            raw_targets,
            annotations,
        } => {
            let lanes = raw_target_lanes(raw_targets, num_qubits, gate)?;
            match noise_policy(gate) {
                NoisePolicy::Single if !lanes.is_empty() => {
                    return Ok(lanes
                        .into_iter()
                        .enumerate()
                        .map(|(slot, lane)| LayerItem::NoiseBox {
                            gate,
                            params,
                            lane,
                            annotations: annotations_for_target_slots(
                                annotations,
                                &[slot],
                                slot == 0,
                            ),
                        })
                        .collect());
                }
                NoisePolicy::Pair if !lanes.is_empty() && lanes.len() % 2 == 0 => {
                    return Ok(lanes
                        .chunks_exact(2)
                        .enumerate()
                        .map(|(pair_index, pair)| {
                            let first_slot = pair_index * 2;
                            LayerItem::NoisePair {
                                gate,
                                params,
                                lane_a: pair[0],
                                lane_b: pair[1],
                                annotations: annotations_for_target_slots(
                                    annotations,
                                    &[first_slot, first_slot + 1],
                                    pair_index == 0,
                                ),
                            }
                        })
                        .collect());
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(vec![LayerItem::Operation(op)])
}

fn annotations_for_target_slots<'a>(
    annotations: &'a [Qp101Annotation],
    target_slots: &[usize],
    include_operation_annotations: bool,
) -> Vec<&'a Qp101Annotation> {
    annotations
        .iter()
        .filter(|annotation| {
            if annotation.target_slots.is_empty() {
                include_operation_annotations
            } else {
                annotation
                    .target_slots
                    .iter()
                    .any(|slot| target_slots.contains(slot))
            }
        })
        .collect()
}

fn render_layer_item(
    out: &mut String,
    x: i32,
    num_qubits: usize,
    item: &LayerItem<'_>,
    state: &mut RenderState,
) -> Result<(), String> {
    match item {
        LayerItem::ControlledPair {
            gate,
            control_lane,
            target_lane,
        } => {
            render_controlled_pair(out, x, *control_lane, *target_lane, gate);
            Ok(())
        }
        LayerItem::SwapPair { lane_a, lane_b } => {
            render_swap_pair(out, x, *lane_a, *lane_b);
            Ok(())
        }
        LayerItem::NoiseBox {
            gate,
            params,
            lane,
            annotations,
        } => {
            render_known_noise_box(out, x, gate, params, *lane, annotations);
            Ok(())
        }
        LayerItem::NoisePair {
            gate,
            params,
            lane_a,
            lane_b,
            annotations,
        } => {
            render_known_noise_pair(out, x, gate, params, *lane_a, *lane_b, annotations);
            Ok(())
        }
        LayerItem::Operation(op) => match op {
            Qp101Operation::Gate {
                gate,
                targets,
                controls,
                raw_targets,
                display,
                annotations,
                ..
            } => render_gate(
                out,
                x,
                num_qubits,
                gate,
                targets,
                controls,
                raw_targets.as_deref(),
                display.as_ref(),
                annotations,
                state,
            ),
            Qp101Operation::Noise {
                gate,
                params,
                raw_targets,
                annotations,
                ..
            } => render_noise(
                out,
                x,
                num_qubits,
                gate,
                params,
                raw_targets,
                annotations,
                state,
            ),
            _ => Ok(()),
        },
    }
}

fn render_known_noise_box(
    out: &mut String,
    x: i32,
    gate: &str,
    params: &[f64],
    lane: usize,
    annotations: &[&Qp101Annotation],
) {
    render_noise_box(out, x, lane_y(lane), noise_label(gate));
    let mut below_line_offset = 0usize;
    if let Some(note) = noise_param_note(params) {
        render_param_note(out, x, &[lane], &note, below_line_offset);
        below_line_offset += 1;
    }
    render_annotation_refs_with_line_offset(out, x, &[lane], annotations, below_line_offset);
}

fn render_known_noise_pair(
    out: &mut String,
    x: i32,
    gate: &str,
    params: &[f64],
    lane_a: usize,
    lane_b: usize,
    annotations: &[&Qp101Annotation],
) {
    render_noise_pair(out, x, lane_a, lane_b, noise_label(gate));
    let upper_lane = lane_a.min(lane_b);
    let lower_lane = lane_a.max(lane_b);
    let mut below_line_offset = 0usize;
    if let Some(note) = noise_param_note(params) {
        render_param_note(out, x, &[lower_lane], &note, below_line_offset);
        below_line_offset += 1;
    }
    render_sample_annotation_refs(out, x, &[upper_lane], annotations);
    render_below_annotation_refs_with_line_offset(
        out,
        x,
        &[lower_lane],
        annotations,
        below_line_offset,
    );
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
    let mut resolved_lanes = Vec::new();
    let mut fallback_lanes = Vec::new();
    let mut text = String::new();
    let mut needs_separator = false;

    for source in sources {
        let resolved = resolve_source_ref(source, measurements);
        if let Some(lane) = resolved.resolved_lane {
            resolved_lanes.push(lane);
        }
        if let Some(lane) = target_ref_lane(source, num_qubits) {
            fallback_lanes.push(lane);
        }

        match source {
            Qp101TargetRef::Combiner => {
                text.push('*');
                needs_separator = false;
            }
            _ => {
                if needs_separator {
                    text.push('*');
                }
                text.push_str(&resolved.text);
                needs_separator = true;
            }
        }
    }

    let host_lane = resolved_lanes
        .into_iter()
        .min()
        .or_else(|| fallback_lanes.into_iter().min())
        .unwrap_or(0);
    let text = if sources.is_empty() {
        "-".to_string()
    } else {
        text
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
        } => format!(
            "{}{}{qubit}",
            inverted_prefix(*inverted),
            pauli_basis_text(basis)
        ),
        Qp101TargetRef::Combiner => "*".to_string(),
        Qp101TargetRef::Sweep { index } => format!("sweep[{index}]"),
    }
}

fn inverted_prefix(inverted: Option<bool>) -> &'static str {
    if inverted.unwrap_or(false) {
        "!"
    } else {
        ""
    }
}

fn pauli_basis_text(basis: &Qp101PauliBasis) -> &'static str {
    match basis {
        Qp101PauliBasis::X => "X",
        Qp101PauliBasis::Y => "Y",
        Qp101PauliBasis::Z => "Z",
    }
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

fn measurement_targets(
    gate: &str,
    targets: &[u32],
    raw_targets: Option<&[Qp101TargetRef]>,
    num_qubits: usize,
    state: &mut RenderState,
) -> Result<Vec<MeasurementTarget>, String> {
    let mut measurement_targets = Vec::new();
    match gate {
        "M" | "MX" | "MY" | "MZ" | "MR" | "MRX" | "MRY" | "MRZ" => {
            record_target_measurements(
                &mut measurement_targets,
                state,
                targets,
                1,
                num_qubits,
                gate,
            )?;
        }
        "ML" | "MXL" | "MYL" | "MZL" | "MRL" | "MRXL" | "MRYL" | "MRZL" => {
            record_target_measurements(
                &mut measurement_targets,
                state,
                targets,
                2,
                num_qubits,
                gate,
            )?;
        }
        "MPP" => record_mpp_measurements(
            &mut measurement_targets,
            state,
            raw_targets,
            targets,
            num_qubits,
            gate,
        )?,
        "MXX" | "MYY" | "MZZ" => {
            record_pair_measurements(&mut measurement_targets, state, targets, num_qubits, gate)?
        }
        "MPAD" | "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1" => {
            record_raw_target_measurements(
                &mut measurement_targets,
                state,
                raw_targets,
                targets,
                num_qubits,
                gate,
            )?;
        }
        _ => {}
    }
    Ok(measurement_targets)
}

fn record_target_measurements(
    measurement_targets: &mut Vec<MeasurementTarget>,
    state: &mut RenderState,
    targets: &[u32],
    output_count: usize,
    num_qubits: usize,
    gate: &str,
) -> Result<(), String> {
    for &target in targets {
        let lane = validate_lane(target, num_qubits, gate)?;
        record_measurement_target(measurement_targets, state, lane, output_count);
    }
    Ok(())
}

fn record_pair_measurements(
    measurement_targets: &mut Vec<MeasurementTarget>,
    state: &mut RenderState,
    targets: &[u32],
    num_qubits: usize,
    gate: &str,
) -> Result<(), String> {
    let mut chunks = targets.chunks_exact(2);
    for chunk in &mut chunks {
        let lane_a = validate_lane(chunk[0], num_qubits, gate)?;
        let lane_b = validate_lane(chunk[1], num_qubits, gate)?;
        record_measurement_target(measurement_targets, state, lane_a.min(lane_b), 1);
    }
    for &target in chunks.remainder() {
        validate_lane(target, num_qubits, gate)?;
    }
    Ok(())
}

fn record_mpp_measurements(
    measurement_targets: &mut Vec<MeasurementTarget>,
    state: &mut RenderState,
    raw_targets: Option<&[Qp101TargetRef]>,
    targets: &[u32],
    num_qubits: usize,
    gate: &str,
) -> Result<(), String> {
    let Some(raw_targets) = raw_targets else {
        return record_target_measurements(
            measurement_targets,
            state,
            targets,
            1,
            num_qubits,
            gate,
        );
    };

    let mut group_lanes = Vec::new();
    let mut group_has_target = false;
    let mut previous_was_combiner = false;
    for target in raw_targets {
        if matches!(target, Qp101TargetRef::Combiner) {
            previous_was_combiner = true;
            continue;
        }

        if group_has_target && !previous_was_combiner {
            record_mpp_group(measurement_targets, state, &group_lanes, group_has_target);
            group_lanes.clear();
        }
        group_has_target = true;
        previous_was_combiner = false;
        if let Some(lane) = validated_target_ref_lane(target, num_qubits, gate)? {
            group_lanes.push(lane);
        }
    }
    record_mpp_group(measurement_targets, state, &group_lanes, group_has_target);
    Ok(())
}

fn record_mpp_group(
    measurement_targets: &mut Vec<MeasurementTarget>,
    state: &mut RenderState,
    group_lanes: &[usize],
    group_has_target: bool,
) {
    if !group_has_target {
        return;
    }
    let lane = group_lanes.iter().copied().min().unwrap_or(0);
    record_measurement_target(measurement_targets, state, lane, 1);
}

fn record_raw_target_measurements(
    measurement_targets: &mut Vec<MeasurementTarget>,
    state: &mut RenderState,
    raw_targets: Option<&[Qp101TargetRef]>,
    targets: &[u32],
    num_qubits: usize,
    gate: &str,
) -> Result<(), String> {
    if let Some(raw_targets) = raw_targets {
        for target in raw_targets {
            if matches!(target, Qp101TargetRef::Combiner) {
                continue;
            }
            let lane = validated_target_ref_lane(target, num_qubits, gate)?.unwrap_or(0);
            record_measurement_target(measurement_targets, state, lane, 1);
        }
        return Ok(());
    }

    record_target_measurements(measurement_targets, state, targets, 1, num_qubits, gate)
}

fn validated_target_ref_lane(
    source: &Qp101TargetRef,
    num_qubits: usize,
    gate: &str,
) -> Result<Option<usize>, String> {
    match source {
        Qp101TargetRef::Qubit { index, .. } => Ok(Some(validate_lane(*index, num_qubits, gate)?)),
        Qp101TargetRef::Pauli { qubit, .. } => Ok(Some(validate_lane(*qubit, num_qubits, gate)?)),
        Qp101TargetRef::Rec { .. } | Qp101TargetRef::Combiner | Qp101TargetRef::Sweep { .. } => {
            Ok(None)
        }
    }
}

fn record_measurement_target(
    measurement_targets: &mut Vec<MeasurementTarget>,
    state: &mut RenderState,
    lane: usize,
    output_count: usize,
) {
    if output_count == 0 {
        return;
    }

    let first_index = state.next_measurement_index + 1;
    state.next_measurement_index += output_count;
    for output_offset in 0..output_count {
        state.measurements.push(MeasurementRecord {
            index: first_index + output_offset,
            lane,
        });
    }
    measurement_targets.push(MeasurementTarget {
        lane,
        first_index,
        output_count,
    });
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
    state: &mut RenderState,
) -> Result<(), String> {
    let label = gate_label(gate, display);
    let lanes = if let Some(raw_targets) = raw_targets {
        raw_target_lanes(raw_targets, num_qubits, gate)?
    } else {
        gate_lanes(targets, controls, num_qubits, gate)?
    };
    let measurement_targets = measurement_targets(gate, targets, raw_targets, num_qubits, state)?;

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
            if controls.is_empty() && is_simple_single_qubit_gate(gate) && !lanes.is_empty() {
                render_single_qubit_boxes(out, x, &label, &lanes);
            } else {
                render_generic_box(out, x, num_qubits, &label, &lanes, "#ffffff")?;
            }
        }
    }

    render_measurement_anchors(out, x, &measurement_targets);
    render_annotations_with_line_offset(
        out,
        x,
        &lanes,
        annotations,
        measurement_annotation_line_offset(&measurement_targets),
    );
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

fn is_simple_single_qubit_gate(gate: &str) -> bool {
    matches!(gate, "H" | "X" | "Y" | "Z" | "S" | "T" | "R" | "RX")
}

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
    Some(values)
}

fn render_noise(
    out: &mut String,
    x: i32,
    num_qubits: usize,
    gate: &str,
    params: &[f64],
    raw_targets: &[Qp101TargetRef],
    annotations: &[Qp101Annotation],
    state: &mut RenderState,
) -> Result<(), String> {
    let lanes = raw_target_lanes(raw_targets, num_qubits, gate)?;
    let note = noise_param_note(params);
    let measurement_targets = measurement_targets(gate, &[], Some(raw_targets), num_qubits, state)?;

    match noise_policy(gate) {
        NoisePolicy::Single if !lanes.is_empty() => {
            for &lane in &lanes {
                render_noise_box(out, x, lane_y(lane), noise_label(gate));
                if let Some(note) = note.as_deref() {
                    render_param_note(out, x, &[lane], note, 0);
                }
            }
        }
        NoisePolicy::Pair if !lanes.is_empty() && lanes.len() % 2 == 0 => {
            for pair in lanes.chunks_exact(2) {
                render_noise_pair(out, x, pair[0], pair[1], noise_label(gate));
                if let Some(note) = note.as_deref() {
                    render_param_note(out, x, pair, note, 0);
                }
            }
        }
        _ => {
            if let Some(note) = note.as_deref() {
                render_param_note(
                    out,
                    x,
                    &lanes,
                    note,
                    usize::from(!measurement_targets.is_empty()),
                );
            }
            render_generic_box(out, x, num_qubits, gate, &lanes, "#fff7ed")?;
        }
    }

    render_measurement_anchors(out, x, &measurement_targets);
    let annotation_line_offset =
        usize::from(!measurement_targets.is_empty()) + usize::from(note.is_some());
    render_annotations_with_line_offset(out, x, &lanes, annotations, annotation_line_offset);
    Ok(())
}

fn render_param_note(out: &mut String, x: i32, lanes: &[usize], note: &str, line_offset: usize) {
    let y = lanes
        .iter()
        .min()
        .map(|lane| below_gate_text_y_with_offset(*lane, line_offset))
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

fn render_single_qubit_boxes(out: &mut String, x: i32, label: &str, lanes: &[usize]) {
    for &lane in lanes {
        render_gate_box(out, x, lane_y(lane), label, "#ffffff");
    }
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
    render_gate_box_with_width(out, x, y, GATE_WIDTH, label, fill);
}

fn render_gate_box_with_width(
    out: &mut String,
    x: i32,
    y: i32,
    width: i32,
    label: &str,
    fill: &str,
) {
    out.push_str(&format!(
        "<rect class=\"gate-box\" x=\"{}\" y=\"{}\" width=\"{width}\" height=\"{GATE_HEIGHT}\" rx=\"4\" ry=\"4\" stroke=\"#111827\" fill=\"{fill}\" />\n",
        x - width / 2,
        y - GATE_HEIGHT / 2
    ));
    out.push_str(&format!(
        "<text x=\"{x}\" y=\"{y}\" fill=\"#111827\" text-anchor=\"middle\" dominant-baseline=\"middle\">{}</text>\n",
        escape_xml(label)
    ));
}

fn render_source_operation(
    out: &mut String,
    x: i32,
    lane: usize,
    label: &str,
    source: &str,
    highlighted: bool,
) {
    render_source_gate_box(
        out,
        x,
        lane_y(lane),
        source_gate_width(label),
        label,
        highlighted,
    );
    out.push_str(&format!(
        "<text class=\"source-label\" x=\"{x}\" y=\"{}\" fill=\"#475467\" text-anchor=\"middle\" font-size=\"11\">{}</text>\n",
        below_gate_text_y(lane),
        escape_xml(source)
    ));
}

fn render_source_gate_box(
    out: &mut String,
    x: i32,
    y: i32,
    width: i32,
    label: &str,
    highlighted: bool,
) {
    let fill = if highlighted { "#dbeafe" } else { "#f8fafc" };
    let stroke = if highlighted { "#2563eb" } else { "#111827" };
    let text_fill = if highlighted { "#1d4ed8" } else { "#111827" };
    out.push_str(&format!(
        "<rect class=\"gate-box\" x=\"{}\" y=\"{}\" width=\"{width}\" height=\"{GATE_HEIGHT}\" rx=\"4\" ry=\"4\" stroke=\"{stroke}\" fill=\"{fill}\" />\n",
        x - width / 2,
        y - GATE_HEIGHT / 2
    ));
    out.push_str(&format!(
        "<text x=\"{x}\" y=\"{y}\" fill=\"{text_fill}\" text-anchor=\"middle\" dominant-baseline=\"middle\" font-size=\"11\">{}</text>\n",
        escape_xml(label)
    ));
}

fn source_operation_column_span_for_item(label: &str, source: &str) -> usize {
    let width = source_gate_width(label).max(source_text_width(source));
    source_operation_column_span_for_width(width)
}

fn source_operation_column_span_for_width(width: i32) -> usize {
    usize::try_from((width + COLUMN_GAP - 1) / COLUMN_GAP)
        .unwrap_or(1)
        .max(SOURCE_OPERATION_MIN_COLUMN_SPAN)
}

fn source_gate_width(label: &str) -> i32 {
    let text_width = label.chars().count() as i32 * SOURCE_GATE_CHAR_WIDTH + SOURCE_GATE_TEXT_PAD;
    SOURCE_GATE_MIN_WIDTH.max(text_width)
}

fn source_text_width(source: &str) -> i32 {
    source.chars().count() as i32 * SOURCE_GATE_CHAR_WIDTH + SOURCE_GATE_TEXT_PAD
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

fn render_repeat_backgrounds(out: &mut String, groups: &[RepeatGroupSpan], num_qubits: usize) {
    for group in groups.iter().rev() {
        render_repeat_group_background(out, group, num_qubits);
        render_repeat_iteration_boundary_lines(out, group, num_qubits);
    }
}

fn render_repeat_labels(
    out: &mut String,
    groups: &[RepeatGroupSpan],
    num_qubits: usize,
    content_y_offset: i32,
) {
    for group in groups.iter().rev() {
        render_repeat_group_label(out, group);
        render_repeat_iteration_labels(out, group, num_qubits, content_y_offset);
    }
}

fn render_repeat_group_background(out: &mut String, group: &RepeatGroupSpan, num_qubits: usize) {
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
}

fn render_repeat_group_label(out: &mut String, group: &RepeatGroupSpan) {
    let x_start = x_for_column(group.start_column);
    let x_end = x_for_column(group.end_column);
    let left = x_start - COLUMN_GAP / 2 + REPEAT_GROUP_X_PAD;
    let right = x_end + COLUMN_GAP / 2 - REPEAT_GROUP_X_PAD;
    out.push_str(&format!(
        "<text class=\"repeat-group-label\" x=\"{}\" y=\"{}\" fill=\"#475467\" text-anchor=\"start\" font-size=\"12\">repeat x{}</text>\n",
        repeat_group_label_x(left, right, group.depth),
        repeat_group_label_y(),
        group.count
    ));
}

fn repeat_group_label_x(left: i32, right: i32, depth: usize) -> i32 {
    let min_x = left + REPEAT_GROUP_LABEL_X_PAD;
    let max_x = (right - REPEAT_GROUP_LABEL_X_PAD).max(min_x);
    let desired_x = min_x + depth as i32 * REPEAT_GROUP_LABEL_DEPTH_STAGGER;
    desired_x.min(max_x)
}

fn render_repeat_annotations(
    out: &mut String,
    group: &RepeatGroupSpan,
    annotations: &[Qp101Annotation],
) {
    let x_start = x_for_column(group.start_column);
    let x_end = x_for_column(group.end_column);
    let left = x_start - COLUMN_GAP / 2 + REPEAT_GROUP_X_PAD;
    let right = x_end + COLUMN_GAP / 2 - REPEAT_GROUP_X_PAD;
    render_annotations_with_line_offset(
        out,
        repeat_group_label_x(left, right, group.depth),
        &[0],
        annotations,
        REPEAT_ANNOTATION_LINE_OFFSET,
    );
}

fn render_repeat_iteration_boundary_lines(
    out: &mut String,
    group: &RepeatGroupSpan,
    num_qubits: usize,
) {
    let top = lane_y(0) - GATE_HEIGHT / 2 - REPEAT_GROUP_TOP_PAD;
    let bottom = lane_y(num_qubits.saturating_sub(1)) + GATE_HEIGHT / 2 + REPEAT_GROUP_BOTTOM_PAD;
    for &start_column in group.iteration_starts.iter().skip(1) {
        let x = x_for_column(start_column) - COLUMN_GAP / 2;
        out.push_str(&format!(
            "<line class=\"repeat-iteration-boundary\" x1=\"{x}\" y1=\"{top}\" x2=\"{x}\" y2=\"{bottom}\" stroke=\"#98a2b3\" stroke-width=\"1\" stroke-dasharray=\"4 4\" />\n"
        ));
    }
}

fn render_repeat_iteration_labels(
    out: &mut String,
    group: &RepeatGroupSpan,
    num_qubits: usize,
    content_y_offset: i32,
) {
    for (iteration_offset, &start_column) in group.iteration_starts.iter().enumerate().skip(1) {
        let x = x_for_column(start_column) - COLUMN_GAP / 2;
        out.push_str(&format!(
            "<text class=\"repeat-iteration-label\" x=\"{x}\" y=\"{}\" fill=\"#475467\" text-anchor=\"middle\" font-size=\"11\">iter {}</text>\n",
            repeat_iteration_label_y(num_qubits) + content_y_offset,
            iteration_offset + 1
        ));
    }
}

fn repeat_group_label_y() -> i32 {
    lane_y(0) - GATE_HEIGHT / 2 - REPEAT_GROUP_LABEL_GATE_GAP
}

fn repeat_iteration_label_y(num_qubits: usize) -> i32 {
    lane_y(num_qubits.saturating_sub(1)) + GATE_HEIGHT / 2 + REPEAT_GROUP_BOTTOM_PAD - 4
}

fn render_annotations(out: &mut String, x: i32, lanes: &[usize], annotations: &[Qp101Annotation]) {
    render_annotations_with_line_offset(out, x, lanes, annotations, 0);
}

fn render_annotations_with_line_offset(
    out: &mut String,
    x: i32,
    lanes: &[usize],
    annotations: &[Qp101Annotation],
    line_offset: usize,
) {
    render_sample_annotations(out, x, lanes, annotations);
    render_below_annotations_with_line_offset(out, x, lanes, annotations, line_offset);
}

fn render_source_annotations_with_line_offset(
    out: &mut String,
    x: i32,
    lanes: &[usize],
    annotations: &[Qp101Annotation],
    line_offset: usize,
) {
    render_source_sample_annotations(out, x, lanes, annotations);
    render_below_annotations_with_line_offset(out, x, lanes, annotations, line_offset);
}

fn render_below_annotations_with_line_offset(
    out: &mut String,
    x: i32,
    lanes: &[usize],
    annotations: &[Qp101Annotation],
    line_offset: usize,
) {
    let base_lane = lanes.first().copied().unwrap_or(0);
    let base_y = below_gate_text_y_with_offset(base_lane, line_offset);
    let mut below_index = 0usize;
    for annotation in annotations
        .iter()
        .filter(|annotation| !is_sample_annotation(annotation))
    {
        render_annotation_text(
            out,
            x,
            base_y + below_index as i32 * ANNOTATION_LINE_GAP,
            annotation,
            None,
        );
        below_index += 1;
    }
}

fn render_annotation_refs_with_line_offset(
    out: &mut String,
    x: i32,
    lanes: &[usize],
    annotations: &[&Qp101Annotation],
    line_offset: usize,
) {
    render_sample_annotation_refs(out, x, lanes, annotations);
    render_below_annotation_refs_with_line_offset(out, x, lanes, annotations, line_offset);
}

fn render_below_annotation_refs_with_line_offset(
    out: &mut String,
    x: i32,
    lanes: &[usize],
    annotations: &[&Qp101Annotation],
    line_offset: usize,
) {
    let base_lane = lanes.first().copied().unwrap_or(0);
    let base_y = below_gate_text_y_with_offset(base_lane, line_offset);
    let mut below_index = 0usize;
    for annotation in annotations
        .iter()
        .copied()
        .filter(|annotation| !is_sample_annotation(annotation))
    {
        render_annotation_text(
            out,
            x,
            base_y + below_index as i32 * ANNOTATION_LINE_GAP,
            annotation,
            None,
        );
        below_index += 1;
    }
}

fn render_sample_annotations(
    out: &mut String,
    x: i32,
    lanes: &[usize],
    annotations: &[Qp101Annotation],
) {
    let sample_annotations = annotations
        .iter()
        .filter(|annotation| is_sample_annotation(annotation))
        .collect::<Vec<_>>();
    render_sample_annotation_slice(out, x, lanes, &sample_annotations);
}

fn render_source_sample_annotations(
    out: &mut String,
    x: i32,
    lanes: &[usize],
    annotations: &[Qp101Annotation],
) {
    let sample_annotations = annotations
        .iter()
        .filter(|annotation| {
            is_sample_annotation(annotation) && !is_source_block_highlight_annotation(annotation)
        })
        .collect::<Vec<_>>();
    render_sample_annotation_slice(out, x, lanes, &sample_annotations);
}

fn render_sample_annotation_refs(
    out: &mut String,
    x: i32,
    lanes: &[usize],
    annotations: &[&Qp101Annotation],
) {
    let sample_annotations = annotations
        .iter()
        .copied()
        .filter(|annotation| is_sample_annotation(annotation))
        .collect::<Vec<_>>();
    render_sample_annotation_slice(out, x, lanes, &sample_annotations);
}

fn render_sample_annotation_slice(
    out: &mut String,
    x: i32,
    lanes: &[usize],
    annotations: &[&Qp101Annotation],
) {
    if annotations.is_empty() {
        return;
    }
    let base_lane = lanes.first().copied().unwrap_or(0);
    let mut used_rows = Vec::new();
    for annotation in annotations {
        let mut row = 0usize;
        while used_rows.contains(&row) {
            row += 1;
        }
        used_rows.push(row);
        render_annotation_text(
            out,
            x,
            above_gate_text_y_for_row(base_lane, row),
            annotation,
            Some(&sample_annotation_content(annotation)),
        );
    }
}

fn render_annotation_text(
    out: &mut String,
    x: i32,
    y: i32,
    annotation: &Qp101Annotation,
    content_override: Option<&str>,
) {
    let mut parts = Vec::new();
    parts.push(annotation.kind.clone());
    if let Some(label) = annotation.label.as_deref() {
        parts.push(label.to_string());
    }
    if let Some(text) = annotation.text.as_deref() {
        parts.push(text.to_string());
    }
    let fallback_content;
    let raw_content = if let Some(content) = content_override {
        content
    } else {
        fallback_content = parts.join(": ");
        &fallback_content
    };
    let content = escape_xml(raw_content);
    let attrs = annotation_svg_attrs(annotation);
    out.push_str(&format!(
        "<text {attrs} x=\"{x}\" y=\"{y}\" text-anchor=\"middle\" font-size=\"11\">{content}</text>\n",
    ));
}

fn sample_annotation_content(annotation: &Qp101Annotation) -> String {
    match (annotation.label.as_deref(), annotation.text.as_deref()) {
        (Some(label), Some(text)) if !text.is_empty() => format!("{label}: {text}"),
        (Some(label), _) => label.to_string(),
        (None, Some(text)) => text.to_string(),
        (None, None) => annotation.kind.clone(),
    }
}

fn is_sample_annotation(annotation: &Qp101Annotation) -> bool {
    annotation
        .tags
        .iter()
        .any(|tag| tag == "sample-trace" || tag == "query-result")
}

fn source_block_highlighted(annotations: &[Qp101Annotation]) -> bool {
    annotations.iter().any(is_source_block_highlight_annotation)
}

fn is_source_block_highlight_annotation(annotation: &Qp101Annotation) -> bool {
    is_sample_annotation(annotation) && is_info_annotation(annotation)
}

fn is_info_annotation(annotation: &Qp101Annotation) -> bool {
    annotation.style.as_ref().is_some_and(|style| {
        matches!(style.preset.as_deref(), Some("info" | "blue"))
            || matches!(style.color.as_deref(), Some("info" | "blue"))
    })
}

fn annotation_svg_attrs(annotation: &Qp101Annotation) -> String {
    let mut classes = vec!["annotation".to_string()];
    let mut attrs = Vec::new();
    if let Some(style) = annotation.style.as_ref() {
        if let Some(preset) = style.preset.as_deref() {
            classes.push(format!("annotation-preset-{}", css_token(preset)));
            attrs.push(format!("data-style-preset=\"{}\"", escape_xml(preset)));
        }
        if let Some(highlight) = style.highlight {
            attrs.push(format!("data-style-highlight=\"{highlight}\""));
        }
    }
    if !annotation.tags.is_empty() {
        attrs.push(format!(
            "data-annotation-tags=\"{}\"",
            escape_xml(&annotation.tags.join(" "))
        ));
    }
    attrs.insert(0, format!("class=\"{}\"", classes.join(" ")));
    attrs.push(format!(
        "fill=\"{}\"",
        escape_xml(&annotation_fill(annotation))
    ));
    attrs.join(" ")
}

fn annotation_fill(annotation: &Qp101Annotation) -> String {
    annotation
        .style
        .as_ref()
        .and_then(|style| style.color.as_deref())
        .map(annotation_color)
        .or_else(|| {
            annotation
                .style
                .as_ref()
                .and_then(|style| style.preset.as_deref())
                .map(annotation_color)
        })
        .unwrap_or("#7a5af8")
        .to_string()
}

fn annotation_color(value: &str) -> &str {
    match value {
        "danger" | "red" => "#dc2626",
        "info" | "blue" => "#2563eb",
        "warning" | "yellow" => "#ca8a04",
        "success" | "green" => "#16a34a",
        other => other,
    }
}

fn css_token(value: &str) -> String {
    let mut token = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            token.push(ch);
        } else {
            token.push('-');
        }
    }
    if token.is_empty() {
        "custom".to_string()
    } else {
        token
    }
}

fn measurement_annotation_line_offset(targets: &[MeasurementTarget]) -> usize {
    usize::from(!targets.is_empty())
}

fn render_measurement_anchors(out: &mut String, x: i32, targets: &[MeasurementTarget]) {
    for target in targets {
        out.push_str(&format!(
            "<text class=\"measurement-anchor\" x=\"{x}\" y=\"{}\" fill=\"#2563eb\" text-anchor=\"middle\" font-size=\"11\">{}</text>\n",
            below_gate_text_y(target.lane),
            escape_xml(&target.anchor())
        ));
    }
}

fn below_gate_text_y(lane: usize) -> i32 {
    below_gate_text_y_with_offset(lane, 0)
}

fn below_gate_text_y_with_offset(lane: usize, line_offset: usize) -> i32 {
    lane_y(lane) + GATE_HEIGHT / 2 + 14 + line_offset as i32 * ANNOTATION_LINE_GAP
}

fn above_gate_text_y_for_row(lane: usize, row: usize) -> i32 {
    lane_y(lane) - GATE_HEIGHT / 2 - ABOVE_GATE_TEXT_GAP - row as i32 * ANNOTATION_LINE_GAP
}
