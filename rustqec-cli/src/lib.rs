use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum, error::ErrorKind};
use serde::Serialize;

mod decode;
mod pipeline;

pub const SCHEMA_VERSION: &str = "rustqec.cli.v1";
const CIRCUIT_STATS_COMMAND: &str = "circuit.stats";

#[derive(Debug, Parser)]
#[command(
    name = "rustqec",
    version,
    about = "Unified RustQEC command line interface"
)]
pub struct Cli {
    /// Select the error channel independently of command output
    #[arg(long, global = true, value_enum)]
    error_format: Option<ErrorFormat>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect and operate on Stim-like circuits
    Circuit {
        #[command(subcommand)]
        command: CircuitCommand,
    },
    /// Discover the commands and contracts implemented by this binary
    Capabilities {
        #[arg(long, value_enum)]
        format: CapabilitiesFormat,
    },
    /// Decode a public loss-visible decoder dataset
    Decode {
        /// Loss-aware decoder implementation
        #[arg(long, value_enum)]
        decoder: DecodeDecoder,
        /// Directory containing manifest.json, circuit.stim, and shots.b8
        #[arg(long)]
        dataset: PathBuf,
        /// Destination bit-packed observable predictions
        #[arg(long)]
        out: PathBuf,
        /// Destination structured decode statistics
        #[arg(long = "stats-out")]
        stats_out: PathBuf,
        /// Per-shot Envelope-MLE time limit
        #[arg(long = "shot-timeout-ms")]
        shot_timeout_ms: Option<u64>,
    },
    /// Produce decoder-ready datasets
    Dataset {
        #[command(subcommand)]
        command: DatasetCommand,
    },
}

#[derive(Debug, Subcommand)]
enum DatasetCommand {
    /// Export a public decoder dataset and private answer bundle
    Export {
        /// Circuit file to sample (loss-visible circuits supported)
        #[arg(long)]
        circuit: PathBuf,
        /// Number of shots to sample into the dataset
        #[arg(long)]
        shots: u64,
        /// Dataset mode: detectors or measurements_blinded
        #[arg(long)]
        mode: String,
        /// Destination directory for the public dataset bundle
        #[arg(long = "public-out")]
        public_out: PathBuf,
        /// Destination directory for the private answer bundle
        #[arg(long = "private-out")]
        private_out: PathBuf,
        /// Sampling seed for reproducible datasets
        #[arg(long)]
        seed: Option<u64>,
        /// Apply a logical-X flip on the given data qubits before export
        #[arg(long = "logical-x-qubits")]
        logical_x_qubits: Option<String>,
        /// Apply a logical-Z flip on the given data qubits before export
        #[arg(long = "logical-z-qubits")]
        logical_z_qubits: Option<String>,
        /// Select human-readable or versioned JSON output
        #[arg(long, value_enum, default_value_t = pipeline::PipelineFormat::Json)]
        format: pipeline::PipelineFormat,
    },
    /// Package a third-party circuit and shot payload into a public dataset
    Import {
        /// Loss-visible circuit file (must satisfy subset v1)
        #[arg(long)]
        circuit: PathBuf,
        /// Shot payload file produced by an external sampler
        #[arg(long)]
        shots: PathBuf,
        /// Shot payload encoding: 01 (text rows) or b8 (packed binary)
        #[arg(long = "shots-format", default_value = "01")]
        shots_format: String,
        /// Destination directory for the public dataset bundle
        #[arg(long)]
        out: PathBuf,
        /// Optional rustqec.loss-log.v1 sidecar to cross-check flag bits
        #[arg(long = "loss-log")]
        loss_log: Option<PathBuf>,
        /// Select human-readable or versioned JSON output
        #[arg(long, value_enum, default_value_t = pipeline::PipelineFormat::Json)]
        format: pipeline::PipelineFormat,
    },
}

