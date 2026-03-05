use rstim::parser::parse_lines;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::dem::{DemTarget, DemInstruction, DetectorErrorModel};

fn circuit_to_dem(circuit_str: &str) -> DetectorErrorModel {
    let instrs = parse_lines(circuit_str).unwrap();
    ErrorAnalyzer::circuit_to_dem(&instrs).unwrap()
}

fn error_count(dem: &DetectorErrorModel) -> usize {
    dem.instructions().iter().filter(|i| matches!(i, DemInstruction::Error { .. })).count()
}

fn assert_has_error(dem: &DetectorErrorModel, prob: f64, expected_targets: &[DemTarget]) {
    let found = dem.instructions().iter().any(|instr| {
        if let DemInstruction::Error { probability, targets } = instr {
            (*probability - prob).abs() < 1e-12 && targets == expected_targets
        } else {
            false
        }
    });
    assert!(found, "expected error({prob}) {:?} but not found in:\n{dem}", expected_targets);
}

// --- No-op gates ---

#[test]
fn noop_i_gate() {
    let dem = circuit_to_dem("R 0\nX_ERROR(0.1) 0\nI 0\nM 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
}

#[test]
fn noop_x_y_z_gates() {
    let dem = circuit_to_dem("R 0\nX_ERROR(0.1) 0\nX 0\nY 0\nZ 0\nM 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
}

#[test]
fn noop_tick_and_coords() {
    let dem = circuit_to_dem("R 0\nQUBIT_COORDS(0,0) 0\nTICK\nSHIFT_COORDS(1,0)\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
}

#[test]
fn noop_i_error_ii_error() {
    let dem = circuit_to_dem("R 0 1\nI_ERROR(0.5) 0\nII_ERROR(0.5) 0 1\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
}

// --- CY gate ---

#[test]
fn cy_gate_propagation() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nCY 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

// --- XCX gate ---

#[test]
fn xcx_gate_propagation() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nXCX 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

// --- XCY gate ---

#[test]
fn xcy_gate_propagation() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nXCY 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

// --- YCX gate ---

#[test]
fn ycx_gate_propagation() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nYCX 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

// --- YCY gate ---

#[test]
fn ycy_gate_propagation() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nYCY 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

// --- YCZ gate ---

#[test]
fn ycz_gate_propagation() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nYCZ 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

// --- CXSWAP gate ---

#[test]
fn cxswap_gate_propagation() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nCXSWAP 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

// --- SWAPCX gate ---

#[test]
fn swapcx_gate_propagation() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nSWAPCX 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

// --- CZSWAP gate ---

#[test]
fn czswap_gate_propagation() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nCZSWAP 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

// --- ISWAP_DAG gate ---

#[test]
fn iswap_dag_gate_propagation() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nISWAP_DAG 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

// --- S aliases ---

#[test]
fn sqrt_z_alias() {
    let dem = circuit_to_dem("R 0\nX_ERROR(0.1) 0\nSQRT_Z 0\nM 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
}

#[test]
fn s_dag_alias() {
    let dem = circuit_to_dem("R 0\nX_ERROR(0.1) 0\nS_DAG 0\nM 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
}

#[test]
fn sqrt_z_dag_alias() {
    let dem = circuit_to_dem("R 0\nX_ERROR(0.1) 0\nSQRT_Z_DAG 0\nM 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
}

// --- SQRT_X_DAG, SQRT_Y_DAG ---

#[test]
fn sqrt_x_dag_propagation() {
    let dem = circuit_to_dem("R 0\nZ_ERROR(0.1) 0\nSQRT_X_DAG 0\nM 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
}

#[test]
fn sqrt_y_dag_propagation() {
    let dem = circuit_to_dem("R 0\nZ_ERROR(0.1) 0\nSQRT_Y_DAG 0\nM 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
}

// --- RX and RY resets ---

#[test]
fn rx_reset_clears_sensitivity() {
    let dem = circuit_to_dem("R 0\nX_ERROR(0.1) 0\nRX 0\nMX 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 0);
}

#[test]
fn ry_reset_clears_sensitivity() {
    let dem = circuit_to_dem("R 0\nX_ERROR(0.1) 0\nRY 0\nMY 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 0);
}

// --- ELSE_CORRELATED_ERROR ---

#[test]
fn else_correlated_error() {
    let dem = circuit_to_dem("R 0\nELSE_CORRELATED_ERROR(0.1) X0\nM 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

// --- PAULI_CHANNEL_2 ---

#[test]
fn pauli_channel_2_produces_errors() {
    let dem = circuit_to_dem(
        "R 0 1\nPAULI_CHANNEL_2(0.01,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]"
    );
    assert!(error_count(&dem) > 0);
}

#[test]
fn pauli_channel_2_zi_channel() {
    let dem = circuit_to_dem(
        "R 0 1\nPAULI_CHANNEL_2(0,0,0,0,0,0,0,0,0,0,0,0.1,0,0,0) 0 1\nMX 0\nMX 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]"
    );
    assert!(error_count(&dem) > 0);
}

// --- HERALDED_ERASE ---

#[test]
fn heralded_erase_produces_errors() {
    let dem = circuit_to_dem(
        "R 0\nHERALDED_ERASE(1.0) 0\nM 0\nDETECTOR rec[-1]"
    );
    assert!(error_count(&dem) > 0);
}

#[test]
fn heralded_erase_zero_prob() {
    let dem = circuit_to_dem(
        "R 0\nHERALDED_ERASE(0) 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 0);
}

// --- HERALDED_PAULI_CHANNEL_1 ---

#[test]
fn heralded_pauli_channel_1_x() {
    let dem = circuit_to_dem(
        "R 0\nHERALDED_PAULI_CHANNEL_1(0,0.1,0,0) 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn heralded_pauli_channel_1_y() {
    let dem = circuit_to_dem(
        "R 0\nHERALDED_PAULI_CHANNEL_1(0,0,0.1,0) 0\nM 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

#[test]
fn heralded_pauli_channel_1_z() {
    let dem = circuit_to_dem(
        "R 0\nHERALDED_PAULI_CHANNEL_1(0,0,0,0.1) 0\nMX 0\nDETECTOR rec[-1]"
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

// --- SPP_DAG ---

#[test]
fn spp_dag_modifies_sensitivity() {
    let dem = circuit_to_dem("R 0\nSPP_DAG Z0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

// --- MPP with X and Y products ---

#[test]
fn mpp_x_product() {
    let dem = circuit_to_dem("R 0 1\nZ_ERROR(0.1) 0\nMPP X0*X1\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

#[test]
fn mpp_y_product() {
    let dem = circuit_to_dem("R 0\nX_ERROR(0.1) 0\nMPP Y0\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

#[test]
fn mpp_multiple_products() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nMPP Z0 Z1\nDETECTOR rec[-2]\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

// --- SPP with X and Y products ---

#[test]
fn spp_x_product() {
    let dem = circuit_to_dem("R 0\nSPP X0\nZ_ERROR(0.1) 0\nMX 0\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

#[test]
fn spp_y_product() {
    let dem = circuit_to_dem("R 0\nSPP Y0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

#[test]
fn spp_multi_qubit() {
    let dem = circuit_to_dem("R 0 1\nSPP Z0*Z1\nX_ERROR(0.1) 0\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

// --- Z_ERROR with Z-sensitive detector ---

#[test]
fn z_error_detected_by_mx() {
    let dem = circuit_to_dem("RX 0\nZ_ERROR(0.1) 0\nMX 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

// --- DEPOLARIZE1 with Z sensitivity ---

#[test]
fn depolarize1_with_z_sensitivity() {
    // Y and Z errors both flip MX measurement → merged into one entry with p = 2*0.01 = 0.02
    let dem = circuit_to_dem("RX 0\nDEPOLARIZE1(0.03) 0\nMX 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.02, &[DemTarget::Detector(0)]);
}

// --- PAULI_CHANNEL_1 with Y and Z ---

#[test]
fn pauli_channel_1_y_error() {
    let dem = circuit_to_dem("R 0\nPAULI_CHANNEL_1(0,0.1,0) 0\nM 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
}

#[test]
fn pauli_channel_1_z_detected_by_mx() {
    let dem = circuit_to_dem("RX 0\nPAULI_CHANNEL_1(0,0,0.1) 0\nMX 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

// --- MRY with sensitivity ---

#[test]
fn mry_detects_both_x_and_z_errors() {
    let dem = circuit_to_dem("R 0\nX_ERROR(0.1) 0\nMRY 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
}

// --- CNOT alias ---

#[test]
fn cnot_alias_propagation() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nCNOT 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]");
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0), DemTarget::Detector(1)]);
}

// --- ZCX/ZCY/ZCZ aliases ---

#[test]
fn zcx_alias_propagation() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nZCX 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]");
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0), DemTarget::Detector(1)]);
}

#[test]
fn zcy_alias_propagation() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nZCY 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

#[test]
fn zcz_alias_propagation() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nZCZ 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

// --- count_qubits with Repeat body ---

#[test]
fn count_qubits_in_repeat_body() {
    let dem = circuit_to_dem("REPEAT 2 {\nR 0 1\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\nR 0\n}");
    assert!(error_count(&dem) >= 2);
}

// --- count_measurements with various measurement types ---

#[test]
fn count_mxx_measurements() {
    let dem = circuit_to_dem("R 0 1\nZ_ERROR(0.1) 0\nMXX 0 1\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

#[test]
fn count_myy_measurements() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nMYY 0 1\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

#[test]
fn count_mzz_measurements() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nMZZ 0 1\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

#[test]
fn count_mpad_measurements() {
    let dem = circuit_to_dem("R 0\nX_ERROR(0.1) 0\nM 0\nMPAD 0 0\nDETECTOR rec[-3]");
    assert_eq!(error_count(&dem), 1);
}

#[test]
fn count_mpp_measurements() {
    let dem = circuit_to_dem("R 0 1\nX_ERROR(0.1) 0\nMPP Z0 Z1\nDETECTOR rec[-2]");
    assert_eq!(error_count(&dem), 1);
}

#[test]
fn count_heralded_measurements() {
    let dem = circuit_to_dem("R 0\nHERALDED_ERASE(0.1) 0\nM 0\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

#[test]
fn count_heralded_pauli_channel_measurements() {
    let dem = circuit_to_dem("R 0\nHERALDED_PAULI_CHANNEL_1(0,0.1,0,0) 0\nM 0\nDETECTOR rec[-1]");
    assert!(error_count(&dem) > 0);
}

// --- count_annotations with Repeat ---

#[test]
fn count_annotations_in_repeat() {
    let dem = circuit_to_dem("R 0\nREPEAT 3 {\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\nR 0\n}");
    assert!(error_count(&dem) >= 3);
    assert!(dem.num_detectors() >= 3);
}

// --- Correlated error with Y target ---

#[test]
fn correlated_error_z_target() {
    let dem = circuit_to_dem("RX 0\nCORRELATED_ERROR(0.1) Z0\nMX 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.1, &[DemTarget::Detector(0)]);
}

// --- Inverted measurement targets ---

#[test]
fn inverted_mz_target() {
    let dem = circuit_to_dem("R 0\nX_ERROR(0.1) 0\nM !0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
}

// --- Unsupported instruction error ---

#[test]
fn unsupported_instruction_returns_error() {
    let instrs = parse_lines("FOOBAR 0").unwrap();
    let result = ErrorAnalyzer::circuit_to_dem(&instrs);
    assert!(result.is_err());
}

// --- DEPOLARIZE2 with Z-only sensitivity ---

#[test]
fn depolarize2_with_mx_detectors() {
    let dem = circuit_to_dem(
        "RX 0 1\nDEPOLARIZE2(0.15) 0 1\nMX 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]"
    );
    assert!(error_count(&dem) > 0);
}

// --- MZ alias ---

#[test]
fn mz_alias() {
    let dem = circuit_to_dem("R 0\nX_ERROR(0.1) 0\nMZ 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
}

// --- MRZ alias ---

#[test]
fn mrz_alias() {
    let dem = circuit_to_dem("R 0\nX_ERROR(0.1) 0\nMRZ 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 1);
}

// --- RZ alias ---

#[test]
fn rz_alias() {
    let dem = circuit_to_dem("R 0\nX_ERROR(0.1) 0\nRZ 0\nM 0\nDETECTOR rec[-1]");
    assert_eq!(error_count(&dem), 0);
}
