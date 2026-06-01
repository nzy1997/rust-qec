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
    pub fn observables_from_correction(&self, correction: &[bool]) -> Vec<bool> {
        let mut out = self.baseline_observables.clone();
        for (column, enabled) in self.columns.iter().zip(correction.iter().copied()) {
            if !enabled {
                continue;
            }
            for &obs in &column.observables {
                out[obs] ^= true;
            }
        }
        out
    }
}
