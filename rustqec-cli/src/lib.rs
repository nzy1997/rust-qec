use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum, error::ErrorKind};
use serde::Serialize;

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
        Command::Capabilities { format } => {
            write_capabilities(format, stdout).map_err(RunError::Command)
        }
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
                })?;
            text
        }
    };

    let summary = rstim::stats::summarize_text(&text).map_err(|message| CommandError {
        command: CIRCUIT_STATS_COMMAND,
        code: "invalid_circuit",
        message,
        json,
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
    })
}

fn write_capabilities(
    _format: CapabilitiesFormat,
    stdout: &mut dyn Write,
) -> Result<(), CommandError> {
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
            commands: vec![Capability {
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
            }],
        },
    )
    .map_err(|message| CommandError {
        command: "capabilities",
        code: "output_error",
        message,
        json: true,
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
        RunError::Command(_) => 2,
    }
}

fn parse_error_context(args: &[OsString]) -> (&'static str, bool) {
    let words = args
        .iter()
        .skip(1)
        .map(|value| value.to_string_lossy())
        .collect::<Vec<_>>();
    let is_stats = words
        .windows(2)
        .any(|pair| pair[0] == "circuit" && pair[1] == "stats");
    let is_capabilities = words.iter().any(|word| word == "capabilities");
    let command = if is_stats {
        CIRCUIT_STATS_COMMAND
    } else if is_capabilities {
        "capabilities"
    } else {
        "rustqec"
    };

    let explicit_error_format = option_value(&words, "--error-format");
    let explicit_output_format = option_value(&words, "--format");
    let json = match explicit_error_format {
        Some(Some("human")) => false,
        Some(_) => true,
        None if is_capabilities => true,
        None if is_stats => match explicit_output_format {
            Some(Some("human")) => false,
            Some(_) => true,
            None => false,
        },
        None => false,
    };
    (command, json)
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
