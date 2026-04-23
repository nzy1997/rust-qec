use crate::config::DecoderConfig;
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

const CERTAINTY_LLR: f64 = 1.0e9;

#[derive(Debug, Clone)]
pub(crate) struct BpSnapshot {
    pub(crate) hard_decision: Correction,
    pub(crate) reliability: Vec<f64>,
    pub(crate) iterations: usize,
    pub(crate) converged: bool,
    pub(crate) residual_weight: usize,
}

pub(crate) fn run_minimum_sum(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    prior_llrs: &[f64],
    config: &DecoderConfig,
) -> BpSnapshot {
    let mut v_to_c: Vec<Vec<f64>> = (0..pcm.num_checks())
        .map(|check| {
            pcm.row_neighbors(check)
                .iter()
                .map(|&bit| prior_llrs[bit])
                .collect()
        })
        .collect();
    let mut c_to_v: Vec<Vec<f64>> = (0..pcm.num_checks())
        .map(|check| vec![0.0; pcm.row_neighbors(check).len()])
        .collect();
    let mut posterior_llrs = prior_llrs.to_vec();
    let mut hard_decision = Correction::from(
        posterior_llrs
            .iter()
            .map(|&llr| llr < 0.0)
            .collect::<Vec<_>>(),
    );
    let mut residual_weight = pcm
        .multiply(&hard_decision)
        .as_slice()
        .iter()
        .zip(syndrome.as_slice().iter())
        .filter(|(lhs, rhs)| lhs != rhs)
        .count();

    if config.max_bp_iterations == 0 {
        return BpSnapshot {
            hard_decision,
            reliability: posterior_llrs.iter().map(|value| value.abs()).collect(),
            iterations: 0,
            converged: residual_weight == 0,
            residual_weight,
        };
    }

    for iteration in 1..=config.max_bp_iterations {
        for check in 0..pcm.num_checks() {
            let syndrome_sign = if syndrome.as_slice()[check] {
                -1.0
            } else {
                1.0
            };
            let row_messages = &v_to_c[check];

            for edge_idx in 0..row_messages.len() {
                let message = if row_messages.len() == 1 {
                    syndrome_sign * CERTAINTY_LLR
                } else {
                    let mut sign = syndrome_sign;
                    let mut min_abs = f64::INFINITY;

                    for (other_idx, &other_message) in row_messages.iter().enumerate() {
                        if other_idx == edge_idx {
                            continue;
                        }
                        if other_message < 0.0 {
                            sign = -sign;
                        }
                        min_abs = min_abs.min(other_message.abs());
                    }

                    sign * min_abs
                };
                c_to_v[check][edge_idx] = message;
            }
        }

        let mut incoming = vec![0.0; pcm.num_bits()];
        for check in 0..pcm.num_checks() {
            for (edge_idx, &bit) in pcm.row_neighbors(check).iter().enumerate() {
                incoming[bit] += c_to_v[check][edge_idx];
            }
        }

        posterior_llrs = prior_llrs
            .iter()
            .zip(incoming.iter())
            .map(|(prior, sum)| prior + sum)
            .collect();
        hard_decision = Correction::from(
            posterior_llrs
                .iter()
                .map(|&llr| llr < 0.0)
                .collect::<Vec<_>>(),
        );
        residual_weight = pcm
            .multiply(&hard_decision)
            .as_slice()
            .iter()
            .zip(syndrome.as_slice().iter())
            .filter(|(lhs, rhs)| lhs != rhs)
            .count();

        if residual_weight == 0 {
            let snapshot = BpSnapshot {
                hard_decision: hard_decision.clone(),
                reliability: posterior_llrs.iter().map(|value| value.abs()).collect(),
                iterations: iteration,
                converged: true,
                residual_weight,
            };
            if config.early_stop {
                return snapshot;
            }
        }

        for check in 0..pcm.num_checks() {
            for (edge_idx, &bit) in pcm.row_neighbors(check).iter().enumerate() {
                v_to_c[check][edge_idx] = posterior_llrs[bit] - c_to_v[check][edge_idx];
            }
        }
    }

    BpSnapshot {
        hard_decision,
        reliability: posterior_llrs.iter().map(|value| value.abs()).collect(),
        iterations: config.max_bp_iterations,
        converged: residual_weight == 0,
        residual_weight,
    }
}
