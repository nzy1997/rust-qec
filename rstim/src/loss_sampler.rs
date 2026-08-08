use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::ir::{StimInstr, StimTarget};
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

    pub(crate) fn run_shot<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec<bool> {
        let mut shot = LossShot::new(self.num_qubits, self.num_measurements);
        shot.execute(&self.ops, rng);
        shot.measurements
    }

    pub(crate) fn run_batch<R: Rng + ?Sized>(
        &self,
        num_shots: usize,
        rng: &mut R,
    ) -> Vec<Vec<bool>> {
        if num_shots == 0 {
            return Vec::new();
        }
        if num_shots == 1 {
            return vec![self.run_shot(rng)];
        }

        // Deriving one seed per shot makes results independent of scheduling and thread count.
        // Disjoint output chunks preserve shot order without synchronization.
        let seeds: Vec<[u8; 32]> = (0..num_shots).map(|_| rng.r#gen()).collect();
        let worker_count = std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1)
            .min(num_shots);
        let chunk_len = num_shots.div_ceil(worker_count);
        let mut results = vec![Vec::new(); num_shots];

        std::thread::scope(|scope| {
            for (seed_chunk, result_chunk) in
                seeds.chunks(chunk_len).zip(results.chunks_mut(chunk_len))
            {
                scope.spawn(move || {
                    let mut shot = LossShot::new(self.num_qubits, self.num_measurements);
                    for (seed, result) in seed_chunk.iter().zip(result_chunk) {
                        shot.reset();
                        let mut shot_rng = StdRng::from_seed(*seed);
                        shot.execute(&self.ops, &mut shot_rng);
                        *result = shot.measurements.clone();
                    }
                });
            }
        });

        results
    }
}

struct LossShot {
    tableau: PackedInverseTableau,
    lost: Vec<bool>,
    measurements: Vec<bool>,
}

impl LossShot {
    fn new(num_qubits: usize, num_measurements: usize) -> Self {
        Self {
            tableau: PackedInverseTableau::identity(num_qubits),
            lost: vec![false; num_qubits],
            measurements: Vec::with_capacity(num_measurements),
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
    use rand::SeedableRng;
    use rand::rngs::StdRng;

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
    fn parallel_loss_batch_is_seeded_and_schedule_independent() {
        let circuit =
            parse_lines("R 0 1\nH 0\nCX 0 1\nDEPOLARIZE2(0.2) 0 1\nLOSS(0.3) 0 1\nM 0 1\n")
                .unwrap();
        let plan = LossSamplerPlan::try_compile(&circuit).expect("supported loss plan");
        let mut first_rng = StdRng::seed_from_u64(0x5eed);
        let mut second_rng = StdRng::seed_from_u64(0x5eed);

        assert_eq!(
            plan.run_batch(257, &mut first_rng),
            plan.run_batch(257, &mut second_rng)
        );
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
