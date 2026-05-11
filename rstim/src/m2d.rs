use crate::data_path::{build_reference_sample, ReferenceSampleMode};
use crate::ir::{StimInstr, StimTarget};
use crate::sim::bit_table::BitTable;

pub struct M2dOutput {
    pub detections: BitTable,
    pub observable_flips: BitTable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct M2dOptions {
    pub reference_sample_mode: ReferenceSampleMode,
    pub ran_without_feedback: bool,
}

impl Default for M2dOptions {
    fn default() -> Self {
        Self {
            reference_sample_mode: ReferenceSampleMode::SimulateNoiseless,
            ran_without_feedback: false,
        }
    }
}

/// Convert raw measurement bits to detection events.
/// meas_table: BitTable(num_measurements, num_shots)
/// Returns M2dOutput with detections and observable_flips BitTables.
pub fn measurements_to_detections(
    instrs: &[StimInstr],
    meas_table: &BitTable,
) -> Result<M2dOutput, String> {
    measurements_to_detections_with_options(instrs, meas_table, None, M2dOptions::default())
}

pub fn measurements_to_detections_with_options(
    instrs: &[StimInstr],
    meas_table: &BitTable,
    sweep_table: Option<&BitTable>,
    options: M2dOptions,
) -> Result<M2dOutput, String> {
    let normalization = if options.ran_without_feedback {
        Some(crate::transforms::normalize_feedbackless_m2d(instrs)?)
    } else {
        None
    };
    let work_instrs = normalization
        .as_ref()
        .map(|normalized| normalized.circuit.as_slice())
        .unwrap_or(instrs);
    let empty_corrections: &[Vec<usize>] = &[];
    let measurement_corrections = normalization
        .as_ref()
        .map(|normalized| normalized.measurement_corrections.as_slice())
        .unwrap_or(empty_corrections);

    let shared_reference = match options.reference_sample_mode {
        ReferenceSampleMode::SimulateNoiseless if sweep_table.is_none() => {
            Some(build_reference_sample(work_instrs, ReferenceSampleMode::SimulateNoiseless)?)
        }
        ReferenceSampleMode::AssumeAllZero => Some(vec![false; crate::stats::num_measurements(work_instrs)]),
        ReferenceSampleMode::SimulateNoiseless => None,
    };
    let n_meas = shared_reference
        .as_ref()
        .map(|reference| reference.len())
        .unwrap_or_else(|| crate::stats::num_measurements(work_instrs));
    let n_shots = meas_table.num_minor();

    if meas_table.num_major() != n_meas {
        return Err(format!(
            "meas_table has {} bits but circuit has {} measurements",
            meas_table.num_major(), n_meas
        ));
    }
    if let Some(sweep_table) = sweep_table {
        if sweep_table.num_minor() != n_shots {
            return Err(format!(
                "sweep shots {} do not match measurement shots {}",
                sweep_table.num_minor(),
                n_shots
            ));
        }
    }

    let det_obs = collect_det_obs(work_instrs)?;
    let n_dets = det_obs.detectors.len();
    let n_obs = det_obs.observables.len();

    let mut dets = BitTable::new(n_dets, n_shots);
    let mut obs = BitTable::new(n_obs, n_shots);

    for shot in 0..n_shots {
        let per_shot_reference;
        let reference = if let Some(reference) = shared_reference.as_ref() {
            reference.as_slice()
        } else {
            let sweep_table = sweep_table.expect("validated above");
            let sweep_row: Vec<bool> = (0..sweep_table.num_major())
                .map(|i| sweep_table.get(i, shot))
                .collect();
            per_shot_reference =
                crate::executor::reference_sample_with_sweep_bits(work_instrs, Some(&sweep_row))?;
            per_shot_reference.as_slice()
        };
        let mut flips = Vec::with_capacity(n_meas);
        for i in 0..n_meas {
            let mut flip = meas_table.get(i, shot) ^ reference[i];
            if let Some(extra_terms) = measurement_corrections.get(i) {
                for &j in extra_terms {
                    flip ^= flips[j];
                }
            }
            flips.push(flip);
        }

        for (d, rec_offsets) in det_obs.detectors.iter().enumerate() {
            let val = rec_offsets.iter().fold(false, |acc, &r| acc ^ flips[r]);
            if val { dets.set(d, shot, true); }
        }
        for (o, rec_offsets) in det_obs.observables.iter().enumerate() {
            let val = rec_offsets.iter().fold(false, |acc, &r| acc ^ flips[r]);
            if val { obs.set(o, shot, true); }
        }
    }

    Ok(M2dOutput { detections: dets, observable_flips: obs })
}

struct DetObsDef {
    detectors: Vec<Vec<usize>>,
    observables: Vec<Vec<usize>>,
}

fn collect_det_obs(instrs: &[StimInstr]) -> Result<DetObsDef, String> {
    let mut detectors = Vec::new();
    let mut observables: Vec<Vec<usize>> = Vec::new();
    let mut meas_count = 0usize;
    collect_det_obs_instrs(instrs, &mut meas_count, &mut detectors, &mut observables)?;
    Ok(DetObsDef { detectors, observables })
}

fn collect_det_obs_instrs(
    instrs: &[StimInstr],
    meas_count: &mut usize,
    detectors: &mut Vec<Vec<usize>>,
    observables: &mut Vec<Vec<usize>>,
) -> Result<(), String> {
    for instr in instrs {
        match instr {
            StimInstr::Op { name, targets, args, .. } => {
                match name.as_str() {
                    "DETECTOR" => {
                        let indices: Vec<usize> = targets.iter().filter_map(|t| {
                            if let StimTarget::Rec(r) = t {
                                Some((*meas_count as i64 + *r as i64) as usize)
                            } else { None }
                        }).collect();
                        detectors.push(indices);
                    }
                    "OBSERVABLE_INCLUDE" => {
                        let idx = args.first().copied().unwrap_or(0.0) as usize;
                        while observables.len() <= idx { observables.push(Vec::new()); }
                        for t in targets {
                            if let StimTarget::Rec(r) = t {
                                let abs = (*meas_count as i64 + *r as i64) as usize;
                                observables[idx].push(abs);
                            }
                        }
                    }
                    _ => {
                        *meas_count += count_measurements_op(name, targets);
                    }
                }
            }
            StimInstr::Repeat { count, body } => {
                for _ in 0..*count {
                    collect_det_obs_instrs(body, meas_count, detectors, observables)?;
                }
            }
        }
    }
    Ok(())
}

fn count_measurements_op(name: &str, targets: &[StimTarget]) -> usize {
    match name {
        "M" | "MX" | "MY" | "MR" | "MRX" | "MRY" | "MZ" | "MRZ" => {
            targets.iter().filter(|t| matches!(t, StimTarget::Qubit(_) | StimTarget::QubitInv(_))).count()
        }
        "ML" | "MXL" | "MYL" | "MZL" | "MRL" | "MRXL" | "MRYL" | "MRZL" => {
            2 * targets
                .iter()
                .filter(|t| matches!(t, StimTarget::Qubit(_) | StimTarget::QubitInv(_)))
                .count()
        }
        "MXX" | "MYY" | "MZZ" => targets.len() / 2,
        "MPP" => targets.iter().filter(|t| matches!(t, StimTarget::Combiner)).count() + 1,
        "MPAD" => targets.len(),
        _ => 0,
    }
}
