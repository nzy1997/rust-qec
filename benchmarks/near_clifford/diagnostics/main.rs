//! Public API diagnostic probe. No simulator hooks or altered defaults.
use rand::{RngCore, SeedableRng, rngs::SmallRng};
use rstim::near_clifford::{CompiledNearCliffordExecutor as Executor, CompiledRotationArithmetic};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{hint::black_box, time::Instant};

fn peak_rss_bytes() -> Result<u64, String> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // getrusage writes the complete structure on success; failure is checked.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let usage = unsafe { usage.assume_init() };
    let bytes = u64::try_from(usage.ru_maxrss).map_err(|_| "negative peak RSS")?;
    #[cfg(target_os = "linux")]
    let bytes = bytes.checked_mul(1024).ok_or("RSS overflow")?;
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    return Err("RSS units unsupported on this OS".into());
    Ok(bytes)
}

fn positive(v: &Value, key: &str) -> Result<usize, String> {
    let value = v[key]
        .as_u64()
        .ok_or_else(|| format!("missing integer {key}"))?;
    let value = usize::try_from(value).map_err(|_| "integer overflow")?;
    if value == 0 {
        return Err(format!("{key} must be positive"));
    }
    Ok(value)
}

fn run(config: &Value) -> Result<Value, String> {
    let text = std::fs::read_to_string(config["circuit"].as_str().ok_or("missing circuit")?)
        .map_err(|e| e.to_string())?;
    let input_sha256 = format!("{:x}", Sha256::digest(text.as_bytes()));
    let arithmetic = match config["arithmetic"].as_str() {
        Some("strict") => CompiledRotationArithmetic::Strict,
        Some("fused") => CompiledRotationArithmetic::Fused,
        _ => return Err("arithmetic must be explicit strict or fused".into()),
    };
    let cache = usize::try_from(
        config["cache_bytes"]
            .as_u64()
            .ok_or("missing cache_bytes")?,
    )
    .map_err(|_| "cache overflow")?;
    let shots = positive(config, "shots")?;
    let repetitions = positive(config, "repetitions")?;
    if shots > 65536 || repetitions > 64 {
        return Err("probe bounds exceeded".into());
    }
    let compile = || Executor::compile_text_with_arithmetic(&text, arithmetic);
    let start = Instant::now();
    let executor = match compile() {
        Ok(plan) => plan,
        Err(error) => {
            return Ok(json!({"status":"unsupported", "error":error,
            "input_sha256":input_sha256, "arithmetic":config["arithmetic"]}));
        }
    };
    let compile_ns = start.elapsed().as_nanos();
    let start = Instant::now();
    let mut sampler = executor.prepare_sampler_with_cache_budget(cache)?;
    let prepare_ns = start.elapsed().as_nanos();
    let mut metadata = json!({"schema":"rstim.near-clifford-diagnostic-probe.v1",
        "status":"ok", "input_sha256":input_sha256, "arithmetic":config["arithmetic"],
        "cache_bytes":cache, "peak_active_rank":executor.peak_active_rank(),
        "compile_ns":compile_ns, "prepare_ns":prepare_ns, "shots":shots,
        "rng":"SmallRng/rand-0.8.7", "config":config});
    let action = config["action"].as_str().ok_or("missing action")?;
    if action == "inspect" {
        return Ok(metadata);
    }
    if action == "dump" {
        let total = positive(config, "total")?;
        if total % shots != 0 || total > 131072 {
            return Err("invalid dump total".into());
        }
        let mut rng = SmallRng::seed_from_u64(739);
        let mut bits = Vec::new();
        let mut width = 0;
        for _ in 0..total / shots {
            match config["call_kind"].as_str().unwrap_or("flat") {
                "flat" => {
                    let output = sampler.sample_measurements_u8(shots, &mut rng)?;
                    width = output.measurements_per_shot;
                    bits.extend(output.measurements);
                }
                "structured" => {
                    let output = sampler.sample(shots, &mut rng)?;
                    width = output.first().map_or(0, |s| s.measurements.len());
                    bits.extend(
                        output
                            .iter()
                            .flat_map(|s| s.measurements.iter().map(|b| u8::from(*b))),
                    );
                }
                _ => return Err("dump call_kind must be flat or structured".into()),
            }
        }
        metadata["shots"] = json!(total);
        metadata["call_shots"] = json!(shots);
        metadata["call_kind"] = json!(config["call_kind"].as_str().unwrap_or("flat"));
        metadata["width"] = json!(width);
        metadata["measurements"] = json!(bits);
        metadata["continuation"] = json!((0..16).map(|_| rng.next_u64()).collect::<Vec<_>>());
    } else if action == "bench" {
        let mut cold = Vec::new();
        for rep in 0..repetitions {
            let start = Instant::now();
            let fresh = compile()?;
            let compile_ns = start.elapsed().as_nanos();
            let start = Instant::now();
            let mut first = fresh.prepare_sampler_with_cache_budget(cache)?;
            let prepare_ns = start.elapsed().as_nanos();
            let mut rng = SmallRng::seed_from_u64(739 + rep as u64);
            let start = Instant::now();
            let output = first.sample_measurements_u8(shots, &mut rng)?;
            let first_ns = start.elapsed().as_nanos();
            black_box(output);
            cold.push(json!({"compile_ns":compile_ns, "prepare_ns":prepare_ns,
                "first_ns":first_ns, "cache_reserved_bytes":first.coefficient_cache_reserved_bytes()}));
        }
        for rep in 0..3 {
            let mut rng = SmallRng::seed_from_u64(1739 + rep);
            black_box(sampler.sample_measurements_u8(shots, &mut rng)?);
        }
        let mut observations = Vec::new();
        for rep in 0..repetitions {
            let mut elapsed = 0u128;
            let mut calls = 0usize;
            while elapsed < 50_000_000 {
                let mut rng = SmallRng::seed_from_u64(2739 + (rep * 1_000_000 + calls) as u64);
                let start = Instant::now();
                let output = sampler.sample_measurements_u8(shots, &mut rng)?;
                elapsed += start.elapsed().as_nanos();
                black_box(output);
                calls += 1;
                if calls >= 1_000_000 {
                    return Err("warm timing bound exceeded".into());
                }
            }
            observations.push(json!({"elapsed_ns":elapsed, "calls":calls,
                "ns_per_call":elapsed as f64 / calls as f64}));
        }
        metadata["cold"] = json!(cold);
        metadata["warm"] = json!(observations);
    } else if action == "lifetime" {
        let history = config["history"].as_array().ok_or("missing history")?;
        if history.is_empty() || history.len() > 64 {
            return Err("invalid history length".into());
        }
        let mut histories = Vec::new();
        for rep in 0..repetitions {
            let wall = Instant::now();
            let fresh = compile()?;
            let compile_ns = wall.elapsed().as_nanos();
            let start = Instant::now();
            let mut sampler = fresh.prepare_sampler_with_cache_budget(cache)?;
            let prepare_ns = start.elapsed().as_nanos();
            let mut rng = SmallRng::seed_from_u64(739 + rep as u64);
            let mut calls = Vec::new();
            let mut outputs = Vec::new();
            let mut sum = 0u128;
            for request in history {
                let count = positive(request, "shots")?;
                if count > 65536 {
                    return Err("history shots exceeds bound".into());
                }
                let before = sampler.coefficient_cache_reserved_bytes();
                let start = Instant::now();
                let (elapsed, output) = match request["kind"].as_str() {
                    Some("flat") => {
                        let output = sampler.sample_measurements_u8(count, &mut rng)?;
                        (start.elapsed().as_nanos(), json!(output.measurements))
                    }
                    Some("structured") => {
                        let output = sampler.sample(count, &mut rng)?;
                        let elapsed = start.elapsed().as_nanos();
                        (
                            elapsed,
                            json!(
                                output
                                    .iter()
                                    .map(|s| json!({"measurements":s.measurements,
                            "detectors":s.detectors,"observables":s.observables}))
                                    .collect::<Vec<_>>()
                            ),
                        )
                    }
                    _ => return Err("history kind must be flat or structured".into()),
                };
                sum += elapsed;
                calls.push(json!({"request":request, "ns":elapsed,
                    "reserved_before":before,"reserved_after":sampler.coefficient_cache_reserved_bytes()}));
                outputs.push(output);
            }
            // Serialization and diagnostic bookkeeping are outside the API sum.
            // This total is the sum of measured phases, not a wall-clock claim.
            histories.push(json!({"compile_ns":compile_ns,"prepare_ns":prepare_ns,
                "sampling_ns":sum,"phase_sum_ns":compile_ns+prepare_ns+sum,"calls":calls,
                "outputs_sha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&outputs).map_err(|e|e.to_string())?)),
                "continuation":(0..16).map(|_|rng.next_u64()).collect::<Vec<_>>() }));
        }
        metadata["histories"] = json!(histories);
        metadata["cache_reserved_bytes"] = json!(
            metadata["histories"]
                .as_array()
                .and_then(|rows| rows.last())
                .and_then(|row| row["calls"].as_array())
                .and_then(|calls| calls.last())
                .map(|call| &call["reserved_after"])
        );
    } else {
        return Err("invalid action".into());
    }
    if action != "lifetime" {
        metadata["cache_reserved_bytes"] = json!(sampler.coefficient_cache_reserved_bytes());
    }
    metadata["peak_rss_bytes"] = json!(peak_rss_bytes()?);
    metadata["rss_scope"] = json!(
        "whole probe process high-water mark, includes validation/output bookkeeping where present"
    );
    Ok(metadata)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 {
        return Err("usage: near-clifford-diagnostics CONFIG.json".into());
    }
    let config: Value = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let result = run(&config)
        .unwrap_or_else(|error| json!({"status":"error","error":error,"config":config}));
    println!("{result}");
    Ok(())
}
