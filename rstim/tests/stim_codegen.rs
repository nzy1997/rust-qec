// Ported from Stim's circuit_gen_params.test.cc
// Tests: noise parameter application in code generation.
// Avoids overlap with existing codegen_noise.rs tests.

use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::codegen::*;
use rstim::ir::{StimInstr, StimTarget, circuit_to_string};
use rstim::sampler::sample_batch;

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
            StimInstr::Repeat { count, body } => {
                (*count as usize) * count_qubit_targets_named(body, op_name)
            }
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

fn op_name(instr: &StimInstr) -> Option<&str> {
    match instr {
        StimInstr::Op { name, .. } => Some(name.as_str()),
        StimInstr::Repeat { .. } => None,
    }
}

fn op_qubit_target_count(instr: &StimInstr) -> usize {
    match instr {
        StimInstr::Op { targets, .. } => targets
            .iter()
            .filter(|target| matches!(target, StimTarget::Qubit(_) | StimTarget::QubitInv(_)))
            .count(),
        StimInstr::Repeat { .. } => 0,
    }
}

#[test]
fn surface_noise_helpers_ignore_repeat_blocks_for_inline_counts() {
    let repeat = StimInstr::Repeat {
        count: 2,
        body: vec![StimInstr::new(
            "X_ERROR",
            vec![0.125],
            vec![StimTarget::Qubit(7)],
        )],
    };

    assert_eq!(op_name(&repeat), None);
    assert_eq!(op_qubit_target_count(&repeat), 0);
}

fn qubit_targets_in(instrs: &[StimInstr]) -> Vec<u32> {
    let mut qubits = Vec::new();
    for instr in instrs {
        if let StimInstr::Op { targets, .. } = instr {
            qubits.extend(targets.iter().filter_map(StimTarget::qubit_index));
        }
    }
    qubits.sort_unstable();
    qubits
}

fn assert_each_x_error_run_is_immediately_before_measurement(instrs: &[StimInstr]) {
    let mut i = 0;
    let mut runs = 0;
    while i < instrs.len() {
        if op_name(&instrs[i]) != Some("X_ERROR") {
            i += 1;
            continue;
        }

        let start = i;
        while i < instrs.len() && op_name(&instrs[i]) == Some("X_ERROR") {
            i += 1;
        }
        let error_end = i;
        let error_targets: usize = instrs[start..i].iter().map(op_qubit_target_count).sum();
        let error_qubits = qubit_targets_in(&instrs[start..error_end]);

        let Some(measure_name @ ("MR" | "M")) = instrs.get(i).and_then(op_name) else {
            panic!("X_ERROR run should be immediately followed by MR or M");
        };
        let measure_start = i;
        while i < instrs.len() && op_name(&instrs[i]) == Some(measure_name) {
            i += 1;
        }
        let measure_targets: usize = instrs[measure_start..i]
            .iter()
            .map(op_qubit_target_count)
            .sum();
        let measure_qubits = qubit_targets_in(&instrs[measure_start..i]);

        assert_eq!(
            error_targets, measure_targets,
            "X_ERROR run before {measure_name} should cover the same qubits"
        );
        assert_eq!(
            error_qubits, measure_qubits,
            "X_ERROR run before {measure_name} should cover the same qubits"
        );
        runs += 1;
    }
    assert!(runs > 0, "expected at least one before-measure X_ERROR run");
}

fn assert_each_x_error_run_is_immediately_after_reset(instrs: &[StimInstr]) {
    let mut i = 0;
    let mut runs = 0;
    while i < instrs.len() {
        if op_name(&instrs[i]) != Some("X_ERROR") {
            i += 1;
            continue;
        }

        let start = i;
        while i < instrs.len() && op_name(&instrs[i]) == Some("X_ERROR") {
            i += 1;
        }
        let error_targets: usize = instrs[start..i].iter().map(op_qubit_target_count).sum();

        let mut reset_start = start;
        while reset_start > 0 {
            let previous = reset_start - 1;
            if !matches!(op_name(&instrs[previous]), Some("R" | "MR")) {
                break;
            }
            reset_start = previous;
        }
        assert!(
            reset_start < start,
            "X_ERROR run should be immediately after R or MR"
        );
        let reset_name = op_name(&instrs[reset_start]).unwrap();
        assert!(matches!(reset_name, "R" | "MR"));
        let reset_targets: usize = instrs[reset_start..start]
            .iter()
            .map(op_qubit_target_count)
            .sum();
        let error_qubits = qubit_targets_in(&instrs[start..i]);
        let reset_qubits = qubit_targets_in(&instrs[reset_start..start]);

        assert_eq!(
            error_targets, reset_targets,
            "X_ERROR run after {reset_name} should cover the same qubits"
        );
        assert_eq!(
            error_qubits, reset_qubits,
            "X_ERROR run after {reset_name} should cover the same qubits"
        );
        runs += 1;
    }
    assert!(runs > 0, "expected at least one after-reset X_ERROR run");
}

