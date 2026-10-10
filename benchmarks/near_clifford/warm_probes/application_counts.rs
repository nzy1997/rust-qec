//! Original-circuit raw postselection with structured or native Rust counts.
//! Native counts may lazily prepare an affine model; its cost is in first_ns.
//! Fallback scalar/packed counts retain early rejection and every typed draw.
#[path = "../phase_gate.rs"]
mod phase_gate;
use rand::{RngCore, SeedableRng, rngs::SmallRng};
use rstim::near_clifford::{
    CompiledNearCliffordExecutor, CompiledRotationArithmetic, NearCliffordShot,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{hint::black_box, time::Instant};

fn structured_counts(records: &[NearCliffordShot]) -> Result<(usize, usize), String> {
    let mut accepted = 0usize;
    let mut logical_errors = 0usize;
    for row in records {
        if row.detectors.iter().all(|d| !d) {
            accepted += 1;
            let mut found = false;
            let mut observable = false;
            for (index, parity) in &row.observables {
                if *index == 0 {
                    found = true;
                    observable ^= parity;
                }
            }
            if !found {
                return Err("observable 0 required".into());
            }
            logical_errors += usize::from(observable);
        }
    }
    Ok((accepted, logical_errors))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 6 && args.len() != 7 {
        return Err("usage: PROBE CIRCUIT SHOTS REPETITIONS strict|fused bench|validate [structured|native]".into());
    }
    let route = args.get(6).map_or("structured", String::as_str);
    if route != "structured" && route != "native" {
        return Err("Rust route must be structured or native".into());
    }
    let native = route == "native";
    let text = std::fs::read_to_string(&args[1])?;
    let shots: usize = args[2].parse()?;
    let repetitions: usize = args[3].parse()?;
    if shots == 0 || shots > 131072 || repetitions == 0 || repetitions > 64 {
        return Err("shots/repetitions exceeds bounded probe limits".into());
    }
    let arithmetic = match args[4].as_str() {
        "strict" => CompiledRotationArithmetic::Strict,
        "fused" => CompiledRotationArithmetic::Fused,
        _ => return Err("policy must be strict or fused".into()),
    };
    let start = Instant::now();
    let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, arithmetic)?;
    let compile_ns = start.elapsed().as_nanos();
    let start = Instant::now();
    let mut sampler = plan.prepare_sampler()?;
    let prepare_ns = start.elapsed().as_nanos();
    let validation = args[5] == "validate";
    if !validation && args[5] != "bench" {
        return Err("action must be bench or validate".into());
    }
    let mut reference_sampler = if native && validation {
        Some(plan.prepare_sampler()?)
    } else {
        None
    };
    let mut witness_bits = Vec::new();
    let mut witness_width = 0;
    let mut sample = |seed: u64| -> Result<_, String> {
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut reference_rng = rng.clone();
        let start = Instant::now();
        let (ns, records, accepted, logical_errors) = if native {
            let counts = sampler.sample_postselected_counts(shots, 0, &mut rng)?;
            let ns = start.elapsed().as_nanos();
            let records = if validation {
                let records = reference_sampler
                    .as_mut()
                    .unwrap()
                    .sample(shots, &mut reference_rng)?;
                if structured_counts(&records)? != (counts.accepted, counts.logical_errors)
                    || (0..16).any(|_| rng.next_u64() != reference_rng.next_u64())
                {
                    return Err(
                        "native counts or RNG continuation differs from structured witness".into(),
                    );
                }
                records
            } else {
                Vec::new()
            };
            (ns, records, counts.accepted, counts.logical_errors)
        } else {
            let records = sampler.sample(shots, &mut rng)?;
            let (accepted, logical_errors) = structured_counts(&records)?;
            (
                start.elapsed().as_nanos(),
                records,
                accepted,
                logical_errors,
            )
        };
        if validation {
            witness_width = records.first().map_or(0, |row| row.measurements.len());
            witness_bits.extend(
                records
                    .iter()
                    .flat_map(|row| row.measurements.iter().map(|b| u8::from(*b))),
            );
        }
        black_box(records);
        Ok((ns, accepted, logical_errors))
    };
    let first = if validation { (0, 0, 0) } else { sample(739)? };
    let phase = if validation {
        None
    } else {
        phase_gate::Gate::from_env()?
    };
    let mut observations = Vec::new();
    for rep in 0..repetitions {
        let mut elapsed = 0u128;
        let mut calls = 0usize;
        let mut attempted = 0usize;
        let mut accepted = 0usize;
        let mut errors = 0usize;
        while calls == 0
            || (validation && attempted < 8192)
            || (!validation && elapsed < 50_000_000)
        {
            let (ns, pass, logical) = sample(1739 + (rep * 1_000_000 + calls) as u64)?;
            elapsed += ns;
            calls += 1;
            attempted += shots;
            accepted += pass;
            errors += logical;
            if calls >= 1_000_000 {
                return Err("timing call limit reached".into());
            }
        }
        observations.push(
            json!({"elapsed_ns":elapsed,"calls":calls,"attempted":attempted,
            "accepted":accepted,"discarded":attempted-accepted,"logical_errors":errors,
            "ns_per_call":elapsed as f64/calls as f64}),
        );
    }
    if let Some(phase) = phase {
        phase.finish()?;
    }
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // The OS writes rusage only on success, checked before assume_init.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut rss = u64::try_from(unsafe { usage.assume_init() }.ru_maxrss)?;
    if cfg!(target_os = "linux") {
        rss = rss.checked_mul(1024).ok_or("RSS overflow")?;
    }
    let mut output = json!({"backend":"rstim","status":"ok","arithmetic":args[4],
        "input_sha256":format!("{:x}",Sha256::digest(text.as_bytes())),
        "output_contract":"all-zero raw detector postselection; raw observable 0 counts; no reference normalization",
        "execution":if native { "native raw postselected counts; optional affine model and scalar/packed early rejection" } else { "full structured records then filter; no early rejection" },
        "compile_ns":compile_ns,"prepare_ns":prepare_ns,"first_ns":first.0,
        "shots":shots,"observations":observations,"peak_rss_bytes":rss,
        "peak_active_rank":plan.peak_active_rank(),
        "cache_reserved_bytes":sampler.coefficient_cache_reserved_bytes()});
    if validation {
        if native {
            output["exact_native_counts_rng"] = json!(true);
        }
        output["measurements"] = json!(witness_bits);
        output["width"] = json!(witness_width);
        output["call_shots"] = json!(shots);
        output["shots"] = json!(
            shots
                * observations
                    .iter()
                    .map(|row| row["calls"].as_u64().unwrap() as usize)
                    .sum::<usize>()
        );
    }
    println!("{output}");
    Ok(())
}
