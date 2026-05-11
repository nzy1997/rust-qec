use rand::Rng;

use crate::coords::CoordState;
use crate::ir::{PauliBasis, StimInstr, StimTarget};
use crate::recorder::Recorder;
use crate::sim::tableau::StabilizerState;

pub struct Executor {
    instrs: Vec<StimInstr>,
}

pub struct ExecOutput {
    pub measurements: Vec<bool>,
    pub detectors: Vec<bool>,
    pub detector_coords: Vec<Vec<f64>>,
    pub observables: Vec<(u32, bool)>,
    pub qubit_coords: std::collections::HashMap<u32, Vec<f64>>,
}

impl Executor {
    pub fn from_instrs(instrs: Vec<StimInstr>) -> Result<Self, String> {
        Ok(Self { instrs })
    }

    pub fn run(&mut self, rng: &mut impl Rng) -> Result<ExecOutput, String> {
        let n = max_qubit(&self.instrs)?;
        let mut state = StabilizerState::new(n);
        let mut lost = vec![false; n];
        let mut recorder = Recorder::default();
        let mut detectors = Vec::new();
        let mut detector_coords = Vec::new();
        let mut observables = Vec::new();
        let mut coords = CoordState::default();
        let mut last_correlated_error_occurred = false;

        for instr in &self.instrs {
            match instr {
                StimInstr::Op {
                    name,
                    args,
                    targets,
                    ..
                } => {
                    match name.as_str() {
                        "I" | "I_ERROR" | "II_ERROR" => {}
                        "H" => for_each_present_qubit(targets, &lost, |q| state.h(q))?,
                        "H_XY" => for_each_present_qubit(targets, &lost, |q| state.h_xy(q))?,
                        "H_YZ" => for_each_present_qubit(targets, &lost, |q| state.h_yz(q))?,
                        "S" | "SQRT_Z" => for_each_present_qubit(targets, &lost, |q| state.s(q))?,
                        "S_DAG" | "SQRT_Z_DAG" => {
                            for_each_present_qubit(targets, &lost, |q| state.s_dag(q))?
                        }
                        "SQRT_X" => for_each_present_qubit(targets, &lost, |q| state.sqrt_x(q))?,
                        "SQRT_X_DAG" => {
                            for_each_present_qubit(targets, &lost, |q| state.sqrt_x_dag(q))?
                        }
                        "SQRT_Y" => for_each_present_qubit(targets, &lost, |q| state.sqrt_y(q))?,
                        "SQRT_Y_DAG" => {
                            for_each_present_qubit(targets, &lost, |q| state.sqrt_y_dag(q))?
                        }
                        "X" => for_each_present_qubit(targets, &lost, |q| state.x_gate(q))?,
                        "Y" => for_each_present_qubit(targets, &lost, |q| state.y_gate(q))?,
                        "Z" => for_each_present_qubit(targets, &lost, |q| state.z_gate(q))?,
                        "C_XYZ" => for_each_present_qubit(targets, &lost, |q| state.c_xyz(q))?,
                        "C_ZYX" => for_each_present_qubit(targets, &lost, |q| state.c_zyx(q))?,
                        "C_NXYZ" => for_each_present_qubit(targets, &lost, |q| state.c_nxyz(q))?,
                        "C_NZYX" => for_each_present_qubit(targets, &lost, |q| state.c_nzyx(q))?,
                        "C_XNYZ" => for_each_present_qubit(targets, &lost, |q| state.c_xnyz(q))?,
                        "C_XYNZ" => for_each_present_qubit(targets, &lost, |q| state.c_xynz(q))?,
                        "C_ZNYX" => for_each_present_qubit(targets, &lost, |q| state.c_znyx(q))?,
                        "C_ZYNX" => for_each_present_qubit(targets, &lost, |q| state.c_zynx(q))?,
                        "H_NXY" => for_each_present_qubit(targets, &lost, |q| state.h_nxy(q))?,
                        "H_NXZ" => for_each_present_qubit(targets, &lost, |q| state.h_nxz(q))?,
                        "H_NYZ" => for_each_present_qubit(targets, &lost, |q| state.h_nyz(q))?,
                        "CX" | "CNOT" | "ZCX" => {
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            for (c, t) in pairs {
                                state.cx(c, t);
                            }
                        }
                        "CY" | "ZCY" => {
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            for (c, t) in pairs {
                                state.cy(c, t);
                            }
                        }
                        "CZ" | "ZCZ" => {
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            for (c, t) in pairs {
                                state.cz(c, t);
                            }
                        }
                        "XCX" => {
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            for (a, b) in pairs {
                                state.xcx(a, b);
                            }
                        }
                        "XCY" => {
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            for (a, b) in pairs {
                                state.xcy(a, b);
                            }
                        }
                        "XCZ" => {
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            for (a, b) in pairs {
                                state.xcz(a, b);
                            }
                        }
                        "YCX" => {
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            for (a, b) in pairs {
                                state.ycx(a, b);
                            }
                        }
                        "YCY" => {
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            for (a, b) in pairs {
                                state.ycy(a, b);
                            }
                        }
                        "YCZ" => {
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            for (a, b) in pairs {
                                state.ycz(a, b);
                            }
                        }
                        "SWAP" => {
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            for (a, b) in pairs {
                                state.swap(a, b);
                            }
                        }
                        "ISWAP" => {
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            for (a, b) in pairs {
                                state.iswap(a, b);
                            }
                        }
                        "ISWAP_DAG" => {
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            for (a, b) in pairs {
                                state.iswap_dag(a, b);
                            }
                        }
                        "CXSWAP" => {
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            for (a, b) in pairs {
                                state.cxswap(a, b);
                            }
                        }
                        "SWAPCX" => {
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            for (a, b) in pairs {
                                state.swapcx(a, b);
                            }
                        }
                        "CZSWAP" => {
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            for (a, b) in pairs {
                                state.czswap(a, b);
                            }
                        }
                        "M" | "MZ" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                record_measure_z(&mut state, &lost, q, inv, rng, &mut recorder);
                            }
                        }
                        "MX" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                record_measure_x(&mut state, &lost, q, inv, rng, &mut recorder);
                            }
                        }
                        "MY" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                record_measure_y(&mut state, &lost, q, inv, rng, &mut recorder);
                            }
                        }
                        "MR" | "MRZ" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                measure_reset_z(&mut state, &mut lost, q, inv, rng, &mut recorder);
                            }
                        }
                        "MRX" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                measure_reset_x(&mut state, &mut lost, q, inv, rng, &mut recorder);
                            }
                        }
                        "MRY" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                measure_reset_y(&mut state, &mut lost, q, inv, rng, &mut recorder);
                            }
                        }
                        "ML" | "MZL" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                record_loss_visible_measure_z(
                                    &mut state,
                                    &lost,
                                    q,
                                    inv,
                                    rng,
                                    &mut recorder,
                                );
                            }
                        }
                        "MXL" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                record_loss_visible_measure_x(
                                    &mut state,
                                    &lost,
                                    q,
                                    inv,
                                    rng,
                                    &mut recorder,
                                );
                            }
                        }
                        "MYL" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                record_loss_visible_measure_y(
                                    &mut state,
                                    &lost,
                                    q,
                                    inv,
                                    rng,
                                    &mut recorder,
                                );
                            }
                        }
                        "MRL" | "MRZL" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                measure_reset_loss_visible_z(
                                    &mut state,
                                    &mut lost,
                                    q,
                                    inv,
                                    rng,
                                    &mut recorder,
                                );
                            }
                        }
                        "MRXL" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                measure_reset_loss_visible_x(
                                    &mut state,
                                    &mut lost,
                                    q,
                                    inv,
                                    rng,
                                    &mut recorder,
                                );
                            }
                        }
                        "MRYL" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                measure_reset_loss_visible_y(
                                    &mut state,
                                    &mut lost,
                                    q,
                                    inv,
                                    rng,
                                    &mut recorder,
                                );
                            }
                        }
                        "MPAD" => {
                            let p = args.first().copied().unwrap_or(0.0);
                            for t in targets {
                                let q = expect_qubit(t)?;
                                let mut bit = q != 0;
                                if p > 0.0 && rng.r#gen::<f64>() < p {
                                    bit = !bit;
                                }
                                recorder.push(bit);
                            }
                        }
                        "R" | "RZ" => {
                            for q in qubits(targets)? {
                                lost[q] = false;
                                state.reset_z(q, rng);
                            }
                        }
                        "RX" => {
                            for q in qubits(targets)? {
                                lost[q] = false;
                                state.reset_x(q, rng);
                            }
                        }
                        "RY" => {
                            for q in qubits(targets)? {
                                lost[q] = false;
                                state.reset_y(q, rng);
                            }
                        }
                        "LOSS" => {
                            let p = args.get(0).copied().unwrap_or(0.0);
                            for q in qubits(targets)? {
                                if rng.r#gen::<f64>() < p {
                                    lost[q] = true;
                                }
                            }
                        }
                        "X_ERROR" => {
                            let p = args.get(0).copied().unwrap_or(0.0);
                            for q in qubits(targets)? {
                                if !lost[q] && rng.r#gen::<f64>() < p {
                                    state.x_gate(q);
                                }
                            }
                        }
                        "Y_ERROR" => {
                            let p = args.get(0).copied().unwrap_or(0.0);
                            for q in qubits(targets)? {
                                if !lost[q] && rng.r#gen::<f64>() < p {
                                    state.y_gate(q);
                                }
                            }
                        }
                        "Z_ERROR" => {
                            let p = args.get(0).copied().unwrap_or(0.0);
                            for q in qubits(targets)? {
                                if !lost[q] && rng.r#gen::<f64>() < p {
                                    state.z_gate(q);
                                }
                            }
                        }
                        "DEPOLARIZE1" => {
                            let p = args.get(0).copied().unwrap_or(0.0);
                            for q in qubits(targets)? {
                                if !lost[q] && rng.r#gen::<f64>() < p {
                                    match rng.gen_range(0..3) {
                                        0 => state.x_gate(q),
                                        1 => state.y_gate(q),
                                        _ => state.z_gate(q),
                                    }
                                }
                            }
                        }
                        "DEPOLARIZE2" => {
                            let p = args.get(0).copied().unwrap_or(0.0);
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            for (a, b) in pairs {
                                if rng.r#gen::<f64>() < p {
                                    let r = rng.gen_range(0..15);
                                    let (pa, pb) = two_qubit_pauli(r);
                                    apply_pauli(&mut state, a, pa);
                                    apply_pauli(&mut state, b, pb);
                                }
                            }
                        }
                        "QUBIT_COORDS" => {
                            let coords_vec = coords.apply_offset(args);
                            for t in targets {
                                if let StimTarget::Qubit(q) = t {
                                    coords.qubit_coords.insert(*q, coords_vec.clone());
                                } else {
                                    return Err("QUBIT_COORDS expects qubit targets".to_string());
                                }
                            }
                        }
                        "SHIFT_COORDS" => {
                            coords.shift(args);
                        }
                        "TICK" => {
                            coords.tick += 1;
                        }
                        "DETECTOR" => {
                            let bit = xor_recs(&recorder, targets)?;
                            detectors.push(bit);
                            let det_coords = coords.apply_offset(args);
                            detector_coords.push(det_coords);
                        }
                        "OBSERVABLE_INCLUDE" => {
                            let index = args.get(0).copied().unwrap_or(0.0) as u32;
                            let bit = xor_recs(&recorder, targets)?;
                            observables.push((index, bit));
                        }
                        "MXX" => {
                            let p = args.first().copied().unwrap_or(0.0);
                            pair_measure(
                                &mut state,
                                &lost,
                                targets,
                                PauliBasis::X,
                                p,
                                rng,
                                &mut recorder,
                            )?;
                        }
                        "MYY" => {
                            let p = args.first().copied().unwrap_or(0.0);
                            pair_measure(
                                &mut state,
                                &lost,
                                targets,
                                PauliBasis::Y,
                                p,
                                rng,
                                &mut recorder,
                            )?;
                        }
                        "MZZ" => {
                            let p = args.first().copied().unwrap_or(0.0);
                            pair_measure(
                                &mut state,
                                &lost,
                                targets,
                                PauliBasis::Z,
                                p,
                                rng,
                                &mut recorder,
                            )?;
                        }
                        "MPP" => {
                            let p = args.first().copied().unwrap_or(0.0);
                            let products = split_pauli_products(targets)?;
                            for product in &products {
                                let mut bit = if product.terms.iter().any(|(q, _)| lost[*q]) {
                                    true
                                } else {
                                    measure_pauli_product(
                                        &mut state,
                                        &product.terms,
                                        product.inverted,
                                        rng,
                                    )
                                };
                                if p > 0.0 && rng.r#gen::<f64>() < p {
                                    bit = !bit;
                                }
                                recorder.push(bit);
                            }
                        }
                        "SPP" => {
                            let products = split_pauli_products(targets)?;
                            for product in &products {
                                apply_spp(&mut state, &product.terms, product.inverted, false);
                            }
                        }
                        "SPP_DAG" => {
                            let products = split_pauli_products(targets)?;
                            for product in &products {
                                apply_spp(&mut state, &product.terms, product.inverted, true);
                            }
                        }
                        "PAULI_CHANNEL_1" => {
                            let px = args.first().copied().unwrap_or(0.0);
                            let py = args.get(1).copied().unwrap_or(0.0);
                            let pz = args.get(2).copied().unwrap_or(0.0);
                            for q in qubits(targets)? {
                                if !lost[q] {
                                    let r: f64 = rng.r#gen();
                                    if r < px {
                                        state.x_gate(q);
                                    } else if r < px + py {
                                        state.y_gate(q);
                                    } else if r < px + py + pz {
                                        state.z_gate(q);
                                    }
                                }
                            }
                        }
                        "PAULI_CHANNEL_2" => {
                            let probs: Vec<f64> = (0..15)
                                .map(|i| args.get(i).copied().unwrap_or(0.0))
                                .collect();
                            let pairs = present_qubit_pairs(targets, &lost)?;
                            let paulis: [(u8, u8); 15] = [
                                (0, 1),
                                (0, 2),
                                (0, 3),
                                (1, 0),
                                (1, 1),
                                (1, 2),
                                (1, 3),
                                (2, 0),
                                (2, 1),
                                (2, 2),
                                (2, 3),
                                (3, 0),
                                (3, 1),
                                (3, 2),
                                (3, 3),
                            ];
                            for (a, b) in pairs {
                                let r: f64 = rng.r#gen();
                                let mut cumulative = 0.0;
                                let mut chosen = None;
                                for (i, &(pa, pb)) in paulis.iter().enumerate() {
                                    cumulative += probs[i];
                                    if r < cumulative {
                                        chosen = Some((pa, pb));
                                        break;
                                    }
                                }
                                if let Some((pa, pb)) = chosen {
                                    apply_pauli(&mut state, a, pa);
                                    apply_pauli(&mut state, b, pb);
                                }
                            }
                        }
                        "HERALDED_ERASE" => {
                            let p = args.first().copied().unwrap_or(0.0);
                            for q in qubits(targets)? {
                                if p > 0.0 && rng.r#gen::<f64>() < p {
                                    recorder.push(true);
                                    match rng.gen_range(0u8..4) {
                                        1 => state.x_gate(q),
                                        2 => state.y_gate(q),
                                        3 => state.z_gate(q),
                                        _ => {} // I
                                    }
                                } else {
                                    recorder.push(false);
                                }
                            }
                        }
                        "HERALDED_PAULI_CHANNEL_1" => {
                            let pi = args.first().copied().unwrap_or(0.0);
                            let px = args.get(1).copied().unwrap_or(0.0);
                            let py = args.get(2).copied().unwrap_or(0.0);
                            let pz = args.get(3).copied().unwrap_or(0.0);
                            let total = pi + px + py + pz;
                            for q in qubits(targets)? {
                                let r: f64 = rng.r#gen();
                                if r < total {
                                    recorder.push(true);
                                    let inner = r;
                                    if inner < pi {
                                        // I — false positive
                                    } else if inner < pi + px {
                                        state.x_gate(q);
                                    } else if inner < pi + px + py {
                                        state.y_gate(q);
                                    } else {
                                        state.z_gate(q);
                                    }
                                } else {
                                    recorder.push(false);
                                }
                            }
                        }
                        "CORRELATED_ERROR" | "E" => {
                            let p = args.first().copied().unwrap_or(0.0);
                            if p > 0.0 && rng.r#gen::<f64>() < p {
                                apply_pauli_targets(&mut state, targets)?;
                                last_correlated_error_occurred = true;
                            } else {
                                last_correlated_error_occurred = false;
                            }
                        }
                        "ELSE_CORRELATED_ERROR" => {
                            if !last_correlated_error_occurred {
                                let p = args.first().copied().unwrap_or(0.0);
                                if p > 0.0 && rng.r#gen::<f64>() < p {
                                    apply_pauli_targets(&mut state, targets)?;
                                    last_correlated_error_occurred = true;
                                }
                            }
                        }
                        _ => return Err(format!("unsupported instruction {}", name)),
                    }
                }
                StimInstr::Repeat { count, body } => {
                    for _ in 0..*count {
                        let mut inner = Executor::from_instrs(body.clone())?;
                        let out = inner.run(rng)?;
                        recorder.extend(out.measurements);
                        detectors.extend(out.detectors);
                        detector_coords.extend(out.detector_coords);
                        observables.extend(out.observables);
                        coords.qubit_coords.extend(out.qubit_coords);
                    }
                }
            }
        }

        Ok(ExecOutput {
            measurements: recorder_bits(recorder),
            detectors,
            detector_coords,
            observables,
            qubit_coords: coords.qubit_coords,
        })
    }
}

