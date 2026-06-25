use crate::error::DecodeError;
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Gf2SolveStats {
    pub(crate) solve_count: usize,
    pub(crate) full_elimination_count: usize,
}

#[cfg(test)]
pub(crate) fn sort_columns_by_reliability(scores: &[f64]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|&a, &b| {
        scores[b]
            .partial_cmp(&scores[a])
            .unwrap()
            .then_with(|| a.cmp(&b))
    });
    order
}

#[cfg(test)]
pub(crate) fn sort_columns_by_unreliability(scores: &[f64]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|&a, &b| {
        scores[a]
            .partial_cmp(&scores[b])
            .unwrap()
            .then_with(|| a.cmp(&b))
    });
    order
}

#[cfg(test)]
pub(crate) fn solve_with_column_order(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    column_order: &[usize],
) -> Result<Correction, DecodeError> {
    PreparedLinearSystem::from_pcm(pcm).solve_with_column_order(syndrome, column_order)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DetailedSolution {
    pub(crate) correction: Correction,
    pub(crate) pivot_columns: Vec<usize>,
    pub(crate) free_columns: Vec<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct ReducedLinearSystem {
    rows: Vec<Vec<bool>>,
    rhs: Vec<bool>,
    pivot_columns: Vec<usize>,
    free_columns: Vec<usize>,
    is_free: Vec<bool>,
    num_bits: usize,
}

#[derive(Debug)]
pub(crate) struct PreparedLinearSystem {
    base_rows: Vec<Vec<bool>>,
    scratch_rows: Vec<Vec<bool>>,
    scratch_rhs: Vec<bool>,
    pivot_columns: Vec<usize>,
    num_bits: usize,
}

impl PreparedLinearSystem {
    pub(crate) fn from_pcm(pcm: &ParityCheckMatrix) -> Self {
        let base_rows = pcm.dense_rows();
        let scratch_rows = base_rows.clone();
        Self {
            base_rows,
            scratch_rows,
            scratch_rhs: vec![false; pcm.num_checks()],
            pivot_columns: Vec::with_capacity(pcm.num_checks()),
            num_bits: pcm.num_bits(),
        }
    }

    pub(crate) fn solve_with_column_order(
        &mut self,
        syndrome: &Syndrome,
        column_order: &[usize],
    ) -> Result<Correction, DecodeError> {
        self.solve_with_column_order_detailed_with_stats(syndrome, column_order, &[])
            .map(|(solution, _)| solution.correction)
    }

    pub(crate) fn solve_with_column_order_detailed(
        &mut self,
        syndrome: &Syndrome,
        column_order: &[usize],
        forced_true_columns: &[usize],
    ) -> Result<DetailedSolution, DecodeError> {
        self.solve_with_column_order_detailed_with_stats(
            syndrome,
            column_order,
            forced_true_columns,
        )
        .map(|(solution, _)| solution)
    }

    pub(crate) fn solve_with_column_order_detailed_with_stats(
        &mut self,
        syndrome: &Syndrome,
        column_order: &[usize],
        forced_true_columns: &[usize],
    ) -> Result<(DetailedSolution, Gf2SolveStats), DecodeError> {
        let mut stats = Gf2SolveStats::default();
        let solution = self.solve_with_column_order_detailed_counting(
            syndrome,
            column_order,
            forced_true_columns,
            &mut stats,
        )?;
        Ok((solution, stats))
    }

    pub(crate) fn solve_with_column_order_detailed_counting(
        &mut self,
        syndrome: &Syndrome,
        column_order: &[usize],
        forced_true_columns: &[usize],
        stats: &mut Gf2SolveStats,
    ) -> Result<DetailedSolution, DecodeError> {
        stats.solve_count += 1;
        let reduced = self.reduce_with_column_order_counting(syndrome, column_order, stats)?;
        reduced.solve_with_forced_columns(forced_true_columns)
    }

    pub(crate) fn reduce_with_column_order_counting(
        &mut self,
        syndrome: &Syndrome,
        column_order: &[usize],
        stats: &mut Gf2SolveStats,
    ) -> Result<ReducedLinearSystem, DecodeError> {
        stats.full_elimination_count += 1;
        self.scratch_rows.clone_from(&self.base_rows);
        self.scratch_rhs.copy_from_slice(syndrome.as_slice());
        self.pivot_columns.clear();
        let mut row = 0usize;

        for (pivot_position, &column) in column_order.iter().enumerate() {
            if row == self.scratch_rows.len() {
                break;
            }
            let pivot = (row..self.scratch_rows.len())
                .find(|&candidate| self.scratch_rows[candidate][column]);
            if let Some(pivot_row) = pivot {
                self.scratch_rows.swap(row, pivot_row);
                self.scratch_rhs.swap(row, pivot_row);
                for other in 0..self.scratch_rows.len() {
                    if other != row && self.scratch_rows[other][column] {
                        for &physical in column_order.iter().skip(pivot_position) {
                            self.scratch_rows[other][physical] ^= self.scratch_rows[row][physical];
                        }
                        self.scratch_rhs[other] ^= self.scratch_rhs[row];
                    }
                }
                self.pivot_columns.push(column);
                row += 1;
            }
        }

        if self.scratch_rhs.iter().skip(row).any(|&bit| bit) {
            return Err(DecodeError::SingularSystem);
        }

        let mut is_pivot = vec![false; self.num_bits];
        for &column in &self.pivot_columns {
            is_pivot[column] = true;
        }
        let free_columns = column_order
            .iter()
            .copied()
            .filter(|&column| !is_pivot[column])
            .collect::<Vec<_>>();
        let mut is_free = vec![false; self.num_bits];
        for &column in &free_columns {
            is_free[column] = true;
        }

        Ok(ReducedLinearSystem {
            rows: self.scratch_rows.clone(),
            rhs: self.scratch_rhs.clone(),
            pivot_columns: self.pivot_columns.clone(),
            free_columns,
            is_free,
            num_bits: self.num_bits,
        })
    }
}

impl ReducedLinearSystem {
    fn solve_with_forced_columns(
        &self,
        forced_true_columns: &[usize],
    ) -> Result<DetailedSolution, DecodeError> {
        let mut solution = vec![false; self.num_bits];
        for &column in forced_true_columns {
            if column >= self.num_bits {
                return Err(DecodeError::InvalidColumnIndex {
                    column,
                    num_bits: self.num_bits,
                });
            }
            if !self.is_free[column] {
                return Err(DecodeError::SingularSystem);
            }
            solution[column] = true;
        }

        for (pivot_row, &column) in self.pivot_columns.iter().enumerate().rev() {
            let mut value = self.rhs[pivot_row];
            for (physical, &coefficient) in self.rows[pivot_row].iter().enumerate() {
                if physical != column && coefficient && solution[physical] {
                    value ^= true;
                }
            }
            solution[column] = value;
        }

        Ok(DetailedSolution {
            correction: Correction::from(solution),
            pivot_columns: self.pivot_columns.clone(),
            free_columns: self.free_columns.clone(),
        })
    }

    pub(crate) fn solve_with_forced_columns_counting(
        &self,
        forced_true_columns: &[usize],
        stats: &mut Gf2SolveStats,
    ) -> Result<DetailedSolution, DecodeError> {
        stats.solve_count += 1;
        self.solve_with_forced_columns(forced_true_columns)
    }
}

#[cfg(test)]
mod tests {
    use crate::error::DecodeError;
    use crate::matrix::ParityCheckMatrix;
    use crate::vector::{Correction, Syndrome};

    use super::{
        PreparedLinearSystem, solve_with_column_order, sort_columns_by_reliability,
        sort_columns_by_unreliability,
    };

    #[test]
    fn reliability_sort_is_stable_for_equal_scores() {
        let order = sort_columns_by_reliability(&[0.9, 0.9, 0.4, 0.9]);
        assert_eq!(order, vec![0, 1, 3, 2]);
    }

    #[test]
    fn unreliability_sort_is_stable_for_equal_scores() {
        let order = sort_columns_by_unreliability(&[0.9, 0.9, 0.4, 0.9]);
        assert_eq!(order, vec![2, 0, 1, 3]);
    }

    #[test]
    fn solve_with_column_order_returns_a_valid_solution() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
        let syndrome = Syndrome::from(vec![true, false]);
        let order = vec![0, 1, 2];

        let correction = solve_with_column_order(&pcm, &syndrome, &order).unwrap();

        assert_eq!(pcm.multiply(&correction), syndrome);
    }

    #[test]
    fn solve_with_column_order_reordered_returns_a_valid_solution() {
        let pcm =
            ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1, 2], vec![1, 2]]).unwrap();
        let syndrome = Syndrome::from(vec![false, true]);
        let order = vec![2, 0, 1];

        let correction = solve_with_column_order(&pcm, &syndrome, &order).unwrap();

        assert_eq!(pcm.multiply(&correction), syndrome);
    }

    #[test]
    fn prepared_system_solves_multiple_rhs_without_rebuilding_matrix_storage() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
        let mut prepared = PreparedLinearSystem::from_pcm(&pcm);

        let first = prepared
            .solve_with_column_order(&Syndrome::from(vec![true, false]), &[0, 1, 2])
            .unwrap();
        let second = prepared
            .solve_with_column_order(&Syndrome::from(vec![false, true]), &[2, 0, 1])
            .unwrap();

        assert_eq!(pcm.multiply(&first), Syndrome::from(vec![true, false]));
        assert_eq!(pcm.multiply(&second), Syndrome::from(vec![false, true]));
    }

    #[test]
    fn prepared_system_can_force_free_columns() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 2], vec![1, 2]]).unwrap();
        let syndrome = Syndrome::from(vec![true, true]);
        let mut prepared = PreparedLinearSystem::from_pcm(&pcm);

        let osd0 = prepared
            .solve_with_column_order_detailed(&syndrome, &[0, 1, 2], &[])
            .unwrap();
        let forced = prepared
            .solve_with_column_order_detailed(&syndrome, &[0, 1, 2], &[2])
            .unwrap();

        assert_eq!(osd0.correction, Correction::from(vec![true, true, false]));
        assert_eq!(osd0.pivot_columns, vec![0, 1]);
        assert_eq!(osd0.free_columns, vec![2]);
        assert_eq!(
            forced.correction,
            Correction::from(vec![false, false, true])
        );
        assert_eq!(pcm.multiply(&forced.correction), syndrome);
    }

    #[test]
    fn osd_forced_pivot_columns_are_rejected_after_optimization() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 2], vec![1, 2]]).unwrap();
        let syndrome = Syndrome::from(vec![true, true]);
        let mut prepared = PreparedLinearSystem::from_pcm(&pcm);

        let pivot_error = prepared
            .solve_with_column_order_detailed(&syndrome, &[0, 1, 2], &[0])
            .unwrap_err();
        let out_of_range_error = prepared
            .solve_with_column_order_detailed(&syndrome, &[0, 1, 2], &[3])
            .unwrap_err();
        let outside_ordered_free_error = prepared
            .solve_with_column_order_detailed(&syndrome, &[0, 1], &[2])
            .unwrap_err();

        assert_eq!(pivot_error, DecodeError::SingularSystem);
        assert_eq!(
            out_of_range_error,
            DecodeError::InvalidColumnIndex {
                column: 3,
                num_bits: 3,
            }
        );
        assert_eq!(outside_ordered_free_error, DecodeError::SingularSystem);
    }

    #[test]
    fn prepared_system_counts_failed_detailed_solve_attempts() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 2], vec![1, 2]]).unwrap();
        let syndrome = Syndrome::from(vec![true, true]);
        let mut prepared = PreparedLinearSystem::from_pcm(&pcm);
        let mut stats = super::Gf2SolveStats::default();

        let error = prepared
            .solve_with_column_order_detailed_counting(&syndrome, &[0, 1, 2], &[0], &mut stats)
            .unwrap_err();

        assert_eq!(error, DecodeError::SingularSystem);
        assert_eq!(stats.solve_count, 1);
        assert_eq!(stats.full_elimination_count, 1);
    }

    #[test]
    fn prepared_system_counts_reduction_time_singular_solve_attempts() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 1, vec![vec![0], vec![0]]).unwrap();
        let syndrome = Syndrome::from(vec![true, false]);
        let mut prepared = PreparedLinearSystem::from_pcm(&pcm);
        let mut stats = super::Gf2SolveStats::default();

        let error = prepared
            .solve_with_column_order_detailed_counting(&syndrome, &[0], &[], &mut stats)
            .unwrap_err();

        assert_eq!(error, DecodeError::SingularSystem);
        assert_eq!(stats.solve_count, 1);
        assert_eq!(stats.full_elimination_count, 1);
    }

    #[test]
    fn reduced_system_reuses_one_elimination_across_multiple_forced_solves() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 2], vec![1, 2]]).unwrap();
        let syndrome = Syndrome::from(vec![true, true]);
        let mut prepared = PreparedLinearSystem::from_pcm(&pcm);
        let mut stats = super::Gf2SolveStats::default();

        let reduced = prepared
            .reduce_with_column_order_counting(&syndrome, &[0, 1, 2], &mut stats)
            .unwrap();
        let base = reduced
            .solve_with_forced_columns_counting(&[], &mut stats)
            .unwrap();
        let forced = reduced
            .solve_with_forced_columns_counting(&[2], &mut stats)
            .unwrap();

        assert_eq!(base.correction, Correction::from(vec![true, true, false]));
        assert_eq!(
            forced.correction,
            Correction::from(vec![false, false, true])
        );
        assert_eq!(stats.solve_count, 2);
        assert_eq!(stats.full_elimination_count, 1);
    }

    #[test]
    fn detailed_solving_without_forced_columns_matches_basic_solving() {
        let pcm =
            ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1, 2], vec![1, 2]]).unwrap();
        let syndrome = Syndrome::from(vec![false, true]);
        let order = vec![2, 0, 1];
        let mut detailed_prepared = PreparedLinearSystem::from_pcm(&pcm);
        let mut basic_prepared = PreparedLinearSystem::from_pcm(&pcm);

        let detailed = detailed_prepared
            .solve_with_column_order_detailed(&syndrome, &order, &[])
            .unwrap();
        let basic = basic_prepared
            .solve_with_column_order(&syndrome, &order)
            .unwrap();

        assert_eq!(detailed.correction, basic);
    }

    #[test]
    fn solve_with_column_order_prefers_low_reliability_basis_for_syndrome_decoding() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0], vec![1, 2]]).unwrap();
        let syndrome = Syndrome::from(vec![false, true]);

        // For syndrome decoding, the dependent basis columns should come from the
        // least reliable positions so the free variables can stay on the more
        // reliable side. The current ldpc OSD_0 behavior prefers column 1 here.
        let low_reliability_first = vec![0, 1, 2];
        let correction = solve_with_column_order(&pcm, &syndrome, &low_reliability_first).unwrap();

        assert_eq!(correction, Correction::from(vec![false, true, false]));
    }
}
