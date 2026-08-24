//! Pipeline verbs routed through the unified `rustqec` command.
//!
//! Each verb wraps the corresponding `rstim` library entry point (the same
//! ones the `rstim` CLI uses) and adds the automation-facing contract:
//! structured errors with stable codes, a versioned JSON success envelope,
//! and declared artifacts.

use std::fs;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{CommandError, ErrorFormat};

pub const CIRCUIT_GEN_COMMAND: &str = "circuit.gen";
pub const CIRCUIT_SAMPLE_COMMAND: &str = "circuit.sample";
pub const CIRCUIT_DETECT_COMMAND: &str = "circuit.detect";
pub const CIRCUIT_DEM_COMMAND: &str = "circuit.dem";
pub const DATASET_EXPORT_COMMAND: &str = "dataset.export";
pub const DATASET_IMPORT_COMMAND: &str = "dataset.import";

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum PipelineFormat {
    Human,
    Json,
}

impl PipelineFormat {
    pub fn is_json(self, error_format: Option<ErrorFormat>) -> bool {
        match error_format {
            Some(ErrorFormat::Human) => false,
            Some(ErrorFormat::Json) => true,
            None => matches!(self, PipelineFormat::Json),
        }
    }
}

#[derive(Debug)]
pub struct GenOptions {
    pub code: String,
    pub task: String,
    pub distance: usize,
    pub rounds: usize,
    pub noise: f64,
    pub after_clifford_loss_probability: f64,
    pub operation_loss_probability: f64,
    pub measurement_loss_probability: f64,
    pub out: PathBuf,
    pub format: PipelineFormat,
}

#[derive(Serialize)]
pub struct GenResult {
    pub code: String,
    pub task: String,
    pub distance: usize,
    pub rounds: usize,
    pub circuit: String,
}

#[derive(Debug)]
pub struct SampleOptions {
    pub shots: u64,
    pub input: Option<PathBuf>,
    pub out: PathBuf,
    pub out_format: String,
    pub seed: Option<u64>,
    pub skip_reference_sample: bool,
    pub format: PipelineFormat,
}

#[derive(Serialize)]
pub struct SampleResult {
    pub shots: u64,
    pub num_measurements: usize,
    pub out: String,
    pub out_format: String,
}

#[derive(Debug)]
pub struct DetectOptions {
    pub shots: u64,
    pub input: Option<PathBuf>,
    pub out: PathBuf,
    pub out_format: String,
    pub seed: Option<u64>,
    pub append_observables: bool,
    pub obs_out: Option<PathBuf>,
    pub obs_out_format: String,
    pub format: PipelineFormat,
}

#[derive(Serialize)]
pub struct DetectResult {
    pub shots: u64,
    pub num_detectors: usize,
    pub out: String,
    pub out_format: String,
    pub observables_out: Option<String>,
}

#[derive(Debug)]
pub struct DemOptions {
    pub input: Option<PathBuf>,
    pub out: PathBuf,
    pub approximate_disjoint_errors: bool,
    pub allow_gauge_detectors: bool,
    pub decompose_errors: bool,
    pub format: PipelineFormat,
}

#[derive(Serialize)]
pub struct DemResult {
    pub num_detectors: usize,
    pub num_observables: usize,
    pub out: String,
}

#[derive(Debug)]
pub struct DatasetImportOptions {
    pub circuit: PathBuf,
    pub shots: PathBuf,
    pub shots_format: String,
    pub out: PathBuf,
    pub loss_log: Option<PathBuf>,
    pub format: PipelineFormat,
}

#[derive(Serialize)]
pub struct DatasetImportResult {
    pub shots: usize,
    pub measurements: usize,
    pub loss_flags: usize,
    pub out: String,
}