// --- reference_sample: noiseless biased simulation ---

pub fn reference_sample(instrs: &[StimInstr]) -> Result<Vec<bool>, String> {
    reference_sample_with_sweep_bits(instrs, None)
}

pub fn reference_sample_with_sweep_bits(
    instrs: &[StimInstr],
    sweep_bits: Option<&[bool]>,
) -> Result<Vec<bool>, String> {
    let n = max_qubit(instrs)?;
    let mut state = StabilizerState::new(n);
    let mut lost = vec![false; n];
    let mut measurements = Vec::new();
    ref_sample_instrs(&mut state, &mut lost, &mut measurements, instrs, sweep_bits)?;
    Ok(measurements)
}

fn ref_sample_instrs(
    state: &mut StabilizerState,
    lost: &mut [bool],
    measurements: &mut Vec<bool>,
    instrs: &[StimInstr],
    sweep_bits: Option<&[bool]>,
) -> Result<(), String> {
    for instr in instrs {
        match instr {
            StimInstr::Op {
                name,
                args,
                targets,
                ..
            } => {
                ref_sample_op(
                    state,
                    lost,
                    measurements,
                    name.as_str(),
                    args,
                    targets,
                    sweep_bits,
                )?;
            }
            StimInstr::Repeat { count, body } => {
                for _ in 0..*count {
                    ref_sample_instrs(state, lost, measurements, body, sweep_bits)?;
                }
            }
        }
    }
    Ok(())
}

