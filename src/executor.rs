use rand::Rng;

use crate::coords::CoordState;
use crate::ir::{StimInstr, StimTarget};
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

        for instr in &self.instrs {
            match instr {
                StimInstr::Op { name, args, targets, .. } => {
                    match name.as_str() {
                        "H" => for_each_qubit(targets, |q| state.h(q))?,
                        "S" => for_each_qubit(targets, |q| state.s(q))?,
                        "X" => for_each_qubit(targets, |q| state.x_gate(q))?,
                        "Y" => for_each_qubit(targets, |q| state.y_gate(q))?,
                        "Z" => for_each_qubit(targets, |q| state.z_gate(q))?,
                        "CX" | "CNOT" => {
                            let pairs = qubit_pairs(targets)?;
                            for (c, t) in pairs {
                                state.cx(c, t);
                            }
                        }
                        "CZ" => {
                            let pairs = qubit_pairs(targets)?;
                            for (c, t) in pairs {
                                state.cz(c, t);
                            }
                        }
                        "M" => {
                            for q in qubits(targets)? {
                                let (bit, _) = state.measure_z(q, rng);
                                recorder.push(bit == 1);
                            }
                        }
                        "MX" => {
                            for q in qubits(targets)? {
                                state.h(q);
                                let (bit, _) = state.measure_z(q, rng);
                                state.h(q);
                                recorder.push(bit == 1);
                            }
                        }
                        "MY" => {
                            for q in qubits(targets)? {
                                state.s_dag(q);
                                state.h(q);
                                let (bit, _) = state.measure_z(q, rng);
                                state.h(q);
                                state.s(q);
                                recorder.push(bit == 1);
                            }
                        }
                        "X_ERROR" => {
                            let p = args.get(0).copied().unwrap_or(0.0);
                            for q in qubits(targets)? {
                                if rng.r#gen::<f64>() < p {
                                    state.x_gate(q);
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

fn recorder_bits(r: Recorder) -> Vec<bool> {
    let mut out = Vec::new();
    for i in 1..=r.len() {
        out.push(r.rec(-(i as i32)).unwrap());
    }
    out.reverse();
    out
}

fn max_qubit(instrs: &[StimInstr]) -> Result<usize, String> {
    let mut max_q: Option<u32> = None;
    for i in instrs {
        match i {
            StimInstr::Op { targets, .. } => {
                for t in targets {
                    if let StimTarget::Qubit(q) = t {
                        max_q = Some(max_q.map_or(*q, |m| m.max(*q)));
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

fn for_each_qubit<F: FnMut(usize)>(targets: &[StimTarget], mut f: F) -> Result<(), String> {
    for t in targets {
        f(expect_qubit(t)?);
    }
    Ok(())
}

fn expect_qubit(t: &StimTarget) -> Result<usize, String> {
    match t {
        StimTarget::Qubit(q) => Ok(*q as usize),
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
