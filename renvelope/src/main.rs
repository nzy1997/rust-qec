use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use renvelope::{
    AtomLossCase, DecodeOutcome, EnvelopeMatchingCase, PrepareConfig, decode, decode_matching,
    prepare,
};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(about = "Reference decoders for explicit atom-loss Pauli envelopes")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Prepare loss-visible measurements for the reference decoders.
    Prepare {
        /// RStim Mid-SWAP circuit containing loss-visible measurements.
        #[arg(long, value_name = "CIRCUIT")]
        circuit: PathBuf,
        /// Pure-loss calibration measurements in b8 format.
        #[arg(long = "calibration_in", value_name = "CALIBRATION_B8")]
        calibration_in: PathBuf,
        /// Number of calibration shots in calibration_in.
        #[arg(long = "calibration_shots", value_name = "N")]
        calibration_shots: usize,
        /// Target measurement shots in b8 format.
        #[arg(long = "in", value_name = "MEASUREMENTS_B8")]
        input: PathBuf,
        /// Number of target shots in the input file.
        #[arg(long, value_name = "M")]
        shots: usize,
        /// Destination directory for the prepared decoder bundle.
        #[arg(long, value_name = "PREPARED_DIRECTORY")]
        out: PathBuf,
    },
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
    /// Decode shots by rescaling MWPM edges compatible with reported losses.
    Matching {
        /// Input atom-loss-envelope-matching.v0 JSON file.
        #[arg(long = "in", value_name = "CASE_JSON")]
        input: PathBuf,
        /// Destination result JSON file.
        #[arg(long, value_name = "RESULT_JSON")]
        out: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum BackendArg {
    Highs,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Prepare {
            circuit,
            calibration_in,
            calibration_shots,
            input,
            shots,
            out,
        } => run_prepare(
            &circuit,
            &calibration_in,
            calibration_shots,
            &input,
            shots,
            &out,
        ),
        Command::Decode {
            input,
            out,
            backend: BackendArg::Highs,
        } => run_decode(&input, &out),
        Command::Matching { input, out } => run_matching(&input, &out),
    }
}

fn run_prepare(
    circuit: &Path,
    calibration_in: &Path,
    calibration_shots: usize,
    input: &Path,
    shots: usize,
    out: &Path,
) -> ExitCode {
    match prepare(&PrepareConfig {
        circuit: circuit.to_path_buf(),
        calibration_in: calibration_in.to_path_buf(),
        calibration_shots,
        input: input.to_path_buf(),
        shots,
        out: out.to_path_buf(),
    }) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_matching(input: &Path, output: &Path) -> ExitCode {
    let result = (|| -> Result<(), String> {
        let handle = File::open(input)
            .map_err(|error| format!("failed to open input {}: {error}", input.display()))?;
        let case: EnvelopeMatchingCase = serde_json::from_reader(BufReader::new(handle))
            .map_err(|error| format!("failed to parse input {}: {error}", input.display()))?;
        let outcome = decode_matching(&case).map_err(|error| format!("decode failed: {error}"))?;
        write_result(output, &outcome)
    })();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
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

fn write_result(output: &Path, value: &impl Serialize) -> Result<(), String> {
    let handle = File::create(output)
        .map_err(|error| format!("failed to create output {}: {error}", output.display()))?;
    write_json(BufWriter::new(handle), value)
        .map_err(|error| format!("failed to write output {}: {error}", output.display()))
}
