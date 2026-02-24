use crate::ir::StimInstr;

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
/// MPP produces 1 per Pauli product; MXX/MYY/MZZ produce 1 per pair;
/// MPAD produces its arg count; HERALDED_* produce 1 per target.
pub fn num_measurements(instrs: &[StimInstr]) -> usize {
    let mut count = 0;
    for instr in instrs {
        match instr {
            StimInstr::Op { name, targets, args, .. } => {
                count += match name.as_str() {
                    "M" | "MX" | "MY" | "MZ" | "MR" | "MRX" | "MRY" | "MRZ" => targets.len(),
                    "MPP" => targets.split(|t| matches!(t, crate::ir::StimTarget::Combiner))
                        .filter(|g| !g.is_empty()).count(),
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