/// Packages a third-party-produced circuit and shot payload into a public
/// decoder dataset bundle. The staged bundle is validated with exactly the
/// checks `decode` performs (manifest structure, hashes, row widths, and the
/// full loss-visible circuit subset compilation) before it is published
/// atomically; a failed import leaves no output behind.
pub fn run_dataset_import(
    options: &DatasetImportOptions,
    error_format: Option<ErrorFormat>,
) -> Result<DatasetImportResult, CommandError> {
    let json = options.format.is_json(error_format);
    let text = fs::read_to_string(&options.circuit).map_err(|error| {
        command_error(
            DATASET_IMPORT_COMMAND,
            "input_error",
            format!("failed to read {}: {error}", options.circuit.display()),
            json,
        )
    })?;
    validate_circuit(DATASET_IMPORT_COMMAND, &text, json)?;
    let summary = rstim::stats::summarize_text(&text).map_err(|message| {
        command_error(DATASET_IMPORT_COMMAND, "invalid_circuit", message, json)
    })?;
    let row_bits = summary.num_measurements;
    if row_bits == 0 {
        return Err(command_error(
            DATASET_IMPORT_COMMAND,
            "invalid_arguments",
            "circuit has no measurement records to import".to_string(),
            json,
        ));
    }
    let shots_b8 = read_shots_payload(options, row_bits, json)?;
    let row_bytes = row_bits.div_ceil(8);
    let shots = shots_b8.len() / row_bytes;
    if shots == 0 {
        return Err(command_error(
            DATASET_IMPORT_COMMAND,
            "invalid_arguments",
            "shots payload is empty".to_string(),
            json,
        ));
    }
    if options.out.exists() {
        return Err(command_error(
            DATASET_IMPORT_COMMAND,
            "output_error",
            format!("{} already exists", options.out.display()),
            json,
        ));
    }
    let staging = options.out.with_file_name(format!(
        ".{}.import-staging-{}",
        options
            .out
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "dataset".to_string()),
        std::process::id()
    ));
    fs::create_dir(&staging).map_err(|error| {
        command_error(
            DATASET_IMPORT_COMMAND,
            "output_error",
            format!("failed to create {}: {error}", staging.display()),
            json,
        )
    })?;
    let staged = (|| -> Result<Vec<usize>, CommandError> {
        crate::decode::write_public_bundle(&staging, &text, &shots_b8, shots).map_err(
            |failure| command_error(DATASET_IMPORT_COMMAND, failure.code, failure.message, json),
        )?;
        let loss_flags = crate::decode::validate_public_bundle(&staging).map_err(|failure| {
            command_error(DATASET_IMPORT_COMMAND, failure.code, failure.message, json)
        })?;
        if let Some(loss_log) = &options.loss_log {
            check_loss_log(loss_log, &loss_flags, &shots_b8, row_bits, shots, json)?;
        }
        Ok(loss_flags)
    })();
    let loss_flags = match staged {
        Ok(loss_flags) => loss_flags,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
    };
    fs::rename(&staging, &options.out).map_err(|error| {
        let _ = fs::remove_dir_all(&staging);
        command_error(
            DATASET_IMPORT_COMMAND,
            "output_error",
            format!("failed to publish {}: {error}", options.out.display()),
            json,
        )
    })?;
    Ok(DatasetImportResult {
        shots,
        measurements: row_bits,
        loss_flags: loss_flags.len(),
        out: options.out.display().to_string(),
    })
}

