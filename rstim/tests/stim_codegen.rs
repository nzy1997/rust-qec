// Ported from Stim's circuit_gen_params.test.cc
// Tests: noise parameter application in code generation.
// Avoids overlap with existing codegen_noise.rs tests.

use rstim::codegen::*;
use rstim::ir::{circuit_to_string, StimInstr, StimTarget};
use rstim::sampler::sample_batch;
use rand::rngs::StdRng;
use rand::SeedableRng;

fn rng() -> StdRng {
    StdRng::seed_from_u64(77)
}

fn count_qubit_targets_named(instrs: &[StimInstr], op_name: &str) -> usize {
    instrs
        .iter()
        .map(|instr| match instr {
            StimInstr::Op { name, targets, .. } if name == op_name => targets
                .iter()
                .filter(|target| matches!(target, StimTarget::Qubit(_) | StimTarget::QubitInv(_)))
                .count(),
            StimInstr::Repeat { count, body } => (*count as usize) * count_qubit_targets_named(body, op_name),
            _ => 0,
        })
        .sum()
}

fn tail_after_last_tick(instrs: &[StimInstr]) -> &[StimInstr] {
    let last_tick = instrs
        .iter()
        .rposition(|instr| matches!(instr, StimInstr::Op { name, .. } if name == "TICK"))
        .expect("surface-code circuit should contain TICK instructions");
    &instrs[last_tick + 1..]
}

// --- NoiseParams::none produces clean circuit ---
#[test]
fn noise_params_none_is_clean() {
    let params = NoiseParams::none();
    assert_eq!(params.before_round_data_depolarization, 0.0);
    assert_eq!(params.after_clifford_depolarization, 0.0);
    assert_eq!(params.before_measure_flip_probability, 0.0);
    assert_eq!(params.after_reset_flip_probability, 0.0);
}

// --- NoiseParams::uniform sets all channels ---
#[test]
fn noise_params_uniform_sets_all() {
    let params = NoiseParams::uniform(0.05);
    assert_eq!(params.before_round_data_depolarization, 0.05);
    assert_eq!(params.after_clifford_depolarization, 0.05);
    assert_eq!(params.before_measure_flip_probability, 0.05);
    assert_eq!(params.after_reset_flip_probability, 0.05);
}

// --- rep code with per-channel noise ---
#[test]
fn rep_code_per_channel_noise_all_present() {
    let params = NoiseParams {
        before_round_data_depolarization: 0.011,
        after_clifford_depolarization: 0.022,
        before_measure_flip_probability: 0.033,
        after_reset_flip_probability: 0.044,
    };
    let circuit = repetition_code_memory_with_params(3, 2, params);
    let text = circuit_to_string(&circuit);
    assert!(text.contains("DEPOLARIZE1(0.011)"), "data depol: {text}");
    assert!(text.contains("DEPOLARIZE2(0.022)"), "clifford depol: {text}");
    assert!(text.contains("X_ERROR(0.033)"), "measure flip: {text}");
    assert!(text.contains("X_ERROR(0.044)"), "reset flip: {text}");
}

// --- rep code with no noise has no error instructions ---
#[test]
fn rep_code_no_noise_no_errors() {
    let circuit = repetition_code_memory_with_params(5, 3, NoiseParams::none());
    let text = circuit_to_string(&circuit);
    assert!(!text.contains("ERROR"), "no error gates: {text}");
    assert!(!text.contains("DEPOLARIZE"), "no depolarize: {text}");
}

// --- rep code legacy API still works ---
#[test]
fn rep_code_legacy_api() {
    let circuit = repetition_code_memory(3, 2, 0.01);
    assert!(!circuit.is_empty(), "should produce instructions");
    let text = circuit_to_string(&circuit);
    assert!(text.contains("M"), "should have measurements: {text}");
}

// --- surface code per-channel noise ---
#[test]
fn surface_code_per_channel_noise_present() {
    let params = NoiseParams {
        before_round_data_depolarization: 0.001,
        after_clifford_depolarization: 0.002,
        before_measure_flip_probability: 0.003,
        after_reset_flip_probability: 0.004,
    };
    let circuit = rotated_memory_z_with_params(3, 3, params);
    let text = circuit_to_string(&circuit);
    assert!(text.contains("DEPOLARIZE2(0.002)"), "2q depol: {text}");
}

