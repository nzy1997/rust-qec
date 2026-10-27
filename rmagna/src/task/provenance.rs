//! Versioned identities of loaded content and declared configuration.
use super::{EvaluationOptions, LoadedModules, TaskRunOptions};
use crate::circuit::Circuit;
use crate::instrument::{InsertionPreparation, QueryPreparation};
use crate::noise::NoiseSampling;
use crate::numeric::NumericOptions;
use crate::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use crate::unitary::CompilerStrategy;
use serde_json::value::RawValue;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt};

const ENCODING: &str = "rmagna-provenance-v1";

#[derive(Clone, Copy, Debug)]
pub struct FingerprintOptions {
    /// Maximum canonical JSON bytes in each independently hashed category.
    pub max_json_bytes: usize,
    /// Root depth is zero; each array element or object value adds one.
    pub max_depth: usize,
}
impl Default for FingerprintOptions {
    fn default() -> Self {
        Self {
            max_json_bytes: 16 * 1024 * 1024,
            max_depth: 64,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FingerprintError {
    ByteLimit { limit: usize },
    DepthLimit { limit: usize },
    UnknownController { name: String },
    InvalidJson { message: String },
}
impl fmt::Display for FingerprintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByteLimit { limit } => write!(f, "canonical JSON exceeds {limit} bytes"),
            Self::DepthLimit { limit } => write!(f, "canonical JSON exceeds nesting depth {limit}"),
            Self::InvalidJson { message } => write!(f, "cannot decode raw JSON: {message}"),
            Self::UnknownController { name } => {
                write!(f, "identity material names unregistered controller {name}")
            }
        }
    }
}
impl std::error::Error for FingerprintError {}

