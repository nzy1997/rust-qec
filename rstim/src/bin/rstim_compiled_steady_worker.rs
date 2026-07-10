use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use clap::Parser;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use rstim::data_path::ReferenceSampleMode;
use rstim::output::write_shots_b8;
use rstim::parser::parse_lines;
use rstim::sampler::SampleOutputMode;
use rstim::CompiledMeasurementSampler;

const READY: u8 = b'R';
const SAMPLE: u8 = b'S';
const RESULT: u8 = b'T';
const STOP: u8 = b'P';
const FINAL: u8 = b'F';
const ERROR: u8 = b'E';

#[derive(Parser)]
#[command(name = "rstim", version)]
struct Args {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    seed: u64,
}

#[derive(Deserialize)]
struct SampleRequest {
    request_id: u64,
    shots: usize,
}

#[derive(Serialize)]
struct Telemetry {
    variant: &'static str,
    compile_count: usize,
    reference_build_count: usize,
    sample_call_count: usize,
    fixture_sha256: String,
    measurement_count: usize,
    bytes_per_shot: usize,
}

fn write_frame(frame_type: u8, payload: &[u8]) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(&[frame_type])?;
    stdout.write_all(&(payload.len() as u64).to_le_bytes())?;
    stdout.write_all(payload)?;
    stdout.flush()
}

fn read_frame() -> io::Result<(u8, Vec<u8>)> {
    let mut stdin = io::stdin().lock();
    let mut header = [0_u8; 9];
    stdin.read_exact(&mut header)?;
    let payload_len = u64::from_le_bytes(header[1..].try_into().expect("fixed header length"));
    let payload_len = usize::try_from(payload_len)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "frame payload is too large"))?;
    let mut payload = vec![0_u8; payload_len];
    stdin.read_exact(&mut payload)?;
    Ok((header[0], payload))
}

fn telemetry(
    sampler: &CompiledMeasurementSampler,
    fixture_sha256: &str,
    measurement_count: usize,
) -> Telemetry {
    let diagnostics = sampler.diagnostics();
    Telemetry {
        variant: "rstim",
        compile_count: diagnostics.compiled_ir_builds,
        reference_build_count: diagnostics.reference_builds,
        sample_call_count: diagnostics.sample_calls,
        fixture_sha256: fixture_sha256.to_owned(),
        measurement_count,
        bytes_per_shot: measurement_count.div_ceil(8),
    }
}

fn write_json_frame(frame_type: u8, value: &Telemetry) -> Result<(), String> {
    let payload = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    write_frame(frame_type, &payload).map_err(|error| error.to_string())
}

fn write_error(message: impl std::fmt::Display) -> Result<(), String> {
    write_frame(ERROR, message.to_string().as_bytes()).map_err(|error| error.to_string())
}

fn run(args: Args) -> Result<(), String> {
    let input_bytes =
        fs::read(&args.input).map_err(|error| format!("failed to read {}: {error}", args.input.display()))?;
    let input_text = std::str::from_utf8(&input_bytes).map_err(|error| error.to_string())?;
    let instructions = parse_lines(input_text)?;
    let mut sampler =
        CompiledMeasurementSampler::compile(&instructions, ReferenceSampleMode::SimulateNoiseless)?;
    let measurement_count = rstim::stats::num_measurements(&instructions);
    let fixture_sha256 = format!("{:x}", Sha256::digest(&input_bytes));
    let mut rng = StdRng::seed_from_u64(args.seed);

    write_json_frame(
        READY,
        &telemetry(&sampler, &fixture_sha256, measurement_count),
    )?;

    loop {
        let (frame_type, payload) = read_frame().map_err(|error| error.to_string())?;
        match frame_type {
            STOP => {
                write_json_frame(
                    FINAL,
                    &telemetry(&sampler, &fixture_sha256, measurement_count),
                )?;
                return Ok(());
            }
            SAMPLE => {
                let request: SampleRequest = match serde_json::from_slice(&payload) {
                    Ok(request) => request,
                    Err(error) => {
                        write_error(format!("invalid SAMPLE JSON: {error}"))?;
                        continue;
                    }
                };
                let output = match sampler.sample(
                    request.shots,
                    &mut rng,
                    SampleOutputMode::MeasurementsOnly,
                ) {
                    Ok(output) => output,
                    Err(error) => {
                        write_error(error)?;
                        continue;
                    }
                };
                let mut result = Vec::new();
                result.extend_from_slice(&request.request_id.to_le_bytes());
                result
                    .extend_from_slice(&(sampler.diagnostics().sample_calls as u64).to_le_bytes());
                write_shots_b8(&output.measurements, &mut result)
                    .map_err(|error| error.to_string())?;
                write_frame(RESULT, &result).map_err(|error| error.to_string())?;
            }
            _ => write_error(format!("unexpected frame: {frame_type:?}"))?,
        }
    }
}

fn main() {
    if let Err(error) = run(Args::parse()) {
        let _ = write_error(&error);
        eprintln!("{error}");
        std::process::exit(1);
    }
}
