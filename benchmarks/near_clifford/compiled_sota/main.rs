//! Measurement-record API probe. Process launch, validation and destruction are untimed.
use rand::{RngCore, SeedableRng, rngs::SmallRng};
use rstim::near_clifford::{CompiledNearCliffordExecutor, CompiledRotationArithmetic};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{hint::black_box, time::Instant};

const ROTATION_ARITHMETIC: CompiledRotationArithmetic = CompiledRotationArithmetic::Fused;

fn arithmetic_name(arithmetic: CompiledRotationArithmetic) -> &'static str {
    match arithmetic {
        CompiledRotationArithmetic::Strict => "strict",
        CompiledRotationArithmetic::Fused => "fused",
    }
}

fn compile_frozen(text: &str) -> Result<CompiledNearCliffordExecutor, String> {
    // The immutable policy also governs deterministic-prefix compilation.
    let plan =
        CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, ROTATION_ARITHMETIC)?;
    if !matches!(
        plan.rotation_arithmetic(),
        CompiledRotationArithmetic::Fused
    ) {
        return Err("compiled rotation arithmetic differs from the frozen fused policy".into());
    }
    Ok(plan)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 6 && args.len() != 7 {
        return Err(
            "usage: near-clifford-compiled-sota CIRCUIT SHOTS REPETITIONS WARMUPS dump|bench"
                .into(),
        );
    }
    let text = std::fs::read_to_string(&args[1])?;
    // Bind evidence to the exact bytes consumed by compile_text, untimed.
    let input_sha256 = format!("{:x}", Sha256::digest(text.as_bytes()));
    let shots: usize = args[2].parse()?;
    let repetitions: usize = args[3].parse()?;
    let warmups: usize = args[4].parse()?;
    if repetitions == 0 || shots == 0 {
        return Err("SHOTS and REPETITIONS must be positive".into());
    }
    if !["dump", "bench"].contains(&args[5].as_str()) {
        return Err("last argument must be dump or bench".into());
    }
    let start = Instant::now();
    let executor = compile_frozen(&text)?;
    let compile_ns = start.elapsed().as_nanos();
    let rotation_arithmetic = arithmetic_name(executor.rotation_arithmetic());
    let start = Instant::now();
    let mut sampler = executor.prepare_sampler()?;
    let prepare_ns = start.elapsed().as_nanos();
    if args[5] == "dump" {
        let mut rng = SmallRng::seed_from_u64(739);
        let total: usize = args.get(6).map(|s| s.parse()).transpose()?.unwrap_or(shots);
        if total < shots || total % shots != 0 {
            return Err("dump total must be a positive multiple of call shots".into());
        }
        let mut records = Vec::new();
        let mut width = 0;
        for _ in 0..total / shots {
            let output = sampler.sample_measurements_u8(shots, &mut rng)?;
            width = output.measurements_per_shot;
            records.extend(output.measurements);
        }
        println!(
            "{}",
            json!({"backend": "rstim", "api": "CompiledNearCliffordExecutor",
            "input_sha256": input_sha256, "rotation_arithmetic": rotation_arithmetic,
            "rng": "SmallRng/rand-0.8.7", "peak_active_rank": executor.peak_active_rank(),
            "cache_reserved_bytes": sampler.coefficient_cache_reserved_bytes(), "shots": total, "call_shots": shots, "width": width,
            "measurements": records, "continuation": rng.next_u64()})
        );
        return Ok(());
    }
    let mut first_ns = Vec::new();
    for rep in 0..repetitions {
        let fresh = compile_frozen(&text)?;
        let mut first = fresh.prepare_sampler()?;
        let mut rng = SmallRng::seed_from_u64(739 + rep as u64);
        let start = Instant::now();
        let output = first.sample_measurements_u8(shots, &mut rng)?;
        first_ns.push(start.elapsed().as_nanos());
        black_box(output);
    }
    // Warm each workload with its requested batch, rather than silently warming
    // a one-shot workload with an unrelated large batch.
    for rep in 0..warmups {
        let mut rng = SmallRng::seed_from_u64(1739 + rep as u64);
        black_box(sampler.sample_measurements_u8(shots, &mut rng)?);
    }
    let mut warm_ns = Vec::new();
    let mut warm_totals_ns = Vec::new();
    let mut warm_calls = Vec::new();
    let mut width = 0;
    for rep in 0..repetitions {
        let mut elapsed = 0;
        let mut calls = 0;
        while elapsed < 50_000_000 {
            let mut rng = SmallRng::seed_from_u64(2739 + (rep * 1_000_000 + calls) as u64);
            let start = Instant::now();
            let output = sampler.sample_measurements_u8(shots, &mut rng)?;
            elapsed += start.elapsed().as_nanos();
            width = output.measurements_per_shot;
            black_box(output);
            calls += 1;
            if calls >= 1_000_000 {
                return Err("warm timing did not reach 50 ms in one million calls".into());
            }
        }
        warm_totals_ns.push(elapsed);
        warm_calls.push(calls);
        warm_ns.push(elapsed as f64 / calls as f64);
    }
    println!(
        "{}",
        json!({"backend": "rstim", "api": "CompiledNearCliffordExecutor",
        "input_sha256": input_sha256, "rotation_arithmetic": rotation_arithmetic,
        "peak_active_rank": executor.peak_active_rank(),
        "cache_reserved_bytes": sampler.coefficient_cache_reserved_bytes(), "rng": "SmallRng/rand-0.8.7",
        "compile_ns": compile_ns, "prepare_ns": prepare_ns, "shots": shots,
        "width": width, "first_ns": first_ns, "warm_ns": warm_ns,
        "warm_totals_ns": warm_totals_ns, "warm_calls": warm_calls})
    );
    Ok(())
}
