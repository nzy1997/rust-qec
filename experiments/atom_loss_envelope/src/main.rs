use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use atom_loss_envelope::{AtomLossCase, DecodeOutcome, decode};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(about = "Experimental decoder for explicit atom-loss Pauli envelopes")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Decode one versioned JSON envelope case.
    Decode {
        /// Input atom-loss-envelope.v0 JSON file.
        #[arg(long = "in", value_name = "CASE_JSON")]
        input: PathBuf,
        /// Destination result JSON file.
        #[arg(long, value_name = "RESULT_JSON")]
        out: PathBuf,
        /// Integer-programming backend.
        #[arg(long, value_enum)]
        backend: BackendArg,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum BackendArg {
    Highs,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Decode {
            input,
            out,
            backend: BackendArg::Highs,
        } => run_decode(&input, &out),
    }
}

fn run_decode(input: &Path, output: &Path) -> ExitCode {
    let result = (|| -> Result<(DecodeOutcome, bool), String> {
        let handle = File::open(input)
            .map_err(|error| format!("failed to open input {}: {error}", input.display()))?;
        let case: AtomLossCase = serde_json::from_reader(BufReader::new(handle))
            .map_err(|error| format!("failed to parse input {}: {error}", input.display()))?;
        let outcome = decode(&case).map_err(|error| format!("decode failed: {error}"))?;
        let infeasible = matches!(outcome, DecodeOutcome::Infeasible(_));
        write_outcome(output, &outcome)?;
        Ok((outcome, infeasible))
    })();

    match result {
        Ok((_, false)) => ExitCode::SUCCESS,
        Ok((_, true)) => ExitCode::from(3),
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn write_outcome(output: &Path, outcome: &DecodeOutcome) -> Result<(), String> {
    let handle = File::create(output)
        .map_err(|error| format!("failed to create output {}: {error}", output.display()))?;
    let writer = BufWriter::new(handle);
    match outcome {
        DecodeOutcome::Optimal(result) => write_json(writer, result),
        DecodeOutcome::Infeasible(result) => write_json(writer, result),
    }
    .map_err(|error| format!("failed to write output {}: {error}", output.display()))
}

fn write_json(writer: BufWriter<File>, value: &impl Serialize) -> serde_json::Result<()> {
    serde_json::to_writer_pretty(writer, value)
}
