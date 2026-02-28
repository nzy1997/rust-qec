use crate::ir::{StimInstr, StimTarget};

/// Generate a rotated surface code memory-X experiment circuit.
///
/// d=3: 9 data qubits + 8 ancilla = 17 total.
/// Logical observable: X on the left column of data qubits.
pub fn rotated_memory_x(distance: usize, rounds: usize, noise: f64) -> Vec<StimInstr> {
    rotated_surface_code(distance, rounds, noise, true)
}

/// Generate a rotated surface code memory-Z experiment circuit.
///
/// d=3: 9 data qubits + 8 ancilla = 17 total.
/// Logical observable: Z on the bottom row of data qubits.
pub fn rotated_memory_z(distance: usize, rounds: usize, noise: f64) -> Vec<StimInstr> {
    rotated_surface_code(distance, rounds, noise, false)
}

fn rotated_surface_code(d: usize, rounds: usize, noise: f64, is_memory_x: bool) -> Vec<StimInstr> {
    assert!(d >= 2, "distance must be >= 2");
    assert!(rounds >= 1, "rounds must be >= 1");

    // --- Build coordinate sets ---
    // Data qubits: x in {1,3,...,2d-1}, y in {1,3,...,2d-1}
    let mut data_coords: Vec<(i32, i32)> = Vec::new();
    let mut x_observable: Vec<(i32, i32)> = Vec::new(); // left column (x==1)
    let mut z_observable: Vec<(i32, i32)> = Vec::new(); // bottom row (y==1)
    for xi in 0..d {
        for yi in 0..d {
            let cx = (xi as i32) * 2 + 1;
            let cy = (yi as i32) * 2 + 1;
            data_coords.push((cx, cy));
            if xi == 0 {
                x_observable.push((cx, cy));
            }
            if yi == 0 {
                z_observable.push((cx, cy));
            }
        }
    }
    data_coords.sort();

    // Measurement qubits: x in {0,2,...,2d}, y in {0,2,...,2d} with filtering
    let mut x_measure_coords: Vec<(i32, i32)> = Vec::new();
    let mut z_measure_coords: Vec<(i32, i32)> = Vec::new();
    for xi in 0..=(d as i32) {
        for yi in 0..=(d as i32) {
            let cx = xi * 2;
            let cy = yi * 2;
            let on_boundary_1 = xi == 0 || xi == d as i32;
            let on_boundary_2 = yi == 0 || yi == d as i32;
            let parity = (xi % 2) != (yi % 2);
            if on_boundary_1 && parity {
                continue;
            }
            if on_boundary_2 && !parity {
                continue;
            }
            if parity {
                x_measure_coords.push((cx, cy));
            } else {
                z_measure_coords.push((cx, cy));
            }
        }
    }
    x_measure_coords.sort();
    z_measure_coords.sort();

    // All measurement qubits in sorted order (x first, then z)
    let mut measure_coords: Vec<(i32, i32)> = Vec::new();
    measure_coords.extend_from_slice(&x_measure_coords);
    measure_coords.extend_from_slice(&z_measure_coords);
    measure_coords.sort();

    // --- Assign qubit indices in sorted (x,y) order ---
    let mut all_coords: Vec<(i32, i32)> = Vec::new();
    all_coords.extend_from_slice(&data_coords);
    all_coords.extend_from_slice(&measure_coords);
    all_coords.sort();
    all_coords.dedup();

    let coord_to_idx = |c: (i32, i32)| -> u32 {
        all_coords.iter().position(|&x| x == c).unwrap() as u32
    };

    let data_qubits: Vec<u32> = data_coords.iter().map(|&c| coord_to_idx(c)).collect();
    let x_measure_qubits: Vec<u32> = x_measure_coords.iter().map(|&c| coord_to_idx(c)).collect();
    let _z_measure_qubits: Vec<u32> = z_measure_coords.iter().map(|&c| coord_to_idx(c)).collect();
    let measure_qubits: Vec<u32> = measure_coords.iter().map(|&c| coord_to_idx(c)).collect();

    // Chosen basis
    let chosen_measure_coords: &Vec<(i32, i32)> = if is_memory_x { &x_measure_coords } else { &z_measure_coords };
    let chosen_observable: &Vec<(i32, i32)> = if is_memory_x { &x_observable } else { &z_observable };

    // Interaction orders (from Stim source)
    // Z-ancilla: [(1,1),(1,-1),(-1,1),(-1,-1)]
    // X-ancilla: [(1,1),(-1,1),(1,-1),(-1,-1)]
    let z_order: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
    let x_order: [(i32, i32); 4] = [(1, 1), (-1, 1), (1, -1), (-1, -1)];

    // Build CNOT target lists for each of 4 layers
    // X: ancilla -> data (measure is control, data is target)
    // Z: data -> ancilla (data is control, measure is target)
    let mut cnot_layers: [Vec<u32>; 4] = [vec![], vec![], vec![], vec![]];
    for k in 0..4 {
        for &mc in &x_measure_coords {
            let dc = (mc.0 + x_order[k].0, mc.1 + x_order[k].1);
            if data_coords.contains(&dc) {
                cnot_layers[k].push(coord_to_idx(mc));
                cnot_layers[k].push(coord_to_idx(dc));
            }
        }
        for &mc in &z_measure_coords {
            let dc = (mc.0 + z_order[k].0, mc.1 + z_order[k].1);
            if data_coords.contains(&dc) {
                cnot_layers[k].push(coord_to_idx(dc));
                cnot_layers[k].push(coord_to_idx(mc));
            }
        }
    }

    let mut instrs: Vec<StimInstr> = Vec::new();

    // QUBIT_COORDS
    for &(cx, cy) in &all_coords {
        let q = coord_to_idx((cx, cy));
        instrs.push(op("QUBIT_COORDS", &[cx as f64, cy as f64], &[StimTarget::Qubit(q)]));
    }

    // Reset data qubits in chosen basis
    let data_reset_op = if is_memory_x { "RX" } else { "R" };
    for &q in &data_qubits {
        instrs.push(op(data_reset_op, &[], &[StimTarget::Qubit(q)]));
    }
    if noise > 0.0 {
        for &q in &data_qubits {
            instrs.push(op("DEPOLARIZE1", &[noise], &[StimTarget::Qubit(q)]));
        }
    }

    // Reset all ancilla
    for &q in &measure_qubits {
        instrs.push(op("R", &[], &[StimTarget::Qubit(q)]));
    }

    // measure_coord_to_order: position in measure_qubits list
    let measure_coord_to_order = |c: (i32, i32)| -> usize {
        measure_coords.iter().position(|&x| x == c).unwrap()
    };
    let data_coord_to_order = |c: (i32, i32)| -> usize {
        data_coords.iter().position(|&x| x == c).unwrap()
    };

    let n_measure = measure_qubits.len();
    let n_data = data_qubits.len();

    // First round (head cycle)
    instrs.push(op("TICK", &[], &[]));
    // H on X ancilla
    for &q in &x_measure_qubits {
        instrs.push(op("H", &[], &[StimTarget::Qubit(q)]));
    }
    // 4 CNOT layers
    for k in 0..4 {
        instrs.push(op("TICK", &[], &[]));
        let targets: Vec<StimTarget> = cnot_layers[k].iter().map(|&q| StimTarget::Qubit(q)).collect();
        if !targets.is_empty() {
            instrs.push(op("CX", &[], &targets));
        }
        if noise > 0.0 && !cnot_layers[k].is_empty() {
            let pairs: Vec<StimTarget> = cnot_layers[k].iter().map(|&q| StimTarget::Qubit(q)).collect();
            instrs.push(op("DEPOLARIZE2", &[noise], &pairs));
        }
    }
    instrs.push(op("TICK", &[], &[]));
    // H on X ancilla
    for &q in &x_measure_qubits {
        instrs.push(op("H", &[], &[StimTarget::Qubit(q)]));
    }
    instrs.push(op("TICK", &[], &[]));
    // MR all ancilla
    for &q in &measure_qubits {
        instrs.push(op("MR", &[], &[StimTarget::Qubit(q)]));
    }
    // Detectors for first round: only chosen-basis ancilla
    for &mc in chosen_measure_coords.iter() {
        let order = measure_coord_to_order(mc);
        let rec_offset = -((n_measure - order) as i32);
        instrs.push(op(
            "DETECTOR",
            &[mc.0 as f64, mc.1 as f64, 0.0],
            &[StimTarget::Rec(rec_offset)],
        ));
    }

    // Subsequent rounds (body cycle, rounds-1 times)
    for _round in 1..rounds {
        instrs.push(op("SHIFT_COORDS", &[0.0, 0.0, 1.0], &[]));
        instrs.push(op("TICK", &[], &[]));
        for &q in &x_measure_qubits {
            instrs.push(op("H", &[], &[StimTarget::Qubit(q)]));
        }
        for k in 0..4 {
            instrs.push(op("TICK", &[], &[]));
            let targets: Vec<StimTarget> = cnot_layers[k].iter().map(|&q| StimTarget::Qubit(q)).collect();
            if !targets.is_empty() {
                instrs.push(op("CX", &[], &targets));
            }
            if noise > 0.0 && !cnot_layers[k].is_empty() {
                let pairs: Vec<StimTarget> = cnot_layers[k].iter().map(|&q| StimTarget::Qubit(q)).collect();
                instrs.push(op("DEPOLARIZE2", &[noise], &pairs));
            }
        }
        instrs.push(op("TICK", &[], &[]));
        for &q in &x_measure_qubits {
            instrs.push(op("H", &[], &[StimTarget::Qubit(q)]));
        }
        instrs.push(op("TICK", &[], &[]));
        for &q in &measure_qubits {
            instrs.push(op("MR", &[], &[StimTarget::Qubit(q)]));
        }
        // Detectors: all ancilla, compare current to previous
        for &mc in &measure_coords {
            let order = measure_coord_to_order(mc);
            let k = (n_measure - order - 1) as i32;
            instrs.push(op(
                "DETECTOR",
                &[mc.0 as f64, mc.1 as f64, 0.0],
                &[StimTarget::Rec(-(k + 1)), StimTarget::Rec(-(k + 1 + n_measure as i32))],
            ));
        }
    }

    // Tail: measure data qubits in chosen basis
    instrs.push(op("TICK", &[], &[]));
    if noise > 0.0 {
        for &q in &data_qubits {
            instrs.push(op("DEPOLARIZE1", &[noise], &[StimTarget::Qubit(q)]));
        }
    }
    let data_meas_op = if is_memory_x { "MX" } else { "M" };
    for &q in &data_qubits {
        instrs.push(op(data_meas_op, &[], &[StimTarget::Qubit(q)]));
    }

    // Tail detectors: for each chosen-basis ancilla, compare last ancilla meas + neighboring data
    for &mc in chosen_measure_coords.iter() {
        let mut det_targets: Vec<StimTarget> = Vec::new();
        // Neighboring data qubits (using z_order offsets per Stim)
        for &delta in &z_order {
            let dc = (mc.0 + delta.0, mc.1 + delta.1);
            if let Some(dorder) = data_coords.iter().position(|&x| x == dc) {
                let offset = -((n_data - dorder) as i32);
                det_targets.push(StimTarget::Rec(offset));
            }
        }
        // Last ancilla measurement
        let morder = measure_coord_to_order(mc);
        let anc_offset = -((n_data + n_measure - morder) as i32);
        det_targets.push(StimTarget::Rec(anc_offset));
        det_targets.sort_by_key(|t| match t { StimTarget::Rec(r) => *r, _ => 0 });
        instrs.push(op(
            "DETECTOR",
            &[mc.0 as f64, mc.1 as f64, 1.0],
            &det_targets,
        ));
    }

    // Observable
    let obs_targets: Vec<StimTarget> = {
        let mut v: Vec<StimTarget> = chosen_observable.iter().map(|&c| {
            let dorder = data_coord_to_order(c);
            StimTarget::Rec(-((n_data - dorder) as i32))
        }).collect();
        v.sort_by_key(|t| match t { StimTarget::Rec(r) => *r, _ => 0 });
        v
    };
    instrs.push(op("OBSERVABLE_INCLUDE", &[0.0], &obs_targets));

    instrs
}

