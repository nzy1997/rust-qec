use std::sync::Mutex;

use crate::bp::BpWorkspace;
use crate::config::{ChannelModel, DecoderConfig};
use crate::decoder_core::BpCore;
use crate::error::DecodeError;
use crate::matrix::ParityCheckMatrix;
use crate::osd::{
    OsdWorkspace, decode_osd_with_workspace, diagnose_osd_candidate_search_with_workspace,
};
use crate::vector::{Correction, Syndrome};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeResult {
    pub correction: Correction,
    pub converged: bool,
    pub bp_iterations: usize,
    pub used_osd: bool,
    pub residual_syndrome_weight: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsdPathDiagnostic {
    pub syndrome_weight: usize,
    pub bp_converged: bool,
    pub bp_iterations: usize,
    pub used_osd: bool,
    pub residual_syndrome_weight: usize,
    pub osd_order: usize,
    pub free_column_count: usize,
    pub candidate_search_frontier_size: usize,
    pub max_candidate_order: usize,
    pub planned_candidate_count: u128,
}

#[derive(Debug)]
pub struct BpOsdDecoder {
    pcm: ParityCheckMatrix,
    core: BpCore,
    config: DecoderConfig,
    bp_workspace: Mutex<BpWorkspace>,
    osd_workspace: Mutex<OsdWorkspace>,
}

impl Clone for BpOsdDecoder {
    fn clone(&self) -> Self {
        Self {
            pcm: self.pcm.clone(),
            core: self.core.clone(),
            config: self.config,
            bp_workspace: Mutex::new(self.core.workspace()),
            osd_workspace: Mutex::new(OsdWorkspace::new(&self.pcm)),
        }
    }
}

impl BpOsdDecoder {
    pub fn new(
        pcm: ParityCheckMatrix,
        channel: ChannelModel,
        config: DecoderConfig,
    ) -> Result<Self, DecodeError> {
        let core = BpCore::new(&pcm, &channel)?;
        let bp_workspace = Mutex::new(core.workspace());
        let osd_workspace = Mutex::new(OsdWorkspace::new(&pcm));
        Ok(Self {
            pcm,
            core,
            config,
            bp_workspace,
            osd_workspace,
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
        let bp_info = self
            .core
            .run_bp_in_place(syndrome, &self.config, &mut bp_workspace);
        if bp_info.residual_weight == 0 {
            return Ok(DecodeResult {
                correction: Correction::from(bp_workspace.hard_decision_bits.clone()),
                converged: bp_info.converged,
                bp_iterations: bp_info.iterations,
                used_osd: false,
                residual_syndrome_weight: bp_info.residual_weight,
            });
        }

        let correction = {
            let mut osd_workspace = self.osd_workspace.lock().unwrap();
            decode_osd_with_workspace(
                &self.pcm,
                syndrome,
                &bp_workspace.hard_decision_bits,
                &bp_workspace.reliability,
                &mut osd_workspace,
                self.config.osd_order,
            )?
        };
        drop(bp_workspace);

        Ok(DecodeResult {
            correction,
            converged: bp_info.converged,
            bp_iterations: bp_info.iterations,
            used_osd: true,
            residual_syndrome_weight: 0,
        })
    }

    pub fn diagnose_osd_path(&self, syndrome: &Syndrome) -> Result<OsdPathDiagnostic, DecodeError> {
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
                return Ok(OsdPathDiagnostic {
                    syndrome_weight: 0,
                    bp_converged: true,
                    bp_iterations: 0,
                    used_osd: false,
                    residual_syndrome_weight: 0,
                    osd_order: self.config.osd_order,
                    free_column_count: 0,
                    candidate_search_frontier_size: 0,
                    max_candidate_order: 0,
                    planned_candidate_count: 0,
                });
            }
        }

        let mut bp_workspace = self.bp_workspace.lock().unwrap();
        let bp_info = self
            .core
            .run_bp_in_place(syndrome, &self.config, &mut bp_workspace);
        if bp_info.residual_weight == 0 {
            return Ok(OsdPathDiagnostic {
                syndrome_weight: syndrome.weight(),
                bp_converged: bp_info.converged,
                bp_iterations: bp_info.iterations,
                used_osd: false,
                residual_syndrome_weight: 0,
                osd_order: self.config.osd_order,
                free_column_count: 0,
                candidate_search_frontier_size: 0,
                max_candidate_order: 0,
                planned_candidate_count: 0,
            });
        }

        let plan = {
            let mut osd_workspace = self.osd_workspace.lock().unwrap();
            diagnose_osd_candidate_search_with_workspace(
                &self.pcm,
                syndrome,
                &bp_workspace.hard_decision_bits,
                &bp_workspace.reliability,
                &mut osd_workspace,
                self.config.osd_order,
            )?
        };

        Ok(OsdPathDiagnostic {
            syndrome_weight: syndrome.weight(),
            bp_converged: bp_info.converged,
            bp_iterations: bp_info.iterations,
            used_osd: true,
            residual_syndrome_weight: bp_info.residual_weight,
            osd_order: self.config.osd_order,
            free_column_count: plan.free_column_count,
            candidate_search_frontier_size: plan.candidate_search_frontier_size,
            max_candidate_order: plan.max_candidate_order,
            planned_candidate_count: plan.planned_candidate_count,
        })
    }
}
