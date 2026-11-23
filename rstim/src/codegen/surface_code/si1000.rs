use super::{RotatedLayout, op};
use crate::ir::{StimInstr, StimTarget};

pub(super) struct Schedule {
    pub initialization: Vec<StimInstr>,
    pub round: Vec<StimInstr>,
    pub readout: Vec<StimInstr>,
}

impl Schedule {
    pub(super) fn new(layout: &RotatedLayout, memory_x: bool, p: f64) -> Self {
        let all: Vec<_> = (0..layout.all_coords.len() as u32).collect();
        let moments = Moments { all: &all, p };
        let mut initialization = Vec::new();
        emit(&mut initialization, "R", &[], &all);
        noise(&mut initialization, "X_ERROR", 2.0 * p, &all);
        if memory_x {
            moments.hadamards(&mut initialization, &layout.data_qubits);
        }

        let mut round = Vec::new();
        let mut x_ancilla = vec![false; all.len()];
        for &q in &layout.x_measure_qubits {
            x_ancilla[q as usize] = true;
        }
        // Move the CX target conjugations into shared H moments. The X
        // ancilla's preparation/readout H gates are included in its frame.
        // An unused qubit keeps its current basis until it next participates.
        let mut frame = vec![false; all.len()];
        for layer in &layout.cnot_layers {
            let mut hadamards = Vec::new();
            for pair in layer.chunks_exact(2) {
                let control = pair[0] as usize;
                let target = pair[1] as usize;
                for (q, basis) in [(control, x_ancilla[control]), (target, !x_ancilla[target])] {
                    if frame[q] != basis {
                        hadamards.push(q as u32);
                        frame[q] = basis;
                    }
                }
            }
            hadamards.sort_unstable();
            moments.hadamards(&mut round, &hadamards);
            tick(&mut round);
            emit(&mut round, "CZ", &[], layer);
            noise(&mut round, "DEPOLARIZE2", p, layer);
            moments.idle(&mut round, layer, false);
        }
        let hadamards: Vec<_> = all.iter().copied().filter(|&q| frame[q as usize]).collect();
        moments.hadamards(&mut round, &hadamards);
        moments.measure(&mut round, &layout.measure_qubits);

        tick(&mut round);
        emit(&mut round, "R", &[], &layout.measure_qubits);
        noise(&mut round, "X_ERROR", 2.0 * p, &layout.measure_qubits);
        moments.idle(&mut round, &layout.measure_qubits, true);

        let mut readout = Vec::new();
        if memory_x {
            moments.hadamards(&mut readout, &layout.data_qubits);
        }
        moments.measure(&mut readout, &layout.data_qubits);
        Self {
            initialization,
            round,
            readout,
        }
    }
}

struct Moments<'a> {
    all: &'a [u32],
    p: f64,
}

impl Moments<'_> {
    fn hadamards(&self, out: &mut Vec<StimInstr>, active: &[u32]) {
        if active.is_empty() {
            return;
        }
        tick(out);
        emit(out, "H", &[], active);
        // The H gate and ordinary idle channels have the same rate.
        noise(out, "DEPOLARIZE1", self.p / 10.0, self.all);
    }

    fn measure(&self, out: &mut Vec<StimInstr>, active: &[u32]) {
        tick(out);
        noise(out, "X_ERROR", 5.0 * self.p, active);
        emit(out, "M", &[], active);
        self.idle(out, active, true);
    }

    fn idle(&self, out: &mut Vec<StimInstr>, active: &[u32], resonator: bool) {
        if self.p == 0.0 {
            return;
        }
        let mut idle = vec![true; self.all.len()];
        for &q in active {
            idle[q as usize] = false;
        }
        let spectators: Vec<_> = idle
            .iter()
            .enumerate()
            .filter_map(|(q, &is_idle)| is_idle.then_some(q as u32))
            .collect();
        noise(out, "DEPOLARIZE1", self.p / 10.0, &spectators);
        if resonator {
            noise(out, "DEPOLARIZE1", 2.0 * self.p, &spectators);
        }
    }
}

fn tick(out: &mut Vec<StimInstr>) {
    out.push(op("TICK", &[], &[]));
}

fn emit(out: &mut Vec<StimInstr>, gate: &str, args: &[f64], qubits: &[u32]) {
    if !qubits.is_empty() {
        let targets: Vec<_> = qubits.iter().copied().map(StimTarget::Qubit).collect();
        out.push(op(gate, args, &targets));
    }
}

fn noise(out: &mut Vec<StimInstr>, gate: &str, p: f64, qubits: &[u32]) {
    if p > 0.0 {
        emit(out, gate, &[p], qubits);
    }
}