#[test]
fn surface_code_after_clifford_depolarization_matches_h_layer_noise_placement() {
    let params = NoiseParams {
        before_round_data_depolarization: 0.0,
        after_clifford_depolarization: 0.001,
        before_measure_flip_probability: 0.0,
        after_reset_flip_probability: 0.0,
    };
    let circuit = rotated_memory_z_with_params(3, 3, params);

    let h_targets = count_qubit_targets_named(&circuit, "H");
    let dep1_targets = count_qubit_targets_named(&circuit, "DEPOLARIZE1");
    assert_eq!(
        dep1_targets,
        h_targets,
        "after_clifford_depolarization should add one DEPOLARIZE1 target per X-ancilla H target"
    );
}

#[test]
fn surface_code_before_round_data_depolarization_does_not_extend_into_tail_measurement_step() {
    let params = NoiseParams {
        before_round_data_depolarization: 0.001,
        after_clifford_depolarization: 0.0,
        before_measure_flip_probability: 0.0,
        after_reset_flip_probability: 0.0,
    };
    let circuit = rotated_memory_z_with_params(3, 3, params);

    let total_dep1_targets = count_qubit_targets_named(&circuit, "DEPOLARIZE1");
    let final_data_measure_targets = count_qubit_targets_named(&circuit, "M");
    assert_eq!(
        total_dep1_targets,
        3 * final_data_measure_targets,
        "before_round_data_depolarization should apply once per round to data qubits, not again in the tail"
    );

    let tail_dep1_targets = count_qubit_targets_named(tail_after_last_tick(&circuit), "DEPOLARIZE1");
    assert_eq!(
        tail_dep1_targets,
        0,
        "the tail data-measurement step should not inject an extra DEPOLARIZE1 layer"
    );
}

// --- surface code no noise ---
#[test]
fn surface_code_no_noise_clean() {
    let circuit = rotated_memory_z_with_params(3, 3, NoiseParams::none());
    let text = circuit_to_string(&circuit);
    assert!(!text.contains("ERROR"), "no error: {text}");
    assert!(!text.contains("DEPOLARIZE"), "no depol: {text}");
}

// --- color code per-channel noise ---
#[test]
fn color_code_per_channel_noise_present() {
    let params = NoiseParams {
        before_round_data_depolarization: 0.01,
        after_clifford_depolarization: 0.02,
        before_measure_flip_probability: 0.03,
        after_reset_flip_probability: 0.04,
    };
    let circuit = memory_xyz_with_params(5, 4, params);
    let text = circuit_to_string(&circuit);
    assert!(text.contains("DEPOLARIZE2(0.02)"), "2q depol: {text}");
}

// --- generated circuit can be sampled ---
#[test]
fn rep_code_circuit_samples_correctly() {
    let circuit = repetition_code_memory_with_params(3, 2, NoiseParams::none());
    let mut r = rng();
    let out = sample_batch(&circuit, 100, &mut r).unwrap();
    // With no noise, no detectors should fire.
    for d in 0..out.detections.num_major() {
        for s in 0..100 {
            assert!(
                !out.detections.get(d, s),
                "det {d} shot {s}: noiseless rep code should have no detections"
            );
        }
    }
}

// --- generated circuit produces DEM ---
#[test]
fn rep_code_circuit_produces_dem() {
    let circuit = repetition_code_memory_with_params(3, 2, NoiseParams::uniform(0.01));
    let dem = rstim::error_analyzer::ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();
    assert!(dem.num_detectors() > 0, "should have detectors");
    assert!(dem.num_observables() > 0, "should have observables");
}

// --- surface code circuit can be sampled ---
#[test]
fn surface_code_circuit_samples() {
    let circuit = rotated_memory_z_with_params(3, 3, NoiseParams::none());
    let mut r = rng();
    let out = sample_batch(&circuit, 10, &mut r).unwrap();
    // Noiseless should have no detections.
    for d in 0..out.detections.num_major() {
        for s in 0..10 {
            assert!(
                !out.detections.get(d, s),
                "det {d} shot {s}: noiseless should have no detections"
            );
        }
    }
}

// --- noise params per-channel independence ---
#[test]
fn noise_params_independent_channels() {
    let params = NoiseParams {
        before_round_data_depolarization: 0.0,
        after_clifford_depolarization: 0.05,
        before_measure_flip_probability: 0.0,
        after_reset_flip_probability: 0.0,
    };
    let circuit = repetition_code_memory_with_params(3, 2, params);
    let text = circuit_to_string(&circuit);
    // Only DEPOLARIZE2 should be present, no X_ERROR.
    assert!(text.contains("DEPOLARIZE2(0.05)"), "should have 2q depol: {text}");
    assert!(!text.contains("X_ERROR"), "no X_ERROR: {text}");
    assert!(!text.contains("DEPOLARIZE1"), "no 1q depol: {text}");
}
