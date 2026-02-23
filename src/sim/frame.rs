use rand::Rng;

use crate::ir::{PauliBasis, StimInstr, StimTarget};
use crate::sim::bit_table::BitTable;
use crate::sim::measure_record_batch::MeasureRecordBatch;

pub struct FrameSimulator {
    pub num_qubits: usize,
    pub batch_size: usize,
    pub x_table: BitTable,
    pub z_table: BitTable,
    pub m_record: MeasureRecordBatch,
    last_correlated_error_occurred: Vec<u64>,
    det_records: Vec<Vec<u64>>,
    obs_records: Vec<Vec<u64>>,
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
            det_records: Vec::new(),
            obs_records: Vec::new(),
        }
    }

    pub fn run(
        &mut self,
        instrs: &[StimInstr],
        ref_sample: &[bool],
        rng: &mut impl Rng,
    ) -> Result<(), String> {
        for instr in instrs {
            match instr {
                StimInstr::Op { name, args, targets, .. } => {
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
                    let noise = random_bits_with_prob(wpr, p, rng);
                    let x = self.x_table.row_words_mut(q);
                    for w in 0..wpr { x[w] ^= noise[w]; }
                }
            }
            "Z_ERROR" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits(targets)? {
                    let noise = random_bits_with_prob(wpr, p, rng);
                    let z = self.z_table.row_words_mut(q);
                    for w in 0..wpr { z[w] ^= noise[w]; }
                }
            }
            "Y_ERROR" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits(targets)? {
                    let noise = random_bits_with_prob(wpr, p, rng);
                    let x = self.x_table.row_words_mut(q);
                    for w in 0..wpr { x[w] ^= noise[w]; }
                    let z = self.z_table.row_words_mut(q);
                    for w in 0..wpr { z[w] ^= noise[w]; }
                }
            }
            "DEPOLARIZE1" => {
                let p = args.first().copied().unwrap_or(0.0);
                if p <= 0.0 { /* skip */ }
                else {
                    for q in qubits(targets)? {
                        let mut xf = vec![0u64; wpr];
                        let mut zf = vec![0u64; wpr];
                        for w in 0..wpr {
                            for bit in 0..64u32 {
                                if rng.r#gen::<f64>() < p {
                                    match rng.gen_range(0u8..3) {
                                        0 => xf[w] |= 1u64 << bit,
                                        1 => { xf[w] |= 1u64 << bit; zf[w] |= 1u64 << bit; }
                                        _ => zf[w] |= 1u64 << bit,
                                    }
                                }
                            }
                        }
                        let x = self.x_table.row_words_mut(q);
                        for w in 0..wpr { x[w] ^= xf[w]; }
                        let z = self.z_table.row_words_mut(q);
                        for w in 0..wpr { z[w] ^= zf[w]; }
                    }
                }
            }
            "DEPOLARIZE2" => {
                let p = args.first().copied().unwrap_or(0.0);
                if p > 0.0 {
                    for (qa, qb) in qubit_pairs(targets)? {
                        let mut xa = vec![0u64; wpr];
                        let mut za = vec![0u64; wpr];
                        let mut xb = vec![0u64; wpr];
                        let mut zb = vec![0u64; wpr];
                        for w in 0..wpr {
                            for bit in 0..64u32 {
                                if rng.r#gen::<f64>() < p {
                                    let r = rng.gen_range(0u8..15);
                                    let (pa, pb) = two_qubit_pauli(r);
                                    apply_pauli_bits(pa, &mut xa, &mut za, w, bit);
                                    apply_pauli_bits(pb, &mut xb, &mut zb, w, bit);
                                }
                            }
                        }
                        let x = self.x_table.row_words_mut(qa);
                        for w in 0..wpr { x[w] ^= xa[w]; }
                        let z = self.z_table.row_words_mut(qa);
                        for w in 0..wpr { z[w] ^= za[w]; }
                        let x = self.x_table.row_words_mut(qb);
                        for w in 0..wpr { x[w] ^= xb[w]; }
                        let z = self.z_table.row_words_mut(qb);
                        for w in 0..wpr { z[w] ^= zb[w]; }
                    }
                }
            }

            "CORRELATED_ERROR" | "E" => {
                let p = args.first().copied().unwrap_or(0.0);
                let noise = random_bits_with_prob(wpr, p, rng);
                apply_pauli_noise_to_targets(&mut self.x_table, &mut self.z_table, targets, &noise, wpr);
                self.last_correlated_error_occurred = noise;
            }
            "ELSE_CORRELATED_ERROR" => {
                let p = args.first().copied().unwrap_or(0.0);
                let candidate = random_bits_with_prob(wpr, p, rng);
                let mut noise = vec![0u64; wpr];
                for w in 0..wpr {
                    noise[w] = candidate[w] & !self.last_correlated_error_occurred[w];
                }
                apply_pauli_noise_to_targets(&mut self.x_table, &mut self.z_table, targets, &noise, wpr);
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
                    for w in 0..wpr { x[w] ^= xf[w]; }
                    let z = self.z_table.row_words_mut(q);
                    for w in 0..wpr { z[w] ^= zf[w]; }
                }
            }
            "PAULI_CHANNEL_2" => {
                let probs: Vec<f64> = (0..15).map(|i| args.get(i).copied().unwrap_or(0.0)).collect();
                let mut cum = [0.0f64; 15];
                cum[0] = probs[0];
                for i in 1..15 { cum[i] = cum[i - 1] + probs[i]; }
                let paulis: [(u8, u8); 15] = [
                    (0, 1), (0, 2), (0, 3),
                    (1, 0), (1, 1), (1, 2), (1, 3),
                    (2, 0), (2, 1), (2, 2), (2, 3),
                    (3, 0), (3, 1), (3, 2), (3, 3),
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
                    for w in 0..wpr { x[w] ^= xa[w]; }
                    let z = self.z_table.row_words_mut(qa);
                    for w in 0..wpr { z[w] ^= za[w]; }
                    let x = self.x_table.row_words_mut(qb);
                    for w in 0..wpr { x[w] ^= xb[w]; }
                    let z = self.z_table.row_words_mut(qb);
                    for w in 0..wpr { z[w] ^= zb[w]; }
                }
            }

            "HERALDED_ERASE" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits(targets)? {
                    let herald = random_bits_with_prob(wpr, p, rng);
                    self.m_record.push_row(&herald);
                    let mut xf = vec![0u64; wpr];
                    let mut zf = vec![0u64; wpr];
                    for w in 0..wpr {
                        let mut bits = herald[w];
                        while bits != 0 {
                            let bit = bits.trailing_zeros();
                            match rng.gen_range(0u8..4) {
                                1 => xf[w] |= 1u64 << bit,
                                2 => { xf[w] |= 1u64 << bit; zf[w] |= 1u64 << bit; }
                                3 => zf[w] |= 1u64 << bit,
                                _ => {}
                            }
                            bits &= bits - 1;
                        }
                    }
                    let x = self.x_table.row_words_mut(q);
                    for w in 0..wpr { x[w] ^= xf[w]; }
                    let z = self.z_table.row_words_mut(q);
                    for w in 0..wpr { z[w] ^= zf[w]; }
                }
            }
            "HERALDED_PAULI_CHANNEL_1" => {
                let pi = args.first().copied().unwrap_or(0.0);
                let px = args.get(1).copied().unwrap_or(0.0);
                let py = args.get(2).copied().unwrap_or(0.0);
                let pz = args.get(3).copied().unwrap_or(0.0);
                let total = pi + px + py + pz;
                for q in qubits(targets)? {
                    let herald = random_bits_with_prob(wpr, total, rng);
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
                    for w in 0..wpr { x[w] ^= xf[w]; }
                    let z = self.z_table.row_words_mut(q);
                    for w in 0..wpr { z[w] ^= zf[w]; }
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
                    for w in &mut result { *w ^= !0u64; }
                }
                self.det_records.push(result);
            }
            "OBSERVABLE_INCLUDE" => {
                let idx = args.first().copied().unwrap_or(0.0) as usize;
                let wpr = self.m_record.words_per_row();
                while self.obs_records.len() <= idx {
                    self.obs_records.push(vec![0u64; wpr]);
                }
                let mut ref_parity = false;
                for t in targets {
                    if let StimTarget::Rec(offset) = t {
                        let k = (-*offset) as usize;
                        self.m_record.xor_lookback_into(k, &mut self.obs_records[idx]);
                        let m_idx = self.m_record.len() - k;
                        if m_idx < ref_sample.len() && ref_sample[m_idx] {
                            ref_parity = !ref_parity;
                        }
                    }
                }
                if ref_parity {
                    for w in &mut self.obs_records[idx] { *w ^= !0u64; }
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
        let non_anchor: Vec<usize> = product.terms.iter()
            .map(|&(q, _)| q).filter(|&q| q != anchor).collect();
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
        let non_anchor: Vec<usize> = product.terms.iter()
            .map(|&(q, _)| q).filter(|&q| q != anchor).collect();
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
    targets.iter().map(|t| match t {
        StimTarget::Qubit(q) => Ok(*q as usize),
        _ => Err("expected qubit target".to_string()),
    }).collect()
}

fn qubits_ignoring_inv(targets: &[StimTarget]) -> Result<Vec<usize>, String> {
    targets.iter().map(|t| match t {
        StimTarget::Qubit(q) | StimTarget::QubitInv(q) => Ok(*q as usize),
        _ => Err("expected qubit target".to_string()),
    }).collect()
}

fn qubit_pairs(targets: &[StimTarget]) -> Result<Vec<(usize, usize)>, String> {
    if targets.len() % 2 != 0 {
        return Err("odd number of targets for pair gate".to_string());
    }
    let mut out = Vec::new();
    let mut it = targets.iter();
    while let (Some(a), Some(b)) = (it.next(), it.next()) {
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

fn random_bits_with_prob(words: usize, p: f64, rng: &mut impl Rng) -> Vec<u64> {
    let mut result = vec![0u64; words];
    if p <= 0.0 { return result; }
    if p >= 1.0 { result.fill(!0u64); return result; }
    for w in &mut result {
        for bit in 0..64u32 {
            if rng.r#gen::<f64>() < p {
                *w |= 1u64 << bit;
            }
        }
    }
    result
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
                    for w in 0..wpr { x[w] ^= noise[w]; }
                }
                PauliBasis::Y => {
                    let x = x_table.row_words_mut(q);
                    for w in 0..wpr { x[w] ^= noise[w]; }
                    let z = z_table.row_words_mut(q);
                    for w in 0..wpr { z[w] ^= noise[w]; }
                }
                PauliBasis::Z => {
                    let z = z_table.row_words_mut(q);
                    for w in 0..wpr { z[w] ^= noise[w]; }
                }
            }
        }
    }
}

fn two_qubit_pauli(r: u8) -> (u8, u8) {
    let mut idx = 0u8;
    for a in 0..4u8 {
        for b in 0..4u8 {
            if a == 0 && b == 0 { continue; }
            if idx == r { return (a, b); }
            idx += 1;
        }
    }
    (0, 0)
}

fn apply_pauli_bits(p: u8, xf: &mut [u64], zf: &mut [u64], w: usize, bit: u32) {
    match p {
        1 => xf[w] |= 1u64 << bit,
        2 => { xf[w] |= 1u64 << bit; zf[w] |= 1u64 << bit; }
        3 => zf[w] |= 1u64 << bit,
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
            _ => return Err("MPP/SPP targets must be Pauli targets".to_string()),
        }
    }
    if !current_terms.is_empty() {
        products.push(PauliProduct { terms: current_terms, inverted });
    }
    Ok(products)
}
