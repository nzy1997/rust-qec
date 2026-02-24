use crate::dem::{DemTarget, DetectorErrorModel};
use crate::ir::{PauliBasis, StimInstr, StimTarget};

#[derive(Debug, Clone, Default)]
struct SparseXorVec {
    targets: Vec<DemTarget>,
}

impl SparseXorVec {
    fn xor_item(&mut self, item: DemTarget) {
        match self.targets.binary_search(&item) {
            Ok(pos) => { self.targets.remove(pos); }
            Err(pos) => { self.targets.insert(pos, item); }
        }
    }

    fn xor_other(&mut self, other: &SparseXorVec) {
        for item in &other.targets {
            self.xor_item(item.clone());
        }
    }

    fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    fn clear(&mut self) {
        self.targets.clear();
    }
}

pub struct ErrorAnalyzer {
    x_sens: Vec<SparseXorVec>,
    z_sens: Vec<SparseXorVec>,
    measurement_sens: Vec<SparseXorVec>,
    num_measurements: usize,
    det_index: usize,
    errors: Vec<(f64, Vec<DemTarget>)>,
}

impl ErrorAnalyzer {
    pub fn circuit_to_dem(instrs: &[StimInstr]) -> Result<DetectorErrorModel, String> {
        let num_qubits = count_qubits(instrs);
        let num_measurements = count_measurements(instrs);
        let num_detectors = count_annotations(instrs, "DETECTOR");
        let num_observables = count_annotations(instrs, "OBSERVABLE_INCLUDE");

        let mut analyzer = ErrorAnalyzer {
            x_sens: vec![SparseXorVec::default(); num_qubits],
            z_sens: vec![SparseXorVec::default(); num_qubits],
            measurement_sens: vec![SparseXorVec::default(); num_measurements],
            num_measurements,
            det_index: num_detectors,
            errors: Vec::new(),
        };

        analyzer.undo_circuit(instrs)?;

        let mut dem = DetectorErrorModel::new();
        dem.set_min_counts(num_detectors, num_observables);
        for (prob, targets) in analyzer.errors.into_iter().rev() {
            if prob > 0.0 && !targets.is_empty() {
                dem.add_error(prob, targets);
            }
        }
        Ok(dem)
    }

