use rand::Rng;

use crate::rare_error_iterator::RareErrorIndexSampler;
use crate::sim::bit_table::BitTable;
use crate::sim::packed_inverse_tableau::PackedInverseTableau;

use super::{LossOp, LossSamplerPlan, NativeClifford, two_qubit_pauli};

pub(super) fn sample_batch<R: Rng + ?Sized>(
    plan: &LossSamplerPlan,
    shots: usize,
    rng: &mut R,
) -> Result<BitTable, String> {
    let mut measurements = BitTable::try_new(plan.num_measurements, shots)
        .map_err(|error| format!("loss tableau allocation failed: {error:?}"))?;
    if shots == 0 {
        return Ok(measurements);
    }

    let mut trajectory = LossShot::new(plan.num_simulated_qubits, plan.num_measurements);
    for shot in 0..shots {
        trajectory.execute(&plan.ops, rng);
        if trajectory.measurements.len() != plan.num_measurements {
            return Err(format!(
                "loss tableau produced {} measurements, expected {}",
                trajectory.measurements.len(),
                plan.num_measurements,
            ));
        }
        for (index, &bit) in trajectory.measurements.iter().enumerate() {
            measurements.set(index, shot, bit);
        }
        if shot + 1 < shots {
            trajectory.reset();
        }
    }
    Ok(measurements)
}

// A loss can suppress a Clifford and change the stabilizers that govern later measurements.
// Each shot carries its own signed tableau and loss state; reuse the storage across trajectories.
struct LossShot {
    tableau: PackedInverseTableau,
    lost: Vec<bool>,
    measurements: Vec<bool>,
    present_targets: Vec<(usize, bool)>,
}

impl LossShot {
    fn new(num_qubits: usize, num_measurements: usize) -> Self {
        Self {
            tableau: PackedInverseTableau::identity(num_qubits),
            lost: vec![false; num_qubits],
            measurements: Vec::with_capacity(num_measurements),
            present_targets: Vec::new(),
        }
    }

    fn reset(&mut self) {
        self.tableau.reset_identity();
        self.lost.fill(false);
        self.measurements.clear();
    }

    fn execute<R: Rng + ?Sized>(&mut self, ops: &[LossOp], rng: &mut R) {
        for op in ops {
            match op {
                LossOp::H(qubits) => {
                    for &q in qubits {
                        if !self.lost[q] {
                            self.tableau.h(q);
                        }
                    }
                }
                LossOp::NativeClifford(gate, qubits) => {
                    for &q in qubits {
                        if !self.lost[q] {
                            apply_clifford(&mut self.tableau, *gate, q);
                        }
                    }
                }
                LossOp::Cx(pairs) | LossOp::Cz(pairs) => {
                    let is_cz = matches!(op, LossOp::Cz(_));
                    for &(control, target) in pairs {
                        if self.lost[control] || self.lost[target] {
                            continue;
                        }
                        if is_cz {
                            self.tableau.h(target);
                        }
                        self.tableau.cx(control, target);
                        if is_cz {
                            self.tableau.h(target);
                        }
                    }
                }
                LossOp::X(qubits) => self.apply_independent_error(1.0, qubits, 1, rng),
                LossOp::Y(qubits) => self.apply_independent_error(1.0, qubits, 2, rng),
                LossOp::Z(qubits) => self.apply_independent_error(1.0, qubits, 3, rng),
                LossOp::ResetZ(qubits) => {
                    self.tableau.reset_z_many(qubits, rng);
                    for &q in qubits {
                        self.lost[q] = false;
                    }
                }
                LossOp::MeasureZ(targets) => self.measure_z(targets, false, rng),
                LossOp::MeasureLossZ(targets) => self.measure_z(targets, true, rng),
                LossOp::MeasureResetZ(targets) => self.measure_reset_z(targets, rng),
                LossOp::XError {
                    probability,
                    qubits,
                } => self.apply_independent_error(*probability, qubits, 1, rng),
                LossOp::YError {
                    probability,
                    qubits,
                } => self.apply_independent_error(*probability, qubits, 2, rng),
                LossOp::ZError {
                    probability,
                    qubits,
                } => self.apply_independent_error(*probability, qubits, 3, rng),
                LossOp::Depolarize1 {
                    probability,
                    qubits,
                } => {
                    let mut events = RareErrorIndexSampler::new(*probability, qubits.len());
                    while let Some(index) = events.next_index(rng) {
                        let q = qubits[index];
                        if !self.lost[q] {
                            apply_pauli(&mut self.tableau, q, rng.gen_range(1..=3));
                        }
                    }
                }
                LossOp::Depolarize2 { probability, pairs } => {
                    let mut events = RareErrorIndexSampler::new(*probability, pairs.len());
                    while let Some(index) = events.next_index(rng) {
                        let (a, b) = pairs[index];
                        if self.lost[a] || self.lost[b] {
                            continue;
                        }
                        let (pa, pb) = two_qubit_pauli(rng.gen_range(0..15));
                        apply_pauli(&mut self.tableau, a, pa);
                        apply_pauli(&mut self.tableau, b, pb);
                    }
                }
                LossOp::Loss {
                    probability,
                    qubits,
                } => {
                    let mut events = RareErrorIndexSampler::new(*probability, qubits.len());
                    while let Some(index) = events.next_index(rng) {
                        self.lost[qubits[index]] = true;
                    }
                }
                LossOp::Repeat { count, body } => {
                    for _ in 0..*count {
                        self.execute(body, rng);
                    }
                }
            }
        }
    }

