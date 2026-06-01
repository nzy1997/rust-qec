use std::num::NonZeroU32;
use std::os::raw::c_int;

use highs::{ColProblem, HighsModelStatus, HighsSolutionStatus, Model};

use crate::backend::BatchBackend;
use crate::config::IlpDecoderConfig;
use crate::error::IlpDecodeError;
use crate::problem::LoweredDemProblem;

#[derive(Debug)]
pub struct HighsBatchBackend {
    model: Option<Model>,
    num_detectors: usize,
    num_error_columns: usize,
    forced_syndrome: Vec<bool>,
}

impl HighsBatchBackend {
    pub fn new(
        problem: &LoweredDemProblem,
        config: &IlpDecoderConfig,
    ) -> Result<Self, IlpDecodeError> {
        let mut col_problem = ColProblem::new();
        let mut rows = Vec::with_capacity(problem.num_detectors);
        for _ in 0..problem.num_detectors {
            rows.push(col_problem.add_row(0.0..=0.0));
        }

        for column in &problem.columns {
            let entries = column.detectors.iter().map(|&det| (rows[det], 1.0));
            col_problem.add_integer_column(column.weight, 0.0..=1.0, entries);
        }
        for &row in &rows {
            col_problem.add_integer_column(0.0, 0.0.., [(row, -2.0)]);
        }

        let mut model = Model::try_new(col_problem)
            .map_err(|err| IlpDecodeError::Highs(format!("failed to create model: {err:?}")))?;
        if !config.backend.verbose {
            model.make_quiet();
        }
        if let Some(threads) = config.backend.threads.and_then(NonZeroU32::new) {
            model.set_threads(threads);
        }
        if let Some(limit) = config.backend.time_limit_seconds {
            model.try_set_option("time_limit", limit).map_err(|err| {
                IlpDecodeError::Highs(format!("failed to set time_limit option: {err:?}"))
            })?;
        }
        if let Some(gap) = config.backend.mip_gap {
            model.try_set_option("mip_rel_gap", gap).map_err(|err| {
                IlpDecodeError::Highs(format!("failed to set mip_rel_gap option: {err:?}"))
            })?;
        }

        Ok(Self {
            model: Some(model),
            num_detectors: problem.num_detectors,
            num_error_columns: problem.columns.len(),
            forced_syndrome: problem.forced_syndrome.clone(),
        })
    }
}

fn accept_solved_model_status(
    model_status: HighsModelStatus,
    primal_status: HighsSolutionStatus,
) -> bool {
    match model_status {
        HighsModelStatus::Optimal => true,
        HighsModelStatus::ReachedTimeLimit => primal_status == HighsSolutionStatus::Feasible,
        _ => false,
    }
}

impl BatchBackend for HighsBatchBackend {
    fn solve(&mut self, syndrome: &[bool]) -> Result<Vec<bool>, IlpDecodeError> {
        if syndrome.len() != self.num_detectors {
            return Err(IlpDecodeError::DetectorWidthMismatch {
                expected: self.num_detectors,
                actual: syndrome.len(),
            });
        }

        let mut model = self
            .model
            .take()
            .ok_or_else(|| IlpDecodeError::Highs("model already in use".to_string()))?;

        for (row, (&bit, &forced)) in syndrome.iter().zip(&self.forced_syndrome).enumerate() {
            let rhs = if bit ^ forced { 1.0 } else { 0.0 };
            let status = unsafe {
                highs_sys::Highs_changeRowBounds(model.as_mut_ptr(), row as c_int, rhs, rhs)
            };
            if status != highs_sys::STATUS_OK {
                self.model = Some(model);
                return Err(IlpDecodeError::Highs(format!(
                    "failed to set row bounds for detector {row}: status {status}"
                )));
            }
        }

        let solved = model
            .try_solve()
            .map_err(|err| IlpDecodeError::Highs(format!("solve failed: {err:?}")))?;
        let model_status = solved.status();
        let primal_status = solved.primal_solution_status();
        if !accept_solved_model_status(model_status, primal_status) {
            self.model = Some(solved.into());
            return Err(IlpDecodeError::Highs(format!(
                "unexpected HiGHS solve status: model={model_status:?}, primal={primal_status:?}"
            )));
        }

        let columns = solved.get_solution().columns().to_vec();
        self.model = Some(solved.into());

        let expected_solution_width = self.num_error_columns + self.num_detectors;
        if columns.len() != expected_solution_width {
            return Err(IlpDecodeError::Highs(format!(
                "solution width mismatch: expected {}, got {}",
                expected_solution_width,
                columns.len()
            )));
        }

        Ok(columns[..self.num_error_columns]
            .iter()
            .map(|&value| value > 0.5)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use highs::{HighsModelStatus, HighsSolutionStatus};

    use super::accept_solved_model_status;

    #[test]
    fn accepts_optimal_solution() {
        assert!(accept_solved_model_status(
            HighsModelStatus::Optimal,
            HighsSolutionStatus::Feasible,
        ));
    }

    #[test]
    fn accepts_time_limited_feasible_solution() {
        assert!(accept_solved_model_status(
            HighsModelStatus::ReachedTimeLimit,
            HighsSolutionStatus::Feasible,
        ));
    }

    #[test]
    fn rejects_time_limited_run_without_feasible_solution() {
        assert!(!accept_solved_model_status(
            HighsModelStatus::ReachedTimeLimit,
            HighsSolutionStatus::None,
        ));
    }

    #[test]
    fn rejects_other_non_optimal_statuses_even_with_feasible_solution() {
        assert!(!accept_solved_model_status(
            HighsModelStatus::ReachedInterrupt,
            HighsSolutionStatus::Feasible,
        ));
    }
}