struct Encoder {
    hasher: Sha256,
    bytes: usize,
    options: FingerprintOptions,
}
impl Encoder {
    fn new(domain: &str, options: FingerprintOptions) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(ENCODING.as_bytes());
        hasher.update([0]);
        hasher.update(domain.as_bytes());
        hasher.update([0]);
        Self {
            hasher,
            bytes: 0,
            options,
        }
    }
    fn finish(self) -> String {
        format!("{:x}", self.hasher.finalize())
    }
    fn raw_value(&mut self, value: &RawValue, depth: usize) -> Result<(), FingerprintError> {
        if depth > self.options.max_depth {
            return Err(FingerprintError::DepthLimit {
                limit: self.options.max_depth,
            });
        }
        let text = value.get().trim();
        match text.as_bytes().first().copied() {
            Some(b'{') => {
                self.write(b"{")?;
                let values: BTreeMap<String, Box<RawValue>> = read_raw_json(text)?;
                for (index, (key, value)) in values.iter().enumerate() {
                    if index != 0 {
                        self.write(b",")?;
                    }
                    self.string(key)?;
                    self.write(b":")?;
                    self.raw_value(value, depth.saturating_add(1))?;
                }
                self.write(b"}")
            }
            Some(b'[') => {
                self.write(b"[")?;
                let values: Vec<Box<RawValue>> = read_raw_json(text)?;
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        self.write(b",")?;
                    }
                    self.raw_value(value, depth.saturating_add(1))?;
                }
                self.write(b"]")
            }
            Some(b'"') => self.string(&read_raw_json::<String>(text)?),
            // RawValue has already checked the JSON grammar. Number tokens
            // are emitted directly, without binary64 or arbitrary_precision.
            Some(b'-' | b'0'..=b'9' | b't' | b'f' | b'n') => self.write(text.as_bytes()),
            _ => Err(FingerprintError::InvalidJson {
                message: "expected a JSON value".to_owned(),
            }),
        }
    }
    fn write(&mut self, bytes: &[u8]) -> Result<(), FingerprintError> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|&n| n <= self.options.max_json_bytes)
            .ok_or(FingerprintError::ByteLimit {
                limit: self.options.max_json_bytes,
            })?;
        self.hasher.update(bytes);
        Ok(())
    }
    fn string(&mut self, text: &str) -> Result<(), FingerprintError> {
        self.write(b"\"")?;
        // Non-ASCII UTF-8 is retained. Only JSON-required escapes and the five
        // conventional control escapes are emitted, with lowercase hex digits.
        for &byte in text.as_bytes() {
            match byte {
                b'"' => self.write(b"\\\"")?,
                b'\\' => self.write(b"\\\\")?,
                b'\n' => self.write(b"\\n")?,
                b'\r' => self.write(b"\\r")?,
                b'\t' => self.write(b"\\t")?,
                8 => self.write(b"\\b")?,
                12 => self.write(b"\\f")?,
                0..=31 => {
                    let hex = b"0123456789abcdef";
                    self.write(&[
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        hex[(byte / 16) as usize],
                        hex[(byte % 16) as usize],
                    ])?;
                }
                _ => self.write(&[byte])?,
            }
        }
        self.write(b"\"")
    }
    fn value(&mut self, value: &Value, depth: usize) -> Result<(), FingerprintError> {
        if depth > self.options.max_depth {
            return Err(FingerprintError::DepthLimit {
                limit: self.options.max_depth,
            });
        }
        match value {
            Value::Null => self.write(b"null"),
            Value::Bool(value) => self.write(if *value { b"true" } else { b"false" }),
            Value::Number(value) => self.write(value.to_string().as_bytes()),
            Value::String(value) => self.string(value),
            Value::Array(values) => {
                self.write(b"[")?;
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        self.write(b",")?;
                    }
                    self.value(value, depth.saturating_add(1))?;
                }
                self.write(b"]")
            }
            Value::Object(values) => {
                self.write(b"{")?;
                // Sort explicitly even if serde_json's preserve_order feature
                // is enabled by another workspace consumer.
                let mut keys = values.keys().collect::<Vec<_>>();
                keys.sort_unstable();
                for (index, key) in keys.into_iter().enumerate() {
                    if index != 0 {
                        self.write(b",")?;
                    }
                    self.string(key)?;
                    self.write(b":")?;
                    self.value(&values[key], depth.saturating_add(1))?;
                }
                self.write(b"}")
            }
        }
    }
}
fn read_raw_json<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, FingerprintError> {
    serde_json::from_str(text).map_err(|error| FingerprintError::InvalidJson {
        message: error.to_string(),
    })
}
fn fingerprint(
    domain: &str,
    value: &Value,
    options: FingerprintOptions,
) -> Result<String, FingerprintError> {
    let mut encoder = Encoder::new(domain, options);
    encoder.value(value, 0)?;
    Ok(encoder.finish())
}
/// SHA-256 of `rmagna-provenance-v1\0json\0` followed by canonical UTF-8 JSON.
/// Keys sort lexicographically; arrays retain order; numbers use serde_json's
/// Display representation. This is a versioned local encoding, not RFC 8785.
pub fn canonical_fingerprint(
    value: &Value,
    options: FingerprintOptions,
) -> Result<String, FingerprintError> {
    fingerprint("json", value, options)
}

/// Encode checked raw JSON in the same json domain while preserving number
/// token spelling, including arbitrary-width integers and large exponents.
/// Object keys sort; duplicate keys keep the last value, as in serde_json Value.
/// Parsed strings use the same UTF-8/escape rules. Limits bound encoded output.
pub fn canonical_raw_fingerprint(
    value: &RawValue,
    options: FingerprintOptions,
) -> Result<String, FingerprintError> {
    let mut encoder = Encoder::new("json", options);
    encoder.raw_value(value, 0)?;
    Ok(encoder.finish())
}

