use std::collections::BTreeMap;

#[cfg(test)]
use std::collections::BTreeSet;

use crate::dem::{DemInstruction, DemTarget, DetectorErrorModel};
use crate::ir::{PauliBasis, StimInstr, StimTarget};

#[derive(Debug, Clone, Copy, Default)]
pub struct AnalyzeOptions {
    pub approximate_disjoint_errors: bool,
    pub allow_gauge_detectors: bool,
}

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

    fn xor_targets(&mut self, targets: &[DemTarget]) {
        for item in targets {
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

fn canonicalize_error_targets(targets: &[DemTarget]) -> Vec<DemTarget> {
    let mut components: Vec<Vec<DemTarget>> = Vec::new();
    let mut current = Vec::new();
    for target in targets {
        if matches!(target, DemTarget::Separator) {
            if !current.is_empty() {
                current.sort();
                components.push(current);
                current = Vec::new();
            }
        } else {
            current.push(target.clone());
        }
    }
    if !current.is_empty() {
        current.sort();
        components.push(current);
    }
    components.sort();

    let mut out = Vec::new();
    for (k, component) in components.into_iter().enumerate() {
        if k > 0 {
            out.push(DemTarget::Separator);
        }
        out.extend(component);
    }
    out
}

pub struct ErrorAnalyzer {
    x_sens: Vec<SparseXorVec>,
    z_sens: Vec<SparseXorVec>,
    measurement_sens: Vec<SparseXorVec>,
    num_measurements: usize,
    det_index: usize,
    errors: Vec<(f64, Vec<DemTarget>)>,
    decompose_channel_errors: bool,
    // Phase 2 options are threaded first and consumed by later tasks.
    #[allow(dead_code)]
    options: AnalyzeOptions,
}

impl ErrorAnalyzer {
    pub fn circuit_to_dem(instrs: &[StimInstr]) -> Result<DetectorErrorModel, String> {
        Self::circuit_to_dem_inner(instrs, AnalyzeOptions::default(), false)
    }

    pub fn circuit_to_dem_with_options(
        instrs: &[StimInstr],
        options: AnalyzeOptions,
    ) -> Result<DetectorErrorModel, String> {
        Self::circuit_to_dem_inner(instrs, options, false)
    }

    fn circuit_to_dem_inner(
        instrs: &[StimInstr],
        options: AnalyzeOptions,
        decompose_channel_errors: bool,
    ) -> Result<DetectorErrorModel, String> {
        let flattened_instrs;
        let instrs = if instrs.iter().any(|instr| matches!(instr, StimInstr::Repeat { .. })) {
            flattened_instrs = crate::transforms::flattened(instrs);
            &flattened_instrs[..]
        } else {
            instrs
        };

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
            decompose_channel_errors,
            options,
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
                let targets = canonicalize_error_targets(&targets);
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
        let mut dem = Self::circuit_to_dem_inner(instrs, AnalyzeOptions::default(), true)?;
        decompose_errors(&mut dem)?;
        Ok(dem)
    }

    pub fn circuit_to_dem_with_options_decomposed(
        instrs: &[StimInstr],
        options: AnalyzeOptions,
    ) -> Result<DetectorErrorModel, String> {
        let mut dem = Self::circuit_to_dem_inner(instrs, options, true)?;
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
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits_inv(targets).into_iter().rev() {
                    self.ensure_z_collapse_is_deterministic(q)?;
                    self.emit_measurement_noise(p);
                    self.undo_mz(q);
                }
            }
            "MX" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits_inv(targets).into_iter().rev() {
                    self.ensure_x_collapse_is_deterministic(q)?;
                    self.emit_measurement_noise(p);
                    self.undo_mx(q);
                }
            }
            "MY" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits_inv(targets).into_iter().rev() {
                    self.ensure_y_collapse_is_deterministic(q)?;
                    self.emit_measurement_noise(p);
                    self.undo_my(q);
                }
            }
            "MR" | "MRZ" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits_inv(targets).into_iter().rev() {
                    self.ensure_z_collapse_is_deterministic(q)?;
                    self.emit_measurement_noise(p);
                    self.x_sens[q].clear();
                    self.z_sens[q].clear();
                    self.undo_mz(q);
                }
            }
            "MRX" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits_inv(targets).into_iter().rev() {
                    self.ensure_x_collapse_is_deterministic(q)?;
                    self.emit_measurement_noise(p);
                    self.x_sens[q].clear();
                    self.z_sens[q].clear();
                    self.undo_mx(q);
                }
            }
            "MRY" => {
                let p = args.first().copied().unwrap_or(0.0);
                for q in qubits_inv(targets).into_iter().rev() {
                    self.ensure_y_collapse_is_deterministic(q)?;
                    self.emit_measurement_noise(p);
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
                    if self.num_measurements == 0 {
                        return Err("MPAD underflow in error_analyzer".to_string());
                    }
                    self.measurement_sens[self.num_measurements - 1].clear();
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
                        self.emit_error_combinations(
                            &[0.0, q, q, q],
                            &[
                                self.x_sens[q_idx].targets.clone(),
                                self.z_sens[q_idx].targets.clone(),
                            ],
                            false,
                        );
                    }
                }
            }
            "DEPOLARIZE2" => {
                let p = args.first().copied().unwrap_or(0.0);
                ensure_valid_depolarize2_probability(p)?;
                if p > 0.0 {
                    let q = depolarize2_to_independent(p);
                    for (qa, qb) in qubit_pairs(targets) {
                        self.emit_error_combinations(
                            &[0.0, q, q, q, q, q, q, q, q, q, q, q, q, q, q, q],
                            &[
                                self.x_sens[qa].targets.clone(),
                                self.z_sens[qa].targets.clone(),
                                self.x_sens[qb].targets.clone(),
                                self.z_sens[qb].targets.clone(),
                            ],
                            false,
                        );
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
                if args.iter().any(|p| *p > 0.0) && !self.options.approximate_disjoint_errors {
                    return Err(
                        "PAULI_CHANNEL_2 requires an approximation mode that rstim does not yet expose"
                            .to_string(),
                    );
                }
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
                    let mut components = Vec::new();
                    for (i, (xa, za, xb, zb)) in paulis.iter().enumerate() {
                        if probs[i] > 0.0 {
                            let mut sens = SparseXorVec::default();
                            if *xa { sens.xor_other(&self.x_sens[qa]); }
                            if *za { sens.xor_other(&self.z_sens[qa]); }
                            if *xb { sens.xor_other(&self.x_sens[qb]); }
                            if *zb { sens.xor_other(&self.z_sens[qb]); }
                            if !sens.is_empty() {
                                components.push((probs[i], sens.targets));
                            }
                        }
                    }
                    self.emit_noise_channel(components);
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

    fn xor_sorted_targets(a: &[DemTarget], b: &[DemTarget]) -> Vec<DemTarget> {
        let mut i = 0;
        let mut j = 0;
        let mut out = Vec::with_capacity(a.len() + b.len());
        while i < a.len() || j < b.len() {
            if i == a.len() {
                out.push(b[j].clone());
                j += 1;
            } else if j == b.len() {
                out.push(a[i].clone());
                i += 1;
            } else {
                match a[i].cmp(&b[j]) {
                    std::cmp::Ordering::Less => {
                        out.push(a[i].clone());
                        i += 1;
                    }
                    std::cmp::Ordering::Greater => {
                        out.push(b[j].clone());
                        j += 1;
                    }
                    std::cmp::Ordering::Equal => {
                        i += 1;
                        j += 1;
                    }
                }
            }
        }
        out
    }

    fn join_components_with_separators(components: Vec<Vec<DemTarget>>) -> Vec<DemTarget> {
        let mut out = Vec::new();
        for (k, component) in components.into_iter().enumerate() {
            if component.is_empty() {
                continue;
            }
            if !out.is_empty() && k > 0 {
                out.push(DemTarget::Separator);
            }
            out.extend(component);
        }
        out
    }

    fn decompose_combination_targets(stored_ids: &mut [Vec<DemTarget>], detector_masks: &[u64]) {
        let mut detector_counts = vec![0u8; detector_masks.len()];
        for k in 1..detector_masks.len() {
            detector_counts[k] = detector_masks[k].count_ones() as u8;
        }

        let mut solved = 0u64;
        let mut single_detectors_union = 0u64;
        for k in 1..detector_masks.len() {
            if detector_counts[k] == 1 {
                single_detectors_union |= detector_masks[k];
                solved |= 1 << k;
            }
        }

        let mut irreducible_pairs = Vec::new();
        for k in 1..detector_masks.len() {
            if detector_counts[k] == 2 && (detector_masks[k] & !single_detectors_union) != 0 {
                irreducible_pairs.push(k);
                solved |= 1 << k;
            }
        }

        for goal_k in 1..detector_masks.len() {
            if detector_counts[goal_k] == 0 || ((solved >> goal_k) & 1) != 0 {
                continue;
            }

            let goal = detector_masks[goal_k];
            let mut components = Vec::new();
            let mut remnants = if (goal & !single_detectors_union) == 0 {
                goal
            } else {
                let mut solved_with_pairs = None;

                for &pair_k in &irreducible_pairs {
                    let mask = detector_masks[pair_k];
                    if (goal & mask) == mask && (goal & !(single_detectors_union | mask)) == 0 {
                        components.push(stored_ids[pair_k].clone());
                        solved_with_pairs = Some(goal & !mask);
                        break;
                    }
                }

                if solved_with_pairs.is_none() {
                    'search_two_pairs: for i in 0..irreducible_pairs.len() {
                        let k1 = irreducible_pairs[i];
                        let m1 = detector_masks[k1];
                        for &candidate_k2 in irreducible_pairs.iter().skip(i + 1) {
                            let k2 = candidate_k2;
                            let m2 = detector_masks[k2];
                            if (m1 & m2) == 0 && (goal & !(single_detectors_union | m1 | m2)) == 0 {
                                let mut first = k1;
                                let mut second = k2;
                                if stored_ids[second] < stored_ids[first] {
                                    std::mem::swap(&mut first, &mut second);
                                }
                                components.push(stored_ids[first].clone());
                                components.push(stored_ids[second].clone());
                                solved_with_pairs = Some(goal & !(m1 | m2));
                                break 'search_two_pairs;
                            }
                        }
                    }
                }

                match solved_with_pairs {
                    Some(remnants) => remnants,
                    None => {
                        components.push(stored_ids[goal_k].clone());
                        0
                    }
                }
            };

            for k2 in 0..detector_masks.len() {
                if remnants == 0 {
                    break;
                }
                if detector_counts[k2] == 1 && (detector_masks[k2] & !remnants) == 0 {
                    remnants &= !detector_masks[k2];
                    components.push(stored_ids[k2].clone());
                }
            }

            stored_ids[goal_k] = Self::join_components_with_separators(components);
        }
    }

    fn emit_error_combinations(
        &mut self,
        probabilities: &[f64],
        basis_errors: &[Vec<DemTarget>],
        probabilities_are_disjoint: bool,
    ) {
        let s = basis_errors.len();
        let num_cases = 1usize << s;
        debug_assert_eq!(probabilities.len(), num_cases);

        let mut stored_ids = vec![Vec::<DemTarget>::new(); num_cases];
        let mut detector_masks = vec![0u64; num_cases];

        for k in 0..s {
            let slot = 1usize << k;
            stored_ids[slot] = basis_errors[k].clone();
        }

        let mut involved_detectors = Vec::<usize>::new();
        if self.decompose_channel_errors {
            for k in 0..s {
                let slot = 1usize << k;
                for target in &basis_errors[k] {
                    if let DemTarget::Detector(det) = target {
                        let bit = if let Some(existing) = involved_detectors.iter().position(|value| value == det) {
                            existing
                        } else {
                            involved_detectors.push(*det);
                            involved_detectors.len() - 1
                        };
                        if bit < 64 {
                            detector_masks[slot] ^= 1u64 << bit;
                        }
                    }
                }
            }
        }

        for k in 3..num_cases {
            let c1 = k & (k - 1);
            let c2 = k ^ c1;
            if c1 != 0 {
                stored_ids[k] = Self::xor_sorted_targets(&stored_ids[c1], &stored_ids[c2]);
                if self.decompose_channel_errors {
                    detector_masks[k] = detector_masks[c1] ^ detector_masks[c2];
                }
            }
        }

        if self.decompose_channel_errors && involved_detectors.len() < 64 {
            Self::decompose_combination_targets(&mut stored_ids, &detector_masks);
        }

        let mut probs = probabilities.to_vec();
        if probabilities_are_disjoint {
            for k in 1..num_cases {
                if stored_ids[k].is_empty() {
                    for dst in 0..num_cases {
                        let src = dst ^ k;
                        if src > dst {
                            probs[dst] += probs[src];
                            probs[src] = 0.0;
                        }
                    }
                }
            }
        }

        for k in 1..num_cases {
            let prob = probs[k];
            if prob > 0.0 && !stored_ids[k].is_empty() {
                self.errors.push((prob, stored_ids[k].clone()));
            }
        }
    }

    fn emit_measurement_noise(&mut self, probability: f64) {
        if probability <= 0.0 || self.num_measurements == 0 {
            return;
        }
        let targets = self.measurement_sens[self.num_measurements - 1].targets.clone();
        if !targets.is_empty() {
            self.errors.push((probability, targets));
        }
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
        if block.len() > 2 && !self.options.approximate_disjoint_errors {
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
        if self.options.allow_gauge_detectors {
            return Ok(());
        }
        if !self.x_sens[q].is_empty() {
            return Err(format!(
                "non-deterministic {} encountered",
                self.sensitivity_kind_for_qubit(q),
            ));
        }
        Ok(())
    }

    fn ensure_y_collapse_is_deterministic(&self, q: usize) -> Result<(), String> {
        if self.options.allow_gauge_detectors {
            return Ok(());
        }
        if self.x_sens[q].targets != self.z_sens[q].targets {
            return Err(format!(
                "non-deterministic {} encountered",
                self.sensitivity_kind_for_qubit(q),
            ));
        }
        Ok(())
    }

    fn ensure_z_collapse_is_deterministic(&self, q: usize) -> Result<(), String> {
        if self.options.allow_gauge_detectors {
            return Ok(());
        }
        if !self.z_sens[q].is_empty() {
            return Err(format!(
                "non-deterministic {} encountered",
                self.sensitivity_kind_for_qubit(q),
            ));
        }
        Ok(())
    }

    fn ensure_no_pending_gauge(&self) -> Result<(), String> {
        if self.options.allow_gauge_detectors {
            return Ok(());
        }
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

#[cfg(test)]
fn symmetric_difference(
    a: &BTreeSet<DemTarget>,
    b: &BTreeSet<DemTarget>,
) -> BTreeSet<DemTarget> {
    a.symmetric_difference(b).cloned().collect()
}

#[cfg(test)]
fn detector_count(targets: &BTreeSet<DemTarget>) -> usize {
    targets
        .iter()
        .filter(|t| matches!(t, DemTarget::Detector(_)))
        .count()
}

#[cfg(test)]
fn set_to_targets(set: &BTreeSet<DemTarget>) -> Vec<DemTarget> {
    set.iter().cloned().collect()
}

fn detector_symptom_key(targets: &[DemTarget]) -> Vec<DemTarget> {
    let mut key: Vec<DemTarget> = targets
        .iter()
        .filter(|target| matches!(target, DemTarget::Detector(_)))
        .cloned()
        .collect();
    key.sort();
    key
}

fn obs_mask_of_targets(targets: &[DemTarget]) -> Result<(u64, u64), String> {
    if targets.len() >= 64 {
        return Err("not implemented: decomposing errors with more than 64 terms".to_string());
    }
    let mut obs_mask = 0u64;
    let mut used_mask = 0u64;
    for (k, target) in targets.iter().enumerate() {
        if let DemTarget::Observable(index) = target {
            if *index >= 64 {
                return Err(
                    "not implemented: decomposing errors with observable ids larger than 63"
                        .to_string(),
                );
            }
            obs_mask |= 1u64 << index;
            used_mask |= 1u64 << k;
        }
    }
    Ok((obs_mask, used_mask))
}

fn brute_force_decomp_helper(
    start: usize,
    mut used_term_mask: u64,
    remaining_obs_mask: u64,
    problem: &[DemTarget],
    known_symptoms: &BTreeMap<Vec<DemTarget>, Vec<DemTarget>>,
    out_result: &mut Vec<Vec<DemTarget>>,
) -> Result<bool, String> {
    let mut start = start;
    loop {
        if start >= problem.len() {
            return Ok(remaining_obs_mask == 0);
        }
        if ((used_term_mask >> start) & 1) == 0 {
            break;
        }
        start += 1;
    }
    used_term_mask |= 1 << start;

    for k in start + 1..=problem.len() {
        let mut key = vec![problem[start].clone()];
        if k < problem.len() {
            if ((used_term_mask >> k) & 1) != 0 {
                continue;
            }
            key.push(problem[k].clone());
            key.sort();
            used_term_mask ^= 1 << k;
        }
        if let Some(component) = known_symptoms.get(&key) {
            let obs_change = obs_mask_of_targets(component)?.0;
            if brute_force_decomp_helper(
                start + 1,
                used_term_mask,
                remaining_obs_mask ^ obs_change,
                problem,
                known_symptoms,
                out_result,
            )? {
                out_result.push(component.clone());
                return Ok(true);
            }
        }
        if k < problem.len() {
            used_term_mask ^= 1 << k;
        }
    }

    Ok(false)
}

fn brute_force_decomposition_into_known_graphlike_errors(
    problem: &[DemTarget],
    known_symptoms: &BTreeMap<Vec<DemTarget>, Vec<DemTarget>>,
) -> Result<Option<Vec<DemTarget>>, String> {
    let mut out = Vec::with_capacity(problem.len());
    let (obs_mask, used_mask) = obs_mask_of_targets(problem)?;
    let success = brute_force_decomp_helper(0, used_mask, obs_mask, problem, known_symptoms, &mut out)?;
    if !success {
        return Ok(None);
    }

    let mut flat = Vec::new();
    for component in out.iter().rev() {
        if !flat.is_empty() {
            flat.push(DemTarget::Separator);
        }
        flat.extend(component.iter().cloned());
    }
    Ok(Some(flat))
}

fn decompose_component_with_remnants(
    component: &[DemTarget],
    known_symptoms: &BTreeMap<Vec<DemTarget>, Vec<DemTarget>>,
) -> Option<Vec<DemTarget>> {
    let mut done = vec![false; component.len()];
    let mut num_component_detectors = 0usize;
    for (k, target) in component.iter().enumerate() {
        if matches!(target, DemTarget::Detector(_)) {
            num_component_detectors += 1;
        } else {
            done[k] = true;
        }
    }
    if num_component_detectors <= 2 {
        return Some(component.to_vec());
    }

    let mut flat = Vec::new();
    let mut sparse = SparseXorVec::default();
    sparse.xor_targets(component);

    for k in 0..component.len() {
        if done[k] {
            continue;
        }
        for k2 in k + 1..component.len() {
            if done[k2] {
                continue;
            }
            let key = detector_symptom_key(&[component[k].clone(), component[k2].clone()]);
            if let Some(match_component) = known_symptoms.get(&key) {
                done[k] = true;
                done[k2] = true;
                if !flat.is_empty() {
                    flat.push(DemTarget::Separator);
                }
                flat.extend(match_component.iter().cloned());
                sparse.xor_targets(match_component);
                break;
            }
        }
    }

    let mut missed = 0usize;
    for k in 0..component.len() {
        if !done[k] {
            let key = detector_symptom_key(&[component[k].clone()]);
            if let Some(match_component) = known_symptoms.get(&key) {
                done[k] = true;
                if !flat.is_empty() {
                    flat.push(DemTarget::Separator);
                }
                flat.extend(match_component.iter().cloned());
                sparse.xor_targets(match_component);
            }
        }
        if !done[k] {
            missed += 1;
        }
    }

    if missed > 2 {
        return None;
    }
    if !sparse.is_empty() {
        if !flat.is_empty() {
            flat.push(DemTarget::Separator);
        }
        flat.extend(sparse.targets);
    }
    Some(flat)
}

/// Decompose non-graphlike errors in a DEM into graphlike components.
///
/// A graphlike error has at most 2 detector targets per ^-separated component.
/// Non-graphlike errors (3+ detectors) are decomposed by finding combinations
/// of existing graphlike errors whose detector sets XOR to produce the
/// non-graphlike error's detector set.
pub fn decompose_errors(dem: &mut DetectorErrorModel) -> Result<(), String> {
    let instrs = dem.instructions().to_vec();
    if !instrs.iter().any(|instr| {
        matches!(
            instr,
            DemInstruction::Error { targets, .. } if !component_is_graphlike(targets)
        )
    }) {
        return Ok(());
    }

    let mut new_instrs = instrs;
    let mut known_symptoms: BTreeMap<Vec<DemTarget>, Vec<DemTarget>> = BTreeMap::new();
    for instr in &new_instrs {
        let DemInstruction::Error { probability, targets } = instr else {
            continue;
        };
        if *probability == 0.0 || targets.is_empty() {
            continue;
        }

        let mut start = 0usize;
        for k in 0..=targets.len() {
            if k == targets.len() || matches!(targets[k], DemTarget::Separator) {
                let component = &targets[start..k];
                let key = detector_symptom_key(component);
                if key.len() == 1 || key.len() == 2 {
                    known_symptoms.insert(key, component.to_vec());
                }
                start = k + 1;
            }
        }
    }

    for instr in &mut new_instrs {
        let DemInstruction::Error { targets, .. } = instr else {
            continue;
        };
        if component_is_graphlike(targets) {
            continue;
        }

        let original_targets = targets.clone();
        let mut rewritten = Vec::new();
        let mut start = 0usize;
        for k in 0..=original_targets.len() {
            if k == original_targets.len() || matches!(original_targets[k], DemTarget::Separator) {
                let component = &original_targets[start..k];
                let decomposed = if let Some(flat) =
                    brute_force_decomposition_into_known_graphlike_errors(component, &known_symptoms)?
                {
                    flat
                } else if let Some(flat) =
                    decompose_component_with_remnants(component, &known_symptoms)
                {
                    flat
                } else {
                    return Err(format!(
                        "failed to decompose non-graphlike error into graphlike components: {:?}",
                        original_targets
                    ));
                };
                if !rewritten.is_empty() && !decomposed.is_empty() {
                    rewritten.push(DemTarget::Separator);
                }
                rewritten.extend(decomposed);
                start = k + 1;
            }
        }
        *targets = rewritten;
    }

    let mut merged_errors: BTreeMap<Vec<DemTarget>, f64> = BTreeMap::new();
    let mut annotations = Vec::new();
    for instr in new_instrs {
        match instr {
            DemInstruction::Error {
                probability,
                targets,
            } => {
                if probability > 0.0 && !targets.is_empty() {
                    let targets = canonicalize_error_targets(&targets);
                    merged_errors
                        .entry(targets)
                        .and_modify(|existing| {
                            *existing = *existing + probability - 2.0 * *existing * probability;
                        })
                        .or_insert(probability);
                }
            }
            other => annotations.push(other),
        }
    }

    let mut new_dem = DetectorErrorModel::new();
    new_dem.set_min_counts(dem.num_detectors(), dem.num_observables());
    for (targets, probability) in merged_errors {
        if probability > 0.0 {
            new_dem.push(DemInstruction::Error {
                probability,
                targets,
            });
        }
    }
    for instr in annotations {
        new_dem.push(instr);
    }
    *dem = new_dem;
    Ok(())
}

#[cfg(test)]
mod internal_branch_tests {
    use super::*;
    use crate::ir::{PauliBasis, StimInstr, StimTarget};

    fn make_analyzer(num_qubits: usize, num_measurements: usize) -> ErrorAnalyzer {
        ErrorAnalyzer {
            x_sens: vec![SparseXorVec::default(); num_qubits],
            z_sens: vec![SparseXorVec::default(); num_qubits],
            measurement_sens: vec![SparseXorVec::default(); num_measurements],
            num_measurements,
            det_index: 0,
            errors: Vec::new(),
            decompose_channel_errors: false,
            options: AnalyzeOptions::default(),
        }
    }

    #[test]
    fn undo_op_reports_internal_mpad_underflow() {
        let mut analyzer = make_analyzer(0, 0);
        let err = analyzer
            .undo_op("MPAD", &[], &[StimTarget::Qubit(0)])
            .unwrap_err();
        assert!(err.contains("underflow"));
    }

    #[test]
    fn undo_op_reports_internal_else_correlated_error_without_leader() {
        let mut analyzer = make_analyzer(0, 0);
        let err = analyzer
            .undo_op("ELSE_CORRELATED_ERROR", &[0.25], &[StimTarget::pauli(0, PauliBasis::X, false)])
            .unwrap_err();
        assert!(err.contains("ELSE_CORRELATED_ERROR"));
    }

    #[test]
    fn emit_error_combinations_folds_disjoint_empty_combinations() {
        let mut analyzer = make_analyzer(0, 0);
        analyzer.emit_error_combinations(
            &[0.0, 0.1, 0.2, 0.3],
            &[
                vec![DemTarget::Detector(0)],
                vec![DemTarget::Detector(0)],
            ],
            true,
        );

        assert_eq!(analyzer.errors.len(), 1);
        assert_eq!(analyzer.errors[0].1, vec![DemTarget::Detector(0)]);
        assert!((analyzer.errors[0].0 - 0.3).abs() < 1e-12);
    }

    #[test]
    fn join_components_skips_empty_components() {
        let joined = ErrorAnalyzer::join_components_with_separators(vec![
            vec![DemTarget::Detector(0)],
            Vec::new(),
            vec![DemTarget::Detector(1)],
        ]);
        assert_eq!(
            joined,
            vec![
                DemTarget::Detector(0),
                DemTarget::Separator,
                DemTarget::Detector(1),
            ]
        );
    }

    #[test]
    fn decompose_combination_targets_keeps_unsolved_goal_as_single_component() {
        let mut stored_ids = vec![Vec::new(); 8];
        stored_ids[7] = vec![
            DemTarget::Detector(0),
            DemTarget::Detector(1),
            DemTarget::Detector(2),
        ];
        let detector_masks = vec![0, 0, 0, 0, 0, 0, 0, 0b111];

        ErrorAnalyzer::decompose_combination_targets(&mut stored_ids, &detector_masks);

        assert_eq!(
            stored_ids[7],
            vec![
                DemTarget::Detector(0),
                DemTarget::Detector(1),
                DemTarget::Detector(2),
            ]
        );
    }

    #[test]
    fn correlated_branch_from_instr_rejects_invalid_instruction() {
        let analyzer = make_analyzer(1, 0);
        let err = analyzer
            .correlated_branch_from_instr(&StimInstr::new(
                "X_ERROR",
                vec![0.1],
                vec![StimTarget::Qubit(0)],
            ))
            .unwrap_err();
        assert!(err.contains("invalid correlated error block"));
    }

    #[test]
    fn allow_gauge_short_circuits_x_and_y_determinism_checks() {
        let mut x_analyzer = make_analyzer(1, 0);
        x_analyzer.options.allow_gauge_detectors = true;
        x_analyzer.x_sens[0].targets = vec![DemTarget::Detector(0)];
        assert!(x_analyzer.ensure_x_collapse_is_deterministic(0).is_ok());

        let mut y_analyzer = make_analyzer(1, 0);
        y_analyzer.options.allow_gauge_detectors = true;
        y_analyzer.x_sens[0].targets = vec![DemTarget::Detector(0)];
        assert!(y_analyzer.ensure_y_collapse_is_deterministic(0).is_ok());
    }

    #[test]
    fn ensure_no_pending_gauge_distinguishes_observable_from_detector() {
        let mut observable_analyzer = make_analyzer(0, 1);
        observable_analyzer.measurement_sens[0].targets = vec![DemTarget::Observable(0)];
        let observable_err = observable_analyzer.ensure_no_pending_gauge().unwrap_err();
        assert!(observable_err.contains("observable"));

        let mut detector_analyzer = make_analyzer(0, 1);
        detector_analyzer.measurement_sens[0].targets = vec![DemTarget::Detector(0)];
        let detector_err = detector_analyzer.ensure_no_pending_gauge().unwrap_err();
        assert!(detector_err.contains("detector"));
    }

    #[test]
    fn target_helpers_ignore_non_matching_targets() {
        assert_eq!(
            split_pauli_products(&[
                StimTarget::pauli(0, PauliBasis::X, false),
                StimTarget::Combiner,
                StimTarget::Sweep(1),
                StimTarget::pauli(1, PauliBasis::Z, false),
            ])
            .len(),
            1
        );
        assert_eq!(qubits(&[StimTarget::Qubit(2), StimTarget::Rec(-1)]), vec![2]);
        assert_eq!(
            qubits_inv(&[StimTarget::QubitInv(3), StimTarget::Sweep(1)]),
            vec![3]
        );
    }

    #[test]
    fn try_disjoint_to_independent_xyz_covers_recursions_and_none_cases() {
        assert!(try_disjoint_to_independent_xyz(0.1, 0.9, 0.0).is_some());
        assert!(try_disjoint_to_independent_xyz(0.0, 0.2, 0.8).is_some());
        assert!(try_disjoint_to_independent_xyz(0.0, 0.0, 0.9).is_some());
        assert!(try_disjoint_to_independent_xyz(0.3, 0.3, 0.0).is_none());
        assert!(try_disjoint_to_independent_xyz(0.0, 0.01, 0.01).is_none());
    }

    #[test]
    fn decompose_helpers_cover_set_operations_and_separator_overflow() {
        let mut a = BTreeSet::new();
        a.insert(DemTarget::Detector(0));
        a.insert(DemTarget::Detector(1));
        let mut b = BTreeSet::new();
        b.insert(DemTarget::Detector(1));
        b.insert(DemTarget::Detector(2));

        let diff = symmetric_difference(&a, &b);
        assert_eq!(detector_count(&diff), 2);
        assert_eq!(
            set_to_targets(&diff),
            vec![DemTarget::Detector(0), DemTarget::Detector(2)]
        );
        assert!(!component_is_graphlike(&[
            DemTarget::Detector(0),
            DemTarget::Detector(1),
            DemTarget::Detector(2),
            DemTarget::Separator,
            DemTarget::Detector(3),
        ]));
    }
}
