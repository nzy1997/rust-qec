use std::sync::Mutex;

use crate::bp::BpWorkspace;
use crate::config::{ChannelModel, DecoderConfig, LsdConfig, LsdMethod};
use crate::decoder::DecodeResult;
use crate::decoder_core::BpCore;
use crate::error::DecodeError;
use crate::gf2::PreparedLinearSystem;
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

#[derive(Debug)]
pub struct BpLsdDecoder {
    pcm: ParityCheckMatrix,
    core: BpCore,
    config: LsdConfig,
    bp_config: DecoderConfig,
    bp_workspace: Mutex<BpWorkspace>,
    fallback_workspace: Mutex<LsdFallbackWorkspace>,
}

impl Clone for BpLsdDecoder {
    fn clone(&self) -> Self {
        Self {
            pcm: self.pcm.clone(),
            core: self.core.clone(),
            config: self.config,
            bp_config: self.bp_config,
            bp_workspace: Mutex::new(self.core.workspace()),
            fallback_workspace: Mutex::new(LsdFallbackWorkspace::new(&self.pcm)),
        }
    }
}

impl BpLsdDecoder {
    pub fn new(
        pcm: ParityCheckMatrix,
        channel: ChannelModel,
        config: LsdConfig,
    ) -> Result<Self, DecodeError> {
        match config.method {
            LsdMethod::LocalizedStatistics => {}
        }
        if config.lsd_order != 0 {
            return Err(DecodeError::UnsupportedLsdOrder {
                order: config.lsd_order,
            });
        }

        let core = BpCore::new(&pcm, &channel)?;
        let bp_workspace = Mutex::new(core.workspace());
        let fallback_workspace = Mutex::new(LsdFallbackWorkspace::new(&pcm));

        Ok(Self {
            pcm,
            core,
            config,
            bp_config: DecoderConfig::default(),
            bp_workspace,
            fallback_workspace,
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
            let mut fallback_workspace = self.fallback_workspace.lock().unwrap();
            fallback_workspace.solve_order_zero(
                &self.pcm,
                syndrome,
                &bp_workspace.hard_decision_bits,
                &bp_workspace.reliability,
            )?
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

#[derive(Debug)]
struct LsdFallbackWorkspace {
    prepared: PreparedLinearSystem,
    column_order: Vec<usize>,
}

impl LsdFallbackWorkspace {
    fn new(pcm: &ParityCheckMatrix) -> Self {
        Self {
            prepared: PreparedLinearSystem::from_pcm(pcm),
            column_order: (0..pcm.num_bits()).collect(),
        }
    }

    fn solve_order_zero(
        &mut self,
        pcm: &ParityCheckMatrix,
        syndrome: &Syndrome,
        base_correction_bits: &[bool],
        reliability: &[f64],
    ) -> Result<Correction, DecodeError> {
        let target_syndrome = xor_syndromes(&multiply_bits(pcm, base_correction_bits), syndrome);
        self.sort_unreliable_columns(reliability);
        let residual = self
            .prepared
            .solve_with_column_order(&target_syndrome, &self.column_order)?;
        Ok(xor_correction_bits(base_correction_bits, &residual))
    }

    fn sort_unreliable_columns(&mut self, reliability: &[f64]) {
        self.column_order.clear();
        self.column_order.extend(0..reliability.len());
        self.column_order.sort_by(|&a, &b| {
            reliability[a]
                .partial_cmp(&reliability[b])
                .unwrap()
                .then_with(|| a.cmp(&b))
        });
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
