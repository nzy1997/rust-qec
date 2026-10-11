//! Fixed call prefixes, including first use; no adaptive timing or warm-up.
use rand::{RngCore, SeedableRng, rngs::SmallRng};
use rstim::near_clifford::{
    CompiledNearCliffordExecutor, CompiledRotationArithmetic, NearCliffordShot,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{hint::black_box, time::Instant};

fn structured_counts(rows: &[NearCliffordShot]) -> Result<(usize, usize), String> {
    let mut accepted = 0;
    let mut errors = 0;
    for row in rows {
        if row.detectors.iter().all(|bit| !bit) {
            let mut found = false;
            let mut logical = false;
            for (index, bit) in &row.observables {
                if *index == 0 {
                    found = true;
                    logical ^= bit;
                }
            }
            if !found {
                return Err("observable 0 required".into());
            }
            accepted += 1;
            errors += usize::from(logical);
        }
    }
    Ok((accepted, errors))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 6 {
        return Err("usage: PROBE CIRCUIT SHOTS strict|fused default|off bench|validate".into());
    }
    let shots: usize = args[2].parse()?;
    if ![1, 64, 1024].contains(&shots) {
        return Err("shots must be one of 1, 64, 1024".into());
    }
    let arithmetic = match args[3].as_str() {
        "strict" => CompiledRotationArithmetic::Strict,
        "fused" => CompiledRotationArithmetic::Fused,
        _ => return Err("unknown arithmetic".into()),
    };
    let cache_off = match args[4].as_str() {
        "default" => false,
        "off" => true,
        _ => return Err("unknown cache factor".into()),
    };
    let validate = match args[5].as_str() {
        "bench" => false,
        "validate" => true,
        _ => return Err("unknown action".into()),
    };
    let text = std::fs::read_to_string(&args[1])?;
    let start = Instant::now();
    let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, arithmetic)?;
    let compile_ns = start.elapsed().as_nanos();
    let start = Instant::now();
    let mut sampler = if cache_off {
        plan.prepare_sampler_with_cache_budget(0)?
    } else {
        plan.prepare_sampler()?
    };
    let prepare_ns = start.elapsed().as_nanos();
    let mut reference = if validate {
        Some(plan.prepare_sampler_with_cache_budget(0)?)
    } else {
        None
    };
    let maximum = if validate { 8 } else { 256 };
    let mut calls = Vec::with_capacity(maximum);
    let mut prefixes = Vec::new();
    let mut cumulative_ns = 0u128;
    for index in 0..maximum {
        let seed = 1739 + index as u64;
        let start = Instant::now();
        let mut rng = SmallRng::seed_from_u64(seed);
        let result = sampler.sample_postselected_counts(shots, 0, &mut rng)?;
        let scalars = (
            result.attempted,
            result.accepted,
            result.attempted.saturating_sub(result.accepted),
            result.logical_errors,
        );
        black_box(scalars);
        black_box(result);
        let elapsed_ns = start.elapsed().as_nanos();
        if result.attempted != shots
            || result.accepted > result.attempted
            || result.logical_errors > result.accepted
        {
            return Err("invalid native counts".into());
        }
        cumulative_ns += elapsed_ns;
        let mut row = json!({"call_index":index,"seed":seed,"elapsed_ns":elapsed_ns,
            "attempted":scalars.0,"accepted":scalars.1,"discarded":scalars.2,
            "logical_errors":scalars.3,
            "cache_reserved_after":sampler.coefficient_cache_reserved_bytes()});
        if let Some(reference) = reference.as_mut() {
            let mut reference_rng = SmallRng::seed_from_u64(seed);
            let records = reference.sample(shots, &mut reference_rng)?;
            if structured_counts(&records)? != (result.accepted, result.logical_errors) {
                return Err("counts differ from uncached structured reference".into());
            }
            let tail: Vec<u64> = (0..16).map(|_| rng.next_u64()).collect();
            let reference_tail: Vec<u64> = (0..16).map(|_| reference_rng.next_u64()).collect();
            if tail != reference_tail {
                return Err("RNG continuation differs from uncached structured reference".into());
            }
            row["rng_tail"] = json!(tail);
            row["measurements"] =
                json!(records.iter().map(|r| &r.measurements).collect::<Vec<_>>());
        }
        calls.push(row);
        let horizon = index + 1;
        if horizon.is_power_of_two() {
            prefixes.push(json!({"calls":horizon,"sampling_ns":cumulative_ns,
                "compile_prepare_sampling_ns":compile_ns+prepare_ns+cumulative_ns}));
        }
    }
    let reserved = sampler.coefficient_cache_reserved_bytes();
    let start = Instant::now();
    drop(sampler);
    let teardown_ns = start.elapsed().as_nanos();
    println!(
        "{}",
        json!({"schema":"rstim.fixed-call-lifecycle.draft.v1",
        "timing_contract":"fixed-seeded-counts-prefix-v1","action":args[5],
        "input_sha256":format!("{:x}",Sha256::digest(text.as_bytes())),
        "arithmetic":args[3],"cache_factor":args[4],"shots":shots,
        "compile_ns":compile_ns,"prepare_ns":prepare_ns,"teardown_ns":teardown_ns,
        "final_compile_prepare_sampling_teardown_ns":compile_ns+prepare_ns+cumulative_ns+teardown_ns,
        "cache_reserved_final":reserved,"peak_active_rank":plan.peak_active_rank(),
        "calls":calls,"prefixes":prefixes})
    );
    Ok(())
}
