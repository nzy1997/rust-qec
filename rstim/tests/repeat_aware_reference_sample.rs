use rstim::data_path::{
    build_reference_sample_with_decision, ReferenceBuildPhaseCounters, ReferenceSampleDecision,
};
use rstim::ir::StimInstr;
use rstim::parser::parse_lines;
use sha2::{Digest, Sha256};

const SURFACE_D11_R100: &str = include_str!(
    "../../benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
);
const SURFACE_DIGEST: &str = "d95f3eacd05c1ca0d3a90e4a48e1d68b7ef5f2d817da11121ba4b77454b24d3d";

fn parse_circuit(source: &str) -> Vec<StimInstr> {
    parse_lines(source).expect("test circuit parses")
}

fn build(source: &str) -> (Vec<bool>, ReferenceBuildPhaseCounters) {
    let result = build_reference_sample_with_decision(&parse_circuit(source))
        .expect("reference sample builds");
    assert_eq!(result.decision, ReferenceSampleDecision::PackedInverse);
    (result.bits, result.phase_counters)
}

fn pack_b8(bits: &[bool]) -> Vec<u8> {
    let mut packed = vec![0_u8; bits.len().div_ceil(8)];
    for (index, bit) in bits.iter().enumerate() {
        if *bit {
            packed[index / 8] |= 1 << (index % 8);
        }
    }
    packed
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn period_one_repeat_executes_once_and_skips_rest() {
    let (bits, counters) = build("REPEAT 12 {\n  M 0\n}\n");
    assert_eq!(bits, vec![false; 12]);
    assert_eq!(counters.measurement_reset_batches, 1);
    assert_eq!(counters.expanded_repeat_iterations, 12);
    assert_eq!(counters.executed_repeat_iterations, 1);
    assert_eq!(counters.skipped_repeat_iterations, 11);
}

#[test]
fn period_two_repeat_stores_period_output_before_skipping() {
    let (bits, counters) = build("REPEAT 12 {\n  X 0\n  M 0\n}\n");
    let expected: Vec<bool> = (0..12).map(|index| index % 2 == 0).collect();
    assert_eq!(bits, expected);
    assert_eq!(counters.measurement_reset_batches, 2);
    assert_eq!(counters.expanded_repeat_iterations, 12);
    assert_eq!(counters.executed_repeat_iterations, 2);
    assert_eq!(counters.skipped_repeat_iterations, 10);
}

#[test]
fn short_repeats_below_ten_execute_normally() {
    let (bits, counters) = build("REPEAT 9 {\n  M 0\n}\n");
    assert_eq!(bits, vec![false; 9]);
    assert_eq!(counters.measurement_reset_batches, 9);
    assert_eq!(counters.expanded_repeat_iterations, 9);
    assert_eq!(counters.executed_repeat_iterations, 9);
    assert_eq!(counters.skipped_repeat_iterations, 0);
}

#[test]
fn nested_long_repeats_fold_recursively_inside_short_parent() {
    let (bits, counters) = build("REPEAT 2 {\n  REPEAT 12 {\n    M 0\n  }\n}\n");
    assert_eq!(bits, vec![false; 24]);
    assert_eq!(counters.measurement_reset_batches, 2);
    assert_eq!(counters.expanded_repeat_iterations, 26);
    assert_eq!(counters.executed_repeat_iterations, 4);
    assert_eq!(counters.skipped_repeat_iterations, 22);
}

#[test]
fn folded_outer_repeat_preserves_nested_logical_repeat_counters() {
    let (bits, counters) = build("REPEAT 12 {\n  REPEAT 12 {\n    M 0\n  }\n}\n");
    assert_eq!(bits, vec![false; 144]);
    assert_eq!(counters.measurement_reset_batches, 1);
    assert_eq!(counters.expanded_repeat_iterations, 156);
    assert_eq!(counters.executed_repeat_iterations, 2);
    assert_eq!(counters.skipped_repeat_iterations, 154);
}

#[test]
fn state_alternating_empty_period_is_not_folded_by_bits_only() {
    let (bits, counters) = build("REPEAT 99 {\n  X 0\n}\nM 0\n");
    assert_eq!(bits, vec![true]);
    assert_eq!(counters.measurement_reset_batches, 1);
    assert_eq!(counters.expanded_repeat_iterations, 99);
    assert_eq!(counters.executed_repeat_iterations, 3);
    assert_eq!(counters.skipped_repeat_iterations, 96);
}

#[test]
fn surface_fixture_skips_periodic_reference_rounds_and_preserves_digest() {
    let result = build_reference_sample_with_decision(&parse_circuit(SURFACE_D11_R100))
        .expect("surface reference sample builds");
    assert_eq!(result.decision, ReferenceSampleDecision::PackedInverse);
    assert_eq!(result.bits.len(), 12_121);
    assert!(result.bits.iter().all(|bit| !*bit));
    assert_eq!(sha256_hex(&pack_b8(&result.bits)), SURFACE_DIGEST);

    let counters = result.phase_counters;
    assert_eq!(counters.measurement_reset_batches, 5);
    assert_eq!(counters.canonical_materializations, 0);
    assert_eq!(counters.canonical_writebacks, 0);
    assert_eq!(counters.direct_inverse_batches, 5);
    assert_eq!(counters.transposed_collapse_batches, 2);
    assert_eq!(counters.collapse_pivots, 120);
    assert_eq!(counters.expanded_repeat_iterations, 99);
    assert_eq!(counters.executed_repeat_iterations, 1);
    assert_eq!(counters.skipped_repeat_iterations, 98);
    assert_eq!(counters.measurement_bits, 12_121);
}
