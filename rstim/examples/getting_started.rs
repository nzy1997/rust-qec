use rstim::codegen::{repetition_code_memory_with_params, NoiseParams};
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::sampler::sample_batch;
use rstim::parser::parse_lines;
use rand::SeedableRng;
use rand::rngs::StdRng;

fn main() {
    let mut rng = StdRng::seed_from_u64(42);

    // 1. Parse and sample a simple Bell pair circuit
    let circuit = parse_lines("H 0\nCNOT 0 1\nM 0 1").unwrap();
    let output = sample_batch(&circuit, 10, &mut rng).unwrap();
    println!("=== Bell pair ===");
    println!(
        "{} measurements, {} shots",
        output.measurements.num_major(),
        output.measurements.num_minor()
    );
    for shot in 0..10 {
        let m0 = output.measurements.get(0, shot) as u8;
        let m1 = output.measurements.get(1, shot) as u8;
        println!("  shot {}: {} {}", shot, m0, m1);
    }

    // 2. Generate a repetition code circuit with per-channel noise
    let params = NoiseParams {
        before_round_data_depolarization: 0.01,
        after_clifford_depolarization: 0.02,
        before_measure_flip_probability: 0.01,
        after_reset_flip_probability: 0.01,
    };
    let circuit = repetition_code_memory_with_params(5, 3, params);
    println!("\n=== Repetition code (d=5, r=3) ===");
    println!("{} instructions", circuit.len());

    // 3. Extract a detector error model (DEM)
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
    println!(
        "DEM: {} detectors, {} observables",
        dem.num_detectors(),
        dem.num_observables()
    );

    // 4. Sample detections and count detection events
    let output = sample_batch(&circuit, 100, &mut rng).unwrap();
    let det_fires: usize = (0..100)
        .map(|s| {
            (0..output.detections.num_major())
                .filter(|&d| output.detections.get(d, s))
                .count()
        })
        .sum();
    println!("Detection events in 100 shots: {}", det_fires);
}
