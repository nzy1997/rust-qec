#[path = "support/near_clifford_oracle.rs"]
mod oracle;

use oracle::DenseOracle;
use rand::{RngCore, SeedableRng, rngs::StdRng};
use rstim::near_clifford::NearCliffordExecutor;
use rstim::parser::parse_lines;

const BENCHMARK_CIRCUIT: &str = include_str!("fixtures/near_clifford_batch_20q_8t.stim");
const REPEATED_BENCHMARK_CIRCUIT: &str =
    include_str!("fixtures/near_clifford_batch_20q_8t_repeated.stim");

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

fn assert_prepared_batches_match_individual_runs(
    circuit: &NearCliffordExecutor,
    batches: &[usize],
    sweeps: &[&[bool]],
) {
    let mut sampler = circuit.prepare_sampler().unwrap();
    let mut batch_rng = StdRng::seed_from_u64(739);
    let mut reference_rng = StdRng::seed_from_u64(739);
    for (&shots, &sweep) in batches.iter().zip(sweeps) {
        let batch = sampler
            .sample_with_sweep(shots, sweep, &mut batch_rng)
            .unwrap();
        let reference = (0..shots)
            .map(|_| circuit.run_with_sweep(sweep, &mut reference_rng).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(batch, reference, "shots={shots}, sweep={sweep:?}");
    }
    assert_eq!(batch_rng.next_u64(), reference_rng.next_u64());
}

fn assert_flat_measurements_match_shots(
    circuit: &NearCliffordExecutor,
    batches: &[usize],
    sweeps: &[&[bool]],
) {
    let mut flat_sampler = circuit.prepare_sampler().unwrap();
    let mut shot_sampler = circuit.prepare_sampler().unwrap();
    let mut flat_rng = StdRng::seed_from_u64(739);
    let mut shot_rng = StdRng::seed_from_u64(739);
    for (&shots, &sweep) in batches.iter().zip(sweeps) {
        let flat = flat_sampler
            .sample_measurements_u8_with_sweep(shots, sweep, &mut flat_rng)
            .unwrap();
        let individual = shot_sampler
            .sample_with_sweep(shots, sweep, &mut shot_rng)
            .unwrap();
        assert_eq!(flat.shots, shots);
        assert_eq!(flat.measurements.len(), shots * flat.measurements_per_shot);
        let expected = individual
            .iter()
            .flat_map(|shot| shot.measurements.iter().map(|&bit| u8::from(bit)))
            .collect::<Vec<_>>();
        assert_eq!(flat.measurements, expected);
    }
    assert_eq!(flat_rng.next_u64(), shot_rng.next_u64());
}

#[test]
fn flat_measurement_batches_match_shots_and_rng_across_sampling_paths() {
    let benchmark = NearCliffordExecutor::compile_text(BENCHMARK_CIRCUIT).unwrap();
    let reordered = NearCliffordExecutor::compile_text(
        "H 0\nCX 0 1\nT 0\nH 2\nCX 1 2\nMX 1\nM !0\nMY 2\nDETECTOR rec[-1] rec[-3]",
    )
    .unwrap();
    let in_record_order = NearCliffordExecutor::compile_text("H 0 1 2\nM 0 1 2").unwrap();
    let no_records = NearCliffordExecutor::compile_text("H 0\nT 0").unwrap();
    let no_sweep: &[bool] = &[];
    for circuit in [&benchmark, &reordered, &in_record_order, &no_records] {
        assert_flat_measurements_match_shots(circuit, &[0, 1, 63, 64, 256], &[no_sweep; 5]);
    }

    let feedback =
        NearCliffordExecutor::compile_text("H 0\nT 0\nCX sweep[0] 0\nM 0\nCX rec[-1] 1\nMX 1")
            .unwrap();
    let zero = [false];
    let one = [true];
    assert_flat_measurements_match_shots(&feedback, &[0, 1, 64, 128], &[&zero, &one, &zero, &one]);
}

#[test]
fn flat_and_structured_batches_share_terminal_cache_without_changing_rng() {
    let circuit = NearCliffordExecutor::compile_text(BENCHMARK_CIRCUIT).unwrap();
    let mut mixed = circuit.prepare_sampler().unwrap();
    let mut structured = circuit.prepare_sampler().unwrap();
    let mut mixed_rng = StdRng::seed_from_u64(739);
    let mut structured_rng = StdRng::seed_from_u64(739);

    let flat = mixed.sample_measurements_u8(128, &mut mixed_rng).unwrap();
    let reference = structured.sample(128, &mut structured_rng).unwrap();
    assert_eq!(flat.shots, reference.len());
    assert_eq!(flat.measurements_per_shot, 20);
    assert_eq!(
        flat.measurements,
        reference
            .iter()
            .flat_map(|shot| shot.measurements.iter().map(|&bit| u8::from(bit)))
            .collect::<Vec<_>>()
    );

    assert_eq!(
        mixed.sample(256, &mut mixed_rng).unwrap(),
        structured.sample(256, &mut structured_rng).unwrap()
    );
    assert_eq!(mixed_rng.next_u64(), structured_rng.next_u64());
}

#[test]
fn repeated_terminal_measurements_preserve_shots_records_and_rng() {
    let second_round = (0..20)
        .map(|q| {
            if q % 2 == 0 {
                format!("!{q}")
            } else {
                q.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let repeated = NearCliffordExecutor::compile_text(&format!(
        "{BENCHMARK_CIRCUIT}M {second_round}\nDETECTOR rec[-1] rec[-21]\n"
    ))
    .unwrap();
    let changed_basis = NearCliffordExecutor::compile_text(
        "H 0\nT 0\nH 0\nM !0 1\nM 1 !0\nMX 0\nMX !0\nM 0\nDETECTOR rec[-1] rec[-2]",
    )
    .unwrap();
    let no_sweep: &[bool] = &[];
    for circuit in [&repeated, &changed_basis] {
        assert_matches_individual_runs(circuit, 512, no_sweep);
        assert_flat_measurements_match_shots(circuit, &[0, 1, 64, 512], &[no_sweep; 4]);
    }
    let plain = NearCliffordExecutor::compile_text(REPEATED_BENCHMARK_CIRCUIT).unwrap();
    assert_matches_individual_runs(&plain, 2048, no_sweep);
    let mut rng = StdRng::seed_from_u64(739);
    for shot in plain.sample(512, &mut rng).unwrap() {
        assert_eq!(&shot.measurements[..20], &shot.measurements[20..]);
    }
    let mut rng = StdRng::seed_from_u64(739);
    for shot in repeated.sample(512, &mut rng).unwrap() {
        assert_eq!(shot.detectors, [false]);
        for q in 0..20 {
            assert_eq!(
                shot.measurements[20 + q],
                shot.measurements[q] ^ (q % 2 == 0)
            );
        }
    }

    let new_qubit_after_repeat =
        NearCliffordExecutor::compile_text("H 0 1\nT 0\nM 0\nM 0\nM 1").unwrap();
    assert_matches_individual_runs(&new_qubit_after_repeat, 512, no_sweep);
}

#[test]
fn prepared_sampler_reuses_terminal_cache_across_batches_without_changing_rng() {
    let benchmark = NearCliffordExecutor::compile_text(BENCHMARK_CIRCUIT).unwrap();
    let entangled = NearCliffordExecutor::compile_text(
        "H 0\nCX 0 1\nT 0\nH 2\nCX 1 2\nM !0\nMX 1\nMY 2\nDETECTOR rec[-1] rec[-3]",
    )
    .unwrap();
    for circuit in [&benchmark, &entangled] {
        let batches = [1, 0, 63, 64, 128, 512, 1];
        let no_sweep: &[bool] = &[];
        assert_prepared_batches_match_individual_runs(circuit, &batches, &[no_sweep; 7]);
    }
}

#[test]
fn prepared_sampler_preserves_sweep_and_feedback_across_batches() {
    let circuit =
        NearCliffordExecutor::compile_text("H 0\nT 0\nCX sweep[0] 0\nM 0\nCX rec[-1] 1\nMX 1")
            .unwrap();
    let batches = [64, 2, 128, 0, 65];
    let zero = [false];
    let one = [true];
    assert_prepared_batches_match_individual_runs(
        &circuit,
        &batches,
        &[&zero, &one, &zero, &one, &one],
    );
}

#[test]
fn terminal_batch_matches_individual_runs_and_rng_state() {
    let two_t = NearCliffordExecutor::compile_text("H 0\nCX 0 1\nT 0\nH 0\nT 0\nMX 0 1").unwrap();
    let benchmark = NearCliffordExecutor::compile_text(BENCHMARK_CIRCUIT).unwrap();
    for circuit in [&two_t, &benchmark] {
        for shots in [0, 1, 2, 3, 63, 64, 512] {
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
fn terminal_plan_handles_split_measurements_metadata_and_annotations() {
    for text in [
        "H 0\nCX 0 1\nT 0\nH 0\nM !0\nTICK\nM 1\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(2) rec[-1]\nSHIFT_COORDS(1,2)\n",
        "H 0\nT 0\nMX 0\nMY 1\nM !2\nDETECTOR rec[-1] rec[-3]\n",
        "REPEAT 2 {\nH 0\nT 0\nH 0\n}\nREPEAT 2 {\nM 0\nDETECTOR rec[-1]\nTICK\n}\nOBSERVABLE_INCLUDE(3) rec[-2]",
        "H 0\nT 0\nM 0 64\nDETECTOR rec[-1] rec[-2]",
    ] {
        let circuit = NearCliffordExecutor::compile_text(text).unwrap();
        assert_matches_individual_runs(&circuit, 128, &[]);
    }
}

#[test]
fn symbolic_clifford_suffix_preserves_correlations_and_rng() {
    for text in [
        "H 0\nCX 0 1\nS 1\nM 0\nMX 1\nMY 2\nM 1\nDETECTOR rec[-1] rec[-4]",
        "H 0\nCX 0 1\nCX 1 2\nM 0 1 2\nM 0 1 2",
        "H 0\nT 0\nM 0\nH 1\nCX 1 2\nM 1 2\nMX 2",
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
fn long_terminal_target_lists_preserve_active_and_clifford_sampling() {
    let inactive_targets = vec!["0"; 256].join(" ");
    let inactive =
        NearCliffordExecutor::compile_text(&format!("H 0\nM {inactive_targets}")).unwrap();
    assert_matches_individual_runs(&inactive, 64, &[]);

    let active_targets = vec!["1"; 70].join(" ");
    let active =
        NearCliffordExecutor::compile_text(&format!("H 0\nT 0\nH 0\nM {active_targets}")).unwrap();
    assert_matches_individual_runs(&active, 128, &[]);

    for width in [64, 65, 74, 80, 128, 129, 193] {
        let wide_prefix = (0..width)
            .map(|q| format!("H {q}"))
            .collect::<Vec<_>>()
            .join("\n");
        let wide_targets = (0..width)
            .map(|q| q.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let wide = NearCliffordExecutor::compile_text(&format!("{wide_prefix}\nM {wide_targets}"))
            .unwrap();
        let shots = if width <= 80 { 64 } else { 8 };
        assert_matches_individual_runs(&wide, shots, &[]);
        assert_prepared_batches_match_individual_runs(&wide, &[1, shots], &[&[], &[]]);
    }
}

#[test]
fn wide_symbolic_suffix_preserves_cross_word_correlations_and_rng() {
    let prefix = (0..129)
        .map(|q| format!("H {q}"))
        .collect::<Vec<_>>()
        .join("\n");
    let random_targets = (0..129)
        .map(|q| q.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let circuit = NearCliffordExecutor::compile_text(&format!(
        "{prefix}\nCX 63 129\nCX 64 129\nCX 127 130\nCX 128 130\nM {random_targets} 129 130 63 64 127 128"
    ))
    .unwrap();
    let no_sweep: &[bool] = &[];
    assert_prepared_batches_match_individual_runs(&circuit, &[1, 8], &[no_sweep; 2]);
    assert_flat_measurements_match_shots(&circuit, &[1, 8], &[no_sweep; 2]);
}

#[test]
fn rank_eleven_cache_matches_individual_sampling() {
    let prefix = (0..11)
        .map(|q| format!("H {q}\nT {q}"))
        .collect::<Vec<_>>()
        .join("\n");
    let circuit =
        NearCliffordExecutor::compile_text(&format!("{prefix}\nMX 0 1 2 3 4 5 6 7 8 9 10"))
            .unwrap();
    let no_sweep: &[bool] = &[];
    assert_prepared_batches_match_individual_runs(&circuit, &[1, 8, 32], &[no_sweep; 3]);
    assert_flat_measurements_match_shots(&circuit, &[1, 8, 32], &[no_sweep; 3]);
}

#[test]
fn independent_terminal_measurement_does_not_use_an_active_axis() {
    let circuit =
        NearCliffordExecutor::compile_with_limit(parse_lines("H 0\nH 1\nT 0\nM 1").unwrap(), 1)
            .unwrap();
    assert_matches_individual_runs(&circuit, 64, &[]);
}

#[test]
fn reordered_entangled_mixed_basis_measurements_match_dense_joint_distribution() {
    let circuit = NearCliffordExecutor::compile_text(
        "H 0\nCX 0 1\nT 0\nH 0\nH 2\nCX 1 2\nT_DAG 2\nS 1\nMX 1\nM 0\nMY 2\nDETECTOR rec[-1] rec[-3]",
    )
    .unwrap();
    let no_sweep: &[bool] = &[];
    assert_prepared_batches_match_individual_runs(&circuit, &[1, 63, 256], &[no_sweep; 3]);
    let mut oracle = DenseOracle::new(3);
    oracle.h(0);
    oracle.cx(0, 1);
    oracle.t(0);
    oracle.h(0);
    oracle.h(2);
    oracle.cx(1, 2);
    oracle.t_dag(2);
    oracle.s(1);

    let mut expected = [0.0; 8];
    for (pattern, probability) in expected.iter_mut().enumerate() {
        let mut state = oracle.clone();
        *probability = 1.0;
        for (index, (qubit, basis)) in [(1, 'X'), (0, 'Z'), (2, 'Y')].into_iter().enumerate() {
            let outcome = pattern & (1 << index) != 0;
            let conditional = state.measurement_probability(qubit, basis, outcome);
            *probability *= conditional;
            if conditional > 1e-14 {
                state.collapse(qubit, basis, outcome);
            }
        }
    }

    let mut rng = StdRng::seed_from_u64(739);
    let shots = circuit.sample(10_000, &mut rng).unwrap();
    let mut counts = [0usize; 8];
    for shot in shots {
        let pattern = shot
            .measurements
            .iter()
            .enumerate()
            .fold(0, |pattern, (index, bit)| {
                pattern | (usize::from(*bit) << index)
            });
        counts[pattern] += 1;
        assert_eq!(
            shot.detectors,
            vec![shot.measurements[0] ^ shot.measurements[2]]
        );
    }
    for (count, probability) in counts.into_iter().zip(expected) {
        assert!((count as f64 / 10_000.0 - probability).abs() < 0.025);
    }
}