pub struct FingerprintInputs<'a> {
    pub physical: &'a Circuit,
    pub modules: &'a LoadedModules,
    pub controllers: &'a ControllerRegistry,
    /// Optional caller declarations, not independently verified attestations.
    pub controller_identities: &'a BTreeMap<String, Value>,
    pub evaluation: EvaluationOptions,
    pub run: TaskRunOptions,
    pub sampling: &'a NoiseSampling,
    pub numeric: NumericOptions,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentFingerprints {
    pub circuit: String,
    pub task: String,
    pub ideal_state: String,
    pub recovery: String,
    pub controllers: String,
    pub options: String,
    pub sampling: String,
}
impl ContentFingerprints {
    pub fn to_json(&self) -> Value {
        json!({
            "algorithm": "sha256", "encoding": ENCODING,
            "controller_attestation": "caller_declared_unverified",
            "circuit": self.circuit, "task": self.task,
            "ideal_state": self.ideal_state, "recovery": self.recovery,
            "controllers": self.controllers, "options": self.options,
            "sampling": self.sampling,
        })
    }
}
fn circuit(circuit: &Circuit) -> Value {
    json!({"format": circuit.format_version(), "source": circuit.to_text(), "effective_qubits": circuit.num_qubits()})
}
fn preparation(mode: QueryPreparation) -> Value {
    match mode {
        QueryPreparation::Cold => json!({"mode": "cold"}),
        QueryPreparation::Trace => json!({"mode": "trace"}),
        QueryPreparation::Native(options) => {
            let s = options.selection;
            let c = s.construction;
            json!({
                "mode": mode.name(),
                "selection": {
                    "max_quotient_rank": s.max_quotient_rank,
                    "max_core_table_bytes": s.max_core_table_bytes,
                    "max_signature_work": s.max_signature_work.to_string(),
                    "max_transform_work": s.max_transform_work.to_string(),
                    "construction": {"max_json_bytes": c.max_json_bytes, "max_entries": c.max_entries,
                        "max_dimension": c.max_dimension, "max_table_values": c.max_table_values,
                        "max_terms": c.max_terms, "max_constraint_rows": c.max_constraint_rows,
                        "max_parity_rows": c.max_parity_rows, "max_verification_work": c.max_verification_work.to_string()},
                    "preparation": {"max_terms": s.preparation.max_terms, "max_geometry_words": s.preparation.max_geometry_words,
                        "core": {"max_dimension": s.preparation.core.max_dimension,
                            "max_table_values": s.preparation.core.max_table_values,
                            "max_work": s.preparation.core.max_work.to_string()}},
                },
                "injection": options.injection.map(|i| json!({"max_work": i.max_work.to_string(), "max_geometry_words": i.max_geometry_words, "max_terms": i.max_terms})),
            })
        }
        QueryPreparation::FixedCatalogue(options) => json!({
            "mode": "catalogue",
            "fibers": {"max_search_rank": options.fibers.max_search_rank, "attempts": options.fibers.attempts},
            "catalogue": {"max_terms": options.catalogue.max_terms, "max_geometry_words": options.catalogue.max_geometry_words,
                "core": {"max_dimension": options.catalogue.core.max_dimension, "max_table_values": options.catalogue.core.max_table_values,
                    "max_work": options.catalogue.core.max_work.to_string()}},
        }),
        QueryPreparation::FixedFibers(options) => json!({
            "mode": "fixed", "max_search_rank": options.max_search_rank, "attempts": options.attempts,
        }),
    }
}
pub(crate) fn resolved_options(input: &FingerprintInputs<'_>) -> Value {
    let mut options = execution_configuration(input.evaluation, input.run);
    options["numeric_work_bits"] = json!(input.numeric.max_work_bits);
    options
}
pub(crate) fn execution_configuration(evaluation: EvaluationOptions, run: TaskRunOptions) -> Value {
    let runtime = evaluation.runtime;
    let compile = runtime.compile;
    let options = json!({
        "history_sampler": match runtime.history_sampler { HistorySampler::ExactMarginal => "exact_marginal", HistorySampler::AffineSupport => "affine_support" },
        "causal_strategy": match runtime.strategy { CausalStrategy::FixedTemplate => "fixed_template", CausalStrategy::ColdPrefix => "cold_prefix" },
        "max_history_queries": runtime.max_history_queries, "max_history_inputs": runtime.max_history_inputs,
        "max_prefix_compiles": runtime.max_prefix_compiles,
        "compile": {
            "insertion_preparation": match compile.insertion_preparation {
                InsertionPreparation::Direct=>json!({"mode":"direct"}),
                InsertionPreparation::Prepared(options)=>json!({"mode":"prepared","max_catalog_entries":options.max_catalog_entries,"max_table_bytes":options.max_table_bytes}),
            },
            "compiler_strategy": match compile.compiler_strategy { CompilerStrategy::ParityCopies => "parity_copies", CompilerStrategy::SharedCommutingLayer => "shared_commuting_layer" },
            "probability_preparation": preparation(compile.probability_preparation),
            "max_prepared_probabilities": compile.max_prepared_probabilities,
            "density_preparation": preparation(compile.density_preparation),
            "max_prepared_densities": compile.max_prepared_densities,
            "moment_preparation": preparation(compile.moment_preparation),
            "max_prepared_moments": compile.max_prepared_moments,
            "max_coherent_qubits": compile.max_coherent_qubits, "max_gates": compile.max_gates,
            "max_noise_sites": compile.max_noise_sites, "max_path_bits": compile.max_path_bits,
        },
        "noise": {"max_denominator_bits": runtime.noise.max_denominator_bits, "max_entropy_words": runtime.noise.max_entropy_words},
        "records": {"max_terms": runtime.sampling.max_terms, "max_entropy_words": runtime.sampling.max_entropy_words},
        "scalar_coefficient_bits": evaluation.scalars.max_coefficient_bits(),
        "field_coefficient_bits": evaluation.fields.max_coefficient_bits(),
        "run": {
            "execution_limits": run.execution_limits.to_json(),
            "shots": run.shots, "seed": run.seed,
            "max_retained_record_values": run.max_retained_record_values,
            "statistics": {"max_dimensions": run.statistics.max_dimensions, "coefficient_bits": run.statistics.arithmetic.max_coefficient_bits()},
        },
    });
    options
}

