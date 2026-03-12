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

#[derive(Debug, Clone, PartialEq)]
pub struct FeedbacklessM2dNormalization {
    pub circuit: Vec<StimInstr>,
    pub measurement_corrections: Vec<Vec<usize>>,
}

pub fn normalize_feedbackless_m2d(
    instrs: &[StimInstr],
) -> Result<FeedbacklessM2dNormalization, String> {
    let flat = flattened(instrs);
    let num_qubits = crate::stats::num_qubits(&flat);
    let mut pending_x: Vec<Vec<usize>> = vec![Vec::new(); num_qubits];
    let mut pending_z: Vec<Vec<usize>> = vec![Vec::new(); num_qubits];
    let mut circuit = Vec::with_capacity(flat.len());
    let mut measurement_corrections = Vec::new();
    let mut measurement_count = 0usize;

    for instr in flat {
        match &instr {
            StimInstr::Op { name, targets, .. } if is_feedback_operation(name, targets) => {
                accumulate_feedback(
                    name,
                    targets,
                    measurement_count,
                    &mut pending_x,
                    &mut pending_z,
                )?;
            }
            StimInstr::Op { name, .. } if is_annotation(name) => {
                circuit.push(instr);
            }
            StimInstr::Op { name, targets, .. } if is_single_qubit_measurement(name) => {
                for target in targets {
                    measurement_corrections.push(take_measurement_correction(
                        name,
                        target,
                        &mut pending_x,
                        &mut pending_z,
                    ));
                    measurement_count += 1;
                }
                circuit.push(instr);
            }
            StimInstr::Op { name, targets, .. } => {
                if touches_pending_qubits(targets, &pending_x, &pending_z) {
                    return Err(format!("unsupported feedback before {name}"));
                }
                measurement_count += crate::stats::num_measurements(std::slice::from_ref(&instr));
                if measurement_count > measurement_corrections.len() {
                    measurement_corrections.resize(measurement_count, Vec::new());
                }
                circuit.push(instr);
            }
            StimInstr::Repeat { .. } => unreachable!("flattened removed repeats"),
        }
    }

    for q in 0..num_qubits {
        if !pending_x[q].is_empty() || !pending_z[q].is_empty() {
            return Err(format!("unsupported feedback left pending on qubit {q}"));
        }
    }

    Ok(FeedbacklessM2dNormalization {
        circuit,
        measurement_corrections,
    })
}

fn is_feedback_operation(name: &str, targets: &[crate::ir::StimTarget]) -> bool {
    matches!(name, "CX" | "CNOT" | "ZCX" | "CY" | "ZCY" | "CZ" | "ZCZ")
        && matches!(
            targets,
            [crate::ir::StimTarget::Rec(_), crate::ir::StimTarget::Qubit(_)]
        )
}

fn is_annotation(name: &str) -> bool {
    matches!(name, "DETECTOR" | "OBSERVABLE_INCLUDE" | "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS")
}

fn is_single_qubit_measurement(name: &str) -> bool {
    matches!(name, "M" | "MZ" | "MR" | "MRZ" | "MX" | "MRX" | "MY" | "MRY")
}

fn accumulate_feedback(
    name: &str,
    targets: &[crate::ir::StimTarget],
    measurement_count: usize,
    pending_x: &mut [Vec<usize>],
    pending_z: &mut [Vec<usize>],
) -> Result<(), String> {
    if targets.len() != 2 {
        return Err(format!("unsupported feedback form for {name}"));
    }
    let (offset, qubit) = match (&targets[0], &targets[1]) {
        (crate::ir::StimTarget::Rec(offset), crate::ir::StimTarget::Qubit(qubit)) => (*offset, *qubit as usize),
        _ => return Err(format!("unsupported feedback form for {name}")),
    };
    let measurement = resolve_rec_measurement_index(measurement_count, offset)?;
    match name {
        "CX" | "CNOT" | "ZCX" => pending_x[qubit].push(measurement),
        "CZ" | "ZCZ" => pending_z[qubit].push(measurement),
        "CY" | "ZCY" => {
            pending_x[qubit].push(measurement);
            pending_z[qubit].push(measurement);
        }
        _ => return Err(format!("unsupported feedback form for {name}")),
    }
    Ok(())
}

fn resolve_rec_measurement_index(measurement_count: usize, offset: i32) -> Result<usize, String> {
    let abs = measurement_count as i64 + offset as i64;
    if abs < 0 || abs >= measurement_count as i64 {
        return Err(format!("unsupported feedback rec[{offset}]"));
    }
    Ok(abs as usize)
}

fn take_measurement_correction(
    name: &str,
    target: &crate::ir::StimTarget,
    pending_x: &mut [Vec<usize>],
    pending_z: &mut [Vec<usize>],
) -> Vec<usize> {
    let qubit = match target {
        crate::ir::StimTarget::Qubit(qubit) | crate::ir::StimTarget::QubitInv(qubit) => *qubit as usize,
        _ => return Vec::new(),
    };
    let x = std::mem::take(&mut pending_x[qubit]);
    let z = std::mem::take(&mut pending_z[qubit]);
    match name {
        "M" | "MZ" | "MR" | "MRZ" => x,
        "MX" | "MRX" => z,
        "MY" | "MRY" => {
            let mut corrections = x;
            corrections.extend(z);
            corrections
        }
        _ => Vec::new(),
    }
}

fn touches_pending_qubits(
    targets: &[crate::ir::StimTarget],
    pending_x: &[Vec<usize>],
    pending_z: &[Vec<usize>],
) -> bool {
    targets.iter().filter_map(|target| target.qubit_index()).any(|q| {
        let q = q as usize;
        !pending_x[q].is_empty() || !pending_z[q].is_empty()
    })
}
