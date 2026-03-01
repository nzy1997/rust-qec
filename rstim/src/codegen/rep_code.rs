use crate::ir::{StimInstr, StimTarget};
use super::NoiseParams;

/// Generate a repetition code memory experiment circuit (uniform noise convenience wrapper).
pub fn repetition_code_memory(distance: usize, rounds: usize, noise: f64) -> Vec<StimInstr> {
    repetition_code_memory_with_params(distance, rounds, NoiseParams::uniform(noise))
}

/// Generate a repetition code memory experiment circuit with per-channel noise.
///
/// Layout: d data qubits (0..d-1) interleaved with d-1 ancilla qubits (d..2d-2).
/// Each round: reset ancillas, CNOT from data[i] and data[i+1] to ancilla[i],
/// measure ancillas, compare with previous round via DETECTOR.
/// Final round: measure all data qubits, create detectors and observable.
pub fn repetition_code_memory_with_params(distance: usize, rounds: usize, params: NoiseParams) -> Vec<StimInstr> {
    assert!(distance >= 2, "distance must be >= 2");
    assert!(rounds >= 1, "rounds must be >= 1");

    let n_data = distance;
    let n_ancilla = distance - 1;
    let mut instrs = Vec::new();

    let data: Vec<u32> = (0..n_data as u32).collect();
    let ancilla: Vec<u32> = (n_data as u32..(n_data + n_ancilla) as u32).collect();

    for &q in data.iter().chain(ancilla.iter()) {
        instrs.push(op("R", &[], &[StimTarget::Qubit(q)]));
    }

    for (i, &q) in data.iter().enumerate() {
        instrs.push(op("QUBIT_COORDS", &[2.0 * i as f64, 0.0], &[StimTarget::Qubit(q)]));
    }
    for (i, &q) in ancilla.iter().enumerate() {
        instrs.push(op("QUBIT_COORDS", &[2.0 * i as f64 + 1.0, 0.0], &[StimTarget::Qubit(q)]));
    }

    for round in 0..rounds {
        instrs.push(op("TICK", &[], &[]));

        for &a in &ancilla {
            instrs.push(op("R", &[], &[StimTarget::Qubit(a)]));
        }
        // after_reset_flip_probability: X_ERROR after ancilla reset
        if params.after_reset_flip_probability > 0.0 {
            for &a in &ancilla {
                instrs.push(op("X_ERROR", &[params.after_reset_flip_probability], &[StimTarget::Qubit(a)]));
            }
        }

        // before_round_data_depolarization: DEPOLARIZE1 on data qubits at start of round
        if params.before_round_data_depolarization > 0.0 {
            for &d in &data {
                instrs.push(op("DEPOLARIZE1", &[params.before_round_data_depolarization], &[StimTarget::Qubit(d)]));
            }
        }

        for (i, &a) in ancilla.iter().enumerate() {
            instrs.push(op("CX", &[], &[StimTarget::Qubit(data[i]), StimTarget::Qubit(a)]));
        }
        // after_clifford_depolarization: DEPOLARIZE2 after first CX layer
        if params.after_clifford_depolarization > 0.0 {
            for (i, &a) in ancilla.iter().enumerate() {
                instrs.push(op("DEPOLARIZE2", &[params.after_clifford_depolarization], &[StimTarget::Qubit(data[i]), StimTarget::Qubit(a)]));
            }
        }

        for (i, &a) in ancilla.iter().enumerate() {
            instrs.push(op("CX", &[], &[StimTarget::Qubit(data[i + 1]), StimTarget::Qubit(a)]));
        }
        // after_clifford_depolarization: DEPOLARIZE2 after second CX layer
        if params.after_clifford_depolarization > 0.0 {
            for (i, &a) in ancilla.iter().enumerate() {
                instrs.push(op("DEPOLARIZE2", &[params.after_clifford_depolarization], &[StimTarget::Qubit(data[i + 1]), StimTarget::Qubit(a)]));
            }
        }

        // before_measure_flip_probability: X_ERROR before measurement
        if params.before_measure_flip_probability > 0.0 {
            for &a in &ancilla {
                instrs.push(op("X_ERROR", &[params.before_measure_flip_probability], &[StimTarget::Qubit(a)]));
            }
        }

        for &a in &ancilla {
            instrs.push(op("M", &[], &[StimTarget::Qubit(a)]));
        }

        for i in 0..n_ancilla {
            let rec_offset = -(n_ancilla as i32) + i as i32;
            if round == 0 {
                instrs.push(op(
                    "DETECTOR",
                    &[2.0 * i as f64 + 1.0, 0.0, round as f64],
                    &[StimTarget::Rec(rec_offset)],
                ));
            } else {
                let prev_offset = rec_offset - n_ancilla as i32;
                instrs.push(op(
                    "DETECTOR",
                    &[2.0 * i as f64 + 1.0, 0.0, round as f64],
                    &[StimTarget::Rec(rec_offset), StimTarget::Rec(prev_offset)],
                ));
            }
        }
    }

    instrs.push(op("TICK", &[], &[]));

    // before_measure_flip_probability: X_ERROR before final data measurement
    if params.before_measure_flip_probability > 0.0 {
        for &d in &data {
            instrs.push(op("X_ERROR", &[params.before_measure_flip_probability], &[StimTarget::Qubit(d)]));
        }
    }

    for &d in &data {
        instrs.push(op("M", &[], &[StimTarget::Qubit(d)]));
    }

    for i in 0..n_ancilla {
        let last_ancilla_offset = -(n_data as i32) - (n_ancilla as i32) + i as i32;
        let data_i_offset = -(n_data as i32) + i as i32;
        let data_i1_offset = data_i_offset + 1;
        instrs.push(op(
            "DETECTOR",
            &[2.0 * i as f64 + 1.0, 0.0, rounds as f64],
            &[
                StimTarget::Rec(data_i_offset),
                StimTarget::Rec(data_i1_offset),
                StimTarget::Rec(last_ancilla_offset),
            ],
        ));
    }

    instrs.push(op(
        "OBSERVABLE_INCLUDE",
        &[0.0],
        &[StimTarget::Rec(-(n_data as i32))],
    ));

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
