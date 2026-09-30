//! Reproducible, single-process measurements for one near-Clifford fixture.
//! Run via benchmarks/near_clifford/run.py so each fixture gets its own RSS peak.

use rand::{RngCore, SeedableRng, rngs::StdRng};
use rstim::near_clifford::{NearCliffordExecutor, NearCliffordSampler};
use serde_json::{Value, json};
use std::{hint::black_box, time::Instant};

const TERMINAL_20Q: &str = include_str!("fixtures/terminal.stim");
const REPEATED_20Q: &str = include_str!("fixtures/repeated.stim");

fn circuit(name: &str) -> Result<String, String> {
    let parts = name.split('_').collect::<Vec<_>>();
    let number = |index: usize| -> Result<usize, String> {
        parts
            .get(index)
            .ok_or("missing parameter")?
            .parse()
            .map_err(|_| "invalid parameter".into())
    };
    let targets = |n: usize| (0..n).map(|q| q.to_string()).collect::<Vec<_>>().join(" ");
    match parts[0] {
        "terminal" => Ok(TERMINAL_20Q.into()),
        "repeated" => Ok(REPEATED_20Q.into()),
        "records" => Ok(format!("H 0\nM {}\n", vec!["0"; number(1)?].join(" "))),
        "random" => {
            let n = number(1)?;
            Ok(format!("H {}\nM {}\n", targets(n), targets(n)))
        }
        "rank" | "combo" => {
            let rank = number(1)?;
            let width = if parts[0] == "combo" { number(2)? } else { 0 };
            let mut text = String::new();
            for q in 0..rank {
                text.push_str(&format!("H {q}\nT {q}\n"));
            }
            for q in rank..rank + width {
                text.push_str(&format!("H {q}\n"));
            }
            // Wide suffix precedes active measurements to stress retained active states.
            if width > 0 {
                text.push_str(&format!(
                    "M {}\n",
                    (rank..rank + width)
                        .map(|q| q.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            text.push_str(&format!("MX {}\n", targets(rank)));
            Ok(text)
        }
        "interleaved" => {
            let mut text = String::new();
            for _ in 0..number(1)? {
                text.push_str("H 0\nT 0\nMX 0\nR 0\n");
            }
            Ok(text)
        }
        "feedback" => {
            let mut text = String::from("H 0\nT 0\n");
            for _ in 0..number(1)? {
                text.push_str("X_ERROR(0.5) 1\nM 1\nCX rec[-1] 0\nT 0\nH 0\nMR 1\n");
            }
            text.push_str("M 0\n");
            Ok(text)
        }
        "qec" => {
            let mut text = String::from("H 1\nT 1\n");
            for round in 0..number(1)? {
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
        _ => Err(format!("unknown fixture {name}")),
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

fn measure_fresh_executor<T>(
    text: &str,
    repetitions: usize,
    mut operation: impl FnMut(&NearCliffordExecutor, usize) -> Result<T, String>,
) -> Result<Value, String> {
    let mut times = Vec::new();
    for repetition in 0..repetitions {
        let executor = NearCliffordExecutor::compile_text(text)?;
        let start = Instant::now();
        let result = operation(&executor, repetition)?;
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
    let shots = structured.sample(16, &mut structured_rng)?;
    let bytes = flat.sample_measurements_u8(16, &mut flat_rng)?;
    let expected = shots
        .iter()
        .flat_map(|shot| shot.measurements.iter().map(|&bit| u8::from(bit)))
        .collect::<Vec<_>>();
    let continuation = structured_rng.next_u64();
    if bytes.measurements != expected || continuation != flat_rng.next_u64() {
        return Err("flat and structured outputs or RNG continuation differ".into());
    }
    let mut reference_rng = StdRng::seed_from_u64(739);
    let reference = (0..16)
        .map(|_| executor.run(&mut reference_rng))
        .collect::<Result<Vec<_>, _>>()?;
    if shots != reference || continuation != reference_rng.next_u64() {
        return Err("prepared batch differs from individual execution".into());
    }
    Ok(())
}

fn run(name: &str, shots: usize, repetitions: usize) -> Result<Value, String> {
    let text = circuit(name)?;
    let compile = measure(
        |_| NearCliffordExecutor::compile_text(black_box(&text)),
        repetitions,
    )?;
    let executor = NearCliffordExecutor::compile_text(&text)?;
    verify_fixture(&executor)?;
    let mut prepare_times = Vec::new();
    for _ in 0..repetitions {
        let fresh = NearCliffordExecutor::compile_text(&text)?;
        let start = Instant::now();
        let sampler = fresh.prepare_sampler()?;
        prepare_times.push(start.elapsed().as_nanos());
        black_box(sampler);
    }
    let prepare = json!({"median_ns": median(prepare_times.clone()), "raw_ns": prepare_times});
    let counts = [shots];
    let mut measurements = Vec::new();
    for shots in counts {
        // Use independent seeded streams for fresh and retained calls.
        let mut unprepared_rngs = rngs(repetitions);
        let unprepared = measure_fresh_executor(&text, repetitions, |fresh, rep| {
            fresh.sample(shots, &mut unprepared_rngs[rep])
        })?;
        let mut cold_rngs = rngs(repetitions);
        let cold = measure_fresh_executor(&text, repetitions, |fresh, rep| {
            let mut sampler = fresh.prepare_sampler()?;
            sampler.sample(shots, &mut cold_rngs[rep])
        })?;
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
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let name = args.first().ok_or("expected fixture")?;
    let text = circuit(name)?;
    if args.get(1).map(String::as_str) == Some("verify") {
        let executor = NearCliffordExecutor::compile_text(&text)?;
        verify_fixture(&executor)?;
        let mut rng = StdRng::seed_from_u64(739);
        let shots = (0..16)
            .map(|_| executor.run(&mut rng))
            .collect::<Result<Vec<_>, _>>()?;
        println!(
            "{}",
            json!({"fixture":name,"shots":shots.iter().map(|s| json!({"m":s.measurements,"d":s.detectors,"o":s.observables})).collect::<Vec<_>>(),"continuation":rng.next_u64()})
        );
        return Ok(());
    }
    #[cfg(diagnostics)]
    if args.get(1).map(String::as_str) == Some("diagnose") {
        let executor = NearCliffordExecutor::compile_text(&text)?;
        let mut sampler = executor.prepare_sampler()?;
        let initial = sampler.benchmark_snapshot();
        let mut rng = StdRng::seed_from_u64(739);
        black_box(sampler.sample_measurements_u8(64, &mut rng)?);
        let warmup = sampler.benchmark_snapshot();
        let warmup_counters = rstim::near_clifford::benchmark_counters();
        rstim::near_clifford::benchmark_reset_counters();
        black_box(sampler.sample_measurements_u8(256, &mut rng)?);
        println!(
            "{}",
            json!({"fixture":name,"initial":initial,"after_warmup":warmup,"warmup_counters":warmup_counters,"after_probe":sampler.benchmark_snapshot(),"counters":rstim::near_clifford::benchmark_counters()})
        );
        return Ok(());
    }
    let shots = args
        .get(1)
        .ok_or("expected shots")?
        .parse()
        .map_err(|_| "invalid shots")?;
    let repetitions = args
        .get(2)
        .ok_or("expected repetitions")?
        .parse()
        .map_err(|_| "invalid repetitions")?;
    println!("{}", run(name, shots, repetitions)?);
    Ok(())
}
