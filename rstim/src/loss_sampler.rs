use rand::Rng;

use crate::ir::{StimInstr, StimTarget};
use crate::rare_error_iterator::RareErrorIndexSampler;
use crate::sim::bit_table::BitTable;
#[cfg(test)]
use crate::sim::packed_inverse_tableau::PackedInverseTableau;

/// A compiled, allocation-light execution plan for the common loss-aware sampling subset.
///
/// Unsupported instructions cause compilation to return `None`, allowing callers to preserve
/// the general executor as a correctness fallback.
#[derive(Debug)]
pub(crate) struct LossSamplerPlan {
    num_qubits: usize,
    num_measurements: usize,
    ops: Vec<LossOp>,
}

#[derive(Debug)]
enum LossOp {
    H(Vec<usize>),
    Cx(Vec<(usize, usize)>),
    X(Vec<usize>),
    Y(Vec<usize>),
    Z(Vec<usize>),
    ResetZ(Vec<usize>),
    MeasureZ(Vec<(usize, bool)>),
    MeasureResetZ(Vec<(usize, bool)>),
    XError {
        probability: f64,
        qubits: Vec<usize>,
    },
    YError {
        probability: f64,
        qubits: Vec<usize>,
    },
    ZError {
        probability: f64,
        qubits: Vec<usize>,
    },
    Depolarize1 {
        probability: f64,
        qubits: Vec<usize>,
    },
    Depolarize2 {
        probability: f64,
        pairs: Vec<(usize, usize)>,
    },
    Loss {
        probability: f64,
        qubits: Vec<usize>,
    },
    Repeat {
        count: u64,
        body: Vec<LossOp>,
    },
}

impl LossSamplerPlan {
    pub(crate) fn try_compile(instrs: &[StimInstr]) -> Option<Self> {
        let (ops, has_loss) = compile_ops(instrs)?;
        if !has_loss {
            return None;
        }
        Some(Self {
            num_qubits: crate::stats::num_qubits(instrs),
            num_measurements: crate::stats::num_measurements(instrs),
            ops,
        })
    }

    #[cfg(test)]
    pub(crate) fn run_shot<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec<bool> {
        let mut shot = LossShot::new(self.num_qubits, self.num_measurements);
        shot.execute(&self.ops, rng);
        shot.measurements
    }

    pub(crate) fn run_batch<R: Rng + ?Sized>(
        &self,
        num_shots: usize,
        reference_sample: &[bool],
        rng: &mut R,
    ) -> Result<BitTable, String> {
        if reference_sample.len() != self.num_measurements {
            return Err(format!(
                "loss frame expected {} reference measurements, got {}",
                self.num_measurements,
                reference_sample.len()
            ));
        }

        let mut batch =
            LossFrameBatch::try_new(self.num_qubits, self.num_measurements, num_shots, rng)?;
        batch.execute(&self.ops, reference_sample, rng)?;
        if batch.measurement_index != self.num_measurements {
            return Err(format!(
                "loss frame produced {} measurements, expected {}",
                batch.measurement_index, self.num_measurements
            ));
        }
        Ok(batch.measurements)
    }
}

/// A Pauli-frame batch with one loss bit per qubit and shot.
///
/// Loss only changes which Clifford/noise operations propagate a shot's frame. This is the
/// bit-sliced form of the executor's reset-based loss semantics: a lost target is frozen until
/// reset, paired gates are masked whenever either endpoint is lost, and lost measurements are
/// forced to one. Keeping shots in the minor dimension makes every Clifford layer operate on 64
/// shots per machine word instead of evolving one tableau per shot.
struct LossFrameBatch {
    batch_size: usize,
    x_table: BitTable,
    z_table: BitTable,
    lost_table: BitTable,
    measurements: BitTable,
    measurement_index: usize,
}

impl LossFrameBatch {
    fn try_new<R: Rng + ?Sized>(
        num_qubits: usize,
        num_measurements: usize,
        batch_size: usize,
        rng: &mut R,
    ) -> Result<Self, String> {
        let alloc = |rows| {
            BitTable::try_new(rows, batch_size)
                .map_err(|error| format!("loss frame allocation failed: {error:?}"))
        };
        let x_table = alloc(num_qubits)?;
        let mut z_table = alloc(num_qubits)?;
        for q in 0..num_qubits {
            z_table.randomize_row(q, rng);
        }
        Ok(Self {
            batch_size,
            x_table,
            z_table,
            lost_table: alloc(num_qubits)?,
            measurements: alloc(num_measurements)?,
            measurement_index: 0,
        })
    }