fn ref_sample_op(
    state: &mut StabilizerState,
    lost: &mut [bool],
    measurements: &mut Vec<bool>,
    name: &str,
    _args: &[f64],
    targets: &[StimTarget],
    sweep_bits: Option<&[bool]>,
) -> Result<(), String> {
    match name {
        // Identity
        "I" => {}

        // Single-qubit Cliffords
        "H" => for_each_present_qubit(targets, lost, |q| state.h(q))?,
        "H_XY" => for_each_present_qubit(targets, lost, |q| state.h_xy(q))?,
        "H_YZ" => for_each_present_qubit(targets, lost, |q| state.h_yz(q))?,
        "S" | "SQRT_Z" => for_each_present_qubit(targets, lost, |q| state.s(q))?,
        "S_DAG" | "SQRT_Z_DAG" => for_each_present_qubit(targets, lost, |q| state.s_dag(q))?,
        "SQRT_X" => for_each_present_qubit(targets, lost, |q| state.sqrt_x(q))?,
        "SQRT_X_DAG" => for_each_present_qubit(targets, lost, |q| state.sqrt_x_dag(q))?,
        "SQRT_Y" => for_each_present_qubit(targets, lost, |q| state.sqrt_y(q))?,
        "SQRT_Y_DAG" => for_each_present_qubit(targets, lost, |q| state.sqrt_y_dag(q))?,
        "X" => for_each_present_qubit(targets, lost, |q| state.x_gate(q))?,
        "Y" => for_each_present_qubit(targets, lost, |q| state.y_gate(q))?,
        "Z" => for_each_present_qubit(targets, lost, |q| state.z_gate(q))?,
        "C_XYZ" => for_each_present_qubit(targets, lost, |q| state.c_xyz(q))?,
        "C_ZYX" => for_each_present_qubit(targets, lost, |q| state.c_zyx(q))?,
        "C_NXYZ" => for_each_present_qubit(targets, lost, |q| state.c_nxyz(q))?,
        "C_NZYX" => for_each_present_qubit(targets, lost, |q| state.c_nzyx(q))?,
        "C_XNYZ" => for_each_present_qubit(targets, lost, |q| state.c_xnyz(q))?,
        "C_XYNZ" => for_each_present_qubit(targets, lost, |q| state.c_xynz(q))?,
        "C_ZNYX" => for_each_present_qubit(targets, lost, |q| state.c_znyx(q))?,
        "C_ZYNX" => for_each_present_qubit(targets, lost, |q| state.c_zynx(q))?,
        "H_NXY" => for_each_present_qubit(targets, lost, |q| state.h_nxy(q))?,
        "H_NXZ" => for_each_present_qubit(targets, lost, |q| state.h_nxz(q))?,
        "H_NYZ" => for_each_present_qubit(targets, lost, |q| state.h_nyz(q))?,

        // Two-qubit Cliffords
        "CX" | "CNOT" | "ZCX" => {
            apply_reference_controlled_pairs(state, name, targets, sweep_bits)?;
        }
        "CY" | "ZCY" => {
            apply_reference_controlled_pairs(state, name, targets, sweep_bits)?;
        }
        "CZ" | "ZCZ" => {
            apply_reference_controlled_pairs(state, name, targets, sweep_bits)?;
        }
        "XCX" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.xcx(a, b);
            }
        }
        "XCY" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.xcy(a, b);
            }
        }
        "XCZ" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.xcz(a, b);
            }
        }
        "YCX" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.ycx(a, b);
            }
        }
        "YCY" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.ycy(a, b);
            }
        }
        "YCZ" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.ycz(a, b);
            }
        }
        "SWAP" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.swap(a, b);
            }
        }
        "ISWAP" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.iswap(a, b);
            }
        }
        "ISWAP_DAG" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.iswap_dag(a, b);
            }
        }
        "CXSWAP" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.cxswap(a, b);
            }
        }
        "SWAPCX" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.swapcx(a, b);
            }
        }
        "CZSWAP" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.czswap(a, b);
            }
        }

        // Measurements (biased)
        "M" | "MZ" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                record_reference_measure_z(state, lost, q, inv, measurements);
            }
        }
        "MX" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                record_reference_measure_x(state, lost, q, inv, measurements);
            }
        }
        "MY" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                record_reference_measure_y(state, lost, q, inv, measurements);
            }
        }
        "MR" | "MRZ" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                reference_measure_reset_z(state, lost, q, inv, measurements);
            }
        }
        "MRX" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                reference_measure_reset_x(state, lost, q, inv, measurements);
            }
        }
        "MRY" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                reference_measure_reset_y(state, lost, q, inv, measurements);
            }
        }
        "ML" | "MZL" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                record_reference_loss_visible_measure_z(state, lost, q, inv, measurements);
            }
        }
        "MXL" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                record_reference_loss_visible_measure_x(state, lost, q, inv, measurements);
            }
        }
        "MYL" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                record_reference_loss_visible_measure_y(state, lost, q, inv, measurements);
            }
        }
        "MRL" | "MRZL" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                reference_measure_reset_loss_visible_z(state, lost, q, inv, measurements);
            }
        }
        "MRXL" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                reference_measure_reset_loss_visible_x(state, lost, q, inv, measurements);
            }
        }
        "MRYL" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                reference_measure_reset_loss_visible_y(state, lost, q, inv, measurements);
            }
        }
        "MPAD" => {
            for t in targets {
                let q = expect_qubit(t)?;
                measurements.push(q != 0);
            }
        }

        // Resets (biased)
        "R" | "RZ" => {
            for q in qubits(targets)? {
                lost[q] = false;
                state.reset_z_biased(q);
            }
        }
        "RX" => {
            for q in qubits(targets)? {
                lost[q] = false;
                state.reset_x_biased(q);
            }
        }
        "RY" => {
            for q in qubits(targets)? {
                lost[q] = false;
                state.reset_y_biased(q);
            }
        }
        "LOSS" => for _q in qubits(targets)? {},

        // Multi-qubit Pauli measurements (biased)
        "MPP" => {
            let products = split_pauli_products(targets)?;
            for product in &products {
                let bit = reference_measure_pauli_product_biased(
                    state,
                    lost,
                    &product.terms,
                    product.inverted,
                );
                measurements.push(bit);
            }
        }
        "MXX" => {
            let pairs = qubits_with_inversion_pairs(targets)?;
            for ((a, inv_a), (b, _)) in pairs {
                let bit = reference_measure_pair_biased(state, lost, a, b, PauliBasis::X, inv_a);
                measurements.push(bit);
            }
        }
        "MYY" => {
            let pairs = qubits_with_inversion_pairs(targets)?;
            for ((a, inv_a), (b, _)) in pairs {
                let bit = reference_measure_pair_biased(state, lost, a, b, PauliBasis::Y, inv_a);
                measurements.push(bit);
            }
        }
        "MZZ" => {
            let pairs = qubits_with_inversion_pairs(targets)?;
            for ((a, inv_a), (b, _)) in pairs {
                let bit = reference_measure_pair_biased(state, lost, a, b, PauliBasis::Z, inv_a);
                measurements.push(bit);
            }
        }

        // SPP gates (no measurements, same logic as executor)
        "SPP" => {
            let products = split_pauli_products(targets)?;
            for product in &products {
                apply_spp(state, &product.terms, product.inverted, false);
            }
        }
        "SPP_DAG" => {
            let products = split_pauli_products(targets)?;
            for product in &products {
                apply_spp(state, &product.terms, product.inverted, true);
            }
        }

        // Heralded channels: push false per target (no herald in noiseless)
        "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1" => {
            for _q in qubits(targets)? {
                measurements.push(false);
            }
        }

        // Noise instructions: skip
        "X_ERROR"
        | "Z_ERROR"
        | "Y_ERROR"
        | "DEPOLARIZE1"
        | "DEPOLARIZE2"
        | "CORRELATED_ERROR"
        | "E"
        | "ELSE_CORRELATED_ERROR"
        | "PAULI_CHANNEL_1"
        | "PAULI_CHANNEL_2"
        | "I_ERROR"
        | "II_ERROR" => {}

        // Metadata: skip
        "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" | "DETECTOR" | "OBSERVABLE_INCLUDE" => {}

        _ => {
            return Err(format!(
                "reference_sample: unsupported instruction {}",
                name
            ));
        }
    }
    Ok(())
}

