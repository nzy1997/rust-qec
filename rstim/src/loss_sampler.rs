use rand::Rng;

use crate::ir::{StimInstr, StimTarget};
use crate::rare_error_iterator::RareErrorIndexSampler;
use crate::sim::bit_table::BitTable;

/// A compiled, allocation-light execution plan for the common loss-aware sampling subset.
///
/// Unsupported instructions cause compilation to return `None`, allowing callers to preserve
/// the general executor as a correctness fallback.
#[derive(Debug)]
pub(crate) struct LossSamplerPlan {
    num_simulated_qubits: usize,
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
        let (mut ops, has_loss) = compile_ops(instrs)?;
        if !has_loss {
            return None;
        }
        let num_simulated_qubits = compact_qubit_indices(&mut ops);
        Some(Self {
            num_simulated_qubits,
            num_measurements: crate::stats::num_measurements(instrs),
            ops,
        })
    }

    pub(crate) fn num_measurements(&self) -> usize {
        self.num_measurements
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

        let mut batch = LossFrameBatch::try_new(
            self.num_simulated_qubits,
            self.num_measurements,
            num_shots,
            rng,
        )?;
        batch.execute(&self.ops, rng)?;
        if batch.measurement_index != self.num_measurements {
            return Err(format!(
                "loss frame produced {} measurements, expected {}",
                batch.measurement_index, self.num_measurements
            ));
        }
        Ok(batch.measurements)
    }
}

