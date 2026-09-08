use std::sync::Mutex;
use std::time::Instant;

use crate::bp::BpWorkspace;
use crate::config::{ChannelModel, DecoderConfig, OsdVariant};
use crate::decoder_core::BpCore;
use crate::error::DecodeError;
use crate::matrix::ParityCheckMatrix;
use crate::osd::{
    OsdWorkspace, decode_osd_with_workspace, diagnose_osd_candidate_search_with_workspace,
    effective_osd_variant, profile_osd_with_workspace,
};
use crate::vector::{Correction, Syndrome};

/// Work counters and elapsed times for one decode operation.
#[derive(Debug, Clone, Default)]
pub struct DecodeStats {
    /// Seconds spent in belief propagation.
    pub bp_seconds: f64,
    /// Seconds spent in ordered-statistics post-processing.
    pub osd_seconds: f64,
    /// Number of decode calls represented by these statistics.
    pub decode_call_count: usize,
    /// Total BP iterations.
    pub bp_iteration_count: usize,
    /// Number of times OSD was used.
    pub osd_use_count: usize,
    /// OSD candidates examined.
    pub osd_candidate_count: usize,
    /// GF(2) solve operations.
    pub gf2_solve_count: usize,
    /// Full GF(2) eliminations.
    pub gf2_full_elimination_count: usize,
}

impl DecodeStats {
    fn counters(&self) -> [usize; 6] {
        [
            self.decode_call_count,
            self.bp_iteration_count,
            self.osd_use_count,
            self.osd_candidate_count,
            self.gf2_solve_count,
            self.gf2_full_elimination_count,
        ]
    }
}

impl PartialEq for DecodeStats {
    fn eq(&self, other: &Self) -> bool {
        self.counters() == other.counters()
    }
}

impl Eq for DecodeStats {}

/// Correction and diagnostics returned by a successful decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeResult {
    /// Bit correction satisfying the requested syndrome.
    pub correction: Correction,
    /// Whether BP itself converged.
    pub converged: bool,
    /// BP iterations performed.
    pub bp_iterations: usize,
    /// Whether OSD produced the final correction.
    pub used_osd: bool,
    /// Weight of the residual syndrome after decoding.
    pub residual_syndrome_weight: usize,
    /// Timing and work counters for this call.
    pub stats: DecodeStats,
}

/// Description of the OSD path planned for one syndrome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsdPathDiagnostic {
    /// Weight of the input syndrome.
    pub syndrome_weight: usize,
    /// Whether BP converged.
    pub bp_converged: bool,
    /// BP iterations performed.
    pub bp_iterations: usize,
    /// Whether the plan reaches OSD.
    pub used_osd: bool,
    /// Residual syndrome weight after BP.
    pub residual_syndrome_weight: usize,
    /// Stable planner name.
    pub osd_planner: &'static str,
    /// Configured OSD order.
    pub osd_order: usize,
    /// Number of free columns in the reduced system.
    pub free_column_count: usize,
    /// Number of free columns eligible for multi-column candidates.
    pub candidate_search_frontier_size: usize,
    /// Largest candidate order the planner will visit.
    pub max_candidate_order: usize,
    /// Number of candidates the planner will visit.
    pub planned_candidate_count: u128,
}

/// Reusable belief-propagation decoder with ordered-statistics fallback.
#[derive(Debug)]
pub struct BpOsdDecoder {
    pcm: ParityCheckMatrix,
    core: BpCore,
    config: DecoderConfig,
    bp_workspace: Mutex<BpWorkspace>,
    osd_workspace: Mutex<OsdWorkspace>,
}

fn osd_inputs_for_variant<'a>(
    planner: OsdVariant,
    core: &'a BpCore,
    bp_workspace: &'a BpWorkspace,
    zero_base: &'a [bool],
) -> (&'a [bool], &'a [f64], &'a [f64]) {
    match planner {
        OsdVariant::LdpcCombinationSweep => (
            zero_base,
            &bp_workspace.posterior_llr,
            core.channel_probability_objective_weights(),
        ),
        OsdVariant::Osd0 | OsdVariant::LegacyCombinationSweep => (
            &bp_workspace.hard_decision_bits,
            &bp_workspace.reliability,
            &bp_workspace.reliability,
        ),
    }
}

