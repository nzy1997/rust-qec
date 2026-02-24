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
        let mut recorder = Recorder::default();
        let mut detectors = Vec::new();
        let mut detector_coords = Vec::new();
        let mut observables = Vec::new();
        let mut coords = CoordState::default();
        let mut last_correlated_error_occurred = false;

        for instr in &self.instrs {
            match instr {
                StimInstr::Op { name, args, targets, .. } => {
                    match name.as_str() {
                        "I" | "I_ERROR" | "II_ERROR" => {}
                        "H" => for_each_qubit(targets, |q| state.h(q))?,
                        "H_XY" => for_each_qubit(targets, |q| state.h_xy(q))?,
                        "H_YZ" => for_each_qubit(targets, |q| state.h_yz(q))?,
                        "S" | "SQRT_Z" => for_each_qubit(targets, |q| state.s(q))?,
                        "S_DAG" | "SQRT_Z_DAG" => for_each_qubit(targets, |q| state.s_dag(q))?,
                        "SQRT_X" => for_each_qubit(targets, |q| state.sqrt_x(q))?,
                        "SQRT_X_DAG" => for_each_qubit(targets, |q| state.sqrt_x_dag(q))?,
                        "SQRT_Y" => for_each_qubit(targets, |q| state.sqrt_y(q))?,
                        "SQRT_Y_DAG" => for_each_qubit(targets, |q| state.sqrt_y_dag(q))?,
                        "X" => for_each_qubit(targets, |q| state.x_gate(q))?,
                        "Y" => for_each_qubit(targets, |q| state.y_gate(q))?,
                        "Z" => for_each_qubit(targets, |q| state.z_gate(q))?,
                        "CX" | "CNOT" | "ZCX" => {
                            let pairs = qubit_pairs(targets)?;
                            for (c, t) in pairs { state.cx(c, t); }
                        }
                        "CY" | "ZCY" => {
                            let pairs = qubit_pairs(targets)?;
                            for (c, t) in pairs { state.cy(c, t); }
                        }
                        "CZ" | "ZCZ" => {
                            let pairs = qubit_pairs(targets)?;
                            for (c, t) in pairs { state.cz(c, t); }
                        }
                        "XCX" => {
                            let pairs = qubit_pairs(targets)?;
                            for (a, b) in pairs { state.xcx(a, b); }
                        }
                        "XCY" => {
                            let pairs = qubit_pairs(targets)?;
                            for (a, b) in pairs { state.xcy(a, b); }
                        }
                        "XCZ" => {
                            let pairs = qubit_pairs(targets)?;
                            for (a, b) in pairs { state.xcz(a, b); }
                        }
                        "YCX" => {
                            let pairs = qubit_pairs(targets)?;
                            for (a, b) in pairs { state.ycx(a, b); }
                        }
                        "YCY" => {
                            let pairs = qubit_pairs(targets)?;
                            for (a, b) in pairs { state.ycy(a, b); }
                        }
                        "YCZ" => {
                            let pairs = qubit_pairs(targets)?;
                            for (a, b) in pairs { state.ycz(a, b); }
                        }
                        "SWAP" => {
                            let pairs = qubit_pairs(targets)?;
                            for (a, b) in pairs { state.swap(a, b); }
                        }
                        "ISWAP" => {
                            let pairs = qubit_pairs(targets)?;
                            for (a, b) in pairs { state.iswap(a, b); }
                        }
                        "ISWAP_DAG" => {
                            let pairs = qubit_pairs(targets)?;
                            for (a, b) in pairs { state.iswap_dag(a, b); }
                        }
                        "CXSWAP" => {
                            let pairs = qubit_pairs(targets)?;
                            for (a, b) in pairs { state.cxswap(a, b); }
                        }
                        "SWAPCX" => {
                            let pairs = qubit_pairs(targets)?;
                            for (a, b) in pairs { state.swapcx(a, b); }
                        }
                        "CZSWAP" => {
                            let pairs = qubit_pairs(targets)?;
                            for (a, b) in pairs { state.czswap(a, b); }
                        }
                        "M" | "MZ" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                let (bit, _) = state.measure_z(q, rng);
                                let b = (bit == 1) ^ inv;
                                recorder.push(b);
                            }
                        }
                        "MX" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                state.h(q);
                                let (bit, _) = state.measure_z(q, rng);
                                state.h(q);
                                let b = (bit == 1) ^ inv;
                                recorder.push(b);
                            }
                        }
                        "MY" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                state.s_dag(q);
                                state.h(q);
                                let (bit, _) = state.measure_z(q, rng);
                                state.h(q);
                                state.s(q);
                                let b = (bit == 1) ^ inv;
                                recorder.push(b);
                            }
                        }
                        "MR" | "MRZ" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                let (bit, _) = state.measure_z(q, rng);
                                let b = (bit == 1) ^ inv;
                                recorder.push(b);
                                if bit == 1 { state.x_gate(q); }
                            }
                        }
                        "MRX" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                state.h(q);
                                let (bit, _) = state.measure_z(q, rng);
                                let b = (bit == 1) ^ inv;
                                recorder.push(b);
                                if bit == 1 { state.x_gate(q); }
                                state.h(q);
                            }
                        }
                        "MRY" => {
                            for (q, inv) in qubits_with_inversion(targets)? {
                                state.s_dag(q);
                                state.h(q);
                                let (bit, _) = state.measure_z(q, rng);
                                let b = (bit == 1) ^ inv;
                                recorder.push(b);
                                if bit == 1 { state.x_gate(q); }
                                state.h(q);
                                state.s(q);
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
                            for q in qubits(targets)? { state.reset_z(q, rng); }
                        }
                        "RX" => {
                            for q in qubits(targets)? { state.reset_x(q, rng); }
                        }
                        "RY" => {
                            for q in qubits(targets)? { state.reset_y(q, rng); }
                        }
                        "X_ERROR" => {
                            let p = args.get(0).copied().unwrap_or(0.0);
                            for q in qubits(targets)? {
                                if rng.r#gen::<f64>() < p {
                                    state.x_gate(q);
                                }
                            }
                        }
                        "Y_ERROR" => {
                            let p = args.get(0).copied().unwrap_or(0.0);
                            for q in qubits(targets)? {
                                if rng.r#gen::<f64>() < p {
                                    state.y_gate(q);
                                }
                            }
                        }
                        "Z_ERROR" => {
                            let p = args.get(0).copied().unwrap_or(0.0);
                            for q in qubits(targets)? {
                                if rng.r#gen::<f64>() < p {
                                    state.z_gate(q);
                                }
                            }
                        }
                        "DEPOLARIZE1" => {
                            let p = args.get(0).copied().unwrap_or(0.0);
                            for q in qubits(targets)? {
                                if rng.r#gen::<f64>() < p {
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
                            let pairs = qubit_pairs(targets)?;
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
                            pair_measure(&mut state, targets, PauliBasis::X, p, rng, &mut recorder)?;
                        }
                        "MYY" => {
                            let p = args.first().copied().unwrap_or(0.0);
                            pair_measure(&mut state, targets, PauliBasis::Y, p, rng, &mut recorder)?;
                        }
                        "MZZ" => {
                            let p = args.first().copied().unwrap_or(0.0);
                            pair_measure(&mut state, targets, PauliBasis::Z, p, rng, &mut recorder)?;
                        }
                        "MPP" => {
                            let p = args.first().copied().unwrap_or(0.0);
                            let products = split_pauli_products(targets)?;
                            for product in &products {
                                let mut bit = measure_pauli_product(
                                    &mut state,
                                    &product.terms,
                                    product.inverted,
                                    rng,
                                );
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
                        "PAULI_CHANNEL_2" => {
                            let probs: Vec<f64> = (0..15).map(|i| args.get(i).copied().unwrap_or(0.0)).collect();
                            let pairs = qubit_pairs(targets)?;
                            let paulis: [(u8, u8); 15] = [
                                (0, 1), (0, 2), (0, 3),
                                (1, 0), (1, 1), (1, 2), (1, 3),
                                (2, 0), (2, 1), (2, 2), (2, 3),
                                (3, 0), (3, 1), (3, 2), (3, 3),
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
    let n = max_qubit(instrs)?;
    let mut state = StabilizerState::new(n);
    let mut measurements = Vec::new();
    ref_sample_instrs(&mut state, &mut measurements, instrs)?;
    Ok(measurements)
}

fn ref_sample_instrs(
    state: &mut StabilizerState,
    measurements: &mut Vec<bool>,
    instrs: &[StimInstr],
) -> Result<(), String> {
    for instr in instrs {
        match instr {
            StimInstr::Op { name, args, targets, .. } => {
                ref_sample_op(state, measurements, name.as_str(), args, targets)?;
            }
            StimInstr::Repeat { count, body } => {
                for _ in 0..*count {
                    ref_sample_instrs(state, measurements, body)?;
                }
            }
        }
    }
    Ok(())
}

fn ref_sample_op(
    state: &mut StabilizerState,
    measurements: &mut Vec<bool>,
    name: &str,
    _args: &[f64],
    targets: &[StimTarget],
) -> Result<(), String> {
    match name {
        // Identity
        "I" => {}

        // Single-qubit Cliffords
        "H" => for_each_qubit(targets, |q| state.h(q))?,
        "H_XY" => for_each_qubit(targets, |q| state.h_xy(q))?,
        "H_YZ" => for_each_qubit(targets, |q| state.h_yz(q))?,
        "S" | "SQRT_Z" => for_each_qubit(targets, |q| state.s(q))?,
        "S_DAG" | "SQRT_Z_DAG" => for_each_qubit(targets, |q| state.s_dag(q))?,
        "SQRT_X" => for_each_qubit(targets, |q| state.sqrt_x(q))?,
        "SQRT_X_DAG" => for_each_qubit(targets, |q| state.sqrt_x_dag(q))?,
        "SQRT_Y" => for_each_qubit(targets, |q| state.sqrt_y(q))?,
        "SQRT_Y_DAG" => for_each_qubit(targets, |q| state.sqrt_y_dag(q))?,
        "X" => for_each_qubit(targets, |q| state.x_gate(q))?,
        "Y" => for_each_qubit(targets, |q| state.y_gate(q))?,
        "Z" => for_each_qubit(targets, |q| state.z_gate(q))?,

        // Two-qubit Cliffords
        "CX" | "CNOT" | "ZCX" => {
            for (c, t) in qubit_pairs(targets)? { state.cx(c, t); }
        }
        "CY" | "ZCY" => {
            for (c, t) in qubit_pairs(targets)? { state.cy(c, t); }
        }
        "CZ" | "ZCZ" => {
            for (a, b) in qubit_pairs(targets)? { state.cz(a, b); }
        }
        "XCX" => {
            for (a, b) in qubit_pairs(targets)? { state.xcx(a, b); }
        }
        "XCY" => {
            for (a, b) in qubit_pairs(targets)? { state.xcy(a, b); }
        }
        "XCZ" => {
            for (a, b) in qubit_pairs(targets)? { state.xcz(a, b); }
        }
        "YCX" => {
            for (a, b) in qubit_pairs(targets)? { state.ycx(a, b); }
        }
        "YCY" => {
            for (a, b) in qubit_pairs(targets)? { state.ycy(a, b); }
        }
        "YCZ" => {
            for (a, b) in qubit_pairs(targets)? { state.ycz(a, b); }
        }
        "SWAP" => {
            for (a, b) in qubit_pairs(targets)? { state.swap(a, b); }
        }
        "ISWAP" => {
            for (a, b) in qubit_pairs(targets)? { state.iswap(a, b); }
        }
        "ISWAP_DAG" => {
            for (a, b) in qubit_pairs(targets)? { state.iswap_dag(a, b); }
        }
        "CXSWAP" => {
            for (a, b) in qubit_pairs(targets)? { state.cxswap(a, b); }
        }
        "SWAPCX" => {
            for (a, b) in qubit_pairs(targets)? { state.swapcx(a, b); }
        }
        "CZSWAP" => {
            for (a, b) in qubit_pairs(targets)? { state.czswap(a, b); }
        }

        // Measurements (biased)
        "M" | "MZ" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                let bit = state.measure_z_biased(q);
                measurements.push((bit == 1) ^ inv);
            }
        }
        "MX" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                state.h(q);
                let bit = state.measure_z_biased(q);
                state.h(q);
                measurements.push((bit == 1) ^ inv);
            }
        }
        "MY" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                state.s_dag(q);
                state.h(q);
                let bit = state.measure_z_biased(q);
                state.h(q);
                state.s(q);
                measurements.push((bit == 1) ^ inv);
            }
        }
        "MR" | "MRZ" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                let bit = state.measure_z_biased(q);
                measurements.push((bit == 1) ^ inv);
                if bit == 1 { state.x_gate(q); }
            }
        }
        "MRX" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                state.h(q);
                let bit = state.measure_z_biased(q);
                measurements.push((bit == 1) ^ inv);
                if bit == 1 { state.x_gate(q); }
                state.h(q);
            }
        }
        "MRY" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                state.s_dag(q);
                state.h(q);
                let bit = state.measure_z_biased(q);
                measurements.push((bit == 1) ^ inv);
                if bit == 1 { state.x_gate(q); }
                state.h(q);
                state.s(q);
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
            for q in qubits(targets)? { state.reset_z_biased(q); }
        }
        "RX" => {
            for q in qubits(targets)? { state.reset_x_biased(q); }
        }
        "RY" => {
            for q in qubits(targets)? { state.reset_y_biased(q); }
        }

        // Multi-qubit Pauli measurements (biased)
        "MPP" => {
            let products = split_pauli_products(targets)?;
            for product in &products {
                let bit = measure_pauli_product_biased(
                    state,
                    &product.terms,
                    product.inverted,
                );
                measurements.push(bit);
            }
        }
        "MXX" => {
            let pairs = qubits_with_inversion_pairs(targets)?;
            for ((a, inv_a), (b, _)) in pairs {
                let terms = vec![(a, PauliBasis::X), (b, PauliBasis::X)];
                let bit = measure_pauli_product_biased(state, &terms, inv_a);
                measurements.push(bit);
            }
        }
        "MYY" => {
            let pairs = qubits_with_inversion_pairs(targets)?;
            for ((a, inv_a), (b, _)) in pairs {
                let terms = vec![(a, PauliBasis::Y), (b, PauliBasis::Y)];
                let bit = measure_pauli_product_biased(state, &terms, inv_a);
                measurements.push(bit);
            }
        }
        "MZZ" => {
            let pairs = qubits_with_inversion_pairs(targets)?;
            for ((a, inv_a), (b, _)) in pairs {
                let terms = vec![(a, PauliBasis::Z), (b, PauliBasis::Z)];
                let bit = measure_pauli_product_biased(state, &terms, inv_a);
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
        "X_ERROR" | "Z_ERROR" | "Y_ERROR"
        | "DEPOLARIZE1" | "DEPOLARIZE2"
        | "CORRELATED_ERROR" | "E" | "ELSE_CORRELATED_ERROR"
        | "PAULI_CHANNEL_1" | "PAULI_CHANNEL_2"
        | "I_ERROR" | "II_ERROR" => {}

        // Metadata: skip
        "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS"
        | "DETECTOR" | "OBSERVABLE_INCLUDE" => {}

        _ => return Err(format!("reference_sample: unsupported instruction {}", name)),
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
    let non_anchor: Vec<usize> = terms.iter().map(|&(q, _)| q).filter(|&q| q != anchor).collect();
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
                        StimTarget::Combiner | StimTarget::Rec(_) => {}
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
        out.push(expect_qubit(t)?);
    }
    Ok(out)
}

