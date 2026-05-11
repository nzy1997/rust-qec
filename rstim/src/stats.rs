use crate::ir::StimInstr;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CircuitStatsSummary {
    pub instruction_count: usize,
    pub repeat_blocks: usize,
    pub max_repeat_depth: usize,
    pub num_qubits: usize,
    pub num_measurements: usize,
    pub num_detectors: usize,
    pub num_observables: usize,
    pub num_ticks: usize,
    pub num_sweep_bits: usize,
}

pub fn summarize(instrs: &[StimInstr]) -> CircuitStatsSummary {
    let (instruction_count, repeat_blocks, max_repeat_depth) = summarize_structure(instrs, 0);
    CircuitStatsSummary {
        instruction_count,
        repeat_blocks,
        max_repeat_depth,
        num_qubits: num_qubits(instrs),
        num_measurements: num_measurements(instrs),
        num_detectors: num_detectors(instrs),
        num_observables: num_observables(instrs),
        num_ticks: num_ticks(instrs),
        num_sweep_bits: num_sweep_bits(instrs),
    }
}

fn summarize_structure(instrs: &[StimInstr], depth: usize) -> (usize, usize, usize) {
    let mut instruction_count = 0;
    let mut repeat_blocks = 0;
    let mut max_repeat_depth = depth;
    for instr in instrs {
        instruction_count += 1;
        if let StimInstr::Repeat { body, .. } = instr {
            repeat_blocks += 1;
            let (inner_instruction_count, inner_repeat_blocks, inner_max_repeat_depth) =
                summarize_structure(body, depth + 1);
            instruction_count += inner_instruction_count;
            repeat_blocks += inner_repeat_blocks;
            max_repeat_depth = max_repeat_depth.max(inner_max_repeat_depth);
        }
    }
    (instruction_count, repeat_blocks, max_repeat_depth)
}

/// One more than the largest qubit index used in the circuit.
pub fn num_qubits(instrs: &[StimInstr]) -> usize {
    let mut max_q: Option<u32> = None;
    for instr in instrs {
        match instr {
            StimInstr::Op { targets, .. } => {
                for t in targets {
                    if let Some(q) = t.qubit_index() {
                        max_q = Some(max_q.map_or(q, |m: u32| m.max(q)));
                    }
                }
            }
            StimInstr::Repeat { body, .. } => {
                let inner = num_qubits(body);
                if inner > 0 {
                    let q = (inner - 1) as u32;
                    max_q = Some(max_q.map_or(q, |m| m.max(q)));
                }
            }
        }
    }
    max_q.map_or(0, |m| (m + 1) as usize)
}

/// Total measurement count. M/MX/MY/MZ/MR/MRX/MRY/MRZ produce 1 per target;
/// ML/MXL/MYL/MZL/MRL/MRXL/MRYL/MRZL produce 2 per target;
/// MPP produces 1 per Pauli product; MXX/MYY/MZZ produce 1 per pair;
/// MPAD produces its arg count; HERALDED_* produce 1 per target.
pub fn num_measurements(instrs: &[StimInstr]) -> usize {
    let mut count = 0;
    for instr in instrs {
        match instr {
            StimInstr::Op {
                name,
                targets,
                args,
                ..
            } => {
                count += match name.as_str() {
                    "M" | "MX" | "MY" | "MZ" | "MR" | "MRX" | "MRY" | "MRZ" => targets.len(),
                    "ML" | "MXL" | "MYL" | "MZL" | "MRL" | "MRXL" | "MRYL" | "MRZL" => {
                        2 * targets.len()
                    }
                    "MPP" => targets
                        .split(|t| matches!(t, crate::ir::StimTarget::Combiner))
                        .filter(|g| !g.is_empty())
                        .count(),
                    "MXX" | "MYY" | "MZZ" => targets.len() / 2,
                    "MPAD" => args.first().map_or(0, |a| *a as usize),
                    "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1" => targets.len(),
                    _ => 0,
                };
            }
            StimInstr::Repeat { count: c, body } => {
                count += (*c as usize) * num_measurements(body);
            }
        }
    }
    count
}

/// Total DETECTOR annotation count.
pub fn num_detectors(instrs: &[StimInstr]) -> usize {
    let mut count = 0;
    for instr in instrs {
        match instr {
            StimInstr::Op { name, .. } if name == "DETECTOR" => count += 1,
            StimInstr::Repeat { count: c, body } => {
                count += (*c as usize) * num_detectors(body);
            }
            _ => {}
        }
    }
    count
}

/// One more than the largest OBSERVABLE_INCLUDE index.
pub fn num_observables(instrs: &[StimInstr]) -> usize {
    let mut max_idx: Option<usize> = None;
    for instr in instrs {
        match instr {
            StimInstr::Op { name, args, .. } if name == "OBSERVABLE_INCLUDE" => {
                if let Some(&idx) = args.first() {
                    let i = idx as usize;
                    max_idx = Some(max_idx.map_or(i, |m| m.max(i)));
                }
            }
            StimInstr::Repeat { body, .. } => {
                let inner = num_observables(body);
                if inner > 0 {
                    let i = inner - 1;
                    max_idx = Some(max_idx.map_or(i, |m| m.max(i)));
                }
            }
            _ => {}
        }
    }
    max_idx.map_or(0, |m| m + 1)
}

/// Total TICK count.
pub fn num_ticks(instrs: &[StimInstr]) -> usize {
    let mut count = 0;
    for instr in instrs {
        match instr {
            StimInstr::Op { name, .. } if name == "TICK" => count += 1,
            StimInstr::Repeat { count: c, body } => {
                count += (*c as usize) * num_ticks(body);
            }
            _ => {}
        }
    }
    count
}

/// One more than the largest sweep bit index used in the circuit.
pub fn num_sweep_bits(instrs: &[StimInstr]) -> usize {
    let mut max_k: Option<u32> = None;
    for instr in instrs {
        match instr {
            StimInstr::Op { targets, .. } => {
                for t in targets {
                    if let crate::ir::StimTarget::Sweep(k) = t {
                        max_k = Some(max_k.map_or(*k, |m| m.max(*k)));
                    }
                }
            }
            StimInstr::Repeat { body, .. } => {
                let inner = num_sweep_bits(body);
                if inner > 0 {
                    let k = (inner - 1) as u32;
                    max_k = Some(max_k.map_or(k, |m| m.max(k)));
                }
            }
        }
    }
    max_k.map_or(0, |m| (m + 1) as usize)
}