#[derive(Debug, Subcommand)]
enum CircuitCommand {
    /// Summarize circuit structure and counts
    Stats {
        /// Read the circuit from a file instead of stdin
        #[arg(long = "in")]
        input: Option<PathBuf>,
        /// Select human-readable or versioned JSON output
        #[arg(long, value_enum, default_value_t = StatsFormat::Human)]
        format: StatsFormat,
    },
    /// Generate a built-in QEC circuit (common families and Mid-SWAP)
    Gen {
        /// Code family, e.g. surface_code
        #[arg(long)]
        code: String,
        /// Task within the family, e.g. rotated_memory_z_midswap
        #[arg(long)]
        task: String,
        /// Code distance
        #[arg(long)]
        distance: usize,
        /// Number of syndrome-extraction rounds
        #[arg(long)]
        rounds: usize,
        /// Uniform noise strength (after-Clifford depolarization)
        #[arg(long, default_value = "0")]
        noise: f64,
        /// Loss probability after Clifford gates (non-Mid-SWAP families)
        #[arg(long, default_value = "0")]
        after_clifford_loss_probability: f64,
        /// Operation-loss probability for gates and resets (Mid-SWAP only)
        #[arg(long, default_value = "0")]
        operation_loss_probability: f64,
        /// Measurement-loss probability (Mid-SWAP only)
        #[arg(long, default_value = "0")]
        measurement_loss_probability: f64,
        /// Destination circuit file
        #[arg(long)]
        out: PathBuf,
        /// Select human-readable or versioned JSON output
        #[arg(long, value_enum, default_value_t = pipeline::PipelineFormat::Json)]
        format: pipeline::PipelineFormat,
    },
    /// Sample measurement results from a circuit
    Sample {
        /// Number of shots
        #[arg(long)]
        shots: u64,
        /// Read the circuit from a file instead of stdin
        #[arg(long = "in")]
        input: Option<PathBuf>,
        /// Destination shot file
        #[arg(long)]
        out: PathBuf,
        /// Shot output format: 01, b8, r8, hits, ptb64
        #[arg(long = "out-format", default_value = "01")]
        out_format: String,
        /// Sampling seed
        #[arg(long)]
        seed: Option<u64>,
        /// Skip the reference noiseless sample (requires a noiseless circuit)
        #[arg(long = "skip-reference-sample")]
        skip_reference_sample: bool,
        /// Select human-readable or versioned JSON output
        #[arg(long, value_enum, default_value_t = pipeline::PipelineFormat::Json)]
        format: pipeline::PipelineFormat,
    },
    /// Sample detection events and observable flips from a circuit
    Detect {
        /// Number of shots
        #[arg(long)]
        shots: u64,
        /// Read the circuit from a file instead of stdin
        #[arg(long = "in")]
        input: Option<PathBuf>,
        /// Destination detection-event file
        #[arg(long)]
        out: PathBuf,
        /// Detection output format: 01, b8, r8, hits, dets, ptb64
        #[arg(long = "out-format", default_value = "01")]
        out_format: String,
        /// Sampling seed
        #[arg(long)]
        seed: Option<u64>,
        /// Append observable flips after the detection events
        #[arg(long = "append-observables")]
        append_observables: bool,
        /// Write observable flips to a separate file
        #[arg(long = "obs-out")]
        obs_out: Option<PathBuf>,
        /// Observable output format when --obs-out is set
        #[arg(long = "obs-out-format", default_value = "01")]
        obs_out_format: String,
        /// Select human-readable or versioned JSON output
        #[arg(long, value_enum, default_value_t = pipeline::PipelineFormat::Json)]
        format: pipeline::PipelineFormat,
    },
    /// Convert a circuit into a detector error model
    Dem {
        /// Read the circuit from a file instead of stdin
        #[arg(long = "in")]
        input: Option<PathBuf>,
        /// Destination detector error model file
        #[arg(long)]
        out: PathBuf,
        /// Allow statistical approximation of disjoint errors
        #[arg(long = "approximate-disjoint-errors")]
        approximate_disjoint_errors: bool,
        /// Allow gauge detectors with ambiguous observables
        #[arg(long = "allow-gauge-detectors")]
        allow_gauge_detectors: bool,
        /// Decompose errors into graphlike components
        #[arg(long = "decompose-errors")]
        decompose_errors: bool,
        /// Select human-readable or versioned JSON output
        #[arg(long, value_enum, default_value_t = pipeline::PipelineFormat::Json)]
        format: pipeline::PipelineFormat,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum StatsFormat {
    Human,
    Json,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CapabilitiesFormat {
    Json,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ErrorFormat {
    Human,
    Json,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum DecodeDecoder {
    EnvelopeMatching,
    EnvelopeMle,
}

impl From<DecodeDecoder> for decode::DecoderKind {
    fn from(value: DecodeDecoder) -> Self {
        match value {
            DecodeDecoder::EnvelopeMatching => Self::EnvelopeMatching,
            DecodeDecoder::EnvelopeMle => Self::EnvelopeMle,
        }
    }
}

#[derive(Debug)]
pub enum RunError {
    Clap {
        error: clap::Error,
        command: &'static str,
        json: bool,
    },
    Command(CommandError),
}

#[derive(Debug)]
pub struct CommandError {
    pub command: &'static str,
    pub code: &'static str,
    pub message: String,
    json: bool,
    exit_code: u8,
}

#[derive(Serialize)]
struct SuccessEnvelope<T> {
    schema_version: &'static str,
    status: &'static str,
    command: &'static str,
    result: T,
    warnings: Vec<String>,
    artifacts: Vec<String>,
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    schema_version: &'static str,
    status: &'static str,
    command: &'static str,
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'static str,
    message: &'a str,
}

#[derive(Serialize)]
struct CapabilitiesDocument {
    schema_version: &'static str,
    global_arguments: Vec<ArgumentCapability>,
    commands: Vec<Capability>,
}

#[derive(Serialize)]
struct Capability {
    name: &'static str,
    argv: Vec<&'static str>,
    input_sources: Vec<&'static str>,
    formats: Vec<&'static str>,
    output_schema: &'static str,
    arguments: Vec<ArgumentCapability>,
    success_exit_code: u8,
    errors: Vec<ErrorCapability>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    decoders: Vec<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    artifacts: Vec<ArtifactCapability>,
}

#[derive(Serialize)]
struct ArtifactCapability {
    name: &'static str,
    flag: &'static str,
    format: &'static str,
}

#[derive(Serialize)]
struct ArgumentCapability {
    name: &'static str,
    flag: &'static str,
    required: bool,
    values: Vec<&'static str>,
    default: Option<&'static str>,
}

#[derive(Serialize)]
struct ErrorCapability {
    code: &'static str,
    exit_code: u8,
    channel: &'static str,
}

pub fn run<I, T>(args: I, stdin: &mut dyn Read, stdout: &mut dyn Write) -> Result<(), RunError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<OsString>>();
    let (parse_command, parse_json) = parse_error_context(&args);
    let cli = Cli::try_parse_from(&args).map_err(|error| RunError::Clap {
        error,
        command: parse_command,
        json: parse_json,
    })?;
    let error_format = cli.error_format;
    match cli.command {
        Command::Circuit {
            command: CircuitCommand::Stats { input, format },
        } => {
            run_circuit_stats(input, format, error_format, stdin, stdout).map_err(RunError::Command)
        }
        Command::Circuit {
            command:
                CircuitCommand::Gen {
                    code,
                    task,
                    distance,
                    rounds,
                    noise,
                    after_clifford_loss_probability,
                    operation_loss_probability,
                    measurement_loss_probability,
                    out,
                    format,
                },
        } => {
            let options = pipeline::GenOptions {
                code,
                task,
                distance,
                rounds,
                noise,
                after_clifford_loss_probability,
                operation_loss_probability,
                measurement_loss_probability,
                out,
                format,
            };
            let artifacts = vec![options.out.display().to_string()];
            pipeline::run_gen(&options, error_format)
                .and_then(|result| {
                    write_pipeline_success(
                        pipeline::CIRCUIT_GEN_COMMAND,
                        &result,
                        artifacts,
                        format,
                        error_format,
                        stdout,
                    )
                })
                .map_err(RunError::Command)
        }
        Command::Circuit {
            command:
                CircuitCommand::Sample {
                    shots,
                    input,
                    out,
                    out_format,
                    seed,
                    skip_reference_sample,
                    format,
                },
        } => {
            let options = pipeline::SampleOptions {
                shots,
                input,
                out,
                out_format,
                seed,
                skip_reference_sample,
                format,
            };
            let artifacts = vec![options.out.display().to_string()];
            pipeline::run_sample(&options, error_format, stdin)
                .and_then(|result| {
                    write_pipeline_success(
                        pipeline::CIRCUIT_SAMPLE_COMMAND,
                        &result,
                        artifacts,
                        format,
                        error_format,
                        stdout,
                    )
                })
                .map_err(RunError::Command)
        }
        Command::Circuit {
            command:
                CircuitCommand::Detect {
                    shots,
                    input,
                    out,
                    out_format,
                    seed,
                    append_observables,
                    obs_out,
                    obs_out_format,
                    format,
                },
        } => {
            let mut artifacts = vec![out.display().to_string()];
            if let Some(obs_out) = &obs_out {
                artifacts.push(obs_out.display().to_string());
            }
            let options = pipeline::DetectOptions {
                shots,
                input,
                out,
                out_format,
                seed,
                append_observables,
                obs_out,
                obs_out_format,
                format,
            };
            pipeline::run_detect(&options, error_format, stdin)
                .and_then(|result| {
                    write_pipeline_success(
                        pipeline::CIRCUIT_DETECT_COMMAND,
                        &result,
                        artifacts,
                        format,
                        error_format,
                        stdout,
                    )
                })
                .map_err(RunError::Command)
        }
        Command::Circuit {
            command:
                CircuitCommand::Dem {
                    input,
                    out,
                    approximate_disjoint_errors,
                    allow_gauge_detectors,
                    decompose_errors,
                    format,
                },
        } => {
            let options = pipeline::DemOptions {
                input,
                out,
                approximate_disjoint_errors,
                allow_gauge_detectors,
                decompose_errors,
                format,
            };
            let artifacts = vec![options.out.display().to_string()];
            pipeline::run_dem(&options, error_format, stdin)
                .and_then(|result| {
                    write_pipeline_success(
                        pipeline::CIRCUIT_DEM_COMMAND,
                        &result,
                        artifacts,
                        format,
                        error_format,
                        stdout,
                    )
                })
                .map_err(RunError::Command)
        }
        Command::Dataset {
            command:
                DatasetCommand::Export {
                    circuit,
                    shots,
                    mode,
                    public_out,
                    private_out,
                    seed,
                    logical_x_qubits,
                    logical_z_qubits,
                    format,
                },
        } => {
            let artifacts = vec![
                public_out.display().to_string(),
                private_out.display().to_string(),
            ];
            let options = pipeline::DatasetExportOptions {
                circuit,
                shots,
                mode,
                public_out,
                private_out,
                seed,
                logical_x_qubits,
                logical_z_qubits,
                format,
            };
            pipeline::run_dataset_export(&options, error_format)
                .and_then(|result| {
                    write_pipeline_success(
                        pipeline::DATASET_EXPORT_COMMAND,
                        &result,
                        artifacts,
                        format,
                        error_format,
                        stdout,
                    )
                })
                .map_err(RunError::Command)
        }
        Command::Dataset {
            command:
                DatasetCommand::Import {
                    circuit,
                    shots,
                    shots_format,
                    out,
                    loss_log,
                    format,
                },
        } => {
            let artifacts = vec![out.display().to_string()];
            let options = pipeline::DatasetImportOptions {
                circuit,
                shots,
                shots_format,
                out,
                loss_log,
                format,
            };
            pipeline::run_dataset_import(&options, error_format)
                .and_then(|result| {
                    write_pipeline_success(
                        pipeline::DATASET_IMPORT_COMMAND,
                        &result,
                        artifacts,
                        format,
                        error_format,
                        stdout,
                    )
                })
                .map_err(RunError::Command)
        }
        Command::Capabilities { format } => {
            write_capabilities(format, error_format, stdout).map_err(RunError::Command)
        }
        Command::Decode {
            decoder,
            dataset,
            out,
            stats_out,
            shot_timeout_ms,
        } => decode::run(&decode::DecodeOptions {
            decoder: decoder.into(),
            dataset,
            predictions_out: out,
            stats_out,
            shot_timeout_ms,
        })
        .map(|_| ())
        .map_err(|failure| {
            RunError::Command(CommandError {
                command: decode::COMMAND,
                code: failure.code,
                message: failure.message,
                json: !matches!(error_format, Some(ErrorFormat::Human)),
                exit_code: if matches!(failure.code, "decode_timeout" | "decode_infeasible") {
                    3
                } else {
                    2
                },
            })
        }),
    }
}

fn run_circuit_stats(
    input: Option<PathBuf>,
    format: StatsFormat,
    error_format: Option<ErrorFormat>,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
) -> Result<(), CommandError> {
    let json = match error_format {
        Some(ErrorFormat::Human) => false,
        Some(ErrorFormat::Json) => true,
        None => matches!(format, StatsFormat::Json),
    };
    let text = match input {
        Some(path) => fs::read_to_string(&path).map_err(|error| CommandError {
            command: CIRCUIT_STATS_COMMAND,
            code: "input_error",
            message: format!("failed to read {}: {error}", path.display()),
            json,
            exit_code: 2,
        })?,
        None => {
            let mut text = String::new();
            stdin
                .read_to_string(&mut text)
                .map_err(|error| CommandError {
                    command: CIRCUIT_STATS_COMMAND,
                    code: "input_error",
                    message: format!("failed to read stdin: {error}"),
                    json,
                    exit_code: 2,
                })?;
            text
        }
    };

    let summary = rstim::stats::summarize_text(&text).map_err(|message| CommandError {
        command: CIRCUIT_STATS_COMMAND,
        code: "invalid_circuit",
        message,
        json,
        exit_code: 2,
    })?;

    match format {
        StatsFormat::Human => rstim::stats::write_human(&summary, stdout),
        StatsFormat::Json => write_json(
            stdout,
            &SuccessEnvelope {
                schema_version: SCHEMA_VERSION,
                status: "ok",
                command: CIRCUIT_STATS_COMMAND,
                result: summary,
                warnings: Vec::new(),
                artifacts: Vec::new(),
            },
        ),
    }
    .map_err(|message| CommandError {
        command: CIRCUIT_STATS_COMMAND,
        code: "output_error",
        message,
        json,
        exit_code: 2,
    })
}

fn write_pipeline_success<T: Serialize>(
    command: &'static str,
    result: &T,
    artifacts: Vec<String>,
    format: pipeline::PipelineFormat,
    error_format: Option<ErrorFormat>,
    stdout: &mut dyn Write,
) -> Result<(), CommandError> {
    let json = format.is_json(error_format);
    let outcome = match format {
        pipeline::PipelineFormat::Json => write_json(
            stdout,
            &SuccessEnvelope {
                schema_version: SCHEMA_VERSION,
                status: "ok",
                command,
                result,
                warnings: Vec::new(),
                artifacts,
            },
        ),
        pipeline::PipelineFormat::Human => serde_json::to_value(result)
            .map_err(|error| error.to_string())
            .and_then(|value| {
                let mut text = String::from("status: ok\n");
                if let serde_json::Value::Object(fields) = value {
                    for (key, field) in fields {
                        let rendered = match field {
                            serde_json::Value::String(text) => text,
                            serde_json::Value::Null => continue,
                            other => other.to_string(),
                        };
                        text.push_str(&format!("{key}: {rendered}\n"));
                    }
                }
                for artifact in &artifacts {
                    text.push_str(&format!("artifact: {artifact}\n"));
                }
                stdout
                    .write_all(text.as_bytes())
                    .map_err(|error| error.to_string())
            }),
    };
    outcome.map_err(|message| CommandError {
        command,
        code: "output_error",
        message,
        json,
        exit_code: 2,
    })
}

fn write_capabilities(
    _format: CapabilitiesFormat,
    error_format: Option<ErrorFormat>,
    stdout: &mut dyn Write,
) -> Result<(), CommandError> {
    let json = !matches!(error_format, Some(ErrorFormat::Human));
    write_json(
        stdout,
        &CapabilitiesDocument {
            schema_version: SCHEMA_VERSION,
            global_arguments: vec![ArgumentCapability {
                name: "error_format",
                flag: "--error-format",
                required: false,
                values: vec!["human", "json"],
                default: None,
            }],
            commands: vec![
                Capability {
                    name: CIRCUIT_STATS_COMMAND,
                    argv: vec!["circuit", "stats"],
                    input_sources: vec!["stdin", "file"],
                    formats: vec!["human", "json"],
                    output_schema: SCHEMA_VERSION,
                    arguments: vec![
                        ArgumentCapability {
                            name: "input",
                            flag: "--in",
                            required: false,
                            values: vec!["path"],
                            default: Some("stdin"),
                        },
                        ArgumentCapability {
                            name: "format",
                            flag: "--format",
                            required: false,
                            values: vec!["human", "json"],
                            default: Some("human"),
                        },
                    ],
                    success_exit_code: 0,
                    errors: vec![
                        ErrorCapability {
                            code: "invalid_arguments",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "invalid_circuit",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "input_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "output_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                    ],
                    decoders: Vec::new(),
                    artifacts: Vec::new(),
                },
                Capability {
                    name: pipeline::CIRCUIT_GEN_COMMAND,
                    argv: vec!["circuit", "gen"],
                    input_sources: vec!["built_in_generators"],
                    formats: vec!["human", "json"],
                    output_schema: SCHEMA_VERSION,
                    arguments: vec![
                        ArgumentCapability {
                            name: "code",
                            flag: "--code",
                            required: true,
                            values: vec!["surface_code", "repetition_code", "color_code"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "task",
                            flag: "--task",
                            required: true,
                            values: vec![
                                "rotated_memory_z_midswap",
                                "rotated_memory_x",
                                "rotated_memory_z",
                                "unrotated_memory_x",
                                "unrotated_memory_z",
                                "memory",
                                "memory_xyz",
                            ],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "distance",
                            flag: "--distance",
                            required: true,
                            values: vec!["positive_integer"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "rounds",
                            flag: "--rounds",
                            required: true,
                            values: vec!["non_negative_integer"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "noise",
                            flag: "--noise",
                            required: false,
                            values: vec!["probability"],
                            default: Some("0"),
                        },
                        ArgumentCapability {
                            name: "after_clifford_loss_probability",
                            flag: "--after-clifford-loss-probability",
                            required: false,
                            values: vec!["probability"],
                            default: Some("0"),
                        },
                        ArgumentCapability {
                            name: "operation_loss_probability",
                            flag: "--operation-loss-probability",
                            required: false,
                            values: vec!["probability"],
                            default: Some("0"),
                        },
                        ArgumentCapability {
                            name: "measurement_loss_probability",
                            flag: "--measurement-loss-probability",
                            required: false,
                            values: vec!["probability"],
                            default: Some("0"),
                        },
                        ArgumentCapability {
                            name: "out",
                            flag: "--out",
                            required: true,
                            values: vec!["path"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "format",
                            flag: "--format",
                            required: false,
                            values: vec!["human", "json"],
                            default: Some("json"),
                        },
                    ],
                    success_exit_code: 0,
                    errors: vec![
                        ErrorCapability {
                            code: "invalid_arguments",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "output_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                    ],
                    decoders: Vec::new(),
                    artifacts: vec![ArtifactCapability {
                        name: "circuit",
                        flag: "--out",
                        format: "stim",
                    }],
                },
                Capability {
                    name: pipeline::CIRCUIT_SAMPLE_COMMAND,
                    argv: vec!["circuit", "sample"],
                    input_sources: vec!["stdin", "file"],
                    formats: vec!["human", "json"],
                    output_schema: SCHEMA_VERSION,
                    arguments: vec![
                        ArgumentCapability {
                            name: "shots",
                            flag: "--shots",
                            required: true,
                            values: vec!["non_negative_integer"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "input",
                            flag: "--in",
                            required: false,
                            values: vec!["path"],
                            default: Some("stdin"),
                        },
                        ArgumentCapability {
                            name: "out",
                            flag: "--out",
                            required: true,
                            values: vec!["path"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "out_format",
                            flag: "--out-format",
                            required: false,
                            values: vec!["01", "b8", "r8", "hits", "ptb64"],
                            default: Some("01"),
                        },
                        ArgumentCapability {
                            name: "seed",
                            flag: "--seed",
                            required: false,
                            values: vec!["non_negative_integer"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "skip_reference_sample",
                            flag: "--skip-reference-sample",
                            required: false,
                            values: vec!["flag"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "format",
                            flag: "--format",
                            required: false,
                            values: vec!["human", "json"],
                            default: Some("json"),
                        },
                    ],
                    success_exit_code: 0,
                    errors: vec![
                        ErrorCapability {
                            code: "invalid_arguments",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "invalid_circuit",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "input_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "execution_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "output_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                    ],
                    decoders: Vec::new(),
                    artifacts: vec![ArtifactCapability {
                        name: "measurements",
                        flag: "--out",
                        format: "01|b8|r8|hits|ptb64",
                    }],
                },
                Capability {
                    name: pipeline::CIRCUIT_DETECT_COMMAND,
                    argv: vec!["circuit", "detect"],
                    input_sources: vec!["stdin", "file"],
                    formats: vec!["human", "json"],
                    output_schema: SCHEMA_VERSION,
                    arguments: vec![
                        ArgumentCapability {
                            name: "shots",
                            flag: "--shots",
                            required: true,
                            values: vec!["non_negative_integer"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "input",
                            flag: "--in",
                            required: false,
                            values: vec!["path"],
                            default: Some("stdin"),
                        },
                        ArgumentCapability {
                            name: "out",
                            flag: "--out",
                            required: true,
                            values: vec!["path"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "out_format",
                            flag: "--out-format",
                            required: false,
                            values: vec!["01", "b8", "r8", "hits", "dets", "ptb64"],
                            default: Some("01"),
                        },
                        ArgumentCapability {
                            name: "seed",
                            flag: "--seed",
                            required: false,
                            values: vec!["non_negative_integer"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "append_observables",
                            flag: "--append-observables",
                            required: false,
                            values: vec!["flag"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "obs_out",
                            flag: "--obs-out",
                            required: false,
                            values: vec!["path"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "obs_out_format",
                            flag: "--obs-out-format",
                            required: false,
                            values: vec!["01", "b8", "r8", "hits", "dets", "ptb64"],
                            default: Some("01"),
                        },
                        ArgumentCapability {
                            name: "format",
                            flag: "--format",
                            required: false,
                            values: vec!["human", "json"],
                            default: Some("json"),
                        },
                    ],
                    success_exit_code: 0,
                    errors: vec![
                        ErrorCapability {
                            code: "invalid_arguments",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "invalid_circuit",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "input_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "execution_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "output_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                    ],
                    decoders: Vec::new(),
                    artifacts: vec![
                        ArtifactCapability {
                            name: "detection_events",
                            flag: "--out",
                            format: "01|b8|r8|hits|dets|ptb64",
                        },
                        ArtifactCapability {
                            name: "observable_flips",
                            flag: "--obs-out",
                            format: "01|b8|r8|hits|dets|ptb64",
                        },
                    ],
                },
                Capability {
                    name: pipeline::CIRCUIT_DEM_COMMAND,
                    argv: vec!["circuit", "dem"],
                    input_sources: vec!["stdin", "file"],
                    formats: vec!["human", "json"],
                    output_schema: SCHEMA_VERSION,
                    arguments: vec![
                        ArgumentCapability {
                            name: "input",
                            flag: "--in",
                            required: false,
                            values: vec!["path"],
                            default: Some("stdin"),
                        },
                        ArgumentCapability {
                            name: "out",
                            flag: "--out",
                            required: true,
                            values: vec!["path"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "approximate_disjoint_errors",
                            flag: "--approximate-disjoint-errors",
                            required: false,
                            values: vec!["flag"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "allow_gauge_detectors",
                            flag: "--allow-gauge-detectors",
                            required: false,
                            values: vec!["flag"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "decompose_errors",
                            flag: "--decompose-errors",
                            required: false,
                            values: vec!["flag"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "format",
                            flag: "--format",
                            required: false,
                            values: vec!["human", "json"],
                            default: Some("json"),
                        },
                    ],
                    success_exit_code: 0,
                    errors: vec![
                        ErrorCapability {
                            code: "invalid_arguments",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "invalid_circuit",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "input_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "execution_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "output_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                    ],
                    decoders: Vec::new(),
                    artifacts: vec![ArtifactCapability {
                        name: "detector_error_model",
                        flag: "--out",
                        format: "dem",
                    }],
                },
                Capability {
                    name: pipeline::DATASET_EXPORT_COMMAND,
                    argv: vec!["dataset", "export"],
                    input_sources: vec!["file"],
                    formats: vec!["human", "json"],
                    output_schema: SCHEMA_VERSION,
                    arguments: vec![
                        ArgumentCapability {
                            name: "circuit",
                            flag: "--circuit",
                            required: true,
                            values: vec!["path"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "shots",
                            flag: "--shots",
                            required: true,
                            values: vec!["non_negative_integer"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "mode",
                            flag: "--mode",
                            required: true,
                            values: vec!["detectors", "measurements_blinded"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "public_out",
                            flag: "--public-out",
                            required: true,
                            values: vec!["path"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "private_out",
                            flag: "--private-out",
                            required: true,
                            values: vec!["path"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "seed",
                            flag: "--seed",
                            required: false,
                            values: vec!["non_negative_integer"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "logical_x_qubits",
                            flag: "--logical-x-qubits",
                            required: false,
                            values: vec!["qubit_index_list"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "logical_z_qubits",
                            flag: "--logical-z-qubits",
                            required: false,
                            values: vec!["qubit_index_list"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "format",
                            flag: "--format",
                            required: false,
                            values: vec!["human", "json"],
                            default: Some("json"),
                        },
                    ],
                    success_exit_code: 0,
                    errors: vec![
                        ErrorCapability {
                            code: "invalid_arguments",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "invalid_circuit",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "input_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "execution_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                    ],
                    decoders: Vec::new(),
                    artifacts: vec![
                        ArtifactCapability {
                            name: "public_dataset",
                            flag: "--public-out",
                            format: "directory",
                        },
                        ArtifactCapability {
                            name: "private_answers",
                            flag: "--private-out",
                            format: "directory",
                        },
                    ],
                },
                Capability {
                    name: pipeline::DATASET_IMPORT_COMMAND,
                    argv: vec!["dataset", "import"],
                    input_sources: vec!["file"],
                    formats: vec!["human", "json"],
                    output_schema: SCHEMA_VERSION,
                    arguments: vec![
                        ArgumentCapability {
                            name: "circuit",
                            flag: "--circuit",
                            required: true,
                            values: vec!["path"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "shots",
                            flag: "--shots",
                            required: true,
                            values: vec!["path"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "shots_format",
                            flag: "--shots-format",
                            required: false,
                            values: vec!["01", "b8"],
                            default: Some("01"),
                        },
                        ArgumentCapability {
                            name: "out",
                            flag: "--out",
                            required: true,
                            values: vec!["path"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "loss_log",
                            flag: "--loss-log",
                            required: false,
                            values: vec!["path"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "format",
                            flag: "--format",
                            required: false,
                            values: vec!["human", "json"],
                            default: Some("json"),
                        },
                    ],
                    success_exit_code: 0,
                    errors: vec![
                        ErrorCapability {
                            code: "invalid_arguments",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "invalid_circuit",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "input_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "invalid_dataset",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "unsupported_circuit",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "loss_log_mismatch",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "output_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                    ],
                    decoders: Vec::new(),
                    artifacts: vec![ArtifactCapability {
                        name: "public_dataset",
                        flag: "--out",
                        format: "directory",
                    }],
                },
                Capability {
                    name: decode::COMMAND,
                    argv: vec!["decode"],
                    input_sources: vec!["public_decoder_dataset_directory"],
                    formats: vec!["b8", "json"],
                    output_schema: decode::STATS_SCHEMA_VERSION,
                    arguments: vec![
                        ArgumentCapability {
                            name: "decoder",
                            flag: "--decoder",
                            required: true,
                            values: vec!["envelope-matching", "envelope-mle"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "dataset",
                            flag: "--dataset",
                            required: true,
                            values: vec!["path"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "predictions",
                            flag: "--out",
                            required: true,
                            values: vec!["path"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "stats",
                            flag: "--stats-out",
                            required: true,
                            values: vec!["path"],
                            default: None,
                        },
                        ArgumentCapability {
                            name: "shot_timeout_ms",
                            flag: "--shot-timeout-ms",
                            required: false,
                            values: vec!["non_negative_integer"],
                            default: None,
                        },
                    ],
                    success_exit_code: 0,
                    errors: vec![
                        ErrorCapability {
                            code: "invalid_arguments",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "missing_dataset_file",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "invalid_dataset",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "unsupported_dataset_mode",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "unsupported_circuit",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "decode_timeout",
                            exit_code: 3,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "decode_infeasible",
                            exit_code: 3,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "decode_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                        ErrorCapability {
                            code: "output_error",
                            exit_code: 2,
                            channel: "stderr",
                        },
                    ],
                    decoders: vec!["envelope-matching", "envelope-mle"],
                    artifacts: vec![
                        ArtifactCapability {
                            name: "predictions",
                            flag: "--out",
                            format: "b8",
                        },
                        ArtifactCapability {
                            name: "stats",
                            flag: "--stats-out",
                            format: "rustqec.decode-stats.v1+json",
                        },
                    ],
                },
            ],
        },
    )
    .map_err(|message| CommandError {
        command: "capabilities",
        code: "output_error",
        message,
        json,
        exit_code: 2,
    })
}

fn write_json(out: &mut dyn Write, value: &impl Serialize) -> Result<(), String> {
    serde_json::to_writer_pretty(&mut *out, value).map_err(|error| error.to_string())?;
    out.write_all(b"\n").map_err(|error| error.to_string())
}

pub fn write_error(error: &RunError, stdout: &mut dyn Write, stderr: &mut dyn Write) {
    match error {
        RunError::Clap { error, .. }
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            let _ = write!(stdout, "{error}");
        }
        RunError::Clap {
            error,
            command,
            json: true,
        } => {
            let message = error.to_string();
            let envelope = ErrorEnvelope {
                schema_version: SCHEMA_VERSION,
                status: "error",
                command,
                error: ErrorBody {
                    code: "invalid_arguments",
                    message: &message,
                },
            };
            let _ = write_json(stderr, &envelope);
        }
        RunError::Clap { error, .. } => {
            let _ = write!(stderr, "{error}");
        }
        RunError::Command(error) if error.json => {
            let envelope = ErrorEnvelope {
                schema_version: SCHEMA_VERSION,
                status: "error",
                command: error.command,
                error: ErrorBody {
                    code: error.code,
                    message: &error.message,
                },
            };
            let _ = write_json(stderr, &envelope);
        }
        RunError::Command(error) => {
            let _ = writeln!(stderr, "{}", error.message);
        }
    }
}

pub fn exit_code(error: &RunError) -> u8 {
    match error {
        RunError::Clap { error, .. } => error.exit_code() as u8,
        RunError::Command(error) => error.exit_code,
    }
}

fn parse_error_context(args: &[OsString]) -> (&'static str, bool) {
    let words = args
        .iter()
        .skip(1)
        .map(|value| value.to_string_lossy())
        .collect::<Vec<_>>();
    let command_words = words_without_global_options(&words);
    let is_stats = command_words
        .windows(2)
        .any(|pair| pair[0] == "circuit" && pair[1] == "stats");
    let pipeline_command = ["gen", "sample", "detect", "dem"].iter().find_map(|verb| {
        command_words
            .windows(2)
            .any(|pair| pair[0] == "circuit" && pair[1] == *verb)
            .then_some(match *verb {
                "gen" => pipeline::CIRCUIT_GEN_COMMAND,
                "sample" => pipeline::CIRCUIT_SAMPLE_COMMAND,
                "detect" => pipeline::CIRCUIT_DETECT_COMMAND,
                _ => pipeline::CIRCUIT_DEM_COMMAND,
            })
    });
    let is_dataset_export = command_words
        .windows(2)
        .any(|pair| pair[0] == "dataset" && pair[1] == "export");
    let is_capabilities = command_words.contains(&"capabilities");
    let is_decode = command_words.contains(&"decode");
    let command = if is_stats {
        CIRCUIT_STATS_COMMAND
    } else if let Some(pipeline_command) = pipeline_command {
        pipeline_command
    } else if is_dataset_export {
        pipeline::DATASET_EXPORT_COMMAND
    } else if is_capabilities {
        "capabilities"
    } else if is_decode {
        decode::COMMAND
    } else {
        "rustqec"
    };

    let explicit_error_format = option_value(&words, "--error-format");
    let explicit_output_format = option_value(&words, "--format");
    let json = match explicit_error_format {
        Some(Some("human")) => false,
        Some(_) => true,
        None if is_capabilities || is_decode => true,
        None if pipeline_command.is_some() || is_dataset_export => {
            !matches!(explicit_output_format, Some(Some("human")))
        }
        None if is_stats => match explicit_output_format {
            Some(Some("human")) => false,
            Some(_) => true,
            None => false,
        },
        None => false,
    };
    (command, json)
}

fn words_without_global_options<'a>(words: &'a [std::borrow::Cow<'a, str>]) -> Vec<&'a str> {
    let mut command_words = Vec::new();
    let mut index = 0;
    while index < words.len() {
        let word = words[index].as_ref();
        if word == "--error-format" {
            index += 2;
        } else if word.starts_with("--error-format=") {
            index += 1;
        } else {
            command_words.push(word);
            index += 1;
        }
    }
    command_words
}

fn option_value<'a>(words: &'a [std::borrow::Cow<'a, str>], flag: &str) -> Option<Option<&'a str>> {
    for (index, word) in words.iter().enumerate() {
        if word == flag {
            return Some(words.get(index + 1).map(|value| value.as_ref()));
        }
        if let Some(value) = word
            .strip_prefix(flag)
            .and_then(|rest| rest.strip_prefix('='))
        {
            return Some(Some(value));
        }
    }
    None
}
