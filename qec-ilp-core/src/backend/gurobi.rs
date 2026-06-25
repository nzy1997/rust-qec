use gurobi::{Constr, ConstrSense, Env, Model, Status, Var, VarType, attr, param};

use crate::backend::BinaryBackend;
use crate::config::{BackendKind, BinaryIlpConfig};
use crate::error::BinaryIlpError;
use crate::model::{BinaryIlpModel, ConstraintSense, ModelSolution, ModelSolutionStatus};

pub struct GurobiBinaryBackend {
    _env: Env,
    model: Model,
    row_constraints: Vec<Constr>,
    solution_vars: Vec<Var>,
}

impl GurobiBinaryBackend {
    pub fn new(problem: &BinaryIlpModel, config: &BinaryIlpConfig) -> Result<Self, BinaryIlpError> {
        let mut env = Env::new("").map_err(gurobi_error)?;
        if !config.backend.verbose {
            env.set(param::OutputFlag, 0).map_err(gurobi_error)?;
        }
        if let Some(threads) = config.backend.threads {
            env.set(param::Threads, threads as i32)
                .map_err(gurobi_error)?;
        }
        if let Some(limit) = config.backend.time_limit_seconds {
            env.set(param::TimeLimit, limit).map_err(gurobi_error)?;
        }
        if let Some(gap) = config.backend.mip_gap {
            env.set(param::MIPGap, gap).map_err(gurobi_error)?;
        }

        let mut model = Model::new("qec-ilp-core", &env).map_err(gurobi_error)?;

        let row_constraints = problem
            .constraints
            .iter()
            .map(|constraint| {
                model
                    .add_constr(
                        &constraint.name,
                        0.0.into(),
                        gurobi_constraint_sense(constraint.sense),
                        constraint.rhs,
                    )
                    .map_err(gurobi_error)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut binary_vars = Vec::with_capacity(problem.binary_vars.len());
        for (column_index, var) in problem.binary_vars.iter().enumerate() {
            let (constrs, coeffs) =
                collect_constraint_refs(&row_constraints, &problem.constraints, column_index, true);
            let handle = model
                .add_var(
                    &var.name,
                    VarType::Binary,
                    var.objective,
                    var.lower,
                    var.upper,
                    &constrs,
                    &coeffs,
                )
                .map_err(gurobi_error)?;
            binary_vars.push(handle);
        }

        for (column_index, var) in problem.integer_vars.iter().enumerate() {
            let (constrs, coeffs) = collect_constraint_refs(
                &row_constraints,
                &problem.constraints,
                column_index,
                false,
            );
            model
                .add_var(
                    &var.name,
                    VarType::Integer,
                    var.objective,
                    var.lower,
                    var.upper,
                    &constrs,
                    &coeffs,
                )
                .map_err(gurobi_error)?;
        }

        model.update().map_err(gurobi_error)?;

        Ok(Self {
            _env: env,
            model,
            row_constraints,
            solution_vars: binary_vars[..problem.solution_binary_prefix_len].to_vec(),
        })
    }
}

impl BinaryBackend for GurobiBinaryBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Gurobi
    }

    fn solve(&mut self) -> Result<ModelSolution, BinaryIlpError> {
        self.model.optimize().map_err(gurobi_error)?;
        let status = self.model.status().map_err(gurobi_error)?;
        let sol_count = self.model.get(attr::SolCount).map_err(gurobi_error)?;
        let solution_status =
            accepted_gurobi_solution_status(status, sol_count).ok_or_else(|| {
                BinaryIlpError::Gurobi(format!(
                    "unexpected Gurobi solve status: status={status:?}, sol_count={sol_count}"
                ))
            })?;

        let values = self
            .model
            .get_values(attr::X, &self.solution_vars)
            .map_err(gurobi_error)?;

        Ok(ModelSolution {
            binary_values: values.into_iter().map(|value| value > 0.5).collect(),
            status: solution_status,
        })
    }

    fn set_rhs(&mut self, row: usize, rhs: f64) -> Result<(), BinaryIlpError> {
        let constraint = self
            .row_constraints
            .get(row)
            .ok_or(BinaryIlpError::UnknownConstraintRow(row))?;
        constraint
            .set(&mut self.model, attr::RHS, rhs)
            .map_err(gurobi_error)?;
        Ok(())
    }
}

fn collect_constraint_refs(
    row_constraints: &[Constr],
    constraints: &[crate::model::LinearConstraint],
    column_index: usize,
    is_binary: bool,
) -> (Vec<Constr>, Vec<f64>) {
    let mut constrs = Vec::new();
    let mut coeffs = Vec::new();
    for (row_index, row) in constraints.iter().enumerate() {
        let terms = if is_binary {
            &row.binary_terms
        } else {
            &row.integer_terms
        };
        for &(index, coeff) in terms {
            if index == column_index {
                constrs.push(row_constraints[row_index].clone());
                coeffs.push(coeff);
            }
        }
    }
    (constrs, coeffs)
}

fn gurobi_constraint_sense(sense: ConstraintSense) -> ConstrSense {
    match sense {
        ConstraintSense::Eq => ConstrSense::Equal,
        ConstraintSense::Le => ConstrSense::Less,
        ConstraintSense::Ge => ConstrSense::Greater,
    }
}

fn accepted_gurobi_solution_status(status: Status, sol_count: i32) -> Option<ModelSolutionStatus> {
    match status {
        Status::Optimal => Some(ModelSolutionStatus::Optimal),
        Status::TimeLimit if sol_count > 0 => Some(ModelSolutionStatus::TimeLimit),
        Status::SolutionLimit if sol_count > 0 => Some(ModelSolutionStatus::SolutionLimit),
        Status::SubOptimal if sol_count > 0 => Some(ModelSolutionStatus::SubOptimal),
        _ => None,
    }
}

fn gurobi_error(err: gurobi::Error) -> BinaryIlpError {
    BinaryIlpError::Gurobi(err.to_string())
}

#[cfg(test)]
mod tests {
    use gurobi::Status;

    use super::accepted_gurobi_solution_status;
    use crate::model::ModelSolutionStatus;

    #[test]
    fn maps_gurobi_optimal_status_without_solution_count_check() {
        assert_eq!(
            accepted_gurobi_solution_status(Status::Optimal, 0),
            Some(ModelSolutionStatus::Optimal),
        );
    }

    #[test]
    fn maps_gurobi_time_limit_with_incumbent() {
        assert_eq!(
            accepted_gurobi_solution_status(Status::TimeLimit, 1),
            Some(ModelSolutionStatus::TimeLimit),
        );
    }

    #[test]
    fn maps_gurobi_solution_limit_with_incumbent() {
        assert_eq!(
            accepted_gurobi_solution_status(Status::SolutionLimit, 1),
            Some(ModelSolutionStatus::SolutionLimit),
        );
    }

    #[test]
    fn maps_gurobi_suboptimal_with_incumbent() {
        assert_eq!(
            accepted_gurobi_solution_status(Status::SubOptimal, 1),
            Some(ModelSolutionStatus::SubOptimal),
        );
    }

    #[test]
    fn rejects_time_limited_run_without_incumbent() {
        assert_eq!(accepted_gurobi_solution_status(Status::TimeLimit, 0), None);
    }
}
