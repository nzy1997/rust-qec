//! Explicit file preparation with owned receipts and fresh seeded batch sessions.
use super::*;
use crate::task::{
    FamilyPreparationCounts, SamplingPreparationOptions, SamplingPreparationReport,
    fingerprint_with_configuration,
};
use std::num::NonZeroU64;

#[derive(Clone, Copy, Debug, Default)]
pub struct FilePreparationOptions {
    pub queries: SamplingPreparationOptions,
    pub execution_limits: ExecutionLimits,
}
impl FilePreparationOptions {
    fn to_json(self) -> Value {
        json!({"method":"configured_reusable_sampling_families",
            "max_requests":self.queries.max_requests,
            "max_history_rows":self.queries.max_history_rows,
            "max_layout_rows":self.queries.max_layout_rows,
            "execution_limits":self.execution_limits.to_json()})
    }
}
/// A reusable task plus the owned result of its explicit preparation stage.
/// Failed stages retain compiled partial resources for inspection and retry.
#[derive(Debug)]
pub struct PreparedFileTask {
    compiled: Option<CompiledFileTask>,
    report: FilePreparationReport,
}
impl PreparedFileTask {
    pub fn report(&self) -> &FilePreparationReport {
        &self.report
    }
    /// Also available after a family-preparation failure; no readiness claim.
    pub fn partial_task(&self) -> Option<&CompiledFileTask> {
        self.compiled.as_ref()
    }
    pub fn into_parts(self) -> (Option<CompiledFileTask>, FilePreparationReport) {
        (self.compiled, self.report)
    }
    fn ready(&self) -> Result<&CompiledFileTask, FileApiError> {
        self.report.require_complete()?;
        Ok(self.compiled.as_ref().expect("successful compilation"))
    }
    pub fn inspect(&self, run: &TaskRunOptions) -> Result<Value, FileApiError> {
        let compiled = self.ready()?;
        let mut value = compiled.inspect(run)?;
        self.bind_metadata(compiled, &mut value, run)?;
        Ok(value)
    }
    /// Prior compilation and query preparation remain outside this batch's
    /// quota, RNG, statistics and sampling time. The saved receipt stays visible.
    pub fn run(&self, run: &TaskRunOptions) -> Result<FileRunReport, FileApiError> {
        self.run_observed(run, None)
    }
    pub fn run_with_progress(
        &self,
        run: &TaskRunOptions,
        observer: &mut ProgressObserver<'_>,
    ) -> Result<FileRunReport, FileApiError> {
        self.run_observed(run, Some(observer))
    }
    fn run_observed(
        &self,
        run: &TaskRunOptions,
        mut observer: Option<&mut ProgressObserver<'_>>,
    ) -> Result<FileRunReport, FileApiError> {
        if let Some(observer) = observer.as_deref_mut() {
            observer.reset();
        }
        let interval = observer.as_ref().map(|o| o.receipt().every);
        let compiled = self.ready()?;
        // Validate fingerprint/export configuration before any draw or callback.
        let descriptor = compiled
            .task
            .preparation_descriptor(&self.report.options, Some(run), interval)
            .map_err(|(_, error)| error)?;
        let mut report = match observer {
            Some(observer) => compiled.run_with_progress(run, observer)?,
            None => compiled.run(run)?,
        };
        report
            .metadata
            .as_object_mut()
            .expect("run metadata")
            .extend(descriptor.as_object().expect("descriptor").clone());
        bind_stage(&mut report.metadata, self.report.stage.as_ref(), false);
        Ok(report)
    }
    fn bind_metadata(
        &self,
        compiled: &CompiledFileTask,
        value: &mut Value,
        run: &TaskRunOptions,
    ) -> Result<(), FileApiError> {
        let descriptor = compiled
            .task
            .preparation_descriptor(&self.report.options, Some(run), None)
            .map_err(|(_, error)| error)?;
        value
            .as_object_mut()
            .expect("metadata")
            .extend(descriptor.as_object().expect("descriptor").clone());
        bind_stage(value, self.report.stage.as_ref(), false);
        Ok(())
    }
}
/// Complete typed stage results and metadata survive any subsequent JSON use.
#[derive(Debug)]
pub struct FilePreparationReport {
    options: FilePreparationOptions,
    result: Result<(), TaskError>,
    stage: Option<SamplingPreparationReport>,
    metadata: Value,
    metadata_failure: Option<TaskError>,
    execution_costs: ExecutionCosts,
    seconds: f64,
}
impl FilePreparationReport {
    pub fn options(&self) -> FilePreparationOptions {
        self.options
    }
    pub fn raw_result(&self) -> Result<(), TaskError> {
        self.result.clone()
    }
    pub fn stage(&self) -> Option<&SamplingPreparationReport> {
        self.stage.as_ref()
    }
    pub fn metadata(&self) -> &Value {
        &self.metadata
    }
    pub fn metadata_failure(&self) -> Option<&TaskError> {
        self.metadata_failure.as_ref()
    }
    pub fn execution_costs(&self) -> ExecutionCosts {
        self.execution_costs
    }
    pub fn total_seconds(&self) -> f64 {
        self.seconds
    }
    pub fn is_complete(&self) -> bool {
        self.result.is_ok() && self.metadata_failure.is_none()
    }
    fn require_complete(&self) -> Result<(), FileApiError> {
        self.result.clone()?;
        if let Some(error) = &self.metadata_failure {
            return Err(error.clone().into());
        }
        Ok(())
    }
    pub fn to_json(&self) -> Value {
        let mut value = self.metadata.clone();
        value["kind"] = json!("preparation");
        value["status"] = json!(if self.is_complete() {
            "complete"
        } else {
            "failed"
        });
        value["execution_costs"] = costs_json(self.execution_costs);
        value["timings"] = json!({"total_seconds":self.seconds,
            "query_preparation_seconds":self.stage.as_ref().map(|s|s.elapsed_seconds),
            "sampling_seconds":0.0,"components_are_additive":false});
        value["failure"] = self.result.as_ref().err().map(|e| json!({"phase":if self.metadata_failure.is_some() && self.stage.is_none(){"metadata"}else if self.stage.is_some(){"sampling_families"}else{"compilation"},"field":e.field,"message":e.message})).unwrap_or(Value::Null);
        value["metadata_failure"] = self
            .metadata_failure
            .as_ref()
            .map(|e| json!({"field":e.field,"message":e.message}))
            .unwrap_or(Value::Null);
        value["sampling_preparation"] = self.stage.as_ref().map(stage_json).unwrap_or(Value::Null);
        value
    }
}
impl FileTask {
    fn preparation_descriptor(
        &self,
        options: &FilePreparationOptions,
        run: Option<&TaskRunOptions>,
        every: Option<NonZeroU64>,
    ) -> Result<Value, (Value, FileApiError)> {
        let defaults = TaskRunOptions::default();
        let chosen = run.unwrap_or(&defaults);
        if let Some(run) = run {
            let task = self.inputs.modules.task();
            run.validate(task.sample_records().len(), task.observables().len())
                .map_err(|error| (Value::Null, error.into()))?;
        }
        let input = self.fingerprint_input(chosen);
        let mut configured = resolved_options(&input);
        let object = configured.as_object_mut().expect("options");
        if run.is_none() {
            object.remove("run");
        }
        object.insert("sampling_preparation".into(), options.to_json());
        if let (Some(_), Some(every)) = (run, every) {
            configured["run"]["progress_every"] = json!(every.get());
        }
        let sampling = sampling_configuration(&self.inputs.sampling);
        let fingerprints = fingerprint_with_configuration(
            &input,
            &configured,
            &sampling,
            self.inputs.options.fingerprints,
        )
        .map_err(|error| {
            (
                self.descriptor_value_with_fingerprints(configured.clone(), Value::Null),
                error.into(),
            )
        })?;
        Ok(self.descriptor_value(configured, fingerprints))
    }
    /// Compile and prepare with one shared deadline and query quota. Ordinary
    /// compilation/family failures return owned receipts rather than losing costs.
    pub fn prepare_sampling(
        &self,
        options: &FilePreparationOptions,
    ) -> Result<PreparedFileTask, FileApiError> {
        Ok(self.prepare_sampling_owned(options))
    }
    // Shared owned implementation prevents comparison from discarding another
    // completed pilot through an outer preparation error.
    pub(super) fn prepare_sampling_owned(
        &self,
        options: &FilePreparationOptions,
    ) -> PreparedFileTask {
        let started = Instant::now();
        let meter = ExecutionMeter::new(options.execution_limits);
        let mut metadata = match self.preparation_descriptor(options, None, None) {
            Ok(metadata) => metadata,
            Err((metadata, error)) => {
                return PreparedFileTask {
                    compiled: None,
                    report: metadata_failure_report(
                        metadata, error, options, &meter, started, None,
                    ),
                };
            }
        };
        let i = &self.inputs;
        let compiled = TaskEvaluator::new_with_costs(
            &i.physical,
            &i.modules,
            &i.registry,
            i.options.evaluation,
            &i.sampling,
            &meter,
        );
        let preparation = Preparation {
            seconds: started.elapsed().as_secs_f64(),
            costs: meter.snapshot(),
            limits: options.execution_limits,
        };
        match compiled {
            Ok(evaluator) => {
                let compiled = CompiledFileTask {
                    task: self.clone(),
                    evaluator,
                    preparation,
                };
                let report =
                    compiled.prepare_sampling_report(metadata, options, &meter, started, true);
                PreparedFileTask {
                    compiled: Some(compiled),
                    report,
                }
            }
            Err(error) => {
                metadata["route"] = json!("unavailable");
                metadata["compilation"] = json!({"status":"preparation_failed","query_preparation":"not_started",
                    "preparation_seconds":preparation.seconds,"preparation_costs":costs_json(preparation.costs),
                    "preparation_limits":preparation.limits.to_json(),"preparation_included_in_this_call":true});
                PreparedFileTask {
                    compiled: None,
                    report: FilePreparationReport {
                        options: *options,
                        result: Err(error),
                        stage: None,
                        metadata,
                        metadata_failure: None,
                        execution_costs: meter.snapshot(),
                        seconds: started.elapsed().as_secs_f64(),
                    },
                }
            }
        }
    }
}
impl CompiledFileTask {
    /// Reuse original compiled content with a new call-local preparation budget.
    /// The returned report is owned; this task continues to own retained plans.
    pub fn prepare_sampling(
        &self,
        options: &FilePreparationOptions,
    ) -> Result<FilePreparationReport, FileApiError> {
        let started = Instant::now();
        let meter = ExecutionMeter::new(options.execution_limits);
        let metadata = match self.task.preparation_descriptor(options, None, None) {
            Ok(metadata) => metadata,
            Err((metadata, error)) => {
                return Ok(metadata_failure_report(
                    metadata,
                    error,
                    options,
                    &meter,
                    started,
                    Some((self.evaluator.execution_route(), self.preparation)),
                ));
            }
        };
        Ok(self.prepare_sampling_report(metadata, options, &meter, started, false))
    }
    fn prepare_sampling_report(
        &self,
        mut metadata: Value,
        options: &FilePreparationOptions,
        meter: &ExecutionMeter,
        started: Instant,
        included: bool,
    ) -> FilePreparationReport {
        let stage = self
            .evaluator
            .prepare_sampling_queries(options.queries, meter);
        let result = stage.result.clone();
        let metadata_failure =
            match compilation_snapshot(&self.evaluator, self.preparation, included) {
                Ok(snapshot) => {
                    add_snapshot(&mut metadata, snapshot);
                    None
                }
                Err(error) => Some(error),
            };
        bind_stage(&mut metadata, Some(&stage), true);
        FilePreparationReport {
            options: *options,
            result,
            stage: Some(stage),
            metadata,
            metadata_failure,
            execution_costs: meter.snapshot(),
            seconds: started.elapsed().as_secs_f64(),
        }
    }
}
// Fingerprint admission can fail before compilation. Preserve the resolved
// configuration and known preparation costs without inventing content hashes.
fn metadata_failure_report(
    mut metadata: Value,
    source: FileApiError,
    options: &FilePreparationOptions,
    meter: &ExecutionMeter,
    started: Instant,
    prior: Option<(&'static str, Preparation)>,
) -> FilePreparationReport {
    let included = prior.is_none();
    let (route, preparation) = prior.unwrap_or((
        "unavailable",
        Preparation {
            seconds: started.elapsed().as_secs_f64(),
            costs: meter.snapshot(),
            limits: options.execution_limits,
        },
    ));
    metadata["route"] = json!(route);
    metadata["compilation"] = json!({
        "status":if included {"not_started"} else {"compiled"},
        "query_preparation":"not_started",
        "preparation_seconds":preparation.seconds,
        "preparation_costs":costs_json(preparation.costs),
        "preparation_limits":preparation.limits.to_json(),
        "preparation_included_in_this_call":included,
        "query_preparation_included_in_this_call":false,
        "metadata_status":"fingerprint_failed",
    });
    let error = TaskError {
        field: "preparation.metadata".into(),
        message: source.to_string(),
    };
    FilePreparationReport {
        options: *options,
        result: Err(error.clone()),
        stage: None,
        metadata,
        metadata_failure: Some(error),
        execution_costs: meter.snapshot(),
        seconds: started.elapsed().as_secs_f64(),
    }
}
fn counts_json(c: FamilyPreparationCounts) -> Value {
    json!({"planned":c.planned,"attempted":c.attempted,"created":c.created,"reused":c.reused,"cold":c.cold,"failed":c.failed})
}
fn stage_json(stage: &SamplingPreparationReport) -> Value {
    let kinds = stage
        .by_kind
        .iter()
        .map(|(k, v)| (k.to_string(), counts_json(*v)))
        .collect::<serde_json::Map<_, _>>();
    json!({"scope":stage.scope,"execution_route":stage.execution_route,
        "status":if stage.result.is_ok(){"complete"}else{"failed"},
        "all_families_retained":stage.all_families_retained(),
        "phase_resource_policy":"configured_per_family; trace policies defer phase construction",
        "counts":counts_json(stage.counts),"by_kind":kinds,
        "history_rows":stage.history_rows,"layout_rows":stage.layout_rows,
        "elapsed_seconds":stage.elapsed_seconds,
        "failure":stage.result.as_ref().err().map(|e|json!({"field":e.field,"message":e.message}))})
}
fn bind_stage(metadata: &mut Value, stage: Option<&SamplingPreparationReport>, included: bool) {
    let Some(stage) = stage else { return };
    metadata["compilation"]["query_preparation"] = json!(if stage.result.is_ok() {
        "explicit_configured_sampling_families"
    } else {
        "explicit_preparation_failed"
    });
    metadata["compilation"]["reference_scope"] = json!(
        "configured reusable sampling families; cold and trace policies remain explicit; exact enumeration excluded"
    );
    metadata["compilation"]["query_preparation_included_in_this_call"] = json!(included);
    metadata["compilation"]["query_preparation_receipt"] = stage_json(stage);
}