fn apply_reference_controlled_pairs(
    state: &mut StabilizerState,
    name: &str,
    targets: &[StimTarget],
    sweep_bits: Option<&[bool]>,
) -> Result<(), String> {
    if targets.len() % 2 != 0 {
        return Err("odd number of targets".to_string());
    }
    let mut it = targets.iter();
    while let (Some(a), Some(b)) = (it.next(), it.next()) {
        match (a, b) {
            (StimTarget::Qubit(c), StimTarget::Qubit(t)) => match name {
                "CX" | "CNOT" | "ZCX" => state.cx(*c as usize, *t as usize),
                "CY" | "ZCY" => state.cy(*c as usize, *t as usize),
                "CZ" | "ZCZ" => state.cz(*c as usize, *t as usize),
                _ => return Err(format!("unsupported reference pair op {name}")),
            },
            (StimTarget::Sweep(k), StimTarget::Qubit(q)) => {
                let active = sweep_bits
                    .and_then(|bits| bits.get(*k as usize))
                    .copied()
                    .unwrap_or(false);
                if active {
                    match name {
                        "CX" | "CNOT" | "ZCX" => state.x_gate(*q as usize),
                        "CY" | "ZCY" => state.y_gate(*q as usize),
                        "CZ" | "ZCZ" => state.z_gate(*q as usize),
                        _ => return Err(format!("unsupported reference pair op {name}")),
                    }
                }
            }
            (StimTarget::Sweep(_), _) | (_, StimTarget::Sweep(_)) => {
                return Err("unsupported sweep target placement".to_string());
            }
            _ => return Err("expected qubit target in pair".to_string()),
        }
    }
    Ok(())
}

