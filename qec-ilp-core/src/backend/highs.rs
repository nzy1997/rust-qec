use std::ffi::CString;
use std::num::NonZeroU32;
use std::os::raw::c_int;

use highs::{ColProblem, HighsModelStatus, HighsSolutionStatus, HighsStatus, Model};

use crate::backend::BinaryBackend;
use crate::config::{BackendKind, BinaryIlpConfig};
use crate::error::BinaryIlpError;
use crate::model::{BinaryIlpModel, ConstraintSense, ModelSolution, ModelSolutionStatus};

#[derive(Debug)]
pub struct HighsBinaryBackend {
    model: Option<Model>,
    row_senses: Vec<ConstraintSense>,
    row_structural_bounds: Vec<(f64, f64)>,
    solution_binary_prefix_len: usize,
}

impl HighsBinaryBackend {
    pub fn new(problem: &BinaryIlpModel, config: &BinaryIlpConfig) -> Result<Self, BinaryIlpError> {
        let mut col_problem = ColProblem::new();
        let mut rows = Vec::with_capacity(problem.constraints.len());
        let mut row_senses = Vec::with_capacity(problem.constraints.len());
        let mut row_structural_bounds = Vec::with_capacity(problem.constraints.len());

        for constraint in &problem.constraints {
            let (lower, upper) = row_bounds(constraint.sense, constraint.rhs);
            let row = match constraint.sense {
                ConstraintSense::Eq => col_problem.add_row(constraint.rhs..=constraint.rhs),
                ConstraintSense::Le => col_problem.add_row(..=constraint.rhs),
                ConstraintSense::Ge => col_problem.add_row(constraint.rhs..),
            };
            rows.push(row);
            row_senses.push(constraint.sense);
            row_structural_bounds.push((lower, upper));
        }

        for (column_index, var) in problem.binary_vars.iter().enumerate() {
            let mut entries = Vec::new();
            for (row_index, row) in problem.constraints.iter().enumerate() {
                for &(index, coeff) in &row.binary_terms {
                    if index == column_index {
                        entries.push((rows[row_index], coeff));
                    }
                }
            }
            col_problem.add_integer_column(var.objective, var.lower..=var.upper, entries);
        }

        for (column_index, var) in problem.integer_vars.iter().enumerate() {
            let mut entries = Vec::new();
            for (row_index, row) in problem.constraints.iter().enumerate() {
                for &(index, coeff) in &row.integer_terms {
                    if index == column_index {
                        entries.push((rows[row_index], coeff));
                    }
                }
            }
            col_problem.add_integer_column(var.objective, var.lower..=var.upper, entries);
        }

        let mut model = Model::try_new(col_problem)
            .map_err(|err| BinaryIlpError::Highs(format!("failed to create model: {err:?}")))?;
        if !config.backend.verbose {
            model.make_quiet();
        }
        if let Some(threads) = config.backend.threads.and_then(NonZeroU32::new) {
            model.set_threads(threads);
        }
        if let Some(limit) = config.backend.time_limit_seconds {
            model.try_set_option("time_limit", limit).map_err(|err| {
                BinaryIlpError::Highs(format!("failed to set time_limit option: {err:?}"))
            })?;
        }
        if let Some(gap) = config.backend.mip_gap {
            model.try_set_option("mip_rel_gap", gap).map_err(|err| {
                BinaryIlpError::Highs(format!("failed to set mip_rel_gap option: {err:?}"))
            })?;
        }

        Ok(Self {
            model: Some(model),
            row_senses,
            row_structural_bounds,
            solution_binary_prefix_len: problem.solution_binary_prefix_len,
        })
    }
}

impl BinaryBackend for HighsBinaryBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Highs
    }

    fn solve(&mut self) -> Result<ModelSolution, BinaryIlpError> {
        let model = self
            .model
            .as_mut()
            .ok_or_else(|| BinaryIlpError::Highs("model already in use".to_string()))?;
        let status = unsafe { highs_sys::Highs_run(model.as_mut_ptr()) };
        accept_call_status(status, "solve failed")?;
        let model_status = read_model_status(model)?;
        let primal_status = read_primal_solution_status(model)?;
        let solution_status =
            accepted_model_solution_status(model_status, primal_status).ok_or_else(|| {
                BinaryIlpError::Highs(format!(
                    "unexpected HiGHS solve status: model={model_status:?}, primal={primal_status:?}"
                ))
            })?;

        if solution_status == ModelSolutionStatus::Infeasible {
            return Ok(ModelSolution {
                binary_values: Vec::new(),
                status: solution_status,
            });
        }

        let columns = read_solution_columns(model)?;

        if columns.len() < self.solution_binary_prefix_len {
            return Err(BinaryIlpError::Highs(format!(
                "solution width mismatch: expected at least {}, got {}",
                self.solution_binary_prefix_len,
                columns.len()
            )));
        }

        Ok(ModelSolution {
            binary_values: columns[..self.solution_binary_prefix_len]
                .iter()
                .map(|&value| value > 0.5)
                .collect(),
            status: solution_status,
        })
    }

    fn set_rhs(&mut self, row: usize, rhs: f64) -> Result<(), BinaryIlpError> {
        let sense = *self
            .row_senses
            .get(row)
            .ok_or(BinaryIlpError::UnknownConstraintRow(row))?;
        let (structural_lower, structural_upper) = self
            .row_structural_bounds
            .get(row)
            .copied()
            .ok_or(BinaryIlpError::UnknownConstraintRow(row))?;
        let (lower, upper) = match sense {
            ConstraintSense::Eq => (rhs, rhs),
            ConstraintSense::Le => (structural_lower, rhs),
            ConstraintSense::Ge => (rhs, structural_upper),
        };

        let model = self
            .model
            .as_mut()
            .ok_or_else(|| BinaryIlpError::Highs("model already in use".to_string()))?;
        let status = unsafe {
            highs_sys::Highs_changeRowBounds(model.as_mut_ptr(), row as c_int, lower, upper)
        };
        if status != highs_sys::STATUS_OK {
            return Err(BinaryIlpError::Highs(format!(
                "failed to set row bounds for row {row}: status {status}"
            )));
        }

        Ok(())
    }
}