pub(crate) fn sampling_configuration(policy: &NoiseSampling) -> Value {
    match policy {
        NoiseSampling::Physical => json!({"mode": "physical"}),
        NoiseSampling::Importance { proposal_scale } => json!({
            "mode": "importance", "proposal_scale": {
                "numerator": proposal_scale.numer().to_string(), "denominator": proposal_scale.denom().to_string(),
            },
        }),
    }
}

/// Fingerprint loaded snapshots without opening paths or invoking controllers.
/// All seven categories must fit; errors return no partial fingerprint set.
pub fn fingerprint_content(
    input: &FingerprintInputs<'_>,
    limits: FingerprintOptions,
) -> Result<ContentFingerprints, FingerprintError> {
    fingerprint_with_options(input, resolved_options(input), limits)
}

/// Validate captured content without assuming a future run's limits or encoding.
pub(crate) fn validate_snapshot_fingerprints(
    input: &FingerprintInputs<'_>,
    limits: FingerprintOptions,
) -> Result<(), FingerprintError> {
    let mut options = resolved_options(input);
    options
        .as_object_mut()
        .expect("resolved options object")
        .remove("run");
    fingerprint_with_options(input, options, limits).map(|_| ())
}

fn fingerprint_with_options(
    input: &FingerprintInputs<'_>,
    options: Value,
    limits: FingerprintOptions,
) -> Result<ContentFingerprints, FingerprintError> {
    fingerprint_with_configuration(
        input,
        &options,
        &sampling_configuration(input.sampling),
        limits,
    )
}

pub(crate) fn fingerprint_with_configuration(
    input: &FingerprintInputs<'_>,
    options: &Value,
    sampling: &Value,
    limits: FingerprintOptions,
) -> Result<ContentFingerprints, FingerprintError> {
    let versions = input.controllers.versions();
    for name in input.controller_identities.keys() {
        if !versions.contains_key(name) {
            return Err(FingerprintError::UnknownController { name: name.clone() });
        }
    }
    let task = input.modules.task();
    let words = task
        .observables()
        .iter()
        .map(|word| {
            json!({
                "width": word.width(), "phase": word.phase(),
                "x": word.x().ones().collect::<Vec<_>>(), "z": word.z().ones().collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    Ok(ContentFingerprints {
        circuit: fingerprint("circuit", &circuit(input.physical), limits)?,
        task: fingerprint("task", &json!({"outputs": task.outputs(), "error_observables": task.error_observables(), "sample_records": task.sample_records(), "observables": words}), limits)?,
        ideal_state: fingerprint("ideal_state", &input.modules.ideal_state().map(|target| json!({"circuit": circuit(target.circuit()), "outputs": target.outputs(), "postselection": target.postselection()})).unwrap_or(Value::Null), limits)?,
        recovery: fingerprint("recovery", &input.modules.recovery().map(circuit).unwrap_or(Value::Null), limits)?,
        controllers: fingerprint("controllers", &json!({"attestation": "caller_declared_unverified", "versions": versions, "identities": input.controller_identities}), limits)?,
        options: fingerprint("options", options, limits)?,
        sampling: fingerprint("sampling", sampling, limits)?,
    })
}
