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
pub struct DatasetExportOptions {
    pub circuit: PathBuf,
    pub shots: u64,
    pub mode: String,
    pub public_out: PathBuf,
    pub private_out: PathBuf,
    pub seed: Option<u64>,
    pub logical_x_qubits: Option<String>,
    pub logical_z_qubits: Option<String>,
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
        let code = if message.contains("format") {
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
    let code = if message.contains("format") {
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
