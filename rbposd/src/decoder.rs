use crate::bp::run_minimum_sum;
use crate::config::{ChannelModel, DecoderConfig};
use crate::error::DecodeError;
use crate::matrix::ParityCheckMatrix;
use crate::osd::decode_osd0;
use crate::vector::{Correction, Syndrome};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeResult {
    pub correction: Correction,
    pub converged: bool,
    pub bp_iterations: usize,
    pub used_osd: bool,
    pub residual_syndrome_weight: usize,
}

#[derive(Debug, Clone)]
pub struct BpOsdDecoder {
    pcm: ParityCheckMatrix,
    config: DecoderConfig,
    prior_llrs: Vec<f64>,
}

impl BpOsdDecoder {
    pub fn new(
        pcm: ParityCheckMatrix,
        channel: ChannelModel,
        config: DecoderConfig,
    ) -> Result<Self, DecodeError> {
        let prior_llrs = compute_prior_llrs(&pcm, &channel)?;
        Ok(Self {
            pcm,
            config,
            prior_llrs,
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
            let prior_correction = prior_hard_decision(&self.prior_llrs);
            if self.pcm.multiply(&prior_correction) == *syndrome {
                return Ok(DecodeResult {
                    correction: prior_correction,
                    converged: true,
                    bp_iterations: 0,
                    used_osd: false,
                    residual_syndrome_weight: 0,
                });
            }

            return Ok(DecodeResult {
                correction: Correction::zero(self.pcm.num_bits()),
                converged: true,
                bp_iterations: 0,
                used_osd: false,
                residual_syndrome_weight: 0,
            });
        }

        let snapshot = run_minimum_sum(&self.pcm, syndrome, &self.prior_llrs, &self.config);
        if snapshot.residual_weight == 0 {
            return Ok(DecodeResult {
                correction: snapshot.hard_decision,
                converged: snapshot.converged,
                bp_iterations: snapshot.iterations,
                used_osd: false,
                residual_syndrome_weight: snapshot.residual_weight,
            });
        }

        let correction = decode_osd0(&self.pcm, syndrome, &snapshot.reliability)?;

        Ok(DecodeResult {
            correction,
            converged: snapshot.converged,
            bp_iterations: snapshot.iterations,
            used_osd: true,
            residual_syndrome_weight: 0,
        })
    }
}

fn compute_prior_llrs(
    pcm: &ParityCheckMatrix,
    channel: &ChannelModel,
) -> Result<Vec<f64>, DecodeError> {
    match channel {
        ChannelModel::Bsc { error_rate } => {
            let probability = validate_probability(*error_rate)?;
            let llr = probability_to_llr(probability);
            Ok(vec![llr; pcm.num_bits()])
        }
        ChannelModel::BitFlipProbabilities(probabilities) => {
            if probabilities.len() != pcm.num_bits() {
                return Err(DecodeError::DimensionMismatch {
                    what: "channel probabilities",
                    expected: pcm.num_bits(),
                    actual: probabilities.len(),
                });
            }

            probabilities
                .iter()
                .map(|&probability| validate_probability(probability).map(probability_to_llr))
                .collect()
        }
    }
}

fn validate_probability(probability: f64) -> Result<f64, DecodeError> {
    if !probability.is_finite() || probability <= 0.0 || probability >= 1.0 {
        return Err(DecodeError::InvalidProbability);
    }
    Ok(probability)
}

fn probability_to_llr(probability: f64) -> f64 {
    ((1.0 - probability) / probability).ln()
}

fn prior_hard_decision(prior_llrs: &[f64]) -> Correction {
    Correction::from(prior_llrs.iter().map(|&llr| llr < 0.0).collect::<Vec<_>>())
}
