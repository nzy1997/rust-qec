use rand::{RngCore, SeedableRng, rngs::SmallRng};
use rstim::near_clifford::{CompiledNearCliffordExecutor, CompiledRotationArithmetic};
use serde_json::{Value, json};
use std::{hint::black_box, time::Instant};

fn indices(value: &Value) -> Vec<usize> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as usize)
        .collect()
}
fn fold(
    bits: &[u8],
    detectors: &[Vec<usize>],
    observable: &[usize],
    width: usize,
) -> (usize, usize) {
    let mut accepted = 0;
    let mut errors = 0;
    for row in bits.chunks_exact(width) {
        if detectors
            .iter()
            .all(|mask| mask.iter().fold(0, |bit, &index| bit ^ row[index]) == 0)
        {
            accepted += 1;
            errors += observable.iter().fold(0, |bit, &index| bit ^ row[index]) as usize;
        }
    }
    (accepted, errors)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 8 {
        return Err(
            "CIRCUIT MASKS SHOTS strict|fused structured|flat|native validate|bench REPETITIONS required"
                .into(),
        );
    }
    let text = std::fs::read_to_string(&args[1])?;
    let masks: Value = serde_json::from_str(&std::fs::read_to_string(&args[2])?)?;
    let width = masks["width"].as_u64().unwrap() as usize;
    let detectors: Vec<Vec<usize>> = masks["detectors"]
        .as_array()
        .unwrap()
        .iter()
        .map(indices)
        .collect();
    let observable = indices(&masks["observable"]);
    let shots: usize = args[3].parse()?;
    let policy = match args[4].as_str() {
        "strict" => CompiledRotationArithmetic::Strict,
        "fused" => CompiledRotationArithmetic::Fused,
        _ => return Err("policy".into()),
    };
    let plan = CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, policy)?;
    if args[6] == "validate" {
        let mut exact = true;
        for seed in [739, 1739, 2739, 1002739] {
            let mut scalar = plan.prepare_sampler()?;
            let mut flat = plan.prepare_sampler()?;
            let mut native = plan.prepare_sampler()?;
            let mut a = SmallRng::seed_from_u64(seed);
            let mut b = SmallRng::seed_from_u64(seed);
            let mut c = SmallRng::seed_from_u64(seed);
            let records = scalar.sample(shots, &mut a)?;
            let bits = flat.sample_measurements_u8(shots, &mut b)?;
            if bits.measurements_per_shot != width {
                return Err("measurement width".into());
            }
            let scalar_bits: Vec<u8> = records
                .iter()
                .flat_map(|row| row.measurements.iter().copied().map(u8::from))
                .collect();
            let structured_counts = records
                .iter()
                .filter(|row| row.detectors.iter().all(|&bit| !bit))
                .fold((0, 0), |(a, e), row| {
                    (
                        a + 1,
                        e + usize::from(
                            row.observables
                                .iter()
                                .filter(|(index, _)| *index == 0)
                                .fold(false, |bit, (_, b)| bit ^ *b),
                        ),
                    )
                });
            let counts = fold(&bits.measurements, &detectors, &observable, width);
            let native_counts = native.sample_postselected_counts(shots, 0, &mut c)?;
            let continuation_a: Vec<_> = (0..16).map(|_| a.next_u64()).collect();
            let continuation_b: Vec<_> = (0..16).map(|_| b.next_u64()).collect();
            let continuation_c: Vec<_> = (0..16).map(|_| c.next_u64()).collect();
            exact &= scalar_bits == bits.measurements
                && structured_counts == counts
                && continuation_a == continuation_b
                && counts == (native_counts.accepted, native_counts.logical_errors)
                && native_counts.attempted == shots
                && continuation_a == continuation_c;
            if !exact {
                return Err(format!(
                    "route mismatch seed{seed}: raw={}, counts={}, continuation={}",
                    scalar_bits == bits.measurements,
                    structured_counts == counts,
                    continuation_a == continuation_b
                )
                .into());
            }
        }
        println!(
            "{}",
            json!({"status":"ok","shots":shots,"policy":args[4],"exact_records_counts_rng":exact,"native_counts_rng":exact,"seeds":4})
        );
        return Ok(());
    }
    if args[6] != "bench" {
        return Err("action".into());
    }
    let reps: usize = args[7].parse()?;
    let mut sampler = plan.prepare_sampler()?;
    let mut sample = |seed| -> Result<_, String> {
        let mut rng = SmallRng::seed_from_u64(seed);
        let start = Instant::now();
        match args[5].as_str() {
            "structured" => {
                let records = sampler.sample(shots, &mut rng)?;
                let counts = records
                    .iter()
                    .filter(|row| row.detectors.iter().all(|&bit| !bit))
                    .fold((0, 0), |(a, e), row| {
                        (
                            a + 1,
                            e + usize::from(
                                row.observables
                                    .iter()
                                    .filter(|(index, _)| *index == 0)
                                    .fold(false, |bit, (_, b)| bit ^ *b),
                            ),
                        )
                    });
                let elapsed = start.elapsed().as_nanos();
                black_box((records, counts));
                Ok(elapsed)
            }
            "flat" => {
                let records = sampler.sample_measurements_u8(shots, &mut rng)?;
                let counts = fold(&records.measurements, &detectors, &observable, width);
                let elapsed = start.elapsed().as_nanos();
                black_box((records, counts));
                Ok(elapsed)
            }
            "native" => {
                let counts = sampler.sample_postselected_counts(shots, 0, &mut rng)?;
                let elapsed = start.elapsed().as_nanos();
                black_box(counts);
                Ok(elapsed)
            }
            _ => Err("route".into()),
        }
    };
    for seed in 739..742 {
        black_box(sample(seed)?);
    }
    let mut observations = Vec::new();
    for rep in 0..reps {
        let mut ns = 0u128;
        let mut calls = 0u64;
        while ns < 50_000_000 {
            ns += sample(2739 + rep as u64 * 1_000_000 + calls)?;
            calls += 1;
        }
        observations
            .push(json!({"elapsed_ns":ns,"calls":calls,"ns_per_call":ns as f64/calls as f64}));
    }
    println!(
        "{}",
        json!({"shots":shots,"policy":args[4],"route":args[5],"observations":observations})
    );
    Ok(())
}
