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
    let graph = CompiledGraph::from_pcm(pcm);
    let mut workspace = BpWorkspace::new(&graph);
    run_minimum_sum_compiled(&graph, syndrome, prior_llrs, config, &mut workspace)
}

fn recompute_residual_from_hard_decision(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    workspace: &mut BpWorkspace,
) -> usize {
    let mut residual_weight = 0;
    for check in 0..graph.num_checks {
        let start = graph.check_edge_offsets[check];
        let end = graph.check_edge_offsets[check + 1];
        let mut parity = false;
        for edge in start..end {
            parity ^= workspace.hard_decision_bits[graph.edge_bits[edge]];
        }
        let unsatisfied = parity != syndrome.as_slice()[check];
        workspace.unsatisfied_checks[check] = unsatisfied;
        residual_weight += usize::from(unsatisfied);
    }
    workspace.residual_weight = residual_weight;
    residual_weight
}

pub(crate) fn run_minimum_sum_compiled(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    prior_llrs: &[f64],
    config: &DecoderConfig,
    workspace: &mut BpWorkspace,
) -> BpSnapshot {
    workspace.reset(graph, prior_llrs);

    if config.max_bp_iterations == 0 {
        let hard_decision = Correction::from(vec![false; graph.num_bits]);
        return BpSnapshot {
            hard_decision,
            reliability: vec![0.0; graph.num_bits],
            iterations: 0,
            converged: false,
            residual_weight: syndrome.weight(),
        };
    }

    for edge in 0..graph.edge_bits.len() {
        workspace.v_to_c[edge] = prior_llrs[graph.edge_bits[edge]];
    }

    let mut best = None;

    for iteration in 1..=config.max_bp_iterations {
        for check in 0..graph.num_checks {
            let start = graph.check_edge_offsets[check];
            let end = graph.check_edge_offsets[check + 1];
            let syndrome_sign = if syndrome.as_slice()[check] { -1.0 } else { 1.0 };

            for edge in start..end {
                let mut sign = syndrome_sign;
                let mut min_abs = f64::INFINITY;
                for other in start..end {
                    if other == edge {
                        continue;
                    }
                    let msg = workspace.v_to_c[other];
                    if msg < 0.0 {
                        sign = -sign;
                    }
                    min_abs = min_abs.min(msg.abs());
                }
                workspace.c_to_v[edge] = if end - start == 1 {
                    syndrome_sign * CERTAINTY_LLR
                } else {
                    sign * min_abs
                };
            }
        }

        workspace.incoming_llr_sum.fill(0.0);
        for bit in 0..graph.num_bits {
            let start = graph.bit_edge_offsets[bit];
            let end = graph.bit_edge_offsets[bit + 1];
            for slot in start..end {
                let edge = graph.bit_edges[slot];
                workspace.incoming_llr_sum[bit] += workspace.c_to_v[edge];
            }
            workspace.posterior_llr[bit] = prior_llrs[bit] + workspace.incoming_llr_sum[bit];
            workspace.hard_decision_bits[bit] = workspace.posterior_llr[bit] < 0.0;
            workspace.reliability[bit] = workspace.posterior_llr[bit].abs();
        }

        let residual_weight = recompute_residual_from_hard_decision(graph, syndrome, workspace);
        if residual_weight == 0 {
            let snapshot = BpSnapshot {
                hard_decision: Correction::from(workspace.hard_decision_bits.clone()),
                reliability: workspace.reliability.clone(),
                iterations: iteration,
                converged: true,
                residual_weight: 0,
            };
            if config.early_stop {
                return snapshot;
            }
            best = Some(snapshot);
        }

        for bit in 0..graph.num_bits {
            let start = graph.bit_edge_offsets[bit];
            let end = graph.bit_edge_offsets[bit + 1];
            for slot in start..end {
                let edge = graph.bit_edges[slot];
                workspace.v_to_c[edge] = workspace.posterior_llr[bit] - workspace.c_to_v[edge];
            }
        }
    }

    best.unwrap_or_else(|| BpSnapshot {
        hard_decision: Correction::from(workspace.hard_decision_bits.clone()),
        reliability: workspace.reliability.clone(),
        iterations: config.max_bp_iterations,
        converged: workspace.residual_weight == 0,
        residual_weight: workspace.residual_weight,
    })
}

#[cfg(test)]
mod tests {
    use crate::config::DecoderConfig;
    use crate::matrix::ParityCheckMatrix;
    use crate::vector::{Correction, Syndrome};

    use super::{run_minimum_sum_compiled, BpWorkspace, CompiledGraph};

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

    #[test]
    fn compiled_minimum_sum_matches_the_repetition_reference_case() {
        let pcm = ParityCheckMatrix::from_sparse_rows(
            4,
            5,
            vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 4]],
        )
        .unwrap();
        let graph = CompiledGraph::from_pcm(&pcm);
        let mut workspace = BpWorkspace::new(&graph);
        let prior_llrs = vec![((1.0_f64 - 0.05) / 0.05).ln(); 5];

        let snapshot = run_minimum_sum_compiled(
            &graph,
            &Syndrome::from(vec![true, false, false, false]),
            &prior_llrs,
            &DecoderConfig::default(),
            &mut workspace,
        );

        assert!(snapshot.converged);
        assert_eq!(snapshot.residual_weight, 0);
        assert_eq!(
            snapshot.hard_decision,
            Correction::from(vec![true, false, false, false, false])
        );
    }
}
