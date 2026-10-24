//! Task schema 1, exact output masks, and relative module identities.
mod compose;
mod export;
pub use export::{BatchExportError, export_batch_metrics};
mod evaluate;
pub use evaluate::{
    BatchProgress, ContractionResources, EvaluationOptions, ExactInterval, FamilyPreparationCounts,
    ProgressObserver, ProgressReceipt, RunFailure, RunPhase, RunStatus, RunTimings,
    SamplingPreparationOptions, SamplingPreparationReport, TaskEnumeration, TaskEnumerationOptions,
    TaskEvaluation, TaskEvaluator, TaskRunOptions, TaskRunResult, TaskRunStatistics,
    TaskSectorMasses, TaskVariances, TaskWeightSector, run_task_batch,
};
pub(crate) use evaluate::{RunHooks, run_task_batch_inspected};
mod parse;
mod provenance;
use crate::circuit::{Circuit, Operation, ParseOptions, SourceLocation, parse_circuit};
use crate::gf2::BitVector;
use crate::pauli::PauliWord;
pub(crate) use export::Encoder as NumericEncoder;
use num_bigint::BigUint;
pub use parse::parse_task;
pub use provenance::{
    ContentFingerprints, FingerprintError, FingerprintInputs, FingerprintOptions,
    canonical_fingerprint, canonical_raw_fingerprint, fingerprint_content,
};
pub(crate) use provenance::{
    fingerprint_with_configuration, resolved_options, sampling_configuration,
    validate_snapshot_fingerprints,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskError {
    pub field: String,
    pub message: String,
}
impl TaskError {
    fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}
impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "task field {}: {}", self.field, self.message)
    }
}
impl std::error::Error for TaskError {}
impl From<crate::costs::CostError> for TaskError {
    fn from(error: crate::costs::CostError) -> Self {
        Self::new("execution_costs", error.to_string())
    }
}
#[derive(Clone, Copy, Debug)]
pub struct TaskOptions {
    pub max_json_bytes: usize,
    pub max_list_items: usize,
    pub max_mask_bits: u64,
}
impl Default for TaskOptions {
    fn default() -> Self {
        Self {
            max_json_bytes: 16 * 1024 * 1024,
            max_list_items: 1_000_000,
            max_mask_bits: 65536,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetSpec {
    circuit: PathBuf,
    outputs: Option<Vec<usize>>,
    postselection: BTreeMap<usize, bool>,
    object_form: bool,
}
impl TargetSpec {
    pub fn circuit_path(&self) -> &Path {
        &self.circuit
    }
    pub fn outputs(&self) -> Option<&[usize]> {
        self.outputs.as_deref()
    }
    pub fn postselection(&self) -> &BTreeMap<usize, bool> {
        &self.postselection
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskSpec {
    outputs: Option<Vec<usize>>,
    error_observables: Option<Vec<usize>>,
    ideal: Option<TargetSpec>,
    recovery: Option<PathBuf>,
    sample_records: Vec<usize>,
    observables: Vec<(BigUint, BigUint)>,
}
impl TaskSpec {
    pub fn outputs(&self) -> Option<&[usize]> {
        self.outputs.as_deref()
    }
    pub fn error_observables(&self) -> Option<&[usize]> {
        self.error_observables.as_deref()
    }
    pub fn ideal_state(&self) -> Option<&TargetSpec> {
        self.ideal.as_ref()
    }
    pub fn recovery_path(&self) -> Option<&Path> {
        self.recovery.as_deref()
    }
    pub fn sample_records(&self) -> &[usize] {
        &self.sample_records
    }
    pub fn observables(&self) -> &[(BigUint, BigUint)] {
        &self.observables
    }
    pub fn validate(&self, physical: &Circuit) -> Result<ValidatedTask, TaskError> {
        let outputs = self.outputs.clone().unwrap_or_else(|| {
            if self.error_observables.is_some() {
                vec![]
            } else {
                (0..physical.num_qubits()).collect()
            }
        });
        validate_indices("outputs", &outputs, physical.num_qubits(), true)?;
        validate_indices(
            "sample_records",
            &self.sample_records,
            physical.num_records(),
            false,
        )?;
        if let Some(indices) = &self.error_observables {
            if self.ideal.is_some()
                || self.recovery.is_some()
                || !outputs.is_empty()
                || !self.observables.is_empty()
            {
                return Err(TaskError::new(
                    "error_observables",
                    "a classical error definition requires empty outputs and observables and excludes ideal_state and recovery",
                ));
            }
            for index in indices {
                if !physical.observables().contains_key(index) {
                    return Err(TaskError::new(
                        "error_observables",
                        format!("OBSERVABLE_INCLUDE index {index} was not declared"),
                    ));
                }
            }
        }
        let mut observables = Vec::new();
        for (x, z) in &self.observables {
            if x.bits() > outputs.len() as u64 || z.bits() > outputs.len() as u64 {
                return Err(TaskError::new(
                    "observables",
                    "Pauli masks exceed the selected output width",
                ));
            }
            let convert = |value: &BigUint| {
                let mut words = value.to_u64_digits();
                words.resize(outputs.len().div_ceil(64), 0);
                BitVector::from_words(outputs.len(), words).expect("validated mask width")
            };
            let (x, z) = (convert(x), convert(z));
            let phase = (2 * x.ones().filter(|&j| z.get(j).unwrap()).count() % 8) as u8;
            observables.push(PauliWord::new(x, z, phase).expect("validated Pauli masks"));
        }
        Ok(ValidatedTask {
            outputs,
            error_observables: self.error_observables.clone(),
            sample_records: self.sample_records.clone(),
            observables,
        })
    }
    pub fn load_modules(
        &self,
        physical: &Circuit,
        options: &ParseOptions,
    ) -> Result<LoadedModules, TaskError> {
        let task = self.validate(physical)?;
        let ideal = if let Some(spec) = &self.ideal {
            let mut circuit = read_circuit(&spec.circuit, "ideal_state", options)?;
            if let Some(op) = circuit.operations().iter().find(|op| match op.operation {
                Operation::Unitary { control: None, .. } | Operation::Annotation { .. } => false,
                Operation::Detector { .. } | Operation::Observable { .. } => {
                    circuit.declaration_has_targets(op.id)
                }
                _ => true,
            }) {
                return Err(module_operation_error(
                    "ideal_state",
                    &spec.circuit,
                    &op.location,
                    "target preparation must contain only noiseless unitary gates, annotations and detector/observable declarations without targets",
                ));
            }
            if !spec.object_form && circuit.num_qubits() > task.outputs.len() {
                return Err(TaskError::new(
                    "ideal_state",
                    "a path target must use only the selected local output wires; auxiliary wires require the object form",
                ));
            }
            if !spec.object_form {
                circuit = circuit.minimum_width(task.outputs.len());
            }
            if circuit.num_qubits() > options.max_qubits {
                return Err(TaskError::new(
                    "ideal_state",
                    "target width exceeds max_qubits",
                ));
            }
            let outputs = spec
                .outputs
                .clone()
                .unwrap_or_else(|| (0..task.outputs.len()).collect());
            if outputs.len() != task.outputs.len() {
                return Err(TaskError::new(
                    "ideal_state.outputs",
                    "target and physical output mappings must have the same length",
                ));
            }
            validate_indices("ideal_state.outputs", &outputs, circuit.num_qubits(), true)?;
            let selected = outputs.iter().copied().collect::<BTreeSet<_>>();
            for &qubit in spec.postselection.keys() {
                if qubit >= circuit.num_qubits() {
                    return Err(TaskError::new(
                        "ideal_state.postselection",
                        format!(
                            "qubit {qubit} is outside target width {}",
                            circuit.num_qubits()
                        ),
                    ));
                }
                if selected.contains(&qubit) {
                    return Err(TaskError::new(
                        "ideal_state.postselection",
                        "projected auxiliary wires must be disjoint from target outputs",
                    ));
                }
            }
            Some(LoadedTarget {
                circuit,
                outputs,
                postselection: spec.postselection.clone(),
            })
        } else {
            None
        };
        let recovery = if let Some(path) = &self.recovery {
            let circuit =
                read_circuit(path, "recovery", options)?.minimum_width(task.outputs.len());
            if circuit.num_qubits() > options.max_qubits {
                return Err(TaskError::new(
                    "recovery",
                    "recovery width exceeds max_qubits",
                ));
            }
            for op in circuit.operations() {
                let restriction = match &op.operation {
                    Operation::Noise { .. } => {
                        Some("terminal recovery must not declare noise channels")
                    }
                    Operation::Measure { report_flip, .. }
                        if report_flip != &crate::probability::Probability::zero() =>
                    {
                        Some(
                            "terminal recovery measurements must have zero report-flip probability",
                        )
                    }
                    Operation::Postselect { .. } => {
                        Some("terminal recovery must preserve trace and cannot postselect")
                    }
                    _ => None,
                };
                if let Some(message) = restriction {
                    return Err(module_operation_error(
                        "recovery",
                        path,
                        &op.location,
                        message,
                    ));
                }
            }
            Some(circuit)
        } else {
            None
        };
        Ok(LoadedModules {
            task,
            ideal,
            recovery,
        })
    }
}
#[derive(Clone, Debug)]
pub struct ValidatedTask {
    outputs: Vec<usize>,
    error_observables: Option<Vec<usize>>,
    sample_records: Vec<usize>,
    observables: Vec<PauliWord>,
}
impl ValidatedTask {
    pub fn outputs(&self) -> &[usize] {
        &self.outputs
    }
    pub fn error_observables(&self) -> Option<&[usize]> {
        self.error_observables.as_deref()
    }
    pub fn sample_records(&self) -> &[usize] {
        &self.sample_records
    }
    pub fn observables(&self) -> &[PauliWord] {
        &self.observables
    }
}
#[derive(Clone, Debug)]
pub struct LoadedTarget {
    circuit: Circuit,
    outputs: Vec<usize>,
    postselection: BTreeMap<usize, bool>,
}
impl LoadedTarget {
    pub fn circuit(&self) -> &Circuit {
        &self.circuit
    }
    pub fn outputs(&self) -> &[usize] {
        &self.outputs
    }
    pub fn postselection(&self) -> &BTreeMap<usize, bool> {
        &self.postselection
    }
}
#[derive(Clone, Debug)]
pub struct LoadedModules {
    task: ValidatedTask,
    ideal: Option<LoadedTarget>,
    recovery: Option<Circuit>,
}
impl LoadedModules {
    pub fn task(&self) -> &ValidatedTask {
        &self.task
    }
    pub fn ideal_state(&self) -> Option<&LoadedTarget> {
        self.ideal.as_ref()
    }
    pub fn recovery(&self) -> Option<&Circuit> {
        self.recovery.as_ref()
    }
}
fn validate_indices(
    field: &str,
    values: &[usize],
    width: usize,
    unique: bool,
) -> Result<(), TaskError> {
    let mut seen = BTreeSet::new();
    for &index in values {
        if index >= width {
            return Err(TaskError::new(
                field,
                format!("index {index} is outside width {width}"),
            ));
        }
        if unique && !seen.insert(index) {
            return Err(TaskError::new(field, format!("index {index} is repeated")));
        }
    }
    Ok(())
}
fn module_operation_error(
    field: &str,
    path: &Path,
    location: &SourceLocation,
    message: &str,
) -> TaskError {
    let mut context = format!("{}:{}:{}", path.display(), location.line, location.column);
    if !location.repeat.is_empty() {
        context.push_str(&format!(" (repeat {:?})", location.repeat));
    }
    TaskError::new(field, format!("{context}: {message}"))
}
fn read_circuit(path: &Path, field: &str, options: &ParseOptions) -> Result<Circuit, TaskError> {
    use std::io::Read;
    let file = std::fs::File::open(path)
        .map_err(|e| TaskError::new(field, format!("{}: {e}", path.display())))?;
    let mut bytes = Vec::new();
    file.take((options.max_source_bytes as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| TaskError::new(field, format!("{}: {e}", path.display())))?;
    if bytes.len() > options.max_source_bytes {
        return Err(TaskError::new(
            field,
            format!(
                "{}: module exceeds max_source_bytes = {}",
                path.display(),
                options.max_source_bytes
            ),
        ));
    }
    let text = String::from_utf8(bytes).map_err(|_| {
        TaskError::new(
            field,
            format!("{}: circuit module is not UTF-8", path.display()),
        )
    })?;
    let mut options = options.clone();
    options.source = Some(path.display().to_string());
    parse_circuit(&text, &options).map_err(|e| TaskError::new(field, e.to_string()))
}