fn row_bounds(sense: ConstraintSense, rhs: f64) -> (f64, f64) {
    match sense {
        ConstraintSense::Eq => (rhs, rhs),
        ConstraintSense::Le => (f64::NEG_INFINITY, rhs),
        ConstraintSense::Ge => (rhs, f64::INFINITY),
    }
}

fn accept_call_status(status: c_int, context: &str) -> Result<(), BinaryIlpError> {
    match HighsStatus::try_from(status) {
        Ok(HighsStatus::OK | HighsStatus::Warning) => Ok(()),
        Ok(other) => Err(BinaryIlpError::Highs(format!("{context}: {other:?}"))),
        Err(_) => Err(BinaryIlpError::Highs(format!(
            "{context}: unexpected raw status {status}"
        ))),
    }
}

fn read_model_status(model: &mut Model) -> Result<HighsModelStatus, BinaryIlpError> {
    let status = unsafe { highs_sys::Highs_getModelStatus(model.as_mut_ptr()) };
    HighsModelStatus::try_from(status)
        .map_err(|_| BinaryIlpError::Highs(format!("unexpected HiGHS model status value {status}")))
}

fn read_primal_solution_status(model: &mut Model) -> Result<HighsSolutionStatus, BinaryIlpError> {
    let name = CString::new("primal_solution_status")
        .map_err(|_| BinaryIlpError::Highs("invalid HiGHS info key".to_string()))?;
    let mut solution_status = -1;
    let status = unsafe {
        highs_sys::Highs_getIntInfoValue(model.as_mut_ptr(), name.as_ptr(), &mut solution_status)
    };
    accept_call_status(status, "failed to read primal solution status")?;
    HighsSolutionStatus::try_from(solution_status).map_err(|_| {
        BinaryIlpError::Highs(format!(
            "unexpected HiGHS primal solution status value {solution_status}"
        ))
    })
}

fn read_solution_columns(model: &mut Model) -> Result<Vec<f64>, BinaryIlpError> {
    let cols = model.num_cols();
    let rows = model.num_rows();
    let mut colvalue = vec![0.0; cols];
    let mut coldual = vec![0.0; cols];
    let mut rowvalue = vec![0.0; rows];
    let mut rowdual = vec![0.0; rows];
    unsafe {
        highs_sys::Highs_getSolution(
            model.as_mut_ptr(),
            colvalue.as_mut_ptr(),
            coldual.as_mut_ptr(),
            rowvalue.as_mut_ptr(),
            rowdual.as_mut_ptr(),
        );
    }
    Ok(colvalue)
}

fn accepted_model_solution_status(
    model_status: HighsModelStatus,
    primal_status: HighsSolutionStatus,
) -> Option<ModelSolutionStatus> {
    match model_status {
        HighsModelStatus::Optimal => Some(ModelSolutionStatus::Optimal),
        HighsModelStatus::Infeasible => Some(ModelSolutionStatus::Infeasible),
        HighsModelStatus::ReachedTimeLimit if primal_status == HighsSolutionStatus::Feasible => {
            Some(ModelSolutionStatus::TimeLimit)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use highs::{HighsModelStatus, HighsSolutionStatus};

    use super::accepted_model_solution_status;
    use crate::model::ModelSolutionStatus;

    #[test]
    fn maps_optimal_solution_status() {
        assert_eq!(
            accepted_model_solution_status(
                HighsModelStatus::Optimal,
                HighsSolutionStatus::Feasible
            ),
            Some(ModelSolutionStatus::Optimal),
        );
    }

    #[test]
    fn maps_time_limited_feasible_solution_status() {
        assert_eq!(
            accepted_model_solution_status(
                HighsModelStatus::ReachedTimeLimit,
                HighsSolutionStatus::Feasible,
            ),
            Some(ModelSolutionStatus::TimeLimit),
        );
    }

    #[test]
    fn maps_infeasible_solution_status_without_a_primal_solution() {
        assert_eq!(
            accepted_model_solution_status(
                HighsModelStatus::Infeasible,
                HighsSolutionStatus::Infeasible,
            ),
            Some(ModelSolutionStatus::Infeasible),
        );
    }

    #[test]
    fn rejects_time_limited_run_without_feasible_solution() {
        assert_eq!(
            accepted_model_solution_status(
                HighsModelStatus::ReachedTimeLimit,
                HighsSolutionStatus::None,
            ),
            None,
        );
    }

    #[test]
    fn rejects_other_non_optimal_statuses_even_with_feasible_solution() {
        assert_eq!(
            accepted_model_solution_status(
                HighsModelStatus::ReachedInterrupt,
                HighsSolutionStatus::Feasible,
            ),
            None,
        );
    }
}
