use crate::ir::StimInstr;

const NOISE_OPS: &[&str] = &[
    "X_ERROR", "Y_ERROR", "Z_ERROR",
    "DEPOLARIZE1", "DEPOLARIZE2",
    "PAULI_CHANNEL_1", "PAULI_CHANNEL_2",
    "CORRELATED_ERROR", "ELSE_CORRELATED_ERROR", "E",
    "HERALDED_ERASE", "HERALDED_PAULI_CHANNEL_1",
    "I_ERROR", "II_ERROR",
];

/// Expand all REPEAT blocks, producing a flat list of Op instructions.
pub fn flattened(instrs: &[StimInstr]) -> Vec<StimInstr> {
    let mut out = Vec::new();
    for instr in instrs {
        match instr {
            StimInstr::Op { .. } => out.push(instr.clone()),
            StimInstr::Repeat { count, body } => {
                let flat_body = flattened(body);
                for _ in 0..*count {
                    out.extend(flat_body.iter().cloned());
                }
            }
        }
    }
    out
}

/// Remove all noise instructions, preserving structure.
pub fn without_noise(instrs: &[StimInstr]) -> Vec<StimInstr> {
    let mut out = Vec::new();
    for instr in instrs {
        match instr {
            StimInstr::Op { name, .. } => {
                if !NOISE_OPS.contains(&name.as_str()) {
                    out.push(instr.clone());
                }
            }
            StimInstr::Repeat { count, body } => {
                let clean_body = without_noise(body);
                if !clean_body.is_empty() {
                    out.push(StimInstr::Repeat {
                        count: *count,
                        body: clean_body,
                    });
                }
            }
        }
    }
    out
}

/// Remove all instruction tags, preserving structure.
pub fn without_tags(instrs: &[StimInstr]) -> Vec<StimInstr> {
    instrs.iter().map(|instr| match instr {
        StimInstr::Op { name, tag: _, args, targets } => StimInstr::Op {
            name: name.clone(),
            tag: None,
            args: args.clone(),
            targets: targets.clone(),
        },
        StimInstr::Repeat { count, body } => StimInstr::Repeat {
            count: *count,
            body: without_tags(body),
        },
    }).collect()
}
