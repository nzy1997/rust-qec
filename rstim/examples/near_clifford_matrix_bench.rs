//! Reproducible, single-process measurements for one near-Clifford fixture.
//! Run via benchmarks/near_clifford/run.py so each fixture gets its own RSS peak.

use rand::{RngCore, SeedableRng, rngs::StdRng};
use rstim::near_clifford::{NearCliffordExecutor, NearCliffordSampler};
use serde_json::{Value, json};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

const TERMINAL_20Q: &str = include_str!("../tests/fixtures/near_clifford_batch_20q_8t.stim");
const REPEATED_20Q: &str =
    include_str!("../tests/fixtures/near_clifford_batch_20q_8t_repeated.stim");

fn circuit(name: &str) -> Result<String, String> {
    match name {
        "terminal_20q" => Ok(TERMINAL_20Q.into()),
        "repeated_20q" => Ok(REPEATED_20Q.into()),
        "mid_feedback" => Ok("H 0\nT 0\nX_ERROR(0.5) 1\nM 1\nCX rec[-1] 0\nT 0\nH 0\nM 0\n".into()),
        "qec_rounds" => {
            // Three syndrome rounds on four data and three ancilla qubits.
            let mut text = String::from("H 1\nT 1\n");
            for round in 0..3 {
                text.push_str("X_ERROR(0.01) 0 1 2 3\n");
                for (left, right, ancilla) in [(0, 1, 4), (1, 2, 5), (2, 3, 6)] {
                    text.push_str(&format!(
                        "CX {left} {ancilla}\nCX {right} {ancilla}\nMR {ancilla}\n"
                    ));
                    if round > 0 {
                        text.push_str("DETECTOR rec[-1] rec[-4]\n");
                    }
                }
                text.push_str("CX rec[-1] 3\nT 1\n");
            }
            text.push_str("M 0 1 2 3\n");
            Ok(text)
        }
        name if name.starts_with("records_") => {
            let count: usize = name[8..].parse().map_err(|_| "invalid record width")?;
            Ok(format!("H 0\nM {}\n", vec!["0"; count].join(" ")))
        }
        name if name.starts_with("random_") => {
            let count: usize = name[7..].parse().map_err(|_| "invalid random width")?;
            let mut text = String::new();
            for q in 0..count {
                text.push_str(&format!("H {q}\n"));
            }
            text.push_str(&format!(
                "M {}\n",
                (0..count)
                    .map(|q| q.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
            Ok(text)
        }
        name if name.starts_with("rank_") => {
            let count: usize = name[5..].parse().map_err(|_| "invalid active rank")?;
            let mut text = String::new();
            for q in 0..count {
                text.push_str(&format!("H {q}\nT {q}\n"));
            }
            text.push_str(&format!(
                "MX {}\n",
                (0..count)
                    .map(|q| q.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
            Ok(text)
        }
        _ => Err(format!("unknown fixture: {name}")),
    }
}

fn median(mut values: Vec<u128>) -> u128 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn measure<T>(
    mut operation: impl FnMut(usize) -> Result<T, String>,
    repetitions: usize,
) -> Result<Value, String> {
    let mut times = Vec::with_capacity(repetitions);
    for repetition in 0..repetitions {
        let start = Instant::now();
        let result = operation(repetition)?;
        times.push(start.elapsed().as_nanos());
        black_box(result);
    }
    Ok(json!({"median_ns": median(times.clone()), "raw_ns": times}))
}

fn rngs(repetitions: usize) -> Vec<StdRng> {
    (0..repetitions)
        .map(|rep| StdRng::seed_from_u64(739 + rep as u64))
        .collect()
}

fn measure_first<T>(
    executor: &NearCliffordExecutor,
    repetitions: usize,
    mut operation: impl FnMut(&mut NearCliffordSampler<'_>, &mut StdRng) -> Result<T, String>,
) -> Result<Value, String> {
    let mut times = Vec::with_capacity(repetitions);
    for repetition in 0..repetitions {
        let mut sampler = executor.prepare_sampler()?;
        let mut rng = StdRng::seed_from_u64(739 + repetition as u64);
        let start = Instant::now();
        let result = operation(&mut sampler, &mut rng)?;
        times.push(start.elapsed().as_nanos());
        black_box(result);
    }
    Ok(json!({"median_ns": median(times.clone()), "raw_ns": times}))
}

fn verify_fixture(executor: &NearCliffordExecutor) -> Result<(), String> {
    let mut structured = executor.prepare_sampler()?;
    let mut flat = executor.prepare_sampler()?;
    let mut structured_rng = StdRng::seed_from_u64(739);
    let mut flat_rng = StdRng::seed_from_u64(739);
    let shots = structured.sample(128, &mut structured_rng)?;
    let bytes = flat.sample_measurements_u8(128, &mut flat_rng)?;
    let expected = shots
        .iter()
        .flat_map(|shot| shot.measurements.iter().map(|&bit| u8::from(bit)))
        .collect::<Vec<_>>();
    let continuation = structured_rng.next_u64();
    if bytes.measurements != expected || continuation != flat_rng.next_u64() {
        return Err("flat and structured outputs or RNG continuation differ".into());
    }
    let mut reference_rng = StdRng::seed_from_u64(739);
    let reference = (0..128)
        .map(|_| executor.run(&mut reference_rng))
        .collect::<Result<Vec<_>, _>>()?;
    if shots != reference || continuation != reference_rng.next_u64() {
        return Err("prepared batch differs from individual execution".into());
    }
    Ok(())
}

fn run(name: &str, quick: bool) -> Result<Value, String> {
    let text = circuit(name)?;
    let repetitions = if quick { 3 } else { 9 };
    let compile = measure(
        |_| NearCliffordExecutor::compile_text(black_box(&text)),
        repetitions,
    )?;
    let executor = NearCliffordExecutor::compile_text(&text)?;
    verify_fixture(&executor)?;
    let prepare = measure(|_| executor.prepare_sampler(), repetitions)?;
    let counts: &[usize] = if name == "qec_rounds" || name == "mid_feedback" {
        if quick {
            &[1, 64]
        } else {
            &[1, 63, 64, 1_000, 100_000]
        }
    } else if name.starts_with("rank_") {
        if quick { &[64] } else { &[64, 1_000] }
    } else if name.starts_with("random_") || name.starts_with("records_") {
        if quick { &[64] } else { &[64, 1_000] }
    } else if quick {
        &[1, 64]
    } else {
        &[1, 63, 64, 1_000, 100_000]
    };
    let mut measurements = Vec::new();
    for &shots in counts {
        // Use independent seeded streams for fresh and retained calls.
        let mut unprepared_rngs = rngs(repetitions);
        let unprepared = measure(
            |rep| executor.sample(shots, &mut unprepared_rngs[rep]),
            repetitions,
        )?;
        let mut cold_rngs = rngs(repetitions);
        let cold = measure(
            |rep| {
                let mut sampler = executor.prepare_sampler()?;
                sampler.sample(shots, &mut cold_rngs[rep])
            },
            repetitions,
        )?;
        let first_structured = measure_first(&executor, repetitions, |sampler, rng| {
            sampler.sample(shots, rng)
        })?;
        let first_flat = measure_first(&executor, repetitions, |sampler, rng| {
            sampler.sample_measurements_u8(shots, rng)
        })?;
        let mut structured_sampler = executor.prepare_sampler()?;
        let mut flat_sampler = executor.prepare_sampler()?;
        let mut warm_rng = StdRng::seed_from_u64(1379);
        black_box(structured_sampler.sample(64, &mut warm_rng)?);
        let mut flat_rng = StdRng::seed_from_u64(1379);
        black_box(flat_sampler.sample_measurements_u8(64, &mut flat_rng)?);
        let mut warm_structured_rngs = rngs(repetitions);
        let warm_structured = measure(
            |rep| structured_sampler.sample(shots, &mut warm_structured_rngs[rep]),
            repetitions,
        )?;
        let mut warm_flat_rngs = rngs(repetitions);
        let warm_flat = measure(
            |rep| flat_sampler.sample_measurements_u8(shots, &mut warm_flat_rngs[rep]),
            repetitions,
        )?;
        measurements.push(json!({
            "shots": shots,
            "unprepared": unprepared,
            "cold_prepared_structured": cold,
            "first_prepared_structured": first_structured,
            "first_prepared_flat": first_flat,
            "warm_prepared_structured": warm_structured,
            "warm_prepared_flat": warm_flat,
        }));
    }
    Ok(json!({
        "fixture": name,
        "circuit": text,
        "compile": compile,
        "prepare": prepare,
        "measurements": measurements,
        "peak_rss_bytes": peak_rss_bytes(),
    }))
}

#[cfg(unix)]
fn peak_rss_bytes() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return None;
    }
    let usage = unsafe { usage.assume_init() };
    let rss = u64::try_from(usage.ru_maxrss).ok()?;
    #[cfg(target_os = "macos")]
    {
        Some(rss)
    }
    #[cfg(not(target_os = "macos"))]
    {
        rss.checked_mul(1024)
    }
}

#[cfg(not(unix))]
fn peak_rss_bytes() -> Option<u64> {
    None
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let name = args
        .next()
        .ok_or("usage: near_clifford_matrix_bench FIXTURE [--quick]")?;
    if let Some(rank) = name.strip_prefix("profile_rank_") {
        let seconds: u64 = args
            .next()
            .ok_or("profile_rank_N requires a duration in seconds")?
            .parse()
            .map_err(|_| "profile duration must be an integer")?;
        if seconds == 0 || seconds > 120 {
            return Err("profile duration must be in 1..=120 seconds".into());
        }
        let text = circuit(&format!("rank_{rank}"))?;
        let executor = NearCliffordExecutor::compile_text(&text)?;
        let mut sampler = executor.prepare_sampler()?;
        let mut rng = StdRng::seed_from_u64(739);
        black_box(sampler.sample_measurements_u8(64, &mut rng)?);
        println!("profiling rank {rank}, pid {}", std::process::id());
        let start = Instant::now();
        let mut batches = 0usize;
        while start.elapsed() < Duration::from_secs(seconds) {
            black_box(sampler.sample_measurements_u8(1_000, &mut rng)?);
            batches += 1;
        }
        println!(
            "rank={rank}, batches={batches}, elapsed_s={:.3}",
            start.elapsed().as_secs_f64()
        );
        return Ok(());
    }
    let quick = match args.next().as_deref() {
        None => false,
        Some("--quick") => true,
        _ => return Err("expected optional --quick".into()),
    };
    println!("{}", run(&name, quick)?);
    Ok(())
}