fn read_shots_payload(
    options: &DatasetImportOptions,
    row_bits: usize,
    json: bool,
) -> Result<Vec<u8>, CommandError> {
    let invalid =
        |message: String| command_error(DATASET_IMPORT_COMMAND, "invalid_arguments", message, json);
    match options.shots_format.as_str() {
        "01" => {
            let text = fs::read_to_string(&options.shots).map_err(|error| {
                command_error(
                    DATASET_IMPORT_COMMAND,
                    "input_error",
                    format!("failed to read {}: {error}", options.shots.display()),
                    json,
                )
            })?;
            let row_bytes = row_bits.div_ceil(8);
            let mut packed = Vec::new();
            for (line_number, line) in text.lines().enumerate() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if line.len() != row_bits || !line.chars().all(|c| c == '0' || c == '1') {
                    return Err(invalid(format!(
                        "01 row {} must contain exactly {row_bits} bits",
                        line_number + 1
                    )));
                }
                let mut row = vec![0u8; row_bytes];
                for (bit, c) in line.chars().enumerate() {
                    if c == '1' {
                        row[bit / 8] |= 1 << (bit % 8);
                    }
                }
                packed.extend_from_slice(&row);
            }
            Ok(packed)
        }
        "b8" => {
            let bytes = fs::read(&options.shots).map_err(|error| {
                command_error(
                    DATASET_IMPORT_COMMAND,
                    "input_error",
                    format!("failed to read {}: {error}", options.shots.display()),
                    json,
                )
            })?;
            let row_bytes = row_bits.div_ceil(8);
            if bytes.len() % row_bytes != 0 {
                return Err(invalid(format!(
                    "b8 payload has {} bytes, not a multiple of the {row_bytes}-byte row",
                    bytes.len()
                )));
            }
            let padding = row_bits % 8;
            if padding != 0 {
                let mask = 0xffu8 << padding;
                for (shot, row) in bytes.chunks_exact(row_bytes).enumerate() {
                    if row[row_bytes - 1] & mask != 0 {
                        return Err(invalid(format!("b8 row {shot} has nonzero padding bits")));
                    }
                }
            }
            Ok(bytes)
        }
        other => Err(invalid(format!(
            "unknown shots format {other:?}; expected 01 or b8"
        ))),
    }
}

/// Cross-checks a loss sidecar (`rustqec.loss-log.v1`: per-shot lists of
/// loss-visible readout ordinals) against the flag bits actually present in
/// the packed shots payload.
fn check_loss_log(
    path: &Path,
    loss_flags: &[usize],
    shots_b8: &[u8],
    row_bits: usize,
    shots: usize,
    json: bool,
) -> Result<(), CommandError> {
    let text = fs::read_to_string(path).map_err(|error| {
        command_error(
            DATASET_IMPORT_COMMAND,
            "input_error",
            format!("failed to read {}: {error}", path.display()),
            json,
        )
    })?;
    let log: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
        command_error(
            DATASET_IMPORT_COMMAND,
            "invalid_arguments",
            format!("invalid loss log: {error}"),
            json,
        )
    })?;
    if log["schema_version"] != "rustqec.loss-log.v1" {
        return Err(command_error(
            DATASET_IMPORT_COMMAND,
            "invalid_arguments",
            "loss log must declare schema_version rustqec.loss-log.v1".to_string(),
            json,
        ));
    }
    let entries = log["shots"].as_array().ok_or_else(|| {
        command_error(
            DATASET_IMPORT_COMMAND,
            "invalid_arguments",
            "loss log must contain a shots array".to_string(),
            json,
        )
    })?;
    if entries.len() != shots {
        return Err(command_error(
            DATASET_IMPORT_COMMAND,
            "loss_log_mismatch",
            format!(
                "loss log lists {} shots, payload carries {shots}",
                entries.len()
            ),
            json,
        ));
    }
    let row_bytes = row_bits.div_ceil(8);
    for (shot, entry) in entries.iter().enumerate() {
        let declared = entry.as_array().ok_or_else(|| {
            command_error(
                DATASET_IMPORT_COMMAND,
                "invalid_arguments",
                format!("loss log shot {shot} must be an array of readout ordinals"),
                json,
            )
        })?;
        let mut declared: Vec<usize> = declared
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .map(|ordinal| ordinal as usize)
                    .ok_or_else(|| {
                        command_error(
                            DATASET_IMPORT_COMMAND,
                            "invalid_arguments",
                            format!("loss log shot {shot} contains a non-integer ordinal"),
                            json,
                        )
                    })
            })
            .collect::<Result<_, _>>()?;
        declared.sort_unstable();
        let row = &shots_b8[shot * row_bytes..(shot + 1) * row_bytes];
        let actual: Vec<usize> = loss_flags
            .iter()
            .enumerate()
            .filter_map(|(ordinal, &flag)| {
                ((row[flag / 8] >> (flag % 8)) & 1 == 1).then_some(ordinal)
            })
            .collect();
        if declared != actual {
            return Err(command_error(
                DATASET_IMPORT_COMMAND,
                "loss_log_mismatch",
                format!(
                    "loss log shot {shot} declares readouts {declared:?} but flag bits mark {actual:?}"
                ),
                json,
            ));
        }
    }
    Ok(())
}

