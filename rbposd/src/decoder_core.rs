use crate::bp::{run_bp_compiled_in_place, BpRunInfo, BpWorkspace, CompiledGraph};
use crate::config::{ChannelModel, DecoderConfig};
use crate::error::DecodeError;
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

#[derive(Debug, Clone)]
pub(crate) struct BpCore {
    graph: CompiledGraph,
    prior_llrs: Vec<f64>,
}

impl BpCore {
    pub(crate) fn new(
        pcm: &ParityCheckMatrix,
        channel: &ChannelModel,
    ) -> Result<Self, DecodeError> {
        Ok(Self {
            graph: CompiledGraph::from_pcm(pcm),
            prior_llrs: compute_prior_llrs(pcm, channel)?,
        })
    }

    pub(crate) fn workspace(&self) -> BpWorkspace {
        BpWorkspace::new(&self.graph)
    }

    pub(crate) fn hard_decision_from_prior(&self) -> Correction {
        prior_hard_decision(&self.prior_llrs)
    }

    pub(crate) fn channel_prior_objective_weights(&self) -> &[f64] {
        &self.prior_llrs
    }

    pub(crate) fn run_bp_in_place(
        &self,
        syndrome: &Syndrome,
        config: &DecoderConfig,
        workspace: &mut BpWorkspace,
    ) -> BpRunInfo {
        run_bp_compiled_in_place(&self.graph, syndrome, &self.prior_llrs, config, workspace)
    }
}

pub(crate) fn compute_prior_llrs(
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

pub(crate) fn prior_hard_decision(prior_llrs: &[f64]) -> Correction {
    Correction::from(prior_llrs.iter().map(|&llr| llr < 0.0).collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use crate::config::ChannelModel;
    use crate::error::DecodeError;
    use crate::matrix::ParityCheckMatrix;
    use crate::vector::Correction;

    use super::{compute_prior_llrs, prior_hard_decision, BpCore};

    #[test]
    fn computes_uniform_prior_llrs_from_bsc() {
        let pcm = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap();

        let llrs = compute_prior_llrs(&pcm, &ChannelModel::Bsc { error_rate: 0.2 }).unwrap();

        let expected = ((1.0_f64 - 0.2) / 0.2).ln();
        assert_eq!(llrs.len(), 3);
        assert!(llrs.iter().all(|value| (*value - expected).abs() < 1.0e-12));
    }

    #[test]
    fn rejects_probability_vector_length_mismatch() {
        let pcm = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap();

        let error = compute_prior_llrs(&pcm, &ChannelModel::BitFlipProbabilities(vec![0.1, 0.2]))
            .unwrap_err();

        assert_eq!(
            error,
            DecodeError::DimensionMismatch {
                what: "channel probabilities",
                expected: 3,
                actual: 2,
            }
        );
    }

    #[test]
    fn prior_hard_decision_uses_negative_llrs() {
        let decision = prior_hard_decision(&[2.0, -1.0, 0.0, -0.5]);

        assert_eq!(decision, Correction::from(vec![false, true, false, true]));
    }

    #[test]
    fn bp_core_builds_workspace_for_its_compiled_graph() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
        let core = BpCore::new(&pcm, &ChannelModel::Bsc { error_rate: 0.05 }).unwrap();

        let workspace = core.workspace();

        assert_eq!(workspace.hard_decision_bits.len(), 3);
        assert_eq!(workspace.unsatisfied_checks.len(), 2);
    }

    #[test]
    fn bp_core_exposes_channel_prior_objective_weights() {
        let pcm = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap();
        let core = BpCore::new(
            &pcm,
            &ChannelModel::BitFlipProbabilities(vec![0.2, 0.3, 0.4]),
        )
        .unwrap();

        let expected = compute_prior_llrs(
            &pcm,
            &ChannelModel::BitFlipProbabilities(vec![0.2, 0.3, 0.4]),
        )
        .unwrap();

        assert_eq!(core.channel_prior_objective_weights(), expected.as_slice());
    }
}