fn measure_pauli_product_biased(
    state: &mut StabilizerState,
    terms: &[(usize, PauliBasis)],
    inverted: bool,
) -> bool {
    if terms.is_empty() {
        return inverted;
    }

    for &(q, basis) in terms {
        match basis {
            PauliBasis::X => state.h(q),
            PauliBasis::Y => state.h_yz(q),
            PauliBasis::Z => {}
        }
    }

    let anchor = terms.last().unwrap().0;
    let non_anchor: Vec<usize> = terms
        .iter()
        .map(|&(q, _)| q)
        .filter(|&q| q != anchor)
        .collect();
    for &q in &non_anchor {
        state.cx(q, anchor);
    }

    let bit = state.measure_z_biased(anchor);
    let result = (bit == 1) ^ inverted;

    for &q in non_anchor.iter().rev() {
        state.cx(q, anchor);
    }

    for &(q, basis) in terms {
        match basis {
            PauliBasis::X => state.h(q),
            PauliBasis::Y => state.h_yz(q),
            PauliBasis::Z => {}
        }
    }

    result
}

fn reference_measure_pauli_product_biased(
    state: &mut StabilizerState,
    lost: &[bool],
    terms: &[(usize, PauliBasis)],
    inverted: bool,
) -> bool {
    if terms.iter().any(|(q, _)| lost[*q]) {
        true
    } else {
        measure_pauli_product_biased(state, terms, inverted)
    }
}

fn reference_measure_pair_biased(
    state: &mut StabilizerState,
    lost: &[bool],
    a: usize,
    b: usize,
    basis: PauliBasis,
    inverted: bool,
) -> bool {
    if lost[a] || lost[b] {
        true ^ inverted
    } else {
        let terms = [(a, basis), (b, basis)];
        measure_pauli_product_biased(state, &terms, inverted)
    }
}

fn recorder_bits(r: Recorder) -> Vec<bool> {
    let mut out = Vec::new();
    for i in 1..=r.len() {
        out.push(r.rec(-(i as i32)).unwrap());
    }
    out.reverse();
    out
}