#[derive(Debug)]
pub struct DatasetExportOptions {
    pub circuit: PathBuf,
    pub shots: u64,
    pub mode: String,
    pub public_out: PathBuf,
    pub private_out: PathBuf,
    pub seed: Option<u64>,
    pub logical_x_qubits: Option<String>,
    pub logical_z_qubits: Option<String>,
    pub error_trace: bool,
    pub format: PipelineFormat,
}

#[derive(Serialize)]
pub struct DatasetExportResult {
    pub shots: u64,
    pub mode: String,
    pub public_out: String,
    pub private_out: String,
}

fn command_error(
    command: &'static str,
    code: &'static str,
    message: String,
    json: bool,
) -> CommandError {
    CommandError {
        command,
        code,
        message,
        json,
        exit_code: 2,
    }
}

fn read_circuit_text(
    command: &'static str,
    input: Option<&Path>,
    stdin: &mut dyn Read,
    json: bool,
) -> Result<String, CommandError> {
    match input {
        Some(path) => fs::read_to_string(path).map_err(|error| {
            command_error(
                command,
                "input_error",
                format!("failed to read {}: {error}", path.display()),
                json,
            )
        }),
        None => {
            let mut text = String::new();
            stdin
                .read_to_string(&mut text)
                .map_err(|error| {
                    command_error(
                        command,
                        "input_error",
                        format!("failed to read stdin: {error}"),
                        json,
                    )
                })
                .map(|_| text)
        }
    }
}

/// Pre-validates the circuit so parse and semantic failures carry the stable
/// `invalid_circuit` code instead of leaking into a generic execution error.
fn validate_circuit(command: &'static str, text: &str, json: bool) -> Result<(), CommandError> {
    rstim::validation::parse_and_validate(text)
        .map(|_| ())
        .map_err(|message| command_error(command, "invalid_circuit", message, json))
}

fn write_artifact(
    command: &'static str,
    bytes: &[u8],
    out: &Path,
    json: bool,
) -> Result<(), CommandError> {
    let file = fs::File::create(out).map_err(|error| {
        command_error(
            command,
            "output_error",
            format!("failed to create {}: {error}", out.display()),
            json,
        )
    })?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(bytes)
        .and_then(|_| writer.flush())
        .map_err(|error| {
            command_error(
                command,
                "output_error",
                format!("failed to write {}: {error}", out.display()),
                json,
            )
        })
}

pub fn run_gen(
    options: &GenOptions,
    error_format: Option<ErrorFormat>,
) -> Result<GenResult, CommandError> {
    let json = options.format.is_json(error_format);
    let is_midswap = options.code == "surface_code" && options.task == "rotated_memory_z_midswap";
    let mut buffer = Vec::new();
    if is_midswap {
        if options.after_clifford_loss_probability != 0.0 {
            return Err(command_error(
                CIRCUIT_GEN_COMMAND,
                "invalid_arguments",
                "after_clifford_loss_probability is not used by the Mid-SWAP task; use operation_loss_probability"
                    .to_string(),
                json,
            ));
        }
        let circuit = rstim::codegen::rotated_memory_z_midswap(rstim::codegen::MidSwapConfig {
            distance: options.distance,
            rounds: options.rounds,
            pauli_probability: options.noise,
            operation_loss_probability: options.operation_loss_probability,
            measurement_loss_probability: options.measurement_loss_probability,
        })
        .map_err(|error| {
            command_error(
                CIRCUIT_GEN_COMMAND,
                "invalid_arguments",
                error.to_string(),
                json,
            )
        })?;
        buffer.extend_from_slice(circuit.as_bytes());
    } else {
        if options.operation_loss_probability != 0.0 || options.measurement_loss_probability != 0.0
        {
            return Err(command_error(
                CIRCUIT_GEN_COMMAND,
                "invalid_arguments",
                "operation_loss_probability and measurement_loss_probability are only valid for surface_code/rotated_memory_z_midswap"
                    .to_string(),
                json,
            ));
        }
        let mut params = rstim::codegen::NoiseParams::uniform(options.noise);
        params.after_clifford_loss_probability = options.after_clifford_loss_probability;
        rstim::cli::run_gen_with_params(
            &options.code,
            &options.task,
            options.distance,
            options.rounds,
            params,
            &mut buffer,
        )
        .map_err(|message| {
            command_error(CIRCUIT_GEN_COMMAND, "invalid_arguments", message, json)
        })?;
    }
    write_artifact(CIRCUIT_GEN_COMMAND, &buffer, &options.out, json)?;
    Ok(GenResult {
        code: options.code.clone(),
        task: options.task.clone(),
        distance: options.distance,
        rounds: options.rounds,
        circuit: options.out.display().to_string(),
    })
}

