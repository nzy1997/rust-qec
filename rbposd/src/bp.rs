use crate::config::{BpVariant, DecoderConfig, Schedule};
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

const CERTAINTY_LLR: f64 = 1.0e9;
const TANH_EPSILON: f64 = 1.0e-12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckUpdateRule {
    MinimumSum,
    ProductSum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BpSchedule {
    Parallel,
    Serial,
}

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
    pub(crate) best_hard_decision_bits: Vec<bool>,
    pub(crate) unsatisfied_checks: Vec<bool>,
    pub(crate) reliability: Vec<f64>,
    pub(crate) best_reliability: Vec<f64>,
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
            best_hard_decision_bits: vec![false; graph.num_bits],
            unsatisfied_checks: vec![false; graph.num_checks],
            reliability: vec![0.0; graph.num_bits],
            best_reliability: vec![0.0; graph.num_bits],
            residual_weight: 0,
        }
    }

    pub(crate) fn reset(&mut self, graph: &CompiledGraph, prior_llrs: &[f64]) {
        assert_eq!(self.v_to_c.len(), graph.edge_bits.len());
        assert_eq!(self.c_to_v.len(), graph.edge_bits.len());
        assert_eq!(self.posterior_llr.len(), graph.num_bits);
        assert_eq!(self.incoming_llr_sum.len(), graph.num_bits);
        assert_eq!(self.hard_decision_bits.len(), graph.num_bits);
        assert_eq!(self.best_hard_decision_bits.len(), graph.num_bits);
        assert_eq!(self.unsatisfied_checks.len(), graph.num_checks);
        assert_eq!(self.reliability.len(), graph.num_bits);
        assert_eq!(self.best_reliability.len(), graph.num_bits);
        assert_eq!(prior_llrs.len(), graph.num_bits);

        self.v_to_c.fill(0.0);
        self.c_to_v.fill(0.0);
        self.posterior_llr.copy_from_slice(prior_llrs);
        self.incoming_llr_sum.fill(0.0);
        self.hard_decision_bits.fill(false);
        self.best_hard_decision_bits.fill(false);
        self.unsatisfied_checks.fill(false);
        self.reliability.fill(0.0);
        self.best_reliability.fill(0.0);
        self.residual_weight = 0;
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct BpSnapshot {
    pub(crate) hard_decision: Correction,
    pub(crate) reliability: Vec<f64>,
    pub(crate) iterations: usize,
    pub(crate) converged: bool,
    pub(crate) residual_weight: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BpRunInfo {
    pub(crate) iterations: usize,
    pub(crate) converged: bool,
    pub(crate) residual_weight: usize,
}

#[allow(dead_code)]
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
    workspace.unsatisfied_checks.fill(false);

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

#[allow(dead_code)]
fn update_check_to_variable_messages(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    workspace: &mut BpWorkspace,
) {
    for check in 0..graph.num_checks {
        update_minimum_sum_check_to_variable_messages_for_check(graph, syndrome, workspace, check);
    }
}

fn update_minimum_sum_check_to_variable_messages_for_check(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    workspace: &mut BpWorkspace,
    check: usize,
) {
    let start = graph.check_edge_offsets[check];
    let end = graph.check_edge_offsets[check + 1];
    let syndrome_sign = if syndrome.as_slice()[check] {
        -1.0
    } else {
        1.0
    };

    if end - start == 1 {
        workspace.c_to_v[start] = syndrome_sign * CERTAINTY_LLR;
        return;
    }

    let mut total_sign = syndrome_sign;
    let mut min_abs = f64::INFINITY;
    let mut second_min_abs = f64::INFINITY;
    let mut min_count = 0usize;

    for edge in start..end {
        let msg = workspace.v_to_c[edge];
        if msg < 0.0 {
            total_sign = -total_sign;
        }
        let abs = msg.abs();
        if abs < min_abs {
            second_min_abs = min_abs;
            min_abs = abs;
            min_count = 1;
        } else if abs == min_abs {
            min_count += 1;
        } else if abs < second_min_abs {
            second_min_abs = abs;
        }
    }

    for edge in start..end {
        let msg = workspace.v_to_c[edge];
        let sign = if msg < 0.0 { -total_sign } else { total_sign };
        let abs = msg.abs();
        let excluded_min_abs = if abs == min_abs && min_count == 1 {
            second_min_abs
        } else {
            min_abs
        };
        workspace.c_to_v[edge] = sign * excluded_min_abs;
    }
}

fn clamp_tanh_product(value: f64) -> f64 {
    value.clamp(-1.0 + TANH_EPSILON, 1.0 - TANH_EPSILON)
}

fn product_sum_message_from_extrinsic(extrinsic_product: f64) -> f64 {
    2.0 * clamp_tanh_product(extrinsic_product).atanh()
}

fn update_product_sum_check_to_variable_messages_for_check(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    workspace: &mut BpWorkspace,
    check: usize,
) {
    let start = graph.check_edge_offsets[check];
    let end = graph.check_edge_offsets[check + 1];
    let syndrome_sign = if syndrome.as_slice()[check] {
        -1.0
    } else {
        1.0
    };

    if end - start == 1 {
        workspace.c_to_v[start] = syndrome_sign * CERTAINTY_LLR;
        return;
    }

    for target_edge in start..end {
        let mut product = syndrome_sign;
        for edge in start..end {
            if edge != target_edge {
                product *= (workspace.v_to_c[edge] / 2.0).tanh();
            }
        }
        workspace.c_to_v[target_edge] = product_sum_message_from_extrinsic(product);
    }
}

fn update_check_to_variable_messages_for_check(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    workspace: &mut BpWorkspace,
    check: usize,
    rule: CheckUpdateRule,
) {
    match rule {
        CheckUpdateRule::MinimumSum => {
            update_minimum_sum_check_to_variable_messages_for_check(
                graph, syndrome, workspace, check,
            );
        }
        CheckUpdateRule::ProductSum => {
            update_product_sum_check_to_variable_messages_for_check(
                graph, syndrome, workspace, check,
            );
        }
    }
}

fn update_check_to_variable_messages_with_rule(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    workspace: &mut BpWorkspace,
    rule: CheckUpdateRule,
) {
    for check in 0..graph.num_checks {
        update_check_to_variable_messages_for_check(graph, syndrome, workspace, check, rule);
    }
}

fn refresh_bit_posterior_from_messages(
    graph: &CompiledGraph,
    prior_llrs: &[f64],
    workspace: &mut BpWorkspace,
    bit: usize,
) {
    let start = graph.bit_edge_offsets[bit];
    let end = graph.bit_edge_offsets[bit + 1];
    let mut incoming_sum = 0.0;
    for slot in start..end {
        let edge = graph.bit_edges[slot];
        incoming_sum += workspace.c_to_v[edge];
    }
    workspace.incoming_llr_sum[bit] = incoming_sum;
    workspace.posterior_llr[bit] = prior_llrs[bit] + incoming_sum;
    workspace.hard_decision_bits[bit] = workspace.posterior_llr[bit] < 0.0;
    workspace.reliability[bit] = workspace.posterior_llr[bit].abs();
}

fn refresh_all_bit_posteriors(
    graph: &CompiledGraph,
    prior_llrs: &[f64],
    workspace: &mut BpWorkspace,
) {
    for bit in 0..graph.num_bits {
        refresh_bit_posterior_from_messages(graph, prior_llrs, workspace, bit);
    }
}

fn refresh_variable_to_check_messages_for_bit(
    graph: &CompiledGraph,
    workspace: &mut BpWorkspace,
    bit: usize,
) {
    let start = graph.bit_edge_offsets[bit];
    let end = graph.bit_edge_offsets[bit + 1];
    for slot in start..end {
        let edge = graph.bit_edges[slot];
        workspace.v_to_c[edge] = workspace.posterior_llr[bit] - workspace.c_to_v[edge];
    }
}

fn refresh_all_variable_to_check_messages(graph: &CompiledGraph, workspace: &mut BpWorkspace) {
    for bit in 0..graph.num_bits {
        refresh_variable_to_check_messages_for_bit(graph, workspace, bit);
    }
}

fn initialize_variable_to_check_messages(
    graph: &CompiledGraph,
    prior_llrs: &[f64],
    workspace: &mut BpWorkspace,
) {
    for edge in 0..graph.edge_bits.len() {
        workspace.v_to_c[edge] = prior_llrs[graph.edge_bits[edge]];
    }
}

fn zero_iteration_snapshot(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    prior_llrs: &[f64],
    workspace: &mut BpWorkspace,
) -> BpRunInfo {
    for bit in 0..graph.num_bits {
        workspace.hard_decision_bits[bit] = prior_llrs[bit] < 0.0;
        workspace.reliability[bit] = prior_llrs[bit].abs();
    }
    let residual_weight = recompute_residual_from_hard_decision(graph, syndrome, workspace);
    BpRunInfo {
        iterations: 0,
        converged: residual_weight == 0,
        residual_weight,
    }
}

fn remember_converged_snapshot(workspace: &mut BpWorkspace) {
    workspace
        .best_hard_decision_bits
        .copy_from_slice(&workspace.hard_decision_bits);
    workspace
        .best_reliability
        .copy_from_slice(&workspace.reliability);
}

fn restore_converged_snapshot(workspace: &mut BpWorkspace) {
    workspace
        .hard_decision_bits
        .copy_from_slice(&workspace.best_hard_decision_bits);
    workspace
        .reliability
        .copy_from_slice(&workspace.best_reliability);
    workspace.residual_weight = 0;
}

pub(crate) fn run_minimum_sum_compiled(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    prior_llrs: &[f64],
    config: &DecoderConfig,
    workspace: &mut BpWorkspace,
) -> BpSnapshot {
    let info = run_minimum_sum_compiled_in_place(graph, syndrome, prior_llrs, config, workspace);
    BpSnapshot {
        hard_decision: Correction::from(workspace.hard_decision_bits.clone()),
        reliability: workspace.reliability.clone(),
        iterations: info.iterations,
        converged: info.converged,
        residual_weight: info.residual_weight,
    }
}

pub(crate) fn run_bp_compiled_in_place(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    prior_llrs: &[f64],
    config: &DecoderConfig,
    workspace: &mut BpWorkspace,
) -> BpRunInfo {
    let rule = match config.bp_variant {
        BpVariant::MinimumSum => CheckUpdateRule::MinimumSum,
        BpVariant::ProductSum => CheckUpdateRule::ProductSum,
    };
    let schedule = match config.schedule {
        Schedule::Parallel => BpSchedule::Parallel,
        Schedule::Serial => BpSchedule::Serial,
    };
    run_bp_selected_in_place(
        graph, syndrome, prior_llrs, config, workspace, rule, schedule,
    )
}

pub(crate) fn run_minimum_sum_compiled_in_place(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    prior_llrs: &[f64],
    config: &DecoderConfig,
    workspace: &mut BpWorkspace,
) -> BpRunInfo {
    run_bp_selected_in_place(
        graph,
        syndrome,
        prior_llrs,
        config,
        workspace,
        CheckUpdateRule::MinimumSum,
        BpSchedule::Parallel,
    )
}

fn run_bp_selected_in_place(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    prior_llrs: &[f64],
    config: &DecoderConfig,
    workspace: &mut BpWorkspace,
    rule: CheckUpdateRule,
    schedule: BpSchedule,
) -> BpRunInfo {
    workspace.reset(graph, prior_llrs);

    if config.max_bp_iterations == 0 {
        return zero_iteration_snapshot(graph, syndrome, prior_llrs, workspace);
    }

    initialize_variable_to_check_messages(graph, prior_llrs, workspace);
    refresh_all_bit_posteriors(graph, prior_llrs, workspace);

    match schedule {
        BpSchedule::Parallel => {
            run_bp_parallel_in_place(graph, syndrome, prior_llrs, config, workspace, rule)
        }
        BpSchedule::Serial => {
            run_bp_serial_in_place(graph, syndrome, prior_llrs, config, workspace, rule)
        }
    }
}

fn run_bp_parallel_in_place(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    prior_llrs: &[f64],
    config: &DecoderConfig,
    workspace: &mut BpWorkspace,
    rule: CheckUpdateRule,
) -> BpRunInfo {
    let mut best_info = None;

    for iteration in 1..=config.max_bp_iterations {
        update_check_to_variable_messages_with_rule(graph, syndrome, workspace, rule);
        refresh_all_bit_posteriors(graph, prior_llrs, workspace);

        let residual_weight = recompute_residual_from_hard_decision(graph, syndrome, workspace);
        if residual_weight == 0 {
            let info = BpRunInfo {
                iterations: iteration,
                converged: true,
                residual_weight: 0,
            };
            if config.early_stop {
                return info;
            }
            remember_converged_snapshot(workspace);
            best_info = Some(info);
        }

        refresh_all_variable_to_check_messages(graph, workspace);
    }

    if let Some(info) = best_info {
        restore_converged_snapshot(workspace);
        return info;
    }

    BpRunInfo {
        iterations: config.max_bp_iterations,
        converged: workspace.residual_weight == 0,
        residual_weight: workspace.residual_weight,
    }
}

fn run_bp_serial_in_place(
    graph: &CompiledGraph,
    syndrome: &Syndrome,
    prior_llrs: &[f64],
    config: &DecoderConfig,
    workspace: &mut BpWorkspace,
    rule: CheckUpdateRule,
) -> BpRunInfo {
    let mut best_info = None;

    for iteration in 1..=config.max_bp_iterations {
        for check in 0..graph.num_checks {
            update_check_to_variable_messages_for_check(graph, syndrome, workspace, check, rule);
            let start = graph.check_edge_offsets[check];
            let end = graph.check_edge_offsets[check + 1];
            for edge in start..end {
                let bit = graph.edge_bits[edge];
                refresh_bit_posterior_from_messages(graph, prior_llrs, workspace, bit);
                refresh_variable_to_check_messages_for_bit(graph, workspace, bit);
            }
        }

        let residual_weight = recompute_residual_from_hard_decision(graph, syndrome, workspace);
        if residual_weight == 0 {
            let info = BpRunInfo {
                iterations: iteration,
                converged: true,
                residual_weight: 0,
            };
            if config.early_stop {
                return info;
            }
            remember_converged_snapshot(workspace);
            best_info = Some(info);
        }
    }

    if let Some(info) = best_info {
        restore_converged_snapshot(workspace);
        return info;
    }

    BpRunInfo {
        iterations: config.max_bp_iterations,
        converged: workspace.residual_weight == 0,
        residual_weight: workspace.residual_weight,
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{BpVariant, DecoderConfig, Schedule};
    use crate::matrix::ParityCheckMatrix;
    use crate::vector::{Correction, Syndrome};

    use super::{
        recompute_residual_from_hard_decision, run_bp_compiled_in_place, run_bp_selected_in_place,
        run_minimum_sum_compiled, run_minimum_sum_compiled_in_place,
        update_check_to_variable_messages, update_check_to_variable_messages_with_rule, BpSchedule,
        BpWorkspace, CheckUpdateRule, CompiledGraph,
    };

    #[test]
    fn compiled_graph_flattens_sparse_rows_into_stable_edge_ranges() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 2], vec![1, 2]]).unwrap();

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
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 2], vec![1, 2]]).unwrap();
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
        let second_pcm = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap();
        let first_graph = CompiledGraph::from_pcm(&first_pcm);
        let second_graph = CompiledGraph::from_pcm(&second_pcm);
        let mut workspace = BpWorkspace::new(&first_graph);

        workspace.reset(&second_graph, &[0.5, 0.25, 0.125]);
    }

    #[test]
    fn residual_tracker_matches_pcm_multiply_for_manual_hard_decision() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 2], vec![1, 2]]).unwrap();
        let graph = CompiledGraph::from_pcm(&pcm);
        let syndrome = Syndrome::from(vec![false, false]);
        let mut workspace = BpWorkspace::new(&graph);
        workspace.unsatisfied_checks.fill(true);
        workspace.hard_decision_bits = vec![true, false, true];

        let residual_weight =
            recompute_residual_from_hard_decision(&graph, &syndrome, &mut workspace);

        assert_eq!(
            pcm.multiply(&Correction::from(workspace.hard_decision_bits.clone())),
            Syndrome::from(vec![false, true])
        );
        assert_eq!(residual_weight, 1);
        assert_eq!(workspace.unsatisfied_checks, vec![false, true]);
        assert_eq!(workspace.residual_weight, 1);
    }

    #[test]
    fn check_update_uses_second_minimum_when_unique_smallest_edge_is_excluded() {
        let pcm = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap();
        let graph = CompiledGraph::from_pcm(&pcm);
        let mut workspace = BpWorkspace::new(&graph);
        workspace.v_to_c = vec![0.1, 2.0, -3.0];

        update_check_to_variable_messages(&graph, &Syndrome::from(vec![false]), &mut workspace);

        assert_eq!(workspace.c_to_v, vec![-2.0, -0.1, 0.1]);
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

    #[test]
    fn compiled_minimum_sum_in_place_leaves_snapshot_state_in_workspace() {
        let pcm = ParityCheckMatrix::from_sparse_rows(
            4,
            5,
            vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 4]],
        )
        .unwrap();
        let graph = CompiledGraph::from_pcm(&pcm);
        let mut workspace = BpWorkspace::new(&graph);
        let prior_llrs = vec![((1.0_f64 - 0.05) / 0.05).ln(); 5];

        let info = run_minimum_sum_compiled_in_place(
            &graph,
            &Syndrome::from(vec![true, false, false, false]),
            &prior_llrs,
            &DecoderConfig::default(),
            &mut workspace,
        );

        assert!(info.converged);
        assert_eq!(info.residual_weight, 0);
        assert_eq!(
            workspace.hard_decision_bits,
            vec![true, false, false, false, false]
        );
        assert!(workspace.reliability.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn selector_dispatch_runs_product_sum_serial_path() {
        let pcm = ParityCheckMatrix::from_sparse_rows(
            4,
            5,
            vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 4]],
        )
        .unwrap();
        let graph = CompiledGraph::from_pcm(&pcm);
        let mut workspace = BpWorkspace::new(&graph);
        let prior_llrs = vec![((1.0_f64 - 0.05) / 0.05).ln(); 5];
        let config = DecoderConfig {
            bp_variant: BpVariant::ProductSum,
            schedule: Schedule::Serial,
            ..DecoderConfig::default()
        };

        let info = run_bp_compiled_in_place(
            &graph,
            &Syndrome::from(vec![true, false, false, false]),
            &prior_llrs,
            &config,
            &mut workspace,
        );

        assert!(info.converged);
        assert_eq!(info.residual_weight, 0);
        assert_eq!(
            workspace.hard_decision_bits,
            vec![true, false, false, false, false]
        );
    }

    #[test]
    fn compiled_minimum_sum_zero_iterations_preserves_prior_snapshot() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
        let graph = CompiledGraph::from_pcm(&pcm);
        let mut workspace = BpWorkspace::new(&graph);
        let config = DecoderConfig {
            max_bp_iterations: 0,
            ..DecoderConfig::default()
        };
        let prior_llrs = vec![2.0, 1.0, -3.0];

        let snapshot = run_minimum_sum_compiled(
            &graph,
            &Syndrome::from(vec![true, false]),
            &prior_llrs,
            &config,
            &mut workspace,
        );

        assert_eq!(
            snapshot.hard_decision,
            Correction::from(vec![false, false, true])
        );
        assert_eq!(snapshot.reliability, vec![2.0, 1.0, 3.0]);
        assert_eq!(snapshot.iterations, 0);
        assert!(!snapshot.converged);
        assert_eq!(snapshot.residual_weight, 2);
    }

    #[test]
    fn product_sum_check_update_differs_from_minimum_sum_for_degree_three_check() {
        let pcm = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap();
        let graph = CompiledGraph::from_pcm(&pcm);
        let syndrome = Syndrome::from(vec![false]);
        let mut minimum_workspace = BpWorkspace::new(&graph);
        let mut product_workspace = BpWorkspace::new(&graph);
        minimum_workspace.v_to_c = vec![0.8, -1.2, 1.6];
        product_workspace.v_to_c = minimum_workspace.v_to_c.clone();

        update_check_to_variable_messages_with_rule(
            &graph,
            &syndrome,
            &mut minimum_workspace,
            CheckUpdateRule::MinimumSum,
        );
        update_check_to_variable_messages_with_rule(
            &graph,
            &syndrome,
            &mut product_workspace,
            CheckUpdateRule::ProductSum,
        );

        assert_ne!(minimum_workspace.c_to_v, product_workspace.c_to_v);
        assert!(product_workspace
            .c_to_v
            .iter()
            .all(|value| value.is_finite()));
    }

    #[test]
    fn serial_schedule_updates_messages_differently_from_parallel_schedule() {
        let pcm =
            ParityCheckMatrix::from_sparse_rows(3, 4, vec![vec![0, 1], vec![1, 2], vec![2, 3]])
                .unwrap();
        let graph = CompiledGraph::from_pcm(&pcm);
        let syndrome = Syndrome::from(vec![true, false, true]);
        let prior_llrs = vec![
            ((1.0_f64 - 0.2) / 0.2).ln(),
            ((1.0_f64 - 0.35) / 0.35).ln(),
            ((1.0_f64 - 0.2) / 0.2).ln(),
            ((1.0_f64 - 0.2) / 0.2).ln(),
        ];
        let config = DecoderConfig {
            max_bp_iterations: 1,
            early_stop: false,
            bp_variant: BpVariant::ProductSum,
            schedule: Schedule::Parallel,
            ..DecoderConfig::default()
        };
        let mut parallel_workspace = BpWorkspace::new(&graph);
        let mut serial_workspace = BpWorkspace::new(&graph);
        run_bp_selected_in_place(
            &graph,
            &syndrome,
            &prior_llrs,
            &config,
            &mut parallel_workspace,
            CheckUpdateRule::ProductSum,
            BpSchedule::Parallel,
        );
        run_bp_selected_in_place(
            &graph,
            &syndrome,
            &prior_llrs,
            &config,
            &mut serial_workspace,
            CheckUpdateRule::ProductSum,
            BpSchedule::Serial,
        );

        assert!(parallel_workspace.reliability[3] < 1e-12);
        assert!(serial_workspace.reliability[3] > 0.5);
        assert_ne!(
            parallel_workspace.hard_decision_bits,
            serial_workspace.hard_decision_bits
        );
    }

    #[test]
    fn serial_schedule_preserves_isolated_bit_prior_snapshot() {
        let pcm = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1]]).unwrap();
        let graph = CompiledGraph::from_pcm(&pcm);
        let syndrome = Syndrome::from(vec![false]);
        let prior_llrs = vec![2.0, 2.0, -4.0];
        let config = DecoderConfig {
            max_bp_iterations: 1,
            early_stop: false,
            bp_variant: BpVariant::ProductSum,
            schedule: Schedule::Serial,
            ..DecoderConfig::default()
        };
        let mut workspace = BpWorkspace::new(&graph);

        run_bp_compiled_in_place(&graph, &syndrome, &prior_llrs, &config, &mut workspace);

        assert!(workspace.hard_decision_bits[2]);
        assert_eq!(workspace.posterior_llr[2], -4.0);
        assert_eq!(workspace.reliability[2], 4.0);
    }
}