    fn apply_independent_error<R: Rng + ?Sized>(
        &mut self,
        probability: f64,
        qubits: &[usize],
        pauli: u8,
        rng: &mut R,
    ) {
        let mut events = RareErrorIndexSampler::new(probability, qubits.len());
        while let Some(index) = events.next_index(rng) {
            let q = qubits[index];
            if !self.lost[q] {
                apply_pauli(&mut self.tableau, q, pauli);
            }
        }
    }

    fn measure_z<R: Rng + ?Sized>(
        &mut self,
        targets: &[(usize, bool)],
        record_loss: bool,
        rng: &mut R,
    ) {
        self.present_targets.clear();
        self.present_targets
            .extend(targets.iter().copied().filter(|&(q, _)| !self.lost[q]));
        let mut present_bits = self
            .tableau
            .measure_z_many(&self.present_targets, rng)
            .into_iter();
        for &(q, inverted) in targets {
            let lost = self.lost[q];
            if record_loss {
                self.measurements.push(lost);
            }
            let bit = if lost {
                !inverted
            } else {
                present_bits.next().expect("one result per present target")
            };
            self.measurements.push(bit);
        }
    }

    fn measure_reset_z<R: Rng + ?Sized>(&mut self, targets: &[(usize, bool)], rng: &mut R) {
        let reset_bits = self.tableau.measure_reset_z_many(targets, rng);
        for (&(q, inverted), bit) in targets.iter().zip(reset_bits) {
            self.measurements
                .push(if self.lost[q] { true ^ inverted } else { bit });
            self.lost[q] = false;
        }
    }
}

fn apply_clifford(tableau: &mut PackedInverseTableau, gate: NativeClifford, q: usize) {
    match gate {
        NativeClifford::S => tableau.s(q),
        NativeClifford::SDag => tableau.s_dag(q),
        NativeClifford::SqrtX => {
            tableau.h(q);
            tableau.s(q);
            tableau.h(q);
        }
        NativeClifford::SqrtXDag => {
            tableau.h(q);
            tableau.s_dag(q);
            tableau.h(q);
        }
        NativeClifford::SqrtY => {
            tableau.h(q);
            tableau.x_gate(q);
        }
        NativeClifford::SqrtYDag => {
            tableau.h(q);
            tableau.z_gate(q);
        }
    }
}

fn apply_pauli(tableau: &mut PackedInverseTableau, q: usize, pauli: u8) {
    match pauli {
        0 => {}
        1 => tableau.x_gate(q),
        2 => tableau.y_gate(q),
        3 => tableau.z_gate(q),
        _ => unreachable!("invalid Pauli index"),
    }
}
