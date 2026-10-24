//! Owned file-task snapshots, reusable compilation, and schema-2 run reports.
mod enumeration;
use crate::circuit::{Circuit, ParseOptions, parse_circuit};
use crate::costs::{ExecutionCosts, ExecutionLimits, ExecutionMeter, OperationCost};
use crate::noise::NoiseSampling;
use crate::numeric::NumericOptions;
use crate::runtime::ControllerRegistry;
use crate::task::{
    BatchExportError, ContentFingerprints, EvaluationOptions, FingerprintError, FingerprintInputs,
    FingerprintOptions, LoadedModules, TaskError, TaskEvaluator, TaskOptions, TaskRunOptions,
    TaskRunResult, export_batch_metrics, fingerprint_content, parse_task, resolved_options,
    run_task_batch_inspected, sampling_configuration, validate_snapshot_fingerprints,
};
pub use enumeration::{FileEnumerationOptions, FileEnumerationReport};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fmt, io::Read, path::Path, sync::Arc, time::Instant};

#[derive(Clone, Copy, Debug, Default)]
pub struct FileTaskOptions {
    pub evaluation: EvaluationOptions,
    pub numeric: NumericOptions,
    pub fingerprints: FingerprintOptions,
}
#[derive(Clone, Debug, Default)]
pub struct FileLoadOptions {
    pub circuit: ParseOptions,
    pub task: TaskOptions,
}
#[derive(Debug)]
pub enum FileApiError {
    Input(TaskError),
    Fingerprint(FingerprintError),
    /// Metadata failure must not discard an already executed batch.
    Snapshot {
        result: Box<TaskRunResult>,
        source: TaskError,
    },
}
impl FileApiError {
    pub fn retained_result(&self) -> Option<&TaskRunResult> {
        match self {
            Self::Snapshot { result, .. } => Some(result),
            _ => None,
        }
    }
}
impl fmt::Display for FileApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(e) => e.fmt(f),
            Self::Fingerprint(e) => write!(f, "file-task fingerprint: {e}"),
            Self::Snapshot { result, source } => write!(
                f,
                "cannot snapshot metadata after {} completed draws: {source}; typed result retained",
                result.completed
            ),
        }
    }
}
impl std::error::Error for FileApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input(e) => Some(e),
            Self::Fingerprint(e) => Some(e),
            Self::Snapshot { source, .. } => Some(source),
        }
    }
}
impl From<TaskError> for FileApiError {
    fn from(e: TaskError) -> Self {
        Self::Input(e)
    }
}
impl From<FingerprintError> for FileApiError {
    fn from(e: FingerprintError) -> Self {
        Self::Fingerprint(e)
    }
}
fn input_error(field: &str, message: impl fmt::Display) -> FileApiError {
    TaskError {
        field: field.to_owned(),
        message: message.to_string(),
    }
    .into()
}
fn read_text(path: &Path, limit: usize, field: &str) -> Result<String, FileApiError> {
    let file = std::fs::File::open(path)
        .map_err(|e| input_error(field, format!("{}: {e}", path.display())))?;
    let mut bytes = Vec::new();
    file.take((limit as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| input_error(field, format!("{}: {e}", path.display())))?;
    if bytes.len() > limit {
        return Err(input_error(
            field,
            format!("{} exceeds the {limit}-byte input limit", path.display()),
        ));
    }
    String::from_utf8(bytes)
        .map_err(|_| input_error(field, format!("{} is not UTF-8", path.display())))
}
struct Inputs {
    physical: Circuit,
    modules: LoadedModules,
    registry: ControllerRegistry,
    identities: BTreeMap<String, Value>,
    sampling: NoiseSampling,
    options: FileTaskOptions,
}
/// Immutable loaded content. Clones share the snapshot; callbacks retain their
/// existing deterministic/stateless caller contract and unverified declarations.
#[derive(Clone)]
pub struct FileTask {
    inputs: Arc<Inputs>,
}
impl fmt::Debug for FileTask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileTask")
            .field("qubits", &self.inputs.physical.num_qubits())
            .field("records", &self.inputs.physical.num_records())
            .finish_non_exhaustive()
    }
}
impl FileTask {
    pub fn from_loaded(
        physical: Circuit,
        modules: LoadedModules,
        registry: &ControllerRegistry,
        sampling: NoiseSampling,
        options: FileTaskOptions,
        identities: BTreeMap<String, Value>,
    ) -> Result<Self, FileApiError> {
        let task = Self {
            inputs: Arc::new(Inputs {
                physical,
                modules,
                registry: registry.clone(),
                identities,
                sampling,
                options,
            }),
        };
        validate_snapshot_fingerprints(
            &task.fingerprint_input(&TaskRunOptions::default()),
            task.inputs.options.fingerprints,
        )?;
        Ok(task)
    }
    /// Resolve all module paths now; inspection, compilation and runs never
    /// reopen these source/module files. Controller hosts are separate bindings.
    pub fn load(
        physical: &Path,
        task: Option<&Path>,
        registry: &ControllerRegistry,
        sampling: NoiseSampling,
        options: FileTaskOptions,
        load: &FileLoadOptions,
        identities: BTreeMap<String, Value>,
    ) -> Result<Self, FileApiError> {
        let mut parse = load.circuit.clone();
        parse.controllers.extend(registry.names());
        parse.source = Some(physical.display().to_string());
        let source = read_text(physical, parse.max_source_bytes, "circuit")?;
        let circuit = parse_circuit(&source, &parse).map_err(|e| input_error("circuit", e))?;
        let (text, base) = match task {
            Some(path) => (
                read_text(path, load.task.max_json_bytes, "task")?,
                path.parent().unwrap_or(Path::new(".")),
            ),
            None => ("{}".to_owned(), physical.parent().unwrap_or(Path::new("."))),
        };
        let spec = parse_task(&text, base, &load.task)?;
        let modules = spec.load_modules(&circuit, &parse)?;
        Self::from_loaded(circuit, modules, registry, sampling, options, identities)
    }
    fn fingerprint_input<'a>(&'a self, run: &TaskRunOptions) -> FingerprintInputs<'a> {
        let i = &self.inputs;
        FingerprintInputs {
            physical: &i.physical,
            modules: &i.modules,
            controllers: &i.registry,
            controller_identities: &i.identities,
            evaluation: i.options.evaluation,
            run: *run,
            sampling: &i.sampling,
            numeric: i.options.numeric,
        }
    }
    pub fn fingerprints(&self, run: &TaskRunOptions) -> Result<ContentFingerprints, FileApiError> {
        let t = self.inputs.modules.task();
        run.validate(t.sample_records().len(), t.observables().len())?;
        Ok(fingerprint_content(
            &self.fingerprint_input(run),
            self.inputs.options.fingerprints,
        )?)
    }
    fn descriptor(&self, run: &TaskRunOptions) -> Result<Value, FileApiError> {
        let fingerprints = self.fingerprints(run)?;
        Ok(self.descriptor_value(resolved_options(&self.fingerprint_input(run)), fingerprints))
    }
    fn descriptor_value(&self, options: Value, fingerprints: ContentFingerprints) -> Value {
        let i = &self.inputs;
        let t = i.modules.task();
        let target=i.modules.ideal_state().map(|p|json!({"qubits":p.circuit().num_qubits(),"outputs":p.outputs(),"postselection":p.postselection()}));
        let recovery = i.modules.recovery().map(circuit_summary);
        let history = match i.options.evaluation.runtime.history_sampler {
            crate::runtime::HistorySampler::ExactMarginal => "exact_marginal",
            crate::runtime::HistorySampler::AffineSupport => "affine_support",
        };
        json!({
            "schema_version":2,"software_version":concat!("rmagna-",env!("CARGO_PKG_VERSION")),
            "software":{"name":"rmagna","version":env!("CARGO_PKG_VERSION"),"implementation":"independent_rust"},
            "circuit":circuit_summary(&i.physical),
            "options":options,
            "fingerprints":fingerprints.to_json(),
            "provenance_limits":{"max_json_bytes":i.options.fingerprints.max_json_bytes,"max_depth":i.options.fingerprints.max_depth},
            "method":match i.sampling {NoiseSampling::Physical=>"global_boundary_with_independent_original_noise",NoiseSampling::Importance{..}=>"global_boundary_with_noise_importance_sampling"},
            "history_sampler":history,
            "sampling":{"method":match i.sampling {NoiseSampling::Physical=>"physical",NoiseSampling::Importance{..}=>"importance"},"configuration":sampling_configuration(&i.sampling),"history_sampler":history},
            "evaluation":{"ideal_state":target,"recovery":recovery,"output_mapping":t.outputs(),"error_observables":t.error_observables(),"sample_records":t.sample_records(),"moment_count":t.observables().len(),
                "error_metric":if t.error_observables().is_some(){"wrong when any listed OBSERVABLE_INCLUDE parity is one"}else if target.is_some(){"target infidelity after terminal recovery"}else{"declared output task"},
                "observables":if recovery.is_some(){"after recovery, before target projection"}else{"physical output"}}
        })
    }
    /// Validate and describe captured content for a proposed run without quantum
    /// queries, callbacks, entropy, or construction of execution plans.
    pub fn inspect(&self, run: &TaskRunOptions) -> Result<Value, FileApiError> {
        let mut value = self.descriptor(run)?;
        let object = value.as_object_mut().expect("descriptor object");
        object.insert("kind".to_owned(), json!("inspection"));
        object.insert("route".to_owned(), json!("not_compiled"));
        object.insert(
            "compilation".to_owned(),
            json!({"status":"not_compiled","query_preparation":"not_started"}),
        );
        Ok(value)
    }
    pub fn compile(&self) -> Result<CompiledFileTask, FileApiError> {
        self.compile_with_limits(ExecutionLimits::default())
    }
    pub fn compile_with_limits(
        &self,
        limits: ExecutionLimits,
    ) -> Result<CompiledFileTask, FileApiError> {
        let i = &self.inputs;
        let started = Instant::now();
        let meter = ExecutionMeter::new(limits);
        let evaluator = TaskEvaluator::new_with_costs(
            &i.physical,
            &i.modules,
            &i.registry,
            i.options.evaluation,
            &i.sampling,
            &meter,
        )?;
        let preparation = Preparation {
            seconds: started.elapsed().as_secs_f64(),
            costs: meter.snapshot(),
            limits,
        };
        Ok(CompiledFileTask {
            task: self.clone(),
            evaluator,
            preparation,
        })
    }
    /// Fresh preparation shares the run quota/deadline; ordinary preparation
    /// failures produce failed reports with zero attempted draws.
    pub fn run(&self, run: &TaskRunOptions) -> Result<FileRunReport, FileApiError> {
        let mut metadata = self.descriptor(run)?;
        let i = &self.inputs;
        let (result, snapshot) = run_task_batch_inspected(
            &i.physical,
            &i.modules,
            &i.registry,
            i.options.evaluation,
            &i.sampling,
            run,
            |evaluator, costs, seconds| {
                compilation_snapshot(
                    evaluator,
                    Preparation {
                        seconds,
                        costs,
                        limits: run.execution_limits,
                    },
                    true,
                )
            },
        )?;
        match snapshot {
            Some(Ok(snapshot)) => add_snapshot(&mut metadata, snapshot),
            Some(Err(source)) => {
                return Err(FileApiError::Snapshot {
                    result: Box::new(result),
                    source,
                });
            }
            None => {
                let object = metadata.as_object_mut().expect("descriptor object");
                object.insert("route".to_owned(), json!("unavailable"));
                object.insert("compilation".to_owned(),json!({"status":"preparation_failed","query_preparation":"unavailable","preparation_included_in_this_call":true}));
            }
        }
        Ok(FileRunReport {
            result,
            metadata,
            numeric: i.options.numeric,
        })
    }
}
#[derive(Clone, Copy)]
struct Preparation {
    seconds: f64,
    costs: ExecutionCosts,
    limits: ExecutionLimits,
}
/// Repeated batches share the evaluator and captured input identities. Their
/// seeds, statistical accumulators, quotas and operation collectors are local.
pub struct CompiledFileTask {
    task: FileTask,
    evaluator: TaskEvaluator,
    preparation: Preparation,
}
impl fmt::Debug for CompiledFileTask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompiledFileTask")
            .field("task", &self.task)
            .field("route", &self.evaluator.execution_route())
            .finish_non_exhaustive()
    }
}
impl CompiledFileTask {
    pub fn inspect(&self, run: &TaskRunOptions) -> Result<Value, FileApiError> {
        let mut value = self.task.descriptor(run)?;
        add_snapshot(
            &mut value,
            compilation_snapshot(&self.evaluator, self.preparation, false)?,
        );
        value
            .as_object_mut()
            .expect("descriptor object")
            .insert("kind".to_owned(), json!("inspection"));
        Ok(value)
    }
    pub fn run(&self, run: &TaskRunOptions) -> Result<FileRunReport, FileApiError> {
        let mut metadata = self.task.descriptor(run)?;
        let result = self.evaluator.run_batch(run)?;
        let snapshot = match compilation_snapshot(&self.evaluator, self.preparation, false) {
            Ok(snapshot) => snapshot,
            Err(source) => {
                return Err(FileApiError::Snapshot {
                    result: Box::new(result),
                    source,
                });
            }
        };
        add_snapshot(&mut metadata, snapshot);
        Ok(FileRunReport {
            result,
            metadata,
            numeric: self.task.inputs.options.numeric,
        })
    }
}
fn circuit_summary(circuit: &Circuit) -> Value {
    json!({"format":circuit.format_version(),"qubits":circuit.num_qubits(),"records":circuit.num_records(),"instructions":circuit.num_instructions(),"operations":circuit.operations().len(),"detectors":circuit.detectors().len(),"external_calls":circuit.external_calls(),"history":circuit.needs_history_sampling()})
}
fn costs_json(costs: ExecutionCosts) -> Value {
    let row = |c: OperationCost| json!({"attempted":c.attempted,"succeeded":c.succeeded,"failed":c.failed,"seconds":c.seconds});
    json!({"compilation":row(costs.compilation),"query":row(costs.query),"decoder":row(costs.decoder),"support":row(costs.support),"timing":"inclusive_wall_seconds"})
}
fn compilation_snapshot(
    evaluator: &TaskEvaluator,
    preparation: Preparation,
    included: bool,
) -> Result<Value, TaskError> {
    let d = evaluator.compilation();
    let probability = evaluator
        .prepared_probability_statistics()?
        .map(|s| json!({"cached_queries":s.cached_queries,"fixed_resources":s.fixed_resources}));
    let density = evaluator
        .prepared_density_statistics()?
        .map(|s| json!({"cached_queries":s.cached_queries,"fixed_resources":s.fixed_resources}));
    let moment = evaluator
        .prepared_moment_statistics()?
        .map(|s| json!({"cached_queries":s.cached_queries,"fixed_resources":s.fixed_resources}));
    let insertions=evaluator.prepared_insertion_statistics().map(|s|json!({"catalog_entries":s.catalog_entries,"estimated_peak_bytes":s.estimated_peak_bytes}));
    let proposal=evaluator.scaled_proposal_summary().map(|s|json!({"proposal_scale":{"numerator":s.proposal_scale.numer().to_string(),"denominator":s.proposal_scale.denom().to_string()},"categories":s.categories,"scaled_categories":s.scaled_categories,"unchanged_categories":s.unchanged_categories,"excluded_categories":s.excluded_categories,"endpoint_categories":s.endpoint_categories}));
    Ok(
        json!({"route":evaluator.execution_route(),"proposal_summary":proposal,"compilation":{
        "status":"compiled","query_preparation":"on_demand","reference_scope":"compiled protocol diagnostics; caches list retained families, without exhaustive preparation",
            "preparation_seconds":preparation.seconds,"preparation_costs":costs_json(preparation.costs),"preparation_limits":preparation.limits.to_json(),"preparation_included_in_this_call":included,
            "compiler_diagnostics":{"strategy":format!("{:?}",d.strategy),"layer_slots":d.layer_slots,"reference_layer":d.reference_layer,"reference_layer_slots":d.reference_layer_slots,"phase_ancillas":d.phase_ancillas,"shared_phase_coordinates":d.shared_phase_coordinates,"coherent_width":d.coherent_width,"reference_coherent_width":d.reference_coherent_width,"shared_observations_before_reduction":d.shared_observations_before_reduction,"clifford_gates":d.clifford_gates,"path_variables":d.path_variables},
            "retained_caches":{"probability":probability,"density":density,"moment":moment,"insertions":insertions},"cache_scope":"reference evaluator; cold-prefix caches are ephemeral"
        }}),
    )
}
fn add_snapshot(metadata: &mut Value, snapshot: Value) {
    metadata
        .as_object_mut()
        .expect("descriptor object")
        .extend(snapshot.as_object().expect("snapshot object").clone());
}
/// The report owns both its typed batch and captured metadata. Failed numerical
/// serialization leaves the complete typed result available through raw_result.
#[derive(Debug)]
pub struct FileRunReport {
    result: TaskRunResult,
    metadata: Value,
    numeric: NumericOptions,
}
impl FileRunReport {
    /// Captured input identities, resolved options, and actual preparation
    /// metadata, available even when numerical serialization fails.
    pub fn metadata(&self) -> &Value {
        &self.metadata
    }
    pub fn raw_result(&self) -> &TaskRunResult {
        &self.result
    }
    pub fn into_raw_result(self) -> TaskRunResult {
        self.result
    }
    pub fn to_json(&self) -> Result<Value, BatchExportError> {
        let mut value = export_batch_metrics(&self.result, self.numeric)?;
        let object = value.as_object_mut().expect("metrics object");
        object.extend(self.metadata.as_object().expect("metadata object").clone());
        let costs = self.result.execution_costs;
        object.insert("resources".to_owned(),json!({"scope":"this_call_including_failed_attempts","queries":costs.query.attempted,"compilations":costs.compilation.attempted,"limits":self.metadata["options"],"compilation":self.metadata["compilation"]}));
        let times = object
            .get_mut("timings")
            .and_then(Value::as_object_mut)
            .expect("metric timings");
        times.extend(serde_json::from_value::<serde_json::Map<String,Value>>(json!({"compile_seconds":costs.compilation.seconds,"query_seconds":costs.query.seconds,"decoder_seconds":costs.decoder.seconds,"support_seconds":costs.support.seconds,"components_are_additive":false})).expect("timing object"));
        Ok(value)
    }
}