pub fn run_sample(
    options: &SampleOptions,
    error_format: Option<ErrorFormat>,
    stdin: &mut dyn Read,
) -> Result<SampleResult, CommandError> {
    let json = options.format.is_json(error_format);
    let text = read_circuit_text(
        CIRCUIT_SAMPLE_COMMAND,
        options.input.as_deref(),
        stdin,
        json,
    )?;
    validate_circuit(CIRCUIT_SAMPLE_COMMAND, &text, json)?;
    let shots = usize::try_from(options.shots).map_err(|_| {
        command_error(
            CIRCUIT_SAMPLE_COMMAND,
            "invalid_arguments",
            "--shots is too large for this platform".to_string(),
            json,
        )
    })?;
    let summary = rstim::stats::summarize_text(&text).map_err(|message| {
        command_error(CIRCUIT_SAMPLE_COMMAND, "invalid_circuit", message, json)
    })?;
    let mut buffer = Vec::new();
    rstim::cli::run_sample(
        &text,
        shots,
        &options.out_format,
        options.seed,
        options.skip_reference_sample,
        &mut buffer,
    )
    .map_err(|message| {
        let code = if message.starts_with("unknown output format") {
            "invalid_arguments"
        } else {
            "execution_error"
        };
        command_error(CIRCUIT_SAMPLE_COMMAND, code, message, json)
    })?;
    write_artifact(CIRCUIT_SAMPLE_COMMAND, &buffer, &options.out, json)?;
    Ok(SampleResult {
        shots: options.shots,
        num_measurements: summary.num_measurements,
        out: options.out.display().to_string(),
        out_format: options.out_format.clone(),
    })
}

pub fn run_detect(
    options: &DetectOptions,
    error_format: Option<ErrorFormat>,
    stdin: &mut dyn Read,
) -> Result<DetectResult, CommandError> {
    let json = options.format.is_json(error_format);
    let text = read_circuit_text(
        CIRCUIT_DETECT_COMMAND,
        options.input.as_deref(),
        stdin,
        json,
    )?;
    validate_circuit(CIRCUIT_DETECT_COMMAND, &text, json)?;
    let shots = usize::try_from(options.shots).map_err(|_| {
        command_error(
            CIRCUIT_DETECT_COMMAND,
            "invalid_arguments",
            "--shots is too large for this platform".to_string(),
            json,
        )
    })?;
    let summary = rstim::stats::summarize_text(&text).map_err(|message| {
        command_error(CIRCUIT_DETECT_COMMAND, "invalid_circuit", message, json)
    })?;
    let mut buffer = Vec::new();
    match &options.obs_out {
        Some(obs_out) => {
            let mut obs_buffer = Vec::new();
            rstim::cli::run_detect_with_obs(
                &text,
                shots,
                &options.out_format,
                options.seed,
                options.append_observables,
                &mut buffer,
                &mut obs_buffer,
                &options.obs_out_format,
            )
            .map_err(|message| detect_error_code(message, json))?;
            write_artifact(CIRCUIT_DETECT_COMMAND, &buffer, &options.out, json)?;
            write_artifact(CIRCUIT_DETECT_COMMAND, &obs_buffer, obs_out, json)?;
        }
        None => {
            rstim::cli::run_detect(
                &text,
                shots,
                &options.out_format,
                options.seed,
                options.append_observables,
                &mut buffer,
            )
            .map_err(|message| detect_error_code(message, json))?;
            write_artifact(CIRCUIT_DETECT_COMMAND, &buffer, &options.out, json)?;
        }
    }
    Ok(DetectResult {
        shots: options.shots,
        num_detectors: summary.num_detectors,
        out: options.out.display().to_string(),
        out_format: options.out_format.clone(),
        observables_out: options
            .obs_out
            .as_ref()
            .map(|path| path.display().to_string()),
    })
}