    fn undo_circuit(&mut self, instrs: &[StimInstr]) -> Result<(), String> {
        for instr in instrs.iter().rev() {
            match instr {
                StimInstr::Op { name, args, targets, .. } => {
                    self.undo_op(name.as_str(), args, targets)?;
                }
                StimInstr::Repeat { count, body } => {
                    for _ in 0..*count {
                        self.undo_circuit(body)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn undo_op(
        &mut self,
        name: &str,
        args: &[f64],
        targets: &[StimTarget],
    ) -> Result<(), String> {
        match name {
            "I" | "X" | "Y" | "Z" => {}
            "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" => {}

            "H" => {
                for q in qubits(targets) {
                    self.x_sens.swap(q, q);
                    std::mem::swap(&mut self.x_sens[q], &mut self.z_sens[q]);
                }
            }
            "S" | "SQRT_Z" | "S_DAG" | "SQRT_Z_DAG" => {
                for q in qubits(targets) {
                    let z = self.z_sens[q].clone();
                    self.x_sens[q].xor_other(&z);
                }
            }
            "SQRT_X" | "SQRT_X_DAG" => {
                for q in qubits(targets) {
                    let x = self.x_sens[q].clone();
                    self.z_sens[q].xor_other(&x);
                }
            }
            "SQRT_Y" | "SQRT_Y_DAG" => {
                for q in qubits(targets) {
                    std::mem::swap(&mut self.x_sens[q], &mut self.z_sens[q]);
                }
            }
            "H_XY" => {
                for q in qubits(targets) {
                    let z = self.z_sens[q].clone();
                    self.x_sens[q].xor_other(&z);
                }
            }
            "H_YZ" => {
                for q in qubits(targets) {
                    let x = self.x_sens[q].clone();
                    self.z_sens[q].xor_other(&x);
                }
            }
            "C_XYZ" | "C_NXYZ" | "C_XNYZ" | "C_XYNZ" => {
                for q in qubits(targets) {
                    let old_x = self.x_sens[q].clone();
                    let old_z = self.z_sens[q].clone();
                    self.x_sens[q] = old_x.clone();
                    self.x_sens[q].xor_other(&old_z);
                    self.z_sens[q] = old_x;
                }
            }
            "C_ZYX" | "C_NZYX" | "C_ZNYX" | "C_ZYNX" => {
                for q in qubits(targets) {
                    let old_x = self.x_sens[q].clone();
                    let old_z = self.z_sens[q].clone();
                    self.x_sens[q] = old_z.clone();
                    self.z_sens[q] = old_x;
                    self.z_sens[q].xor_other(&old_z);
                }
            }
            "H_NXY" => {
                for q in qubits(targets) {
                    let z = self.z_sens[q].clone();
                    self.x_sens[q].xor_other(&z);
                }
            }
            "H_NXZ" => {
                for q in qubits(targets) {
                    std::mem::swap(&mut self.x_sens[q], &mut self.z_sens[q]);
                }
            }
            "H_NYZ" => {
                for q in qubits(targets) {
                    let x = self.x_sens[q].clone();
                    self.z_sens[q].xor_other(&x);
                }
            }

            "CX" | "CNOT" | "ZCX" => {
                for (c, t) in qubit_pairs(targets) {
                    self.undo_cx(c, t);
                }
            }
            "CY" | "ZCY" => {
                for (c, t) in qubit_pairs(targets) {
                    self.undo_s(t);
                    self.undo_cx(c, t);
                    self.undo_s(t);
                }
            }
            "CZ" | "ZCZ" => {
                for (a, b) in qubit_pairs(targets) {
                    self.undo_cz(a, b);
                }
            }
            "XCX" => {
                for (a, b) in qubit_pairs(targets) {
                    self.undo_h(a);
                    self.undo_cx(a, b);
                    self.undo_h(a);
                }
            }
            "XCY" => {
                for (a, b) in qubit_pairs(targets) {
                    self.undo_sqrt_x(b);
                    self.undo_cx(b, a);
                    self.undo_sqrt_x(b);
                }
            }
            "XCZ" => {
                for (a, b) in qubit_pairs(targets) {
                    self.undo_cx(b, a);
                }
            }
            "YCX" => {
                for (a, b) in qubit_pairs(targets) {
                    self.undo_sqrt_x(a);
                    self.undo_cx(a, b);
                    self.undo_sqrt_x(a);
                }
            }
            "YCY" => {
                for (a, b) in qubit_pairs(targets) {
                    self.undo_sqrt_x(a);
                    self.undo_sqrt_x(b);
                    self.undo_cz(a, b);
                    self.undo_sqrt_x(b);
                    self.undo_sqrt_x(a);
                }
            }
            "YCZ" => {
                for (a, b) in qubit_pairs(targets) {
                    self.undo_s(a);
                    self.undo_cx(b, a);
                    self.undo_s(a);
                }
            }
            "SWAP" => {
                for (a, b) in qubit_pairs(targets) {
                    self.x_sens.swap(a, b);
                    self.z_sens.swap(a, b);
                }
            }
            "ISWAP" | "ISWAP_DAG" => {
                for (a, b) in qubit_pairs(targets) {
                    self.undo_s(a);
                    self.undo_s(b);
                    self.undo_cz(a, b);
                    self.x_sens.swap(a, b);
                    self.z_sens.swap(a, b);
                }
            }
            "CXSWAP" => {
                for (a, b) in qubit_pairs(targets) {
                    self.undo_cx(b, a);
                    self.undo_cx(a, b);
                }
            }
            "SWAPCX" => {
                for (a, b) in qubit_pairs(targets) {
                    self.undo_cx(a, b);
                    self.undo_cx(b, a);
                }
            }
            "CZSWAP" => {
                for (a, b) in qubit_pairs(targets) {
                    self.undo_cz(a, b);
                    self.x_sens.swap(a, b);
                    self.z_sens.swap(a, b);
                }
            }

            "M" | "MZ" => {
                for q in qubits_inv(targets).into_iter().rev() {
                    self.undo_mz(q);
                }
            }
            "MX" => {
                for q in qubits_inv(targets).into_iter().rev() {
                    self.undo_mx(q);
                }
            }
            "MY" => {
                for q in qubits_inv(targets).into_iter().rev() {
                    self.undo_my(q);
                }
            }
            "MR" | "MRZ" => {
                for q in qubits_inv(targets).into_iter().rev() {
                    self.x_sens[q].clear();
                    self.z_sens[q].clear();
                    self.undo_mz(q);
                }
            }
            "MRX" => {
                for q in qubits_inv(targets).into_iter().rev() {
                    self.x_sens[q].clear();
                    self.z_sens[q].clear();
                    self.undo_mx(q);
                }
            }
            "MRY" => {
                for q in qubits_inv(targets).into_iter().rev() {
                    self.x_sens[q].clear();
                    self.z_sens[q].clear();
                    self.undo_my(q);
                }
            }

            "R" | "RZ" => {
                for q in qubits(targets) {
                    self.x_sens[q].clear();
                    self.z_sens[q].clear();
                }
            }
            "RX" => {
                for q in qubits(targets) {
                    self.x_sens[q].clear();
                    self.z_sens[q].clear();
                }
            }
            "RY" => {
                for q in qubits(targets) {
                    self.x_sens[q].clear();
                    self.z_sens[q].clear();
                }
            }

            "MPAD" => {
                for _t in targets.iter().rev() {
                    self.num_measurements -= 1;
                }
            }

            "DETECTOR" => {
                self.det_index -= 1;
                let det_target = DemTarget::Detector(self.det_index);
                for t in targets {
                    if let StimTarget::Rec(offset) = t {
                        let abs_idx = (self.num_measurements as i32 + offset) as usize;
                        self.measurement_sens[abs_idx].xor_item(det_target.clone());
                    }
                }
            }
            "OBSERVABLE_INCLUDE" => {
                let obs_idx = args.first().copied().unwrap_or(0.0) as usize;
                let obs_target = DemTarget::Observable(obs_idx);
                for t in targets {
                    if let StimTarget::Rec(offset) = t {
                        let abs_idx = (self.num_measurements as i32 + offset) as usize;
                        self.measurement_sens[abs_idx].xor_item(obs_target.clone());
                    }
                }
            }

            "X_ERROR" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits(targets) {
                    if !self.x_sens[q].is_empty() {
                        self.errors.push((p, self.x_sens[q].targets.clone()));
                    }
                }
            }
            "Z_ERROR" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits(targets) {
                    if !self.z_sens[q].is_empty() {
                        self.errors.push((p, self.z_sens[q].targets.clone()));
                    }
                }
            }
            "Y_ERROR" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits(targets) {
                    let mut y_sens = self.x_sens[q].clone();
                    y_sens.xor_other(&self.z_sens[q]);
                    if !y_sens.is_empty() {
                        self.errors.push((p, y_sens.targets));
                    }
                }
            }
            "DEPOLARIZE1" => {
                let p = args.first().copied().unwrap_or(0.0);
                if p > 0.0 {
                    let p3 = p / 3.0;
                    for q in qubits(targets) {
                        if !self.x_sens[q].is_empty() {
                            self.errors.push((p3, self.x_sens[q].targets.clone()));
                        }
                        let mut y_sens = self.x_sens[q].clone();
                        y_sens.xor_other(&self.z_sens[q]);
                        if !y_sens.is_empty() {
                            self.errors.push((p3, y_sens.targets));
                        }
                        if !self.z_sens[q].is_empty() {
                            self.errors.push((p3, self.z_sens[q].targets.clone()));
                        }
                    }
                }
            }
            "DEPOLARIZE2" => {
                let p = args.first().copied().unwrap_or(0.0);
                if p > 0.0 {
                    let p15 = p / 15.0;
                    for (qa, qb) in qubit_pairs(targets) {
                        let paulis: [(bool, bool, bool, bool); 15] = [
                            (false, false, true, false), // IX
                            (false, false, true, true),  // IY
                            (false, false, false, true), // IZ
                            (true, false, false, false), // XI
                            (true, false, true, false),  // XX
                            (true, false, true, true),   // XY
                            (true, false, false, true),  // XZ
                            (true, true, false, false),  // YI
                            (true, true, true, false),   // YX
                            (true, true, true, true),    // YY
                            (true, true, false, true),   // YZ
                            (false, true, false, false), // ZI
                            (false, true, true, false),  // ZX
                            (false, true, true, true),   // ZY
                            (false, true, false, true),  // ZZ
                        ];
                        for (xa, za, xb, zb) in &paulis {
                            let mut sens = SparseXorVec::default();
                            if *xa { sens.xor_other(&self.x_sens[qa]); }
                            if *za { sens.xor_other(&self.z_sens[qa]); }
                            if *xb { sens.xor_other(&self.x_sens[qb]); }
                            if *zb { sens.xor_other(&self.z_sens[qb]); }
                            if !sens.is_empty() {
                                self.errors.push((p15, sens.targets));
                            }
                        }
                    }
                }
            }
            "CORRELATED_ERROR" | "E" | "ELSE_CORRELATED_ERROR" => {
                let p = args.first().copied().unwrap_or(0.0);
                let mut sens = SparseXorVec::default();
                for t in targets {
                    if let StimTarget::Pauli { qubit, basis, .. } = t {
                        let q = *qubit as usize;
                        match basis {
                            PauliBasis::X => sens.xor_other(&self.x_sens[q]),
                            PauliBasis::Y => {
                                sens.xor_other(&self.x_sens[q]);
                                sens.xor_other(&self.z_sens[q]);
                            }
                            PauliBasis::Z => sens.xor_other(&self.z_sens[q]),
                        }
                    }
                }
                if !sens.is_empty() {
                    self.errors.push((p, sens.targets));
                }
            }
            "PAULI_CHANNEL_1" => {
                let px = args.first().copied().unwrap_or(0.0);
                let py = args.get(1).copied().unwrap_or(0.0);
                let pz = args.get(2).copied().unwrap_or(0.0);
                for q in qubits(targets) {
                    if px > 0.0 && !self.x_sens[q].is_empty() {
                        self.errors.push((px, self.x_sens[q].targets.clone()));
                    }
                    if py > 0.0 {
                        let mut y_sens = self.x_sens[q].clone();
                        y_sens.xor_other(&self.z_sens[q]);
                        if !y_sens.is_empty() {
                            self.errors.push((py, y_sens.targets));
                        }
                    }
                    if pz > 0.0 && !self.z_sens[q].is_empty() {
                        self.errors.push((pz, self.z_sens[q].targets.clone()));
                    }
                }
            }
            "PAULI_CHANNEL_2" => {
                let probs: Vec<f64> = (0..15).map(|i| args.get(i).copied().unwrap_or(0.0)).collect();
                let paulis: [(bool, bool, bool, bool); 15] = [
                    (false, false, true, false),
                    (false, false, true, true),
                    (false, false, false, true),
                    (true, false, false, false),
                    (true, false, true, false),
                    (true, false, true, true),
                    (true, false, false, true),
                    (true, true, false, false),
                    (true, true, true, false),
                    (true, true, true, true),
                    (true, true, false, true),
                    (false, true, false, false),
                    (false, true, true, false),
                    (false, true, true, true),
                    (false, true, false, true),
                ];
                for (qa, qb) in qubit_pairs(targets) {
                    for (i, (xa, za, xb, zb)) in paulis.iter().enumerate() {
                        if probs[i] > 0.0 {
                            let mut sens = SparseXorVec::default();
                            if *xa { sens.xor_other(&self.x_sens[qa]); }
                            if *za { sens.xor_other(&self.z_sens[qa]); }
                            if *xb { sens.xor_other(&self.x_sens[qb]); }
                            if *zb { sens.xor_other(&self.z_sens[qb]); }
                            if !sens.is_empty() {
                                self.errors.push((probs[i], sens.targets));
                            }
                        }
                    }
                }
            }
            "HERALDED_ERASE" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits(targets).into_iter().rev() {
                    self.num_measurements -= 1;
                    if p > 0.0 {
                        let p4 = p / 4.0;
                        if !self.x_sens[q].is_empty() {
                            self.errors.push((p4, self.x_sens[q].targets.clone()));
                        }
                        let mut y_sens = self.x_sens[q].clone();
                        y_sens.xor_other(&self.z_sens[q]);
                        if !y_sens.is_empty() {
                            self.errors.push((p4, y_sens.targets));
                        }
                        if !self.z_sens[q].is_empty() {
                            self.errors.push((p4, self.z_sens[q].targets.clone()));
                        }
                    }
                }
            }
            "HERALDED_PAULI_CHANNEL_1" => {
                let px = args.get(1).copied().unwrap_or(0.0);
                let py = args.get(2).copied().unwrap_or(0.0);
                let pz = args.get(3).copied().unwrap_or(0.0);
                for q in qubits(targets).into_iter().rev() {
                    self.num_measurements -= 1;
                    if px > 0.0 && !self.x_sens[q].is_empty() {
                        self.errors.push((px, self.x_sens[q].targets.clone()));
                    }
                    if py > 0.0 {
                        let mut y_sens = self.x_sens[q].clone();
                        y_sens.xor_other(&self.z_sens[q]);
                        if !y_sens.is_empty() {
                            self.errors.push((py, y_sens.targets));
                        }
                    }
                    if pz > 0.0 && !self.z_sens[q].is_empty() {
                        self.errors.push((pz, self.z_sens[q].targets.clone()));
                    }
                }
            }
            "I_ERROR" | "II_ERROR" => {}

            "MPP" => {
                self.undo_mpp(targets)?;
            }
            "SPP" | "SPP_DAG" => {
                self.undo_spp(targets)?;
            }
            "MXX" => {
                for (a, b) in qubit_pairs_inv(targets) {
                    self.undo_h(a);
                    self.undo_h(b);
                    self.undo_cx(a, b);
                    self.undo_mz(b);
                    self.undo_cx(a, b);
                    self.undo_h(b);
                    self.undo_h(a);
                }
            }
            "MYY" => {
                for (a, b) in qubit_pairs_inv(targets) {
                    self.undo_sqrt_x(a);
                    self.undo_sqrt_x(b);
                    self.undo_cx(a, b);
                    self.undo_mz(b);
                    self.undo_cx(a, b);
                    self.undo_sqrt_x(b);
                    self.undo_sqrt_x(a);
                }
            }
            "MZZ" => {
                for (a, b) in qubit_pairs_inv(targets) {
                    self.undo_cx(a, b);
                    self.undo_mz(b);
                    self.undo_cx(a, b);
                }
            }

            _ => return Err(format!("error_analyzer: unsupported instruction {}", name)),
        }
        Ok(())
    }

    fn undo_mz(&mut self, q: usize) {
        self.num_measurements -= 1;
        let m_idx = self.num_measurements;
        let m_sens = std::mem::take(&mut self.measurement_sens[m_idx]);
        self.x_sens[q].xor_other(&m_sens);
    }

    fn undo_mx(&mut self, q: usize) {
        self.num_measurements -= 1;
        let m_idx = self.num_measurements;
        let m_sens = std::mem::take(&mut self.measurement_sens[m_idx]);
        self.z_sens[q].xor_other(&m_sens);
    }

    fn undo_my(&mut self, q: usize) {
        self.num_measurements -= 1;
        let m_idx = self.num_measurements;
        let m_sens = std::mem::take(&mut self.measurement_sens[m_idx]);
        self.x_sens[q].xor_other(&m_sens);
        self.z_sens[q].xor_other(&m_sens);
    }

    fn undo_h(&mut self, q: usize) {
        std::mem::swap(&mut self.x_sens[q], &mut self.z_sens[q]);
    }

    fn undo_s(&mut self, q: usize) {
        let z = self.z_sens[q].clone();
        self.x_sens[q].xor_other(&z);
    }

    fn undo_sqrt_x(&mut self, q: usize) {
        let x = self.x_sens[q].clone();
        self.z_sens[q].xor_other(&x);
    }

    fn undo_cx(&mut self, c: usize, t: usize) {
        let xt = self.x_sens[t].clone();
        self.x_sens[c].xor_other(&xt);
        let zc = self.z_sens[c].clone();
        self.z_sens[t].xor_other(&zc);
    }

    fn undo_cz(&mut self, a: usize, b: usize) {
        let zb = self.z_sens[b].clone();
        let za = self.z_sens[a].clone();
        self.x_sens[a].xor_other(&zb);
        self.x_sens[b].xor_other(&za);
    }

    fn undo_mpp(&mut self, targets: &[StimTarget]) -> Result<(), String> {
        let products = split_pauli_products(targets);
        for product in products.into_iter().rev() {
            if product.terms.is_empty() {
                self.num_measurements -= 1;
                continue;
            }
            let anchor = product.terms.last().unwrap().0;
            let non_anchor: Vec<usize> = product.terms.iter()
                .map(|&(q, _)| q).filter(|&q| q != anchor).collect();

            for &(q, basis) in &product.terms {
                match basis {
                    PauliBasis::X => self.undo_h(q),
                    PauliBasis::Y => self.undo_sqrt_x(q),
                    PauliBasis::Z => {}
                }
            }

            for &q in &non_anchor {
                self.undo_cx(q, anchor);
            }

            self.undo_mz(anchor);

            for &q in non_anchor.iter().rev() {
                self.undo_cx(q, anchor);
            }

            for &(q, basis) in &product.terms {
                match basis {
                    PauliBasis::X => self.undo_h(q),
                    PauliBasis::Y => self.undo_sqrt_x(q),
                    PauliBasis::Z => {}
                }
            }
        }
        Ok(())
    }

    fn undo_spp(&mut self, targets: &[StimTarget]) -> Result<(), String> {
        let products = split_pauli_products(targets);
        for product in products.into_iter().rev() {
            if product.terms.is_empty() {
                continue;
            }
            let anchor = product.terms.last().unwrap().0;
            let non_anchor: Vec<usize> = product.terms.iter()
                .map(|&(q, _)| q).filter(|&q| q != anchor).collect();

            for &(q, basis) in &product.terms {
                match basis {
                    PauliBasis::X => self.undo_h(q),
                    PauliBasis::Y => self.undo_sqrt_x(q),
                    PauliBasis::Z => {}
                }
            }

            for &q in &non_anchor {
                self.undo_cx(q, anchor);
            }

            self.undo_s(anchor);

            for &q in non_anchor.iter().rev() {
                self.undo_cx(q, anchor);
            }

            for &(q, basis) in &product.terms {
                match basis {
                    PauliBasis::X => self.undo_h(q),
                    PauliBasis::Y => self.undo_sqrt_x(q),
                    PauliBasis::Z => {}
                }
            }
        }
        Ok(())
    }
}

struct PauliProduct {
    terms: Vec<(usize, PauliBasis)>,
}

fn split_pauli_products(targets: &[StimTarget]) -> Vec<PauliProduct> {
    let mut products = Vec::new();
    let mut current_terms: Vec<(usize, PauliBasis)> = Vec::new();
    let mut after_combiner = false;

    for target in targets {
        match target {
            StimTarget::Pauli { qubit, basis, .. } => {
                if !after_combiner && !current_terms.is_empty() {
                    products.push(PauliProduct { terms: std::mem::take(&mut current_terms) });
                }
                current_terms.push((*qubit as usize, *basis));
                after_combiner = false;
            }
            StimTarget::Combiner => {
                after_combiner = true;
            }
            _ => {}
        }
    }
    if !current_terms.is_empty() {
        products.push(PauliProduct { terms: current_terms });
    }
    products
}

fn qubits(targets: &[StimTarget]) -> Vec<usize> {
    targets.iter().filter_map(|t| match t {
        StimTarget::Qubit(q) => Some(*q as usize),
        _ => None,
    }).collect()
}

fn qubits_inv(targets: &[StimTarget]) -> Vec<usize> {
    targets.iter().filter_map(|t| match t {
        StimTarget::Qubit(q) | StimTarget::QubitInv(q) => Some(*q as usize),
        _ => None,
    }).collect()
}

fn qubit_pairs(targets: &[StimTarget]) -> Vec<(usize, usize)> {
    let qs = qubits(targets);
    qs.chunks(2).filter_map(|c| {
        if c.len() == 2 { Some((c[0], c[1])) } else { None }
    }).collect()
}

fn qubit_pairs_inv(targets: &[StimTarget]) -> Vec<(usize, usize)> {
    let qs = qubits_inv(targets);
    qs.chunks(2).filter_map(|c| {
        if c.len() == 2 { Some((c[0], c[1])) } else { None }
    }).collect()
}

fn count_qubits(instrs: &[StimInstr]) -> usize {
    let mut max_q: Option<u32> = None;
    for instr in instrs {
        match instr {
            StimInstr::Op { targets, .. } => {
                for t in targets {
                    let q = match t {
                        StimTarget::Qubit(q) | StimTarget::QubitInv(q) => Some(*q),
                        StimTarget::Pauli { qubit, .. } => Some(*qubit),
                        _ => None,
                    };
                    if let Some(q) = q {
                        max_q = Some(max_q.map_or(q, |m: u32| m.max(q)));
                    }
                }
            }
            StimInstr::Repeat { body, .. } => {
                let inner = count_qubits(body);
                if inner > 0 {
                    let inner_q = (inner - 1) as u32;
                    max_q = Some(max_q.map_or(inner_q, |m| m.max(inner_q)));
                }
            }
        }
    }
    max_q.map(|m| (m as usize) + 1).unwrap_or(0)
}

fn count_measurements(instrs: &[StimInstr]) -> usize {
    let mut count = 0;
    for instr in instrs {
        match instr {
            StimInstr::Op { name, targets, .. } => {
                match name.as_str() {
                    "M" | "MZ" | "MX" | "MY" | "MR" | "MRZ" | "MRX" | "MRY" => {
                        count += targets.iter().filter(|t| matches!(t,
                            StimTarget::Qubit(_) | StimTarget::QubitInv(_))).count();
                    }
                    "MPAD" => {
                        count += targets.len();
                    }
                    "MPP" => {
                        let products = split_pauli_products(targets);
                        count += products.len();
                    }
                    "MXX" | "MYY" | "MZZ" => {
                        let qs = qubits_inv(targets);
                        count += qs.len() / 2;
                    }
                    "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1" => {
                        count += targets.iter().filter(|t| matches!(t, StimTarget::Qubit(_))).count();
                    }
                    _ => {}
                }
            }
            StimInstr::Repeat { count: n, body } => {
                count += (*n as usize) * count_measurements(body);
            }
        }
    }
    count
}

fn count_annotations(instrs: &[StimInstr], kind: &str) -> usize {
    let mut count = 0;
    for instr in instrs {
        match instr {
            StimInstr::Op { name, .. } if name == kind => {
                count += 1;
            }
            StimInstr::Repeat { count: n, body } => {
                count += (*n as usize) * count_annotations(body, kind);
            }
            _ => {}
        }
    }
    count
}
