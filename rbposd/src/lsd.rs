use crate::error::DecodeError;
use crate::gf2::{DetailedSolution, PreparedLinearSystem};
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

#[derive(Debug)]
pub(crate) struct LsdWorkspace {
    prepared: PreparedLinearSystem,
    column_order: Vec<usize>,
    local_rows: Vec<Vec<usize>>,
    local_to_global_bits: Vec<usize>,
    local_to_global_checks: Vec<usize>,
    local_reliability: Vec<f64>,
    candidate_bits: Vec<bool>,
}

impl LsdWorkspace {
    pub(crate) fn new(pcm: &ParityCheckMatrix) -> Self {
        Self {
            prepared: PreparedLinearSystem::from_pcm(pcm),
            column_order: (0..pcm.num_bits()).collect(),
            local_rows: Vec::new(),
            local_to_global_bits: Vec::new(),
            local_to_global_checks: Vec::new(),
            local_reliability: Vec::new(),
            candidate_bits: vec![false; pcm.num_bits()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LsdCluster {
    checks: Vec<usize>,
    bits: Vec<usize>,
}

pub(crate) fn decode_lsd_with_workspace(
    pcm: &ParityCheckMatrix,
    target_syndrome: &Syndrome,
    reliability: &[f64],
    lsd_order: usize,
    workspace: &mut LsdWorkspace,
) -> Result<Correction, DecodeError> {
    debug_assert_eq!(target_syndrome.len(), pcm.num_checks());
    debug_assert_eq!(reliability.len(), pcm.num_bits());

    match lsd_order {
        0 => solve_order_zero(pcm, target_syndrome, reliability, workspace),
        1 => solve_order_one(pcm, target_syndrome, reliability, workspace),
        _ => Err(DecodeError::UnsupportedLsdOrder { order: lsd_order }),
    }
}

fn solve_order_zero(
    pcm: &ParityCheckMatrix,
    target_syndrome: &Syndrome,
    reliability: &[f64],
    workspace: &mut LsdWorkspace,
) -> Result<Correction, DecodeError> {
    sort_unreliable_columns(&mut workspace.column_order, reliability);
    workspace
        .prepared
        .solve_with_column_order(target_syndrome, &workspace.column_order)
        .map_err(|_| DecodeError::NoLsdSolution)
        .and_then(|correction| verify_residual(pcm, target_syndrome, correction))
}

fn sort_unreliable_columns(column_order: &mut Vec<usize>, reliability: &[f64]) {
    column_order.clear();
    column_order.extend(0..reliability.len());
    column_order.sort_by(|&a, &b| {
        reliability[a]
            .partial_cmp(&reliability[b])
            .unwrap()
            .then_with(|| a.cmp(&b))
    });
}

fn build_unsatisfied_clusters(
    pcm: &ParityCheckMatrix,
    target_syndrome: &Syndrome,
    reliability: &[f64],
) -> Vec<LsdCluster> {
    let mut visited_checks = vec![false; pcm.num_checks()];
    let mut clusters = Vec::new();
    for check in 0..pcm.num_checks() {
        if !target_syndrome.as_slice()[check] || visited_checks[check] {
            continue;
        }
        clusters.push(build_component_cluster(
            pcm,
            check,
            reliability,
            &mut visited_checks,
        ));
    }
    clusters
}

fn build_component_cluster(
    pcm: &ParityCheckMatrix,
    start_check: usize,
    reliability: &[f64],
    visited_checks: &mut [bool],
) -> LsdCluster {
    let mut checks = vec![start_check];
    let mut bits = Vec::new();
    visited_checks[start_check] = true;

    let mut cursor = 0usize;
    while cursor < checks.len() {
        let check = checks[cursor];
        for &bit in pcm.row_neighbors(check) {
            insert_sorted_by_reliability(&mut bits, bit, reliability);
            for neighbor_check in 0..pcm.num_checks() {
                if !visited_checks[neighbor_check]
                    && pcm.row_neighbors(neighbor_check).contains(&bit)
                {
                    visited_checks[neighbor_check] = true;
                    checks.push(neighbor_check);
                }
            }
        }
        cursor += 1;
    }

    checks.sort_unstable();
    bits.dedup();
    bits.sort_by(|&a, &b| {
        reliability[a]
            .partial_cmp(&reliability[b])
            .unwrap()
            .then_with(|| a.cmp(&b))
    });
    LsdCluster { checks, bits }
}

fn insert_sorted_by_reliability(bits: &mut Vec<usize>, bit: usize, reliability: &[f64]) {
    if bits.contains(&bit) {
        return;
    }
    bits.push(bit);
    bits.sort_by(|&a, &b| {
        reliability[a]
            .partial_cmp(&reliability[b])
            .unwrap()
            .then_with(|| a.cmp(&b))
    });
}

fn solve_order_one(
    pcm: &ParityCheckMatrix,
    target_syndrome: &Syndrome,
    reliability: &[f64],
    workspace: &mut LsdWorkspace,
) -> Result<Correction, DecodeError> {
    let clusters = build_unsatisfied_clusters(pcm, target_syndrome, reliability);
    workspace.candidate_bits.clear();
    workspace.candidate_bits.resize(pcm.num_bits(), false);

    for cluster in clusters {
        let local =
            solve_cluster_order_one(pcm, target_syndrome, reliability, &cluster, workspace)?;
        for (global_bit, bit) in cluster
            .bits
            .iter()
            .copied()
            .zip(local.as_slice().iter().copied())
        {
            if bit {
                workspace.candidate_bits[global_bit] ^= true;
            }
        }
    }

    verify_residual(
        pcm,
        target_syndrome,
        Correction::from(workspace.candidate_bits.clone()),
    )
}

fn solve_cluster_order_one(
    pcm: &ParityCheckMatrix,
    target_syndrome: &Syndrome,
    reliability: &[f64],
    cluster: &LsdCluster,
    workspace: &mut LsdWorkspace,
) -> Result<Correction, DecodeError> {
    build_local_problem(pcm, target_syndrome, reliability, cluster, workspace)?;
    let local_pcm = ParityCheckMatrix::from_sparse_rows(
        workspace.local_to_global_checks.len(),
        workspace.local_to_global_bits.len(),
        workspace.local_rows.clone(),
    )
    .map_err(|_| DecodeError::NoLsdSolution)?;
    let local_syndrome = Syndrome::from(
        workspace
            .local_to_global_checks
            .iter()
            .map(|&check| target_syndrome.as_slice()[check])
            .collect::<Vec<_>>(),
    );
    let mut local_prepared = PreparedLinearSystem::from_pcm(&local_pcm);
    let local_order = (0..workspace.local_to_global_bits.len()).collect::<Vec<_>>();

    let base = local_prepared
        .solve_with_column_order_detailed(&local_syndrome, &local_order, &[])
        .map_err(|_| DecodeError::NoLsdSolution)?;
    let best = best_order_one_candidate(
        &mut local_prepared,
        &local_syndrome,
        &local_order,
        &workspace.local_reliability,
        base,
    );
    Ok(best.correction)
}

fn build_local_problem(
    pcm: &ParityCheckMatrix,
    _target_syndrome: &Syndrome,
    reliability: &[f64],
    cluster: &LsdCluster,
    workspace: &mut LsdWorkspace,
) -> Result<(), DecodeError> {
    workspace.local_to_global_checks.clear();
    workspace
        .local_to_global_checks
        .extend(cluster.checks.iter().copied());
    workspace.local_to_global_bits.clear();
    workspace
        .local_to_global_bits
        .extend(cluster.bits.iter().copied());
    workspace.local_to_global_bits.sort_by(|&a, &b| {
        reliability[a]
            .partial_cmp(&reliability[b])
            .unwrap()
            .then_with(|| a.cmp(&b))
    });
    workspace.local_reliability.clear();
    workspace.local_reliability.extend(
        workspace
            .local_to_global_bits
            .iter()
            .map(|&bit| reliability[bit]),
    );

    workspace.local_rows.clear();
    for &global_check in &workspace.local_to_global_checks {
        let mut local_row = Vec::new();
        for (local_bit, &global_bit) in workspace.local_to_global_bits.iter().enumerate() {
            if pcm.row_neighbors(global_check).contains(&global_bit) {
                local_row.push(local_bit);
            }
        }
        workspace.local_rows.push(local_row);
    }

    Ok(())
}

fn best_order_one_candidate(
    prepared: &mut PreparedLinearSystem,
    syndrome: &Syndrome,
    column_order: &[usize],
    reliability: &[f64],
    base: DetailedSolution,
) -> DetailedSolution {
    let mut best = base;
    let free_columns = best.free_columns.clone();
    for column in free_columns {
        if let Ok(candidate) =
            prepared.solve_with_column_order_detailed(syndrome, column_order, &[column])
        {
            if is_better_candidate(
                candidate.correction.as_slice(),
                best.correction.as_slice(),
                reliability,
            ) {
                best = candidate;
            }
        }
    }
    best
}

fn verify_residual(
    pcm: &ParityCheckMatrix,
    target_syndrome: &Syndrome,
    correction: Correction,
) -> Result<Correction, DecodeError> {
    if pcm.multiply(&correction) == *target_syndrome {
        Ok(correction)
    } else {
        Err(DecodeError::NoLsdSolution)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn correction_cost(bits: &[bool], reliability: &[f64]) -> f64 {
    bits.iter()
        .zip(reliability.iter())
        .filter_map(|(&bit, &cost)| bit.then_some(cost))
        .sum()
}

#[cfg_attr(not(test), allow(dead_code))]
fn is_better_candidate(candidate: &[bool], best: &[bool], reliability: &[f64]) -> bool {
    let candidate_cost = correction_cost(candidate, reliability);
    let best_cost = correction_cost(best, reliability);
    if candidate_cost < best_cost {
        return true;
    }
    if candidate_cost == best_cost {
        return candidate < best;
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::error::DecodeError;
    use crate::matrix::ParityCheckMatrix;
    use crate::vector::{Correction, Syndrome};

    use super::{LsdWorkspace, decode_lsd_with_workspace, is_better_candidate};

    #[test]
    fn order_zero_matches_existing_reliability_ordered_residual_solve() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![1, 2], vec![0]]).unwrap();
        let target_syndrome = Syndrome::from(vec![true, false]);
        let reliability = vec![1.0, 0.2, 0.4];
        let mut workspace = LsdWorkspace::new(&pcm);

        let correction =
            decode_lsd_with_workspace(&pcm, &target_syndrome, &reliability, 0, &mut workspace)
                .unwrap();

        assert_eq!(pcm.multiply(&correction), target_syndrome);
    }

    #[test]
    fn order_zero_maps_unsatisfiable_system_to_lsd_failure() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 1, vec![vec![0], vec![0]]).unwrap();
        let target_syndrome = Syndrome::from(vec![true, false]);
        let reliability = vec![1.0];
        let mut workspace = LsdWorkspace::new(&pcm);

        let error =
            decode_lsd_with_workspace(&pcm, &target_syndrome, &reliability, 0, &mut workspace)
                .unwrap_err();

        assert_eq!(error, DecodeError::NoLsdSolution);
    }

    #[test]
    fn order_one_prefers_forced_free_column_on_component_tie() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0], vec![1, 2]]).unwrap();
        let target_syndrome = Syndrome::from(vec![false, true]);
        let reliability = vec![1_000_000_000.5, 0.0, 0.0];
        let mut workspace = LsdWorkspace::new(&pcm);

        let correction =
            decode_lsd_with_workspace(&pcm, &target_syndrome, &reliability, 1, &mut workspace)
                .unwrap();

        assert_eq!(correction, Correction::from(vec![false, false, true]));
        assert_eq!(pcm.multiply(&correction), target_syndrome);
    }

    #[test]
    fn order_one_reports_lsd_failure_for_inconsistent_component() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 1, vec![vec![0], vec![0]]).unwrap();
        let target_syndrome = Syndrome::from(vec![true, false]);
        let reliability = vec![1_000_000_000.0];
        let mut workspace = LsdWorkspace::new(&pcm);

        let error =
            decode_lsd_with_workspace(&pcm, &target_syndrome, &reliability, 1, &mut workspace)
                .unwrap_err();

        assert_eq!(error, DecodeError::NoLsdSolution);
    }

    #[test]
    fn candidate_tie_break_prefers_lexicographically_smaller_bits() {
        let candidate = vec![false, false, true];
        let best = vec![false, true, false];
        let reliability = vec![1.0, 0.0, 0.0];

        assert!(is_better_candidate(&candidate, &best, &reliability));
    }

    #[test]
    fn candidate_cost_break_prefers_lower_reliability_weight() {
        let candidate = vec![false, false, true];
        let best = vec![false, true, false];
        let reliability = vec![1.0, 0.5, 0.1];

        assert!(is_better_candidate(&candidate, &best, &reliability));
    }

    #[test]
    fn candidate_cost_break_rejects_higher_reliability_weight() {
        let candidate = vec![false, false, true];
        let best = vec![false, true, false];
        let reliability = vec![1.0, 0.1, 0.5];

        assert!(!is_better_candidate(&candidate, &best, &reliability));
    }

    #[test]
    fn candidate_cost_break_does_not_treat_near_equal_costs_as_ties() {
        let candidate = vec![false, false, true];
        let best = vec![false, true, false];
        let slightly_higher_cost = f64::from_bits(0.5f64.to_bits() + 1);
        let reliability = vec![0.0, 0.5, slightly_higher_cost];

        assert!(!is_better_candidate(&candidate, &best, &reliability));
    }

    #[test]
    fn correction_cost_uses_only_set_bits() {
        let bits = Correction::from(vec![true, false, true]);
        let reliability = vec![0.25, 100.0, 0.75];

        assert_eq!(super::correction_cost(bits.as_slice(), &reliability), 1.0);
    }
}
