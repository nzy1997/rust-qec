use crate::ir::{PauliBasis, StimInstr, StimTarget};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Qstd101Document {
    pub standard: String,
    pub version: String,
    pub num_qubits: usize,
    pub operations: Vec<Qstd101Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Qstd101Operation {
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
        raw_targets: Option<Vec<Qstd101TargetRef>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        display: Option<Qstd101Display>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tags: Vec<String>,
    },
    #[serde(rename = "tick")]
    Tick,
    #[serde(rename = "repeat")]
    Repeat {
        count: u64,
        body: Vec<Qstd101Operation>,
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
        sources: Vec<Qstd101TargetRef>,
    },
    #[serde(rename = "observable_include")]
    ObservableInclude {
        index: u32,
        sources: Vec<Qstd101TargetRef>,
    },
    #[serde(rename = "noise")]
    Noise {
        gate: String,
        params: Vec<f64>,
        raw_targets: Vec<Qstd101TargetRef>,
    },
    #[serde(rename = "annotation")]
    Annotation {
        kind: String,
        text: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum Qstd101TargetRef {
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
        basis: Qstd101PauliBasis,
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
pub struct Qstd101Display {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Qstd101PauliBasis {
    X,
    Y,
    Z,
}

pub fn export_qstd101(instrs: &[StimInstr]) -> Result<Qstd101Document, String> {
    Ok(Qstd101Document {
        standard: "QSTD101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: crate::stats::num_qubits(instrs),
        operations: export_operations(instrs)?,
        metadata: Some(json!({ "framework": "rstim" })),
        extensions: None,
    })
}

fn export_operations(instrs: &[StimInstr]) -> Result<Vec<Qstd101Operation>, String> {
    let mut ops = Vec::with_capacity(instrs.len());
    for instr in instrs {
        match instr {
            StimInstr::Repeat { count, body } => ops.push(Qstd101Operation::Repeat {
                count: *count,
                body: export_operations(body)?,
            }),
            StimInstr::Op {
                name,
                args,
                targets,
                ..
            } => {
                let op = match name.as_str() {
                    "TICK" => Qstd101Operation::Tick,
                    "QUBIT_COORDS" => Qstd101Operation::QubitCoords {
                        coords: args.clone(),
                        targets: export_qubit_targets(name, targets)?,
                    },
                    "SHIFT_COORDS" => Qstd101Operation::ShiftCoords {
                        delta: args.clone(),
                    },
                    "DETECTOR" => Qstd101Operation::Detector {
                        coords: args.clone(),
                        sources: export_targets(targets),
                    },
                    "OBSERVABLE_INCLUDE" => Qstd101Operation::ObservableInclude {
                        index: parse_observable_index(args)?,
                        sources: export_targets(targets),
                    },
                    n if is_noise_op(n) => Qstd101Operation::Noise {
                        gate: n.to_string(),
                        params: args.clone(),
                        raw_targets: export_targets(targets),
                    },
                    _ => {
                        let targets_out: Vec<u32> = targets
                            .iter()
                            .filter_map(|target| match target {
                                StimTarget::Qubit(q) | StimTarget::QubitInv(q) => Some(*q),
                                _ => None,
                            })
                            .collect();
                        let has_non_plain_targets = targets
                            .iter()
                            .any(|target| !matches!(target, StimTarget::Qubit(_)));
                        Qstd101Operation::Gate {
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
                            tags: Vec::new(),
                        }
                    }
                };
                ops.push(op);
            }
        }
    }
    Ok(ops)
}

fn export_qubit_targets(name: &str, targets: &[StimTarget]) -> Result<Vec<u32>, String> {
    targets
        .iter()
        .map(|target| match target {
            StimTarget::Qubit(q) | StimTarget::QubitInv(q) => Ok(*q),
            other => Err(format!("{name} expects qubit targets, got {other:?}")),
        })
        .collect()
}

fn parse_observable_index(args: &[f64]) -> Result<u32, String> {
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

fn export_targets(targets: &[StimTarget]) -> Vec<Qstd101TargetRef> {
    targets.iter().map(export_target).collect()
}

fn export_target(target: &StimTarget) -> Qstd101TargetRef {
    match target {
        StimTarget::Qubit(index) => Qstd101TargetRef::Qubit {
            index: *index,
            inverted: None,
        },
        StimTarget::QubitInv(index) => Qstd101TargetRef::Qubit {
            index: *index,
            inverted: Some(true),
        },
        StimTarget::Rec(offset) => Qstd101TargetRef::Rec { offset: *offset },
        StimTarget::Pauli {
            qubit,
            basis,
            inverted,
        } => Qstd101TargetRef::Pauli {
            basis: export_pauli_basis(*basis),
            qubit: *qubit,
            inverted: if *inverted { Some(true) } else { None },
        },
        StimTarget::Combiner => Qstd101TargetRef::Combiner,
        StimTarget::Sweep(index) => Qstd101TargetRef::Sweep { index: *index },
    }
}

fn export_pauli_basis(basis: PauliBasis) -> Qstd101PauliBasis {
    match basis {
        PauliBasis::X => Qstd101PauliBasis::X,
        PauliBasis::Y => Qstd101PauliBasis::Y,
        PauliBasis::Z => Qstd101PauliBasis::Z,
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