pub(crate) fn max_qubit(instrs: &[StimInstr]) -> Result<usize, String> {
    let mut max_q: Option<u32> = None;
    for i in instrs {
        match i {
            StimInstr::Op { targets, .. } => {
                for t in targets {
                    match t {
                        StimTarget::Qubit(q) | StimTarget::QubitInv(q) => {
                            max_q = Some(max_q.map_or(*q, |m| m.max(*q)));
                        }
                        StimTarget::Pauli { qubit: q, .. } => {
                            max_q = Some(max_q.map_or(*q, |m| m.max(*q)));
                        }
                        StimTarget::Combiner | StimTarget::Rec(_) | StimTarget::Sweep(_) => {}
                    }
                }
            }
            StimInstr::Repeat { body, .. } => {
                let inner = max_qubit(body)? as u32;
                max_q = Some(max_q.map_or(inner, |m| m.max(inner)));
            }
        }
    }
    Ok(max_q.map(|m| (m as usize) + 1).unwrap_or(0))
}

fn qubits(targets: &[StimTarget]) -> Result<Vec<usize>, String> {
    let mut out = Vec::new();
    for t in targets {
        match t {
            StimTarget::Qubit(q) => out.push(*q as usize),
            StimTarget::Sweep(_) => {} // skip: treated as always-0 (no-op)
            _ => out.push(expect_qubit(t)?),
        }
    }
    Ok(out)
}

fn qubits_with_inversion(targets: &[StimTarget]) -> Result<Vec<(usize, bool)>, String> {
    let mut out = Vec::new();
    for t in targets {
        match t {
            StimTarget::Qubit(q) => out.push((*q as usize, false)),
            StimTarget::QubitInv(q) => out.push((*q as usize, true)),
            StimTarget::Sweep(_) => {} // skip: treated as always-0 (no-op)
            _ => return Err("expected qubit target".to_string()),
        }
    }
    Ok(out)
}

fn qubits_with_inversion_pairs(
    targets: &[StimTarget],
) -> Result<Vec<((usize, bool), (usize, bool))>, String> {
    let flat = qubits_with_inversion(targets)?;
    if flat.len() % 2 != 0 {
        return Err("odd number of targets for pair measurement".to_string());
    }
    Ok(flat.chunks(2).map(|c| (c[0], c[1])).collect())
}

fn for_each_present_qubit<F: FnMut(usize)>(
    targets: &[StimTarget],
    lost: &[bool],
    mut f: F,
) -> Result<(), String> {
    for t in targets {
        let q = expect_qubit(t)?;
        if !lost[q] {
            f(q);
        }
    }
    Ok(())
}

fn expect_qubit(t: &StimTarget) -> Result<usize, String> {
    match t {
        StimTarget::Qubit(q) => Ok(*q as usize),
        StimTarget::QubitInv(_) => {
            Err("inverted qubit target only valid for measurement".to_string())
        }
        StimTarget::Sweep(_) => Err("sweep[] target unexpected here".to_string()),
        _ => Err("expected qubit target".to_string()),
    }
}

fn qubit_pairs(targets: &[StimTarget]) -> Result<Vec<(usize, usize)>, String> {
    if targets.len() % 2 != 0 {
        return Err("odd number of targets".to_string());
    }
    let mut out = Vec::new();
    let mut it = targets.iter();
    while let (Some(a), Some(b)) = (it.next(), it.next()) {
        // Skip any pair that contains a sweep target (sweep=0 means gate is no-op)
        if matches!(a, StimTarget::Sweep(_)) || matches!(b, StimTarget::Sweep(_)) {
            continue;
        }
        out.push((expect_qubit(a)?, expect_qubit(b)?));
    }
    Ok(out)
}

fn present_qubit_pairs(
    targets: &[StimTarget],
    lost: &[bool],
) -> Result<Vec<(usize, usize)>, String> {
    Ok(qubit_pairs(targets)?
        .into_iter()
        .filter(|(a, b)| !lost[*a] && !lost[*b])
        .collect())
}

fn xor_recs(r: &Recorder, targets: &[StimTarget]) -> Result<bool, String> {
    let mut acc = false;
    for t in targets {
        match t {
            StimTarget::Rec(o) => {
                let bit = r.rec(*o).ok_or("rec out of range")?;
                acc ^= bit;
            }
            _ => return Err("detector target must be rec".to_string()),
        }
    }
    Ok(acc)
}

fn apply_pauli_targets(state: &mut StabilizerState, targets: &[StimTarget]) -> Result<(), String> {
    for t in targets {
        match t {
            StimTarget::Pauli { qubit, basis, .. } => {
                let q = *qubit as usize;
                match basis {
                    PauliBasis::X => state.x_gate(q),
                    PauliBasis::Y => state.y_gate(q),
                    PauliBasis::Z => state.z_gate(q),
                }
            }
            _ => return Err("CORRELATED_ERROR targets must be Pauli".to_string()),
        }
    }
    Ok(())
}

fn apply_pauli(state: &mut StabilizerState, q: usize, p: u8) {
    match p {
        0 => {}
        1 => state.x_gate(q),
        2 => state.y_gate(q),
        3 => state.z_gate(q),
        _ => {}
    }
}

fn record_measure_z(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) {
    if lost[q] {
        recorder.push(true ^ inv);
        return;
    }
    let (bit, _) = state.measure_z(q, rng);
    recorder.push((bit == 1) ^ inv);
}

fn record_measure_x(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) {
    if lost[q] {
        recorder.push(true ^ inv);
        return;
    }
    state.h(q);
    let (bit, _) = state.measure_z(q, rng);
    state.h(q);
    recorder.push((bit == 1) ^ inv);
}

fn record_measure_y(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) {
    if lost[q] {
        recorder.push(true ^ inv);
        return;
    }
    state.s_dag(q);
    state.h(q);
    let (bit, _) = state.measure_z(q, rng);
    state.h(q);
    state.s(q);
    recorder.push((bit == 1) ^ inv);
}

fn measure_reset_z(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) {
    if lost[q] {
        recorder.push(true ^ inv);
        lost[q] = false;
        state.reset_z(q, rng);
        return;
    }
    let (bit, _) = state.measure_z(q, rng);
    recorder.push((bit == 1) ^ inv);
    if bit == 1 {
        state.x_gate(q);
    }
}

