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

fn invert_gate(name: &str) -> Result<String, String> {
    match name {
        "I" | "X" | "Y" | "Z" | "H" | "H_XY" | "H_YZ" | "H_NXY" | "H_NXZ" | "H_NYZ"
        | "CX" | "CY" | "CZ" | "CNOT" | "ZCX" | "ZCY" | "ZCZ"
        | "XCX" | "XCY" | "XCZ" | "YCX" | "YCY" | "YCZ"
        | "SWAP" | "CZSWAP" => Ok(name.to_string()),

        "S" => Ok("S_DAG".to_string()),
        "S_DAG" | "SQRT_Z_DAG" => Ok("S".to_string()),
        "SQRT_X" => Ok("SQRT_X_DAG".to_string()),
        "SQRT_X_DAG" => Ok("SQRT_X".to_string()),
        "SQRT_Y" => Ok("SQRT_Y_DAG".to_string()),
        "SQRT_Y_DAG" => Ok("SQRT_Y".to_string()),
        "SQRT_Z" => Ok("S_DAG".to_string()),
        "ISWAP" => Ok("ISWAP_DAG".to_string()),
        "ISWAP_DAG" => Ok("ISWAP".to_string()),
        "CXSWAP" => Ok("SWAPCX".to_string()),
        "SWAPCX" => Ok("CXSWAP".to_string()),

        "C_XYZ" => Ok("C_ZYX".to_string()),
        "C_ZYX" => Ok("C_XYZ".to_string()),
        "C_NXYZ" => Ok("C_XYNZ".to_string()),
        "C_XYNZ" => Ok("C_NXYZ".to_string()),
        "C_XNYZ" => Ok("C_ZNYX".to_string()),
        "C_ZNYX" => Ok("C_XNYZ".to_string()),
        "C_NZYX" => Ok("C_ZYNX".to_string()),
        "C_ZYNX" => Ok("C_NZYX".to_string()),

        n if NOISE_OPS.contains(&n) => Err(format!("cannot invert noise operation: {n}")),
        "M" | "MX" | "MY" | "MZ" | "MR" | "MRX" | "MRY" | "MRZ"
        | "MPP" | "MXX" | "MYY" | "MZZ" | "MPAD" => {
            Err(format!("cannot invert measurement: {name}"))
        }
        "R" | "RX" | "RY" | "RZ" => Err(format!("cannot invert reset: {name}")),
        "DETECTOR" | "OBSERVABLE_INCLUDE" | "TICK" | "QUBIT_COORDS"
        | "SHIFT_COORDS" => Err(format!("cannot invert annotation: {name}")),
        _ => Err(format!("unknown gate for inverse: {name}")),
    }
}

/// Reverse the circuit and invert each gate. Fails if any instruction is non-invertible.
pub fn inverse(instrs: &[StimInstr]) -> Result<Vec<StimInstr>, String> {
    let mut out = Vec::with_capacity(instrs.len());
    for instr in instrs.iter().rev() {
        match instr {
            StimInstr::Op { name, tag, args, targets } => {
                let inv_name = invert_gate(name)?;
                out.push(StimInstr::Op {
                    name: inv_name,
                    tag: tag.clone(),
                    args: args.clone(),
                    targets: targets.clone(),
                });
            }
            StimInstr::Repeat { count, body } => {
                let inv_body = inverse(body)?;
                out.push(StimInstr::Repeat {
                    count: *count,
                    body: inv_body,
                });
            }
        }
    }
    Ok(out)
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
