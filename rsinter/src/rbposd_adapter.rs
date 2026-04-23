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
    decoder: BpOsdDecoder,
    num_dets: usize,
    num_obs: usize,
    observable_columns: Vec<Vec<usize>>,
}

impl Decoder for RbposdDemDecoder {
    fn compile_for_dem(&self, dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder> {
        let (pcm, probabilities, observable_columns, num_dets, num_obs) =
            dem_to_matrix_problem(dem);
        let decoder = BpOsdDecoder::new(
            pcm,
            ChannelModel::BitFlipProbabilities(probabilities),
            self.config.clone(),
        )
        .expect("DEM lowering produced an invalid rbposd problem");

        Box::new(CompiledRbposdDemDecoder {
            decoder,
            num_dets,
            num_obs,
            observable_columns,
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

            let result = self
                .decoder
                .decode(&Syndrome::from(syndrome_bits))
                .expect("rbposd decode failed");
            let observable_bits = correction_to_observables(
                &result.correction,
                &self.observable_columns,
                self.num_obs,
            );

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
) -> (ParityCheckMatrix, Vec<f64>, Vec<Vec<usize>>, usize, usize) {
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

    let pcm =
        ParityCheckMatrix::from_sparse_columns(num_dets, detector_columns.len(), detector_columns)
            .expect("generated DEM matrix should be valid");

    (pcm, probabilities, observable_columns, num_dets, num_obs)
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
    let mut current_dets = Vec::new();
    let mut current_obs = Vec::new();
    for target in targets {
        match target {
            DemTarget::Detector(det) => current_dets.push(detector_offset + det),
            DemTarget::Observable(obs) => current_obs.push(*obs),
            DemTarget::Separator => {
                detector_columns.push(current_dets);
                observable_columns.push(current_obs);
                probabilities.push(probability);
                current_dets = Vec::new();
                current_obs = Vec::new();
            }
        }
    }
    detector_columns.push(current_dets);
    observable_columns.push(current_obs);
    probabilities.push(probability);
}