fn measure_reset_x(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) {
    if lost[q] {
        recorder.push(true ^ inv);
        lost[q] = false;
        state.reset_x(q, rng);
        return;
    }
    state.h(q);
    let (bit, _) = state.measure_z(q, rng);
    recorder.push((bit == 1) ^ inv);
    if bit == 1 {
        state.x_gate(q);
    }
    state.h(q);
}

fn measure_reset_y(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) {
    if lost[q] {
        recorder.push(true ^ inv);
        lost[q] = false;
        state.reset_y(q, rng);
        return;
    }
    state.s_dag(q);
    state.h(q);
    let (bit, _) = state.measure_z(q, rng);
    recorder.push((bit == 1) ^ inv);
    if bit == 1 {
        state.x_gate(q);
    }
    state.h(q);
    state.s(q);
}

fn record_loss_visible_measure_z(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) {
    recorder.push(lost[q]);
    record_measure_z(state, lost, q, inv, rng, recorder);
}

fn record_loss_visible_measure_x(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) {
    recorder.push(lost[q]);
    record_measure_x(state, lost, q, inv, rng, recorder);
}

fn record_loss_visible_measure_y(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) {
    recorder.push(lost[q]);
    record_measure_y(state, lost, q, inv, rng, recorder);
}

fn measure_reset_loss_visible_z(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) {
    recorder.push(lost[q]);
    measure_reset_z(state, lost, q, inv, rng, recorder);
}

fn measure_reset_loss_visible_x(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) {
    recorder.push(lost[q]);
    measure_reset_x(state, lost, q, inv, rng, recorder);
}

fn measure_reset_loss_visible_y(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) {
    recorder.push(lost[q]);
    measure_reset_y(state, lost, q, inv, rng, recorder);
}

fn record_reference_measure_z(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    if lost[q] {
        measurements.push(true ^ inv);
        return;
    }
    let bit = state.measure_z_biased(q);
    measurements.push((bit == 1) ^ inv);
}

fn record_reference_measure_x(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    if lost[q] {
        measurements.push(true ^ inv);
        return;
    }
    state.h(q);
    let bit = state.measure_z_biased(q);
    state.h(q);
    measurements.push((bit == 1) ^ inv);
}

fn record_reference_measure_y(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    if lost[q] {
        measurements.push(true ^ inv);
        return;
    }
    state.s_dag(q);
    state.h(q);
    let bit = state.measure_z_biased(q);
    state.h(q);
    state.s(q);
    measurements.push((bit == 1) ^ inv);
}

fn reference_measure_reset_z(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    if lost[q] {
        measurements.push(true ^ inv);
        lost[q] = false;
        state.reset_z_biased(q);
        return;
    }
    let bit = state.measure_z_biased(q);
    measurements.push((bit == 1) ^ inv);
    if bit == 1 {
        state.x_gate(q);
    }
}

fn reference_measure_reset_x(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    if lost[q] {
        measurements.push(true ^ inv);
        lost[q] = false;
        state.reset_x_biased(q);
        return;
    }
    state.h(q);
    let bit = state.measure_z_biased(q);
    measurements.push((bit == 1) ^ inv);
    if bit == 1 {
        state.x_gate(q);
    }
    state.h(q);
}

fn reference_measure_reset_y(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    if lost[q] {
        measurements.push(true ^ inv);
        lost[q] = false;
        state.reset_y_biased(q);
        return;
    }
    state.s_dag(q);
    state.h(q);
    let bit = state.measure_z_biased(q);
    measurements.push((bit == 1) ^ inv);
    if bit == 1 {
        state.x_gate(q);
    }
    state.h(q);
    state.s(q);
}

fn record_reference_loss_visible_measure_z(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    measurements.push(lost[q]);
    record_reference_measure_z(state, lost, q, inv, measurements);
}

fn record_reference_loss_visible_measure_x(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    measurements.push(lost[q]);
    record_reference_measure_x(state, lost, q, inv, measurements);
}

fn record_reference_loss_visible_measure_y(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    measurements.push(lost[q]);
    record_reference_measure_y(state, lost, q, inv, measurements);
}

fn reference_measure_reset_loss_visible_z(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    measurements.push(lost[q]);
    reference_measure_reset_z(state, lost, q, inv, measurements);
}

fn reference_measure_reset_loss_visible_x(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    measurements.push(lost[q]);
    reference_measure_reset_x(state, lost, q, inv, measurements);
}

fn reference_measure_reset_loss_visible_y(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    measurements.push(lost[q]);
    reference_measure_reset_y(state, lost, q, inv, measurements);
}

fn two_qubit_pauli(r: usize) -> (u8, u8) {
    // Map 0..14 to 15 non-identity pairs from {I,X,Y,Z}^2 \ {II}
    let mut idx = 0usize;
    for a in 0..4 {
        for b in 0..4 {
            if a == 0 && b == 0 {
                continue;
            }
            if idx == r {
                return (a as u8, b as u8);
            }
            idx += 1;
        }
    }
    (0, 0)
}

struct PauliProduct {
    terms: Vec<(usize, PauliBasis)>,
    inverted: bool,
}

fn split_pauli_products(targets: &[StimTarget]) -> Result<Vec<PauliProduct>, String> {
    let mut products = Vec::new();
    let mut current_terms: Vec<(usize, PauliBasis)> = Vec::new();
    let mut inverted = false;
    let mut after_combiner = false;

    for target in targets {
        match target {
            StimTarget::Pauli {
                qubit,
                basis,
                inverted: inv,
            } => {
                if !after_combiner && !current_terms.is_empty() {
                    products.push(PauliProduct {
                        terms: std::mem::take(&mut current_terms),
                        inverted,
                    });
                    inverted = false;
                }
                if current_terms.is_empty() && *inv {
                    inverted = true;
                }
                current_terms.push((*qubit as usize, *basis));
                after_combiner = false;
            }
            StimTarget::Combiner => {
                after_combiner = true;
            }
            _ => return Err("MPP targets must be Pauli targets".to_string()),
        }
    }
    if !current_terms.is_empty() {
        products.push(PauliProduct {
            terms: current_terms,
            inverted,
        });
    }
    Ok(products)
}

