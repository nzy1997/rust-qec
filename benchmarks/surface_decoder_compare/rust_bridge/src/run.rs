use std::fs;
use std::time::Instant;

use rbposd::DecoderConfig;
use rilpqec::backend::{BatchBackend, build_batch_backend};
use rilpqec::{BackendConfig, BackendKind, IlpDecoderConfig, LoweredDemProblem, lower_dem_to_problem};
use rsinter::decode::{CompiledDecoder, Decoder, RbposdDemDecoder, RmatchingDemDecoder};
use rstim::dem::DetectorErrorModel;

use crate::protocol::{BridgeRequest, BridgeResponse};

pub fn handle_request(request: BridgeRequest) -> BridgeResponse {
    match try_handle_request(request) {
        Ok(response) => response,
        Err(message) => BridgeResponse::error(message),
    }
}

fn try_handle_request(request: BridgeRequest) -> Result<BridgeResponse, String> {
    validate_request(&request)?;

    let dem_text = fs::read_to_string(&request.dem_path)
        .map_err(|error| format!("failed to read DEM file {}: {error}", request.dem_path))?;
    let dem = DetectorErrorModel::parse(&dem_text)
        .map_err(|error| format!("failed to parse DEM file {}: {error}", request.dem_path))?;

    let dets = fs::read(&request.dets_b8_path).map_err(|error| {
        format!(
            "failed to read detections file {}: {error}",
            request.dets_b8_path
        )
    })?;
    let obs = fs::read(&request.obs_b8_path).map_err(|error| {
        format!(
            "failed to read observables file {}: {error}",
            request.obs_b8_path
        )
    })?;

    let expected_det_len = request.num_shots * bytes_per_shot(request.num_dets);
    if dets.len() < expected_det_len {
        return Err(format!(
            "detection buffer too short: expected at least {expected_det_len} bytes, got {}",
            dets.len()
        ));
    }

    let expected_obs_len = request.num_shots * bytes_per_shot(request.num_obs);
    if obs.len() < expected_obs_len {
        return Err(format!(
            "observable buffer too short: expected at least {expected_obs_len} bytes, got {}",
            obs.len()
        ));
    }

    match request.decoder.as_str() {
        "rmatching" => {
            let decoder = RmatchingDemDecoder;
            run_native_decoder(request, dem, dets, obs, "native", &decoder)
        }
        "rbposd" => {
            let decoder = RbposdDemDecoder::new(DecoderConfig::default());
            run_native_decoder(request, dem, dets, obs, "native", &decoder)
        }
        "rilpqec" => run_rilpqec(request, dem, dets, obs),
        other => Err(format!("unknown decoder: {other}")),
    }
}

fn run_native_decoder(
    request: BridgeRequest,
    dem: DetectorErrorModel,
    dets: Vec<u8>,
    obs: Vec<u8>,
    backend: &str,
    decoder: &dyn Decoder,
) -> Result<BridgeResponse, String> {
    let compile_started = Instant::now();
    let compiled = decoder.compile_for_dem(&dem);
    let compile_us = elapsed_us(compile_started);
    let summary = decode_batches(&request, &dets, &obs, compiled.as_ref());

    Ok(build_success_response(
        request.decoder,
        backend.to_string(),
        compile_us,
        summary,
    ))
}

fn run_rilpqec(
    request: BridgeRequest,
    dem: DetectorErrorModel,
    dets: Vec<u8>,
    obs: Vec<u8>,
) -> Result<BridgeResponse, String> {
    let compile_started = Instant::now();
    let problem = lower_dem_to_problem(&dem)
        .map_err(|error| format!("failed to lower DEM for rilpqec: {error}"))?;
    let (mut backend, backend_name) = compile_rilpqec_backend(&problem)?;
    let compile_us = elapsed_us(compile_started);
    let summary = decode_rilpqec_batches(&request, &dets, &obs, &problem, backend.as_mut())?;

    Ok(build_success_response(
        request.decoder,
        backend_name,
        compile_us,
        summary,
    ))
}

fn ilp_config(kind: BackendKind) -> IlpDecoderConfig {
    IlpDecoderConfig {
        backend: BackendConfig {
            kind,
            time_limit_seconds: None,
            mip_gap: None,
            threads: Some(1),
            verbose: false,
        },
    }
}

fn decode_batches(
    request: &BridgeRequest,
    dets: &[u8],
    obs: &[u8],
    decoder: &dyn CompiledDecoder,
) -> DecodeSummary {
    let det_bytes = bytes_per_shot(request.num_dets);
    let obs_bytes = bytes_per_shot(request.num_obs);
    let batch_size = request.batch_size.max(1);

    let mut shots_used = 0usize;
    let mut logical_errors = 0usize;
    let mut total_decode_us = 0.0;

    while shots_used < request.num_shots && logical_errors < request.max_errors {
        let remaining = request.num_shots - shots_used;
        let shots_in_batch = remaining.min(batch_size);
        let det_start = shots_used * det_bytes;
        let det_end = det_start + shots_in_batch * det_bytes;
        let obs_start = shots_used * obs_bytes;
        let obs_end = obs_start + shots_in_batch * obs_bytes;

        let decode_started = Instant::now();
        let predictions = decoder.decode_shots_bit_packed(
            &dets[det_start..det_end],
            shots_in_batch,
            request.num_dets,
            request.num_obs,
        );
        total_decode_us += elapsed_us(decode_started);

        let batch_logical_errors = count_logical_errors(
            &predictions,
            &obs[obs_start..obs_end],
            shots_in_batch,
            request.num_obs,
        );
        logical_errors += batch_logical_errors;
        shots_used += shots_in_batch;
    }

    DecodeSummary {
        shots_used,
        logical_errors,
        total_decode_us,
    }
}

