use crate::ir::StimInstr;
use serde::Serialize;
use std::io::Write;

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

pub fn summarize_text(text: &str) -> Result<CircuitStatsSummary, String> {
    let instrs = crate::parser::parse_lines(text)?;
    validate_instruction_names(&instrs)?;
    Ok(summarize(&instrs))
}

pub fn write_human(summary: &CircuitStatsSummary, out: &mut dyn Write) -> Result<(), String> {
    writeln!(out, "instruction_count: {}", summary.instruction_count).map_err(|e| e.to_string())?;
    writeln!(out, "repeat_blocks: {}", summary.repeat_blocks).map_err(|e| e.to_string())?;
    writeln!(out, "max_repeat_depth: {}", summary.max_repeat_depth).map_err(|e| e.to_string())?;
    writeln!(out, "num_qubits: {}", summary.num_qubits).map_err(|e| e.to_string())?;
    writeln!(out, "num_measurements: {}", summary.num_measurements).map_err(|e| e.to_string())?;
    writeln!(out, "num_detectors: {}", summary.num_detectors).map_err(|e| e.to_string())?;
    writeln!(out, "num_observables: {}", summary.num_observables).map_err(|e| e.to_string())?;
    writeln!(out, "num_ticks: {}", summary.num_ticks).map_err(|e| e.to_string())?;
    writeln!(out, "num_sweep_bits: {}", summary.num_sweep_bits).map_err(|e| e.to_string())?;
    Ok(())
}

fn validate_instruction_names(instrs: &[StimInstr]) -> Result<(), String> {
    for instr in instrs {
        match instr {
            StimInstr::Repeat { body, .. } => validate_instruction_names(body)?,
            StimInstr::Op { name, .. } if !is_supported_instruction_name(name) => {
                return Err(format!("unsupported instruction {name}"));
            }
            StimInstr::Op { .. } => {}
        }
    }
    Ok(())
}

fn is_supported_instruction_name(name: &str) -> bool {
    // Keep this non-executing validator aligned with executor::execute_op.
    // Stats must validate REPEAT bodies without expanding their iteration counts.
    matches!(
        name,
        "I" | "I_ERROR"
            | "II_ERROR"
            | "H"
            | "H_XY"
            | "H_YZ"
            | "S"
            | "SQRT_Z"
            | "S_DAG"
            | "SQRT_Z_DAG"
            | "SQRT_X"
            | "SQRT_X_DAG"
            | "SQRT_Y"
            | "SQRT_Y_DAG"
            | "X"
            | "Y"
            | "Z"
            | "C_XYZ"
            | "C_ZYX"
            | "C_NXYZ"
            | "C_NZYX"
            | "C_XNYZ"
            | "C_XYNZ"
            | "C_ZNYX"
            | "C_ZYNX"
            | "H_NXY"
            | "H_NXZ"
            | "H_NYZ"
            | "CX"
            | "CNOT"
            | "ZCX"
            | "CY"
            | "ZCY"
            | "CZ"
            | "ZCZ"
            | "XCX"
            | "XCY"
            | "XCZ"
            | "YCX"
            | "YCY"
            | "YCZ"
            | "SWAP"
            | "ISWAP"
            | "ISWAP_DAG"
            | "CXSWAP"
            | "SWAPCX"
            | "CZSWAP"
            | "M"
            | "MZ"
            | "MX"
            | "MY"
            | "MR"
            | "MRZ"
            | "MRX"
            | "MRY"
            | "ML"
            | "MZL"
            | "MXL"
            | "MYL"
            | "MRL"
            | "MRZL"
            | "MRXL"
            | "MRYL"
            | "MPAD"
            | "R"
            | "RZ"
            | "RX"
            | "RY"
            | "LOSS"
            | "X_ERROR"
            | "Y_ERROR"
            | "Z_ERROR"
            | "DEPOLARIZE1"
            | "DEPOLARIZE2"
            | "QUBIT_COORDS"
            | "SHIFT_COORDS"
            | "TICK"
            | "DETECTOR"
            | "OBSERVABLE_INCLUDE"
            | "MXX"
            | "MYY"
            | "MZZ"
            | "MPP"
            | "SPP"
            | "SPP_DAG"
            | "PAULI_CHANNEL_1"
            | "PAULI_CHANNEL_2"
            | "HERALDED_ERASE"
            | "HERALDED_PAULI_CHANNEL_1"
            | "CORRELATED_ERROR"
            | "E"
            | "ELSE_CORRELATED_ERROR"
    )
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
/// MPAD produces 1 per target; HERALDED_* produce 1 per target.
pub fn num_measurements(instrs: &[StimInstr]) -> usize {
    crate::measurement_transform::num_measurements_unchecked(instrs)
}

/// Total DETECTOR annotation count.
pub fn num_detectors(instrs: &[StimInstr]) -> usize {
    crate::measurement_transform::num_detectors_unchecked(instrs)
}

/// One more than the largest OBSERVABLE_INCLUDE index.
pub fn num_observables(instrs: &[StimInstr]) -> usize {
    crate::measurement_transform::num_observables_unchecked(instrs)
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