fn prior_correction_for_planner(core: &BpCore, planner: OsdVariant) -> Correction {
    if planner == OsdVariant::LdpcCombinationSweep {
        core.hard_decision_from_prior_with_ties_as_errors()
    } else {
        core.hard_decision_from_prior()
    }
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
    /// Construct a decoder for a matrix, channel, and algorithm configuration.
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

    /// Decode one syndrome, reusing internal workspaces.
    pub fn decode(&self, syndrome: &Syndrome) -> Result<DecodeResult, DecodeError> {
        let effective_planner = effective_osd_variant(self.config);
        if syndrome.len() != self.pcm.num_checks() {
            return Err(DecodeError::DimensionMismatch {
                what: "syndrome",
                expected: self.pcm.num_checks(),
                actual: syndrome.len(),
            });
        }

        if syndrome.weight() == 0 {
            let prior_correction = prior_correction_for_planner(&self.core, effective_planner);
            if self.pcm.multiply(&prior_correction) == *syndrome {
                return Ok(DecodeResult {
                    correction: prior_correction,
                    converged: true,
                    bp_iterations: 0,
                    used_osd: false,
                    residual_syndrome_weight: 0,
                    stats: DecodeStats {
                        decode_call_count: 1,
                        ..DecodeStats::default()
                    },
                });
            }
        }

        let bp_start = Instant::now();
        let mut bp_workspace = self.bp_workspace.lock().unwrap();
        let bp_info = self
            .core
            .run_bp_in_place(syndrome, &self.config, &mut bp_workspace);
        let bp_seconds = bp_start.elapsed().as_secs_f64();
        if bp_info.residual_weight == 0 {
            return Ok(DecodeResult {
                correction: Correction::from(bp_workspace.hard_decision_bits.clone()),
                converged: bp_info.converged,
                bp_iterations: bp_info.iterations,
                used_osd: false,
                residual_syndrome_weight: bp_info.residual_weight,
                stats: DecodeStats {
                    bp_seconds,
                    decode_call_count: 1,
                    bp_iteration_count: bp_info.iterations,
                    ..DecodeStats::default()
                },
            });
        }
        let osd_start = Instant::now();
        let osd_outcome = {
            let mut osd_workspace = self.osd_workspace.lock().unwrap();
            let zero_base = vec![false; self.pcm.num_bits()];
            let (base_correction_bits, ordering_reliability, objective_weights) =
                osd_inputs_for_variant(effective_planner, &self.core, &bp_workspace, &zero_base);
            decode_osd_with_workspace(
                &self.pcm,
                syndrome,
                base_correction_bits,
                ordering_reliability,
                objective_weights,
                &mut osd_workspace,
                effective_planner,
                self.config.osd_order,
            )?
        };
        let osd_seconds = osd_start.elapsed().as_secs_f64();
        drop(bp_workspace);

        Ok(DecodeResult {
            correction: osd_outcome.correction,
            converged: bp_info.converged,
            bp_iterations: bp_info.iterations,
            used_osd: true,
            residual_syndrome_weight: 0,
            stats: DecodeStats {
                bp_seconds,
                osd_seconds,
                decode_call_count: 1,
                bp_iteration_count: bp_info.iterations,
                osd_use_count: 1,
                osd_candidate_count: osd_outcome.stats.osd_candidate_count,
                gf2_solve_count: osd_outcome.stats.gf2_solve_count,
                gf2_full_elimination_count: osd_outcome.stats.gf2_full_elimination_count,
            },
        })
    }

    /// Plan and report the OSD path without returning a correction.
    pub fn diagnose_osd_path(&self, syndrome: &Syndrome) -> Result<OsdPathDiagnostic, DecodeError> {
        let effective_planner = effective_osd_variant(self.config);
        if syndrome.len() != self.pcm.num_checks() {
            return Err(DecodeError::DimensionMismatch {
                what: "syndrome",
                expected: self.pcm.num_checks(),
                actual: syndrome.len(),
            });
        }

        if syndrome.weight() == 0 {
            let prior_correction = prior_correction_for_planner(&self.core, effective_planner);
            if self.pcm.multiply(&prior_correction) == *syndrome {
                return Ok(OsdPathDiagnostic {
                    syndrome_weight: 0,
                    bp_converged: true,
                    bp_iterations: 0,
                    used_osd: false,
                    residual_syndrome_weight: 0,
                    osd_planner: effective_planner.planner_name(),
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
                osd_planner: effective_planner.planner_name(),
                osd_order: self.config.osd_order,
                free_column_count: 0,
                candidate_search_frontier_size: 0,
                max_candidate_order: 0,
                planned_candidate_count: 0,
            });
        }

        let plan = {
            let mut osd_workspace = self.osd_workspace.lock().unwrap();
            let zero_base = vec![false; self.pcm.num_bits()];
            let (base_correction_bits, ordering_reliability, _) =
                osd_inputs_for_variant(effective_planner, &self.core, &bp_workspace, &zero_base);
            diagnose_osd_candidate_search_with_workspace(
                &self.pcm,
                syndrome,
                base_correction_bits,
                ordering_reliability,
                &mut osd_workspace,
                effective_planner,
                self.config.osd_order,
            )?
        };

        Ok(OsdPathDiagnostic {
            syndrome_weight: syndrome.weight(),
            bp_converged: bp_info.converged,
            bp_iterations: bp_info.iterations,
            used_osd: true,
            residual_syndrome_weight: bp_info.residual_weight,
            osd_planner: effective_planner.planner_name(),
            osd_order: self.config.osd_order,
            free_column_count: plan.free_column_count,
            candidate_search_frontier_size: plan.candidate_search_frontier_size,
            max_candidate_order: plan.max_candidate_order,
            planned_candidate_count: plan.planned_candidate_count,
        })
    }

    /// Profile a decode while visiting at most `osd_candidate_limit` candidates.
    ///
    /// This diagnostic method returns work statistics rather than a correction.
    pub fn profile_decode_with_osd_candidate_limit(
        &self,
        syndrome: &Syndrome,
        osd_candidate_limit: usize,
    ) -> Result<DecodeStats, DecodeError> {
        let effective_planner = effective_osd_variant(self.config);
        if syndrome.len() != self.pcm.num_checks() {
            return Err(DecodeError::DimensionMismatch {
                what: "syndrome",
                expected: self.pcm.num_checks(),
                actual: syndrome.len(),
            });
        }

        if syndrome.weight() == 0 {
            let prior_correction = prior_correction_for_planner(&self.core, effective_planner);
            if self.pcm.multiply(&prior_correction) == *syndrome {
                return Ok(DecodeStats {
                    decode_call_count: 1,
                    ..DecodeStats::default()
                });
            }
        }

        let bp_start = Instant::now();
        let mut bp_workspace = self.bp_workspace.lock().unwrap();
        let bp_info = self
            .core
            .run_bp_in_place(syndrome, &self.config, &mut bp_workspace);
        let bp_seconds = bp_start.elapsed().as_secs_f64();
        if bp_info.residual_weight == 0 {
            return Ok(DecodeStats {
                bp_seconds,
                decode_call_count: 1,
                bp_iteration_count: bp_info.iterations,
                ..DecodeStats::default()
            });
        }

        let osd_start = Instant::now();
        let osd_stats = {
            let mut osd_workspace = self.osd_workspace.lock().unwrap();
            let zero_base = vec![false; self.pcm.num_bits()];
            let (base_correction_bits, ordering_reliability, _) =
                osd_inputs_for_variant(effective_planner, &self.core, &bp_workspace, &zero_base);
            profile_osd_with_workspace(
                &self.pcm,
                syndrome,
                base_correction_bits,
                ordering_reliability,
                &mut osd_workspace,
                effective_planner,
                self.config.osd_order,
                osd_candidate_limit,
            )?
        };
        let osd_seconds = osd_start.elapsed().as_secs_f64();
        drop(bp_workspace);

        Ok(DecodeStats {
            bp_seconds,
            osd_seconds,
            decode_call_count: 1,
            bp_iteration_count: bp_info.iterations,
            osd_use_count: 1,
            osd_candidate_count: osd_stats.osd_candidate_count,
            gf2_solve_count: osd_stats.gf2_solve_count,
            gf2_full_elimination_count: osd_stats.gf2_full_elimination_count,
        })
    }
}
