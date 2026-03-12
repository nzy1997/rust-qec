use crate::ir::StimInstr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceSampleMode {
    SimulateNoiseless,
    AssumeAllZero,
}

impl Default for ReferenceSampleMode {
    fn default() -> Self {
        Self::SimulateNoiseless
    }
}

pub fn build_reference_sample(
    instrs: &[StimInstr],
    mode: ReferenceSampleMode,
) -> Result<Vec<bool>, String> {
    match mode {
        ReferenceSampleMode::SimulateNoiseless => crate::executor::reference_sample(instrs),
        ReferenceSampleMode::AssumeAllZero => Ok(vec![false; crate::stats::num_measurements(instrs)]),
    }
}
