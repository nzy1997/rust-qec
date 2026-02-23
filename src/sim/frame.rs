use rand::Rng;

use crate::ir::{StimInstr, StimTarget};
use crate::sim::bit_table::BitTable;
use crate::sim::measure_record_batch::MeasureRecordBatch;

pub struct FrameSimulator {
    pub num_qubits: usize,
    pub batch_size: usize,
    pub x_table: BitTable,
    pub z_table: BitTable,
    pub m_record: MeasureRecordBatch,
}

impl FrameSimulator {
    pub fn new(num_qubits: usize, batch_size: usize) -> Self {
        Self {
            num_qubits,
            batch_size,
            x_table: BitTable::new(num_qubits, batch_size),
            z_table: BitTable::new(num_qubits, batch_size),
            m_record: MeasureRecordBatch::new(batch_size),
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
                StimInstr::Op { name, targets, .. } => {
                    self.exec_op(name.as_str(), targets, rng)?;
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
        targets: &[StimTarget],
        rng: &mut impl Rng,
    ) -> Result<(), String> {
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
                    let wpr = self.x_table.words_per_row();
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
                    let wpr = self.x_table.words_per_row();
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

            // Noise channels: no-op for now (Task 4)
            "X_ERROR" | "Y_ERROR" | "Z_ERROR"
            | "DEPOLARIZE1" | "DEPOLARIZE2"
            | "CORRELATED_ERROR" | "E" | "ELSE_CORRELATED_ERROR"
            | "PAULI_CHANNEL_1" | "PAULI_CHANNEL_2"
            | "I_ERROR" | "II_ERROR"
            | "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1" => {}

            // Metadata: no-op
            "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS"
            | "DETECTOR" | "OBSERVABLE_INCLUDE" => {}

            // MPAD: push zeros per target (frame has no error info for padding)
            "MPAD" => {
                for _t in targets {
                    self.m_record.push_zeros();
                }
            }

            _ => return Err(format!("frame_sim: unsupported instruction {}", name)),
        }
        Ok(())
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
