// Compile-only downstream API witness for four standalone-header equivalents.
// Each module imports its own public dependencies; no Python or private modules.
#![allow(dead_code)]
mod reference_network {
    use rmagna::instrument::{
        OriginalError, OriginalNetworkProbability, OriginalRecordParity, OriginalStaticTrace,
    };
    use rmagna::scalar::ScalarArithmetic;
    pub fn prepare<'a>(
        trace: &'a OriginalStaticTrace,
        rows: &[OriginalRecordParity],
        a: &ScalarArithmetic,
    ) -> Result<OriginalNetworkProbability<'a>, OriginalError> {
        trace.prepare_network_probability(rows, &Default::default(), a)
    }
}
mod affine_proposal {
    use rmagna::gf2::BitVector;
    use rmagna::instrument::{OriginalAffineRecordProposal, OriginalAssignmentPlan, OriginalError};
    pub fn prepare(
        plan: &OriginalAssignmentPlan,
        rows: &[BitVector],
        values: &BitVector,
    ) -> Result<OriginalAffineRecordProposal, OriginalError> {
        OriginalAffineRecordProposal::new(
            plan,
            rows,
            values,
            "caller-supplied-physical-coverage",
            Default::default(),
        )
    }
}
mod file_execution {
    use rmagna::file_api::{FileApiError, FileRunReport, FileTask};
    use rmagna::task::TaskRunOptions;
    pub fn run(task: &FileTask, options: &TaskRunOptions) -> Result<FileRunReport, FileApiError> {
        task.run(options)
    }
}
mod file_enumeration {
    use rmagna::file_api::{FileApiError, FileEnumerationOptions, FileEnumerationReport, FileTask};
    pub fn enumerate(
        task: &FileTask,
        options: &FileEnumerationOptions,
    ) -> Result<FileEnumerationReport, FileApiError> {
        task.enumerate(options)
    }
}
pub fn main() {
    let _ = reference_network::prepare;
    let _ = affine_proposal::prepare;
    let _ = file_execution::run;
    let _ = file_enumeration::enumerate;
}