#[test]
fn count_qubit_targets_named_counts_repeat_blocks_recursively() {
    let instrs = vec![
        StimInstr::Repeat {
            count: 3,
            body: vec![
                StimInstr::new(
                    "H",
                    vec![],
                    vec![StimTarget::Qubit(0), StimTarget::QubitInv(1)],
                ),
                StimInstr::new("M", vec![], vec![StimTarget::Qubit(2)]),
            ],
        },
        StimInstr::new("H", vec![], vec![StimTarget::Qubit(3)]),
    ];

    assert_eq!(count_qubit_targets_named(&instrs, "H"), 7);
    assert_eq!(count_qubit_targets_named(&instrs, "M"), 3);
}

// --- NoiseParams::none produces clean circuit ---
#[test]
fn noise_params_none_is_clean() {
    let params = NoiseParams::none();
    assert_eq!(params.before_round_data_depolarization, 0.0);
    assert_eq!(params.after_clifford_depolarization, 0.0);
    assert_eq!(params.before_measure_flip_probability, 0.0);
    assert_eq!(params.after_reset_flip_probability, 0.0);
    assert_eq!(params.after_clifford_loss_probability, 0.0);
}

// --- NoiseParams::uniform sets all channels ---
#[test]
fn noise_params_uniform_sets_all() {
    let params = NoiseParams::uniform(0.05);
    assert_eq!(params.before_round_data_depolarization, 0.05);
    assert_eq!(params.after_clifford_depolarization, 0.05);
    assert_eq!(params.before_measure_flip_probability, 0.05);
    assert_eq!(params.after_reset_flip_probability, 0.05);
    assert_eq!(params.after_clifford_loss_probability, 0.0);
}

// --- rep code with per-channel noise ---
#[test]
fn rep_code_per_channel_noise_all_present() {
    let params = NoiseParams {
        before_round_data_depolarization: 0.011,
        after_clifford_depolarization: 0.022,
        before_measure_flip_probability: 0.033,
        after_reset_flip_probability: 0.044,
        after_clifford_loss_probability: 0.0,
    };
    let circuit = repetition_code_memory_with_params(3, 2, params);
    let text = circuit_to_string(&circuit);
    assert!(text.contains("DEPOLARIZE1(0.011)"), "data depol: {text}");
    assert!(
        text.contains("DEPOLARIZE2(0.022)"),
        "clifford depol: {text}"
    );
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
        after_clifford_loss_probability: 0.0,
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
        after_clifford_loss_probability: 0.0,
    };
    let circuit = rotated_memory_z_with_params(3, 3, params);

    let h_targets = count_qubit_targets_named(&circuit, "H");
    let dep1_targets = count_qubit_targets_named(&circuit, "DEPOLARIZE1");
    let context =
        "after_clifford_depolarization should add one DEPOLARIZE1 target per X-ancilla H target";
    assert_eq!(dep1_targets, h_targets, "{context}");
}

#[test]
fn surface_code_before_round_data_depolarization_does_not_extend_into_tail_measurement_step() {
    let params = NoiseParams {
        before_round_data_depolarization: 0.001,
        after_clifford_depolarization: 0.0,
        before_measure_flip_probability: 0.0,
        after_reset_flip_probability: 0.0,
        after_clifford_loss_probability: 0.0,
    };
    let circuit = rotated_memory_z_with_params(3, 3, params);

    let total_dep1_targets = count_qubit_targets_named(&circuit, "DEPOLARIZE1");
    let final_data_measure_targets = count_qubit_targets_named(&circuit, "M");
    let rounds_context = "before_round_data_depolarization should apply once per round to data qubits, not again in the tail";
    assert_eq!(
        total_dep1_targets,
        3 * final_data_measure_targets,
        "{rounds_context}"
    );

    let tail_dep1_targets =
        count_qubit_targets_named(tail_after_last_tick(&circuit), "DEPOLARIZE1");
    let tail_context =
        "the tail data-measurement step should not inject an extra DEPOLARIZE1 layer";
    assert_eq!(tail_dep1_targets, 0, "{tail_context}");
}

