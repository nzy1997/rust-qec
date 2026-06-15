use rstim::dem::DetectorErrorModel;
use qec_ilp_core::BinaryIlpConfig;
use qec_ilp_core::backend::build_binary_backend;

use crate::config::IlpDecoderConfig;
use crate::error::IlpDecodeError;
use crate::lowering::lower_dem_to_problem;
use crate::problem::LoweredDemProblem;

#[derive(Debug, Clone)]
pub struct IlpDemDecoder {
    problem: LoweredDemProblem,
    config: IlpDecoderConfig,
}

impl IlpDemDecoder {
    pub fn from_dem(
        dem: &DetectorErrorModel,
        config: IlpDecoderConfig,
    ) -> Result<Self, IlpDecodeError> {
        Ok(Self {
            problem: lower_dem_to_problem(dem)?,
            config,
        })
    }

    pub fn decode_batch_bit_packed(
        &self,
        dets: &[u8],
        num_shots: usize,
        num_dets: usize,
        num_obs: usize,
    ) -> Result<Vec<u8>, IlpDecodeError> {
        if num_dets != self.problem.num_detectors {
            return Err(IlpDecodeError::DetectorWidthMismatch {
                expected: self.problem.num_detectors,
                actual: num_dets,
            });
        }
        if num_obs != self.problem.num_observables {
            return Err(IlpDecodeError::ObservableWidthMismatch {
                expected: self.problem.num_observables,
                actual: num_obs,
            });
        }

        let det_bytes = num_dets.div_ceil(8);
        let obs_bytes = num_obs.div_ceil(8);
        let expected_det_len = num_shots * det_bytes;
        if dets.len() != expected_det_len {
            return Err(IlpDecodeError::PackedDetectionsLengthMismatch {
                expected: expected_det_len,
                actual: dets.len(),
            });
        }
        let mut out = vec![0u8; num_shots * obs_bytes];
        if self.problem.columns.is_empty() {
            for shot in 0..num_shots {
                for obs in 0..num_obs {
                    if self.problem.baseline_observables[obs] {
                        out[shot * obs_bytes + (obs / 8)] |= 1 << (obs % 8);
                    }
                }
            }
            return Ok(out);
        }
        let base_model = self.problem.to_binary_ilp_model()?;
        let mut backend = build_binary_backend(
            &base_model,
            &BinaryIlpConfig {
                backend: self.config.backend.clone(),
            },
        )?;

        for shot in 0..num_shots {
            let mut syndrome = vec![false; num_dets];
            for det in 0..num_dets {
                let byte = dets[shot * det_bytes + (det / 8)];
                syndrome[det] = ((byte >> (det % 8)) & 1) != 0;
            }

            for (row, (&bit, &forced)) in syndrome.iter().zip(&self.problem.forced_syndrome).enumerate() {
                let rhs = if bit ^ forced { 1.0 } else { 0.0 };
                backend.set_rhs(row, rhs)?;
            }

            let correction = backend.solve()?.binary_values;
            let observables = self.problem.observables_from_correction(&correction)?;
            for obs in 0..num_obs {
                if observables[obs] {
                    out[shot * obs_bytes + (obs / 8)] |= 1 << (obs % 8);
                }
            }
        }

        Ok(out)
    }
}
