use std::sync::Mutex;

use crate::bp::BpWorkspace;
use crate::config::{ChannelModel, DecoderConfig, LsdConfig, LsdMethod};
use crate::decoder::DecodeResult;
use crate::decoder_core::BpCore;
use crate::error::DecodeError;
use crate::lsd::{LsdWorkspace, decode_lsd_with_workspace};
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

#[derive(Debug)]
pub struct BpLsdDecoder {
    pcm: ParityCheckMatrix,
    core: BpCore,
    config: LsdConfig,
    bp_config: DecoderConfig,
    bp_workspace: Mutex<BpWorkspace>,
    lsd_workspace: Mutex<LsdWorkspace>,
}

impl Clone for BpLsdDecoder {
    fn clone(&self) -> Self {
        Self {
            pcm: self.pcm.clone(),
            core: self.core.clone(),
            config: self.config,
            bp_config: self.bp_config,
            bp_workspace: Mutex::new(self.core.workspace()),
            lsd_workspace: Mutex::new(LsdWorkspace::new(&self.pcm)),
        }
    }
}

impl BpLsdDecoder {
    pub fn new(
        pcm: ParityCheckMatrix,
        channel: ChannelModel,
        config: LsdConfig,
    ) -> Result<Self, DecodeError> {
        Self::with_bp_config(pcm, channel, config, DecoderConfig::default())
    }

    pub fn with_bp_config(
        pcm: ParityCheckMatrix,
        channel: ChannelModel,
        config: LsdConfig,
        bp_config: DecoderConfig,
    ) -> Result<Self, DecodeError> {
        match config.method {
            LsdMethod::LocalizedStatistics => {}
        }
        if config.lsd_order > 1 {
            return Err(DecodeError::UnsupportedLsdOrder {
                order: config.lsd_order,
            });
        }

        let core = BpCore::new(&pcm, &channel)?;
        let bp_workspace = Mutex::new(core.workspace());
        let lsd_workspace = Mutex::new(LsdWorkspace::new(&pcm));

        Ok(Self {
            pcm,
            core,
            config,
            bp_config,
            bp_workspace,
            lsd_workspace,
        })
    }

    pub fn decode(&self, syndrome: &Syndrome) -> Result<DecodeResult, DecodeError> {
        if syndrome.len() != self.pcm.num_checks() {
            return Err(DecodeError::DimensionMismatch {
                what: "syndrome",
                expected: self.pcm.num_checks(),
                actual: syndrome.len(),
            });
        }

        if syndrome.weight() == 0 {
            let prior_correction = self.core.hard_decision_from_prior();
            if self.pcm.multiply(&prior_correction) == *syndrome {
                return Ok(DecodeResult {
                    correction: prior_correction,
                    converged: true,
                    bp_iterations: 0,
                    used_osd: false,
                    residual_syndrome_weight: 0,
                });
            }
        }

        let mut bp_workspace = self.bp_workspace.lock().unwrap();
        let bp_info =
            self.core
                .run_minimum_sum_in_place(syndrome, &self.bp_config, &mut bp_workspace);
        if bp_info.residual_weight == 0 {
            return Ok(DecodeResult {
                correction: Correction::from(bp_workspace.hard_decision_bits.clone()),
                converged: bp_info.converged,
                bp_iterations: bp_info.iterations,
                used_osd: false,
                residual_syndrome_weight: 0,
            });
        }

        let correction = {
            let target_syndrome = xor_syndromes(
                &multiply_bits(&self.pcm, &bp_workspace.hard_decision_bits),
                syndrome,
            );
            let residual = {
                let mut lsd_workspace = self.lsd_workspace.lock().unwrap();
                decode_lsd_with_workspace(
                    &self.pcm,
                    &target_syndrome,
                    &bp_workspace.reliability,
                    self.config.lsd_order,
                    &mut lsd_workspace,
                )?
            };
            xor_correction_bits(&bp_workspace.hard_decision_bits, &residual)
        };
        drop(bp_workspace);

        Ok(DecodeResult {
            correction,
            converged: bp_info.converged,
            bp_iterations: bp_info.iterations,
            used_osd: false,
            residual_syndrome_weight: 0,
        })
    }
}

fn multiply_bits(pcm: &ParityCheckMatrix, bits: &[bool]) -> Syndrome {
    pcm.multiply(&Correction::from(bits.to_vec()))
}

fn xor_syndromes(lhs: &Syndrome, rhs: &Syndrome) -> Syndrome {
    Syndrome::from(
        lhs.as_slice()
            .iter()
            .zip(rhs.as_slice().iter())
            .map(|(a, b)| *a ^ *b)
            .collect::<Vec<_>>(),
    )
}

fn xor_correction_bits(lhs: &[bool], rhs: &Correction) -> Correction {
    Correction::from(
        lhs.iter()
            .zip(rhs.as_slice().iter())
            .map(|(a, b)| *a ^ *b)
            .collect::<Vec<_>>(),
    )
}