fn decode_rilpqec_batches(
    request: &BridgeRequest,
    dets: &[u8],
    obs: &[u8],
    problem: &LoweredDemProblem,
    backend: &mut dyn BatchBackend,
) -> Result<DecodeSummary, String> {
    if request.num_dets != problem.num_detectors {
        return Err(format!(
            "detector width mismatch for rilpqec: expected {}, got {}",
            problem.num_detectors, request.num_dets
        ));
    }
    if request.num_obs != problem.num_observables {
        return Err(format!(
            "observable width mismatch for rilpqec: expected {}, got {}",
            problem.num_observables, request.num_obs
        ));
    }

    let det_bytes = bytes_per_shot(request.num_dets);
    let obs_bytes = bytes_per_shot(request.num_obs);
    let batch_size = request.batch_size.max(1);

    let mut shots_used = 0usize;
    let mut logical_errors = 0usize;
    let mut total_decode_us = 0.0;

    while shots_used < request.num_shots && logical_errors < request.max_errors {
        let remaining = request.num_shots - shots_used;
        let shots_in_batch = remaining.min(batch_size);
        let decode_started = Instant::now();
        let mut predictions = vec![0u8; shots_in_batch * obs_bytes];

        for shot_in_batch in 0..shots_in_batch {
            let absolute_shot = shots_used + shot_in_batch;
            let syndrome = unpack_shot_bits(
                &dets[absolute_shot * det_bytes..(absolute_shot + 1) * det_bytes],
                request.num_dets,
            );
            let correction = backend
                .solve(&syndrome)
                .map_err(|error| format!("rilpqec decode failed: {error}"))?;
            let observables = problem
                .observables_from_correction(&correction)
                .map_err(|error| format!("rilpqec observable projection failed: {error}"))?;
            pack_observables_into(
                &observables,
                &mut predictions[shot_in_batch * obs_bytes..(shot_in_batch + 1) * obs_bytes],
            );
        }

        total_decode_us += elapsed_us(decode_started);

        let obs_start = shots_used * obs_bytes;
        let obs_end = obs_start + shots_in_batch * obs_bytes;
        let batch_logical_errors = count_logical_errors(
            &predictions,
            &obs[obs_start..obs_end],
            shots_in_batch,
            request.num_obs,
        );
        logical_errors += batch_logical_errors;
        shots_used += shots_in_batch;
    }

    Ok(DecodeSummary {
        shots_used,
        logical_errors,
        total_decode_us,
    })
}

fn count_logical_errors(
    predictions: &[u8],
    obs: &[u8],
    num_shots: usize,
    num_obs: usize,
) -> usize {
    let obs_bytes = bytes_per_shot(num_obs);
    let mut logical_errors = 0usize;

    for shot in 0..num_shots {
        let start = shot * obs_bytes;
        let end = start + obs_bytes;
        if shot_has_logical_error(&predictions[start..end], &obs[start..end], num_obs) {
            logical_errors += 1;
        }
    }

    logical_errors
}

fn shot_has_logical_error(predicted: &[u8], actual: &[u8], num_obs: usize) -> bool {
    let full_bytes = num_obs / 8;
    for idx in 0..full_bytes {
        if predicted[idx] != actual[idx] {
            return true;
        }
    }

    let tail_bits = num_obs % 8;
    if tail_bits == 0 {
        return false;
    }

    let mask = (1u8 << tail_bits) - 1;
    (predicted[full_bytes] ^ actual[full_bytes]) & mask != 0
}

fn build_success_response(
    decoder: String,
    backend: String,
    compile_us: f64,
    summary: DecodeSummary,
) -> BridgeResponse {
    BridgeResponse {
        status: "ok".to_string(),
        decoder,
        backend,
        shots_used: summary.shots_used,
        logical_errors: summary.logical_errors,
        compile_us,
        total_decode_us: summary.total_decode_us,
        error: String::new(),
    }
}

fn compile_rilpqec_backend(
    problem: &LoweredDemProblem,
) -> Result<(Box<dyn BatchBackend>, String), String> {
    let preferred_config = ilp_config(BackendKind::Gurobi);
    match build_batch_backend(problem, &preferred_config) {
        Ok(backend) => Ok((backend, "gurobi".to_string())),
        Err(gurobi_error) => {
            let fallback_config = ilp_config(BackendKind::Highs);
            let backend = build_batch_backend(problem, &fallback_config).map_err(|highs_error| {
                format!(
                    "failed to compile rilpqec with gurobi ({gurobi_error}) and highs ({highs_error})"
                )
            })?;
            Ok((backend, "highs".to_string()))
        }
    }
}

fn validate_request(request: &BridgeRequest) -> Result<(), String> {
    if request.num_shots == 0 {
        return Err("num_shots must be positive".to_string());
    }
    if request.batch_size == 0 {
        return Err("batch_size must be positive".to_string());
    }
    Ok(())
}

fn bytes_per_shot(num_bits: usize) -> usize {
    num_bits.div_ceil(8)
}

fn elapsed_us(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1_000_000.0
}

fn unpack_shot_bits(packed: &[u8], num_bits: usize) -> Vec<bool> {
    let mut bits = vec![false; num_bits];
    for bit in 0..num_bits {
        bits[bit] = ((packed[bit / 8] >> (bit % 8)) & 1) != 0;
    }
    bits
}

fn pack_observables_into(observables: &[bool], out: &mut [u8]) {
    out.fill(0);
    for (obs, &value) in observables.iter().enumerate() {
        if value {
            out[obs / 8] |= 1 << (obs % 8);
        }
    }
}

struct DecodeSummary {
    shots_used: usize,
    logical_errors: usize,
    total_decode_us: f64,
}
