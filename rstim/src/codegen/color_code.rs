use crate::ir::{StimInstr, StimTarget};
use super::NoiseParams;

/// Generate a color code memory_xyz experiment circuit (uniform noise convenience wrapper).
pub fn memory_xyz(distance: usize, rounds: usize, noise: f64) -> Vec<StimInstr> {
    memory_xyz_with_params(distance, rounds, NoiseParams::uniform(noise))
}

/// Generate a color code memory_xyz experiment circuit with per-channel noise.
///
/// d must be odd and >= 3. rounds must be >= 2.
/// Layout: triangular grid with w = d + (d-1)/2 rows.
pub fn memory_xyz_with_params(distance: usize, rounds: usize, params: NoiseParams) -> Vec<StimInstr> {
    assert!(distance >= 3 && distance % 2 == 1, "distance must be odd and >= 3");
    assert!(rounds >= 2, "rounds must be >= 2");

    let d = distance;
    let w = d + (d - 1) / 2;

    let mut p2q: std::collections::HashMap<(i64, i64), u32> = std::collections::HashMap::new();
    let mut q2p: Vec<(f64, f64)> = Vec::new();
    let mut data_coords: Vec<(f64, f64)> = Vec::new();
    let mut measure_coords: Vec<(f64, f64)> = Vec::new();
    let mut data_qubits: Vec<u32> = Vec::new();
    let mut measurement_qubits: Vec<u32> = Vec::new();

    for y in 0..w {
        for x in 0..(w - y) {
            let key = (x as i64 * 2 + y as i64, y as i64);
            let idx = q2p.len() as u32;
            p2q.insert(key, idx);
            let coord = (x as f64 + y as f64 / 2.0, y as f64);
            q2p.push(coord);
            if (x + 2 * y) % 3 == 2 {
                measure_coords.push(coord);
                measurement_qubits.push(idx);
            } else {
                data_coords.push(coord);
                data_qubits.push(idx);
            }
        }
    }

    let mut all_qubits: Vec<u32> = data_qubits.iter().chain(measurement_qubits.iter()).cloned().collect();
    all_qubits.sort();
    data_qubits.sort();
    measurement_qubits.sort();

    let coord_key = |c: (f64, f64)| -> (i64, i64) {
        let y_int = c.1 as i64;
        let x2_int = (c.0 * 2.0).round() as i64;
        (x2_int - y_int, y_int)
    };

    let data_coord_to_order: std::collections::HashMap<(i64, i64), usize> = {
        let mut m = std::collections::HashMap::new();
        for (i, &q) in data_qubits.iter().enumerate() {
            let c = q2p[q as usize];
            m.insert(coord_key(c), i);
        }
        m
    };
    let measure_coord_to_order: std::collections::HashMap<(i64, i64), usize> = {
        let mut m = std::collections::HashMap::new();
        for (i, &q) in measurement_qubits.iter().enumerate() {
            let c = q2p[q as usize];
            m.insert(coord_key(c), i);
        }
        m
    };

    let deltas: [(i64, i64); 6] = [
        (2, 0),
        (1, 1),
        (1, -1),
        (-2, 0),
        (-1, 1),
        (-1, -1),
    ];

    let mut cnot_targets: [Vec<u32>; 6] = Default::default();
    for (k, &(ddx2, ddy)) in deltas.iter().enumerate() {
        for &mq in &measurement_qubits {
            let mc = q2p[mq as usize];
            let mk = coord_key(mc);
            let dk = (mk.0 + ddx2, mk.1 + ddy);
            if let Some(&dq) = p2q.get(&dk) {
                cnot_targets[k].push(dq);
                cnot_targets[k].push(mq);
            }
        }
    }

    let m = measurement_qubits.len();
    let nd = data_qubits.len();

    let mut instrs: Vec<StimInstr> = Vec::new();

    // QUBIT_COORDS
    for &q in &all_qubits {
        let (cx, cy) = q2p[q as usize];
        instrs.push(op("QUBIT_COORDS", &[cx, cy], &[StimTarget::Qubit(q)]));
    }

    // Reset all qubits
    for &q in &all_qubits {
        instrs.push(op("R", &[], &[StimTarget::Qubit(q)]));
    }
    if params.after_reset_flip_probability > 0.0 {
        for &q in &all_qubits {
            instrs.push(op("X_ERROR", &[params.after_reset_flip_probability], &[StimTarget::Qubit(q)]));
        }
    }

    // Head: 2 cycles
    for _ in 0..2 {
        emit_cycle(&mut instrs, &data_qubits, &measurement_qubits, &cnot_targets, &params);
    }

    // Detectors after head
    for k in (0..m).rev() {
        let mq = measurement_qubits[m - k - 1];
        let (cx, cy) = q2p[mq as usize];
        let r1 = -((k + 1) as i32);
        let r2 = -((k + 1 + m) as i32);
        instrs.push(op("DETECTOR", &[cx, cy, 0.0], &[StimTarget::Rec(r1), StimTarget::Rec(r2)]));
    }

    // Body: (rounds-2) repetitions
    for _ in 0..(rounds - 2) {
        emit_cycle(&mut instrs, &data_qubits, &measurement_qubits, &cnot_targets, &params);
        instrs.push(op("SHIFT_COORDS", &[0.0, 0.0, 1.0], &[]));
        for k in (0..m).rev() {
            let mq = measurement_qubits[m - k - 1];
            let (cx, cy) = q2p[mq as usize];
            let r1 = -((k + 1) as i32);
            let r2 = -((k + 1 + m) as i32);
            let r3 = -((k + 1 + 2 * m) as i32);
            instrs.push(op("DETECTOR", &[cx, cy, 0.0], &[
                StimTarget::Rec(r1),
                StimTarget::Rec(r2),
                StimTarget::Rec(r3),
            ]));
        }
    }

    // Tail: measure data
    let meas_basis = ["Z", "X", "Y"][rounds % 3];
    let meas_op = match meas_basis {
        "Z" => "M",
        "X" => "MX",
        "Y" => "MY",
        _ => unreachable!(),
    };
    if params.before_round_data_depolarization > 0.0 {
        for &q in &data_qubits {
            instrs.push(op("DEPOLARIZE1", &[params.before_round_data_depolarization], &[StimTarget::Qubit(q)]));
        }
    }
    if params.before_measure_flip_probability > 0.0 {
        for &q in &data_qubits {
            instrs.push(op("X_ERROR", &[params.before_measure_flip_probability], &[StimTarget::Qubit(q)]));
        }
    }
    for &q in &data_qubits {
        instrs.push(op(meas_op, &[], &[StimTarget::Qubit(q)]));
    }

    // Tail detectors
    for &mq in &measurement_qubits {
        let mc = q2p[mq as usize];
        let mk = coord_key(mc);
        let morder = measure_coord_to_order[&mk];

        let mut det_targets: Vec<u32> = Vec::new();
        for &(ddx2, ddy) in &deltas {
            let dk = (mk.0 + ddx2, mk.1 + ddy);
            if let Some(&dq) = p2q.get(&dk) {
                let dc = q2p[dq as usize];
                let dck = coord_key(dc);
                if let Some(&dorder) = data_coord_to_order.get(&dck) {
                    det_targets.push((nd - dorder) as u32);
                }
            }
        }

        let p = (nd + m - morder) as u32;
        match rounds % 3 {
            0 => det_targets.push(p),
            1 => det_targets.push(p + m as u32),
            2 => {
                det_targets.push(p);
                det_targets.push(p + m as u32);
            }
            _ => unreachable!(),
        }

        det_targets.sort();
        let targets: Vec<StimTarget> = det_targets.iter().map(|&r| StimTarget::Rec(-(r as i32))).collect();
        instrs.push(op("DETECTOR", &[mc.0, mc.1, 1.0], &targets));
    }

    // Observable
    let mut obs_targets: Vec<u32> = Vec::new();
    for &q in &data_qubits {
        let (_, cy) = q2p[q as usize];
        if cy == 0.0 {
            let ck = coord_key(q2p[q as usize]);
            let dorder = data_coord_to_order[&ck];
            obs_targets.push((nd - dorder) as u32);
        }
    }
    obs_targets.sort();
    let obs: Vec<StimTarget> = obs_targets.iter().map(|&r| StimTarget::Rec(-(r as i32))).collect();
    instrs.push(op("OBSERVABLE_INCLUDE", &[0.0], &obs));

    instrs
}

