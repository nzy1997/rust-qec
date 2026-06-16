use crate::error::{QecError, Result};
use crate::gf2::BinaryRow;
use crate::{Pauli, StabilizerCode};

#[derive(Debug, Clone)]
pub struct LoweredDistanceProblem {
    pub model: qec_ilp_core::BinaryIlpModel,
    pub stabilizer_var_count: usize,
    pub logical_var_count: usize,
    pub symplectic_var_offset: usize,
    pub qubit_activity_offset: usize,
    pub nonzero_logical_constraint_row: Option<qec_ilp_core::LinearConstraint>,
}

pub fn lower_distance_problem(code: &StabilizerCode) -> Result<LoweredDistanceProblem> {
    if code.num_logical_qubits() == 0 {
        return Err(QecError::DistanceWitnessNotFound);
    }

    let stabilizer_rows = code.stabilizer_rows();
    let basis = code.canonical_logical_basis()?;
    let logical_rows = basis
        .logical_x
        .iter()
        .chain(&basis.logical_z)
        .map(Pauli::to_symplectic_row)
        .collect::<Vec<_>>();
    let width = code
        .n()
        .checked_mul(2)
        .ok_or(QecError::UnsupportedExhaustiveEnumeration { n: code.n() })?;
    let stabilizer_var_count = stabilizer_rows.len();
    let logical_var_count = logical_rows.len();
    let symplectic_var_offset = stabilizer_var_count + logical_var_count;
    let qubit_activity_offset = symplectic_var_offset + width;

    let mut binary_vars = Vec::new();
    for i in 0..stabilizer_var_count {
        binary_vars.push(qec_ilp_core::ModelVar {
            name: format!("lambda_{i}"),
            objective: 0.0,
            lower: 0.0,
            upper: 1.0,
        });
    }
    for i in 0..logical_var_count {
        binary_vars.push(qec_ilp_core::ModelVar {
            name: format!("logical_{i}"),
            objective: 0.0,
            lower: 0.0,
            upper: 1.0,
        });
    }
    for c in 0..width {
        binary_vars.push(qec_ilp_core::ModelVar {
            name: format!("p_{c}"),
            objective: 0.0,
            lower: 0.0,
            upper: 1.0,
        });
    }
    for q in 0..code.n() {
        binary_vars.push(qec_ilp_core::ModelVar {
            name: format!("y_{q}"),
            objective: 1.0,
            lower: 0.0,
            upper: 1.0,
        });
    }

    let integer_vars = (0..width)
        .map(|c| qec_ilp_core::ModelVar {
            name: format!("t_{c}"),
            objective: 0.0,
            lower: 0.0,
            upper: f64::INFINITY,
        })
        .collect::<Vec<_>>();

    let mut constraints = (0..width)
        .map(|c| coordinate_parity_row(c, &stabilizer_rows, &logical_rows, symplectic_var_offset))
        .collect::<Vec<_>>();

    let logical_terms = (stabilizer_var_count..(stabilizer_var_count + logical_var_count))
        .map(|index| (index, 1.0))
        .collect::<Vec<_>>();
    let nonzero_logical_constraint_row = qec_ilp_core::LinearConstraint {
        name: "logical_nonzero".into(),
        sense: qec_ilp_core::ConstraintSense::Ge,
        binary_terms: logical_terms,
        integer_terms: vec![],
        rhs: 1.0,
    };
    constraints.push(nonzero_logical_constraint_row.clone());

    for qubit in 0..code.n() {
        constraints.extend(weight_rows_for_qubit(
            qubit,
            symplectic_var_offset,
            qubit_activity_offset,
            code.n(),
        ));
    }

    Ok(LoweredDistanceProblem {
        model: qec_ilp_core::BinaryIlpModel {
            binary_vars,
            integer_vars,
            constraints,
            solution_binary_prefix_len: qubit_activity_offset + code.n(),
        },
        stabilizer_var_count,
        logical_var_count,
        symplectic_var_offset,
        qubit_activity_offset,
        nonzero_logical_constraint_row: Some(nonzero_logical_constraint_row),
    })
}

fn coordinate_parity_row(
    coord: usize,
    stabilizers: &[BinaryRow],
    logicals: &[BinaryRow],
    symplectic_var_offset: usize,
) -> qec_ilp_core::LinearConstraint {
    let mut binary_terms = Vec::new();
    for (index, row) in stabilizers.iter().enumerate() {
        if row[coord] == 1 {
            binary_terms.push((index, 1.0));
        }
    }
    for (index, row) in logicals.iter().enumerate() {
        if row[coord] == 1 {
            binary_terms.push((stabilizers.len() + index, 1.0));
        }
    }
    binary_terms.push((symplectic_var_offset + coord, -1.0));

    qec_ilp_core::LinearConstraint {
        name: format!("coord_{coord}"),
        sense: qec_ilp_core::ConstraintSense::Eq,
        binary_terms,
        integer_terms: vec![(coord, -2.0)],
        rhs: 0.0,
    }
}

fn weight_rows_for_qubit(
    qubit: usize,
    symplectic_var_offset: usize,
    qubit_activity_offset: usize,
    n: usize,
) -> Vec<qec_ilp_core::LinearConstraint> {
    let x_index = symplectic_var_offset + qubit;
    let z_index = symplectic_var_offset + n + qubit;
    let y_index = qubit_activity_offset + qubit;

    vec![
        qec_ilp_core::LinearConstraint {
            name: format!("weight_x_{qubit}"),
            sense: qec_ilp_core::ConstraintSense::Le,
            binary_terms: vec![(x_index, 1.0), (y_index, -1.0)],
            integer_terms: vec![],
            rhs: 0.0,
        },
        qec_ilp_core::LinearConstraint {
            name: format!("weight_z_{qubit}"),
            sense: qec_ilp_core::ConstraintSense::Le,
            binary_terms: vec![(z_index, 1.0), (y_index, -1.0)],
            integer_terms: vec![],
            rhs: 0.0,
        },
        qec_ilp_core::LinearConstraint {
            name: format!("weight_or_{qubit}"),
            sense: qec_ilp_core::ConstraintSense::Le,
            binary_terms: vec![(y_index, 1.0), (x_index, -1.0), (z_index, -1.0)],
            integer_terms: vec![],
            rhs: 0.0,
        },
    ]
}
