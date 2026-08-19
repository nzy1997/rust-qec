use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
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

#[derive(Debug)]
pub enum RunError {
    Clap(clap::Error),
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
    commands: Vec<Capability>,
}

#[derive(Serialize)]
struct Capability {
    name: &'static str,
    input_sources: Vec<&'static str>,
    formats: Vec<&'static str>,
    output_schema: &'static str,
}

pub fn run<I, T>(args: I, stdin: &mut dyn Read, stdout: &mut dyn Write) -> Result<(), RunError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::try_parse_from(args).map_err(RunError::Clap)?;
    match cli.command {
        Command::Circuit {
            command: CircuitCommand::Stats { input, format },
        } => run_circuit_stats(input, format, stdin, stdout).map_err(RunError::Command),
        Command::Capabilities { format } => {
            write_capabilities(format, stdout).map_err(RunError::Command)
        }
    }
}

fn run_circuit_stats(
    input: Option<PathBuf>,
    format: StatsFormat,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
) -> Result<(), CommandError> {
    let json = matches!(format, StatsFormat::Json);
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
            commands: vec![Capability {
                name: CIRCUIT_STATS_COMMAND,
                input_sources: vec!["stdin", "file"],
                formats: vec!["human", "json"],
                output_schema: SCHEMA_VERSION,
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

pub fn write_error(error: &RunError, stderr: &mut dyn Write) {
    match error {
        RunError::Clap(error) => {
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
        RunError::Clap(error) => error.exit_code() as u8,
        RunError::Command(_) => 2,
    }
}
