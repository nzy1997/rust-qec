use std::collections::BTreeSet;

use rbposd::{BpOsdDecoder, ChannelModel, Correction, DecoderConfig, ParityCheckMatrix, Syndrome};
use rstim::dem::{DemInstruction, DemTarget, DetectorErrorModel};

use crate::decode::{CompiledDecoder, Decoder};

pub struct RbposdDemDecoder {
    config: DecoderConfig,
}

impl RbposdDemDecoder {
    pub fn new(config: DecoderConfig) -> Self {
        Self { config }
    }
}

struct CompiledRbposdDemDecoder {
    decoder: Option<BpOsdDecoder>,
    num_dets: usize,
    num_obs: usize,
    observable_columns: Vec<Vec<usize>>,
    forced_syndrome: Vec<bool>,
    baseline_observables: Vec<bool>,
}

impl Decoder for RbposdDemDecoder {
    fn compile_for_dem(&self, dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder> {
        let (detector_columns, probabilities, observable_columns, num_dets, num_obs) =
            dem_to_matrix_problem(dem);

        let mut filtered_detector_columns = Vec::new();
        let mut filtered_observable_columns = Vec::new();
        let mut filtered_probabilities = Vec::new();
        let mut forced_syndrome = vec![false; num_dets];
        let mut baseline_observables = vec![false; num_obs];

        for ((detectors, observables), probability) in detector_columns
            .into_iter()
            .zip(observable_columns.into_iter())
            .zip(probabilities.into_iter())
        {
            if probability <= 0.0 {
                continue;
            }

            if probability >= 1.0 {
                xor_indices(&mut forced_syndrome, &detectors);
                xor_indices(&mut baseline_observables, &observables);
                continue;
            }

            if detectors.is_empty() {
                if probability > 0.5 {
                    xor_indices(&mut baseline_observables, &observables);
                }
                continue;
            }

            filtered_detector_columns.push(detectors);
            filtered_observable_columns.push(observables);
            filtered_probabilities.push(probability);
        }

        let decoder = if filtered_detector_columns.is_empty() {
            None
        } else {
            let pcm = ParityCheckMatrix::from_sparse_columns(
                num_dets,
                filtered_detector_columns.len(),
                filtered_detector_columns,
            )
            .expect("generated DEM matrix should be valid");

            Some(
                BpOsdDecoder::new(
                    pcm,
                    ChannelModel::BitFlipProbabilities(filtered_probabilities),
                    self.config.clone(),
                )
                .expect("DEM lowering produced an invalid rbposd problem"),
            )
        };

        Box::new(CompiledRbposdDemDecoder {
            decoder,
            num_dets,
            num_obs,
            observable_columns: filtered_observable_columns,
            forced_syndrome,
            baseline_observables,
        })
    }
}

impl CompiledDecoder for CompiledRbposdDemDecoder {
    fn decode_shots_bit_packed(
        &self,
        dets: &[u8],
        num_shots: usize,
        num_dets: usize,
        num_obs: usize,
    ) -> Vec<u8> {
        let det_bytes = num_dets.div_ceil(8);
        let obs_bytes = num_obs.div_ceil(8);
        let mut out = vec![0u8; num_shots * obs_bytes];

        for shot in 0..num_shots {
            let det_offset = shot * det_bytes;
            let mut syndrome_bits = vec![false; self.num_dets];
            for det in 0..num_dets.min(self.num_dets) {
                let byte = dets[det_offset + (det / 8)];
                syndrome_bits[det] = ((byte >> (det % 8)) & 1) != 0;
            }

            xor_bits(&mut syndrome_bits, &self.forced_syndrome);

            let mut observable_bits = self.baseline_observables.clone();
            if let Some(decoder) = &self.decoder {
                let result = decoder
                    .decode(&Syndrome::from(syndrome_bits))
                    .expect("rbposd decode failed");
                let decoded_observables = correction_to_observables(
                    &result.correction,
                    &self.observable_columns,
                    self.num_obs,
                );
                xor_bits(&mut observable_bits, &decoded_observables);
            }

            for obs in 0..num_obs.min(self.num_obs) {
                if observable_bits[obs] {
                    out[shot * obs_bytes + (obs / 8)] |= 1 << (obs % 8);
                }
            }
        }

        out
    }
}

fn correction_to_observables(
    correction: &Correction,
    observable_columns: &[Vec<usize>],
    num_obs: usize,
) -> Vec<bool> {
    let mut observable_bits = vec![false; num_obs];
    for (column, &enabled) in correction.as_slice().iter().enumerate() {
        if !enabled {
            continue;
        }
        for &obs in &observable_columns[column] {
            observable_bits[obs] ^= true;
        }
    }
    observable_bits
}

fn dem_to_matrix_problem(
    dem: &DetectorErrorModel,
) -> (Vec<Vec<usize>>, Vec<f64>, Vec<Vec<usize>>, usize, usize) {
    let num_dets = dem.effective_num_detectors();
    let num_obs = dem.num_observables();
    let mut detector_columns = Vec::new();
    let mut observable_columns = Vec::new();
    let mut probabilities = Vec::new();

    visit_dem(
        dem.instructions(),
        0,
        &mut detector_columns,
        &mut observable_columns,
        &mut probabilities,
    );

    (
        detector_columns,
        probabilities,
        observable_columns,
        num_dets,
        num_obs,
    )
}

fn visit_dem(
    instrs: &[DemInstruction],
    detector_offset: usize,
    detector_columns: &mut Vec<Vec<usize>>,
    observable_columns: &mut Vec<Vec<usize>>,
    probabilities: &mut Vec<f64>,
) -> usize {
    let mut offset = detector_offset;
    for instr in instrs {
        match instr {
            DemInstruction::Error {
                probability,
                targets,
            } => push_error_columns(
                *probability,
                targets,
                offset,
                detector_columns,
                observable_columns,
                probabilities,
            ),
            DemInstruction::ShiftDetectors {
                detector_offset, ..
            } => {
                offset += detector_offset;
            }
            DemInstruction::Repeat { count, body } => {
                for _ in 0..*count {
                    offset = visit_dem(
                        body.instructions(),
                        offset,
                        detector_columns,
                        observable_columns,
                        probabilities,
                    );
                }
            }
            DemInstruction::Detector { .. } | DemInstruction::LogicalObservable { .. } => {}
        }
    }
    offset
}

fn push_error_columns(
    probability: f64,
    targets: &[DemTarget],
    detector_offset: usize,
    detector_columns: &mut Vec<Vec<usize>>,
    observable_columns: &mut Vec<Vec<usize>>,
    probabilities: &mut Vec<f64>,
) {
    let mut current_dets = BTreeSet::new();
    let mut current_obs = BTreeSet::new();
    for target in targets {
        match target {
            DemTarget::Detector(det) => toggle_target(&mut current_dets, detector_offset + det),
            DemTarget::Observable(obs) => toggle_target(&mut current_obs, *obs),
            DemTarget::Separator => {}
        }
    }
    detector_columns.push(current_dets.into_iter().collect());
    observable_columns.push(current_obs.into_iter().collect());
    probabilities.push(probability);
}

fn toggle_target(set: &mut BTreeSet<usize>, value: usize) {
    if !set.insert(value) {
        set.remove(&value);
    }
}

fn xor_indices(bits: &mut [bool], indices: &[usize]) {
    for &index in indices {
        bits[index] ^= true;
    }
}

fn xor_bits(bits: &mut [bool], other: &[bool]) {
    for (lhs, rhs) in bits.iter_mut().zip(other.iter()) {
        *lhs ^= *rhs;
    }
}

#[cfg(test)]
mod tests {
    use super::dem_to_matrix_problem;
    use rstim::dem::DetectorErrorModel;

    #[test]
    fn separator_targets_stay_in_one_dem_column() {
        let dem = DetectorErrorModel::parse("error(0.25) D0 ^ D1 L0\n").unwrap();

        let (detector_columns, probabilities, observable_columns, num_dets, num_obs) =
            dem_to_matrix_problem(&dem);

        assert_eq!(num_dets, 2);
        assert_eq!(num_obs, 1);
        assert_eq!(detector_columns, vec![vec![0, 1]]);
        assert_eq!(probabilities, vec![0.25]);
        assert_eq!(observable_columns, vec![vec![0]]);
    }
}
