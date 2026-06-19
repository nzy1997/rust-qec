use std::collections::BTreeSet;

use rbposd::{
    BpLsdDecoder, BpOsdDecoder, ChannelModel, Correction, DecodeError, DecoderConfig, LsdConfig,
    ParityCheckMatrix, Syndrome,
};
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

pub struct RbposdLsdDemDecoder {
    config: LsdConfig,
    bp_config: DecoderConfig,
}

impl RbposdLsdDemDecoder {
    pub fn new(config: LsdConfig) -> Self {
        Self::with_bp_config(config, DecoderConfig::default())
    }

    pub fn with_bp_config(config: LsdConfig, bp_config: DecoderConfig) -> Self {
        Self { config, bp_config }
    }
}

enum RbposdDemBackendConfig {
    Osd(DecoderConfig),
    Lsd {
        lsd_config: LsdConfig,
        bp_config: DecoderConfig,
    },
}

enum RbposdDemBackend {
    Osd(BpOsdDecoder),
    Lsd(BpLsdDecoder),
}

impl RbposdDemBackendConfig {
    fn compile(
        &self,
        pcm: ParityCheckMatrix,
        probabilities: Vec<f64>,
    ) -> Result<RbposdDemBackend, String> {
        let channel = ChannelModel::BitFlipProbabilities(probabilities);
        match self {
            Self::Osd(config) => {
                BpOsdDecoder::new(pcm, channel, config.clone()).map(RbposdDemBackend::Osd)
            }
            Self::Lsd {
                lsd_config,
                bp_config,
            } => BpLsdDecoder::with_bp_config(pcm, channel, *lsd_config, *bp_config)
                .map(RbposdDemBackend::Lsd),
        }
        .map_err(|error| format!("failed to compile rbposd decoder: {error}"))
    }
}

impl RbposdDemBackend {
    fn decode(&self, syndrome: &Syndrome) -> Result<Correction, DecodeError> {
        match self {
            Self::Osd(decoder) => decoder.decode(syndrome),
            Self::Lsd(decoder) => decoder.decode(syndrome),
        }
        .map(|result| result.correction)
    }
}

struct CompiledRbposdDemDecoder {
    decoder: Option<RbposdDemBackend>,
    num_dets: usize,
    num_obs: usize,
    observable_columns: Vec<Vec<usize>>,
    forced_syndrome: Vec<bool>,
    baseline_observables: Vec<bool>,
}

impl Decoder for RbposdDemDecoder {
    fn compile_for_dem(
        &self,
        dem: &DetectorErrorModel,
    ) -> Result<Box<dyn CompiledDecoder>, String> {
        compile_rbposd_dem_with_backend(dem, RbposdDemBackendConfig::Osd(self.config.clone()))
    }
}

impl Decoder for RbposdLsdDemDecoder {
    fn compile_for_dem(
        &self,
        dem: &DetectorErrorModel,
    ) -> Result<Box<dyn CompiledDecoder>, String> {
        compile_rbposd_dem_with_backend(
            dem,
            RbposdDemBackendConfig::Lsd {
                lsd_config: self.config,
                bp_config: self.bp_config,
            },
        )
    }
}

fn compile_rbposd_dem_with_backend(
    dem: &DetectorErrorModel,
    backend_config: RbposdDemBackendConfig,
) -> Result<Box<dyn CompiledDecoder>, String> {
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
        .map_err(|error| format!("invalid rbposd parity matrix: {error}"))?;

        Some(backend_config.compile(pcm, filtered_probabilities)?)
    };

    Ok(Box::new(CompiledRbposdDemDecoder {
        decoder,
        num_dets,
        num_obs,
        observable_columns: filtered_observable_columns,
        forced_syndrome,
        baseline_observables,
    }))
}

impl CompiledDecoder for CompiledRbposdDemDecoder {
    fn decode_shots_bit_packed(
        &self,
        dets: &[u8],
        num_shots: usize,
        num_dets: usize,
        num_obs: usize,
    ) -> Result<Vec<u8>, String> {
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
                let correction = decoder
                    .decode(&Syndrome::from(syndrome_bits))
                    .map_err(|error| format!("rbposd decode failed: {error}"))?;
                let decoded_observables =
                    correction_to_observables(&correction, &self.observable_columns, self.num_obs);
                xor_bits(&mut observable_bits, &decoded_observables);
            }

            for obs in 0..num_obs.min(self.num_obs) {
                if observable_bits[obs] {
                    out[shot * obs_bytes + (obs / 8)] |= 1 << (obs % 8);
                }
            }
        }

        Ok(out)
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

    #[test]
    fn repeat_shift_and_annotation_instructions_lower_with_offsets() {
        let dem = DetectorErrorModel::parse(
            "repeat 2 {\n    error(0.25) D0 D0 D1 D2 L0 L0\n    shift_detectors 3\n    detector(5, 0) D0\n    logical_observable L0\n}\n",
        )
        .unwrap();

        let (detector_columns, probabilities, observable_columns, num_dets, num_obs) =
            dem_to_matrix_problem(&dem);

        assert_eq!(num_dets, 7);
        assert_eq!(num_obs, 1);
        assert_eq!(detector_columns, vec![vec![1, 2], vec![4, 5]]);
        assert_eq!(probabilities, vec![0.25, 0.25]);
        assert_eq!(
            observable_columns,
            vec![Vec::<usize>::new(), Vec::<usize>::new()]
        );
    }
}
