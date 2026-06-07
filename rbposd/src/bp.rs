use crate::config::DecoderConfig;
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

const CERTAINTY_LLR: f64 = 1.0e9;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompiledGraph {
    pub(crate) num_checks: usize,
    pub(crate) num_bits: usize,
    pub(crate) edge_bits: Vec<usize>,
    pub(crate) edge_checks: Vec<usize>,
    pub(crate) check_edge_offsets: Vec<usize>,
    pub(crate) bit_edge_offsets: Vec<usize>,
    pub(crate) bit_edges: Vec<usize>,
}

impl CompiledGraph {
    pub(crate) fn from_pcm(pcm: &ParityCheckMatrix) -> Self {
        let mut edge_bits = Vec::new();
        let mut edge_checks = Vec::new();
        let mut bit_edge_buckets = vec![Vec::new(); pcm.num_bits()];
        let mut check_edge_offsets = Vec::with_capacity(pcm.num_checks() + 1);
        check_edge_offsets.push(0);
        for check in 0..pcm.num_checks() {
            for &bit in pcm.row_neighbors(check) {
                let edge = edge_bits.len();
                edge_bits.push(bit);
                edge_checks.push(check);
                bit_edge_buckets[bit].push(edge);
            }
            check_edge_offsets.push(edge_bits.len());
        }

        let mut bit_edge_offsets = Vec::with_capacity(pcm.num_bits() + 1);
        let mut bit_edges = Vec::with_capacity(edge_bits.len());
        bit_edge_offsets.push(0);
        for bucket in bit_edge_buckets {
            bit_edges.extend(bucket);
            bit_edge_offsets.push(bit_edges.len());
        }

        Self {
            num_checks: pcm.num_checks(),
            num_bits: pcm.num_bits(),
            edge_bits,
            edge_checks,
            check_edge_offsets,
            bit_edge_offsets,
            bit_edges,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BpWorkspace {
    pub(crate) v_to_c: Vec<f64>,
    pub(crate) c_to_v: Vec<f64>,
    pub(crate) posterior_llr: Vec<f64>,
    pub(crate) incoming_llr_sum: Vec<f64>,
    pub(crate) hard_decision_bits: Vec<bool>,
    pub(crate) unsatisfied_checks: Vec<bool>,
    pub(crate) reliability: Vec<f64>,
    pub(crate) residual_weight: usize,
}

impl BpWorkspace {
    pub(crate) fn new(graph: &CompiledGraph) -> Self {
        Self {
            v_to_c: vec![0.0; graph.edge_bits.len()],
            c_to_v: vec![0.0; graph.edge_bits.len()],
            posterior_llr: vec![0.0; graph.num_bits],
            incoming_llr_sum: vec![0.0; graph.num_bits],
            hard_decision_bits: vec![false; graph.num_bits],
            unsatisfied_checks: vec![false; graph.num_checks],
            reliability: vec![0.0; graph.num_bits],
            residual_weight: 0,
        }
    }

    pub(crate) fn reset(&mut self, graph: &CompiledGraph, prior_llrs: &[f64]) {
        assert_eq!(self.v_to_c.len(), graph.edge_bits.len());
        assert_eq!(self.c_to_v.len(), graph.edge_bits.len());
        assert_eq!(self.posterior_llr.len(), graph.num_bits);
        assert_eq!(self.incoming_llr_sum.len(), graph.num_bits);
        assert_eq!(self.hard_decision_bits.len(), graph.num_bits);
        assert_eq!(self.unsatisfied_checks.len(), graph.num_checks);
        assert_eq!(self.reliability.len(), graph.num_bits);
        assert_eq!(prior_llrs.len(), graph.num_bits);

        self.v_to_c.fill(0.0);
        self.c_to_v.fill(0.0);
        self.posterior_llr.copy_from_slice(prior_llrs);
        self.incoming_llr_sum.fill(0.0);
        self.hard_decision_bits.fill(false);
        self.unsatisfied_checks.fill(false);
        self.reliability.fill(0.0);
        self.residual_weight = 0;
    }
}

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
    let mut best = None;
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
            best = Some(snapshot);
        }

        for check in 0..pcm.num_checks() {
            for (edge_idx, &bit) in pcm.row_neighbors(check).iter().enumerate() {
                v_to_c[check][edge_idx] = posterior_llrs[bit] - c_to_v[check][edge_idx];
            }
        }
    }

    best.unwrap_or_else(|| BpSnapshot {
        hard_decision,
        reliability: posterior_llrs.iter().map(|value| value.abs()).collect(),
        iterations: config.max_bp_iterations,
        converged: residual_weight == 0,
        residual_weight,
    })
}

#[cfg(test)]
mod tests {
    use crate::matrix::ParityCheckMatrix;

    use super::{BpWorkspace, CompiledGraph};

    #[test]
    fn compiled_graph_flattens_sparse_rows_into_stable_edge_ranges() {
        let pcm =
            ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 2], vec![1, 2]]).unwrap();

        let graph = CompiledGraph::from_pcm(&pcm);

        assert_eq!(graph.num_checks, 2);
        assert_eq!(graph.num_bits, 3);
        assert_eq!(graph.edge_bits, vec![0, 2, 1, 2]);
        assert_eq!(graph.edge_checks, vec![0, 0, 1, 1]);
        assert_eq!(graph.check_edge_offsets, vec![0, 2, 4]);
        assert_eq!(graph.bit_edge_offsets, vec![0, 1, 2, 4]);
        assert_eq!(graph.bit_edges, vec![0, 2, 1, 3]);
    }

    #[test]
    fn bp_workspace_reset_clears_messages_and_decision_state() {
        let pcm =
            ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 2], vec![1, 2]]).unwrap();
        let graph = CompiledGraph::from_pcm(&pcm);
        let mut workspace = BpWorkspace::new(&graph);

        workspace.v_to_c.fill(9.0);
        workspace.c_to_v.fill(-7.0);
        workspace.posterior_llr.fill(3.0);
        workspace.incoming_llr_sum.fill(4.0);
        workspace.hard_decision_bits.fill(true);
        workspace.unsatisfied_checks.fill(true);
        workspace.reliability.fill(8.0);
        workspace.residual_weight = 5;

        workspace.reset(&graph, &[0.5, 0.25, 0.125]);

        assert_eq!(workspace.v_to_c, vec![0.0; 4]);
        assert_eq!(workspace.c_to_v, vec![0.0; 4]);
        assert_eq!(workspace.posterior_llr, vec![0.5, 0.25, 0.125]);
        assert_eq!(workspace.incoming_llr_sum, vec![0.0; 3]);
        assert_eq!(workspace.hard_decision_bits, vec![false; 3]);
        assert_eq!(workspace.unsatisfied_checks, vec![false; 2]);
        assert_eq!(workspace.reliability, vec![0.0; 3]);
        assert_eq!(workspace.residual_weight, 0);
    }

    #[test]
    #[should_panic]
    fn bp_workspace_reset_rejects_workspace_from_different_graph() {
        let first_pcm =
            ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 2], vec![1, 2]]).unwrap();
        let second_pcm =
            ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap();
        let first_graph = CompiledGraph::from_pcm(&first_pcm);
        let second_graph = CompiledGraph::from_pcm(&second_pcm);
        let mut workspace = BpWorkspace::new(&first_graph);

        workspace.reset(&second_graph, &[0.5, 0.25, 0.125]);
    }
}
