use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use rstim::data_path::{
    build_reference_sample_with_decision, ReferenceBuildPhaseCounters, ReferenceSampleDecision,
};
use rstim::ir::StimInstr;
use rstim::parser::parse_lines;

const PROTOCOL: &str = "reference-build-v1";
const TIMER_SCOPE: &str = "reference_build_only";
const DIRECT_BACKEND: &str = "direct_inverse_repeat_folded";
const CANONICAL_BACKEND: &str = "canonical_roundtrip";

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum Strategy {
    Direct,
    Canonical,
}

#[derive(Parser)]
#[command(name = "rstim_reference_build_worker", version)]
struct Args {
    #[arg(long)]
    protocol: String,
    #[arg(long, value_enum, default_value_t = Strategy::Direct)]
    strategy: Strategy,
}

#[derive(Deserialize)]
struct RequestHeader {
    protocol: String,
    #[serde(rename = "type")]
    request_type: String,
}

#[derive(Deserialize)]
struct LoadRequest {
    protocol: String,
    #[serde(rename = "type")]
    request_type: String,
    fixture_path: PathBuf,
}

#[derive(Deserialize)]
struct BuildReferenceRequest {
    protocol: String,
    #[serde(rename = "type")]
    request_type: String,
    request_id: u64,
    #[serde(default)]
    include_phase_counters: bool,
}

#[derive(Serialize)]
struct LoadedResponse {
    protocol: &'static str,
    #[serde(rename = "type")]
    response_type: &'static str,
    parse_count: usize,
    measurement_bits: usize,
}

#[derive(Serialize)]
struct ReferenceBuiltResponse {
    protocol: &'static str,
    #[serde(rename = "type")]
    response_type: &'static str,
    request_id: u64,
    backend: &'static str,
    parse_count: usize,
    reference_build_count: usize,
    measurement_bits: usize,
    packed_bytes: usize,
    packed_base64: String,
    byte_sha256: String,
    timer_scope: &'static str,
    elapsed_ns: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase_counters: Option<ReferenceBuildPhaseCounters>,
}

#[derive(Serialize)]
struct ErrorResponse {
    protocol: &'static str,
    #[serde(rename = "type")]
    response_type: &'static str,
    message: String,
}

struct WorkerState {
    strategy: Strategy,
    instructions: Option<Vec<StimInstr>>,
    parse_count: usize,
    reference_build_count: usize,
    measurement_bits: usize,
}

fn write_json_line<T: Serialize>(value: &T) -> Result<(), String> {
    let mut stdout = io::stdout().lock();
    serde_json::to_writer(&mut stdout, value).map_err(|error| error.to_string())?;
    stdout.write_all(b"\n").map_err(|error| error.to_string())?;
    stdout.flush().map_err(|error| error.to_string())
}

fn write_error(message: impl std::fmt::Display) -> Result<(), String> {
    write_json_line(&ErrorResponse {
        protocol: PROTOCOL,
        response_type: "error",
        message: message.to_string(),
    })
}

fn validate_protocol(protocol: &str) -> Result<(), String> {
    if protocol != PROTOCOL {
        return Err(format!("request protocol must be {PROTOCOL:?}"));
    }
    Ok(())
}

fn validate_protocol_and_type(
    protocol: &str,
    request_type: &str,
    expected_type: &str,
) -> Result<(), String> {
    validate_protocol(protocol)?;
    if request_type != expected_type {
        return Err(format!("request type must be {expected_type:?}"));
    }
    Ok(())
}

fn handle_load(line: &str, state: &mut WorkerState) -> Result<LoadedResponse, String> {
    let request: LoadRequest = serde_json::from_str(line).map_err(|error| error.to_string())?;
    validate_protocol_and_type(&request.protocol, &request.request_type, "load")?;

    let input_bytes = fs::read(&request.fixture_path)
        .map_err(|error| format!("failed to read {}: {error}", request.fixture_path.display()))?;
    let input_text = std::str::from_utf8(&input_bytes).map_err(|error| error.to_string())?;
    let instructions = parse_lines(input_text)?;
    state.parse_count += 1;
    state.reference_build_count = 0;
    state.measurement_bits = rstim::stats::num_measurements(&instructions);
    state.instructions = Some(instructions);

    Ok(LoadedResponse {
        protocol: PROTOCOL,
        response_type: "loaded",
        parse_count: state.parse_count,
        measurement_bits: state.measurement_bits,
    })
}

