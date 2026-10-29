//! Independent prepared pilots; never switch or start production sampling.
use super::*;
use crate::runtime::{CausalStrategy, HistorySampler};

#[derive(Clone, Copy, Debug)]
pub struct FileSamplerComparisonOptions {
    /// Equal per-channel counts; affine uses seed, exact uses checked seed + 1.
    pub pilot: TaskRunOptions,
    pub preparation: FilePreparationOptions,
}
impl Default for FileSamplerComparisonOptions {
    fn default() -> Self {
        Self {
            pilot: TaskRunOptions {
                shots: 2,
                ..Default::default()
            },
            preparation: Default::default(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SamplerComparisonStatus {
    Complete,
    Inconclusive,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerDecision {
    pub gamma: f64,
    pub affine_support_seconds_per_draw: f64,
    pub exact_marginal_seconds_per_draw: f64,
    pub recommended_sampler: HistorySampler,
}
impl SamplerDecision {
    /// Compute the native point decision from matched pilot sampling totals.
    ///
    /// Times exclude preparation. Kappa must be finite and nonnegative; both
    /// times and the common completed-draw count must be positive. This pure
    /// calculation neither runs a pilot nor changes a configured sampler, and
    /// carries no confidence guarantee.
    ///
    /// ```
    /// use rmagna::file_api::SamplerDecision;
    /// let decision = SamplerDecision::from_pilot_totals(4.0, 2.0, 4.0, 10)?;
    /// assert_eq!(decision.gamma, 2.0);
    /// # Ok::<(), &'static str>(())
    /// ```
    pub fn from_pilot_totals(
        kappa: f64,
        affine_support_seconds: f64,
        exact_marginal_seconds: f64,
        completed_per_channel: u64,
    ) -> Result<Self, &'static str> {
        decide(
            kappa,
            affine_support_seconds,
            exact_marginal_seconds,
            completed_per_channel,
        )
    }
}
fn decide(
    kappa: f64,
    affine: f64,
    exact: f64,
    count: u64,
) -> Result<SamplerDecision, &'static str> {
    if count == 0
        || !kappa.is_finite()
        || kappa < 0.0
        || !affine.is_finite()
        || affine <= 0.0
        || !exact.is_finite()
        || exact <= 0.0
    {
        return Err("finite nonnegative kappa, positive pilot times and count required");
    }
    // Match the pinned native operation order and tie rule, including range errors.
    let gamma = kappa * (affine / exact);
    if !gamma.is_finite() {
        return Err("pilot Gamma exceeds floating-point range");
    }
    Ok(SamplerDecision {
        gamma,
        affine_support_seconds_per_draw: affine / count as f64,
        exact_marginal_seconds_per_draw: exact / count as f64,
        recommended_sampler: if gamma > 1.0 {
            HistorySampler::ExactMarginal
        } else {
            HistorySampler::AffineSupport
        },
    })
}
#[derive(Debug)]
pub struct FileSamplerPilot {
    history_sampler: HistorySampler,
    seed: u64,
    preparation: FilePreparationReport,
    batch: Option<FileRunReport>,
    /// A metadata error can itself own a completed typed batch.
    failure: Option<FileApiError>,
    export_failure: Option<BatchExportError>,
    exported_batch: Option<Value>,
}
impl FileSamplerPilot {
    pub fn history_sampler(&self) -> HistorySampler {
        self.history_sampler
    }
    pub fn seed(&self) -> u64 {
        self.seed
    }
    pub fn preparation(&self) -> &FilePreparationReport {
        &self.preparation
    }
    pub fn batch(&self) -> Option<&FileRunReport> {
        self.batch.as_ref()
    }
    pub fn failure(&self) -> Option<&FileApiError> {
        self.failure.as_ref()
    }
    pub fn export_failure(&self) -> Option<&BatchExportError> {
        self.export_failure.as_ref()
    }

    pub fn raw_result(&self) -> Option<&TaskRunResult> {
        self.batch
            .as_ref()
            .map(FileRunReport::raw_result)
            .or_else(|| {
                self.failure
                    .as_ref()
                    .and_then(FileApiError::retained_result)
            })
    }
    pub fn is_complete(&self) -> bool {
        self.preparation.is_complete()
            && self.failure.is_none()
            && self.export_failure.is_none()
            && self
                .raw_result()
                .is_some_and(|r| r.status == crate::task::RunStatus::Complete)
    }
    pub fn to_json(&self) -> Value {
        let receipt = self.raw_result().map(|r|json!({"status":if r.status==crate::task::RunStatus::Complete{"complete"}else{"failed"},
            "requested":r.requested,"completed":r.completed,"failed":r.failed,"not_attempted":r.not_attempted,"seed":r.seed,
            "execution_costs":costs_json(r.execution_costs),"sampling_seconds":r.timings.sampling_seconds,
            "estimates_persisted":self.exported_batch.is_some()}));
        json!({"status":if self.is_complete(){"complete"}else{"failed"},
            "history_sampler":sampler_name(self.history_sampler),"seed":self.seed,
            "preparation":self.preparation.to_json(),"sampling":self.exported_batch,
            "sampling_receipt":receipt,
            "failure":self.failure.as_ref().map(|e|e.to_string()),
            "sampling_export_failure":self.export_failure.as_ref().map(|e|e.to_string())})
    }
}
#[derive(Debug)]
pub struct FileSamplerComparison {
    status: SamplerComparisonStatus,
    reason: Option<String>,
    affine_support: FileSamplerPilot,
    exact_marginal: FileSamplerPilot,
    decision: Option<SamplerDecision>,
    pilot_shots_per_channel: u64,
}
impl FileSamplerComparison {
    fn from_pilots(
        affine_support: FileSamplerPilot,
        exact_marginal: FileSamplerPilot,
        count: u64,
    ) -> Self {
        let mut comparison = FileSamplerComparison {
            status: SamplerComparisonStatus::Inconclusive,
            reason: None,
            affine_support,
            exact_marginal,
            decision: None,
            pilot_shots_per_channel: count,
        };
        if !comparison.affine_support.is_complete() || !comparison.exact_marginal.is_complete() {
            comparison.status = SamplerComparisonStatus::Failed;
            comparison.reason = Some("pilot_incomplete".into());
        } else {
            let affine = &comparison.affine_support;
            let exported = affine
                .exported_batch
                .as_ref()
                .expect("complete exported pilot");
            if let Some(kappa) = exported["efficiency"]["kappa"].as_f64() {
                match decide(
                    kappa,
                    affine
                        .raw_result()
                        .expect("completed pilot")
                        .timings
                        .sampling_seconds,
                    comparison
                        .exact_marginal
                        .raw_result()
                        .expect("completed pilot")
                        .timings
                        .sampling_seconds,
                    count,
                ) {
                    Ok(decision) => {
                        comparison.status = SamplerComparisonStatus::Complete;
                        comparison.decision = Some(decision);
                    }
                    Err(reason) => comparison.reason = Some(reason.into()),
                }
            } else {
                comparison.reason = Some(
                    if exported["efficiency"]["status"] == "available" {
                        "kappa_outside_ordinary_binary64_range"
                    } else {
                        exported["efficiency"]["status"]
                            .as_str()
                            .unwrap_or("efficiency_unavailable")
                    }
                    .into(),
                );
            }
        }
        comparison
    }
    pub fn status(&self) -> SamplerComparisonStatus {
        self.status
    }
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
    pub fn affine_support(&self) -> &FileSamplerPilot {
        &self.affine_support
    }
    pub fn exact_marginal(&self) -> &FileSamplerPilot {
        &self.exact_marginal
    }
    pub fn decision(&self) -> Option<SamplerDecision> {
        self.decision
    }
    pub fn pilot_shots_per_channel(&self) -> u64 {
        self.pilot_shots_per_channel
    }

    pub fn to_json(&self) -> Value {
        json!({"schema_version":2,"kind":"sampler_comparison",
            "status":match self.status{SamplerComparisonStatus::Complete=>"complete",SamplerComparisonStatus::Inconclusive=>"inconclusive",SamplerComparisonStatus::Failed=>"failed"},
            "reason":self.reason,"pilot_shots_per_channel":self.pilot_shots_per_channel,
            "pilots":{"affine_support":self.affine_support.to_json(),"exact_marginal":self.exact_marginal.to_json()},
            "gamma":self.decision.map(|d|d.gamma),
            "kappa":self.affine_support.exported_batch.as_ref().map(|v| &v["efficiency"]["kappa"]),
            "recommended_sampler":self.decision.map(|d|sampler_name(d.recommended_sampler)),
            "affine_support_seconds_per_draw":self.decision.map(|d|d.affine_support_seconds_per_draw),
            "exact_marginal_seconds_per_draw":self.decision.map(|d|d.exact_marginal_seconds_per_draw),
            "automatic_switching":false,"production_started":false,
            "interpretation":"Pilot point estimate for matched fidelity precision; excludes preparation. Inspect preparation and resource costs separately. No confidence guarantee or production run."})
    }
}
fn sampler_name(sampler: HistorySampler) -> &'static str {
    match sampler {
        HistorySampler::AffineSupport => "affine_support",
        HistorySampler::ExactMarginal => "exact_marginal",
    }
}
impl FileTask {
    /// Run two independent explicitly prepared pilots. Both channels are attempted
    /// after ordinary pilot failures; caller configuration is never changed.
    pub fn compare_samplers(
        &self,
        options: &FileSamplerComparisonOptions,
    ) -> Result<FileSamplerComparison, FileApiError> {
        let exact_seed = self.validate_comparison(options)?;
        let p = &options.pilot;
        let affine = self.sampler_channel(HistorySampler::AffineSupport);
        let exact = self.sampler_channel(HistorySampler::ExactMarginal);
        let mut exact_run = *p;
        exact_run.seed = exact_seed;
        let affine_support = run_pilot(
            &affine,
            HistorySampler::AffineSupport,
            p,
            &options.preparation,
        );
        let exact_marginal = run_pilot(
            &exact,
            HistorySampler::ExactMarginal,
            &exact_run,
            &options.preparation,
        );
        Ok(FileSamplerComparison::from_pilots(
            affine_support,
            exact_marginal,
            p.shots,
        ))
    }
    fn validate_comparison(
        &self,
        options: &FileSamplerComparisonOptions,
    ) -> Result<u64, FileApiError> {
        let p = &options.pilot;
        if p.shots < 2 {
            return Err(input_error(
                "comparison.pilot.shots",
                "at least two draws per channel required",
            ));
        }
        let exact_seed = p.seed.checked_add(1).ok_or_else(|| {
            input_error(
                "comparison.pilot.seed",
                "seed + 1 overflows u64; choose a smaller seed",
            )
        })?;
        let runtime = self.inputs.options.evaluation.runtime;
        if runtime.strategy != CausalStrategy::FixedTemplate {
            return Err(input_error(
                "comparison.causal_strategy",
                "prepared sampler comparison requires fixed_template",
            ));
        }
        let t = self.inputs.modules.task();
        let c = runtime.compile;
        for (name, needed, mode) in [
            ("probability_preparation", true, c.probability_preparation),
            (
                "density_preparation",
                self.inputs.modules.ideal_state().is_some(),
                c.density_preparation,
            ),
            (
                "moment_preparation",
                !t.observables().is_empty(),
                c.moment_preparation,
            ),
        ] {
            if needed && !mode.is_fixed() {
                return Err(input_error(
                    &format!("comparison.{name}"),
                    "used scalar families require fixed preparation to exclude first-use phase construction from pilot sampling time",
                ));
            }
        }
        p.validate(t.sample_records().len(), t.observables().len())?;
        Ok(exact_seed)
    }
    fn sampler_channel(&self, sampler: HistorySampler) -> FileTask {
        let i = &self.inputs;
        let mut options = i.options;
        options.evaluation.runtime.history_sampler = sampler;
        // Inputs are an admitted immutable snapshot. Descriptor admission is
        // deferred to each owned preparation before its quantum construction.
        Self {
            inputs: Arc::new(Inputs {
                physical: i.physical.clone(),
                modules: i.modules.clone(),
                registry: i.registry.clone(),
                sampling: i.sampling.clone(),
                options,
                identities: i.identities.clone(),
            }),
        }
    }
}
fn run_pilot(
    task: &FileTask,
    sampler: HistorySampler,
    run: &TaskRunOptions,
    options: &FilePreparationOptions,
) -> FileSamplerPilot {
    let prepared = task.prepare_sampling_owned(options);
    let (batch, failure) = if prepared.report().is_complete() {
        match prepared.run(run) {
            Ok(batch) => (Some(batch), None),
            Err(error) => (None, Some(error)),
        }
    } else {
        (None, None)
    };
    let (exported_batch, export_failure) = match batch.as_ref().map(FileRunReport::to_json) {
        Some(Ok(value)) => (Some(value), None),
        Some(Err(error)) => (None, Some(error)),
        None => (None, None),
    };
    let (_, preparation) = prepared.into_parts();
    FileSamplerPilot {
        history_sampler: sampler,
        seed: run.seed,
        preparation,
        batch,
        failure,
        export_failure,
        exported_batch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binary64_decisions_match_the_pinned_native_binding_including_ties_and_range_edges() {
        let data: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/sampler_decision_native.json"
        ))
        .unwrap();
        assert_eq!(
            data["reference_revision"],
            "56847478c840395735146ef1e1ccdc3d8aedc10f"
        );
        assert_eq!(
            data["native_sha256"],
            "24ec8f3c108ca3a624ca5b0fd0b536ceb01111736f5d737643eb8f44a6d4bd77"
        );
        let cases = data["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 24);
        for case in cases {
            let number = |key: &str| {
                f64::from_bits(u64::from_str_radix(case[key].as_str().unwrap(), 16).unwrap())
            };
            let actual = decide(
                number("kappa_bits"),
                number("affine_bits"),
                number("exact_bits"),
                case["count"].as_u64().unwrap(),
            );
            if case.get("error").is_some() {
                assert!(actual.is_err(), "{}", case["name"]);
            } else {
                let result = actual.unwrap();
                let expected = &case["expected"];
                let check = |field: &str, value: f64| {
                    assert_eq!(
                        format!("{:016x}", value.to_bits()),
                        expected[field].as_str().unwrap(),
                        "case {} field {field}",
                        case["name"]
                    )
                };
                check("gamma", result.gamma);
                check(
                    "affine_support_seconds_per_draw",
                    result.affine_support_seconds_per_draw,
                );
                check(
                    "exact_marginal_seconds_per_draw",
                    result.exact_marginal_seconds_per_draw,
                );
                assert_eq!(
                    sampler_name(result.recommended_sampler),
                    expected["recommended_sampler"].as_str().unwrap()
                );
            }
        }
    }
}
