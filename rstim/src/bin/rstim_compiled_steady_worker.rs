use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, ValueEnum};
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use rstim::data_path::ReferenceSampleMode;
use rstim::output::append_shots_b8;
use rstim::parser::parse_lines;
use rstim::sampler::{SampleOptions, SampleOutputMode, SamplingBackend, sample_batch_with_options};
use rstim::{CompiledLossMeasurementSampler, CompiledMeasurementSampler};

const READY: u8 = b'R';
const SAMPLE: u8 = b'S';
const RESULT: u8 = b'T';
const STOP: u8 = b'P';
const FINAL: u8 = b'F';
const ERROR: u8 = b'E';

#[derive(Parser)]
#[command(name = "rstim", version)]
struct Args {
    #[arg(long, value_enum)]
    variant: WorkerVariant,
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    seed: u64,
}

#[derive(Clone, Copy, ValueEnum)]
enum WorkerVariant {
    RstimPrecompiled,
    RstimInterpreted,
    RstimPrecompiledAtomLoss,
}

impl WorkerVariant {
    fn label(self) -> &'static str {
        match self {
            Self::RstimPrecompiled => "rstim-precompiled",
            Self::RstimInterpreted => "rstim-interpreted",
            Self::RstimPrecompiledAtomLoss => "rstim-precompiled-atom-loss",
        }
    }
}

#[derive(Deserialize)]
struct SampleRequest {
    request_id: u64,
    shots: usize,
}

#[derive(Serialize)]
struct Telemetry {
    variant: &'static str,
    precompile_elapsed_ns: u64,
    compile_count: usize,
    reference_build_count: usize,
    sample_call_count: usize,
    fixture_sha256: String,
    measurement_count: usize,
    bytes_per_shot: usize,
}

enum SamplerState {
    Precompiled(CompiledMeasurementSampler),
    PrecompiledLoss(CompiledLossMeasurementSampler),
    Interpreted { sample_calls: usize },
}

impl SamplerState {
    fn compile_count(&self) -> usize {
        match self {
            Self::Precompiled(sampler) => sampler.diagnostics().compiled_ir_builds,
            Self::PrecompiledLoss(sampler) => sampler.diagnostics().compiled_ir_builds,
            Self::Interpreted { .. } => 0,
        }
    }

    fn reference_build_count(&self) -> usize {
        match self {
            Self::Precompiled(sampler) => sampler.diagnostics().reference_builds,
            Self::PrecompiledLoss(sampler) => sampler.diagnostics().reference_builds,
            Self::Interpreted { sample_calls } => *sample_calls,
        }
    }

    fn sample_call_count(&self) -> usize {
        match self {
            Self::Precompiled(sampler) => sampler.diagnostics().sample_calls,
            Self::PrecompiledLoss(sampler) => sampler.diagnostics().sample_calls,
            Self::Interpreted { sample_calls } => *sample_calls,
        }
    }
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
    variant: WorkerVariant,
    sampler: &SamplerState,
    precompile_elapsed_ns: u64,
    fixture_sha256: &str,
    measurement_count: usize,
) -> Telemetry {
    Telemetry {
        variant: variant.label(),
        precompile_elapsed_ns,
        compile_count: sampler.compile_count(),
        reference_build_count: sampler.reference_build_count(),
        sample_call_count: sampler.sample_call_count(),
        fixture_sha256: fixture_sha256.to_owned(),
        measurement_count,
        bytes_per_shot: measurement_count.div_ceil(8),
    }
}

fn elapsed_ns(started: Instant, label: &str) -> Result<u64, String> {
    u64::try_from(started.elapsed().as_nanos())
        .map_err(|_| format!("{label} duration does not fit in u64"))
}

fn write_json_frame(frame_type: u8, value: &Telemetry) -> Result<(), String> {
    let payload = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    write_frame(frame_type, &payload).map_err(|error| error.to_string())
}

fn write_error(message: impl std::fmt::Display) -> Result<(), String> {
    write_frame(ERROR, message.to_string().as_bytes()).map_err(|error| error.to_string())
}

