use qec_ilp_core::BinaryIlpConfig;
use qec_ilp_core::backend::build_binary_backend;
use rstim::dem::DetectorErrorModel;

use crate::config::IlpDecoderConfig;
use crate::error::IlpDecodeError;
use crate::lowering::lower_dem_to_problem;
use crate::problem::LoweredDemProblem;

#[derive(Debug, Clone)]
pub struct IlpDemDecoder {
    problem: LoweredDemProblem,
    config: IlpDecoderConfig,
}

#[derive(Debug)]
pub struct CompiledIlpDemDecoder {
    problem: LoweredDemProblem,
    backend: Option<Box<dyn qec_ilp_core::backend::BinaryBackend>>,
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

    /// Decode shot-major, LSB-first detector bytes into equally packed observable predictions.
    /// Widths must match the DEM. Each shot occupies `ceil(width / 8)` bytes;
    /// unused high bits of the final detector byte are ignored.
    /// Returns an error for invalid dimensions, buffer sizes, or infeasible syndromes.
    pub fn decode_batch_bit_packed(
        &self,
        dets: &[u8],
        num_shots: usize,
        num_dets: usize,
        num_obs: usize,
    ) -> Result<Vec<u8>, IlpDecodeError> {
        validate_input(&self.problem, dets, num_shots, num_dets, num_obs)?;
        self.clone()
            .into_compiled()?
            .decode_batch_bit_packed(dets, num_shots, num_dets, num_obs)
    }

    pub fn into_compiled(self) -> Result<CompiledIlpDemDecoder, IlpDecodeError> {
        let backend = if self.problem.columns.is_empty() {
            None
        } else {
            let base_model = self.problem.to_binary_ilp_model()?;
            Some(build_binary_backend(
                &base_model,
                &BinaryIlpConfig {
                    backend: self.config.backend,
                },
            )?)
        };
        Ok(CompiledIlpDemDecoder {
            problem: self.problem,
            backend,
        })
    }
}

impl CompiledIlpDemDecoder {
    /// Decode shot-major, LSB-first detector bytes into equally packed observable predictions.
    /// Widths must match the DEM. Each shot occupies `ceil(width / 8)` bytes;
    /// unused high bits of the final detector byte are ignored.
    /// Returns an error for invalid dimensions, buffer sizes, or infeasible syndromes.
    pub fn decode_batch_bit_packed(
        &mut self,
        dets: &[u8],
        num_shots: usize,
        num_dets: usize,
        num_obs: usize,
    ) -> Result<Vec<u8>, IlpDecodeError> {
        let output_len = validate_input(&self.problem, dets, num_shots, num_dets, num_obs)?;
        let det_bytes = num_dets.div_ceil(8);
        let obs_bytes = num_obs.div_ceil(8);
        let mut out = Vec::new();
        out.try_reserve_exact(output_len)
            .map_err(|_| IlpDecodeError::OutputAllocationFailed { bytes: output_len })?;
        out.resize(output_len, 0u8);
        if num_dets == 0 && num_obs == 0 {
            return Ok(out);
        }
        if self.problem.columns.is_empty() {
            for shot in 0..num_shots {
                for (det, &forced) in self.problem.forced_syndrome.iter().enumerate() {
                    let bit = ((dets[shot * det_bytes + det / 8] >> (det % 8)) & 1) != 0;
                    if bit != forced {
                        return Err(IlpDecodeError::InfeasibleSyndrome { shot });
                    }
                }
                for obs in 0..num_obs {
                    if self.problem.baseline_observables[obs] {
                        out[shot * obs_bytes + (obs / 8)] |= 1 << (obs % 8);
                    }
                }
            }
            return Ok(out);
        }
        let backend = self
            .backend
            .as_mut()
            .expect("non-empty problem has a compiled backend");

        for shot in 0..num_shots {
            let mut syndrome = vec![false; num_dets];
            for det in 0..num_dets {
                let byte = dets[shot * det_bytes + (det / 8)];
                syndrome[det] = ((byte >> (det % 8)) & 1) != 0;
            }

            for (row, (&bit, &forced)) in syndrome
                .iter()
                .zip(&self.problem.forced_syndrome)
                .enumerate()
            {
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

fn validate_input(
    problem: &LoweredDemProblem,
    dets: &[u8],
    num_shots: usize,
    num_dets: usize,
    num_obs: usize,
) -> Result<usize, IlpDecodeError> {
    if num_dets != problem.num_detectors {
        return Err(IlpDecodeError::DetectorWidthMismatch {
            expected: problem.num_detectors,
            actual: num_dets,
        });
    }
    if num_obs != problem.num_observables {
        return Err(IlpDecodeError::ObservableWidthMismatch {
            expected: problem.num_observables,
            actual: num_obs,
        });
    }
    let expected_det_len = packed_len(num_shots, num_dets, "detectors")?;
    let output_len = packed_len(num_shots, num_obs, "observables")?;
    if dets.len() != expected_det_len {
        return Err(IlpDecodeError::PackedDetectionsLengthMismatch {
            expected: expected_det_len,
            actual: dets.len(),
        });
    }
    Ok(output_len)
}

fn packed_len(shots: usize, bits: usize, buffer: &'static str) -> Result<usize, IlpDecodeError> {
    shots
        .checked_mul(bits.div_ceil(8))
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or(IlpDecodeError::PackedBufferSizeOverflow {
            buffer,
            shots,
            bits,
        })
}