fn handle_build_reference(
    line: &str,
    state: &mut WorkerState,
) -> Result<ReferenceBuiltResponse, String> {
    let request: BuildReferenceRequest =
        serde_json::from_str(line).map_err(|error| error.to_string())?;
    validate_protocol_and_type(&request.protocol, &request.request_type, "build_reference")?;
    let instructions = state
        .instructions
        .as_ref()
        .ok_or_else(|| "cannot build reference before load".to_string())?;

    let started = Instant::now();
    let (backend, bits, phase_counters) = match state.strategy {
        Strategy::Direct => {
            let reference = build_reference_sample_with_decision(instructions)?;
            let bits = match reference.decision {
                ReferenceSampleDecision::PackedInverse => reference.bits,
                other => {
                    return Err(format!("unsupported reference sample decision: {other:?}"));
                }
            };
            (DIRECT_BACKEND, bits, reference.phase_counters)
        }
        Strategy::Canonical => {
            let bits = rstim::executor::reference_sample(instructions)?;
            let counters = canonical_phase_counters(instructions, state.measurement_bits);
            (CANONICAL_BACKEND, bits, counters)
        }
    };
    let packed = pack_b8(&bits);
    let elapsed_ns = started.elapsed().as_nanos() as u64;

    state.reference_build_count += 1;
    Ok(ReferenceBuiltResponse {
        protocol: PROTOCOL,
        response_type: "reference_built",
        request_id: request.request_id,
        backend,
        parse_count: state.parse_count,
        reference_build_count: state.reference_build_count,
        measurement_bits: state.measurement_bits,
        packed_bytes: packed.len(),
        packed_base64: base64_standard(&packed),
        byte_sha256: format!("{:x}", Sha256::digest(&packed)),
        timer_scope: TIMER_SCOPE,
        elapsed_ns,
        phase_counters: request.include_phase_counters.then_some(phase_counters),
    })
}

fn handle_line(line: &str, state: &mut WorkerState) -> Result<(), String> {
    let header: RequestHeader = serde_json::from_str(line).map_err(|error| error.to_string())?;
    validate_protocol(&header.protocol)?;
    match header.request_type.as_str() {
        "load" => write_json_line(&handle_load(line, state)?),
        "build_reference" => write_json_line(&handle_build_reference(line, state)?),
        other => Err(format!("unexpected request type: {other}")),
    }
}

fn pack_b8(bits: &[bool]) -> Vec<u8> {
    let mut packed = vec![0_u8; bits.len().div_ceil(8)];
    for (index, bit) in bits.iter().enumerate() {
        if *bit {
            packed[index / 8] |= 1 << (index % 8);
        }
    }
    packed
}

fn canonical_phase_counters(
    instructions: &[StimInstr],
    measurement_bits: usize,
) -> ReferenceBuildPhaseCounters {
    ReferenceBuildPhaseCounters {
        measurement_reset_batches: measurement_bits,
        canonical_materializations: measurement_bits.max(1),
        canonical_writebacks: measurement_bits,
        expanded_repeat_iterations: count_repeat_iterations(instructions),
        executed_repeat_iterations: count_repeat_iterations(instructions),
        skipped_repeat_iterations: 0,
        measurement_bits,
        ..ReferenceBuildPhaseCounters::default()
    }
}

fn count_repeat_iterations(instructions: &[StimInstr]) -> usize {
    instructions
        .iter()
        .fold(0_usize, |total, instr| match instr {
            StimInstr::Op { .. } => total,
            StimInstr::Repeat { count, body } => {
                let count = usize::try_from(*count).unwrap_or(usize::MAX);
                total
                    .saturating_add(count)
                    .saturating_add(count.saturating_mul(count_repeat_iterations(body)))
            }
        })
}

fn base64_standard(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut index = 0;
    while index + 3 <= bytes.len() {
        let b0 = bytes[index];
        let b1 = bytes[index + 1];
        let b2 = bytes[index + 2];
        encoded.push(TABLE[(b0 >> 2) as usize] as char);
        encoded.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        encoded.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        encoded.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        index += 3;
    }

    match bytes.len() - index {
        0 => {}
        1 => {
            let b0 = bytes[index];
            encoded.push(TABLE[(b0 >> 2) as usize] as char);
            encoded.push(TABLE[((b0 & 0b0000_0011) << 4) as usize] as char);
            encoded.push('=');
            encoded.push('=');
        }
        2 => {
            let b0 = bytes[index];
            let b1 = bytes[index + 1];
            encoded.push(TABLE[(b0 >> 2) as usize] as char);
            encoded.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
            encoded.push(TABLE[((b1 & 0b0000_1111) << 2) as usize] as char);
            encoded.push('=');
        }
        _ => unreachable!("base64 remainder is modulo 3"),
    }
    encoded
}

fn run(args: Args) -> Result<(), String> {
    if args.protocol != PROTOCOL {
        return Err(format!(
            "requires --protocol {PROTOCOL}, got {}",
            args.protocol
        ));
    }

    let stdin = io::stdin();
    let mut state = WorkerState {
        strategy: args.strategy,
        instructions: None,
        parse_count: 0,
        reference_build_count: 0,
        measurement_bits: 0,
    };
    for line_result in stdin.lock().lines() {
        let line = line_result.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        if let Err(error) = handle_line(&line, &mut state) {
            write_error(error)?;
        }
    }
    Ok(())
}

fn main() {
    let args = Args::parse();
    if let Err(error) = run(args) {
        let _ = write_error(&error);
        eprintln!("{error}");
        std::process::exit(1);
    }
}
