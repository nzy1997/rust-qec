//! In-process sampling and b8 packing timings; process startup and disk I/O excluded.
use rand::{SeedableRng, rngs::StdRng};
use rstim::sampler::{SampleOptions, SampleOutputMode, sample_batch_with_options};
use std::time::Instant;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 5, "CIRCUIT SHOTS REPEATS OUT_JSON");
    let shots: usize = args[2].parse().unwrap();
    let repeats: usize = args[3].parse().unwrap();
    let text = std::fs::read_to_string(&args[1]).unwrap();
    let parse_started = Instant::now();
    let circuit = rstim::validation::parse_and_validate(&text).unwrap();
    let parse_seconds = parse_started.elapsed().as_secs_f64();
    let mut records = Vec::new();
    let mut payload = Vec::new();
    for run in 0..repeats + 2 {
        let mut rng = StdRng::seed_from_u64(1700 + run as u64);
        let started = Instant::now();
        let output = sample_batch_with_options(
            &circuit,
            shots,
            &mut rng,
            SampleOptions {
                output_mode: SampleOutputMode::MeasurementsOnly,
                ..Default::default()
            },
        )
        .unwrap();
        let sample_seconds = started.elapsed().as_secs_f64();
        payload.clear();
        let packed = Instant::now();
        rstim::output::append_shots_b8(&output.measurements, &mut payload).unwrap();
        let packing_seconds = packed.elapsed().as_secs_f64();
        if run >= 2 {
            records.push(serde_json::json!({"sample_seconds": sample_seconds, "packing_seconds":packing_seconds,"bytes":payload.len()}));
        }
    }
    std::fs::write(
        &args[4],
        serde_json::to_vec_pretty(&serde_json::json!({
            "backend":"RustQEC auto sampler; loss-visible records", "shots":shots,
            "parse_seconds":parse_seconds,"warmups":2,"records":records
        }))
        .unwrap(),
    )
    .unwrap();
}
