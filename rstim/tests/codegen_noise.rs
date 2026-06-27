use rstim::codegen::*;
use rstim::ir::circuit_to_string;

#[test]
fn noise_params_uniform() {
    let p = NoiseParams::uniform(0.01);
    assert_eq!(p.before_round_data_depolarization, 0.01);
    assert_eq!(p.after_clifford_depolarization, 0.01);
    assert_eq!(p.after_clifford_loss_probability, 0.0);
}

#[test]
fn rep_code_per_channel_noise() {
    let params = NoiseParams {
        before_round_data_depolarization: 0.01,
        after_clifford_depolarization: 0.02,
        before_measure_flip_probability: 0.03,
        after_reset_flip_probability: 0.04,
        after_clifford_loss_probability: 0.0,
    };
    let circuit = repetition_code_memory_with_params(3, 2, params);
    let text = circuit_to_string(&circuit);
    assert!(text.contains("DEPOLARIZE1(0.01)"), "data depol: {}", text);
    assert!(
        text.contains("DEPOLARIZE2(0.02)"),
        "clifford depol: {}",
        text
    );
    assert!(text.contains("X_ERROR(0.03)"), "measure flip: {}", text);
    assert!(text.contains("X_ERROR(0.04)"), "reset flip: {}", text);
}

#[test]
fn rep_code_no_noise_clean() {
    let circuit = repetition_code_memory_with_params(3, 2, NoiseParams::none());
    let text = circuit_to_string(&circuit);
    assert!(!text.contains("ERROR"), "no noise: {}", text);
    assert!(!text.contains("DEPOLARIZE"), "no depol: {}", text);
}

#[test]
fn rep_code_legacy_still_works() {
    let circuit = repetition_code_memory(5, 3, 0.01);
    assert!(!circuit.is_empty());
}

#[test]
fn surface_code_per_channel_noise() {
    let params = NoiseParams {
        before_round_data_depolarization: 0.001,
        after_clifford_depolarization: 0.002,
        before_measure_flip_probability: 0.003,
        after_reset_flip_probability: 0.004,
        after_clifford_loss_probability: 0.0,
    };
    let circuit = rotated_memory_z_with_params(3, 3, params);
    let text = circuit_to_string(&circuit);
    assert!(text.contains("DEPOLARIZE2(0.002)"), "2q depol: {}", text);
}

#[test]
fn color_code_per_channel_noise() {
    let params = NoiseParams {
        before_round_data_depolarization: 0.01,
        after_clifford_depolarization: 0.02,
        before_measure_flip_probability: 0.03,
        after_reset_flip_probability: 0.04,
        after_clifford_loss_probability: 0.0,
    };
    let circuit = memory_xyz_with_params(5, 4, params);
    let text = circuit_to_string(&circuit);
    assert!(text.contains("DEPOLARIZE2(0.02)"), "2q depol: {}", text);
}