fn run(args: Args) -> Result<(), String> {
    let input_bytes = fs::read(&args.input)
        .map_err(|error| format!("failed to read {}: {error}", args.input.display()))?;
    let input_text = std::str::from_utf8(&input_bytes).map_err(|error| error.to_string())?;
    let instructions = parse_lines(input_text)?;
    let (mut sampler, precompile_elapsed_ns) = match args.variant {
        WorkerVariant::RstimPrecompiled => {
            let started = Instant::now();
            let sampler = SamplerState::Precompiled(CompiledMeasurementSampler::compile(
                &instructions,
                ReferenceSampleMode::SimulateNoiseless,
            )?);
            (sampler, elapsed_ns(started, "precompile")?)
        }
        WorkerVariant::RstimPrecompiledAtomLoss => {
            let started = Instant::now();
            let sampler = SamplerState::PrecompiledLoss(CompiledLossMeasurementSampler::compile(
                &instructions,
                ReferenceSampleMode::SimulateNoiseless,
            )?);
            (sampler, elapsed_ns(started, "loss precompile")?)
        }
        WorkerVariant::RstimInterpreted => (SamplerState::Interpreted { sample_calls: 0 }, 0),
    };
    let measurement_count = rstim::stats::num_measurements(&instructions);
    let fixture_sha256 = format!("{:x}", Sha256::digest(&input_bytes));
    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut packed = Vec::new();

    write_json_frame(
        READY,
        &telemetry(
            args.variant,
            &sampler,
            precompile_elapsed_ns,
            &fixture_sha256,
            measurement_count,
        ),
    )?;

    loop {
        let (frame_type, payload) = read_frame().map_err(|error| error.to_string())?;
        match frame_type {
            STOP => {
                write_json_frame(
                    FINAL,
                    &telemetry(
                        args.variant,
                        &sampler,
                        precompile_elapsed_ns,
                        &fixture_sha256,
                        measurement_count,
                    ),
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
                let sample_started = Instant::now();
                let sample_result = match &mut sampler {
                    SamplerState::Precompiled(sampler) => {
                        sampler.sample(request.shots, &mut rng, SampleOutputMode::MeasurementsOnly)
                    }
                    SamplerState::PrecompiledLoss(sampler) => {
                        sampler.sample(request.shots, &mut rng, SampleOutputMode::MeasurementsOnly)
                    }
                    SamplerState::Interpreted { sample_calls } => {
                        let result = sample_batch_with_options(
                            &instructions,
                            request.shots,
                            &mut rng,
                            SampleOptions {
                                backend: SamplingBackend::Interpreted,
                                output_mode: SampleOutputMode::MeasurementsOnly,
                                ..SampleOptions::default()
                            },
                        );
                        if result.is_ok() {
                            *sample_calls += 1;
                        }
                        result
                    }
                };
                let output = match sample_result {
                    Ok(output) => output,
                    Err(error) => {
                        write_error(error)?;
                        continue;
                    }
                };
                let sample_elapsed_ns = elapsed_ns(sample_started, "sample path")?;
                let b8_started = Instant::now();
                packed.clear();
                append_shots_b8(&output.measurements, &mut packed)
                    .map_err(|error| error.to_string())?;
                let b8_elapsed_ns = elapsed_ns(b8_started, "b8 output")?;

                let mut result = Vec::with_capacity(32 + packed.len());
                result.extend_from_slice(&request.request_id.to_le_bytes());
                result.extend_from_slice(&(sampler.sample_call_count() as u64).to_le_bytes());
                result.extend_from_slice(&sample_elapsed_ns.to_le_bytes());
                result.extend_from_slice(&b8_elapsed_ns.to_le_bytes());
                result.extend_from_slice(&packed);
                write_frame(RESULT, &result).map_err(|error| error.to_string())?;
            }
            _ => write_error(format!("unexpected frame: {frame_type:?}"))?,
        }
    }
}

fn main() {
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            if error.use_stderr() {
                let message = error.to_string();
                let _ = write_error(&message);
                eprint!("{message}");
                std::process::exit(error.exit_code());
            }
            error.exit();
        }
    };
    if let Err(error) = run(args) {
        let _ = write_error(&error);
        eprintln!("{error}");
        std::process::exit(1);
    }
}
