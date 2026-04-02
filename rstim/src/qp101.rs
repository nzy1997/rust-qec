use std::collections::BTreeSet;

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
    },
    #[serde(rename = "tick")]
    Tick,
    #[serde(rename = "repeat")]
    Repeat {
        count: u64,
        body: Vec<Qp101Operation>,
    },
    #[serde(rename = "qubit_coords")]
    QubitCoords {
        coords: Vec<f64>,
        targets: Vec<u32>,
    },
    #[serde(rename = "shift_coords")]
    ShiftCoords {
        delta: Vec<f64>,
    },
    #[serde(rename = "detector")]
    Detector {
        coords: Vec<f64>,
        sources: Vec<Qp101TargetRef>,
    },
    #[serde(rename = "observable_include")]
    ObservableInclude {
        index: u32,
        sources: Vec<Qp101TargetRef>,
    },
    #[serde(rename = "noise")]
    Noise {
        gate: String,
        params: Vec<f64>,
        raw_targets: Vec<Qp101TargetRef>,
    },
    #[serde(rename = "annotation")]
    Annotation {
        kind: String,
        text: String,
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

    let mut doc = export_qp101(instrs)?;
    let mut seen = BTreeSet::new();
    let mut highlights = Vec::new();
    for &source_id in source_ids {
        let source = tracked.sources.get(source_id).ok_or_else(|| {
            format!(
                "tracked source index {source_id} missing for DEM error {dem_error_index}"
            )
        })?;
        let highlight = HighlightRecord::from_source(source);
        let dedupe_key = (
            highlight.op_path.clone(),
            highlight.repeat_iterations.clone(),
            highlight.target_slots.clone(),
            highlight.branch.clone(),
        );
        if seen.insert(dedupe_key) {
            highlights.push(highlight);
        }
    }

    let highlight_extension = json!({
        "rstim_query_highlights": {
            "version": "1",
            "query": {
                "kind": "dem_error_origin",
                "dem_error_index": dem_error_index,
            },
            "highlights": highlights,
        }
    });
    doc.extensions = Some(match doc.extensions.take() {
        Some(existing) => merge_extension_objects(existing, highlight_extension),
        None => highlight_extension,
    });
    Ok(doc)
}

fn export_operations(instrs: &[StimInstr]) -> Result<Vec<Qp101Operation>, String> {
    let mut ops = Vec::with_capacity(instrs.len());
    for instr in instrs {
        match instr {
            StimInstr::Repeat { count, body } => ops.push(Qp101Operation::Repeat {
                count: *count,
                body: export_operations(body)?,
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
                        Qp101Operation::Tick
                    }
                    "QUBIT_COORDS" => Qp101Operation::QubitCoords {
                        coords: args.clone(),
                        targets: export_plain_qubit_targets(name, targets)?,
                    },
                    "SHIFT_COORDS" => {
                        validate_no_targets(name, targets)?;
                        Qp101Operation::ShiftCoords {
                            delta: args.clone(),
                        }
                    }
                    "DETECTOR" => Qp101Operation::Detector {
                        coords: args.clone(),
                        sources: export_rec_sources(name, targets)?,
                    },
                    "OBSERVABLE_INCLUDE" => Qp101Operation::ObservableInclude {
                        index: parse_observable_index(args)?,
                        sources: export_rec_sources(name, targets)?,
                    },
                    n if is_noise_op(n) => Qp101Operation::Noise {
                        gate: n.to_string(),
                        params: args.clone(),
                        raw_targets: export_targets(targets),
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
                        }
                    }
                };
                ops.push(op);
            }
        }
    }
    Ok(ops)
}

fn merge_extension_objects(existing: serde_json::Value, added: serde_json::Value) -> serde_json::Value {
    match (existing, added) {
        (serde_json::Value::Object(mut existing_map), serde_json::Value::Object(added_map)) => {
            existing_map.extend(added_map);
            serde_json::Value::Object(existing_map)
        }
        (_, added) => added,
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
