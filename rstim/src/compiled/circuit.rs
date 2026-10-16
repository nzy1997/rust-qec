use crate::ir::{StimInstr, StimTarget};
use crate::stats::{num_detectors, num_measurements, summarize};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompiledFeatureFlags {
    pub has_loss: bool,
    pub has_feedback: bool,
    pub has_nested_repeat: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledOp {
    pub name: String,
    pub args: Vec<f64>,
    pub targets: Vec<StimTarget>,
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
                pending_ops.push(CompiledOp {
                    name: name.clone(),
                    args: args.clone(),
                    targets: targets.clone(),
                });
            }
            StimInstr::Repeat { count, body } => {
                flush_pending_ops(&mut blocks, &mut pending_ops);

                let (body_blocks, body_flags) = compile_blocks(body, true);
                flags.has_loss |= body_flags.has_loss;
                flags.has_feedback |= body_flags.has_feedback;
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
        && matches!(targets, [StimTarget::Rec(_), StimTarget::Qubit(_)])
}