fn qubits_with_inversion(targets: &[StimTarget]) -> Result<Vec<(usize, bool)>, String> {
    let mut out = Vec::new();
    for t in targets {
        match t {
            StimTarget::Qubit(q) => out.push((*q as usize, false)),
            StimTarget::QubitInv(q) => out.push((*q as usize, true)),
            _ => return Err("expected qubit target".to_string()),
        }
    }
    Ok(out)
}

fn qubits_with_inversion_pairs(targets: &[StimTarget]) -> Result<Vec<((usize, bool), (usize, bool))>, String> {
    let flat = qubits_with_inversion(targets)?;
    if flat.len() % 2 != 0 {
        return Err("odd number of targets for pair measurement".to_string());
    }
    Ok(flat.chunks(2).map(|c| (c[0], c[1])).collect())
}

fn for_each_qubit<F: FnMut(usize)>(targets: &[StimTarget], mut f: F) -> Result<(), String> {
    for t in targets {
        f(expect_qubit(t)?);
    }
    Ok(())
}

fn expect_qubit(t: &StimTarget) -> Result<usize, String> {
    match t {
        StimTarget::Qubit(q) => Ok(*q as usize),
        StimTarget::QubitInv(_) => Err("inverted qubit target only valid for measurement".to_string()),
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
        out.push((expect_qubit(a)?, expect_qubit(b)?));
    }
    Ok(out)
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
            StimTarget::Pauli { qubit, basis, inverted: inv } => {
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
        products.push(PauliProduct { terms: current_terms, inverted });
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
    let non_anchor: Vec<usize> = terms.iter().map(|&(q, _)| q).filter(|&q| q != anchor).collect();
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
    let non_anchor: Vec<usize> = terms.iter().map(|&(q, _)| q).filter(|&q| q != anchor).collect();
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
    targets: &[StimTarget],
    basis: PauliBasis,
    noise_p: f64,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) -> Result<(), String> {
    let pairs = qubits_with_inversion_pairs(targets)?;
    for ((a, inv_a), (b, _inv_b)) in pairs {
        let terms = vec![(a, basis), (b, basis)];
        let mut bit = measure_pauli_product(state, &terms, inv_a, rng);
        if noise_p > 0.0 && rng.r#gen::<f64>() < noise_p {
            bit = !bit;
        }
        recorder.push(bit);
    }
    Ok(())
}