fn op(name: &str, args: &[f64], targets: &[StimTarget]) -> StimInstr {
    StimInstr::Op {
        name: name.to_string(),
        tag: None,
        args: args.to_vec(),
        targets: targets.to_vec(),
    }
}

/// Generate an unrotated surface code memory-X experiment circuit.
pub fn unrotated_memory_x(distance: usize, rounds: usize, noise: f64) -> Vec<StimInstr> {
    unrotated_surface_code(distance, rounds, noise, true)
}

/// Generate an unrotated surface code memory-Z experiment circuit.
pub fn unrotated_memory_z(distance: usize, rounds: usize, noise: f64) -> Vec<StimInstr> {
    unrotated_surface_code(distance, rounds, noise, false)
}

fn unrotated_surface_code(d: usize, rounds: usize, noise: f64, is_memory_x: bool) -> Vec<StimInstr> {
    assert!(d >= 2, "distance must be >= 2");
    assert!(rounds >= 1, "rounds must be >= 1");

    let width = 2 * d - 1;

    // Classify each grid position
    let mut data_coords: Vec<(i32, i32)> = Vec::new();
    let mut x_measure_coords: Vec<(i32, i32)> = Vec::new();
    let mut z_measure_coords: Vec<(i32, i32)> = Vec::new();
    let mut x_observable: Vec<(i32, i32)> = Vec::new();
    let mut z_observable: Vec<(i32, i32)> = Vec::new();

    for y in 0..width {
        for x in 0..width {
            let xi = x as i32;
            let yi = y as i32;
            let parity = (x % 2) != (y % 2);
            if parity {
                if x % 2 == 0 {
                    z_measure_coords.push((xi, yi));
                } else {
                    x_measure_coords.push((xi, yi));
                }
            } else {
                data_coords.push((xi, yi));
                if x == 0 {
                    x_observable.push((xi, yi));
                }
                if y == 0 {
                    z_observable.push((xi, yi));
                }
            }
        }
    }

    // Qubit index: x + y * width (row-major)
    let coord_to_idx = |c: (i32, i32)| -> u32 { (c.0 + c.1 * width as i32) as u32 };

    let data_qubits: Vec<u32> = data_coords.iter().map(|&c| coord_to_idx(c)).collect();
    let x_measure_qubits: Vec<u32> = x_measure_coords.iter().map(|&c| coord_to_idx(c)).collect();
    let _z_measure_qubits: Vec<u32> = z_measure_coords.iter().map(|&c| coord_to_idx(c)).collect();

    let mut measure_coords: Vec<(i32, i32)> = Vec::new();
    measure_coords.extend_from_slice(&x_measure_coords);
    measure_coords.extend_from_slice(&z_measure_coords);
    measure_coords.sort();

    let measure_qubits: Vec<u32> = measure_coords.iter().map(|&c| coord_to_idx(c)).collect();

    // All coords for QUBIT_COORDS
    let mut all_coords: Vec<(i32, i32)> = Vec::new();
    all_coords.extend_from_slice(&data_coords);
    all_coords.extend_from_slice(&x_measure_coords);
    all_coords.extend_from_slice(&z_measure_coords);
    all_coords.sort();
    all_coords.dedup();

    let chosen_measure_coords: &Vec<(i32, i32)> = if is_memory_x { &x_measure_coords } else { &z_measure_coords };
    let chosen_observable: &Vec<(i32, i32)> = if is_memory_x { &x_observable } else { &z_observable };

    // Interaction order (same for X and Z ancilla in unrotated code)
    let interact_order: [(i32, i32); 4] = [(1, 0), (0, 1), (0, -1), (-1, 0)];

    // Build CNOT layers
    // X-ancilla: ancilla -> data (ancilla is control, data is target)
    // Z-ancilla: data -> ancilla (data is control, ancilla is target)
    let mut cnot_layers: [Vec<u32>; 4] = [vec![], vec![], vec![], vec![]];
    for k in 0..4 {
        for &mc in &x_measure_coords {
            let dc = (mc.0 + interact_order[k].0, mc.1 + interact_order[k].1);
            if data_coords.contains(&dc) {
                cnot_layers[k].push(coord_to_idx(mc));
                cnot_layers[k].push(coord_to_idx(dc));
            }
        }
        for &mc in &z_measure_coords {
            let dc = (mc.0 + interact_order[k].0, mc.1 + interact_order[k].1);
            if data_coords.contains(&dc) {
                cnot_layers[k].push(coord_to_idx(dc));
                cnot_layers[k].push(coord_to_idx(mc));
            }
        }
    }

    let measure_coord_to_order = |c: (i32, i32)| -> usize {
        measure_coords.iter().position(|&x| x == c).unwrap()
    };
    let data_coord_to_order = |c: (i32, i32)| -> usize {
        data_coords.iter().position(|&x| x == c).unwrap()
    };

    let n_measure = measure_qubits.len();
    let n_data = data_qubits.len();

    let mut instrs: Vec<StimInstr> = Vec::new();

    // QUBIT_COORDS
    for &(cx, cy) in &all_coords {
        let q = coord_to_idx((cx, cy));
        instrs.push(op("QUBIT_COORDS", &[cx as f64, cy as f64], &[StimTarget::Qubit(q)]));
    }

    // Reset data in chosen basis
    let data_reset_op = if is_memory_x { "RX" } else { "R" };
    for &q in &data_qubits {
        instrs.push(op(data_reset_op, &[], &[StimTarget::Qubit(q)]));
    }
    if noise > 0.0 {
        for &q in &data_qubits {
            instrs.push(op("DEPOLARIZE1", &[noise], &[StimTarget::Qubit(q)]));
        }
    }

    // Reset all ancilla
    for &q in &measure_qubits {
        instrs.push(op("R", &[], &[StimTarget::Qubit(q)]));
    }

    // First round
    instrs.push(op("TICK", &[], &[]));
    for &q in &x_measure_qubits {
        instrs.push(op("H", &[], &[StimTarget::Qubit(q)]));
    }
    for k in 0..4 {
        instrs.push(op("TICK", &[], &[]));
        let targets: Vec<StimTarget> = cnot_layers[k].iter().map(|&q| StimTarget::Qubit(q)).collect();
        if !targets.is_empty() {
            instrs.push(op("CX", &[], &targets));
        }
        if noise > 0.0 && !cnot_layers[k].is_empty() {
            let pairs: Vec<StimTarget> = cnot_layers[k].iter().map(|&q| StimTarget::Qubit(q)).collect();
            instrs.push(op("DEPOLARIZE2", &[noise], &pairs));
        }
    }
    instrs.push(op("TICK", &[], &[]));
    for &q in &x_measure_qubits {
        instrs.push(op("H", &[], &[StimTarget::Qubit(q)]));
    }
    instrs.push(op("TICK", &[], &[]));
    for &q in &measure_qubits {
        instrs.push(op("MR", &[], &[StimTarget::Qubit(q)]));
    }
    // First-round detectors: only chosen-basis ancilla
    for &mc in chosen_measure_coords.iter() {
        let order = measure_coord_to_order(mc);
        let rec_offset = -((n_measure - order) as i32);
        instrs.push(op(
            "DETECTOR",
            &[mc.0 as f64, mc.1 as f64, 0.0],
            &[StimTarget::Rec(rec_offset)],
        ));
    }

    // Subsequent rounds
    for _round in 1..rounds {
        instrs.push(op("SHIFT_COORDS", &[0.0, 0.0, 1.0], &[]));
        instrs.push(op("TICK", &[], &[]));
        for &q in &x_measure_qubits {
            instrs.push(op("H", &[], &[StimTarget::Qubit(q)]));
        }
        for k in 0..4 {
            instrs.push(op("TICK", &[], &[]));
            let targets: Vec<StimTarget> = cnot_layers[k].iter().map(|&q| StimTarget::Qubit(q)).collect();
            if !targets.is_empty() {
                instrs.push(op("CX", &[], &targets));
            }
            if noise > 0.0 && !cnot_layers[k].is_empty() {
                let pairs: Vec<StimTarget> = cnot_layers[k].iter().map(|&q| StimTarget::Qubit(q)).collect();
                instrs.push(op("DEPOLARIZE2", &[noise], &pairs));
            }
        }
        instrs.push(op("TICK", &[], &[]));
        for &q in &x_measure_qubits {
            instrs.push(op("H", &[], &[StimTarget::Qubit(q)]));
        }
        instrs.push(op("TICK", &[], &[]));
        for &q in &measure_qubits {
            instrs.push(op("MR", &[], &[StimTarget::Qubit(q)]));
        }
        for &mc in &measure_coords {
            let order = measure_coord_to_order(mc);
            let k = (n_measure - order - 1) as i32;
            instrs.push(op(
                "DETECTOR",
                &[mc.0 as f64, mc.1 as f64, 0.0],
                &[StimTarget::Rec(-(k + 1)), StimTarget::Rec(-(k + 1 + n_measure as i32))],
            ));
        }
    }

    // Tail: measure data in chosen basis
    instrs.push(op("TICK", &[], &[]));
    if noise > 0.0 {
        for &q in &data_qubits {
            instrs.push(op("DEPOLARIZE1", &[noise], &[StimTarget::Qubit(q)]));
        }
    }
    let data_meas_op = if is_memory_x { "MX" } else { "M" };
    for &q in &data_qubits {
        instrs.push(op(data_meas_op, &[], &[StimTarget::Qubit(q)]));
    }

    // Tail detectors
    for &mc in chosen_measure_coords.iter() {
        let mut det_targets: Vec<StimTarget> = Vec::new();
        for &delta in &interact_order {
            let dc = (mc.0 + delta.0, mc.1 + delta.1);
            if let Some(dorder) = data_coords.iter().position(|&x| x == dc) {
                let offset = -((n_data - dorder) as i32);
                det_targets.push(StimTarget::Rec(offset));
            }
        }
        let morder = measure_coord_to_order(mc);
        let anc_offset = -((n_data + n_measure - morder) as i32);
        det_targets.push(StimTarget::Rec(anc_offset));
        det_targets.sort_by_key(|t| match t { StimTarget::Rec(r) => *r, _ => 0 });
        instrs.push(op(
            "DETECTOR",
            &[mc.0 as f64, mc.1 as f64, 1.0],
            &det_targets,
        ));
    }

    // Observable
    let obs_targets: Vec<StimTarget> = {
        let mut v: Vec<StimTarget> = chosen_observable.iter().map(|&c| {
            let dorder = data_coord_to_order(c);
            StimTarget::Rec(-((n_data - dorder) as i32))
        }).collect();
        v.sort_by_key(|t| match t { StimTarget::Rec(r) => *r, _ => 0 });
        v
    };
    instrs.push(op("OBSERVABLE_INCLUDE", &[0.0], &obs_targets));

    instrs
}

