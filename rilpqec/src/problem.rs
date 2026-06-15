use crate::error::IlpDecodeError;

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnTerm {
    pub detectors: Vec<usize>,
    pub observables: Vec<usize>,
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoweredDemProblem {
    pub num_detectors: usize,
    pub num_observables: usize,
    pub columns: Vec<ColumnTerm>,
    pub forced_syndrome: Vec<bool>,
    pub baseline_observables: Vec<bool>,
}

impl LoweredDemProblem {
    pub fn to_binary_ilp_model(&self) -> Result<qec_ilp_core::BinaryIlpModel, IlpDecodeError> {
        let binary_vars = self
            .columns
            .iter()
            .enumerate()
            .map(|(index, column)| qec_ilp_core::ModelVar {
                name: format!("e_{index}"),
                objective: column.weight,
                lower: 0.0,
                upper: 1.0,
            })
            .collect::<Vec<_>>();

        let integer_vars = (0..self.num_detectors)
            .map(|row| qec_ilp_core::ModelVar {
                name: format!("a_{row}"),
                objective: 0.0,
                lower: 0.0,
                upper: f64::INFINITY,
            })
            .collect::<Vec<_>>();

        let mut row_binary_terms = vec![Vec::new(); self.num_detectors];
        for (index, column) in self.columns.iter().enumerate() {
            for &row in &column.detectors {
                let terms =
                    row_binary_terms
                        .get_mut(row)
                        .ok_or(IlpDecodeError::DetectorWidthMismatch {
                            expected: self.num_detectors,
                            actual: row + 1,
                        })?;
                terms.push((index, 1.0));
            }
        }

        let constraints = (0..self.num_detectors)
            .map(|row| qec_ilp_core::LinearConstraint {
                name: format!("det_{row}"),
                sense: qec_ilp_core::ConstraintSense::Eq,
                binary_terms: row_binary_terms[row].clone(),
                integer_terms: vec![(row, -2.0)],
                rhs: 0.0,
            })
            .collect::<Vec<_>>();

        Ok(qec_ilp_core::BinaryIlpModel {
            binary_vars,
            integer_vars,
            constraints,
            solution_binary_prefix_len: self.columns.len(),
        })
    }

    pub fn observables_from_correction(
        &self,
        correction: &[bool],
    ) -> Result<Vec<bool>, IlpDecodeError> {
        if correction.len() != self.columns.len() {
            return Err(IlpDecodeError::CorrectionWidthMismatch {
                expected: self.columns.len(),
                actual: correction.len(),
            });
        }

        let mut out = self.baseline_observables.clone();
        for (column, enabled) in self.columns.iter().zip(correction.iter().copied()) {
            if !enabled {
                continue;
            }
            for &obs in &column.observables {
                out[obs] ^= true;
            }
        }
        Ok(out)
    }
}
