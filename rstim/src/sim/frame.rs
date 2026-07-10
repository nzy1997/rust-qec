use rand::Rng;
#[cfg(debug_assertions)]
use std::cell::RefCell;

use crate::compiled::{CompiledBasis, CompiledBlock, CompiledOp};
use crate::ir::{PauliBasis, StimInstr, StimTarget};
use crate::rare_error_iterator::RareErrorIndexSampler;
use crate::sim::bit_table::BitTable;
use crate::sim::measure_record_batch::MeasureRecordBatch;

#[cfg(debug_assertions)]
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Depolarize2SamplingTelemetry {
    pub sampling_path: &'static str,
    pub iterator_builds: usize,
    pub attempt_count: usize,
}

#[cfg(debug_assertions)]
impl Default for Depolarize2SamplingTelemetry {
    fn default() -> Self {
        Self {
            sampling_path: "none",
            iterator_builds: 0,
            attempt_count: 0,
        }
    }
}

#[cfg(debug_assertions)]
thread_local! {
    static DEPOLARIZE2_SAMPLING_TELEMETRY: RefCell<Depolarize2SamplingTelemetry> =
        const { RefCell::new(Depolarize2SamplingTelemetry {
            sampling_path: "none",
            iterator_builds: 0,
            attempt_count: 0,
        }) };
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn reset_depolarize2_sampling_telemetry() {
    DEPOLARIZE2_SAMPLING_TELEMETRY.with(|telemetry| {
        *telemetry.borrow_mut() = Depolarize2SamplingTelemetry::default();
    });
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn depolarize2_sampling_telemetry() -> Depolarize2SamplingTelemetry {
    DEPOLARIZE2_SAMPLING_TELEMETRY.with(|telemetry| *telemetry.borrow())
}

#[cfg(debug_assertions)]
fn record_depolarize2_sampling(
    sampling_path: &'static str,
    iterator_builds: usize,
    attempt_count: usize,
) {
    DEPOLARIZE2_SAMPLING_TELEMETRY.with(|telemetry| {
        *telemetry.borrow_mut() = Depolarize2SamplingTelemetry {
            sampling_path,
            iterator_builds,
            attempt_count,
        };
    });
}

pub struct FrameSimulator {
    pub num_qubits: usize,
    pub batch_size: usize,
    pub x_table: BitTable,
    pub z_table: BitTable,
    pub m_record: MeasureRecordBatch,
    last_correlated_error_occurred: Vec<u64>,
    depolarize_scratch: DepolarizeScratch,
    det_records: Vec<Vec<u64>>,
    obs_records: Vec<Vec<u64>>,
    materialize_detector_observable_outputs: bool,
    detector_materializations: usize,
    observable_materializations: usize,
}

impl FrameSimulator {
    pub fn new(num_qubits: usize, batch_size: usize) -> Self {
        let words_per_row = (batch_size + 63) / 64;
        Self {
            num_qubits,
            batch_size,
            x_table: BitTable::new(num_qubits, batch_size),
            z_table: BitTable::new(num_qubits, batch_size),
            m_record: MeasureRecordBatch::new(batch_size),
            last_correlated_error_occurred: vec![0u64; words_per_row],
            depolarize_scratch: DepolarizeScratch::new(),
            det_records: Vec::new(),
            obs_records: Vec::new(),
            materialize_detector_observable_outputs: true,
            detector_materializations: 0,
            observable_materializations: 0,
        }
    }

    pub(crate) fn randomize_initial_z_frames(&mut self, rng: &mut impl Rng) {
        for q in 0..self.num_qubits {
            self.z_table.randomize_row(q, rng);
        }
    }

    pub(crate) fn set_materialize_detector_observable_outputs(&mut self, enabled: bool) {
        self.materialize_detector_observable_outputs = enabled;
    }

    pub(crate) fn detector_materializations(&self) -> usize {
        self.detector_materializations
    }

    pub(crate) fn observable_materializations(&self) -> usize {
        self.observable_materializations
    }

    pub fn run(
        &mut self,
        instrs: &[StimInstr],
        ref_sample: &[bool],
        rng: &mut impl Rng,
    ) -> Result<(), String> {
        for instr in instrs {
            match instr {
                StimInstr::Op {
                    name,
                    args,
                    targets,
                    ..
                } => {
                    self.exec_op(name.as_str(), args, targets, ref_sample, rng)?;
                }
                StimInstr::Repeat { count, body } => {
                    for _ in 0..*count {
                        self.run(body, ref_sample, rng)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn run_compiled_blocks(
        &mut self,
        blocks: &[CompiledBlock],
        ref_sample: &[bool],
        rng: &mut impl Rng,
    ) -> Result<(), String> {
        for block in blocks {
            match block {
                CompiledBlock::Ops(ops) => {
                    for op in ops {
                        self.exec_compiled_op(op, ref_sample, rng)?;
                    }
                }
                CompiledBlock::Repeat(region) => {
                    for _ in 0..region.count {
                        self.run_compiled_blocks(&region.body, ref_sample, rng)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn exec_op(
        &mut self,
        name: &str,
        args: &[f64],
        targets: &[StimTarget],
        ref_sample: &[bool],
        rng: &mut impl Rng,
    ) -> Result<(), String> {
        let wpr = self.x_table.words_per_row();
        match name {
            "I" | "X" | "Y" | "Z" => {}

            "H" => {
                for q in qubits(targets)? {
                    do_h(&mut self.x_table, &mut self.z_table, q);
                }
            }
            "S" | "SQRT_Z" | "S_DAG" | "SQRT_Z_DAG" => {
                for q in qubits(targets)? {
                    do_s(&mut self.x_table, &mut self.z_table, q);
                }
            }
            "SQRT_X" | "SQRT_X_DAG" => {
                for q in qubits(targets)? {
                    do_sqrt_x(&mut self.x_table, &mut self.z_table, q);
                }
            }
            "SQRT_Y" | "SQRT_Y_DAG" => {
                for q in qubits(targets)? {
                    do_h(&mut self.x_table, &mut self.z_table, q);
                }
            }
            "H_XY" => {
                for q in qubits(targets)? {
                    do_s(&mut self.x_table, &mut self.z_table, q);
                }
            }
            "H_YZ" => {
                for q in qubits(targets)? {
                    do_sqrt_x(&mut self.x_table, &mut self.z_table, q);
                }
            }
            "C_XYZ" | "C_NXYZ" | "C_XNYZ" | "C_XYNZ" => {
                for q in qubits(targets)? {
                    do_c_xyz(&mut self.x_table, &mut self.z_table, q);
                }
            }
            "C_ZYX" | "C_NZYX" | "C_ZNYX" | "C_ZYNX" => {
                for q in qubits(targets)? {
                    do_c_zyx(&mut self.x_table, &mut self.z_table, q);
                }
            }
            "H_NXY" => {
                for q in qubits(targets)? {
                    do_s(&self.x_table, &mut self.z_table, q);
                }
            }
            "H_NXZ" => {
                for q in qubits(targets)? {
                    do_h(&mut self.x_table, &mut self.z_table, q);
                }
            }
            "H_NYZ" => {
                for q in qubits(targets)? {
                    do_sqrt_x(&mut self.x_table, &mut self.z_table, q);
                }
            }

            "CX" | "CNOT" | "ZCX" => {
                for (c, t) in qubit_pairs(targets)? {
                    do_cx(&mut self.x_table, &mut self.z_table, c, t);
                }
            }
            "CY" | "ZCY" => {
                for (c, t) in qubit_pairs(targets)? {
                    do_s(&mut self.x_table, &mut self.z_table, t);
                    do_cx(&mut self.x_table, &mut self.z_table, c, t);
                    do_s(&mut self.x_table, &mut self.z_table, t);
                }
            }
            "CZ" | "ZCZ" => {
                for (a, b) in qubit_pairs(targets)? {
                    do_cz(&self.x_table, &mut self.z_table, a, b);
                }
            }
            "XCX" => {
                for (a, b) in qubit_pairs(targets)? {
                    do_h(&mut self.x_table, &mut self.z_table, a);
                    do_cx(&mut self.x_table, &mut self.z_table, a, b);
                    do_h(&mut self.x_table, &mut self.z_table, a);
                }
            }
            "XCY" => {
                for (a, b) in qubit_pairs(targets)? {
                    do_sqrt_x(&mut self.x_table, &mut self.z_table, b);
                    do_cx(&mut self.x_table, &mut self.z_table, b, a);
                    do_sqrt_x(&mut self.x_table, &mut self.z_table, b);
                }
            }
            "XCZ" => {
                for (a, b) in qubit_pairs(targets)? {
                    do_cx(&mut self.x_table, &mut self.z_table, b, a);
                }
            }
            "YCX" => {
                for (a, b) in qubit_pairs(targets)? {
                    do_sqrt_x(&mut self.x_table, &mut self.z_table, a);
                    do_cx(&mut self.x_table, &mut self.z_table, a, b);
                    do_sqrt_x(&mut self.x_table, &mut self.z_table, a);
                }
            }
            "YCY" => {
                for (a, b) in qubit_pairs(targets)? {
                    do_sqrt_x(&mut self.x_table, &mut self.z_table, a);
                    do_sqrt_x(&mut self.x_table, &mut self.z_table, b);
                    do_cz(&self.x_table, &mut self.z_table, a, b);
                    do_sqrt_x(&mut self.x_table, &mut self.z_table, b);
                    do_sqrt_x(&mut self.x_table, &mut self.z_table, a);
                }
            }
            "YCZ" => {
                for (a, b) in qubit_pairs(targets)? {
                    do_s(&mut self.x_table, &mut self.z_table, a);
                    do_cx(&mut self.x_table, &mut self.z_table, b, a);
                    do_s(&mut self.x_table, &mut self.z_table, a);
                }
            }
            "SWAP" => {
                for (a, b) in qubit_pairs(targets)? {
                    do_swap(&mut self.x_table, &mut self.z_table, a, b);
                }
            }
            "ISWAP" | "ISWAP_DAG" => {
                for (a, b) in qubit_pairs(targets)? {
                    do_s(&mut self.x_table, &mut self.z_table, a);
                    do_s(&mut self.x_table, &mut self.z_table, b);
                    do_cz(&self.x_table, &mut self.z_table, a, b);
                    do_swap(&mut self.x_table, &mut self.z_table, a, b);
                }
            }
            "CXSWAP" => {
                for (a, b) in qubit_pairs(targets)? {
                    do_cx(&mut self.x_table, &mut self.z_table, b, a);
                    do_cx(&mut self.x_table, &mut self.z_table, a, b);
                }
            }
            "SWAPCX" => {
                for (a, b) in qubit_pairs(targets)? {
                    do_cx(&mut self.x_table, &mut self.z_table, a, b);
                    do_cx(&mut self.x_table, &mut self.z_table, b, a);
                }
            }
            "CZSWAP" => {
                for (a, b) in qubit_pairs(targets)? {
                    do_cz(&self.x_table, &mut self.z_table, a, b);
                    do_swap(&mut self.x_table, &mut self.z_table, a, b);
                }
            }

            "M" | "MZ" => {
                for q in qubits_ignoring_inv(targets)? {
                    self.m_record.push_row(self.x_table.row_words(q));
                    self.x_table.clear_row(q);
                    self.z_table.randomize_row(q, rng);
                }
            }
            "MX" => {
                for q in qubits_ignoring_inv(targets)? {
                    self.m_record.push_row(self.z_table.row_words(q));
                    self.z_table.clear_row(q);
                    self.x_table.randomize_row(q, rng);
                }
            }
            "MY" => {
                for q in qubits_ignoring_inv(targets)? {
                    let mut tmp = vec![0u64; wpr];
                    for w in 0..wpr {
                        tmp[w] = self.x_table.row_words(q)[w] ^ self.z_table.row_words(q)[w];
                    }
                    self.m_record.push_row(&tmp);
                    self.x_table.clear_row(q);
                    self.z_table.clear_row(q);
                    self.x_table.randomize_row(q, rng);
                    self.z_table.randomize_row(q, rng);
                }
            }
            "MR" | "MRZ" => {
                for q in qubits_ignoring_inv(targets)? {
                    self.m_record.push_row(self.x_table.row_words(q));
                    self.x_table.clear_row(q);
                    self.z_table.clear_row(q);
                    self.z_table.randomize_row(q, rng);
                }
            }
            "MRX" => {
                for q in qubits_ignoring_inv(targets)? {
                    self.m_record.push_row(self.z_table.row_words(q));
                    self.x_table.clear_row(q);
                    self.z_table.clear_row(q);
                    self.x_table.randomize_row(q, rng);
                }
            }
            "MRY" => {
                for q in qubits_ignoring_inv(targets)? {
                    let mut tmp = vec![0u64; wpr];
                    for w in 0..wpr {
                        tmp[w] = self.x_table.row_words(q)[w] ^ self.z_table.row_words(q)[w];
                    }
                    self.m_record.push_row(&tmp);
                    self.x_table.clear_row(q);
                    self.z_table.clear_row(q);
                    self.x_table.randomize_row(q, rng);
                    self.z_table.randomize_row(q, rng);
                }
            }

            "R" | "RZ" => {
                for q in qubits(targets)? {
                    self.x_table.clear_row(q);
                    self.z_table.randomize_row(q, rng);
                }
            }
            "RX" => {
                for q in qubits(targets)? {
                    self.z_table.clear_row(q);
                    self.x_table.randomize_row(q, rng);
                }
            }
            "RY" => {
                for q in qubits(targets)? {
                    self.x_table.clear_row(q);
                    self.z_table.clear_row(q);
                    self.x_table.randomize_row(q, rng);
                    self.z_table.randomize_row(q, rng);
                }
            }

            // --- Noise channels ---
            "X_ERROR" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits(targets)? {
                    let noise = random_bits_with_prob(wpr, self.batch_size, p, rng);
                    let x = self.x_table.row_words_mut(q);
                    for w in 0..wpr {
                        x[w] ^= noise[w];
                    }
                }
            }
            "Z_ERROR" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits(targets)? {
                    let noise = random_bits_with_prob(wpr, self.batch_size, p, rng);
                    let z = self.z_table.row_words_mut(q);
                    for w in 0..wpr {
                        z[w] ^= noise[w];
                    }
                }
            }
            "Y_ERROR" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits(targets)? {
                    let noise = random_bits_with_prob(wpr, self.batch_size, p, rng);
                    let x = self.x_table.row_words_mut(q);
                    for w in 0..wpr {
                        x[w] ^= noise[w];
                    }
                    let z = self.z_table.row_words_mut(q);
                    for w in 0..wpr {
                        z[w] ^= noise[w];
                    }
                }
            }
            "DEPOLARIZE1" => {
                let p = args.first().copied().unwrap_or(0.0);
                // random_bits_with_prob_into reuses depolarize scratch instead of allocating per target.
                self.exec_depolarize1(targets, p, wpr, rng)?;
            }
            "DEPOLARIZE2" => {
                let p = args.first().copied().unwrap_or(0.0);
                // random_bits_with_prob_into reuses depolarize scratch instead of allocating per target pair.
                self.exec_depolarize2(targets, p, wpr, rng)?;
            }

            "CORRELATED_ERROR" | "E" => {
                let p = args.first().copied().unwrap_or(0.0);
                let noise = random_bits_with_prob(wpr, self.batch_size, p, rng);
                apply_pauli_noise_to_targets(
                    &mut self.x_table,
                    &mut self.z_table,
                    targets,
                    &noise,
                    wpr,
                );
                self.last_correlated_error_occurred = noise;
            }
            "ELSE_CORRELATED_ERROR" => {
                let p = args.first().copied().unwrap_or(0.0);
                let candidate = random_bits_with_prob(wpr, self.batch_size, p, rng);
                let mut noise = vec![0u64; wpr];
                for w in 0..wpr {
                    noise[w] = candidate[w] & !self.last_correlated_error_occurred[w];
                }
                apply_pauli_noise_to_targets(
                    &mut self.x_table,
                    &mut self.z_table,
                    targets,
                    &noise,
                    wpr,
                );
                for w in 0..wpr {
                    self.last_correlated_error_occurred[w] |= noise[w];
                }
            }

            "PAULI_CHANNEL_1" => {
                let px = args.first().copied().unwrap_or(0.0);
                let py = args.get(1).copied().unwrap_or(0.0);
                let pz = args.get(2).copied().unwrap_or(0.0);
                for q in qubits(targets)? {
                    let mut xf = vec![0u64; wpr];
                    let mut zf = vec![0u64; wpr];
                    for w in 0..wpr {
                        for bit in 0..64u32 {
                            let r: f64 = rng.r#gen();
                            if r < px {
                                xf[w] |= 1u64 << bit;
                            } else if r < px + py {
                                xf[w] |= 1u64 << bit;
                                zf[w] |= 1u64 << bit;
                            } else if r < px + py + pz {
                                zf[w] |= 1u64 << bit;
                            }
                        }
                    }
                    let x = self.x_table.row_words_mut(q);
                    for w in 0..wpr {
                        x[w] ^= xf[w];
                    }
                    let z = self.z_table.row_words_mut(q);
                    for w in 0..wpr {
                        z[w] ^= zf[w];
                    }
                }
            }
            "PAULI_CHANNEL_2" => {
                let probs: Vec<f64> = (0..15)
                    .map(|i| args.get(i).copied().unwrap_or(0.0))
                    .collect();
                let mut cum = [0.0f64; 15];
                cum[0] = probs[0];
                for i in 1..15 {
                    cum[i] = cum[i - 1] + probs[i];
                }
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
                for (qa, qb) in qubit_pairs(targets)? {
                    let mut xa = vec![0u64; wpr];
                    let mut za = vec![0u64; wpr];
                    let mut xb = vec![0u64; wpr];
                    let mut zb = vec![0u64; wpr];
                    for w in 0..wpr {
                        for bit in 0..64u32 {
                            let r: f64 = rng.r#gen();
                            for (i, &(pa, pb)) in paulis.iter().enumerate() {
                                if r < cum[i] {
                                    apply_pauli_bits(pa, &mut xa, &mut za, w, bit);
                                    apply_pauli_bits(pb, &mut xb, &mut zb, w, bit);
                                    break;
                                }
                            }
                        }
                    }
                    let x = self.x_table.row_words_mut(qa);
                    for w in 0..wpr {
                        x[w] ^= xa[w];
                    }
                    let z = self.z_table.row_words_mut(qa);
                    for w in 0..wpr {
                        z[w] ^= za[w];
                    }
                    let x = self.x_table.row_words_mut(qb);
                    for w in 0..wpr {
                        x[w] ^= xb[w];
                    }
                    let z = self.z_table.row_words_mut(qb);
                    for w in 0..wpr {
                        z[w] ^= zb[w];
                    }
                }
            }

            "HERALDED_ERASE" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits(targets)? {
                    let herald = random_bits_with_prob(wpr, self.batch_size, p, rng);
                    self.m_record.push_row(&herald);
                    let mut xf = vec![0u64; wpr];
                    let mut zf = vec![0u64; wpr];
                    for w in 0..wpr {
                        let mut bits = herald[w];
                        while bits != 0 {
                            let bit = bits.trailing_zeros();
                            match rng.gen_range(0u8..4) {
                                1 => xf[w] |= 1u64 << bit,
                                2 => {
                                    xf[w] |= 1u64 << bit;
                                    zf[w] |= 1u64 << bit;
                                }
                                3 => zf[w] |= 1u64 << bit,
                                _ => {}
                            }
                            bits &= bits - 1;
                        }
                    }
                    let x = self.x_table.row_words_mut(q);
                    for w in 0..wpr {
                        x[w] ^= xf[w];
                    }
                    let z = self.z_table.row_words_mut(q);
                    for w in 0..wpr {
                        z[w] ^= zf[w];
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
                    let herald = random_bits_with_prob(wpr, self.batch_size, total, rng);
                    self.m_record.push_row(&herald);
                    let mut xf = vec![0u64; wpr];
                    let mut zf = vec![0u64; wpr];
                    for w in 0..wpr {
                        let mut bits = herald[w];
                        while bits != 0 {
                            let bit = bits.trailing_zeros();
                            let r: f64 = rng.r#gen::<f64>() * total;
                            if r < pi {
                                // I — false positive
                            } else if r < pi + px {
                                xf[w] |= 1u64 << bit;
                            } else if r < pi + px + py {
                                xf[w] |= 1u64 << bit;
                                zf[w] |= 1u64 << bit;
                            } else {
                                zf[w] |= 1u64 << bit;
                            }
                            bits &= bits - 1;
                        }
                    }
                    let x = self.x_table.row_words_mut(q);
                    for w in 0..wpr {
                        x[w] ^= xf[w];
                    }
                    let z = self.z_table.row_words_mut(q);
                    for w in 0..wpr {
                        z[w] ^= zf[w];
                    }
                }
            }

            "I_ERROR" | "II_ERROR" => {}

            // --- Multi-qubit Pauli measurements ---
            "MPP" => {
                let products = split_pauli_products(targets)?;
                for product in &products {
                    self.exec_mpp_product(product, rng);
                }
            }

            "MXX" => {
                for (a, b) in qubit_pairs_ignoring_inv(targets)? {
                    let product = PauliProduct {
                        terms: vec![(a, PauliBasis::X), (b, PauliBasis::X)],
                        inverted: false,
                    };
                    self.exec_mpp_product(&product, rng);
                }
            }
            "MYY" => {
                for (a, b) in qubit_pairs_ignoring_inv(targets)? {
                    let product = PauliProduct {
                        terms: vec![(a, PauliBasis::Y), (b, PauliBasis::Y)],
                        inverted: false,
                    };
                    self.exec_mpp_product(&product, rng);
                }
            }
            "MZZ" => {
                for (a, b) in qubit_pairs_ignoring_inv(targets)? {
                    let product = PauliProduct {
                        terms: vec![(a, PauliBasis::Z), (b, PauliBasis::Z)],
                        inverted: false,
                    };
                    self.exec_mpp_product(&product, rng);
                }
            }

            // --- SPP ---
            "SPP" | "SPP_DAG" => {
                let products = split_pauli_products(targets)?;
                for product in &products {
                    self.exec_spp_product(product);
                }
            }

            "DETECTOR" => {
                if !self.materialize_detector_observable_outputs {
                    return Ok(());
                }
                self.detector_materializations += 1;
                let wpr = self.m_record.words_per_row();
                let mut result = vec![0u64; wpr];
                let mut ref_parity = false;
                for t in targets {
                    if let StimTarget::Rec(offset) = t {
                        let k = (-*offset) as usize;
                        self.m_record.xor_lookback_into(k, &mut result);
                        let m_idx = self.m_record.len() - k;
                        if m_idx < ref_sample.len() && ref_sample[m_idx] {
                            ref_parity = !ref_parity;
                        }
                    }
                }
                if ref_parity {
                    for w in &mut result {
                        *w ^= !0u64;
                    }
                }
                self.det_records.push(result);
            }
            "OBSERVABLE_INCLUDE" => {
                if !self.materialize_detector_observable_outputs {
                    return Ok(());
                }
                self.observable_materializations += 1;
                let idx = args.first().copied().unwrap_or(0.0) as usize;
                let wpr = self.m_record.words_per_row();
                while self.obs_records.len() <= idx {
                    self.obs_records.push(vec![0u64; wpr]);
                }
                let mut ref_parity = false;
                for t in targets {
                    if let StimTarget::Rec(offset) = t {
                        let k = (-*offset) as usize;
                        self.m_record
                            .xor_lookback_into(k, &mut self.obs_records[idx]);
                        let m_idx = self.m_record.len() - k;
                        if m_idx < ref_sample.len() && ref_sample[m_idx] {
                            ref_parity = !ref_parity;
                        }
                    }
                }
                if ref_parity {
                    for w in &mut self.obs_records[idx] {
                        *w ^= !0u64;
                    }
                }
            }

            "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" => {}

            "MPAD" => {
                for _t in targets {
                    self.m_record.push_zeros();
                }
            }

            _ => return Err(format!("frame_sim: unsupported instruction {}", name)),
        }
        Ok(())
    }

    fn exec_compiled_op(
        &mut self,
        op: &CompiledOp,
        ref_sample: &[bool],
        rng: &mut impl Rng,
    ) -> Result<(), String> {
        let wpr = self.x_table.words_per_row();
        match op {
            CompiledOp::Tick
            | CompiledOp::QubitCoords
            | CompiledOp::ShiftCoords
            | CompiledOp::NoOp => {}
            CompiledOp::H { qubits } => {
                for &q in qubits {
                    do_h(&mut self.x_table, &mut self.z_table, q);
                }
            }
            CompiledOp::Reset { basis, qubits } => self.exec_compiled_reset(*basis, qubits, rng),
            CompiledOp::XError {
                probability,
                qubits,
            } => {
                for &q in qubits {
                    let noise = random_bits_with_prob(wpr, self.batch_size, *probability, rng);
                    let x = self.x_table.row_words_mut(q);
                    for w in 0..wpr {
                        x[w] ^= noise[w];
                    }
                }
            }
            CompiledOp::Depolarize1 {
                probability,
                qubits,
            } => {
                self.exec_depolarize1_qubits(qubits, *probability, wpr, rng);
            }
            CompiledOp::Cx { pairs } => {
                for &(control, target) in pairs {
                    do_cx(&mut self.x_table, &mut self.z_table, control, target);
                }
            }
            CompiledOp::Depolarize2 { probability, pairs } => {
                self.exec_depolarize2_pairs(pairs, *probability, wpr, rng);
            }
            CompiledOp::Measure { basis, qubits } => {
                self.exec_compiled_measure(*basis, qubits, wpr, rng);
            }
            CompiledOp::MeasureReset { basis, qubits } => {
                self.exec_compiled_measure_reset(*basis, qubits, wpr, rng);
            }
            CompiledOp::Detector { rec_offsets } => {
                self.exec_compiled_detector(rec_offsets, ref_sample);
            }
            CompiledOp::ObservableInclude {
                observable_index,
                rec_offsets,
            } => self.exec_compiled_observable_include(*observable_index, rec_offsets, ref_sample),
            CompiledOp::UnsupportedSamplerOp { name } => {
                return Err(format!("compiled sampler: unsupported instruction {name}"));
            }
        }
        Ok(())
    }

    fn exec_mpp_product(&mut self, product: &PauliProduct, rng: &mut impl Rng) {
        if product.terms.is_empty() {
            if product.inverted {
                let wpr = self.x_table.words_per_row();
                let ones = vec![!0u64; wpr];
                self.m_record.push_row(&ones);
            } else {
                self.m_record.push_zeros();
            }
            return;
        }

        // 1. Basis change
        for &(q, basis) in &product.terms {
            match basis {
                PauliBasis::X => do_h(&mut self.x_table, &mut self.z_table, q),
                PauliBasis::Y => do_sqrt_x(&mut self.x_table, &mut self.z_table, q),
                PauliBasis::Z => {}
            }
        }

        // 2. CX fold onto anchor
        let anchor = product.terms.last().unwrap().0;
        let non_anchor: Vec<usize> = product
            .terms
            .iter()
            .map(|&(q, _)| q)
            .filter(|&q| q != anchor)
            .collect();
        for &q in &non_anchor {
            do_cx(&mut self.x_table, &mut self.z_table, q, anchor);
        }

        // 3. Measure Z on anchor
        self.m_record.push_row(self.x_table.row_words(anchor));
        self.x_table.clear_row(anchor);
        self.z_table.randomize_row(anchor, rng);

        // 4. Undo CX fold
        for &q in non_anchor.iter().rev() {
            do_cx(&mut self.x_table, &mut self.z_table, q, anchor);
        }

        // 5. Undo basis change
        for &(q, basis) in &product.terms {
            match basis {
                PauliBasis::X => do_h(&mut self.x_table, &mut self.z_table, q),
                PauliBasis::Y => do_sqrt_x(&mut self.x_table, &mut self.z_table, q),
                PauliBasis::Z => {}
            }
        }
    }

    fn exec_spp_product(&mut self, product: &PauliProduct) {
        if product.terms.is_empty() {
            return;
        }

        // 1. Basis change
        for &(q, basis) in &product.terms {
            match basis {
                PauliBasis::X => do_h(&mut self.x_table, &mut self.z_table, q),
                PauliBasis::Y => do_sqrt_x(&mut self.x_table, &mut self.z_table, q),
                PauliBasis::Z => {}
            }
        }

        // 2. CX fold onto anchor
        let anchor = product.terms.last().unwrap().0;
        let non_anchor: Vec<usize> = product
            .terms
            .iter()
            .map(|&(q, _)| q)
            .filter(|&q| q != anchor)
            .collect();
        for &q in &non_anchor {
            do_cx(&mut self.x_table, &mut self.z_table, q, anchor);
        }

        // 3. S on anchor (z ^= x) — identical for S and S_DAG in frame picture
        do_s(&mut self.x_table, &mut self.z_table, anchor);

        // 4. Undo CX fold
        for &q in non_anchor.iter().rev() {
            do_cx(&mut self.x_table, &mut self.z_table, q, anchor);
        }

        // 5. Undo basis change
        for &(q, basis) in &product.terms {
            match basis {
                PauliBasis::X => do_h(&mut self.x_table, &mut self.z_table, q),
                PauliBasis::Y => do_sqrt_x(&mut self.x_table, &mut self.z_table, q),
                PauliBasis::Z => {}
            }
        }
    }

    fn exec_compiled_measure(
        &mut self,
        basis: CompiledBasis,
        qubits: &[usize],
        wpr: usize,
        rng: &mut impl Rng,
    ) {
        for &q in qubits {
            match basis {
                CompiledBasis::Z => {
                    self.m_record.push_row(self.x_table.row_words(q));
                    self.x_table.clear_row(q);
                    self.z_table.randomize_row(q, rng);
                }
                CompiledBasis::X => {
                    self.m_record.push_row(self.z_table.row_words(q));
                    self.z_table.clear_row(q);
                    self.x_table.randomize_row(q, rng);
                }
                CompiledBasis::Y => {
                    let mut tmp = vec![0u64; wpr];
                    for (w, word) in tmp.iter_mut().enumerate() {
                        *word = self.x_table.row_words(q)[w] ^ self.z_table.row_words(q)[w];
                    }
                    self.m_record.push_row(&tmp);
                    self.x_table.clear_row(q);
                    self.z_table.clear_row(q);
                    self.x_table.randomize_row(q, rng);
                    self.z_table.randomize_row(q, rng);
                }
            }
        }
    }

    fn exec_compiled_measure_reset(
        &mut self,
        basis: CompiledBasis,
        qubits: &[usize],
        wpr: usize,
        rng: &mut impl Rng,
    ) {
        for &q in qubits {
            match basis {
                CompiledBasis::Z => {
                    self.m_record.push_row(self.x_table.row_words(q));
                    self.x_table.clear_row(q);
                    self.z_table.clear_row(q);
                    self.z_table.randomize_row(q, rng);
                }
                CompiledBasis::X => {
                    self.m_record.push_row(self.z_table.row_words(q));
                    self.x_table.clear_row(q);
                    self.z_table.clear_row(q);
                    self.x_table.randomize_row(q, rng);
                }
                CompiledBasis::Y => {
                    let mut tmp = vec![0u64; wpr];
                    for (w, word) in tmp.iter_mut().enumerate() {
                        *word = self.x_table.row_words(q)[w] ^ self.z_table.row_words(q)[w];
                    }
                    self.m_record.push_row(&tmp);
                    self.x_table.clear_row(q);
                    self.z_table.clear_row(q);
                    self.x_table.randomize_row(q, rng);
                    self.z_table.randomize_row(q, rng);
                }
            }
        }
    }

    fn exec_compiled_reset(&mut self, basis: CompiledBasis, qubits: &[usize], rng: &mut impl Rng) {
        for &q in qubits {
            match basis {
                CompiledBasis::Z => {
                    self.x_table.clear_row(q);
                    self.z_table.randomize_row(q, rng);
                }
                CompiledBasis::X => {
                    self.z_table.clear_row(q);
                    self.x_table.randomize_row(q, rng);
                }
                CompiledBasis::Y => {
                    self.x_table.clear_row(q);
                    self.z_table.clear_row(q);
                    self.x_table.randomize_row(q, rng);
                    self.z_table.randomize_row(q, rng);
                }
            }
        }
    }

    fn exec_depolarize1(
        &mut self,
        targets: &[StimTarget],
        p: f64,
        wpr: usize,
        rng: &mut impl Rng,
    ) -> Result<(), String> {
        let qubits = qubits(targets)?;
        self.exec_depolarize1_qubits(&qubits, p, wpr, rng);
        Ok(())
    }

    fn exec_depolarize2(
        &mut self,
        targets: &[StimTarget],
        p: f64,
        wpr: usize,
        rng: &mut impl Rng,
    ) -> Result<(), String> {
        let pairs = qubit_pairs(targets)?;
        self.exec_depolarize2_pairs(&pairs, p, wpr, rng);
        Ok(())
    }

    fn exec_depolarize1_qubits(
        &mut self,
        qubits: &[usize],
        p: f64,
        wpr: usize,
        rng: &mut impl Rng,
    ) {
        if p <= 0.0 {
            return;
        }
        for &q in qubits {
            {
                let scratch = &mut self.depolarize_scratch;
                scratch.prepare_one(wpr);
                random_bits_with_prob_into(&mut scratch.events, self.batch_size, p, rng);
                for w in 0..wpr {
                    let mut bits = scratch.events[w];
                    while bits != 0 {
                        let bit = bits.trailing_zeros();
                        match rng.gen_range(0u8..3) {
                            0 => scratch.x_a[w] |= 1u64 << bit,
                            1 => {
                                scratch.x_a[w] |= 1u64 << bit;
                                scratch.z_a[w] |= 1u64 << bit;
                            }
                            _ => scratch.z_a[w] |= 1u64 << bit,
                        }
                        bits &= bits - 1;
                    }
                }
            }
            let scratch = &self.depolarize_scratch;
            let x = self.x_table.row_words_mut(q);
            for w in 0..wpr {
                x[w] ^= scratch.x_a[w];
            }
            let z = self.z_table.row_words_mut(q);
            for w in 0..wpr {
                z[w] ^= scratch.z_a[w];
            }
        }
    }

    fn exec_depolarize2_pairs(
        &mut self,
        pairs: &[(usize, usize)],
        p: f64,
        wpr: usize,
        rng: &mut impl Rng,
    ) {
        if p <= 0.0 || pairs.is_empty() || self.batch_size == 0 {
            #[cfg(debug_assertions)]
            record_depolarize2_sampling("empty", 0, pairs.len().saturating_mul(self.batch_size));
            return;
        }

        let attempt_count = pairs.len() * self.batch_size;
        if p <= SPARSE_BERNOULLI_MAX_PROBABILITY {
            self.exec_depolarize2_pairs_sparse_instruction_wide(pairs, p, attempt_count, rng);
        } else {
            self.exec_depolarize2_pairs_dense(pairs, p, wpr, rng);
        }
    }

    fn exec_depolarize2_pairs_sparse_instruction_wide(
        &mut self,
        pairs: &[(usize, usize)],
        p: f64,
        attempt_count: usize,
        rng: &mut impl Rng,
    ) {
        #[cfg(debug_assertions)]
        let iterator_builds_before =
            crate::rare_error_iterator::rare_error_telemetry().iterator_builds;

        let mut events = RareErrorIndexSampler::new(p, attempt_count);

        #[cfg(debug_assertions)]
        {
            let iterator_builds_after =
                crate::rare_error_iterator::rare_error_telemetry().iterator_builds;
            record_depolarize2_sampling(
                "sparse",
                iterator_builds_after.saturating_sub(iterator_builds_before),
                attempt_count,
            );
        }

        while let Some(event_index) = events.next_index(rng) {
            let (pair_index, shot_index) = decode_depolarize2_event(event_index, self.batch_size);
            let (qa, qb) = pairs[pair_index];
            let branch = sample_depolarize2_branch_index(rng);
            let (pa, pb) = two_qubit_pauli(branch);
            let word = shot_index / 64;
            let bit = (shot_index % 64) as u32;
            let mask = 1u64 << bit;
            apply_pauli_mask_to_tables(pa, &mut self.x_table, &mut self.z_table, qa, word, mask);
            apply_pauli_mask_to_tables(pb, &mut self.x_table, &mut self.z_table, qb, word, mask);
        }
    }

    fn exec_depolarize2_pairs_dense(
        &mut self,
        pairs: &[(usize, usize)],
        p: f64,
        wpr: usize,
        rng: &mut impl Rng,
    ) {
        #[cfg(debug_assertions)]
        record_depolarize2_sampling("dense", 0, pairs.len() * self.batch_size);

        for &(qa, qb) in pairs {
            {
                let scratch = &mut self.depolarize_scratch;
                scratch.prepare_two(wpr);
                random_bits_with_prob_into(&mut scratch.events, self.batch_size, p, rng);
                for w in 0..wpr {
                    let mut bits = scratch.events[w];
                    while bits != 0 {
                        let bit = bits.trailing_zeros();
                        let branch = sample_depolarize2_branch_index(rng);
                        let (pa, pb) = two_qubit_pauli(branch);
                        apply_pauli_bits(pa, &mut scratch.x_a, &mut scratch.z_a, w, bit);
                        apply_pauli_bits(pb, &mut scratch.x_b, &mut scratch.z_b, w, bit);
                        bits &= bits - 1;
                    }
                }
            }
            let scratch = &self.depolarize_scratch;
            let x = self.x_table.row_words_mut(qa);
            for w in 0..wpr {
                x[w] ^= scratch.x_a[w];
            }
            let z = self.z_table.row_words_mut(qa);
            for w in 0..wpr {
                z[w] ^= scratch.z_a[w];
            }
            let x = self.x_table.row_words_mut(qb);
            for w in 0..wpr {
                x[w] ^= scratch.x_b[w];
            }
            let z = self.z_table.row_words_mut(qb);
            for w in 0..wpr {
                z[w] ^= scratch.z_b[w];
            }
        }
    }

    fn exec_compiled_detector(&mut self, rec_offsets: &[usize], ref_sample: &[bool]) {
        if !self.materialize_detector_observable_outputs {
            return;
        }
        self.detector_materializations += 1;
        let wpr = self.m_record.words_per_row();
        let mut result = vec![0u64; wpr];
        let mut ref_parity = false;
        for &k in rec_offsets {
            self.m_record.xor_lookback_into(k, &mut result);
            let m_idx = self.m_record.len() - k;
            if m_idx < ref_sample.len() && ref_sample[m_idx] {
                ref_parity = !ref_parity;
            }
        }
        if ref_parity {
            for word in &mut result {
                *word ^= !0u64;
            }
        }
        self.det_records.push(result);
    }

    fn exec_compiled_observable_include(
        &mut self,
        observable_index: usize,
        rec_offsets: &[usize],
        ref_sample: &[bool],
    ) {
        if !self.materialize_detector_observable_outputs {
            return;
        }
        self.observable_materializations += 1;
        let wpr = self.m_record.words_per_row();
        while self.obs_records.len() <= observable_index {
            self.obs_records.push(vec![0u64; wpr]);
        }
        let mut ref_parity = false;
        for &k in rec_offsets {
            self.m_record
                .xor_lookback_into(k, &mut self.obs_records[observable_index]);
            let m_idx = self.m_record.len() - k;
            if m_idx < ref_sample.len() && ref_sample[m_idx] {
                ref_parity = !ref_parity;
            }
        }
        if ref_parity {
            for word in &mut self.obs_records[observable_index] {
                *word ^= !0u64;
            }
        }
    }

    pub fn measurements(&self, ref_sample: &[bool]) -> BitTable {
        let num_measurements = self.m_record.len();
        let mut result = BitTable::new(num_measurements, self.batch_size);
        for m in 0..num_measurements {
            let src = self.m_record.lookback_words(num_measurements - m);
            let dst = result.row_words_mut(m);
            dst.copy_from_slice(src);
            if ref_sample[m] {
                for w in dst.iter_mut() {
                    *w = !*w;
                }
            }
        }
        result
    }

    pub fn detections(&self) -> BitTable {
        let n = self.det_records.len();
        let mut result = BitTable::new(n, self.batch_size);
        for (i, row) in self.det_records.iter().enumerate() {
            result.row_words_mut(i).copy_from_slice(row);
        }
        result
    }

    pub fn observable_flips(&self) -> BitTable {
        let n = self.obs_records.len();
        let mut result = BitTable::new(n, self.batch_size);
        for (i, row) in self.obs_records.iter().enumerate() {
            result.row_words_mut(i).copy_from_slice(row);
        }
        result
    }
}

// --- Frame gate primitives ---

fn do_c_xyz(x_table: &mut BitTable, z_table: &mut BitTable, q: usize) {
    let wpr = x_table.words_per_row();
    for w in 0..wpr {
        let xi = x_table.row_words(q)[w];
        let zi = z_table.row_words(q)[w];
        x_table.row_words_mut(q)[w] = xi ^ zi;
        z_table.row_words_mut(q)[w] = xi;
    }
}

fn do_c_zyx(x_table: &mut BitTable, z_table: &mut BitTable, q: usize) {
    let wpr = x_table.words_per_row();
    for w in 0..wpr {
        let xi = x_table.row_words(q)[w];
        let zi = z_table.row_words(q)[w];
        x_table.row_words_mut(q)[w] = zi;
        z_table.row_words_mut(q)[w] = xi ^ zi;
    }
}

fn do_h(x_table: &mut BitTable, z_table: &mut BitTable, q: usize) {
    let wpr = x_table.words_per_row();
    for w in 0..wpr {
        let xi = x_table.row_words(q)[w];
        let zi = z_table.row_words(q)[w];
        x_table.row_words_mut(q)[w] = zi;
        z_table.row_words_mut(q)[w] = xi;
    }
}

fn do_s(x_table: &BitTable, z_table: &mut BitTable, q: usize) {
    let wpr = x_table.words_per_row();
    for w in 0..wpr {
        z_table.row_words_mut(q)[w] ^= x_table.row_words(q)[w];
    }
}

fn do_sqrt_x(x_table: &mut BitTable, z_table: &BitTable, q: usize) {
    let wpr = z_table.words_per_row();
    for w in 0..wpr {
        x_table.row_words_mut(q)[w] ^= z_table.row_words(q)[w];
    }
}

fn do_cx(x_table: &mut BitTable, z_table: &mut BitTable, c: usize, t: usize) {
    x_table.xor_row(t, c);
    z_table.xor_row(c, t);
}

fn do_cz(x_table: &BitTable, z_table: &mut BitTable, a: usize, b: usize) {
    let wpr = x_table.words_per_row();
    let xa: Vec<u64> = x_table.row_words(a).to_vec();
    let xb: Vec<u64> = x_table.row_words(b).to_vec();
    for w in 0..wpr {
        z_table.row_words_mut(a)[w] ^= xb[w];
        z_table.row_words_mut(b)[w] ^= xa[w];
    }
}

fn do_swap(x_table: &mut BitTable, z_table: &mut BitTable, a: usize, b: usize) {
    x_table.swap_rows(a, b);
    z_table.swap_rows(a, b);
}

// --- Target helpers ---

fn qubits(targets: &[StimTarget]) -> Result<Vec<usize>, String> {
    let mut out = Vec::new();
    for t in targets {
        match t {
            StimTarget::Qubit(q) => out.push(*q as usize),
            StimTarget::Sweep(_) => {} // skip: treated as always-0 (no-op)
            _ => return Err("expected qubit target".to_string()),
        }
    }
    Ok(out)
}

fn qubits_ignoring_inv(targets: &[StimTarget]) -> Result<Vec<usize>, String> {
    let mut out = Vec::new();
    for t in targets {
        match t {
            StimTarget::Qubit(q) | StimTarget::QubitInv(q) => out.push(*q as usize),
            StimTarget::Sweep(_) => {} // skip: treated as always-0 (no-op)
            _ => return Err("expected qubit target".to_string()),
        }
    }
    Ok(out)
}

fn qubit_pairs(targets: &[StimTarget]) -> Result<Vec<(usize, usize)>, String> {
    if targets.len() % 2 != 0 {
        return Err("odd number of targets for pair gate".to_string());
    }
    let mut out = Vec::new();
    let mut it = targets.iter();
    while let (Some(a), Some(b)) = (it.next(), it.next()) {
        // Skip any pair that contains a sweep target (sweep=0 means gate is no-op)
        if matches!(a, StimTarget::Sweep(_)) || matches!(b, StimTarget::Sweep(_)) {
            continue;
        }
        let qa = match a {
            StimTarget::Qubit(q) => *q as usize,
            _ => return Err("expected qubit target in pair".to_string()),
        };
        let qb = match b {
            StimTarget::Qubit(q) => *q as usize,
            _ => return Err("expected qubit target in pair".to_string()),
        };
        out.push((qa, qb));
    }
    Ok(out)
}

fn qubit_pairs_ignoring_inv(targets: &[StimTarget]) -> Result<Vec<(usize, usize)>, String> {
    if targets.len() % 2 != 0 {
        return Err("odd number of targets for pair measurement".to_string());
    }
    let mut out = Vec::new();
    let mut it = targets.iter();
    while let (Some(a), Some(b)) = (it.next(), it.next()) {
        // Skip any pair that contains a sweep target (sweep=0 means gate is no-op)
        if matches!(a, StimTarget::Sweep(_)) || matches!(b, StimTarget::Sweep(_)) {
            continue;
        }
        let qa = match a {
            StimTarget::Qubit(q) | StimTarget::QubitInv(q) => *q as usize,
            _ => return Err("expected qubit target in pair".to_string()),
        };
        let qb = match b {
            StimTarget::Qubit(q) | StimTarget::QubitInv(q) => *q as usize,
            _ => return Err("expected qubit target in pair".to_string()),
        };
        out.push((qa, qb));
    }
    Ok(out)
}

// --- Noise helpers ---

const SPARSE_BERNOULLI_MAX_PROBABILITY: f64 = 0.02;

const DEPOLARIZE2_BRANCHES: [(u8, u8); 15] = [
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

#[cfg(debug_assertions)]
const DEPOLARIZE2_BRANCH_LABELS: [&str; 15] = [
    "IX", "IY", "IZ", "XI", "XX", "XY", "XZ", "YI", "YX", "YY", "YZ", "ZI", "ZX", "ZY", "ZZ",
];

fn sample_depolarize2_branch_index(rng: &mut impl Rng) -> usize {
    rng.gen_range(0..DEPOLARIZE2_BRANCHES.len())
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn sample_depolarize2_branch_index_for_test(rng: &mut impl Rng) -> usize {
    sample_depolarize2_branch_index(rng)
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn depolarize2_branch_label_for_test(branch_index: usize) -> Option<&'static str> {
    DEPOLARIZE2_BRANCH_LABELS.get(branch_index).copied()
}

fn decode_depolarize2_event(event_index: usize, shots: usize) -> (usize, usize) {
    debug_assert!(shots > 0);
    (event_index / shots, event_index % shots)
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn depolarize2_decode_event_for_test(event_index: usize, shots: usize) -> (usize, usize) {
    decode_depolarize2_event(event_index, shots)
}

fn random_bits_with_prob(words: usize, valid_bits: usize, p: f64, rng: &mut impl Rng) -> Vec<u64> {
    let mut result = vec![0u64; words];
    random_bits_with_prob_into(&mut result, valid_bits, p, rng);
    result
}

fn random_bits_with_prob_into(result: &mut [u64], valid_bits: usize, p: f64, rng: &mut impl Rng) {
    result.fill(0);
    if p <= 0.0 {
        return;
    }
    if p >= 1.0 {
        result.fill(!0u64);
        mask_unused_bits(result, valid_bits);
        return;
    }

    let threshold = probability_threshold_u64(p);
    if threshold == 0 {
        return;
    }

    if p <= SPARSE_BERNOULLI_MAX_PROBABILITY {
        random_sparse_bits_with_prob_into(result, valid_bits, p, rng);
    } else {
        random_dense_bits_with_threshold_into(result, valid_bits, threshold, rng);
    }
}

fn random_dense_bits_with_threshold_into(
    result: &mut [u64],
    valid_bits: usize,
    threshold: u64,
    rng: &mut impl Rng,
) {
    for (word_idx, word) in result.iter_mut().enumerate() {
        let valid_in_word = valid_bits.saturating_sub(word_idx * 64).min(64);
        for bit in 0..valid_in_word {
            if rng.r#gen::<u64>() < threshold {
                *word |= 1u64 << bit;
            }
        }
    }
}

fn random_sparse_bits_with_prob_into(
    result: &mut [u64],
    valid_bits: usize,
    p: f64,
    rng: &mut impl Rng,
) {
    debug_assert!(p > 0.0 && p <= SPARSE_BERNOULLI_MAX_PROBABILITY);
    debug_assert!(valid_bits <= result.len().saturating_mul(64));
    let log_one_minus_p = (-p).ln_1p();
    let mut next_candidate = 0usize;
    while next_candidate < valid_bits {
        let mut u = rng.r#gen::<f64>();
        while u == 0.0 {
            u = rng.r#gen::<f64>();
        }
        let skip = (u.ln() / log_one_minus_p).floor() as usize;
        let shot = next_candidate.saturating_add(skip);
        if shot >= valid_bits {
            break;
        }
        let word_idx = shot / 64;
        let bit = shot % 64;
        let mask = 1u64 << bit;
        result[word_idx] |= mask;
        next_candidate = shot + 1;
    }
}

#[derive(Default)]
struct DepolarizeScratch {
    events: Vec<u64>,
    x_a: Vec<u64>,
    z_a: Vec<u64>,
    x_b: Vec<u64>,
    z_b: Vec<u64>,
}

impl DepolarizeScratch {
    fn new() -> Self {
        Self::default()
    }

    fn prepare_one(&mut self, words: usize) {
        resize_and_clear(&mut self.events, words);
        resize_and_clear(&mut self.x_a, words);
        resize_and_clear(&mut self.z_a, words);
    }

    fn prepare_two(&mut self, words: usize) {
        self.prepare_one(words);
        resize_and_clear(&mut self.x_b, words);
        resize_and_clear(&mut self.z_b, words);
    }
}

fn resize_and_clear(words: &mut Vec<u64>, len: usize) {
    words.resize(len, 0);
    words.fill(0);
}

fn probability_threshold_u64(p: f64) -> u64 {
    (p * 18_446_744_073_709_551_616.0) as u64
}

fn mask_unused_bits(words: &mut [u64], valid_bits: usize) {
    if words.is_empty() {
        return;
    }
    let valid_in_last = valid_bits % 64;
    if valid_in_last != 0 {
        let mask = (1u64 << valid_in_last) - 1;
        if let Some(last) = words.last_mut() {
            *last &= mask;
        }
    }
}

fn apply_pauli_noise_to_targets(
    x_table: &mut BitTable,
    z_table: &mut BitTable,
    targets: &[StimTarget],
    noise: &[u64],
    wpr: usize,
) {
    for t in targets {
        if let StimTarget::Pauli { qubit, basis, .. } = t {
            let q = *qubit as usize;
            match basis {
                PauliBasis::X => {
                    let x = x_table.row_words_mut(q);
                    for w in 0..wpr {
                        x[w] ^= noise[w];
                    }
                }
                PauliBasis::Y => {
                    let x = x_table.row_words_mut(q);
                    for w in 0..wpr {
                        x[w] ^= noise[w];
                    }
                    let z = z_table.row_words_mut(q);
                    for w in 0..wpr {
                        z[w] ^= noise[w];
                    }
                }
                PauliBasis::Z => {
                    let z = z_table.row_words_mut(q);
                    for w in 0..wpr {
                        z[w] ^= noise[w];
                    }
                }
            }
        }
    }
}

fn two_qubit_pauli(branch: usize) -> (u8, u8) {
    DEPOLARIZE2_BRANCHES
        .get(branch)
        .copied()
        .expect("DEPOLARIZE2 branch index must be in 0..15")
}

fn apply_pauli_bits(p: u8, xf: &mut [u64], zf: &mut [u64], w: usize, bit: u32) {
    match p {
        1 => xf[w] |= 1u64 << bit,
        2 => {
            xf[w] |= 1u64 << bit;
            zf[w] |= 1u64 << bit;
        }
        3 => zf[w] |= 1u64 << bit,
        _ => {}
    }
}

fn apply_pauli_mask_to_tables(
    p: u8,
    x_table: &mut BitTable,
    z_table: &mut BitTable,
    q: usize,
    word: usize,
    mask: u64,
) {
    match p {
        1 => x_table.row_words_mut(q)[word] ^= mask,
        2 => {
            x_table.row_words_mut(q)[word] ^= mask;
            z_table.row_words_mut(q)[word] ^= mask;
        }
        3 => z_table.row_words_mut(q)[word] ^= mask,
        _ => {}
    }
}

// --- Pauli product helpers ---

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
            _ => return Err("MPP/SPP targets must be Pauli targets".to_string()),
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
