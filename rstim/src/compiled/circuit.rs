use crate::ir::{StimInstr, StimTarget};
use crate::stats::{num_detectors, num_measurements, summarize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompiledBasis {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompiledFeatureFlags {
    pub has_loss: bool,
    pub has_feedback: bool,
    pub has_sweep_dependency: bool,
    pub has_nested_repeat: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompiledOp {
    Tick,
    QubitCoords,
    ShiftCoords,
    NoOp,
    H {
        qubits: Vec<usize>,
    },
    Reset {
        basis: CompiledBasis,
        qubits: Vec<usize>,
    },
    XError {
        probability: f64,
        qubits: Vec<usize>,
    },
    YError {
        probability: f64,
        qubits: Vec<usize>,
    },
    ZError {
        probability: f64,
        qubits: Vec<usize>,
    },
    Depolarize1 {
        probability: f64,
        qubits: Vec<usize>,
    },
    Cx {
        pairs: Vec<(usize, usize)>,
    },
    Depolarize2 {
        probability: f64,
        pairs: Vec<(usize, usize)>,
    },
    Measure {
        basis: CompiledBasis,
        qubits: Vec<usize>,
    },
    MeasureReset {
        basis: CompiledBasis,
        qubits: Vec<usize>,
    },
    Detector {
        rec_offsets: Vec<usize>,
    },
    ObservableInclude {
        observable_index: usize,
        rec_offsets: Vec<usize>,
    },
    UnsupportedSamplerOp {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledRepeatRegion {
    pub count: u64,
    pub body: Vec<CompiledBlock>,
    pub body_source: Vec<StimInstr>,
    pub measurement_span: usize,
    pub detector_span: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompiledBlock {
    Ops(Vec<CompiledOp>),
    Repeat(CompiledRepeatRegion),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledCircuit {
    pub source: Vec<StimInstr>,
    pub blocks: Vec<CompiledBlock>,
    pub flags: CompiledFeatureFlags,
    pub num_qubits: usize,
    pub num_measurements: usize,
    pub num_detectors: usize,
    pub num_observables: usize,
}

pub fn compile_circuit(instrs: &[StimInstr]) -> Result<CompiledCircuit, String> {
    let summary = summarize(instrs);
    let (blocks, flags) = compile_blocks(instrs, false);
    Ok(CompiledCircuit {
        source: instrs.to_vec(),
        blocks,
        flags,
        num_qubits: summary.num_qubits,
        num_measurements: summary.num_measurements,
        num_detectors: summary.num_detectors,
        num_observables: summary.num_observables,
    })
}

fn compile_blocks(
    instrs: &[StimInstr],
    inside_repeat: bool,
) -> (Vec<CompiledBlock>, CompiledFeatureFlags) {
    let mut blocks = Vec::new();
    let mut pending_ops = Vec::new();
    let mut flags = CompiledFeatureFlags::default();

    for instr in instrs {
        match instr {
            StimInstr::Op {
                name,
                args,
                targets,
                ..
            } => {
                flags.has_loss |= is_loss_operation(name);
                flags.has_feedback |= is_feedback_operation(name, targets);
                flags.has_sweep_dependency |= is_sweep_dependent_operation(name, targets);
                pending_ops.push(compile_sampler_op(name, args, targets));
            }
            StimInstr::Repeat { count, body } => {
                flush_pending_ops(&mut blocks, &mut pending_ops);

                let (body_blocks, body_flags) = compile_blocks(body, true);
                flags.has_loss |= body_flags.has_loss;
                flags.has_feedback |= body_flags.has_feedback;
                flags.has_sweep_dependency |= body_flags.has_sweep_dependency;
                flags.has_nested_repeat |= inside_repeat || body_flags.has_nested_repeat;

                blocks.push(CompiledBlock::Repeat(CompiledRepeatRegion {
                    count: *count,
                    body: body_blocks,
                    body_source: body.clone(),
                    measurement_span: num_measurements(body),
                    detector_span: num_detectors(body),
                }));
            }
        }
    }

    flush_pending_ops(&mut blocks, &mut pending_ops);

    (blocks, flags)
}

fn flush_pending_ops(blocks: &mut Vec<CompiledBlock>, pending_ops: &mut Vec<CompiledOp>) {
    if pending_ops.is_empty() {
        return;
    }
    blocks.push(CompiledBlock::Ops(std::mem::take(pending_ops)));
}

fn compile_sampler_op(name: &str, args: &[f64], targets: &[StimTarget]) -> CompiledOp {
    match name {
        "TICK" => CompiledOp::Tick,
        "QUBIT_COORDS" => CompiledOp::QubitCoords,
        "SHIFT_COORDS" => CompiledOp::ShiftCoords,
        "I" | "X" | "Y" | "Z" | "I_ERROR" | "II_ERROR" => CompiledOp::NoOp,
        "H" => qubits(targets)
            .map(|qubits| CompiledOp::H { qubits })
            .unwrap_or_else(|| unsupported(name)),
        "R" | "RZ" => qubits(targets)
            .map(|qubits| CompiledOp::Reset {
                basis: CompiledBasis::Z,
                qubits,
            })
            .unwrap_or_else(|| unsupported(name)),
        "RX" => qubits(targets)
            .map(|qubits| CompiledOp::Reset {
                basis: CompiledBasis::X,
                qubits,
            })
            .unwrap_or_else(|| unsupported(name)),
        "RY" => qubits(targets)
            .map(|qubits| CompiledOp::Reset {
                basis: CompiledBasis::Y,
                qubits,
            })
            .unwrap_or_else(|| unsupported(name)),
        "X_ERROR" => qubits(targets)
            .map(|qubits| CompiledOp::XError {
                probability: first_arg(args),
                qubits,
            })
            .unwrap_or_else(|| unsupported(name)),
        "Y_ERROR" => qubits(targets)
            .map(|qubits| CompiledOp::YError {
                probability: first_arg(args),
                qubits,
            })
            .unwrap_or_else(|| unsupported(name)),
        "Z_ERROR" => qubits(targets)
            .map(|qubits| CompiledOp::ZError {
                probability: first_arg(args),
                qubits,
            })
            .unwrap_or_else(|| unsupported(name)),
        "DEPOLARIZE1" => qubits(targets)
            .map(|qubits| CompiledOp::Depolarize1 {
                probability: first_arg(args),
                qubits,
            })
            .unwrap_or_else(|| unsupported(name)),
        "CX" | "CNOT" | "ZCX" => qubit_pairs(targets)
            .map(|pairs| CompiledOp::Cx { pairs })
            .unwrap_or_else(|| unsupported(name)),
        "DEPOLARIZE2" => qubit_pairs(targets)
            .map(|pairs| CompiledOp::Depolarize2 {
                probability: first_arg(args),
                pairs,
            })
            .unwrap_or_else(|| unsupported(name)),
        "M" | "MZ" => qubits_ignoring_inv(targets)
            .map(|qubits| CompiledOp::Measure {
                basis: CompiledBasis::Z,
                qubits,
            })
            .unwrap_or_else(|| unsupported(name)),
        "MX" => qubits_ignoring_inv(targets)
            .map(|qubits| CompiledOp::Measure {
                basis: CompiledBasis::X,
                qubits,
            })
            .unwrap_or_else(|| unsupported(name)),
        "MY" => qubits_ignoring_inv(targets)
            .map(|qubits| CompiledOp::Measure {
                basis: CompiledBasis::Y,
                qubits,
            })
            .unwrap_or_else(|| unsupported(name)),
        "MR" | "MRZ" => qubits_ignoring_inv(targets)
            .map(|qubits| CompiledOp::MeasureReset {
                basis: CompiledBasis::Z,
                qubits,
            })
            .unwrap_or_else(|| unsupported(name)),
        "MRX" => qubits_ignoring_inv(targets)
            .map(|qubits| CompiledOp::MeasureReset {
                basis: CompiledBasis::X,
                qubits,
            })
            .unwrap_or_else(|| unsupported(name)),
        "MRY" => qubits_ignoring_inv(targets)
            .map(|qubits| CompiledOp::MeasureReset {
                basis: CompiledBasis::Y,
                qubits,
            })
            .unwrap_or_else(|| unsupported(name)),
        "DETECTOR" => rec_offsets(targets)
            .map(|rec_offsets| CompiledOp::Detector { rec_offsets })
            .unwrap_or_else(|| unsupported(name)),
        "OBSERVABLE_INCLUDE" => rec_offsets(targets)
            .map(|rec_offsets| CompiledOp::ObservableInclude {
                observable_index: first_arg(args) as usize,
                rec_offsets,
            })
            .unwrap_or_else(|| unsupported(name)),
        _ => unsupported(name),
    }
}

fn unsupported(name: &str) -> CompiledOp {
    CompiledOp::UnsupportedSamplerOp {
        name: name.to_string(),
    }
}

fn first_arg(args: &[f64]) -> f64 {
    args.first().copied().unwrap_or(0.0)
}

fn qubits(targets: &[StimTarget]) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    for target in targets {
        match target {
            StimTarget::Qubit(q) => out.push(*q as usize),
            StimTarget::Sweep(_) => {}
            _ => return None,
        }
    }
    Some(out)
}

fn qubits_ignoring_inv(targets: &[StimTarget]) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    for target in targets {
        match target {
            StimTarget::Qubit(q) | StimTarget::QubitInv(q) => out.push(*q as usize),
            StimTarget::Sweep(_) => {}
            _ => return None,
        }
    }
    Some(out)
}

fn qubit_pairs(targets: &[StimTarget]) -> Option<Vec<(usize, usize)>> {
    if targets.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::new();
    let mut iter = targets.iter();
    while let (Some(a), Some(b)) = (iter.next(), iter.next()) {
        if matches!(a, StimTarget::Sweep(_)) || matches!(b, StimTarget::Sweep(_)) {
            continue;
        }
        let StimTarget::Qubit(qa) = a else {
            return None;
        };
        let StimTarget::Qubit(qb) = b else {
            return None;
        };
        out.push((*qa as usize, *qb as usize));
    }
    Some(out)
}

fn rec_offsets(targets: &[StimTarget]) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    for target in targets {
        match target {
            StimTarget::Rec(offset) if *offset < 0 => out.push((-*offset) as usize),
            _ => return None,
        }
    }
    Some(out)
}

fn is_loss_operation(name: &str) -> bool {
    matches!(
        name,
        "LOSS"
            | "ML"
            | "MXL"
            | "MYL"
            | "MZL"
            | "MRL"
            | "MRXL"
            | "MRYL"
            | "MRZL"
            | "HERALDED_ERASE"
            | "HERALDED_PAULI_CHANNEL_1"
    )
}

fn is_feedback_operation(name: &str, targets: &[StimTarget]) -> bool {
    matches!(name, "CX" | "CNOT" | "ZCX" | "CY" | "ZCY" | "CZ" | "ZCZ")
        && targets
            .chunks_exact(2)
            .any(|pair| matches!(pair, [StimTarget::Rec(_), StimTarget::Qubit(_)]))
}

fn is_sweep_dependent_operation(name: &str, targets: &[StimTarget]) -> bool {
    targets
        .iter()
        .any(|target| matches!(target, StimTarget::Sweep(_)))
        && !is_noiselessly_skipped_or_metadata_operation(name)
}

fn is_noiselessly_skipped_or_metadata_operation(name: &str) -> bool {
    matches!(
        name,
        "I" | "I_ERROR"
            | "II_ERROR"
            | "X_ERROR"
            | "Y_ERROR"
            | "Z_ERROR"
            | "DEPOLARIZE1"
            | "DEPOLARIZE2"
            | "TICK"
            | "QUBIT_COORDS"
            | "SHIFT_COORDS"
            | "DETECTOR"
            | "OBSERVABLE_INCLUDE"
    )
}