/// Emit one color code cycle with per-channel noise.
fn emit_cycle(
    instrs: &mut Vec<StimInstr>,
    data_qubits: &[u32],
    measurement_qubits: &[u32],
    cnot_targets: &[Vec<u32>; 6],
    params: &NoiseParams,
) {
    instrs.push(op("TICK", &[], &[]));

    // before_round_data_depolarization on measurement qubits (ancilla idle noise)
    if params.before_round_data_depolarization > 0.0 {
        for &q in measurement_qubits {
            instrs.push(op("DEPOLARIZE1", &[params.before_round_data_depolarization], &[StimTarget::Qubit(q)]));
        }
    }

    // C_XYZ on data
    for &q in data_qubits {
        instrs.push(op("C_XYZ", &[], &[StimTarget::Qubit(q)]));
    }

    // after_clifford_depolarization on data after C_XYZ
    if params.after_clifford_depolarization > 0.0 {
        for &q in data_qubits {
            instrs.push(op("DEPOLARIZE1", &[params.after_clifford_depolarization], &[StimTarget::Qubit(q)]));
        }
    }

    // 6 CNOT layers
    for targets in cnot_targets.iter() {
        instrs.push(op("TICK", &[], &[]));
        if !targets.is_empty() {
            let t: Vec<StimTarget> = targets.iter().map(|&q| StimTarget::Qubit(q)).collect();
            instrs.push(op("CX", &[], &t));
            if params.after_clifford_depolarization > 0.0 {
                instrs.push(op("DEPOLARIZE2", &[params.after_clifford_depolarization], &t));
            }
        }
    }

    // TICK + MR ancilla
    instrs.push(op("TICK", &[], &[]));

    // before_measure_flip_probability: X_ERROR before measurement
    if params.before_measure_flip_probability > 0.0 {
        for &q in measurement_qubits {
            instrs.push(op("X_ERROR", &[params.before_measure_flip_probability], &[StimTarget::Qubit(q)]));
        }
    }

    for &q in measurement_qubits {
        instrs.push(op("MR", &[], &[StimTarget::Qubit(q)]));
    }

    // after_reset_flip_probability: X_ERROR after MR (which includes reset)
    if params.after_reset_flip_probability > 0.0 {
        for &q in measurement_qubits {
            instrs.push(op("X_ERROR", &[params.after_reset_flip_probability], &[StimTarget::Qubit(q)]));
        }
    }
}

fn op(name: &str, args: &[f64], targets: &[StimTarget]) -> StimInstr {
    StimInstr::Op {
        name: name.to_string(),
        tag: None,
        args: args.to_vec(),
        targets: targets.to_vec(),
    }
}
