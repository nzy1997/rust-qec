//! Full structured sampling followed by all-zero detector filtering and counts.
//! This adapter introduces no early rejection or production sampling API.
use rand::{SeedableRng, rngs::SmallRng};
use rstim::near_clifford::{CompiledNearCliffordExecutor, CompiledRotationArithmetic};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{hint::black_box, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 6 {
        return Err("usage: PROBE CIRCUIT SHOTS REPETITIONS strict|fused bench|validate".into());
    }
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
    let mut witness_bits = Vec::new();
    let mut witness_width = 0;
    let mut sample = |seed: u64| -> Result<_, String> {
        let mut rng = SmallRng::seed_from_u64(seed);
        let start = Instant::now();
        let records = sampler.sample(shots, &mut rng)?;
        let mut accepted = 0usize;
        let mut logical_errors = 0usize;
        for row in &records {
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
        let ns = start.elapsed().as_nanos();
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
        "execution":"full structured records then filter; no early rejection",
        "compile_ns":compile_ns,"prepare_ns":prepare_ns,"first_ns":first.0,
        "shots":shots,"observations":observations,"peak_rss_bytes":rss,
        "peak_active_rank":plan.peak_active_rank(),
        "cache_reserved_bytes":sampler.coefficient_cache_reserved_bytes()});
    if validation {
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
