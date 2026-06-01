use gurobi::{attr, param, Constr, ConstrSense, Env, Model, Status, Var, VarType};

use crate::backend::BatchBackend;
use crate::config::IlpDecoderConfig;
use crate::error::IlpDecodeError;
use crate::problem::LoweredDemProblem;

pub struct GurobiBatchBackend {
    _env: Env,
    model: Model,
    row_constraints: Vec<Constr>,
    error_vars: Vec<Var>,
    num_detectors: usize,
    forced_syndrome: Vec<bool>,
}

impl GurobiBatchBackend {
    pub fn new(
        problem: &LoweredDemProblem,
        config: &IlpDecoderConfig,
    ) -> Result<Self, IlpDecodeError> {
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

        let mut model = Model::new("rilpqec", &env).map_err(gurobi_error)?;

        let row_constraints = (0..problem.num_detectors)
            .map(|row| {
                model
                    .add_constr(&format!("det_{row}"), 0.0.into(), ConstrSense::Equal, 0.0)
                    .map_err(gurobi_error)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let error_vars = problem
            .columns
            .iter()
            .enumerate()
            .map(|(column_index, column)| {
                let constrs = column
                    .detectors
                    .iter()
                    .map(|&det| row_constraints[det].clone())
                    .collect::<Vec<_>>();
                let coeffs = vec![1.0; constrs.len()];
                model
                    .add_var(
                        &format!("e_{column_index}"),
                        VarType::Binary,
                        column.weight,
                        0.0,
                        1.0,
                        &constrs,
                        &coeffs,
                    )
                    .map_err(gurobi_error)
            })
            .collect::<Result<Vec<_>, _>>()?;

        for (row, constr) in row_constraints.iter().enumerate() {
            model
                .add_var(
                    &format!("a_{row}"),
                    VarType::Integer,
                    0.0,
                    0.0,
                    gurobi::INFINITY,
                    std::slice::from_ref(constr),
                    &[-2.0],
                )
                .map_err(gurobi_error)?;
        }

        model.update().map_err(gurobi_error)?;

        Ok(Self {
            _env: env,
            model,
            row_constraints,
            error_vars,
            num_detectors: problem.num_detectors,
            forced_syndrome: problem.forced_syndrome.clone(),
        })
    }
}

impl BatchBackend for GurobiBatchBackend {
    fn solve(&mut self, syndrome: &[bool]) -> Result<Vec<bool>, IlpDecodeError> {
        if syndrome.len() != self.num_detectors {
            return Err(IlpDecodeError::DetectorWidthMismatch {
                expected: self.num_detectors,
                actual: syndrome.len(),
            });
        }

        for (row, (&bit, &forced)) in syndrome.iter().zip(&self.forced_syndrome).enumerate() {
            let rhs = if bit ^ forced { 1.0 } else { 0.0 };
            self.row_constraints[row]
                .set(&mut self.model, attr::RHS, rhs)
                .map_err(gurobi_error)?;
        }

        self.model.optimize().map_err(gurobi_error)?;
        let status = self.model.status().map_err(gurobi_error)?;
        let sol_count = self.model.get(attr::SolCount).map_err(gurobi_error)?;
        if !accept_gurobi_status(status, sol_count) {
            return Err(IlpDecodeError::Gurobi(format!(
                "unexpected Gurobi solve status: status={status:?}, sol_count={sol_count}"
            )));
        }

        let values = self
            .model
            .get_values(attr::X, &self.error_vars)
            .map_err(gurobi_error)?;

        Ok(values.into_iter().map(|value| value > 0.5).collect())
    }
}

fn accept_gurobi_status(status: Status, sol_count: i32) -> bool {
    match status {
        Status::Optimal => true,
        Status::TimeLimit | Status::SolutionLimit | Status::SubOptimal => sol_count > 0,
        _ => false,
    }
}

fn gurobi_error(err: gurobi::Error) -> IlpDecodeError {
    IlpDecodeError::Gurobi(err.to_string())
}

#[cfg(test)]
mod tests {
    use gurobi::Status;

    use super::accept_gurobi_status;

    #[test]
    fn accepts_optimal_status_without_solution_count_check() {
        assert!(accept_gurobi_status(Status::Optimal, 0));
    }

    #[test]
    fn accepts_time_limited_run_with_incumbent() {
        assert!(accept_gurobi_status(Status::TimeLimit, 1));
    }

    #[test]
    fn rejects_time_limited_run_without_incumbent() {
        assert!(!accept_gurobi_status(Status::TimeLimit, 0));
    }
}
