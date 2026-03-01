# Getting Started with rstim

## What is rstim?

rstim is a Rust port of [Stim](https://github.com/quantumlib/Stim), a high-performance
tool for quantum error correction circuit simulation. It provides:

- A stabilizer circuit simulator using the tableau/frame formalism
- Noise channels (depolarizing, bit-flip, etc.)
- Detectors and observables for tracking errors
- Code generators for repetition codes, surface codes, and color codes
- A detector error model (DEM) extractor for use with decoders
- Batch sampling for Monte Carlo experiments

If you are familiar with Stim's Python API, rstim exposes the same concepts
through Rust modules: `parser`, `sampler`, `codegen`, `error_analyzer`, and more.

## Create a circuit and sample

Parse a simple Bell pair circuit and sample measurement outcomes:

```rust
use rstim::parser::parse_lines;
use rstim::sampler::sample_batch;
use rand::SeedableRng;
use rand::rngs::StdRng;

fn main() {
    // Parse a Bell pair circuit
    let circuit = parse_lines("H 0\nCNOT 0 1\nM 0 1").unwrap();

    // Sample 10 shots
    let mut rng = StdRng::seed_from_u64(42);
    let output = sample_batch(&circuit, 10, &mut rng).unwrap();

    // Print results (BitTable is indexed as .get(measurement, shot))
    for shot in 0..10 {
        let m0 = output.measurements.get(0, shot) as u8;
        let m1 = output.measurements.get(1, shot) as u8;
        println!("shot {}: {} {}", shot, m0, m1);
    }
}
```

The two measurement outcomes in each shot will always agree (`00` or `11`),
because H + CNOT creates the Bell state (|00> + |11>) / sqrt(2).

## Detectors

Detectors are parity checks on measurement results. They fire (produce a `1`)
when the parity deviates from its expected noiseless value. Adding noise makes
detectors fire probabilistically:

```rust
let circuit = parse_lines("
    H 0
    CNOT 0 1
    X_ERROR(0.1) 0
    M 0 1
    DETECTOR rec[-1] rec[-2]
").unwrap();

let mut rng = StdRng::seed_from_u64(42);
let output = sample_batch(&circuit, 1000, &mut rng).unwrap();

// output.detections is indexed as .get(detector, shot)
let fires: usize = (0..1000)
    .filter(|&s| output.detections.get(0, s))
    .count();
println!("Detector fired {}/1000 times (~10% expected)", fires);
```

## Generate QEC circuits

rstim includes code generators for standard QEC experiments. These produce
full circuits with noise, detectors, and observable annotations.

```rust
use rstim::codegen::{repetition_code_memory, repetition_code_memory_with_params, NoiseParams};

// Simple: single noise parameter applied uniformly
let circuit = repetition_code_memory(5, 3, 0.01);

// Advanced: per-channel noise control
let params = NoiseParams {
    before_round_data_depolarization: 0.01,
    after_clifford_depolarization: 0.005,
    before_measure_flip_probability: 0.01,
    after_reset_flip_probability: 0.005,
};
let circuit = repetition_code_memory_with_params(5, 3, params);

// Surface code (rotated layout, Z-basis memory experiment)
use rstim::codegen::rotated_memory_z_with_params;
let circuit = rotated_memory_z_with_params(3, 9, params);
```

The `distance` parameter controls code size and the `rounds` parameter controls
how many syndrome extraction rounds are performed.

## Detector Error Model

The error analyzer converts a noisy circuit into a detector error model (DEM).
A DEM describes which error mechanisms can cause which detectors to fire and
which observables to flip, along with their probabilities.

```rust
use rstim::error_analyzer::ErrorAnalyzer;

let dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
println!("{}", dem);
```

For minimum-weight perfect matching (MWPM) decoders, use the decomposed
version, which breaks multi-fault error mechanisms into independent
single-fault components:

```rust
let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
```

## Decode with rmatching

To close the loop and actually correct errors, pair rstim with the `rmatching`
crate (a Rust MWPM decoder). The workflow is:

1. Generate a circuit and extract its DEM.
2. Build a `rmatching::Matching` from the DEM.
3. Sample detection events from the circuit.
4. Decode each shot's syndrome to predict observable flips.
5. Compare predictions against actual observable flips to measure the logical
   error rate.

```rust
// Pseudocode (requires rmatching crate as a dependency)
let circuit = repetition_code_memory(5, 10, 0.01);
let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();

// Build decoder from DEM text
let matching = rmatching::Matching::from_dem(&dem.to_string()).unwrap();

// Sample
let output = sample_batch(&circuit, 1000, &mut rng).unwrap();

// Decode each shot
let mut errors = 0;
for shot in 0..1000 {
    let syndrome: Vec<bool> = (0..output.detections.num_major())
        .map(|d| output.detections.get(d, shot))
        .collect();
    let predicted = matching.decode(&syndrome);
    let actual = output.observable_flips.get(0, shot);
    if predicted[0] != actual {
        errors += 1;
    }
}
println!("Logical error rate: {}/{}", errors, 1000);
```

## Estimate threshold

A threshold experiment sweeps over code distances and noise rates. Below
threshold, larger codes perform better; above threshold, they perform worse.

```rust
for d in [3, 5, 7] {
    for noise in [0.01, 0.02, 0.05, 0.1] {
        let circuit = repetition_code_memory(d, d * 3, noise);
        let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
        // ... build decoder, sample, decode, count errors ...
        let error_rate = errors as f64 / shots as f64;
        println!("d={} p={:.3} error_rate={:.4}", d, noise, error_rate);
    }
}
```

When you plot `error_rate` vs `noise` for each distance, the curves cross at
the threshold noise rate.

## Use rsinter for parallel sampling

For large-scale experiments, the `rsinter` crate parallelizes sampling and
decoding across multiple threads. It also supports adaptive stopping (collect
until you have enough errors for statistical confidence) and CSV
save/resume.

```rust
use rsinter::task::{Task, CollectionOptions};
use rsinter::collect::{collect, CollectOptions};
use rsinter::stats::shot_error_rate_to_piece_error_rate;

let task = Task {
    circuit,
    decoder: "rmatching".into(),
    dem,
    metadata: serde_json::json!({"d": 5, "p": 0.01}),
    collection_options: CollectionOptions {
        max_shots: Some(100_000),
        max_errors: Some(500),
    },
};

let options = CollectOptions {
    num_workers: 8,
    max_shots: None,
    max_errors: None,
    max_batch_size: Some(1024),
    start_batch_size: 256,
    save_resume_filepath: None,
    print_progress: true,
};

let results = collect(vec![task], decoders, &options).unwrap();

// Convert shot error rate to per-round error rate
let per_round_rate = shot_error_rate_to_piece_error_rate(
    results[0].errors as f64 / results[0].shots as f64,
    rounds as f64,
);
println!("Per-round logical error rate: {:.6}", per_round_rate);
```
