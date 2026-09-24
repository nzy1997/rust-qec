use rand::{RngCore, SeedableRng, rngs::StdRng};
use rstim::near_clifford::NearCliffordExecutor;
use rstim::parser::parse_lines;

const BENCHMARK_CIRCUIT: &str = include_str!("fixtures/near_clifford_batch_20q_8t.stim");

fn assert_matches_individual_runs(circuit: &NearCliffordExecutor, shots: usize, sweep: &[bool]) {
    let mut batch_rng = StdRng::seed_from_u64(739);
    let mut reference_rng = StdRng::seed_from_u64(739);
    let batch = circuit
        .sample_with_sweep(shots, sweep, &mut batch_rng)
        .unwrap();
    let reference = (0..shots)
        .map(|_| circuit.run_with_sweep(sweep, &mut reference_rng).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(batch, reference);
    assert_eq!(batch_rng.next_u64(), reference_rng.next_u64());
}

#[test]
fn terminal_batch_matches_individual_runs_and_rng_state() {
    let two_t = NearCliffordExecutor::compile_text("H 0\nCX 0 1\nT 0\nH 0\nT 0\nMX 0 1").unwrap();
    let benchmark = NearCliffordExecutor::compile_text(BENCHMARK_CIRCUIT).unwrap();
    for circuit in [&two_t, &benchmark] {
        for shots in [0, 1, 512] {
            assert_matches_individual_runs(circuit, shots, &[]);
        }
    }

    let mut rng = StdRng::seed_from_u64(739);
    let batch = two_t.sample(4096, &mut rng).unwrap();
    let even = batch
        .iter()
        .filter(|shot| shot.measurements[0] == shot.measurements[1])
        .count();
    assert!((even as f64 / 4096.0 - 0.75).abs() < 0.03);
}

#[test]
fn stochastic_feedback_and_sweep_keep_per_shot_behavior() {
    let stochastic = NearCliffordExecutor::compile_text(
        "H 0\nT 0\nX_ERROR(0.5) 1\nM 1\nCX rec[-1] 0\nT 0\nH 0\nM 0",
    )
    .unwrap();
    for shots in [0, 1, 512] {
        assert_matches_individual_runs(&stochastic, shots, &[]);
    }
    let mut rng = StdRng::seed_from_u64(739);
    let batch = stochastic.sample(512, &mut rng).unwrap();
    assert!(batch.iter().any(|shot| shot.measurements[0]));
    assert!(batch.iter().any(|shot| !shot.measurements[0]));

    let swept = NearCliffordExecutor::compile_text("H 0\nT 0\nCX sweep[0] 0\nMX 0").unwrap();
    for shots in [0, 1, 512] {
        assert_matches_individual_runs(&swept, shots, &[false]);
        assert_matches_individual_runs(&swept, shots, &[true]);
    }
}

#[test]
fn terminal_cache_handles_inverted_targets_and_other_measurement_bases() {
    for text in [
        "H 0\nT 0\nM !0 1",
        "H 0\nT 0\nMX !0 1",
        "H 0\nT 0\nMY !0 1",
        "H 0\nREPEAT 2 {\nT 0\n}\nM 0",
    ] {
        let circuit = NearCliffordExecutor::compile_text(text).unwrap();
        assert_matches_individual_runs(&circuit, 512, &[]);
    }
}

#[test]
fn terminal_cache_budget_preserves_long_batch_and_rng_state() {
    let benchmark = NearCliffordExecutor::compile_text(BENCHMARK_CIRCUIT).unwrap();
    assert_matches_individual_runs(&benchmark, 2048, &[]);
}

#[test]
fn terminal_rank_limit_error_does_not_advance_rng() {
    let circuit =
        NearCliffordExecutor::compile_with_limit(parse_lines("H 0\nH 1\nT 0\nM 1").unwrap(), 1)
            .unwrap();
    let mut batch_rng = StdRng::seed_from_u64(739);
    let mut run_rng = StdRng::seed_from_u64(739);
    let batch_error = circuit.sample(64, &mut batch_rng).unwrap_err();
    let run_error = circuit.run(&mut run_rng).unwrap_err();
    assert_eq!(batch_error, run_error);
    assert!(batch_error.contains("rank 2"), "{batch_error}");
    assert_eq!(batch_rng.next_u64(), run_rng.next_u64());
}