#[test]
fn surface_code_before_measure_flip_covers_ancilla_and_final_data_measurements() {
    let rounds = 3;
    let params = NoiseParams {
        before_round_data_depolarization: 0.0,
        after_clifford_depolarization: 0.0,
        before_measure_flip_probability: 0.001,
        after_reset_flip_probability: 0.0,
        after_clifford_loss_probability: 0.0,
    };
    let circuit = rotated_memory_z_with_params(3, rounds, params);

    let x_error_targets = count_qubit_targets_named(&circuit, "X_ERROR");
    let ancilla_measure_targets = count_qubit_targets_named(&circuit, "MR");
    let final_data_measure_targets = count_qubit_targets_named(tail_after_last_tick(&circuit), "M");

    assert_eq!(
        x_error_targets,
        ancilla_measure_targets + final_data_measure_targets,
        "before_measure_flip_probability should apply before every ancilla MR and before final data M"
    );
    assert_each_x_error_run_is_immediately_before_measurement(&circuit);
}

#[test]
fn surface_code_after_reset_flip_covers_initial_resets_and_ancilla_mr_resets() {
    let rounds = 3;
    let params = NoiseParams {
        before_round_data_depolarization: 0.0,
        after_clifford_depolarization: 0.0,
        before_measure_flip_probability: 0.0,
        after_reset_flip_probability: 0.001,
        after_clifford_loss_probability: 0.0,
    };
    let circuit = rotated_memory_z_with_params(3, rounds, params);

    let x_error_targets = count_qubit_targets_named(&circuit, "X_ERROR");
    let ancilla_mr_targets = count_qubit_targets_named(&circuit, "MR");
    let final_data_measure_targets = count_qubit_targets_named(tail_after_last_tick(&circuit), "M");
    assert_eq!(ancilla_mr_targets % rounds, 0);
    let ancilla_count = ancilla_mr_targets / rounds;

    assert_eq!(
        x_error_targets,
        final_data_measure_targets + ancilla_count + ancilla_mr_targets,
        "after_reset_flip_probability should apply after initial data reset, initial ancilla reset, and each ancilla MR reset"
    );
    assert_each_x_error_run_is_immediately_after_reset(&circuit);
}

#[test]
#[should_panic(expected = "X_ERROR run should be immediately followed by MR or M")]
fn before_measure_helper_rejects_error_run_not_followed_by_measurement() {
    let instrs = vec![StimInstr::new(
        "X_ERROR",
        vec![0.125],
        vec![StimTarget::Qubit(7)],
    )];

    assert_each_x_error_run_is_immediately_before_measurement(&instrs);
}

#[test]
fn issue_memory_z_uniform_noise_contains_all_four_noise_channels() {
    let circuit = rotated_memory_z_with_params(3, 9, NoiseParams::uniform(0.008));
    let text = circuit_to_string(&circuit);

    for needle in [
        "DEPOLARIZE1(0.008)",
        "DEPOLARIZE2(0.008)",
        "X_ERROR(0.008)",
        "MR",
        "OBSERVABLE_INCLUDE(0)",
    ] {
        assert!(text.contains(needle), "missing {needle}: {text}");
    }
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
        after_clifford_loss_probability: 0.0,
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
        after_clifford_loss_probability: 0.0,
    };
    let circuit = repetition_code_memory_with_params(3, 2, params);
    let text = circuit_to_string(&circuit);
    // Only DEPOLARIZE2 should be present, no X_ERROR.
    assert!(
        text.contains("DEPOLARIZE2(0.05)"),
        "should have 2q depol: {text}"
    );
    assert!(!text.contains("X_ERROR"), "no X_ERROR: {text}");
    assert!(!text.contains("DEPOLARIZE1"), "no 1q depol: {text}");
}
