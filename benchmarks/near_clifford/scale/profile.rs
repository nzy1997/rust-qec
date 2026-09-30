//! Untimed CPU-profile driver; takes the exact circuit from the retained campaign.
use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::NearCliffordExecutor;
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

fn main() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let text = std::fs::read_to_string(args.first().ok_or("expected circuit path")?)
        .map_err(|e| e.to_string())?;
    let seconds: u64 = args
        .get(1)
        .ok_or("expected seconds")?
        .parse()
        .map_err(|_| "invalid duration")?;
    if !(1..=120).contains(&seconds) {
        return Err("duration must be 1..=120".into());
    }
    let executor = NearCliffordExecutor::compile_text(&text)?;
    let mut sampler = executor.prepare_sampler()?;
    let mut rng = StdRng::seed_from_u64(739);
    black_box(sampler.sample_measurements_u8(64, &mut rng)?);
    let start = Instant::now();
    let mut batches = 0;
    while start.elapsed() < Duration::from_secs(seconds) {
        black_box(sampler.sample_measurements_u8(64, &mut rng)?);
        batches += 1;
    }
    println!(
        "batches={batches} shots_per_batch=64 elapsed_s={:.3}",
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
