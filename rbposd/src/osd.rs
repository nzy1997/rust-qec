use crate::config::{DecoderConfig, OsdVariant};
use crate::error::DecodeError;
use crate::gf2::{DetailedSolution, Gf2SolveStats, PreparedLinearSystem, ReducedLinearSystem};
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

pub(crate) const OSD_FREE_COLUMN_FRONTIER: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OsdCandidateSearchPlan {
    pub(crate) free_column_count: usize,
    pub(crate) candidate_search_frontier_size: usize,
    pub(crate) max_candidate_order: usize,
    pub(crate) planned_candidate_count: u128,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LdpcOsdCsCandidatePlan {
    pub(crate) free_column_count: usize,
    pub(crate) pair_candidate_frontier_size: usize,
    pub(crate) osd_order: usize,
    pub(crate) planned_candidate_count: u128,
}

#[derive(Debug)]
pub(crate) struct OsdWorkspace {
    column_order: Vec<usize>,
    prepared: PreparedLinearSystem,
    num_checks: usize,
    num_bits: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct OsdDecodeStats {
    pub(crate) osd_candidate_count: usize,
    pub(crate) gf2_solve_count: usize,
    pub(crate) gf2_full_elimination_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OsdDecodeOutcome {
    pub(crate) correction: Correction,
    pub(crate) stats: OsdDecodeStats,
}

impl OsdWorkspace {
    pub(crate) fn new(pcm: &ParityCheckMatrix) -> Self {
        Self {
            column_order: (0..pcm.num_bits()).collect(),
            prepared: PreparedLinearSystem::from_pcm(pcm),
            num_checks: pcm.num_checks(),
            num_bits: pcm.num_bits(),
        }
    }

    pub(crate) fn sort_unreliable_columns(&mut self, reliability: &[f64]) -> &[usize] {
        debug_assert_eq!(reliability.len(), self.num_bits);
        self.column_order.clear();
        self.column_order.extend(0..reliability.len());
        self.column_order.sort_by(|&a, &b| {
            reliability[a]
                .partial_cmp(&reliability[b])
                .unwrap()
                .then_with(|| a.cmp(&b))
        });
        &self.column_order
    }
}

#[allow(dead_code)]
pub(crate) fn decode_osd0_with_workspace(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    base_correction_bits: &[bool],
    reliability: &[f64],
    workspace: &mut OsdWorkspace,
) -> Result<Correction, DecodeError> {
    decode_osd_with_workspace(
        pcm,
        syndrome,
        base_correction_bits,
        reliability,
        reliability,
        workspace,
        OsdVariant::Osd0,
        0,
    )
    .map(|outcome| outcome.correction)
}

pub(crate) fn effective_osd_variant(config: DecoderConfig) -> OsdVariant {
    match config.osd_variant {
        OsdVariant::Osd0 if config.osd_order > 0 => OsdVariant::LegacyCombinationSweep,
        other => other,
    }
}

pub(crate) fn decode_osd_with_workspace(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    base_correction_bits: &[bool],
    ordering_reliability: &[f64],
    objective_weights: &[f64],
    workspace: &mut OsdWorkspace,
    planner: OsdVariant,
    osd_order: usize,
) -> Result<OsdDecodeOutcome, DecodeError> {
    debug_assert_eq!(workspace.num_checks, pcm.num_checks());
    debug_assert_eq!(workspace.num_bits, pcm.num_bits());
    debug_assert_eq!(base_correction_bits.len(), pcm.num_bits());
    debug_assert_eq!(ordering_reliability.len(), pcm.num_bits());
    validate_objective_weights(objective_weights, pcm.num_bits())?;
    let target_syndrome = xor_syndromes(&multiply_bits(pcm, base_correction_bits), syndrome);
    workspace.sort_unreliable_columns(ordering_reliability);
    let mut stats = OsdDecodeStats::default();
    let mut gf2_stats = Gf2SolveStats::default();
    let reduced = workspace
        .prepared
        .reduce_with_column_order_counting(
            &target_syndrome,
            &workspace.column_order,
            &mut gf2_stats,
        )
        .map_err(|_| DecodeError::NoOsdSolution)?;
    accumulate_gf2_stats(&mut stats, gf2_stats);

    let mut gf2_stats = Gf2SolveStats::default();
    let base = reduced
        .solve_with_forced_columns_counting(&[], &mut gf2_stats)
        .map_err(|_| DecodeError::NoOsdSolution)?;
    accumulate_gf2_stats(&mut stats, gf2_stats);

    let best = match planner {
        OsdVariant::Osd0 => base,
        OsdVariant::LegacyCombinationSweep => {
            if osd_order == 0 {
                base
            } else {
                best_legacy_osd_candidate(objective_weights, &reduced, base, osd_order, &mut stats)?
            }
        }
        OsdVariant::LdpcCombinationSweep => {
            best_ldpc_osd_candidate(objective_weights, &reduced, base, osd_order, &mut stats)?
        }
    };
    Ok(OsdDecodeOutcome {
        correction: xor_correction_bits(base_correction_bits, &best.correction),
        stats,
    })
}

pub(crate) fn diagnose_osd_candidate_search_with_workspace(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    base_correction_bits: &[bool],
    reliability: &[f64],
    workspace: &mut OsdWorkspace,
    planner: OsdVariant,
    osd_order: usize,
) -> Result<OsdCandidateSearchPlan, DecodeError> {
    debug_assert_eq!(workspace.num_checks, pcm.num_checks());
    debug_assert_eq!(workspace.num_bits, pcm.num_bits());
    debug_assert_eq!(base_correction_bits.len(), pcm.num_bits());
    debug_assert_eq!(reliability.len(), pcm.num_bits());
    let target_syndrome = xor_syndromes(&multiply_bits(pcm, base_correction_bits), syndrome);
    workspace.sort_unreliable_columns(reliability);
    let base = workspace
        .prepared
        .solve_with_column_order_detailed(&target_syndrome, &workspace.column_order, &[])
        .map_err(|_| DecodeError::NoOsdSolution)?;

    Ok(match planner {
        OsdVariant::Osd0 => candidate_search_plan_for_osd0(&base),
        OsdVariant::LegacyCombinationSweep => legacy_candidate_search_plan(&base, osd_order),
        OsdVariant::LdpcCombinationSweep => ldpc_candidate_search_plan(&base, osd_order),
    })
}

pub(crate) fn profile_osd_with_workspace(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    base_correction_bits: &[bool],
    reliability: &[f64],
    workspace: &mut OsdWorkspace,
    planner: OsdVariant,
    osd_order: usize,
    candidate_limit: usize,
) -> Result<OsdDecodeStats, DecodeError> {
    debug_assert_eq!(workspace.num_checks, pcm.num_checks());
    debug_assert_eq!(workspace.num_bits, pcm.num_bits());
    debug_assert_eq!(base_correction_bits.len(), pcm.num_bits());
    debug_assert_eq!(reliability.len(), pcm.num_bits());
    let target_syndrome = xor_syndromes(&multiply_bits(pcm, base_correction_bits), syndrome);
    workspace.sort_unreliable_columns(reliability);
    let mut stats = OsdDecodeStats::default();
    let mut gf2_stats = Gf2SolveStats::default();
    let reduced = workspace
        .prepared
        .reduce_with_column_order_counting(
            &target_syndrome,
            &workspace.column_order,
            &mut gf2_stats,
        )
        .map_err(|_| DecodeError::NoOsdSolution)?;
    accumulate_gf2_stats(&mut stats, gf2_stats);

    let mut gf2_stats = Gf2SolveStats::default();
    let base = reduced
        .solve_with_forced_columns_counting(&[], &mut gf2_stats)
        .map_err(|_| DecodeError::NoOsdSolution)?;
    accumulate_gf2_stats(&mut stats, gf2_stats);

    if candidate_limit == 0 {
        return Ok(stats);
    }

    match planner {
        OsdVariant::Osd0 => {}
        OsdVariant::LegacyCombinationSweep => {
            if osd_order > 0 {
                profile_legacy_osd_candidates(
                    &reduced,
                    &base,
                    osd_order,
                    candidate_limit,
                    &mut stats,
                )?;
            }
        }
        OsdVariant::LdpcCombinationSweep => {
            profile_ldpc_osd_candidates(&reduced, &base, osd_order, candidate_limit, &mut stats)?;
        }
    }

    Ok(stats)
}

fn accumulate_gf2_stats(stats: &mut OsdDecodeStats, gf2_stats: Gf2SolveStats) {
    stats.gf2_solve_count += gf2_stats.solve_count;
    stats.gf2_full_elimination_count += gf2_stats.full_elimination_count;
}

fn best_legacy_osd_candidate(
    objective_weights: &[f64],
    reduced: &ReducedLinearSystem,
    base: DetailedSolution,
    osd_order: usize,
    stats: &mut OsdDecodeStats,
) -> Result<DetailedSolution, DecodeError> {
    let frontier_len = base.free_columns.len().min(OSD_FREE_COLUMN_FRONTIER);
    let frontier = base.free_columns[..frontier_len].to_vec();
    let max_order = osd_order.min(frontier.len());
    let influences = reduced.free_column_influence_vectors(&base, &frontier)?;
    let mut best = base;
    let mut forced = Vec::new();
    for order in 1..=max_order {
        visit_combinations(&frontier, order, 0, &mut forced, &mut |columns| {
            stats.osd_candidate_count += 1;
            let candidate = assemble_candidate_solution(&best, &influences, columns);
            if let Ok(candidate) = candidate {
                if is_better_solution(&candidate, &best, objective_weights) {
                    best = candidate;
                }
            }
        });
    }
    Ok(best)
}

fn best_ldpc_osd_candidate(
    objective_weights: &[f64],
    reduced: &ReducedLinearSystem,
    base: DetailedSolution,
    osd_order: usize,
    stats: &mut OsdDecodeStats,
) -> Result<DetailedSolution, DecodeError> {
    let free_columns = base.free_columns.clone();
    let influences = reduced.free_column_influence_vectors(&base, &free_columns)?;
    let mut best = base;
    for &column in &free_columns {
        stats.osd_candidate_count += 1;
        let candidate = assemble_candidate_solution(&best, &influences, &[column])
            .expect("LDPC OSD-CS single-column candidates are selected from free columns");
        if is_better_solution(&candidate, &best, objective_weights) {
            best = candidate;
        }
    }

    let frontier_len = free_columns.len().min(osd_order);
    let frontier = free_columns[..frontier_len].to_vec();
    if frontier.len() < 2 {
        return Ok(best);
    }

    let mut forced = Vec::new();
    visit_combinations(&frontier, 2, 0, &mut forced, &mut |columns| {
        stats.osd_candidate_count += 1;
        let candidate = assemble_candidate_solution(&best, &influences, columns)
            .expect("LDPC OSD-CS pair candidates are selected from free columns");
        if is_better_solution(&candidate, &best, objective_weights) {
            best = candidate;
        }
    });

    Ok(best)
}

fn candidate_search_plan_for_osd0(base: &DetailedSolution) -> OsdCandidateSearchPlan {
    OsdCandidateSearchPlan {
        free_column_count: base.free_columns.len(),
        candidate_search_frontier_size: 0,
        max_candidate_order: 0,
        planned_candidate_count: 0,
    }
}

fn legacy_candidate_search_plan(
    base: &DetailedSolution,
    osd_order: usize,
) -> OsdCandidateSearchPlan {
    let candidate_search_frontier_size = base.free_columns.len().min(OSD_FREE_COLUMN_FRONTIER);
    let max_candidate_order = osd_order.min(candidate_search_frontier_size);
    let planned_candidate_count = (1..=max_candidate_order)
        .map(|order| binomial(candidate_search_frontier_size, order))
        .sum();

    OsdCandidateSearchPlan {
        free_column_count: base.free_columns.len(),
        candidate_search_frontier_size,
        max_candidate_order,
        planned_candidate_count,
    }
}

fn ldpc_candidate_search_plan(base: &DetailedSolution, osd_order: usize) -> OsdCandidateSearchPlan {
    let plan = ldpc_osd_cs_candidate_plan_for_free_columns(base.free_columns.len(), osd_order);
    let max_candidate_order = if plan.free_column_count == 0 {
        0
    } else if plan.pair_candidate_frontier_size >= 2 {
        2
    } else {
        1
    };

    OsdCandidateSearchPlan {
        free_column_count: plan.free_column_count,
        candidate_search_frontier_size: plan.pair_candidate_frontier_size,
        max_candidate_order,
        planned_candidate_count: plan.planned_candidate_count,
    }
}

#[allow(dead_code)]
pub(crate) fn ldpc_osd_cs_candidate_plan_for_free_columns(
    free_column_count: usize,
    osd_order: usize,
) -> LdpcOsdCsCandidatePlan {
    let pair_candidate_frontier_size = free_column_count.min(osd_order);
    let planned_candidate_count =
        free_column_count as u128 + binomial(pair_candidate_frontier_size, 2);

    LdpcOsdCsCandidatePlan {
        free_column_count,
        pair_candidate_frontier_size,
        osd_order,
        planned_candidate_count,
    }
}

fn profile_legacy_osd_candidates(
    reduced: &ReducedLinearSystem,
    base: &DetailedSolution,
    osd_order: usize,
    candidate_limit: usize,
    stats: &mut OsdDecodeStats,
) -> Result<(), DecodeError> {
    let frontier_len = base.free_columns.len().min(OSD_FREE_COLUMN_FRONTIER);
    let frontier = base.free_columns[..frontier_len].to_vec();
    let max_order = osd_order.min(frontier.len());
    let influences = reduced.free_column_influence_vectors(base, &frontier)?;
    let mut forced = Vec::new();
    let mut visited = 0usize;
    for order in 1..=max_order {
        visit_combinations_until(
            &frontier,
            order,
            0,
            &mut forced,
            &mut visited,
            candidate_limit,
            &mut |columns| {
                stats.osd_candidate_count += 1;
                let _ = influences.correction_for_forced_columns(columns);
            },
        );
        if visited >= candidate_limit {
            break;
        }
    }
    Ok(())
}

fn profile_ldpc_osd_candidates(
    reduced: &ReducedLinearSystem,
    base: &DetailedSolution,
    osd_order: usize,
    candidate_limit: usize,
    stats: &mut OsdDecodeStats,
) -> Result<(), DecodeError> {
    let influences = reduced.free_column_influence_vectors(base, &base.free_columns)?;
    let mut visited = 0usize;
    for &column in &base.free_columns {
        if visited >= candidate_limit {
            return Ok(());
        }
        visited += 1;
        stats.osd_candidate_count += 1;
        let _ = influences.correction_for_forced_columns(&[column]);
    }

    let frontier_len = base.free_columns.len().min(osd_order);
    if frontier_len < 2 {
        return Ok(());
    }

    let frontier = base.free_columns[..frontier_len].to_vec();
    let mut forced = Vec::new();
    visit_combinations_until(
        &frontier,
        2,
        0,
        &mut forced,
        &mut visited,
        candidate_limit,
        &mut |columns| {
            stats.osd_candidate_count += 1;
            let _ = influences.correction_for_forced_columns(columns);
        },
    );
    Ok(())
}

fn assemble_candidate_solution(
    template: &DetailedSolution,
    influences: &crate::gf2::FreeColumnInfluenceVectors,
    forced_true_columns: &[usize],
) -> Result<DetailedSolution, DecodeError> {
    Ok(DetailedSolution {
        correction: influences.correction_for_forced_columns(forced_true_columns)?,
        pivot_columns: template.pivot_columns.clone(),
        free_columns: template.free_columns.clone(),
    })
}

fn binomial(n: usize, k: usize) -> u128 {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut result = 1u128;
    for step in 0..k {
        result = result * (n - step) as u128 / (step + 1) as u128;
    }
    result
}

fn visit_combinations(
    columns: &[usize],
    target_len: usize,
    start: usize,
    forced: &mut Vec<usize>,
    visit: &mut impl FnMut(&[usize]),
) {
    if forced.len() == target_len {
        visit(forced);
        return;
    }
    let remaining = target_len - forced.len();
    for index in start..=columns.len() - remaining {
        forced.push(columns[index]);
        visit_combinations(columns, target_len, index + 1, forced, visit);
        forced.pop();
    }
}

fn visit_combinations_until(
    columns: &[usize],
    target_len: usize,
    start: usize,
    forced: &mut Vec<usize>,
    visited: &mut usize,
    limit: usize,
    visit: &mut impl FnMut(&[usize]),
) {
    if *visited >= limit {
        return;
    }
    if forced.len() == target_len {
        *visited += 1;
        visit(forced);
        return;
    }
    let remaining = target_len - forced.len();
    for index in start..=columns.len() - remaining {
        forced.push(columns[index]);
        visit_combinations_until(
            columns,
            target_len,
            index + 1,
            forced,
            visited,
            limit,
            visit,
        );
        forced.pop();
        if *visited >= limit {
            break;
        }
    }
}

fn is_better_solution(
    candidate: &DetailedSolution,
    best: &DetailedSolution,
    objective_weights: &[f64],
) -> bool {
    let candidate_cost = residual_cost(candidate.correction.as_slice(), objective_weights);
    let best_cost = residual_cost(best.correction.as_slice(), objective_weights);
    if candidate_cost < best_cost - f64::EPSILON {
        return true;
    }
    if (candidate_cost - best_cost).abs() <= f64::EPSILON {
        return candidate.correction.as_slice() < best.correction.as_slice();
    }
    false
}

fn validate_objective_weights(weights: &[f64], expected: usize) -> Result<(), DecodeError> {
    if weights.len() != expected {
        return Err(DecodeError::DimensionMismatch {
            what: "OSD objective weights",
            expected,
            actual: weights.len(),
        });
    }
    if !weights.iter().all(|weight| weight.is_finite()) {
        return Err(DecodeError::InvalidProbability);
    }
    Ok(())
}

fn residual_cost(bits: &[bool], weights: &[f64]) -> f64 {
    bits.iter()
        .zip(weights.iter())
        .filter_map(|(&bit, &weight)| bit.then_some(weight.abs()))
        .sum()
}

fn multiply_bits(pcm: &ParityCheckMatrix, bits: &[bool]) -> Syndrome {
    let mut syndrome = vec![false; pcm.num_checks()];
    for (check, value) in syndrome.iter_mut().enumerate() {
        let mut parity = false;
        for &bit in pcm.row_neighbors(check) {
            parity ^= bits[bit];
        }
        *value = parity;
    }
    Syndrome::from(syndrome)
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

#[cfg(test)]
mod tests {
    use crate::error::DecodeError;
    use crate::matrix::ParityCheckMatrix;
    use crate::vector::{Correction, Syndrome};

    use super::{
        OsdWorkspace, binomial, decode_osd0_with_workspace,
        ldpc_osd_cs_candidate_plan_for_free_columns, validate_objective_weights,
    };

    const LDPC_OSD_CS_CONTRACT_PATH: &str = "rbposd/doc/osd_cs_contract.md";
    const LDPC_OSD_CS_CONTRACT: &str = include_str!("../doc/osd_cs_contract.md");
    const REQUIRED_UPSTREAM_SHAPE: &str =
        "singles over all non-pivot columns + pairs among the first osd_order non-pivot columns";
    const REQUIRED_SCORING_BOUNDARY: &str =
        "Candidate ordering/selection is separate from candidate scoring/objective weights";

    #[test]
    fn decode_osd0_with_workspace_prefers_the_lower_reliability_pivot_basis() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0], vec![1, 2]]).unwrap();
        let syndrome = Syndrome::from(vec![false, true]);
        let base = Correction::from(vec![false, false, false]);
        let reliability = vec![1.0, 1.0, 2.0];
        let mut workspace = OsdWorkspace::new(&pcm);

        let correction = decode_osd0_with_workspace(
            &pcm,
            &syndrome,
            base.as_slice(),
            &reliability,
            &mut workspace,
        )
        .unwrap();

        assert_eq!(correction, Correction::from(vec![false, true, false]));
    }

    #[test]
    fn osd_workspace_orders_columns_by_unreliability_stably() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
        let mut workspace = OsdWorkspace::new(&pcm);

        let order = workspace.sort_unreliable_columns(&[1.0, 1.0, 0.4]);

        assert_eq!(order, &[2, 0, 1]);
    }

    #[test]
    fn osd_objective_weight_validation_rejects_length_mismatch() {
        let error = validate_objective_weights(&[1.0, 2.0], 3).unwrap_err();

        assert_eq!(
            error,
            DecodeError::DimensionMismatch {
                what: "OSD objective weights",
                expected: 3,
                actual: 2,
            }
        );
    }

    #[test]
    fn osd_objective_weight_validation_rejects_non_finite_weights() {
        let error = validate_objective_weights(&[1.0, f64::NAN], 2).unwrap_err();

        assert_eq!(error, DecodeError::InvalidProbability);
    }

    #[test]
    fn binomial_returns_zero_for_oversized_selection() {
        assert_eq!(binomial(3, 4), 0);
    }

    #[test]
    fn ldpc_osd_cs_contract_matches_reference_candidate_plan() {
        println!("contract document: {LDPC_OSD_CS_CONTRACT_PATH}");
        assert_contract_text_is_complete();

        let free_column_count = 20;
        let osd_order = 7;
        let plan = ldpc_osd_cs_candidate_plan_for_free_columns(free_column_count, osd_order);

        assert_eq!(plan.free_column_count, free_column_count);
        assert_eq!(plan.pair_candidate_frontier_size, osd_order);
        assert_eq!(plan.osd_order, osd_order);
        assert_eq!(
            plan.planned_candidate_count,
            free_column_count as u128 + binomial(osd_order, 2)
        );
        assert_eq!(plan.planned_candidate_count, 41);
    }

    #[test]
    fn ldpc_osd_cs_contract_rejects_exhaustive_frontier_plan() {
        assert_contract_text_is_complete();

        let plan = ldpc_osd_cs_candidate_plan_for_free_columns(20, 7);
        let legacy_exhaustive_frontier_count: u128 = (1..=7).map(|order| binomial(16, order)).sum();

        assert_eq!(legacy_exhaustive_frontier_count, 26_332);
        assert_ne!(
            plan.planned_candidate_count, legacy_exhaustive_frontier_count,
            "exhaustive/frontier search remains a separate legacy/internal mode, \
             not the upstream ldpc osd_cs contract"
        );
    }

    fn assert_contract_text_is_complete() {
        assert!(
            LDPC_OSD_CS_CONTRACT.contains(REQUIRED_UPSTREAM_SHAPE),
            "contract document {LDPC_OSD_CS_CONTRACT_PATH} must state `{REQUIRED_UPSTREAM_SHAPE}`"
        );
        assert!(
            LDPC_OSD_CS_CONTRACT.contains(REQUIRED_SCORING_BOUNDARY),
            "contract document {LDPC_OSD_CS_CONTRACT_PATH} must separate ordering/selection \
             from scoring/objective weights"
        );
    }
}
