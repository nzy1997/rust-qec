//! Numeric prefix diagnostics and owned file-run observation receipts.
use super::*;
use crate::task::{BatchProgress, NumericEncoder, ProgressReceipt, fingerprint_with_configuration};
use std::num::NonZeroU64;

/// Numeric diagnostics can be unavailable without stopping physical sampling.
/// The caller still owns the exact typed event, including an arithmetic error.
pub fn export_progress(progress: &BatchProgress, numeric: NumericOptions) -> Value {
    let mut encoder = NumericEncoder::new(numeric);
    let (efficiency, failure) = match &progress.efficiency {
        Ok(efficiency) => match encoder.efficiency(efficiency) {
            Ok(value) => (value, Value::Null),
            Err(error) => (
                Value::Null,
                json!({"phase":"numeric_export","message":error.to_string()}),
            ),
        },
        Err(error) => (
            Value::Null,
            json!({"phase":"efficiency","message":error.to_string()}),
        ),
    };
    json!({"schema_version":2,"kind":"progress","completed":progress.completed,"requested":progress.requested,
        "history_sampler":match progress.history_sampler {crate::runtime::HistorySampler::AffineSupport=>"affine_support",crate::runtime::HistorySampler::ExactMarginal=>"exact_marginal"},
        "efficiency":efficiency,"diagnostic_failure":failure,
        "execution_costs":costs_json(progress.execution_costs),"sampling_seconds":progress.sampling_seconds,
        "numeric_diagnostics":encoder.numerics()})
}
pub(super) fn attach_receipt(metadata: &mut Value, receipt: ProgressReceipt) {
    metadata["progress"] = json!({"every":receipt.every.get(),"attempted":receipt.attempted,"succeeded":receipt.succeeded,
        "failed":receipt.failed,"diagnostic_failures":receipt.diagnostic_failures,"last_completed":receipt.last_completed,
        "seconds":receipt.seconds,"timing":"diagnostics_and_observers_excluded_from_sampling_included_in_total_and_deadline"});
}
impl FileTask {
    pub(super) fn progress_descriptor(
        &self,
        run: &TaskRunOptions,
        every: Option<NonZeroU64>,
    ) -> Result<Value, FileApiError> {
        let Some(every) = every else {
            return self.descriptor(run);
        };
        let task = self.inputs.modules.task();
        run.validate(task.sample_records().len(), task.observables().len())?;
        let input = self.fingerprint_input(run);
        let mut configured = resolved_options(&input);
        configured["run"]["progress_every"] = json!(every.get());
        let fingerprints = fingerprint_with_configuration(
            &input,
            &configured,
            &sampling_configuration(&self.inputs.sampling),
            self.inputs.options.fingerprints,
        )?;
        Ok(self.descriptor_value(configured, fingerprints))
    }
}
