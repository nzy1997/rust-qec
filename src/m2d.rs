use crate::ir::{StimInstr, StimTarget};
use crate::executor::reference_sample;
use crate::sim::bit_table::BitTable;

pub struct M2dOutput {
    pub detections: BitTable,
    pub observable_flips: BitTable,
}

/// Convert raw measurement bits to detection events.
/// meas_table: BitTable(num_measurements, num_shots)
/// Returns M2dOutput with detections and observable_flips BitTables.
pub fn measurements_to_detections(
    instrs: &[StimInstr],
    meas_table: &BitTable,
) -> Result<M2dOutput, String> {
    let reference = reference_sample(instrs)?;
    let n_meas = reference.len();
    let n_shots = meas_table.num_minor();

    if meas_table.num_major() != n_meas {
        return Err(format!(
            "meas_table has {} bits but circuit has {} measurements",
            meas_table.num_major(), n_meas
        ));
    }

    let det_obs = collect_det_obs(instrs)?;
    let n_dets = det_obs.detectors.len();
    let n_obs = det_obs.observables.len();

    let mut dets = BitTable::new(n_dets, n_shots);
    let mut obs = BitTable::new(n_obs, n_shots);

    for shot in 0..n_shots {
        let flips: Vec<bool> = (0..n_meas)
            .map(|i| meas_table.get(i, shot) ^ reference[i])
            .collect();

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
        "MXX" | "MYY" | "MZZ" => targets.len() / 2,
        "MPP" => targets.iter().filter(|t| matches!(t, StimTarget::Combiner)).count() + 1,
        "MPAD" => targets.len(),
        _ => 0,
    }
}
