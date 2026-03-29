use rstim::codegen::bp_flag::{
    LogicalBasis, build_steane_syndrome_circuit, create_surface_code_circuit,
};
use rstim::ir::circuit_to_string;
use rstim::stats;

fn count_lines(text: &str, needle: &str) -> usize {
    text.lines().filter(|line| line.trim() == needle).count()
}

#[test]
fn surface_code_without_flags_matches_small_counts() {
    let circuit = create_surface_code_circuit(3, 0.0, 2, true, false, 0.0);
    assert_eq!(stats::num_qubits(&circuit), 17);
    assert_eq!(stats::num_measurements(&circuit), 25);
    assert_eq!(stats::num_detectors(&circuit), 16);
    assert_eq!(stats::num_observables(&circuit), 1);
}

#[test]
fn surface_code_with_flags_adds_expected_structure() {
    let circuit = create_surface_code_circuit(3, 0.0, 2, true, true, 0.0);
    assert_eq!(stats::num_qubits(&circuit), 25);
    assert_eq!(stats::num_measurements(&circuit), 33);
    assert_eq!(stats::num_detectors(&circuit), 24);
    assert_eq!(stats::num_observables(&circuit), 1);
}

#[test]
fn surface_code_mid_ancilla_noise_targets_only_flagged_checks_when_flags_enabled() {
    let circuit = create_surface_code_circuit(3, 0.0, 1, true, true, 0.2);
    let text = circuit_to_string(&circuit);
    assert_eq!(count_lines(&text, "X_ERROR(0.2) 10"), 1);
    assert_eq!(count_lines(&text, "X_ERROR(0.2) 11"), 1);
    assert_eq!(count_lines(&text, "Z_ERROR(0.2) 13"), 1);
    assert_eq!(count_lines(&text, "Z_ERROR(0.2) 16"), 1);
}

#[test]
fn steane_z_basis_without_flags_matches_small_counts() {
    let circuit = build_steane_syndrome_circuit(0.0, 0.0, 0.0, 0.0, 2, LogicalBasis::Z, false, 0.0);
    assert_eq!(stats::num_qubits(&circuit), 13);
    assert_eq!(stats::num_measurements(&circuit), 19);
    assert_eq!(stats::num_detectors(&circuit), 12);
    assert_eq!(stats::num_observables(&circuit), 1);
}

#[test]
fn steane_x_basis_has_data_hadamards_before_and_after_rounds() {
    let circuit = build_steane_syndrome_circuit(0.0, 0.0, 0.0, 0.0, 2, LogicalBasis::X, false, 0.0);
    let text = circuit_to_string(&circuit);
    assert_eq!(count_lines(&text, "H 0 1 2 3 4 5 6"), 2);
    assert_eq!(stats::num_detectors(&circuit), 12);
}

#[test]
fn steane_flags_and_mid_ancilla_noise_add_expected_structure() {
    let circuit = build_steane_syndrome_circuit(0.0, 0.0, 0.0, 0.0, 2, LogicalBasis::Z, true, 0.2);
    let text = circuit_to_string(&circuit);
    assert_eq!(stats::num_qubits(&circuit), 19);
    assert_eq!(stats::num_measurements(&circuit), 31);
    assert_eq!(stats::num_detectors(&circuit), 24);
    assert_eq!(count_lines(&text, "Z_ERROR(0.2) 7"), 2);
    assert_eq!(count_lines(&text, "Z_ERROR(0.2) 8"), 2);
    assert_eq!(count_lines(&text, "Z_ERROR(0.2) 9"), 2);
    assert_eq!(count_lines(&text, "X_ERROR(0.2) 10"), 2);
    assert_eq!(count_lines(&text, "X_ERROR(0.2) 11"), 2);
    assert_eq!(count_lines(&text, "X_ERROR(0.2) 12"), 2);
}
