use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::dem::{DemInstruction, DemTarget, DetectorErrorModel};
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

    fn has_observable(&self) -> bool {
        self.targets.iter().any(|target| matches!(target, DemTarget::Observable(_)))
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
        analyzer.ensure_no_pending_gauge()?;

        // Merge errors that affect the same target set.
        // When multiple independent noise channels each flip the same set of
        // detectors/observables with probabilities p1, p2, …, the net probability
        // that an ODD number of them fire is:
        //   p_combined = p1 + p2 - 2*p1*p2
        let mut merged: BTreeMap<Vec<DemTarget>, f64> = BTreeMap::new();
        for (prob, targets) in analyzer.errors.into_iter().rev() {
            if prob > 0.0 && !targets.is_empty() {
                merged.entry(targets)
                    .and_modify(|existing| {
                        *existing = *existing + prob - 2.0 * *existing * prob;
                    })
                    .or_insert(prob);
            }
        }

        let mut dem = DetectorErrorModel::new();
        dem.set_min_counts(num_detectors, num_observables);
        for (targets, prob) in merged {
            if prob > 0.0 {
                dem.add_error(prob, targets);
            }
        }

        // Add detector coordinate annotations
        let annotations = collect_detector_annotations(instrs);
        for ann in annotations {
            dem.push(ann);
        }

        Ok(dem)
    }

    pub fn circuit_to_dem_decomposed(instrs: &[StimInstr]) -> Result<DetectorErrorModel, String> {
        let mut dem = Self::circuit_to_dem(instrs)?;
        decompose_errors(&mut dem)?;
        Ok(dem)
    }

    fn undo_circuit(&mut self, instrs: &[StimInstr]) -> Result<(), String> {
        let mut i = instrs.len();
        while i > 0 {
            i -= 1;
            match &instrs[i] {
                StimInstr::Op { name, .. } if name == "ELSE_CORRELATED_ERROR" => {
                    let end = i;
                    let mut start = i;
                    while start > 0 {
                        match &instrs[start - 1] {
                            StimInstr::Op { name, .. } if name == "ELSE_CORRELATED_ERROR" => {
                                start -= 1;
                            }
                            StimInstr::Op { name, .. }
                                if name == "CORRELATED_ERROR" || name == "E" =>
                            {
                                start -= 1;
                                self.undo_correlated_block(&instrs[start..=end])?;
                                i = start;
                                break;
                            }
                            _ => {
                                return Err(
                                    "ELSE_CORRELATED_ERROR without preceding E block".to_string()
                                );
                            }
                        }
                    }
                    if start == 0 {
                        match &instrs[0] {
                            StimInstr::Op { name, .. }
                                if name == "CORRELATED_ERROR" || name == "E" => {}
                            _ => {
                                return Err(
                                    "ELSE_CORRELATED_ERROR without preceding E block".to_string()
                                );
                            }
                        }
                    }
                }
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
                    self.ensure_z_collapse_is_deterministic(q)?;
                    self.undo_mz(q);
                }
            }
            "MX" => {
                for q in qubits_inv(targets).into_iter().rev() {
                    self.ensure_x_collapse_is_deterministic(q)?;
                    self.undo_mx(q);
                }
            }
            "MY" => {
                for q in qubits_inv(targets).into_iter().rev() {
                    self.ensure_y_collapse_is_deterministic(q)?;
                    self.undo_my(q);
                }
            }
            "MR" | "MRZ" => {
                for q in qubits_inv(targets).into_iter().rev() {
                    self.ensure_z_collapse_is_deterministic(q)?;
                    self.x_sens[q].clear();
                    self.z_sens[q].clear();
                    self.undo_mz(q);
                }
            }
            "MRX" => {
                for q in qubits_inv(targets).into_iter().rev() {
                    self.ensure_x_collapse_is_deterministic(q)?;
                    self.x_sens[q].clear();
                    self.z_sens[q].clear();
                    self.undo_mx(q);
                }
            }
            "MRY" => {
                for q in qubits_inv(targets).into_iter().rev() {
                    self.ensure_y_collapse_is_deterministic(q)?;
                    self.x_sens[q].clear();
                    self.z_sens[q].clear();
                    self.undo_my(q);
                }
            }

            "R" | "RZ" => {
                for q in qubits(targets) {
                    self.ensure_z_collapse_is_deterministic(q)?;
                    self.x_sens[q].clear();
                    self.z_sens[q].clear();
                }
            }
            "RX" => {
                for q in qubits(targets) {
                    self.ensure_x_collapse_is_deterministic(q)?;
                    self.x_sens[q].clear();
                    self.z_sens[q].clear();
                }
            }
            "RY" => {
                for q in qubits(targets) {
                    self.ensure_y_collapse_is_deterministic(q)?;
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
                        let abs_idx = checked_rec_index(self.num_measurements, *offset)?;
                        self.measurement_sens[abs_idx].xor_item(det_target.clone());
                    }
                }
            }
            "OBSERVABLE_INCLUDE" => {
                let obs_idx = args.first().copied().unwrap_or(0.0) as usize;
                let obs_target = DemTarget::Observable(obs_idx);
                for t in targets {
                    if let StimTarget::Rec(offset) = t {
                        let abs_idx = checked_rec_index(self.num_measurements, *offset)?;
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
                ensure_valid_depolarize1_probability(p)?;
                if p > 0.0 {
                    let q = depolarize1_to_independent(p);
                    for q_idx in qubits(targets) {
                        // X error (basis index 0)
                        if !self.x_sens[q_idx].is_empty() {
                            self.errors.push((q, self.x_sens[q_idx].targets.clone()));
                        }
                        // Z error (basis index 1)
                        if !self.z_sens[q_idx].is_empty() {
                            self.errors.push((q, self.z_sens[q_idx].targets.clone()));
                        }
                        // Y = X⊕Z error (basis index 0⊕1)
                        let mut y_sens = self.x_sens[q_idx].clone();
                        y_sens.xor_other(&self.z_sens[q_idx]);
                        if !y_sens.is_empty() {
                            self.errors.push((q, y_sens.targets));
                        }
                    }
                }
            }
            "DEPOLARIZE2" => {
                let p = args.first().copied().unwrap_or(0.0);
                ensure_valid_depolarize2_probability(p)?;
                if p > 0.0 {
                    let q = depolarize2_to_independent(p);
                    for (qa, qb) in qubit_pairs(targets) {
                        // 4 basis axes: x_a (bit 0), z_a (bit 1), x_b (bit 2), z_b (bit 3)
                        // Iterate over all 15 non-identity combinations (k = 1..15)
                        for k in 1u8..16 {
                            let mut sens = SparseXorVec::default();
                            if k & 1 != 0 { sens.xor_other(&self.x_sens[qa]); }
                            if k & 2 != 0 { sens.xor_other(&self.z_sens[qa]); }
                            if k & 4 != 0 { sens.xor_other(&self.x_sens[qb]); }
                            if k & 8 != 0 { sens.xor_other(&self.z_sens[qb]); }
                            if !sens.is_empty() {
                                self.errors.push((q, sens.targets));
                            }
                        }
                    }
                }
            }
            "CORRELATED_ERROR" | "E" => {
                self.undo_correlated_op(args, targets);
            }
            "ELSE_CORRELATED_ERROR" => {
                return Err("ELSE_CORRELATED_ERROR without preceding E block".to_string());
            }
            "PAULI_CHANNEL_1" => {
                let px = args.first().copied().unwrap_or(0.0);
                let py = args.get(1).copied().unwrap_or(0.0);
                let pz = args.get(2).copied().unwrap_or(0.0);
                // Try converting disjoint probabilities to independent ones
                let (ix, iy, iz, is_independent) =
                    if let Some((a, b, c)) = try_disjoint_to_independent_xyz(px, py, pz) {
                        (a, b, c, true)
                    } else {
                        (px, py, pz, false)
                    };
                for q in qubits(targets) {
                    if is_independent {
                        // Emit each basis combination independently
                        // combo 01 = X-sens: prob ix
                        if ix > 0.0 && !self.x_sens[q].is_empty() {
                            self.errors.push((ix, self.x_sens[q].targets.clone()));
                        }
                        // combo 10 = Z-sens: prob iz
                        if iz > 0.0 && !self.z_sens[q].is_empty() {
                            self.errors.push((iz, self.z_sens[q].targets.clone()));
                        }
                        // combo 11 = Y-sens = X⊕Z: prob iy
                        if iy > 0.0 {
                            let mut y_sens = self.x_sens[q].clone();
                            y_sens.xor_other(&self.z_sens[q]);
                            if !y_sens.is_empty() {
                                self.errors.push((iy, y_sens.targets));
                            }
                        }
                    } else {
                        // Fall back to disjoint approximation
                        let mut components = Vec::new();
                        if ix > 0.0 && !self.x_sens[q].is_empty() {
                            components.push((ix, self.x_sens[q].targets.clone()));
                        }
                        if iy > 0.0 {
                            let mut y_sens = self.x_sens[q].clone();
                            y_sens.xor_other(&self.z_sens[q]);
                            if !y_sens.is_empty() {
                                components.push((iy, y_sens.targets));
                            }
                        }
                        if iz > 0.0 && !self.z_sens[q].is_empty() {
                            components.push((iz, self.z_sens[q].targets.clone()));
                        }
                        self.emit_noise_channel(components);
                    }
                }
            }
            "PAULI_CHANNEL_2" => {
                if args.iter().any(|p| *p > 0.0) {
                    return Err(
                        "PAULI_CHANNEL_2 requires an approximation mode that rstim does not yet expose"
                            .to_string(),
                    );
                }
            }
            "HERALDED_ERASE" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits(targets).into_iter().rev() {
                    self.num_measurements -= 1;
                    if p > 0.0 {
                        let p4 = p / 4.0;
                        let mut components = Vec::new();
                        if !self.x_sens[q].is_empty() {
                            components.push((p4, self.x_sens[q].targets.clone()));
                        }
                        let mut y_sens = self.x_sens[q].clone();
                        y_sens.xor_other(&self.z_sens[q]);
                        if !y_sens.is_empty() {
                            components.push((p4, y_sens.targets));
                        }
                        if !self.z_sens[q].is_empty() {
                            components.push((p4, self.z_sens[q].targets.clone()));
                        }
                        self.emit_noise_channel(components);
                    }
                }
            }
            "HERALDED_PAULI_CHANNEL_1" => {
                let px = args.get(1).copied().unwrap_or(0.0);
                let py = args.get(2).copied().unwrap_or(0.0);
                let pz = args.get(3).copied().unwrap_or(0.0);
                for q in qubits(targets).into_iter().rev() {
                    self.num_measurements -= 1;
                    let mut components = Vec::new();
                    if px > 0.0 && !self.x_sens[q].is_empty() {
                        components.push((px, self.x_sens[q].targets.clone()));
                    }
                    if py > 0.0 {
                        let mut y_sens = self.x_sens[q].clone();
                        y_sens.xor_other(&self.z_sens[q]);
                        if !y_sens.is_empty() {
                            components.push((py, y_sens.targets));
                        }
                    }
                    if pz > 0.0 && !self.z_sens[q].is_empty() {
                        components.push((pz, self.z_sens[q].targets.clone()));
                    }
                    self.emit_noise_channel(components);
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

    /// Emit errors from a single noise channel (e.g. one DEPOLARIZE1 on one qubit).
    /// Within a channel the Pauli components are mutually exclusive, so errors
    /// with the same target set are combined by addition (not XOR).
    fn emit_noise_channel(&mut self, components: Vec<(f64, Vec<DemTarget>)>) {
        let mut grouped: BTreeMap<Vec<DemTarget>, f64> = BTreeMap::new();
        for (prob, targets) in components {
            if prob > 0.0 && !targets.is_empty() {
                *grouped.entry(targets).or_default() += prob;
            }
        }
        for (targets, prob) in grouped {
            self.errors.push((prob, targets));
        }
    }

    fn correlated_targets(&self, targets: &[StimTarget]) -> Vec<DemTarget> {
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
        sens.targets
    }

    fn undo_correlated_op(&mut self, args: &[f64], targets: &[StimTarget]) {
        let p = args.first().copied().unwrap_or(0.0);
        let targets = self.correlated_targets(targets);
        if p > 0.0 && !targets.is_empty() {
            self.errors.push((p, targets));
        }
    }

    fn undo_correlated_block(&mut self, block: &[StimInstr]) -> Result<(), String> {
        if block.len() > 2 {
            return Err(
                "correlated error block requires approximation mode for >2 branches".to_string(),
            );
        }

        let mut grouped: BTreeMap<Vec<DemTarget>, f64> = BTreeMap::new();
        let mut remaining = 1.0;
        for instr in block {
            let (probability, targets) = self.correlated_branch_from_instr(instr)?;
            let effective_probability = probability * remaining;
            remaining *= 1.0 - probability;
            if effective_probability > 0.0 && !targets.is_empty() {
                *grouped.entry(targets).or_default() += effective_probability;
            }
        }
        for (targets, probability) in grouped {
            self.errors.push((probability, targets));
        }
        Ok(())
    }

    fn correlated_branch_from_instr(
        &self,
        instr: &StimInstr,
    ) -> Result<(f64, Vec<DemTarget>), String> {
        match instr {
            StimInstr::Op { name, args, targets, .. }
                if name == "CORRELATED_ERROR"
                    || name == "E"
                    || name == "ELSE_CORRELATED_ERROR" =>
            {
                Ok((
                    args.first().copied().unwrap_or(0.0),
                    self.correlated_targets(targets),
                ))
            }
            _ => Err("invalid correlated error block".to_string()),
        }
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

    fn ensure_x_collapse_is_deterministic(&self, q: usize) -> Result<(), String> {
        if !self.x_sens[q].is_empty() {
            return Err(format!(
                "non-deterministic {} encountered",
                self.sensitivity_kind_for_qubit(q),
            ));
        }
        Ok(())
    }

    fn ensure_y_collapse_is_deterministic(&self, q: usize) -> Result<(), String> {
        if self.x_sens[q].targets != self.z_sens[q].targets {
            return Err(format!(
                "non-deterministic {} encountered",
                self.sensitivity_kind_for_qubit(q),
            ));
        }
        Ok(())
    }

    fn ensure_z_collapse_is_deterministic(&self, q: usize) -> Result<(), String> {
        if !self.z_sens[q].is_empty() {
            return Err(format!(
                "non-deterministic {} encountered",
                self.sensitivity_kind_for_qubit(q),
            ));
        }
        Ok(())
    }

    fn ensure_no_pending_gauge(&self) -> Result<(), String> {
        for q in 0..self.x_sens.len() {
            if !self.z_sens[q].is_empty() {
                return Err(format!(
                    "non-deterministic {} encountered",
                    self.sensitivity_kind_for_qubit(q),
                ));
            }
        }
        if self.measurement_sens.iter().any(|sens| !sens.is_empty()) {
            let kind = if self.measurement_sens.iter().any(SparseXorVec::has_observable) {
                "observable"
            } else {
                "detector"
            };
            return Err(format!("non-deterministic {kind} encountered"));
        }
        Ok(())
    }

    fn sensitivity_kind_for_qubit(&self, q: usize) -> &'static str {
        if self.x_sens[q].has_observable() || self.z_sens[q].has_observable() {
            "observable"
        } else {
            "detector"
        }
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

fn checked_rec_index(num_measurements: usize, offset: i32) -> Result<usize, String> {
    let idx = num_measurements as i32 + offset;
    if idx < 0 || idx >= num_measurements as i32 {
        return Err(format!(
            "invalid rec[{offset}] reference with {num_measurements} measurements available"
        ));
    }
    Ok(idx as usize)
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

/// Convert DEPOLARIZE1(p) probability to independent per-channel probability.
///
/// DEPOLARIZE1(p) applies X, Y, or Z each with disjoint probability p/3.
/// This converts to independent X-flip and Z-flip probabilities such that
/// the composition of independent X and Z channels reproduces the same
/// disjoint distribution over {I, X, Y, Z}.
fn depolarize1_to_independent(p: f64) -> f64 {
    0.5 - 0.5 * (1.0 - (4.0 * p) / 3.0).sqrt()
}

fn ensure_valid_depolarize1_probability(p: f64) -> Result<(), String> {
    if p > 0.75 {
        return Err(format!("DEPOLARIZE1({p}) exceeds exact-analysis limit of 3/4"));
    }
    Ok(())
}

/// Convert DEPOLARIZE2(p) probability to independent per-channel probability.
///
/// DEPOLARIZE2(p) applies each of 15 non-identity two-qubit Paulis with
/// disjoint probability p/15. This converts to the independent probability
/// for each of the 4 basis axes (Xa, Za, Xb, Zb) such that the composition
/// of 4 independent channels reproduces the same disjoint distribution.
fn depolarize2_to_independent(p: f64) -> f64 {
    0.5 - 0.5 * (1.0 - (16.0 * p) / 15.0).powf(0.125)
}

fn ensure_valid_depolarize2_probability(p: f64) -> Result<(), String> {
    if p > 15.0 / 16.0 {
        return Err(format!("DEPOLARIZE2({p}) exceeds exact-analysis limit of 15/16"));
    }
    Ok(())
}

/// Convert disjoint (mutually exclusive) X/Y/Z probabilities to independent
/// per-channel probabilities. Returns None if no exact solution exists.
fn try_disjoint_to_independent_xyz(
    x: f64,
    y: f64,
    z: f64,
) -> Option<(f64, f64, f64)> {
    let i = (1.0 - x - y - z).max(0.0);
    // Re-arrange so identity is most likely
    if i < x {
        let result = try_disjoint_to_independent_xyz(i, z, y);
        return result.map(|(a, b, c)| (1.0 - a, b, c));
    }
    if i < y {
        let result = try_disjoint_to_independent_xyz(z, i, x);
        return result.map(|(a, b, c)| (a, 1.0 - b, c));
    }
    if i < z {
        let result = try_disjoint_to_independent_xyz(y, x, i);
        return result.map(|(a, b, c)| (a, b, 1.0 - c));
    }
    if x + z >= 0.5 || x + y >= 0.5 || y + z >= 0.5 {
        return None;
    }
    let s_xz = (1.0 - 2.0 * x - 2.0 * z).sqrt();
    let s_xy = (1.0 - 2.0 * x - 2.0 * y).sqrt();
    let s_yz = (1.0 - 2.0 * y - 2.0 * z).sqrt();
    let a = 0.5 - 0.5 * s_xz * s_xy / s_yz;
    let b = 0.5 - 0.5 * s_xy * s_yz / s_xz;
    let c = 0.5 - 0.5 * s_xz * s_yz / s_xy;
    if a >= 0.0 && b >= 0.0 && c >= 0.0 {
        Some((a, b, c))
    } else {
        None
    }
}

/// Collect detector coordinate annotations from a circuit.
/// Walks the circuit forward to build detector and shift_detectors instructions.
fn collect_detector_annotations(instrs: &[StimInstr]) -> Vec<DemInstruction> {
    let mut result = Vec::new();
    let mut det_index: usize = 0;
    collect_detector_annotations_inner(instrs, &mut result, &mut det_index);
    result
}

fn collect_detector_annotations_inner(
    instrs: &[StimInstr],
    result: &mut Vec<DemInstruction>,
    det_index: &mut usize,
) {
    for instr in instrs {
        match instr {
            StimInstr::Op { name, args, .. } => match name.as_str() {
                "DETECTOR" => {
                    if !args.is_empty() {
                        result.push(DemInstruction::Detector {
                            index: *det_index,
                            coords: args.clone(),
                        });
                    }
                    *det_index += 1;
                }
                "SHIFT_COORDS" => {
                    result.push(DemInstruction::ShiftDetectors {
                        detector_offset: 0,
                        coord_offsets: args.clone(),
                    });
                }
                _ => {}
            },
            StimInstr::Repeat { count, body } => {
                for _ in 0..*count {
                    collect_detector_annotations_inner(body, result, det_index);
                }
            }
        }
    }
}

/// Extract the detector/observable target set from a list of DemTargets,
/// ignoring separators. Returns a BTreeSet for XOR-style operations.
fn target_set(targets: &[DemTarget]) -> BTreeSet<DemTarget> {
    targets
        .iter()
        .filter(|t| !matches!(t, DemTarget::Separator))
        .cloned()
        .collect()
}

/// Symmetric difference of two BTreeSets.
fn symmetric_difference(
    a: &BTreeSet<DemTarget>,
    b: &BTreeSet<DemTarget>,
) -> BTreeSet<DemTarget> {
    a.symmetric_difference(b).cloned().collect()
}

/// Count detector targets in a target set.
fn detector_count(targets: &BTreeSet<DemTarget>) -> usize {
    targets
        .iter()
        .filter(|t| matches!(t, DemTarget::Detector(_)))
        .count()
}

/// Check if a single component (no separators) is graphlike (at most 2 detectors).
fn component_is_graphlike(targets: &[DemTarget]) -> bool {
    // Split by separators and check each component
    let mut det_count = 0usize;
    for t in targets {
        match t {
            DemTarget::Separator => {
                if det_count > 2 {
                    return false;
                }
                det_count = 0;
            }
            DemTarget::Detector(_) => {
                det_count += 1;
            }
            _ => {}
        }
    }
    det_count <= 2
}

/// Convert a BTreeSet of targets into a sorted Vec.
fn set_to_targets(set: &BTreeSet<DemTarget>) -> Vec<DemTarget> {
    set.iter().cloned().collect()
}

/// Decompose non-graphlike errors in a DEM into graphlike components.
///
/// A graphlike error has at most 2 detector targets per ^-separated component.
/// Non-graphlike errors (3+ detectors) are decomposed by finding combinations
/// of existing graphlike errors whose detector sets XOR to produce the
/// non-graphlike error's detector set.
pub fn decompose_errors(dem: &mut DetectorErrorModel) -> Result<(), String> {
    let instrs = dem.instructions().to_vec();

    // Build a lookup of known graphlike errors: target_set -> index
    let mut graphlike_map: BTreeMap<BTreeSet<DemTarget>, usize> = BTreeMap::new();
    let mut graphlike_sets: Vec<(usize, BTreeSet<DemTarget>)> = Vec::new();

    // Classify errors
    let mut non_graphlike_indices: Vec<usize> = Vec::new();

    for (i, instr) in instrs.iter().enumerate() {
        if let DemInstruction::Error { targets, .. } = instr {
            if component_is_graphlike(targets) {
                let tset = target_set(targets);
                if !tset.is_empty() {
                    graphlike_map.insert(tset.clone(), i);
                    graphlike_sets.push((i, tset));
                }
            } else {
                non_graphlike_indices.push(i);
            }
        }
    }

    if non_graphlike_indices.is_empty() {
        return Ok(());
    }

    // Try to decompose each non-graphlike error
    let mut new_instrs = instrs;

    for &idx in &non_graphlike_indices {
        let (prob, targets) = match &new_instrs[idx] {
            DemInstruction::Error { probability, targets } => (*probability, targets.clone()),
            _ => unreachable!(),
        };

        let error_set = target_set(&targets);
        let mut decomposed = false;

        // Strategy 1: find K1 such that K1 is graphlike, and S ^ K1 is also a known graphlike error
        for (_, k1_set) in &graphlike_sets {
            // K1 must be a subset of or overlap with error_set for the decomposition to reduce complexity
            let remainder = symmetric_difference(&error_set, k1_set);
            if detector_count(&remainder) <= 2 {
                if let Some(_) = graphlike_map.get(&remainder) {
                    // Decompose: error_set = k1_set ^ remainder
                    let mut new_targets = set_to_targets(k1_set);
                    new_targets.push(DemTarget::Separator);
                    new_targets.extend(set_to_targets(&remainder));
                    new_instrs[idx] = DemInstruction::Error {
                        probability: prob,
                        targets: new_targets,
                    };
                    decomposed = true;
                    break;
                }
                // Even if remainder is not a known error, it's graphlike (<=2 detectors),
                // so we can use it directly as a new component
                let mut new_targets = set_to_targets(k1_set);
                new_targets.push(DemTarget::Separator);
                new_targets.extend(set_to_targets(&remainder));
                new_instrs[idx] = DemInstruction::Error {
                    probability: prob,
                    targets: new_targets,
                };
                decomposed = true;
                break;
            }
        }

        if decomposed {
            continue;
        }

        // Strategy 2: find pairs (K1, K2) of known graphlike errors where K1 ^ K2 = S
        let mut found_pair = false;
        'outer: for (i, (_, k1_set)) in graphlike_sets.iter().enumerate() {
            for (_, k2_set) in graphlike_sets.iter().skip(i) {
                let combined = symmetric_difference(k1_set, k2_set);
                if combined == error_set {
                    let mut new_targets = set_to_targets(k1_set);
                    new_targets.push(DemTarget::Separator);
                    new_targets.extend(set_to_targets(k2_set));
                    new_instrs[idx] = DemInstruction::Error {
                        probability: prob,
                        targets: new_targets,
                    };
                    found_pair = true;
                    break 'outer;
                }
            }
        }

        if !found_pair {
            // Strategy 3: brute-force decompose into components of <=2 detectors each
            // Extract just detectors and observables separately
            let dets: Vec<DemTarget> = error_set
                .iter()
                .filter(|t| matches!(t, DemTarget::Detector(_)))
                .cloned()
                .collect();
            let obs: Vec<DemTarget> = error_set
                .iter()
                .filter(|t| matches!(t, DemTarget::Observable(_)))
                .cloned()
                .collect();

            if dets.len() >= 3 {
                // Split into pairs of detectors, distributing observables into the first component
                let mut new_targets: Vec<DemTarget> = Vec::new();
                let mut first = true;
                for chunk in dets.chunks(2) {
                    if !first {
                        new_targets.push(DemTarget::Separator);
                    }
                    new_targets.extend_from_slice(chunk);
                    if first {
                        // Put observables in the first component
                        new_targets.extend(obs.iter().cloned());
                        first = false;
                    }
                }
                new_instrs[idx] = DemInstruction::Error {
                    probability: prob,
                    targets: new_targets,
                };
            } else {
                return Err(format!(
                    "failed to decompose non-graphlike error: {:?}",
                    targets
                ));
            }
        }
    }

    // Rebuild the DEM
    let mut new_dem = DetectorErrorModel::new();
    new_dem.set_min_counts(dem.num_detectors(), dem.num_observables());
    for instr in new_instrs {
        new_dem.push(instr);
    }
    *dem = new_dem;
    Ok(())
}
