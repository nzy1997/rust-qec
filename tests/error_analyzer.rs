use rstim::parser::parse_lines;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::dem::{DemTarget, DetectorErrorModel};

fn circuit_to_dem(circuit_str: &str) -> DetectorErrorModel {
    let instrs = parse_lines(circuit_str).unwrap();
    ErrorAnalyzer::circuit_to_dem(&instrs).unwrap()
}

fn assert_has_error(dem: &DetectorErrorModel, prob: f64, expected_targets: &[DemTarget]) {
    let found = dem.instructions().iter().any(|instr| {
        if let rstim::dem::DemInstruction::Error { probability, targets } = instr {
            (*probability - prob).abs() < 1e-12 && targets == expected_targets
        } else {
            false
        }
    });
    assert!(found, "expected error({prob}) {:?} but not found in:\n{dem}", expected_targets);
}

fn error_count(dem: &DetectorErrorModel) -> usize {
    dem.instructions().iter().filter(|i| matches!(i, rstim::dem::DemInstruction::Error { .. })).count()
}

#[test]
fn empty_circuit_no_errors() {
    let dem = circuit_to_dem("");
    assert_eq!(error_count(&dem), 0);
}

#[test]
fn x_error_before_mz_detector() {
    let dem = circuit_to_dem(
        "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn z_error_before_mz_no_detection() {
    let dem = circuit_to_dem(
        "R 0\nZ_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 0);
}

#[test]
fn z_error_before_mx_detector() {
    let dem = circuit_to_dem(
        "RX 0\nZ_ERROR(0.1) 0\nMX 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn x_error_before_mx_no_detection() {
    let dem = circuit_to_dem(
        "RX 0\nX_ERROR(0.1) 0\nMX 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 0);
}

#[test]
fn y_error_flips_both_detectors() {
    let dem = circuit_to_dem(
        "R 0\nY_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn cx_propagates_x_sensitivity() {
    let dem = circuit_to_dem(
        "R 0 1\nX_ERROR(0.1) 0\nCX 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]"
    );
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0), DemTarget::Detector(1)]);
}

#[test]
fn cx_propagates_z_sensitivity() {
    let dem = circuit_to_dem(
        "RX 0 1\nZ_ERROR(0.1) 1\nCX 0 1\nMX 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]"
    );
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0), DemTarget::Detector(1)]);
}

