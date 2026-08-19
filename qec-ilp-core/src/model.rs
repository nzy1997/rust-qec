use crate::error::BinaryIlpError;

#[derive(Debug, Clone, PartialEq)]
pub struct ModelVar {
    pub name: String,
    pub objective: f64,
    pub lower: f64,
    pub upper: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintSense {
    Eq,
    Le,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearConstraint {
    pub name: String,
    pub sense: ConstraintSense,
    pub binary_terms: Vec<(usize, f64)>,
    pub integer_terms: Vec<(usize, f64)>,
    pub rhs: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinaryIlpModel {
    pub binary_vars: Vec<ModelVar>,
    pub integer_vars: Vec<ModelVar>,
    pub constraints: Vec<LinearConstraint>,
    pub solution_binary_prefix_len: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelSolution {
    pub binary_values: Vec<bool>,
    pub status: ModelSolutionStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSolutionStatus {
    Optimal,
    Infeasible,
    TimeLimit,
    SolutionLimit,
    SubOptimal,
}

impl BinaryIlpModel {
    pub fn validate(&self) -> Result<(), BinaryIlpError> {
        for (index, var) in self.binary_vars.iter().enumerate() {
            if !is_binary_bound(var.lower) || !is_binary_bound(var.upper) || var.lower > var.upper {
                return Err(BinaryIlpError::InvalidBinaryVarBounds {
                    index,
                    lower: var.lower,
                    upper: var.upper,
                });
            }
        }

        for row in &self.constraints {
            for &(index, _) in &row.binary_terms {
                if index >= self.binary_vars.len() {
                    return Err(BinaryIlpError::UnknownBinaryVar(index));
                }
            }
            for &(index, _) in &row.integer_terms {
                if index >= self.integer_vars.len() {
                    return Err(BinaryIlpError::UnknownIntegerVar(index));
                }
            }
        }

        if self.solution_binary_prefix_len > self.binary_vars.len() {
            return Err(BinaryIlpError::UnknownBinaryVar(
                self.solution_binary_prefix_len,
            ));
        }

        Ok(())
    }
}

fn is_binary_bound(value: f64) -> bool {
    value == 0.0 || value == 1.0
}
