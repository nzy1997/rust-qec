//! Reproduce issue #746's same-process batch-versus-run-loop timing.
//! Run: cargo run --release -p rstim --example near_clifford_batch_bench

use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::NearCliffordExecutor;
use std::{hint::black_box, time::Instant};

const CIRCUIT: &str = include_str!("../tests/fixtures/near_clifford_batch_20q_8t.stim");

fn measure(circuit: &NearCliffordExecutor, shots: usize, batch: bool, seed: u64) -> u128 {
    let mut rng = StdRng::seed_from_u64(seed);
    let start = Instant::now();
    let result = if batch {
        circuit.sample(shots, &mut rng).unwrap()
    } else {
        (0..shots)
            .map(|_| circuit.run(&mut rng).unwrap())
            .collect::<Vec<_>>()
    };
    let elapsed = start.elapsed().as_nanos();
    black_box(result);
    elapsed
}

fn median(values: &[u128]) -> u128 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

fn main() {
    let circuit = NearCliffordExecutor::compile_text(CIRCUIT).unwrap();
    for warmup in 0..5 {
        measure(&circuit, 1000, true, warmup);
        measure(&circuit, 1000, false, warmup);
    }
    let mut batch_ns = Vec::new();
    let mut loop_ns = Vec::new();
    let mut single_ns = Vec::new();
    for repetition in 0..9 {
        let seed = 739 + repetition;
        if repetition % 2 == 0 {
            batch_ns.push(measure(&circuit, 1000, true, seed));
            loop_ns.push(measure(&circuit, 1000, false, seed));
        } else {
            loop_ns.push(measure(&circuit, 1000, false, seed));
            batch_ns.push(measure(&circuit, 1000, true, seed));
        }
        single_ns.push(measure(&circuit, 1, true, seed));
    }
    let ratio = median(&batch_ns) as f64 / median(&loop_ns) as f64;
    println!("fixture=20q_8t shots=1000 warmups=5 repetitions=9");
    println!("batch_ns={batch_ns:?}");
    println!("run_loop_ns={loop_ns:?}");
    println!("single_shot_ns={single_ns:?}");
    println!(
        "median_batch_ms={:.3} median_run_loop_ms={:.3} ratio={ratio:.3} median_single_ms={:.3}",
        median(&batch_ns) as f64 / 1e6,
        median(&loop_ns) as f64 / 1e6,
        median(&single_ns) as f64 / 1e6,
    );
}