#[test]
fn h_gate_swaps_x_z_sensitivity() {
    let dem = circuit_to_dem(
        "R 0\nZ_ERROR(0.1) 0\nH 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn s_gate_x_picks_up_z() {
    let dem = circuit_to_dem(
        "R 0\nX_ERROR(0.1) 0\nS 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn observable_include_produces_observable_target() {
    let dem = circuit_to_dem(
        "R 0\nX_ERROR(0.1) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Observable(0)]);
}

#[test]
fn detector_and_observable_combined() {
    let dem = circuit_to_dem(
        "R 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0), DemTarget::Observable(0)]);
}

#[test]
fn depolarize1_produces_x_and_y_errors() {
    let dem = circuit_to_dem(
        "R 0\nDEPOLARIZE1(0.03) 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 2);
    assert_has_error(&dem, 0.01, &[DemTarget::Detector(0)]);
}

#[test]
fn correlated_error_xz_targets() {
    let dem = circuit_to_dem(
        "R 0 1\nCORRELATED_ERROR(0.1) X0 Z1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn correlated_error_y_target() {
    let dem = circuit_to_dem(
        "R 0\nCORRELATED_ERROR(0.1) Y0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn reset_clears_sensitivity() {
    let dem = circuit_to_dem(
        "R 0\nX_ERROR(0.1) 0\nR 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 0);
}

#[test]
fn mr_clears_old_and_creates_new_measurement() {
    let dem = circuit_to_dem(
        "R 0\nX_ERROR(0.1) 0\nMR 0\nX_ERROR(0.2) 0\nM 0\nDETECTOR rec[-1]\nDETECTOR rec[-2]"
    );
    assert_has_error(&dem, 0.2, &[DemTarget::Detector(0)]);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(1)]);
}

#[test]
fn two_detectors_same_measurement() {
    let dem = circuit_to_dem(
        "M 0\nDETECTOR rec[-1]\nM 0\nDETECTOR rec[-1] rec[-2]"
    );
    assert_eq!(error_count(&dem), 0);
}

#[test]
fn repetition_code_style() {
    let dem = circuit_to_dem(
        "R 0 1\nX_ERROR(0.01) 0\nCX 0 1\nM 1\nDETECTOR rec[-1]\nR 1\nX_ERROR(0.01) 0\nCX 0 1\nM 1\nDETECTOR rec[-1] rec[-2]"
    );
    assert!(error_count(&dem) > 0);
}

#[test]
fn swap_gate_swaps_sensitivities() {
    let dem = circuit_to_dem(
        "R 0 1\nX_ERROR(0.1) 0\nSWAP 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]"
    );
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(1)]);
}

#[test]
fn cz_x_error_detected_locally() {
    let dem = circuit_to_dem(
        "R 0 1\nX_ERROR(0.1) 0\nCZ 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]"
    );
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn cz_propagates_x_to_z_cross_basis() {
    let dem = circuit_to_dem(
        "R 0\nRX 1\nX_ERROR(0.1) 0\nCZ 0 1\nM 0\nMX 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]"
    );
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0), DemTarget::Detector(1)]);
}

#[test]
fn sqrt_x_gate_z_picks_up_x() {
    let dem = circuit_to_dem(
        "R 0\nZ_ERROR(0.1) 0\nSQRT_X 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn sqrt_y_gate_swaps_x_z() {
    let dem = circuit_to_dem(
        "R 0\nZ_ERROR(0.1) 0\nSQRT_Y 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn mpp_z_single_qubit() {
    let dem = circuit_to_dem(
        "R 0\nX_ERROR(0.1) 0\nMPP Z0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn mpp_x_single_qubit() {
    let dem = circuit_to_dem(
        "R 0\nH 0\nZ_ERROR(0.1) 0\nMPP X0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn mpp_zz_bell_pair() {
    let dem = circuit_to_dem(
        "R 0 1\nX_ERROR(0.1) 0\nMPP Z0*Z1\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn mxx_measurement() {
    let dem = circuit_to_dem(
        "R 0 1\nZ_ERROR(0.1) 0\nMXX 0 1\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn mzz_measurement() {
    let dem = circuit_to_dem(
        "R 0 1\nX_ERROR(0.1) 0\nMZZ 0 1\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn pauli_channel_1_x_error() {
    let dem = circuit_to_dem(
        "R 0\nPAULI_CHANNEL_1(0.1,0,0) 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn pauli_channel_1_z_error() {
    let dem = circuit_to_dem(
        "R 0\nPAULI_CHANNEL_1(0,0,0.1) 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 0);
}

#[test]
fn multiple_observables() {
    let dem = circuit_to_dem(
        "R 0 1\nX_ERROR(0.1) 0\nM 0 1\nOBSERVABLE_INCLUDE(0) rec[-2]\nOBSERVABLE_INCLUDE(1) rec[-1]"
    );
    assert_has_error(&dem, 0.1, &[DemTarget::Observable(0)]);
}

#[test]
fn mrx_measurement_reset() {
    let dem = circuit_to_dem(
        "RX 0\nZ_ERROR(0.1) 0\nMRX 0\nZ_ERROR(0.2) 0\nMX 0\nDETECTOR rec[-1]\nDETECTOR rec[-2]"
    );
    assert_has_error(&dem, 0.2, &[DemTarget::Detector(0)]);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(1)]);
}

#[test]
fn xcz_gate() {
    let dem = circuit_to_dem(
        "R 0 1\nX_ERROR(0.1) 1\nXCZ 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]"
    );
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0), DemTarget::Detector(1)]);
}

#[test]
fn depolarize2_produces_errors() {
    let dem = circuit_to_dem(
        "R 0 1\nDEPOLARIZE2(0.15) 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]"
    );
    assert!(error_count(&dem) > 0);
}

#[test]
fn e_alias_for_correlated_error() {
    let dem = circuit_to_dem(
        "R 0\nE(0.1) X0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn mry_measurement_reset() {
    let dem = circuit_to_dem(
        "R 0\nMRY 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 0);
}

#[test]
fn h_xy_sensitivity() {
    let dem = circuit_to_dem(
        "R 0\nX_ERROR(0.1) 0\nH_XY 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn h_yz_sensitivity() {
    let dem = circuit_to_dem(
        "R 0\nZ_ERROR(0.1) 0\nH_YZ 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn mpad_adjusts_measurement_count() {
    let dem = circuit_to_dem(
        "R 0\nX_ERROR(0.1) 0\nM 0\nMPAD 0\nDETECTOR rec[-2]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn no_error_when_zero_probability() {
    let dem = circuit_to_dem(
        "R 0\nX_ERROR(0) 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 0);
}

#[test]
fn iswap_sensitivity() {
    let dem = circuit_to_dem(
        "R 0 1\nX_ERROR(0.1) 0\nISWAP 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]"
    );
    assert!(error_count(&dem) > 0);
}

#[test]
fn repeat_block_error_analysis() {
    let dem = circuit_to_dem(
        "R 0\nREPEAT 3 {\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\nR 0\n}"
    );
    assert!(error_count(&dem) >= 3);
}

#[test]
fn spp_z_modifies_sensitivity() {
    let dem = circuit_to_dem(
        "R 0\nSPP Z0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn myy_measurement() {
    let dem = circuit_to_dem(
        "R 0 1\nX_ERROR(0.1) 0\nMYY 0 1\nDETECTOR rec[-1]"
    );
    assert!(error_count(&dem) > 0);
}

#[test]
fn my_measurement() {
    let dem = circuit_to_dem(
        "R 0\nX_ERROR(0.1) 0\nMY 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}
