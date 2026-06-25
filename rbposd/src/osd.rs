use crate::error::DecodeError;
use crate::gf2::{DetailedSolution, PreparedLinearSystem};
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

#[derive(Debug)]
pub(crate) struct OsdWorkspace {
    column_order: Vec<usize>,
    prepared: PreparedLinearSystem,
    num_checks: usize,
    num_bits: usize,
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
        workspace,
        0,
    )
}

pub(crate) fn decode_osd_with_workspace(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    base_correction_bits: &[bool],
    reliability: &[f64],
    workspace: &mut OsdWorkspace,
    osd_order: usize,
) -> Result<Correction, DecodeError> {
    debug_assert_eq!(workspace.num_checks, pcm.num_checks());
    debug_assert_eq!(workspace.num_bits, pcm.num_bits());
    debug_assert_eq!(base_correction_bits.len(), pcm.num_bits());
    debug_assert_eq!(reliability.len(), pcm.num_bits());
    let target_syndrome = xor_syndromes(&multiply_bits(pcm, base_correction_bits), syndrome);
    workspace.sort_unreliable_columns(reliability);

    if osd_order == 0 {
        let residual = workspace
            .prepared
            .solve_with_column_order(&target_syndrome, &workspace.column_order)
            .map_err(|_| DecodeError::NoOsdSolution)?;
        return Ok(xor_correction_bits(base_correction_bits, &residual));
    }

    let base = workspace
        .prepared
        .solve_with_column_order_detailed(&target_syndrome, &workspace.column_order, &[])
        .map_err(|_| DecodeError::NoOsdSolution)?;
    let best = best_osd_candidate(&target_syndrome, reliability, workspace, base, osd_order)?;
    Ok(xor_correction_bits(base_correction_bits, &best.correction))
}

pub(crate) fn diagnose_osd_candidate_search_with_workspace(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    base_correction_bits: &[bool],
    reliability: &[f64],
    workspace: &mut OsdWorkspace,
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

    Ok(candidate_search_plan(&base, osd_order))
}

fn best_osd_candidate(
    target_syndrome: &Syndrome,
    reliability: &[f64],
    workspace: &mut OsdWorkspace,
    base: DetailedSolution,
    osd_order: usize,
) -> Result<DetailedSolution, DecodeError> {
    let frontier_len = base.free_columns.len().min(OSD_FREE_COLUMN_FRONTIER);
    let frontier = base.free_columns[..frontier_len].to_vec();
    let max_order = osd_order.min(frontier.len());
    let mut best = base;
    let mut forced = Vec::new();
    for order in 1..=max_order {
        visit_combinations(&frontier, order, 0, &mut forced, &mut |columns| {
            if let Ok(candidate) = workspace.prepared.solve_with_column_order_detailed(
                target_syndrome,
                &workspace.column_order,
                columns,
            ) {
                if is_better_solution(&candidate, &best, reliability) {
                    best = candidate;
                }
            }
        });
    }
    Ok(best)
}

fn candidate_search_plan(base: &DetailedSolution, osd_order: usize) -> OsdCandidateSearchPlan {
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

fn is_better_solution(
    candidate: &DetailedSolution,
    best: &DetailedSolution,
    reliability: &[f64],
) -> bool {
    let candidate_cost = residual_cost(candidate.correction.as_slice(), reliability);
    let best_cost = residual_cost(best.correction.as_slice(), reliability);
    if candidate_cost < best_cost - f64::EPSILON {
        return true;
    }
    if (candidate_cost - best_cost).abs() <= f64::EPSILON {
        return candidate.correction.as_slice() < best.correction.as_slice();
    }
    false
}

fn residual_cost(bits: &[bool], reliability: &[f64]) -> f64 {
    bits.iter()
        .zip(reliability.iter())
        .filter_map(|(&bit, &cost)| bit.then_some(cost))
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
    use crate::matrix::ParityCheckMatrix;
    use crate::vector::{Correction, Syndrome};

    use super::{OsdWorkspace, decode_osd0_with_workspace};

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
}