fn detect_error_code(message: String, json: bool) -> CommandError {
    let code = if message.starts_with("unknown output format") {
        "invalid_arguments"
    } else {
        "execution_error"
    };
    command_error(CIRCUIT_DETECT_COMMAND, code, message, json)
}

pub fn run_dem(
    options: &DemOptions,
    error_format: Option<ErrorFormat>,
    stdin: &mut dyn Read,
) -> Result<DemResult, CommandError> {
    let json = options.format.is_json(error_format);
    let text = read_circuit_text(CIRCUIT_DEM_COMMAND, options.input.as_deref(), stdin, json)?;
    validate_circuit(CIRCUIT_DEM_COMMAND, &text, json)?;
    let summary = rstim::stats::summarize_text(&text)
        .map_err(|message| command_error(CIRCUIT_DEM_COMMAND, "invalid_circuit", message, json))?;
    let mut buffer = Vec::new();
    rstim::cli::run_analyze_errors_with_flags(
        &text,
        options.approximate_disjoint_errors,
        options.allow_gauge_detectors,
        options.decompose_errors,
        &mut buffer,
    )
    .map_err(|message| command_error(CIRCUIT_DEM_COMMAND, "execution_error", message, json))?;
    write_artifact(CIRCUIT_DEM_COMMAND, &buffer, &options.out, json)?;
    Ok(DemResult {
        num_detectors: summary.num_detectors,
        num_observables: summary.num_observables,
        out: options.out.display().to_string(),
    })
}

pub fn run_dataset_export(
    options: &DatasetExportOptions,
    error_format: Option<ErrorFormat>,
) -> Result<DatasetExportResult, CommandError> {
    let json = options.format.is_json(error_format);
    if options.logical_x_qubits.is_some() && options.logical_z_qubits.is_some() {
        return Err(command_error(
            DATASET_EXPORT_COMMAND,
            "invalid_arguments",
            "--logical-x-qubits and --logical-z-qubits are mutually exclusive".to_string(),
            json,
        ));
    }
    let text = fs::read_to_string(&options.circuit).map_err(|error| {
        command_error(
            DATASET_EXPORT_COMMAND,
            "input_error",
            format!("failed to read {}: {error}", options.circuit.display()),
            json,
        )
    })?;
    validate_circuit(DATASET_EXPORT_COMMAND, &text, json)?;
    let circuit_path = options.circuit.display().to_string();
    let public_out = options.public_out.display().to_string();
    let private_out = options.private_out.display().to_string();
    let logical_flip = match (
        options.logical_x_qubits.as_deref(),
        options.logical_z_qubits.as_deref(),
    ) {
        (Some(value), None) => Some(rstim::decoder_dataset::LogicalFlip::parse(
            rstim::decoder_dataset::LogicalPauli::X,
            value,
        )),
        (None, Some(value)) => Some(rstim::decoder_dataset::LogicalFlip::parse(
            rstim::decoder_dataset::LogicalPauli::Z,
            value,
        )),
        _ => None,
    }
    .transpose()
    .map_err(|message| command_error(DATASET_EXPORT_COMMAND, "invalid_arguments", message, json))?;
    rstim::cli::run_export_decoder_dataset_with_logical_flip(
        &circuit_path,
        options.shots,
        &options.mode,
        logical_flip,
        &public_out,
        &private_out,
        options.seed,
        options.error_trace,
    )
    .map_err(|message| {
        let code = if message.contains("unknown decoder dataset mode") {
            "invalid_arguments"
        } else {
            "execution_error"
        };
        command_error(DATASET_EXPORT_COMMAND, code, message, json)
    })?;
    Ok(DatasetExportResult {
        shots: options.shots,
        mode: options.mode.clone(),
        public_out,
        private_out,
    })
}
