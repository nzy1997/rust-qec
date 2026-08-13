use crate::data_path::{build_reference_sample, ReferenceSampleMode};
use crate::ir::StimInstr;
use crate::measurement_transform::{CheckedMeasurementLayout, MeasurementTransformLimits};
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
        ReferenceSampleMode::SimulateNoiseless if sweep_table.is_none() => Some(
            build_reference_sample(work_instrs, ReferenceSampleMode::SimulateNoiseless)?,
        ),
        ReferenceSampleMode::AssumeAllZero => {
            Some(vec![false; crate::stats::num_measurements(work_instrs)])
        }
        ReferenceSampleMode::SimulateNoiseless => None,
    };
    measurements_to_detections_impl(
        work_instrs,
        meas_table,
        sweep_table,
        shared_reference.as_deref(),
        measurement_corrections,
    )
}

pub(crate) fn measurements_to_detections_with_reference(
    instrs: &[StimInstr],
    meas_table: &BitTable,
    reference_sample: &[bool],
) -> Result<M2dOutput, String> {
    measurements_to_detections_impl(instrs, meas_table, None, Some(reference_sample), &[])
}

fn measurements_to_detections_impl(
    work_instrs: &[StimInstr],
    meas_table: &BitTable,
    sweep_table: Option<&BitTable>,
    shared_reference: Option<&[bool]>,
    measurement_corrections: &[Vec<usize>],
) -> Result<M2dOutput, String> {
    let layout = CheckedMeasurementLayout::from_circuit_with_limits(
        work_instrs,
        MeasurementTransformLimits::default(),
    )
    .map_err(|err| err.to_string())?;
    let n_meas = layout.num_measurements();
    if let Some(reference) = shared_reference {
        if reference.len() != n_meas {
            return Err(reference_measurement_count_mismatch(
                reference.len(),
                n_meas,
            ));
        }
    }
    let n_shots = meas_table.num_minor();

    if meas_table.num_major() != n_meas {
        return Err(format!(
            "meas_table has {} bits but circuit has {} measurements",
            meas_table.num_major(),
            n_meas
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

    let n_dets = layout.num_detectors();
    let n_obs = layout.num_observables();

    let mut dets = BitTable::try_new(n_dets, n_shots)
        .map_err(|err| format!("failed to allocate detection table: {err:?}"))?;
    let mut obs = BitTable::try_new(n_obs, n_shots)
        .map_err(|err| format!("failed to allocate observable table: {err:?}"))?;

    for shot in 0..n_shots {
        let per_shot_reference;
        let reference = if let Some(reference) = shared_reference {
            reference
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

        for (d, rec_offsets) in layout.detector_rows().iter().enumerate() {
            let val = rec_offsets.iter().fold(false, |acc, &r| acc ^ flips[r]);
            if val {
                dets.set(d, shot, true);
            }
        }
        for (o, rec_offsets) in layout.observable_rows().iter().enumerate() {
            let val = rec_offsets.iter().fold(false, |acc, &r| acc ^ flips[r]);
            if val {
                obs.set(o, shot, true);
            }
        }
    }

    Ok(M2dOutput {
        detections: dets,
        observable_flips: obs,
    })
}

fn reference_measurement_count_mismatch(reference_len: usize, n_meas: usize) -> String {
    format!("reference has {reference_len} bits but circuit has {n_meas} measurements")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precomputed_reference_is_used_instead_of_rebuilding_from_the_circuit() {
        let instrs = crate::parser::parse_lines("X 0\nM 0\nDETECTOR rec[-1]\n").unwrap();
        let mut measurements = BitTable::try_new(1, 1).unwrap();
        measurements.set(0, 0, true);

        let output =
            measurements_to_detections_with_reference(&instrs, &measurements, &[false]).unwrap();

        assert!(output.detections.get(0, 0));
    }

    #[test]
    fn reference_measurement_count_mismatch_is_actionable() {
        assert_eq!(
            reference_measurement_count_mismatch(2, 3),
            "reference has 2 bits but circuit has 3 measurements"
        );
    }
}