/// A CSS-frame batch with one loss bit per qubit and shot.
///
/// The complete randomized CSS frame includes ideal Paulis, so every shot may follow its own
/// Clifford path when loss skips gates. A lost target is frozen until reset, paired gates are
/// masked whenever either endpoint is lost, and lost measurements are forced to one. Keeping shots
/// in the minor dimension makes every layer operate on 64 shots per machine word.
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

    fn execute<R: Rng + ?Sized>(&mut self, ops: &[LossOp], rng: &mut R) -> Result<(), String> {
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
                LossOp::X(qubits) => self.ideal_pauli(qubits, 1),
                LossOp::Y(qubits) => self.ideal_pauli(qubits, 2),
                LossOp::Z(qubits) => self.ideal_pauli(qubits, 3),
                LossOp::ResetZ(qubits) => {
                    for &q in qubits {
                        self.reset_z(q, rng);
                    }
                }
                LossOp::MeasureZ(targets) => {
                    for &(q, inverted) in targets {
                        self.measure_z(q, inverted, false, rng);
                    }
                }
                LossOp::MeasureResetZ(targets) => {
                    for &(q, inverted) in targets {
                        self.measure_z(q, inverted, true, rng);
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
                        self.execute(body, rng)?;
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

    fn measure_z<R: Rng + ?Sized>(&mut self, q: usize, inverted: bool, reset: bool, rng: &mut R) {
        let words = self.x_table.words_per_row();
        for word in 0..words {
            let lost = self.lost_table.row_words(q)[word];
            let present_value = self.x_table.row_words(q)[word];
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
                self.z_table.row_words_mut(q)[word] = (old_z & lost) | (rng.r#gen::<u64>() & !lost);
            }
        }
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

    fn ideal_pauli(&mut self, qubits: &[usize], pauli: u8) {
        for &q in qubits {
            for word in 0..self.x_table.words_per_row() {
                let present = !self.lost_table.row_words(q)[word];
                if matches!(pauli, 1 | 2) {
                    self.x_table.row_words_mut(q)[word] ^= present;
                }
                if matches!(pauli, 2 | 3) {
                    self.z_table.row_words_mut(q)[word] ^= present;
                }
            }
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

/// Remaps only operational qubits onto a dense internal range.
///
/// Stim qubit identifiers are labels and need not be contiguous. Stabilizer tableaux scale with
/// the largest allocated dimension, so preserving holes would charge every shot for qubits that
/// never participate in an operation. Measurement order is stored separately and is unchanged.
fn compact_qubit_indices(ops: &mut [LossOp]) -> usize {
    let mut used_qubits = Vec::new();
    collect_qubit_indices(ops, &mut used_qubits);
    used_qubits.sort_unstable();
    used_qubits.dedup();
    let mut measure_reset_qubits = Vec::new();
    collect_measure_reset_qubits(ops, &mut measure_reset_qubits);
    measure_reset_qubits.sort_unstable();
    measure_reset_qubits.dedup();
    used_qubits.sort_by_key(|q| (measure_reset_qubits.binary_search(q).is_ok(), *q));
    let mut physical_to_dense: Vec<(usize, usize)> = used_qubits
        .iter()
        .enumerate()
        .map(|(dense, &physical)| (physical, dense))
        .collect();
    physical_to_dense.sort_unstable_by_key(|&(physical, _)| physical);
    remap_qubit_indices(ops, &physical_to_dense);
    used_qubits.len()
}

fn collect_measure_reset_qubits(ops: &[LossOp], qubits: &mut Vec<usize>) {
    for op in ops {
        match op {
            LossOp::MeasureResetZ(targets) => {
                qubits.extend(targets.iter().map(|&(q, _)| q));
            }
            LossOp::Repeat { body, .. } => collect_measure_reset_qubits(body, qubits),
            _ => {}
        }
    }
}

fn collect_qubit_indices(ops: &[LossOp], used_qubits: &mut Vec<usize>) {
    for op in ops {
        match op {
            LossOp::H(qubits)
            | LossOp::X(qubits)
            | LossOp::Y(qubits)
            | LossOp::Z(qubits)
            | LossOp::ResetZ(qubits) => used_qubits.extend(qubits),
            LossOp::XError { qubits, .. }
            | LossOp::YError { qubits, .. }
            | LossOp::ZError { qubits, .. }
            | LossOp::Depolarize1 { qubits, .. }
            | LossOp::Loss { qubits, .. } => used_qubits.extend(qubits),
            LossOp::Cx(pairs) | LossOp::Depolarize2 { pairs, .. } => {
                used_qubits.extend(pairs.iter().flat_map(|&(a, b)| [a, b]));
            }
            LossOp::MeasureZ(targets) | LossOp::MeasureResetZ(targets) => {
                used_qubits.extend(targets.iter().map(|&(q, _)| q));
            }
            LossOp::Repeat { body, .. } => collect_qubit_indices(body, used_qubits),
        }
    }
}

fn remap_qubit_indices(ops: &mut [LossOp], physical_to_dense: &[(usize, usize)]) {
    let remap = |q: &mut usize| {
        let index = physical_to_dense
            .binary_search_by_key(q, |&(physical, _)| physical)
            .expect("compiled operation qubit was collected");
        *q = physical_to_dense[index].1;
    };
    for op in ops {
        match op {
            LossOp::H(qubits)
            | LossOp::X(qubits)
            | LossOp::Y(qubits)
            | LossOp::Z(qubits)
            | LossOp::ResetZ(qubits) => qubits.iter_mut().for_each(&remap),
            LossOp::XError { qubits, .. }
            | LossOp::YError { qubits, .. }
            | LossOp::ZError { qubits, .. }
            | LossOp::Depolarize1 { qubits, .. }
            | LossOp::Loss { qubits, .. } => qubits.iter_mut().for_each(&remap),
            LossOp::Cx(pairs) | LossOp::Depolarize2 { pairs, .. } => {
                for (a, b) in pairs {
                    remap(a);
                    remap(b);
                }
            }
            LossOp::MeasureZ(targets) | LossOp::MeasureResetZ(targets) => {
                for (q, _) in targets {
                    remap(q);
                }
            }
            LossOp::Repeat { body, .. } => remap_qubit_indices(body, physical_to_dense),
        }
    }
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

fn two_qubit_pauli(index: usize) -> (u8, u8) {
    let encoded = index + 1;
    ((encoded / 4) as u8, (encoded % 4) as u8)
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    use super::LossSamplerPlan;
    use crate::executor::Executor;
    use crate::parser::parse_lines;

    #[test]
    fn packed_loss_plan_matches_legacy_executor_for_deterministic_loss_semantics() {
        let circuit = parse_lines(
            "R 0 1 2\n\
             X 0\n\
             LOSS(1) 0\n\
             CX 0 2\n\
             M 0 1 2\n\
             MR 0\n\
             M 0\n",
        )
        .unwrap();
        let plan = LossSamplerPlan::try_compile(&circuit).expect("supported loss plan");
        let reference = crate::data_path::build_reference_sample(
            &circuit,
            crate::data_path::ReferenceSampleMode::SimulateNoiseless,
        )
        .unwrap();

        for seed in 0..64 {
            let mut packed_rng = StdRng::seed_from_u64(seed);
            let mut legacy_rng = StdRng::seed_from_u64(seed);
            let batch = plan.run_batch(1, &reference, &mut packed_rng).unwrap();
            let packed: Vec<_> = (0..plan.num_measurements)
                .map(|measurement| batch.get(measurement, 0))
                .collect();
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
    fn loss_before_later_cx_compiles_to_the_css_frame() {
        let circuit = parse_lines("X 0\nLOSS(0.5) 0\nCX 0 2\nM 0 1 2 3\n").unwrap();
        LossSamplerPlan::try_compile(&circuit).expect("supported loss plan");
    }

    #[test]
    fn loss_at_end_of_repeated_body_compiles_with_next_iteration_cx() {
        let circuit = parse_lines("REPEAT 2 {\nCX 0 1\nLOSS(0.5) 0\n}\nM 0 1\n").unwrap();
        LossSamplerPlan::try_compile(&circuit).expect("supported loss plan");
    }

    #[test]
    fn unrelated_cx_after_loss_compiles_to_the_css_frame() {
        let circuit = parse_lines("LOSS(0.5) 0\nCX 1 2\nM 0 1 2\n").unwrap();
        LossSamplerPlan::try_compile(&circuit).expect("supported loss plan");
    }

    #[test]
    fn reset_between_loss_and_later_cx_compiles_to_the_css_frame() {
        let circuit = parse_lines("LOSS(0.5) 0\nR 0\nCX 0 1\nM 0 1\n").unwrap();
        LossSamplerPlan::try_compile(&circuit).expect("supported loss plan");
    }

    #[test]
    fn repeat_reset_loss_compiles_to_the_css_frame() {
        let circuit = parse_lines("REPEAT 3 {\nCX 0 1\nLOSS(0.5) 0\nMR 0\n}\nM 0 1\n").unwrap();
        LossSamplerPlan::try_compile(&circuit).expect("supported loss plan");
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
    fn gapped_qubit_labels_are_compacted_without_changing_loss_semantics() {
        let circuit = parse_lines("R 1 100\nX 1\nLOSS(1) 1\nCX 1 100\nX 100\nM 1 100\n").unwrap();
        let plan = LossSamplerPlan::try_compile(&circuit).expect("supported loss plan");
        assert_eq!(plan.num_simulated_qubits, 2);

        let mut packed_rng = StdRng::seed_from_u64(5);
        let mut legacy_rng = StdRng::seed_from_u64(5);
        let reference = crate::data_path::build_reference_sample(
            &circuit,
            crate::data_path::ReferenceSampleMode::SimulateNoiseless,
        )
        .unwrap();
        let batch = plan.run_batch(1, &reference, &mut packed_rng).unwrap();
        let packed: Vec<_> = (0..plan.num_measurements)
            .map(|measurement| batch.get(measurement, 0))
            .collect();
        let legacy = Executor::from_instrs(circuit)
            .unwrap()
            .run(&mut legacy_rng)
            .unwrap()
            .measurements;
        assert_eq!(packed, vec![true, true]);
        assert_eq!(packed, legacy);
    }

    #[test]
    fn public_atom_loss_fixture_compiles_to_the_css_frame() {
        let circuit = parse_lines(include_str!(
            "../../benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100_atom_loss.stim"
        ))
        .unwrap();
        let plan = LossSamplerPlan::try_compile(&circuit).expect("fixture uses supported loss ops");
        assert_eq!(plan.num_simulated_qubits, 241);
        assert_eq!(plan.num_measurements, 12_121);
    }
}
