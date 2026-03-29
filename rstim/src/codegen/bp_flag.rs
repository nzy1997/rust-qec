use crate::ir::{StimInstr, StimTarget};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalBasis {
    X,
    Z,
}

#[derive(Debug, Clone)]
struct SurfaceCheck {
    check_type: SurfaceCheckType,
    pos: (i32, i32),
    idx: u32,
    data_qubits: [Option<u32>; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SurfaceCheckType {
    X,
    Z,
}

const STEANE_Z_STABS: [[u32; 4]; 3] = [[0, 1, 2, 4], [0, 2, 3, 5], [1, 2, 3, 6]];
const STEANE_X_STABS: [[u32; 4]; 3] = STEANE_Z_STABS;

pub fn create_surface_code_circuit(
    distance: usize,
    error_rate: f64,
    rounds: usize,
    if_measure: bool,
    use_flags: bool,
    p_anc_mid: f64,
) -> Vec<StimInstr> {
    let (data_dict, data_list, data_positions) = generate_data_dict_and_list(distance, distance, 0);
    let check_list = generate_check_list(distance, distance, &data_dict, 0, 0);
    let flag_map = if use_flags {
        build_flag_map(&data_list, &check_list)
    } else {
        BTreeMap::new()
    };

    let mut circuit =
        initialize_surface_code_positions(&data_list, &data_positions, &check_list, &flag_map);
    initialize_surface_code_qubits(&mut circuit, &data_list, &check_list, &flag_map);

    for round in 0..rounds {
        syndrome_measurement(
            &mut circuit,
            &check_list,
            error_rate,
            round as i32,
            &flag_map,
            p_anc_mid,
        );
    }

    if if_measure {
        final_surface_z_measurement(
            &mut circuit,
            &data_list,
            &check_list,
            distance,
            distance,
            error_rate,
            rounds as i32,
            &flag_map,
        );
    }

    circuit
}

pub fn build_steane_syndrome_circuit(
    p_data: f64,
    p_gate: f64,
    p_meas: f64,
    p_reset: f64,
    rounds: usize,
    logical_basis: LogicalBasis,
    use_flags: bool,
    p_anc_mid: f64,
) -> Vec<StimInstr> {
    let n_data = 7u32;
    let anc_z = vec![7, 8, 9];
    let anc_x = vec![10, 11, 12];
    let flag_z = if use_flags { vec![13, 14, 15] } else { vec![] };
    let flag_x = if use_flags { vec![16, 17, 18] } else { vec![] };

    append_steane_syndrome_rounds(
        n_data,
        &anc_z,
        &anc_x,
        &flag_z,
        &flag_x,
        rounds,
        p_data,
        p_gate,
        p_meas,
        p_reset,
        p_anc_mid,
        logical_basis,
    )
}

fn generate_data_dict_and_list(
    m: usize,
    n: usize,
    offset: u32,
) -> (HashMap<(i32, i32), u32>, Vec<u32>, Vec<(i32, i32)>) {
    let mut data_dict = HashMap::new();
    let mut data_list = Vec::new();
    let mut data_positions = Vec::new();
    let mut idx = offset;
    for i in 0..m {
        for j in 0..n {
            let pos = ((2 * i) as i32, (2 * j) as i32);
            data_dict.insert(pos, idx);
            data_list.push(idx);
            data_positions.push(pos);
            idx += 1;
        }
    }
    (data_dict, data_list, data_positions)
}

fn generate_check_list(
    m: usize,
    n: usize,
    data_dict: &HashMap<(i32, i32), u32>,
    offset: u32,
    nztype: usize,
) -> Vec<SurfaceCheck> {
    let zorders = [
        [(-1, -1), (-1, 1), (1, -1), (1, 1)],
        [(1, 1), (1, -1), (-1, 1), (-1, -1)],
        [(1, -1), (-1, -1), (1, 1), (-1, 1)],
        [(-1, 1), (1, 1), (-1, -1), (1, -1)],
    ];
    let xorders = [
        [(-1, -1), (1, -1), (-1, 1), (1, 1)],
        [(1, 1), (-1, 1), (1, -1), (-1, -1)],
        [(1, -1), (1, 1), (-1, -1), (-1, 1)],
        [(-1, 1), (-1, -1), (1, 1), (1, -1)],
    ];
    let xorder = xorders[nztype];
    let zorder = zorders[nztype];

    let mut checks = Vec::new();
    let mut idx = (m * n) as u32 + offset;

    for i in (0..=m).step_by(2) {
        for j in 0..n.saturating_sub(1) {
            let x = (2 * i) as i32 - 1 + 2 * ((j % 2) as i32);
            let y = (2 * j + 1) as i32;
            if x <= (2 * m - 1) as i32 {
                checks.push(SurfaceCheck {
                    check_type: SurfaceCheckType::X,
                    pos: (x, y),
                    idx,
                    data_qubits: [
                        data_dict.get(&(x + xorder[0].0, y + xorder[0].1)).copied(),
                        data_dict.get(&(x + xorder[1].0, y + xorder[1].1)).copied(),
                        data_dict.get(&(x + xorder[2].0, y + xorder[2].1)).copied(),
                        data_dict.get(&(x + xorder[3].0, y + xorder[3].1)).copied(),
                    ],
                });
                idx += 1;
            }
        }
    }

    for i in 0..m.saturating_sub(1) {
        for j in (0..=n).step_by(2) {
            let x = (2 * i + 1) as i32;
            let y = (2 * j + 1) as i32 - 2 * ((i % 2) as i32);
            if y <= (2 * n - 1) as i32 {
                checks.push(SurfaceCheck {
                    check_type: SurfaceCheckType::Z,
                    pos: (x, y),
                    idx,
                    data_qubits: [
                        data_dict.get(&(x + zorder[0].0, y + zorder[0].1)).copied(),
                        data_dict.get(&(x + zorder[1].0, y + zorder[1].1)).copied(),
                        data_dict.get(&(x + zorder[2].0, y + zorder[2].1)).copied(),
                        data_dict.get(&(x + zorder[3].0, y + zorder[3].1)).copied(),
                    ],
                });
                idx += 1;
            }
        }
    }

    checks
}

fn build_flag_map(data_list: &[u32], check_list: &[SurfaceCheck]) -> BTreeMap<u32, u32> {
    let mut flag_map = BTreeMap::new();
    let base = data_list.len() as u32 + check_list.len() as u32;
    for (i, check) in check_list.iter().enumerate() {
        if check.data_qubits.iter().any(Option::is_none) {
            continue;
        }
        flag_map.insert(check.idx, base + i as u32);
    }
    flag_map
}

fn initialize_surface_code_positions(
    data_list: &[u32],
    data_positions: &[(i32, i32)],
    check_list: &[SurfaceCheck],
    flag_map: &BTreeMap<u32, u32>,
) -> Vec<StimInstr> {
    let mut circuit = Vec::new();

    for (&idx, pos) in data_list.iter().zip(data_positions.iter()) {
        circuit.push(op(
            "QUBIT_COORDS",
            &[pos.0 as f64, pos.1 as f64],
            &[StimTarget::Qubit(idx)],
        ));
    }
    for check in check_list {
        circuit.push(op(
            "QUBIT_COORDS",
            &[check.pos.0 as f64, check.pos.1 as f64],
            &[StimTarget::Qubit(check.idx)],
        ));
    }

    if !flag_map.is_empty() {
        let xs = data_positions
            .iter()
            .map(|pos| pos.0)
            .chain(check_list.iter().map(|check| check.pos.0))
            .collect::<Vec<_>>();
        let ys = data_positions
            .iter()
            .map(|pos| pos.1)
            .chain(check_list.iter().map(|check| check.pos.1))
            .collect::<Vec<_>>();
        let min_x = *xs.iter().min().unwrap_or(&0);
        let max_x = *xs.iter().max().unwrap_or(&0);
        let _min_y = *ys.iter().min().unwrap_or(&0);
        let _max_y = *ys.iter().max().unwrap_or(&0);
        let offset_x = (max_x - min_x) + 2;
        let offset_y = 0;

        for check in check_list {
            let Some(flag_idx) = flag_map.get(&check.idx) else {
                continue;
            };
            let flag_pos = (check.pos.0 + offset_x, check.pos.1 + offset_y);
            circuit.push(op(
                "QUBIT_COORDS",
                &[flag_pos.0 as f64, flag_pos.1 as f64],
                &[StimTarget::Qubit(*flag_idx)],
            ));
        }
    }

    circuit
}

fn initialize_surface_code_qubits(
    circuit: &mut Vec<StimInstr>,
    data_list: &[u32],
    check_list: &[SurfaceCheck],
    flag_map: &BTreeMap<u32, u32>,
) {
    for &q in data_list {
        circuit.push(op("R", &[], &[StimTarget::Qubit(q)]));
    }
    for check in check_list {
        circuit.push(op("R", &[], &[StimTarget::Qubit(check.idx)]));
    }
    for &flag_idx in flag_map.values() {
        circuit.push(op("R", &[], &[StimTarget::Qubit(flag_idx)]));
    }
}

fn syndrome_measurement(
    circuit: &mut Vec<StimInstr>,
    check_list: &[SurfaceCheck],
    error_rate: f64,
    round: i32,
    flag_map: &BTreeMap<u32, u32>,
    p_anc_mid: f64,
) {
    for check in check_list {
        if check.check_type == SurfaceCheckType::X {
            circuit.push(op("H", &[], &[StimTarget::Qubit(check.idx)]));
        }
    }
    if !flag_map.is_empty() {
        for check in check_list {
            if check.check_type != SurfaceCheckType::Z {
                continue;
            }
            let Some(&flag_idx) = flag_map.get(&check.idx) else {
                continue;
            };
            circuit.push(op("H", &[], &[StimTarget::Qubit(flag_idx)]));
            if error_rate > 0.0 {
                circuit.push(op(
                    "DEPOLARIZE1",
                    &[error_rate],
                    &[StimTarget::Qubit(flag_idx)],
                ));
            }
        }
    }
    if error_rate > 0.0 {
        for check in check_list {
            if check.check_type == SurfaceCheckType::X {
                circuit.push(op(
                    "DEPOLARIZE1",
                    &[error_rate],
                    &[StimTarget::Qubit(check.idx)],
                ));
            }
        }
    }
    circuit.push(op("TICK", &[], &[]));

    for layer in 0..4 {
        for check in check_list {
            let Some(data_q) = check.data_qubits[layer] else {
                continue;
            };
            match check.check_type {
                SurfaceCheckType::X => {
                    circuit.push(op(
                        "CNOT",
                        &[],
                        &[StimTarget::Qubit(check.idx), StimTarget::Qubit(data_q)],
                    ));
                }
                SurfaceCheckType::Z => {
                    circuit.push(op(
                        "CNOT",
                        &[],
                        &[StimTarget::Qubit(data_q), StimTarget::Qubit(check.idx)],
                    ));
                }
            }
            if error_rate > 0.0 {
                circuit.push(op(
                    "DEPOLARIZE2",
                    &[error_rate],
                    &[StimTarget::Qubit(check.idx), StimTarget::Qubit(data_q)],
                ));
            }
        }
        circuit.push(op("TICK", &[], &[]));

        if !flag_map.is_empty() && (layer == 0 || layer == 2) {
            for check in check_list {
                let Some(&flag_idx) = flag_map.get(&check.idx) else {
                    continue;
                };
                match check.check_type {
                    SurfaceCheckType::Z => {
                        circuit.push(op(
                            "CNOT",
                            &[],
                            &[StimTarget::Qubit(flag_idx), StimTarget::Qubit(check.idx)],
                        ));
                        if error_rate > 0.0 {
                            circuit.push(op(
                                "DEPOLARIZE2",
                                &[error_rate],
                                &[StimTarget::Qubit(flag_idx), StimTarget::Qubit(check.idx)],
                            ));
                        }
                    }
                    SurfaceCheckType::X => {
                        circuit.push(op(
                            "CNOT",
                            &[],
                            &[StimTarget::Qubit(check.idx), StimTarget::Qubit(flag_idx)],
                        ));
                        if error_rate > 0.0 {
                            circuit.push(op(
                                "DEPOLARIZE2",
                                &[error_rate],
                                &[StimTarget::Qubit(check.idx), StimTarget::Qubit(flag_idx)],
                            ));
                        }
                    }
                }
            }
            circuit.push(op("TICK", &[], &[]));
        }

        if p_anc_mid > 0.0 && layer == 1 {
            for check in check_list {
                if !flag_map.is_empty() && !flag_map.contains_key(&check.idx) {
                    continue;
                }
                let noise_name = match check.check_type {
                    SurfaceCheckType::X => "X_ERROR",
                    SurfaceCheckType::Z => "Z_ERROR",
                };
                circuit.push(op(
                    noise_name,
                    &[p_anc_mid],
                    &[StimTarget::Qubit(check.idx)],
                ));
            }
            circuit.push(op("TICK", &[], &[]));
        }
    }

    for check in check_list {
        if check.check_type == SurfaceCheckType::X {
            circuit.push(op("H", &[], &[StimTarget::Qubit(check.idx)]));
        }
    }
    if error_rate > 0.0 {
        for check in check_list {
            if check.check_type == SurfaceCheckType::X {
                circuit.push(op(
                    "DEPOLARIZE1",
                    &[error_rate],
                    &[StimTarget::Qubit(check.idx)],
                ));
            }
        }
    }
    circuit.push(op("TICK", &[], &[]));

    if error_rate > 0.0 {
        for check in check_list {
            circuit.push(op(
                "X_ERROR",
                &[error_rate],
                &[StimTarget::Qubit(check.idx)],
            ));
        }
    }
    for check in check_list {
        circuit.push(op("MR", &[], &[StimTarget::Qubit(check.idx)]));
    }
    if !flag_map.is_empty() {
        for check in check_list {
            let Some(&flag_idx) = flag_map.get(&check.idx) else {
                continue;
            };
            if check.check_type == SurfaceCheckType::Z {
                circuit.push(op("H", &[], &[StimTarget::Qubit(flag_idx)]));
                if error_rate > 0.0 {
                    circuit.push(op(
                        "DEPOLARIZE1",
                        &[error_rate],
                        &[StimTarget::Qubit(flag_idx)],
                    ));
                }
            }
            if error_rate > 0.0 {
                circuit.push(op("X_ERROR", &[error_rate], &[StimTarget::Qubit(flag_idx)]));
            }
            circuit.push(op("MR", &[], &[StimTarget::Qubit(flag_idx)]));
            circuit.push(op(
                "DETECTOR",
                &[check.pos.0 as f64 + 0.2, check.pos.1 as f64, round as f64],
                &[StimTarget::Rec(-1)],
            ));
        }
    }
    circuit.push(op("TICK", &[], &[]));

    let check_count = check_list.len() as i32;
    let flag_count = flag_map.len() as i32;
    let records_per_round = check_count + flag_count;
    if round > 0 {
        for (i, check) in check_list.iter().enumerate() {
            let i = i as i32;
            let rec_cur = StimTarget::Rec(-(records_per_round - i));
            let rec_prev = StimTarget::Rec(-(records_per_round - i) - records_per_round);
            circuit.push(op(
                "DETECTOR",
                &[check.pos.0 as f64, check.pos.1 as f64, round as f64],
                &[rec_cur, rec_prev],
            ));
        }
    } else {
        for (i, check) in check_list.iter().enumerate() {
            if check.check_type != SurfaceCheckType::Z {
                continue;
            }
            let i = i as i32;
            let rec_cur = StimTarget::Rec(-(records_per_round - i));
            circuit.push(op(
                "DETECTOR",
                &[check.pos.0 as f64, check.pos.1 as f64, round as f64],
                &[rec_cur],
            ));
        }
    }
}

fn final_surface_z_measurement(
    circuit: &mut Vec<StimInstr>,
    data_list: &[u32],
    check_list: &[SurfaceCheck],
    m: usize,
    n: usize,
    error_rate: f64,
    round: i32,
    flag_map: &BTreeMap<u32, u32>,
) {
    if error_rate > 0.0 {
        let targets = data_list
            .iter()
            .copied()
            .map(StimTarget::Qubit)
            .collect::<Vec<_>>();
        circuit.push(op("X_ERROR", &[error_rate], &targets));
    }
    let targets = data_list
        .iter()
        .copied()
        .map(StimTarget::Qubit)
        .collect::<Vec<_>>();
    circuit.push(op("MR", &[], &targets));

    let check_count = check_list.len() as i32;
    let flag_count = flag_map.len() as i32;
    for (i, check) in check_list.iter().enumerate() {
        if check.check_type != SurfaceCheckType::Z {
            continue;
        }
        let mut detector_targets = vec![StimTarget::Rec(
            -(flag_count + (m * n) as i32 + (check_count - i as i32)),
        )];
        for data_q in check.data_qubits.iter().flatten() {
            detector_targets.push(StimTarget::Rec(*data_q as i32 - (m * n) as i32));
        }
        circuit.push(op(
            "DETECTOR",
            &[check.pos.0 as f64, check.pos.1 as f64, round as f64],
            &detector_targets,
        ));
    }

    let logical_z = (0..n)
        .map(|i| StimTarget::Rec(i as i32 - (m * n) as i32))
        .collect::<Vec<_>>();
    circuit.push(op("OBSERVABLE_INCLUDE", &[0.0], &logical_z));
}

#[allow(clippy::too_many_arguments)]
fn append_steane_syndrome_rounds(
    n_data: u32,
    anc_z: &[u32],
    anc_x: &[u32],
    flag_z: &[u32],
    flag_x: &[u32],
    rounds: usize,
    p_data: f64,
    p_gate: f64,
    p_meas: f64,
    p_reset: f64,
    p_anc_mid: f64,
    logical_basis: LogicalBasis,
) -> Vec<StimInstr> {
    let mut circuit = Vec::new();
    let n_anc = (anc_z.len() + anc_x.len()) as u32;
    let n_flag = (flag_z.len() + flag_x.len()) as u32;

    circuit.push(op(
        "R",
        &[],
        &(0..(n_data + n_anc + n_flag))
            .map(StimTarget::Qubit)
            .collect::<Vec<_>>(),
    ));
    if p_reset > 0.0 {
        circuit.push(op(
            "X_ERROR",
            &[p_reset],
            &(0..n_data).map(StimTarget::Qubit).collect::<Vec<_>>(),
        ));
    }
    if logical_basis == LogicalBasis::X {
        circuit.push(op(
            "H",
            &[],
            &(0..n_data).map(StimTarget::Qubit).collect::<Vec<_>>(),
        ));
    }

    let mut meas_index = 0i32;
    let mut last_meas: HashMap<usize, i32> = HashMap::new();

    for round in 0..rounds {
        if p_data > 0.0 && round == 0 {
            circuit.push(op(
                "DEPOLARIZE1",
                &[p_data],
                &(0..n_data).map(StimTarget::Qubit).collect::<Vec<_>>(),
            ));
        }

        for (s_idx, (&a, stab)) in anc_z.iter().zip(STEANE_Z_STABS.iter()).enumerate() {
            circuit.push(op("R", &[], &[StimTarget::Qubit(a)]));
            if p_reset > 0.0 {
                circuit.push(op("X_ERROR", &[p_reset], &[StimTarget::Qubit(a)]));
            }
            let flag = flag_z.get(s_idx).copied();
            if let Some(f) = flag {
                circuit.push(op("R", &[], &[StimTarget::Qubit(f)]));
                if p_reset > 0.0 {
                    circuit.push(op("X_ERROR", &[p_reset], &[StimTarget::Qubit(f)]));
                }
                circuit.push(op("H", &[], &[StimTarget::Qubit(f)]));
            }

            for (q_i, &q) in stab.iter().enumerate() {
                circuit.push(op("CX", &[], &[StimTarget::Qubit(q), StimTarget::Qubit(a)]));
                if p_gate > 0.0 {
                    circuit.push(op(
                        "DEPOLARIZE2",
                        &[p_gate],
                        &[StimTarget::Qubit(q), StimTarget::Qubit(a)],
                    ));
                }
                if p_anc_mid > 0.0 && q_i == 1 {
                    circuit.push(op("Z_ERROR", &[p_anc_mid], &[StimTarget::Qubit(a)]));
                }
                if let Some(f) = flag {
                    if q == stab[0] || q == stab[2] {
                        circuit.push(op("CX", &[], &[StimTarget::Qubit(f), StimTarget::Qubit(a)]));
                        if p_gate > 0.0 {
                            circuit.push(op(
                                "DEPOLARIZE2",
                                &[p_gate],
                                &[StimTarget::Qubit(f), StimTarget::Qubit(a)],
                            ));
                        }
                    }
                }
            }
            if p_meas > 0.0 {
                circuit.push(op("X_ERROR", &[p_meas], &[StimTarget::Qubit(a)]));
            }
            circuit.push(op("M", &[], &[StimTarget::Qubit(a)]));
            if let Some(prev) = last_meas.get(&s_idx) {
                let delta = meas_index - *prev + 1;
                circuit.push(op(
                    "DETECTOR",
                    &[],
                    &[StimTarget::Rec(-1), StimTarget::Rec(-delta)],
                ));
            } else if logical_basis == LogicalBasis::Z {
                circuit.push(op("DETECTOR", &[], &[StimTarget::Rec(-1)]));
            }
            last_meas.insert(s_idx, meas_index);
            meas_index += 1;

            if let Some(f) = flag {
                if p_meas > 0.0 {
                    circuit.push(op("X_ERROR", &[p_meas], &[StimTarget::Qubit(f)]));
                }
                circuit.push(op("H", &[], &[StimTarget::Qubit(f)]));
                circuit.push(op("M", &[], &[StimTarget::Qubit(f)]));
                circuit.push(op("DETECTOR", &[], &[StimTarget::Rec(-1)]));
                meas_index += 1;
            }
        }
        circuit.push(op("TICK", &[], &[]));

        for (offset, (&a, stab)) in anc_x.iter().zip(STEANE_X_STABS.iter()).enumerate() {
            let s_idx = STEANE_Z_STABS.len() + offset;
            circuit.push(op("R", &[], &[StimTarget::Qubit(a)]));
            if p_reset > 0.0 {
                circuit.push(op("X_ERROR", &[p_reset], &[StimTarget::Qubit(a)]));
            }
            let flag = flag_x.get(offset).copied();
            if let Some(f) = flag {
                circuit.push(op("R", &[], &[StimTarget::Qubit(f)]));
                if p_reset > 0.0 {
                    circuit.push(op("X_ERROR", &[p_reset], &[StimTarget::Qubit(f)]));
                }
            }
            circuit.push(op("H", &[], &[StimTarget::Qubit(a)]));

            for (q_i, &q) in stab.iter().enumerate() {
                circuit.push(op("CX", &[], &[StimTarget::Qubit(a), StimTarget::Qubit(q)]));
                if p_gate > 0.0 {
                    circuit.push(op(
                        "DEPOLARIZE2",
                        &[p_gate],
                        &[StimTarget::Qubit(a), StimTarget::Qubit(q)],
                    ));
                }
                if p_anc_mid > 0.0 && q_i == 1 {
                    circuit.push(op("X_ERROR", &[p_anc_mid], &[StimTarget::Qubit(a)]));
                }
                if let Some(f) = flag {
                    if q == stab[0] || q == stab[2] {
                        circuit.push(op("CX", &[], &[StimTarget::Qubit(a), StimTarget::Qubit(f)]));
                        if p_gate > 0.0 {
                            circuit.push(op(
                                "DEPOLARIZE2",
                                &[p_gate],
                                &[StimTarget::Qubit(a), StimTarget::Qubit(f)],
                            ));
                        }
                    }
                }
            }

            circuit.push(op("H", &[], &[StimTarget::Qubit(a)]));
            if p_meas > 0.0 {
                circuit.push(op("X_ERROR", &[p_meas], &[StimTarget::Qubit(a)]));
            }
            circuit.push(op("M", &[], &[StimTarget::Qubit(a)]));
            if let Some(prev) = last_meas.get(&s_idx) {
                let delta = meas_index - *prev + 1;
                circuit.push(op(
                    "DETECTOR",
                    &[],
                    &[StimTarget::Rec(-1), StimTarget::Rec(-delta)],
                ));
            } else if logical_basis == LogicalBasis::X {
                circuit.push(op("DETECTOR", &[], &[StimTarget::Rec(-1)]));
            }
            last_meas.insert(s_idx, meas_index);
            meas_index += 1;

            if let Some(f) = flag {
                if p_meas > 0.0 {
                    circuit.push(op("X_ERROR", &[p_meas], &[StimTarget::Qubit(f)]));
                }
                circuit.push(op("M", &[], &[StimTarget::Qubit(f)]));
                circuit.push(op("DETECTOR", &[], &[StimTarget::Rec(-1)]));
                meas_index += 1;
            }
        }
        circuit.push(op("TICK", &[], &[]));

        if p_data > 0.0 && round != rounds.saturating_sub(1) {
            circuit.push(op(
                "DEPOLARIZE1",
                &[p_data],
                &(0..n_data).map(StimTarget::Qubit).collect::<Vec<_>>(),
            ));
        }
    }

    match logical_basis {
        LogicalBasis::Z => {
            if p_meas > 0.0 {
                circuit.push(op(
                    "X_ERROR",
                    &[p_meas],
                    &(0..n_data).map(StimTarget::Qubit).collect::<Vec<_>>(),
                ));
            }
            circuit.push(op(
                "M",
                &[],
                &(0..n_data).map(StimTarget::Qubit).collect::<Vec<_>>(),
            ));
            let data_meas_start = meas_index;
            meas_index += n_data as i32;

            for (s_idx, stab) in STEANE_Z_STABS.iter().enumerate() {
                let Some(&last) = last_meas.get(&s_idx) else {
                    continue;
                };
                let mut targets = Vec::new();
                for &q in stab {
                    let k = data_meas_start + q as i32;
                    targets.push(StimTarget::Rec(-(meas_index - k)));
                }
                targets.push(StimTarget::Rec(-(meas_index - last)));
                circuit.push(op("DETECTOR", &[], &targets));
            }

            let logical = (1..=n_data)
                .map(|i| StimTarget::Rec(-(i as i32)))
                .collect::<Vec<_>>();
            circuit.push(op("OBSERVABLE_INCLUDE", &[0.0], &logical));
        }
        LogicalBasis::X => {
            circuit.push(op(
                "H",
                &[],
                &(0..n_data).map(StimTarget::Qubit).collect::<Vec<_>>(),
            ));
            if p_meas > 0.0 {
                circuit.push(op(
                    "X_ERROR",
                    &[p_meas],
                    &(0..n_data).map(StimTarget::Qubit).collect::<Vec<_>>(),
                ));
            }
            circuit.push(op(
                "M",
                &[],
                &(0..n_data).map(StimTarget::Qubit).collect::<Vec<_>>(),
            ));
            let data_meas_start = meas_index;
            meas_index += n_data as i32;

            for (offset, stab) in STEANE_X_STABS.iter().enumerate() {
                let s_idx = STEANE_Z_STABS.len() + offset;
                let Some(&last) = last_meas.get(&s_idx) else {
                    continue;
                };
                let mut targets = Vec::new();
                for &q in stab {
                    let k = data_meas_start + q as i32;
                    targets.push(StimTarget::Rec(-(meas_index - k)));
                }
                targets.push(StimTarget::Rec(-(meas_index - last)));
                circuit.push(op("DETECTOR", &[], &targets));
            }

            let logical = (1..=n_data)
                .map(|i| StimTarget::Rec(-(i as i32)))
                .collect::<Vec<_>>();
            circuit.push(op("OBSERVABLE_INCLUDE", &[0.0], &logical));
        }
    }

    circuit
}

fn op(name: &str, args: &[f64], targets: &[StimTarget]) -> StimInstr {
    StimInstr::Op {
        name: name.to_string(),
        tag: None,
        args: args.to_vec(),
        targets: targets.to_vec(),
    }
}