fn measure_pauli_product(
    state: &mut StabilizerState,
    terms: &[(usize, PauliBasis)],
    inverted: bool,
    rng: &mut impl Rng,
) -> bool {
    if terms.is_empty() {
        return inverted;
    }

    // Basis change: X→Z via H, Y→Z via H_YZ
    for &(q, basis) in terms {
        match basis {
            PauliBasis::X => state.h(q),
            PauliBasis::Y => state.h_yz(q),
            PauliBasis::Z => {}
        }
    }

    // CX fold: chain all qubits' Z parity onto anchor (last qubit)
    let anchor = terms.last().unwrap().0;
    let non_anchor: Vec<usize> = terms
        .iter()
        .map(|&(q, _)| q)
        .filter(|&q| q != anchor)
        .collect();
    for &q in &non_anchor {
        state.cx(q, anchor);
    }

    // Measure anchor in Z basis
    let (bit, _) = state.measure_z(anchor, rng);
    let result = (bit == 1) ^ inverted;

    // Uncompute CX (reverse order, CX is self-inverse)
    for &q in non_anchor.iter().rev() {
        state.cx(q, anchor);
    }

    // Undo basis change (H and H_YZ are self-inverse)
    for &(q, basis) in terms {
        match basis {
            PauliBasis::X => state.h(q),
            PauliBasis::Y => state.h_yz(q),
            PauliBasis::Z => {}
        }
    }

    result
}

fn apply_spp(
    state: &mut StabilizerState,
    terms: &[(usize, PauliBasis)],
    inverted: bool,
    dag: bool,
) {
    if terms.is_empty() {
        return;
    }

    for &(q, basis) in terms {
        match basis {
            PauliBasis::X => state.h(q),
            PauliBasis::Y => state.h_yz(q),
            PauliBasis::Z => {}
        }
    }

    let anchor = terms.last().unwrap().0;
    let non_anchor: Vec<usize> = terms
        .iter()
        .map(|&(q, _)| q)
        .filter(|&q| q != anchor)
        .collect();
    for &q in &non_anchor {
        state.cx(q, anchor);
    }

    if dag ^ inverted {
        state.s_dag(anchor);
    } else {
        state.s(anchor);
    }

    for &q in non_anchor.iter().rev() {
        state.cx(q, anchor);
    }

    for &(q, basis) in terms {
        match basis {
            PauliBasis::X => state.h(q),
            PauliBasis::Y => state.h_yz(q),
            PauliBasis::Z => {}
        }
    }
}

fn pair_measure(
    state: &mut StabilizerState,
    lost: &[bool],
    targets: &[StimTarget],
    basis: PauliBasis,
    noise_p: f64,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) -> Result<(), String> {
    let pairs = qubits_with_inversion_pairs(targets)?;
    for ((a, inv_a), (b, _inv_b)) in pairs {
        let mut bit = if lost[a] || lost[b] {
            true ^ inv_a
        } else {
            let terms = vec![(a, basis), (b, basis)];
            measure_pauli_product(state, &terms, inv_a, rng)
        };
        if noise_p > 0.0 && rng.r#gen::<f64>() < noise_p {
            bit = !bit;
        }
        recorder.push(bit);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_measurement_helpers_return_one_for_lost_qubits() {
        let mut state = StabilizerState::new(1);
        let lost = vec![true];
        let mut measurements = Vec::new();

        record_reference_measure_z(&mut state, &lost, 0, false, &mut measurements);
        record_reference_measure_x(&mut state, &lost, 0, true, &mut measurements);
        record_reference_measure_y(&mut state, &lost, 0, false, &mut measurements);

        assert_eq!(measurements, vec![true, false, true]);
    }

    #[test]
    fn reference_measure_reset_helpers_clear_loss_and_reset_state() {
        let mut state = StabilizerState::new(3);
        let mut lost = vec![true, true, true];
        let mut measurements = Vec::new();

        reference_measure_reset_z(&mut state, &mut lost, 0, false, &mut measurements);
        reference_measure_reset_x(&mut state, &mut lost, 1, true, &mut measurements);
        reference_measure_reset_y(&mut state, &mut lost, 2, false, &mut measurements);

        assert_eq!(measurements, vec![true, false, true]);
        assert_eq!(lost, vec![false, false, false]);

        let mut post_reset_measurements = Vec::new();
        record_reference_measure_z(&mut state, &lost, 0, false, &mut post_reset_measurements);
        record_reference_measure_x(&mut state, &lost, 1, false, &mut post_reset_measurements);
        record_reference_measure_y(&mut state, &lost, 2, false, &mut post_reset_measurements);
        assert_eq!(post_reset_measurements, vec![false, false, false]);
    }

    #[test]
    fn reference_measure_reset_x_and_y_correct_one_outcomes() {
        let mut state = StabilizerState::new(2);
        let mut lost = vec![false, false];
        let mut measurements = Vec::new();

        state.h(0);
        state.z_gate(0);
        reference_measure_reset_x(&mut state, &mut lost, 0, false, &mut measurements);

        state.h(1);
        state.s_dag(1);
        reference_measure_reset_y(&mut state, &mut lost, 1, false, &mut measurements);

        assert_eq!(measurements, vec![true, true]);

        let mut post_reset_measurements = Vec::new();
        record_reference_measure_x(&mut state, &lost, 0, false, &mut post_reset_measurements);
        record_reference_measure_y(&mut state, &lost, 1, false, &mut post_reset_measurements);
        assert_eq!(post_reset_measurements, vec![false, false]);
    }

    #[test]
    fn reference_multi_pauli_measurements_report_loss() {
        let mut state = StabilizerState::new(2);
        let lost = vec![true, false];

        assert!(reference_measure_pauli_product_biased(
            &mut state,
            &lost,
            &[(0, PauliBasis::X), (1, PauliBasis::Y)],
            true,
        ));
        assert!(reference_measure_pair_biased(
            &mut state,
            &lost,
            0,
            1,
            PauliBasis::X,
            false,
        ));
        assert!(!reference_measure_pair_biased(
            &mut state,
            &lost,
            0,
            1,
            PauliBasis::Y,
            true,
        ));
        assert!(reference_measure_pair_biased(
            &mut state,
            &lost,
            0,
            1,
            PauliBasis::Z,
            false,
        ));
    }
}