    fn execute<R: Rng + ?Sized>(
        &mut self,
        ops: &[LossOp],
        reference_sample: &[bool],
        rng: &mut R,
    ) -> Result<(), String> {
        for op in ops {
            match op {
                LossOp::H(qubits) => {
                    for &q in qubits {
                        self.h(q);
                    }
                }
                LossOp::Cx(pairs) => {
                    for &(control, target) in pairs {
                        self.cx(control, target);
                    }
                }
                // Ideal Pauli gates are already present in the reference sample. If their target
                // is lost, its frame is unobservable and remains frozen until reset.
                LossOp::X(qubits) | LossOp::Y(qubits) | LossOp::Z(qubits) => {
                    self.ignore_reference_paulis(qubits)
                }
                LossOp::ResetZ(qubits) => {
                    for &q in qubits {
                        self.reset_z(q, rng);
                    }
                }
                LossOp::MeasureZ(targets) => {
                    for &(q, inverted) in targets {
                        self.measure_z(q, inverted, false, reference_sample, rng)?;
                    }
                }
                LossOp::MeasureResetZ(targets) => {
                    for &(q, inverted) in targets {
                        self.measure_z(q, inverted, true, reference_sample, rng)?;
                    }
                }
                LossOp::XError {
                    probability,
                    qubits,
                } => self.independent_pauli(*probability, qubits, 1, rng)?,
                LossOp::YError {
                    probability,
                    qubits,
                } => self.independent_pauli(*probability, qubits, 2, rng)?,
                LossOp::ZError {
                    probability,
                    qubits,
                } => self.independent_pauli(*probability, qubits, 3, rng)?,
                LossOp::Depolarize1 {
                    probability,
                    qubits,
                } => self.depolarize1(*probability, qubits, rng)?,
                LossOp::Depolarize2 { probability, pairs } => {
                    self.depolarize2(*probability, pairs, rng)?
                }
                LossOp::Loss {
                    probability,
                    qubits,
                } => self.loss(*probability, qubits, rng)?,
                LossOp::Repeat { count, body } => {
                    for _ in 0..*count {
                        self.execute(body, reference_sample, rng)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn h(&mut self, q: usize) {
        let words = self.x_table.words_per_row();
        for word in 0..words {
            let x = self.x_table.row_words(q)[word];
            let z = self.z_table.row_words(q)[word];
            let lost = self.lost_table.row_words(q)[word];
            let delta = (x ^ z) & !lost;
            self.x_table.row_words_mut(q)[word] ^= delta;
            self.z_table.row_words_mut(q)[word] ^= delta;
        }
    }

    fn cx(&mut self, control: usize, target: usize) {
        let words = self.x_table.words_per_row();
        for word in 0..words {
            let present = !(self.lost_table.row_words(control)[word]
                | self.lost_table.row_words(target)[word]);
            let control_x = self.x_table.row_words(control)[word];
            let target_z = self.z_table.row_words(target)[word];
            self.x_table.row_words_mut(target)[word] ^= control_x & present;
            self.z_table.row_words_mut(control)[word] ^= target_z & present;
        }
    }

    fn reset_z<R: Rng + ?Sized>(&mut self, q: usize, rng: &mut R) {
        self.x_table.clear_row(q);
        self.z_table.randomize_row(q, rng);
        self.lost_table.clear_row(q);
    }

    fn measure_z<R: Rng + ?Sized>(
        &mut self,
        q: usize,
        inverted: bool,
        reset: bool,
        reference_sample: &[bool],
        rng: &mut R,
    ) -> Result<(), String> {
        let reference = *reference_sample
            .get(self.measurement_index)
            .ok_or_else(|| {
                format!(
                    "loss frame measurement {} exceeds the reference sample",
                    self.measurement_index
                )
            })?;
        let words = self.x_table.words_per_row();
        for word in 0..words {
            let lost = self.lost_table.row_words(q)[word];
            let present_value = if reference {
                !self.x_table.row_words(q)[word]
            } else {
                self.x_table.row_words(q)[word]
            };
            self.measurements.row_words_mut(self.measurement_index)[word] = if inverted {
                present_value & !lost
            } else {
                present_value | lost
            };
        }
        self.measurement_index += 1;

        if reset {
            self.reset_z(q, rng);
        } else {
            for word in 0..words {
                let lost = self.lost_table.row_words(q)[word];
                let old_z = self.z_table.row_words(q)[word];
                self.x_table.row_words_mut(q)[word] &= lost;
                self.z_table.row_words_mut(q)[word] = (old_z & lost) | (rng.r#gen::<u64>() & !lost);
            }
        }
        Ok(())
    }

    fn independent_pauli<R: Rng + ?Sized>(
        &mut self,
        probability: f64,
        qubits: &[usize],
        pauli: u8,
        rng: &mut R,
    ) -> Result<(), String> {
        let attempts = self.attempt_count(qubits.len())?;
        let mut events = RareErrorIndexSampler::new(probability, attempts);
        while let Some(index) = events.next_index(rng) {
            let q = qubits[index / self.batch_size];
            let shot = index % self.batch_size;
            if !self.lost_table.get(q, shot) {
                self.apply_pauli(q, shot, pauli);
            }
        }
        Ok(())
    }

    fn depolarize1<R: Rng + ?Sized>(
        &mut self,
        probability: f64,
        qubits: &[usize],
        rng: &mut R,
    ) -> Result<(), String> {
        let attempts = self.attempt_count(qubits.len())?;
        let mut events = RareErrorIndexSampler::new(probability, attempts);
        while let Some(index) = events.next_index(rng) {
            let q = qubits[index / self.batch_size];
            let shot = index % self.batch_size;
            if !self.lost_table.get(q, shot) {
                self.apply_pauli(q, shot, rng.gen_range(1..=3));
            }
        }
        Ok(())
    }

    fn depolarize2<R: Rng + ?Sized>(
        &mut self,
        probability: f64,
        pairs: &[(usize, usize)],
        rng: &mut R,
    ) -> Result<(), String> {
        let attempts = self.attempt_count(pairs.len())?;
        let mut events = RareErrorIndexSampler::new(probability, attempts);
        while let Some(index) = events.next_index(rng) {
            let (a, b) = pairs[index / self.batch_size];
            let shot = index % self.batch_size;
            if self.lost_table.get(a, shot) || self.lost_table.get(b, shot) {
                continue;
            }
            let (pa, pb) = two_qubit_pauli(rng.gen_range(0..15));
            self.apply_pauli(a, shot, pa);
            self.apply_pauli(b, shot, pb);
        }
        Ok(())
    }

    fn loss<R: Rng + ?Sized>(
        &mut self,
        probability: f64,
        qubits: &[usize],
        rng: &mut R,
    ) -> Result<(), String> {
        let attempts = self.attempt_count(qubits.len())?;
        let mut events = RareErrorIndexSampler::new(probability, attempts);
        while let Some(index) = events.next_index(rng) {
            let q = qubits[index / self.batch_size];
            let shot = index % self.batch_size;
            self.lost_table.set(q, shot, true);
        }
        Ok(())
    }

    fn apply_pauli(&mut self, q: usize, shot: usize, pauli: u8) {
        if matches!(pauli, 1 | 2) {
            self.x_table.toggle(q, shot);
        }
        if matches!(pauli, 2 | 3) {
            self.z_table.toggle(q, shot);
        }
    }

    fn attempt_count(&self, target_count: usize) -> Result<usize, String> {
        target_count
            .checked_mul(self.batch_size)
            .ok_or_else(|| "loss frame noise attempt count overflow".to_string())
    }

    #[inline(always)]
    fn ignore_reference_paulis(&self, _qubits: &[usize]) {}
}

#[cfg(test)]
struct LossShot {
    tableau: PackedInverseTableau,
    lost: Vec<bool>,
    measurements: Vec<bool>,
}

#[cfg(test)]
impl LossShot {
    fn new(num_qubits: usize, num_measurements: usize) -> Self {
        Self {
            tableau: PackedInverseTableau::identity(num_qubits),
            lost: vec![false; num_qubits],
            measurements: Vec::with_capacity(num_measurements),
        }
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
                LossOp::Cx(pairs) => {
                    for &(control, target) in pairs {
                        if !self.lost[control] && !self.lost[target] {
                            self.tableau.cx(control, target);
                        }
                    }
                }
                LossOp::X(qubits) => self.apply_present_paulis(qubits, 1),
                LossOp::Y(qubits) => self.apply_present_paulis(qubits, 2),
                LossOp::Z(qubits) => self.apply_present_paulis(qubits, 3),
                LossOp::ResetZ(qubits) => {
                    self.tableau.reset_z_many(qubits, rng);
                    for &q in qubits {
                        self.lost[q] = false;
                    }
                }
                LossOp::MeasureZ(targets) => self.measure_z(targets, rng),
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
                    for &q in qubits {
                        if !self.lost[q] && rng.r#gen::<f64>() < *probability {
                            let pauli = match rng.gen_range(0..3) {
                                0 => 1,
                                1 => 2,
                                _ => 3,
                            };
                            apply_pauli(&mut self.tableau, q, pauli);
                        }
                    }
                }
                LossOp::Depolarize2 { probability, pairs } => {
                    for &(a, b) in pairs {
                        if self.lost[a] || self.lost[b] {
                            continue;
                        }
                        if rng.r#gen::<f64>() < *probability {
                            let (pa, pb) = two_qubit_pauli(rng.gen_range(0..15));
                            apply_pauli(&mut self.tableau, a, pa);
                            apply_pauli(&mut self.tableau, b, pb);
                        }
                    }
                }
                LossOp::Loss {
                    probability,
                    qubits,
                } => {
                    for &q in qubits {
                        if rng.r#gen::<f64>() < *probability {
                            self.lost[q] = true;
                        }
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

    fn apply_present_paulis(&mut self, qubits: &[usize], pauli: u8) {
        for &q in qubits {
            if !self.lost[q] {
                apply_pauli(&mut self.tableau, q, pauli);
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
        for &q in qubits {
            if !self.lost[q] && rng.r#gen::<f64>() < probability {
                apply_pauli(&mut self.tableau, q, pauli);
            }
        }
    }

    fn measure_z<R: Rng + ?Sized>(&mut self, targets: &[(usize, bool)], rng: &mut R) {
        let present: Vec<(usize, bool)> = targets
            .iter()
            .copied()
            .filter(|(q, _)| !self.lost[*q])
            .collect();
        let present_bits = self.tableau.measure_z_many(&present, rng);
        let mut present_bits = present_bits.into_iter();
        for &(q, inverted) in targets {
            self.measurements.push(if self.lost[q] {
                true ^ inverted
            } else {
                present_bits
                    .next()
                    .expect("present measurement result count matches targets")
            });
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

fn compile_ops(instrs: &[StimInstr]) -> Option<(Vec<LossOp>, bool)> {
    let mut ops = Vec::new();
    let mut has_loss = false;
    for instr in instrs {
        match instr {
            StimInstr::Repeat { count, body } => {
                let (body, body_has_loss) = compile_ops(body)?;
                has_loss |= body_has_loss;
                ops.push(LossOp::Repeat {
                    count: *count,
                    body,
                });
            }
            StimInstr::Op {
                name,
                args,
                targets,
                ..
            } => {
                let probability = || args.first().copied().unwrap_or(0.0);
                let op = match name.as_str() {
                    "H" => Some(LossOp::H(plain_qubits(targets)?)),
                    "CX" | "CNOT" | "ZCX" => Some(LossOp::Cx(plain_pairs(targets)?)),
                    "X" => Some(LossOp::X(plain_qubits(targets)?)),
                    "Y" => Some(LossOp::Y(plain_qubits(targets)?)),
                    "Z" => Some(LossOp::Z(plain_qubits(targets)?)),
                    "R" | "RZ" => Some(LossOp::ResetZ(plain_qubits(targets)?)),
                    "M" | "MZ" if args.is_empty() => {
                        Some(LossOp::MeasureZ(measurement_targets(targets)?))
                    }
                    "MR" | "MRZ" if args.is_empty() => {
                        Some(LossOp::MeasureResetZ(measurement_targets(targets)?))
                    }
                    "X_ERROR" => Some(LossOp::XError {
                        probability: probability(),
                        qubits: plain_qubits(targets)?,
                    }),
                    "Y_ERROR" => Some(LossOp::YError {
                        probability: probability(),
                        qubits: plain_qubits(targets)?,
                    }),
                    "Z_ERROR" => Some(LossOp::ZError {
                        probability: probability(),
                        qubits: plain_qubits(targets)?,
                    }),
                    "DEPOLARIZE1" => Some(LossOp::Depolarize1 {
                        probability: probability(),
                        qubits: plain_qubits(targets)?,
                    }),
                    "DEPOLARIZE2" => Some(LossOp::Depolarize2 {
                        probability: probability(),
                        pairs: plain_pairs(targets)?,
                    }),
                    "LOSS" => {
                        has_loss = true;
                        Some(LossOp::Loss {
                            probability: probability(),
                            qubits: plain_qubits(targets)?,
                        })
                    }
                    "QUBIT_COORDS" | "SHIFT_COORDS" | "TICK" | "DETECTOR"
                    | "OBSERVABLE_INCLUDE" => None,
                    _ => return None,
                };
                if let Some(op) = op {
                    ops.push(op);
                }
            }
        }
    }
    Some((ops, has_loss))
}

fn plain_qubits(targets: &[StimTarget]) -> Option<Vec<usize>> {
    targets
        .iter()
        .map(|target| match target {
            StimTarget::Qubit(q) => Some(*q as usize),
            _ => None,
        })
        .collect()
}

fn measurement_targets(targets: &[StimTarget]) -> Option<Vec<(usize, bool)>> {
    targets
        .iter()
        .map(|target| match target {
            StimTarget::Qubit(q) => Some((*q as usize, false)),
            StimTarget::QubitInv(q) => Some((*q as usize, true)),
            _ => None,
        })
        .collect()
}

fn plain_pairs(targets: &[StimTarget]) -> Option<Vec<(usize, usize)>> {
    let qubits = plain_qubits(targets)?;
    if qubits.len() % 2 != 0 {
        return None;
    }
    Some(
        qubits
            .chunks_exact(2)
            .map(|pair| (pair[0], pair[1]))
            .collect(),
    )
}

#[cfg(test)]
fn apply_pauli(tableau: &mut PackedInverseTableau, qubit: usize, pauli: u8) {
    match pauli {
        0 => {}
        1 => tableau.x_gate(qubit),
        2 => tableau.y_gate(qubit),
        3 => tableau.z_gate(qubit),
        _ => unreachable!("Pauli index is in 0..=3"),
    }
}

fn two_qubit_pauli(index: usize) -> (u8, u8) {
    let encoded = index + 1;
    ((encoded / 4) as u8, (encoded % 4) as u8)
}

#[cfg(test)]
mod tests {
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    use super::LossSamplerPlan;
    use crate::executor::Executor;
    use crate::parser::parse_lines;

    #[test]
    fn packed_loss_plan_matches_legacy_executor_for_supported_semantics() {
        let circuit = parse_lines(
            "R 0 1 2\n\
             X_ERROR(0.25) 0 1 2\n\
             H 0\n\
             CX 0 1 1 2\n\
             DEPOLARIZE1(0.2) 0 1 2\n\
             DEPOLARIZE2(0.3) 0 1 1 2\n\
             LOSS(0.4) 0 1 2\n\
             CX 0 1 1 2\n\
             M 0 1\n\
             MR 2\n\
             M 2\n",
        )
        .unwrap();
        let plan = LossSamplerPlan::try_compile(&circuit).expect("supported loss plan");

        for seed in 0..64 {
            let mut packed_rng = StdRng::seed_from_u64(seed);
            let mut legacy_rng = StdRng::seed_from_u64(seed);
            let packed = plan.run_shot(&mut packed_rng);
            let legacy = Executor::from_instrs(circuit.clone())
                .unwrap()
                .run(&mut legacy_rng)
                .unwrap()
                .measurements;
            assert_eq!(packed, legacy, "seed {seed}");
        }
    }

    #[test]
    fn unsupported_loss_circuit_declines_fast_plan() {
        let circuit = parse_lines("LOSS(0.1) 0\nMX 0\n").unwrap();
        assert!(LossSamplerPlan::try_compile(&circuit).is_none());
    }

    #[test]
    fn bit_parallel_loss_batch_is_repeatable_for_a_fixed_seed() {
        let circuit =
            parse_lines("R 0 1\nH 0\nCX 0 1\nDEPOLARIZE2(0.2) 0 1\nLOSS(0.3) 0 1\nM 0 1\n")
                .unwrap();
        let plan = LossSamplerPlan::try_compile(&circuit).expect("supported loss plan");
        let reference = crate::data_path::build_reference_sample(
            &circuit,
            crate::data_path::ReferenceSampleMode::SimulateNoiseless,
        )
        .unwrap();
        let mut first_rng = StdRng::seed_from_u64(0x5eed);
        let mut second_rng = StdRng::seed_from_u64(0x5eed);

        let first = plan.run_batch(257, &reference, &mut first_rng).unwrap();
        let second = plan.run_batch(257, &reference, &mut second_rng).unwrap();
        for measurement in 0..first.num_major() {
            for shot in 0..first.num_minor() {
                assert_eq!(first.get(measurement, shot), second.get(measurement, shot));
            }
        }
    }

    #[test]
    fn public_atom_loss_fixture_compiles_to_the_fast_plan() {
        let circuit = parse_lines(include_str!(
            "../../benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100_atom_loss.stim"
        ))
        .unwrap();
        let plan = LossSamplerPlan::try_compile(&circuit).expect("fixture uses the fast subset");
        assert_eq!(plan.num_qubits, 274);
        assert_eq!(plan.num_measurements, 12_121);
    }
}
