#![cfg(debug_assertions)]

use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use rstim::compiled::compile_circuit;
use rstim::executor::reference_sample;
use rstim::parser::parse_lines;
use rstim::rare_error_iterator::{rare_error_telemetry, reset_rare_error_telemetry};
use rstim::sim::frame::{
    depolarize2_branch_label_for_test, depolarize2_decode_event_for_test,
    depolarize2_sampling_telemetry, reset_depolarize2_sampling_telemetry,
    reset_frame_noise_telemetry, sample_depolarize2_branch_index_for_test,
    take_frame_noise_telemetry, FrameSimulator,
};

struct ScriptedRng {
    draws: Vec<u64>,
    next: usize,
}

impl ScriptedRng {
    fn from_u64s(draws: Vec<u64>) -> Self {
        Self { draws, next: 0 }
    }
}

impl RngCore for ScriptedRng {
    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }

    fn next_u64(&mut self) -> u64 {
        let value = self.draws.get(self.next).copied().unwrap_or(0);
        self.next += 1;
        value
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(8) {
            let bytes = self.next_u64().to_ne_bytes();
            let len = chunk.len();
            chunk.copy_from_slice(&bytes[..len]);
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

fn many_depolarize2_pairs(pair_count: usize, probability: f64) -> String {
    let mut program = format!("DEPOLARIZE2({probability})");
    for pair in 0..pair_count {
        let a = 2 * pair;
        let b = a + 1;
        program.push_str(&format!(" {a} {b}"));
    }
    program.push('\n');
    program
}

fn reset_depolarize2_test_telemetry() {
    reset_depolarize2_sampling_telemetry();
    reset_rare_error_telemetry();
}

fn run_interpreted(program: &str, num_qubits: usize, shots: usize, seed: u64) {
    let instrs = parse_lines(program).unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(seed);
    let mut frame = FrameSimulator::new(num_qubits, shots);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
}

fn run_compiled(program: &str, shots: usize, seed: u64) {
    let instrs = parse_lines(program).unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let compiled = compile_circuit(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(seed);
    let mut frame = FrameSimulator::new(compiled.num_qubits, shots);
    frame
        .run_compiled_blocks(&compiled.blocks, &ref_sample, &mut rng)
        .unwrap();
}

#[test]
fn decode_helper_is_pair_major_at_boundaries() {
    let shots = 1024;
    let cases = [
        (0, (0, 0)),
        (shots - 1, (0, shots - 1)),
        (shots, (1, 0)),
        (2 * shots - 1, (1, shots - 1)),
        (109 * shots, (109, 0)),
        (110 * shots - 1, (109, shots - 1)),
    ];

    for (event_index, expected) in cases {
        assert_eq!(
            depolarize2_decode_event_for_test(event_index, shots),
            expected,
            "event_index={event_index}"
        );
    }
}

#[test]
fn branch_indices_map_to_non_identity_paulis_in_order() {
    let labels: Vec<&'static str> = (0..15)
        .map(|branch| depolarize2_branch_label_for_test(branch).unwrap())
        .collect();
    assert_eq!(
        labels,
        vec![
            "IX", "IY", "IZ", "XI", "XX", "XY", "XZ", "YI", "YX", "YY", "YZ", "ZI", "ZX", "ZY",
            "ZZ",
        ]
    );
    assert_eq!(depolarize2_branch_label_for_test(15), None);
}

#[test]
fn seeded_branch_draws_are_uniform_and_never_ii() {
    let mut rng = StdRng::seed_from_u64(123);
    let mut counts = [0usize; 15];
    for _ in 0..1_500_000 {
        let branch = sample_depolarize2_branch_index_for_test(&mut rng);
        let label = depolarize2_branch_label_for_test(branch).unwrap();
        assert_ne!(label, "II");
        counts[branch] += 1;
    }

    for (branch, count) in counts.into_iter().enumerate() {
        assert!(
            (98_000..=102_000).contains(&count),
            "branch {branch} count {count} outside 98,000-102,000"
        );
    }
}

#[test]
fn scripted_branch_zero_applies_ix_not_ii() {
    let instrs = parse_lines("DEPOLARIZE2(0.001) 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = ScriptedRng::from_u64s(vec![u64::MAX, 0]);
    let mut frame = FrameSimulator::new(2, 1);

    reset_depolarize2_test_telemetry();
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let measurements = frame.measurements(&ref_sample);

    assert_eq!(
        measurements.row_words(0),
        &[0],
        "IX leaves qubit 0 unchanged"
    );
    assert_eq!(measurements.row_words(1), &[1], "IX flips qubit 1");
    let telemetry = depolarize2_sampling_telemetry();
    assert_eq!(telemetry.sampling_path, "sparse");
    assert_eq!(telemetry.iterator_builds, 1);
    assert_eq!(telemetry.attempt_count, 1);
    assert_eq!(rare_error_telemetry().iterator_builds, 1);
}

#[test]
fn sparse_interpreted_depolarize2_uses_one_instruction_wide_iterator() {
    let pair_count = 110;
    let shots = 1024;
    let program = many_depolarize2_pairs(pair_count, 0.001);

    reset_depolarize2_test_telemetry();
    run_interpreted(&program, pair_count * 2, shots, 462);

    let telemetry = depolarize2_sampling_telemetry();
    assert_eq!(telemetry.sampling_path, "sparse");
    assert_eq!(telemetry.iterator_builds, 1);
    assert_eq!(telemetry.attempt_count, pair_count * shots);
    assert_eq!(rare_error_telemetry().iterator_builds, 1);
}

#[test]
fn sparse_compiled_depolarize2_uses_one_instruction_wide_iterator() {
    let pair_count = 110;
    let shots = 1024;
    let program = many_depolarize2_pairs(pair_count, 0.001);

    reset_depolarize2_test_telemetry();
    run_compiled(&program, shots, 462);

    let telemetry = depolarize2_sampling_telemetry();
    assert_eq!(telemetry.sampling_path, "sparse");
    assert_eq!(telemetry.iterator_builds, 1);
    assert_eq!(telemetry.attempt_count, pair_count * shots);
    assert_eq!(rare_error_telemetry().iterator_builds, 1);
}

#[test]
fn dense_probability_keeps_dense_fallback() {
    let pair_count = 3;
    let shots = 1024;
    let program = many_depolarize2_pairs(pair_count, 0.3);

    reset_depolarize2_test_telemetry();
    run_interpreted(&program, pair_count * 2, shots, 463);

    let telemetry = depolarize2_sampling_telemetry();
    assert_eq!(telemetry.sampling_path, "dense");
    assert_eq!(telemetry.iterator_builds, 0);
    assert_eq!(telemetry.attempt_count, pair_count * shots);
    assert_eq!(rare_error_telemetry().iterator_builds, 0);
}

#[test]
fn sparse_threshold_probability_uses_instruction_wide_iterator() {
    let pair_count = 5;
    let shots = 257;
    let program = many_depolarize2_pairs(pair_count, 0.02);

    reset_depolarize2_test_telemetry();
    run_interpreted(&program, pair_count * 2, shots, 464);

    let telemetry = depolarize2_sampling_telemetry();
    assert_eq!(telemetry.sampling_path, "sparse");
    assert_eq!(telemetry.iterator_builds, 1);
    assert_eq!(telemetry.attempt_count, pair_count * shots);
    assert_eq!(rare_error_telemetry().iterator_builds, 1);
}

#[test]
fn probability_above_sparse_threshold_uses_dense_fallback() {
    let pair_count = 5;
    let shots = 257;
    let program = many_depolarize2_pairs(pair_count, 0.0200001);

    reset_depolarize2_test_telemetry();
    run_interpreted(&program, pair_count * 2, shots, 465);

    let telemetry = depolarize2_sampling_telemetry();
    assert_eq!(telemetry.sampling_path, "dense");
    assert_eq!(telemetry.iterator_builds, 0);
    assert_eq!(telemetry.attempt_count, pair_count * shots);
    assert_eq!(rare_error_telemetry().iterator_builds, 0);
}

#[test]
fn frame_noise_telemetry_accumulates_depolarize2_operations() {
    let pair_count = 3;
    let shots = 17;
    let sparse_program = many_depolarize2_pairs(pair_count, 0.001);
    let dense_program = many_depolarize2_pairs(pair_count, 0.3);
    let program = format!("{sparse_program}{dense_program}M 0 1\n");
    let expected_attempt_count = pair_count * shots;

    for backend in ["interpreted", "compiled"] {
        reset_frame_noise_telemetry();
        match backend {
            "interpreted" => run_interpreted(&program, pair_count * 2, shots, 463),
            "compiled" => run_compiled(&program, shots, 463),
            _ => unreachable!(),
        }

        let telemetry = take_frame_noise_telemetry();
        assert_eq!(telemetry.len(), 2, "{backend}");
        assert_eq!(telemetry[0].operation, "DEPOLARIZE2", "{backend}");
        assert_eq!(telemetry[0].sampling_path, "sparse", "{backend}");
        assert_eq!(telemetry[0].targets, None, "{backend}");
        assert_eq!(telemetry[0].pairs, Some(pair_count), "{backend}");
        assert_eq!(telemetry[0].iterator_builds, 1, "{backend}");
        assert_eq!(
            telemetry[0].attempt_count, expected_attempt_count,
            "{backend}"
        );
        assert_eq!(telemetry[1].operation, "DEPOLARIZE2", "{backend}");
        assert_eq!(telemetry[1].sampling_path, "dense", "{backend}");
        assert_eq!(telemetry[1].targets, None, "{backend}");
        assert_eq!(telemetry[1].pairs, Some(pair_count), "{backend}");
        assert_eq!(telemetry[1].iterator_builds, 0, "{backend}");
        assert_eq!(
            telemetry[1].attempt_count, expected_attempt_count,
            "{backend}"
        );
    }
}
