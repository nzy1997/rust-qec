use std::collections::BTreeSet;

use crate::dem::{DemInstruction, DemTarget};
use crate::dem_provenance::{HighlightRecord, TrackedDemResult};
use crate::ir::{PauliBasis, StimInstr, StimTarget};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Qp101Document {
    pub standard: String,
    pub version: String,
    pub num_qubits: usize,
    pub operations: Vec<Qp101Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Qp101Annotation {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_slots: Vec<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<Qp101AnnotationStyle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Qp101AnnotationStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Qp101Operation {
    #[serde(rename = "gate")]
    Gate {
        gate: String,
        targets: Vec<u32>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        controls: Vec<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        control_configs: Option<Vec<bool>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        params: Vec<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        raw_targets: Option<Vec<Qp101TargetRef>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        display: Option<Qp101Display>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tags: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        annotations: Vec<Qp101Annotation>,
    },
    #[serde(rename = "tick")]
    Tick {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        annotations: Vec<Qp101Annotation>,
    },
    #[serde(rename = "repeat")]
    Repeat {
        count: u64,
        body: Vec<Qp101Operation>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        annotations: Vec<Qp101Annotation>,
    },
    #[serde(rename = "qubit_coords")]
    QubitCoords {
        coords: Vec<f64>,
        targets: Vec<u32>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        annotations: Vec<Qp101Annotation>,
    },
    #[serde(rename = "shift_coords")]
    ShiftCoords {
        delta: Vec<f64>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        annotations: Vec<Qp101Annotation>,
    },
    #[serde(rename = "detector")]
    Detector {
        coords: Vec<f64>,
        sources: Vec<Qp101TargetRef>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        annotations: Vec<Qp101Annotation>,
    },
    #[serde(rename = "observable_include")]
    ObservableInclude {
        index: u32,
        sources: Vec<Qp101TargetRef>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        annotations: Vec<Qp101Annotation>,
    },
    #[serde(rename = "noise")]
    Noise {
        gate: String,
        params: Vec<f64>,
        raw_targets: Vec<Qp101TargetRef>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        annotations: Vec<Qp101Annotation>,
    },
    #[serde(rename = "annotation")]
    Annotation {
        kind: String,
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        annotations: Vec<Qp101Annotation>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum Qp101TargetRef {
    #[serde(rename = "qubit")]
    Qubit {
        index: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        inverted: Option<bool>,
    },
    #[serde(rename = "rec")]
    Rec {
        offset: i32,
    },
    #[serde(rename = "pauli")]
    Pauli {
        basis: Qp101PauliBasis,
        qubit: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        inverted: Option<bool>,
    },
    #[serde(rename = "combiner")]
    Combiner,
    #[serde(rename = "sweep")]
    Sweep {
        index: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Qp101Display {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Qp101PauliBasis {
    X,
    Y,
    Z,
}

pub fn export_qp101(instrs: &[StimInstr]) -> Result<Qp101Document, String> {
    Ok(Qp101Document {
        standard: "QP101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: crate::stats::num_qubits(instrs),
        operations: export_operations(instrs)?,
        metadata: Some(json!({ "framework": "rstim" })),
        extensions: None,
    })
}

pub fn export_qp101_with_highlighted_dem_error(
    instrs: &[StimInstr],
    tracked: &TrackedDemResult,
    dem_error_index: usize,
) -> Result<Qp101Document, String> {
    let source_ids = tracked
        .dem_error_to_sources
        .get(dem_error_index)
        .ok_or_else(|| {
            format!(
                "DEM error index {dem_error_index} out of range for {} tracked DEM errors",
                tracked.dem_error_to_sources.len()
            )
        })?;

    let mut source_highlights = Vec::with_capacity(source_ids.len());
    for &source_id in source_ids {
        let source = tracked.sources.get(source_id).ok_or_else(|| {
            format!("tracked source index {source_id} missing for DEM error {dem_error_index}")
        })?;
        source_highlights.push((source, HighlightRecord::from_source(source)));
    }

    let dem_targets = match tracked.dem.instructions().get(dem_error_index) {
        Some(DemInstruction::Error { targets, .. }) => targets.clone(),
        Some(_) => {
            return Err(format!(
                "DEM error index {dem_error_index} does not point to an error instruction"
            ));
        }
        None => {
            return Err(format!(
                "DEM error index {dem_error_index} out of range for {} tracked DEM errors",
                tracked.dem_error_to_sources.len()
            ));
        }
    };

    let mut doc = export_qp101(instrs)?;
    let mut seen = BTreeSet::new();
    for (source, highlight) in source_highlights {
        let dedupe_key = (
            highlight.op_path.clone(),
            highlight.repeat_iterations.clone(),
            highlight.target_slots.clone(),
            highlight.branch.clone(),
        );
        if seen.insert(dedupe_key) {
            let annotation = Qp101Annotation {
                kind: "marker".to_string(),
                target_slots: highlight.target_slots.clone(),
                label: Some(highlight.label.clone()),
                text: format_repeat_annotation_text(&highlight.repeat_iterations),
                style: Some(danger_annotation_style()),
                tags: vec!["dem-origin".to_string(), "query-result".to_string()],
                context: Some(json!({
                    "query_kind": "dem_error_origin",
                    "dem_error_index": dem_error_index,
                    "op_path": highlight.op_path,
                    "repeat_iterations": highlight.repeat_iterations,
                    "target_qubits": highlight.target_qubits,
                    "source_branch": highlight.branch,
                })),
            };
            add_annotation_at_path(&mut doc.operations, &source.op_path, annotation)?;
        }
    }
    add_symptom_annotations(&mut doc.operations, instrs, &dem_targets, dem_error_index)?;
    Ok(doc)
}

fn export_operations(instrs: &[StimInstr]) -> Result<Vec<Qp101Operation>, String> {
    let mut ops = Vec::with_capacity(instrs.len());
    for instr in instrs {
        match instr {
            StimInstr::Repeat { count, body } => ops.push(Qp101Operation::Repeat {
                count: *count,
                body: export_operations(body)?,
                annotations: Vec::new(),
            }),
            StimInstr::Op {
                name,
                tag,
                args,
                targets,
                ..
            } => {
                let op = match name.as_str() {
                    "TICK" => {
                        validate_no_args_or_targets(name, args, targets)?;
                        Qp101Operation::Tick {
                            annotations: Vec::new(),
                        }
                    }
                    "QUBIT_COORDS" => Qp101Operation::QubitCoords {
                        coords: args.clone(),
                        targets: export_plain_qubit_targets(name, targets)?,
                        annotations: Vec::new(),
                    },
                    "SHIFT_COORDS" => {
                        validate_no_targets(name, targets)?;
                        Qp101Operation::ShiftCoords {
                            delta: args.clone(),
                            annotations: Vec::new(),
                        }
                    }
                    "DETECTOR" => Qp101Operation::Detector {
                        coords: args.clone(),
                        sources: export_rec_sources(name, targets)?,
                        annotations: Vec::new(),
                    },
                    "OBSERVABLE_INCLUDE" => Qp101Operation::ObservableInclude {
                        index: parse_observable_index(args)?,
                        sources: export_rec_sources(name, targets)?,
                        annotations: Vec::new(),
                    },
                    n if is_noise_op(n) => Qp101Operation::Noise {
                        gate: n.to_string(),
                        params: args.clone(),
                        raw_targets: export_targets(targets),
                        annotations: Vec::new(),
                    },
                    _ => {
                        let targets_out: Vec<u32> =
                            targets.iter().filter_map(StimTarget::qubit_index).collect();
                        let has_non_plain_targets = targets
                            .iter()
                            .any(|target| !matches!(target, StimTarget::Qubit(_)));
                        Qp101Operation::Gate {
                            gate: name.clone(),
                            targets: targets_out,
                            controls: Vec::new(),
                            control_configs: None,
                            params: args.clone(),
                            raw_targets: if has_non_plain_targets {
                                Some(export_targets(targets))
                            } else {
                                None
                            },
                            display: None,
                            tags: tag.iter().cloned().collect(),
                            annotations: Vec::new(),
                        }
                    }
                };
                ops.push(op);
            }
        }
    }
    Ok(ops)
}

fn format_repeat_annotation_text(repeat_iterations: &[u64]) -> Option<String> {
    (!repeat_iterations.is_empty()).then(|| {
        format!(
            "repeat[{}]",
            repeat_iterations
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    })
}

fn danger_annotation_style() -> Qp101AnnotationStyle {
    Qp101AnnotationStyle {
        preset: Some("danger".to_string()),
        color: Some("red".to_string()),
        highlight: Some(true),
    }
}

#[derive(Debug, Clone)]
struct QueryAnnotationLocation {
    op_path: Vec<usize>,
    repeat_iterations: Vec<u64>,
}

#[derive(Debug, Clone, Default)]
struct QueryTraversalContext {
    op_path: Vec<usize>,
    repeat_iterations: Vec<u64>,
}

impl QueryTraversalContext {
    fn with_op_index(&self, op_index: usize) -> Self {
        let mut op_path = self.op_path.clone();
        op_path.push(op_index);
        Self {
            op_path,
            repeat_iterations: self.repeat_iterations.clone(),
        }
    }

    fn with_repeat_iteration(&self, repeat_op_index: usize, iteration: u64) -> Self {
        let mut op_path = self.op_path.clone();
        op_path.push(repeat_op_index);
        let mut repeat_iterations = self.repeat_iterations.clone();
        repeat_iterations.push(iteration);
        Self {
            op_path,
            repeat_iterations,
        }
    }
}

fn add_symptom_annotations(
    operations: &mut [Qp101Operation],
    instrs: &[StimInstr],
    dem_targets: &[DemTarget],
    dem_error_index: usize,
) -> Result<(), String> {
    let mut detector_locations = Vec::new();
    let mut observable_locations = Vec::new();
    collect_query_annotation_locations(
        instrs,
        &QueryTraversalContext::default(),
        &mut detector_locations,
        &mut observable_locations,
    )?;

    let mut seen = BTreeSet::new();
    for target in dem_targets {
        match target {
            DemTarget::Detector(index) => {
                let location = detector_locations.get(*index).ok_or_else(|| {
                    format!("detector index {index} missing from exported QP101 operations")
                })?;
                let key = (
                    "D".to_string(),
                    *index,
                    location.op_path.clone(),
                    location.repeat_iterations.clone(),
                );
                if seen.insert(key) {
                    add_annotation_at_path(
                        operations,
                        &location.op_path,
                        Qp101Annotation {
                            kind: "marker".to_string(),
                            target_slots: Vec::new(),
                            label: Some(format!("D{index}")),
                            text: format_repeat_annotation_text(&location.repeat_iterations),
                            style: Some(danger_annotation_style()),
                            tags: vec!["dem-symptom".to_string(), "query-result".to_string()],
                            context: Some(json!({
                                "query_kind": "dem_error_origin",
                                "dem_error_index": dem_error_index,
                                "detector_index": index,
                                "op_path": location.op_path,
                                "repeat_iterations": location.repeat_iterations,
                            })),
                        },
                    )?;
                }
            }
            DemTarget::Observable(index) => {
                let matching_locations: Vec<_> = observable_locations
                    .iter()
                    .filter(|entry| entry.0 == *index)
                    .collect();
                // A DEM logical observable target only names the aggregate observable index.
                // If that index is emitted by multiple OBSERVABLE_INCLUDE ops, the exact site is ambiguous.
                if let [location] = matching_locations.as_slice() {
                    let key = (
                        "L".to_string(),
                        *index,
                        location.1.op_path.clone(),
                        location.1.repeat_iterations.clone(),
                    );
                    if seen.insert(key) {
                        add_annotation_at_path(
                            operations,
                            &location.1.op_path,
                            Qp101Annotation {
                                kind: "marker".to_string(),
                                target_slots: Vec::new(),
                                label: Some(format!("L{index}")),
                                text: format_repeat_annotation_text(&location.1.repeat_iterations),
                                style: Some(danger_annotation_style()),
                                tags: vec!["dem-symptom".to_string(), "query-result".to_string()],
                                context: Some(json!({
                                    "query_kind": "dem_error_origin",
                                    "dem_error_index": dem_error_index,
                                    "observable_index": index,
                                    "op_path": location.1.op_path,
                                    "repeat_iterations": location.1.repeat_iterations,
                                })),
                            },
                        )?;
                    }
                }
            }
            DemTarget::Separator => {}
        }
    }
    Ok(())
}

fn collect_query_annotation_locations(
    instrs: &[StimInstr],
    context: &QueryTraversalContext,
    detector_locations: &mut Vec<QueryAnnotationLocation>,
    observable_locations: &mut Vec<(usize, QueryAnnotationLocation)>,
) -> Result<(), String> {
    for (i, instr) in instrs.iter().enumerate() {
        match instr {
            StimInstr::Repeat { count, body } => {
                for iteration in 0..*count {
                    let repeat_context = context.with_repeat_iteration(i, iteration);
                    collect_query_annotation_locations(
                        body,
                        &repeat_context,
                        detector_locations,
                        observable_locations,
                    )?;
                }
            }
            StimInstr::Op { name, args, .. } => {
                let op_context = context.with_op_index(i);
                if name == "DETECTOR" {
                    detector_locations.push(QueryAnnotationLocation {
                        op_path: op_context.op_path,
                        repeat_iterations: op_context.repeat_iterations,
                    });
                } else if name == "OBSERVABLE_INCLUDE" {
                    let index = args.first().copied().unwrap_or(0.0) as usize;
                    observable_locations.push((
                        index,
                        QueryAnnotationLocation {
                            op_path: op_context.op_path,
                            repeat_iterations: op_context.repeat_iterations,
                        },
                    ));
                }
            }
        }
    }
    Ok(())
}

fn add_annotation_at_path(
    operations: &mut [Qp101Operation],
    op_path: &[usize],
    annotation: Qp101Annotation,
) -> Result<(), String> {
    let Some((&head, tail)) = op_path.split_first() else {
        return Err("highlight source is missing an op_path".to_string());
    };
    let op = operations
        .get_mut(head)
        .ok_or_else(|| format!("highlight op_path component {head} out of range"))?;
    if tail.is_empty() {
        op.annotations_mut().push(annotation);
        return Ok(());
    }
    match op {
        Qp101Operation::Repeat { body, .. } => add_annotation_at_path(body, tail, annotation),
        _ => Err(format!(
            "highlight op_path descends into non-repeat operation at component {head}"
        )),
    }
}

impl Qp101Operation {
    fn annotations_mut(&mut self) -> &mut Vec<Qp101Annotation> {
        match self {
            Qp101Operation::Gate { annotations, .. }
            | Qp101Operation::Tick { annotations }
            | Qp101Operation::Repeat { annotations, .. }
            | Qp101Operation::QubitCoords { annotations, .. }
            | Qp101Operation::ShiftCoords { annotations, .. }
            | Qp101Operation::Detector { annotations, .. }
            | Qp101Operation::ObservableInclude { annotations, .. }
            | Qp101Operation::Noise { annotations, .. }
            | Qp101Operation::Annotation { annotations, .. } => annotations,
        }
    }
}

fn export_plain_qubit_targets(name: &str, targets: &[StimTarget]) -> Result<Vec<u32>, String> {
    targets
        .iter()
        .map(|target| match target {
            StimTarget::Qubit(q) => Ok(*q),
            other => Err(format!("{name} expects qubit targets, got {other:?}")),
        })
        .collect()
}

fn validate_no_args_or_targets(name: &str, args: &[f64], targets: &[StimTarget]) -> Result<(), String> {
    if !args.is_empty() {
        return Err(format!("{name} expects no args, got {}", args.len()));
    }
    validate_no_targets(name, targets)
}

fn validate_no_targets(name: &str, targets: &[StimTarget]) -> Result<(), String> {
    if !targets.is_empty() {
        return Err(format!("{name} expects no targets, got {}", targets.len()));
    }
    Ok(())
}

fn parse_observable_index(args: &[f64]) -> Result<u32, String> {
    if args.len() != 1 {
        return Err(format!(
            "OBSERVABLE_INCLUDE requires exactly one argument, got {}",
            args.len()
        ));
    }
    let Some(raw) = args.first().copied() else {
        return Err("OBSERVABLE_INCLUDE requires an observable index".to_string());
    };
    if raw < 0.0 || raw.fract() != 0.0 {
        return Err(format!(
            "OBSERVABLE_INCLUDE index must be a non-negative integer, got {raw}"
        ));
    }
    if raw > u32::MAX as f64 {
        return Err(format!(
            "OBSERVABLE_INCLUDE index exceeds u32 range, got {raw}"
        ));
    }
    Ok(raw as u32)
}

fn export_rec_sources(name: &str, targets: &[StimTarget]) -> Result<Vec<Qp101TargetRef>, String> {
    targets
        .iter()
        .map(|target| match target {
            StimTarget::Rec(offset) => Ok(Qp101TargetRef::Rec { offset: *offset }),
            other => Err(format!("{name} expects rec sources, got {other:?}")),
        })
        .collect()
}

fn export_targets(targets: &[StimTarget]) -> Vec<Qp101TargetRef> {
    targets.iter().map(export_target).collect()
}

fn export_target(target: &StimTarget) -> Qp101TargetRef {
    match target {
        StimTarget::Qubit(index) => Qp101TargetRef::Qubit {
            index: *index,
            inverted: None,
        },
        StimTarget::QubitInv(index) => Qp101TargetRef::Qubit {
            index: *index,
            inverted: Some(true),
        },
        StimTarget::Rec(offset) => Qp101TargetRef::Rec { offset: *offset },
        StimTarget::Pauli {
            qubit,
            basis,
            inverted,
        } => Qp101TargetRef::Pauli {
            basis: export_pauli_basis(*basis),
            qubit: *qubit,
            inverted: if *inverted { Some(true) } else { None },
        },
        StimTarget::Combiner => Qp101TargetRef::Combiner,
        StimTarget::Sweep(index) => Qp101TargetRef::Sweep { index: *index },
    }
}

fn export_pauli_basis(basis: PauliBasis) -> Qp101PauliBasis {
    match basis {
        PauliBasis::X => Qp101PauliBasis::X,
        PauliBasis::Y => Qp101PauliBasis::Y,
        PauliBasis::Z => Qp101PauliBasis::Z,
    }
}

fn is_noise_op(name: &str) -> bool {
    matches!(
        name,
        "X_ERROR"
            | "Y_ERROR"
            | "Z_ERROR"
            | "DEPOLARIZE1"
            | "DEPOLARIZE2"
            | "PAULI_CHANNEL_1"
            | "PAULI_CHANNEL_2"
            | "CORRELATED_ERROR"
            | "ELSE_CORRELATED_ERROR"
            | "E"
            | "HERALDED_ERASE"
            | "HERALDED_PAULI_CHANNEL_1"
            | "I_ERROR"
            | "II_ERROR"
    )
}
