use rstim::ir::{circuit_to_string, PauliBasis, StimInstr, StimTarget};
use rstim::parser::parse_lines;
use rstim::stats;
use rstim::transforms;
use rstim::sampler::sample_batch;
use rstim::error_analyzer::ErrorAnalyzer;
use rand::SeedableRng;
use rand::rngs::StdRng;

// ========== circuit_to_string coverage (src/ir.rs) ==========

#[test]
fn circuit_to_string_simple_op() {
    let instrs = parse_lines("H 0\nM 0").unwrap();
    let s = circuit_to_string(&instrs);
    assert!(s.contains("H 0"));
    assert!(s.contains("M 0"));
}

#[test]
fn circuit_to_string_with_tag() {
    let instrs = vec![StimInstr::Op {
        name: "H".to_string(),
        tag: Some("my_tag".to_string()),
        args: vec![],
        targets: vec![StimTarget::Qubit(0)],
    }];
    let s = circuit_to_string(&instrs);
    assert!(s.contains("H[my_tag] 0"));
}

#[test]
fn circuit_to_string_with_integer_args() {
    let instrs = parse_lines("DETECTOR(1,2,3) rec[-1]").unwrap();
    let s = circuit_to_string(&instrs);
    assert!(s.contains("DETECTOR(1,2,3)"));
    assert!(s.contains("rec[-1]"));
}

#[test]
fn circuit_to_string_with_float_args() {
    let instrs = parse_lines("DEPOLARIZE1(0.001) 0").unwrap();
    let s = circuit_to_string(&instrs);
    assert!(s.contains("DEPOLARIZE1(0.001)"));
}

#[test]
fn circuit_to_string_qubit_inv() {
    let instrs = vec![StimInstr::Op {
        name: "M".to_string(),
        tag: None,
        args: vec![],
        targets: vec![StimTarget::QubitInv(5)],
    }];
    let s = circuit_to_string(&instrs);
    assert!(s.contains("!5"));
}

#[test]
fn circuit_to_string_rec_target() {
    let instrs = parse_lines("M 0\nDETECTOR rec[-1]").unwrap();
    let s = circuit_to_string(&instrs);
    assert!(s.contains("rec[-1]"));
}

#[test]
fn circuit_to_string_pauli_targets() {
    let instrs = vec![StimInstr::Op {
        name: "MPP".to_string(),
        tag: None,
        args: vec![],
        targets: vec![
            StimTarget::Pauli { qubit: 0, basis: PauliBasis::X, inverted: false },
            StimTarget::Combiner,
            StimTarget::Pauli { qubit: 1, basis: PauliBasis::Y, inverted: false },
            StimTarget::Pauli { qubit: 2, basis: PauliBasis::Z, inverted: true },
        ],
    }];
    let s = circuit_to_string(&instrs);
    assert!(s.contains("X0"));
    assert!(s.contains("*"));
    assert!(s.contains("Y1"));
    assert!(s.contains("!Z2"));
}

#[test]
fn circuit_to_string_repeat_block() {
    let instrs = parse_lines("REPEAT 3 {\n  H 0\n  M 0\n}").unwrap();
    let s = circuit_to_string(&instrs);
    assert!(s.contains("REPEAT 3 {"));
    assert!(s.contains("    H 0"));
    assert!(s.contains("}"));
}

#[test]
fn circuit_to_string_nested_repeat() {
    let instrs = parse_lines("REPEAT 2 {\n  REPEAT 3 {\n    H 0\n  }\n}").unwrap();
    let s = circuit_to_string(&instrs);
    assert!(s.contains("REPEAT 2 {"));
    assert!(s.contains("    REPEAT 3 {"));
    assert!(s.contains("        H 0"));
}

#[test]
fn circuit_to_string_roundtrip() {
    let original = "R 0 1 2\nH 0\nCX 0 1\nDEPOLARIZE1(0.01) 0\nM 0 1\nDETECTOR(1,2,3) rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-2]";
    let instrs = parse_lines(original).unwrap();
    let s = circuit_to_string(&instrs);
    let re_parsed = parse_lines(&s).unwrap();
    assert_eq!(instrs.len(), re_parsed.len());
}

#[test]
fn circuit_to_string_gen_roundtrip() {
    let instrs = rstim::circuit_gen::repetition_code_memory(3, 2, 0.001);
    let s = circuit_to_string(&instrs);
    let re_parsed = parse_lines(&s).unwrap();
    assert_eq!(instrs.len(), re_parsed.len());
}

#[test]
fn circuit_to_string_empty() {
    let s = circuit_to_string(&[]);
    assert_eq!(s, "");
}

#[test]
fn circuit_to_string_multiple_args() {
    let instrs = parse_lines("PAULI_CHANNEL_1(0.1,0.2,0.3) 0").unwrap();
    let s = circuit_to_string(&instrs);
    assert!(s.contains("0.1"));
    assert!(s.contains("0.2"));
    assert!(s.contains("0.3"));
}

// ========== Frame simulator exotic gates coverage (src/sim/frame.rs) ==========

#[test]
fn frame_c_xyz_family_with_noise() {
    for gate in &["C_XYZ", "C_NXYZ", "C_XNYZ", "C_XYNZ"] {
        let circuit = format!("R 0\n{gate} 0\nX_ERROR(0.5) 0\nM 0");
        let instrs = parse_lines(&circuit).unwrap();
        let mut rng = StdRng::seed_from_u64(42);
        let result = sample_batch(&instrs, 64, &mut rng).unwrap();
        assert_eq!(result.measurements.num_major(), 1);
    }
}

#[test]
fn frame_c_zyx_family_with_noise() {
    for gate in &["C_ZYX", "C_NZYX", "C_ZNYX", "C_ZYNX"] {
        let circuit = format!("R 0\n{gate} 0\nX_ERROR(0.5) 0\nM 0");
        let instrs = parse_lines(&circuit).unwrap();
        let mut rng = StdRng::seed_from_u64(42);
        let result = sample_batch(&instrs, 64, &mut rng).unwrap();
        assert_eq!(result.measurements.num_major(), 1);
    }
}

#[test]
fn frame_h_nxy_with_noise() {
    let instrs = parse_lines("R 0\nH_NXY 0\nX_ERROR(0.5) 0\nM 0").unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let result = sample_batch(&instrs, 64, &mut rng).unwrap();
    assert_eq!(result.measurements.num_major(), 1);
}

#[test]
fn frame_h_nxz_with_noise() {
    let instrs = parse_lines("R 0\nH_NXZ 0\nX_ERROR(0.5) 0\nM 0").unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let result = sample_batch(&instrs, 64, &mut rng).unwrap();
    assert_eq!(result.measurements.num_major(), 1);
}

#[test]
fn frame_h_nyz_with_noise() {
    let instrs = parse_lines("R 0\nH_NYZ 0\nX_ERROR(0.5) 0\nM 0").unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let result = sample_batch(&instrs, 64, &mut rng).unwrap();
    assert_eq!(result.measurements.num_major(), 1);
}

#[test]
fn frame_exotic_c_gates_noiseless_deterministic() {
    for gate in &["C_XYZ", "C_ZYX", "C_NXYZ", "C_NZYX", "C_XNYZ", "C_XYNZ", "C_ZNYX", "C_ZYNX"] {
        let circuit = format!("R 0\n{gate} 0\n{gate} 0\n{gate} 0\nM 0");
        let instrs = parse_lines(&circuit).unwrap();
        let mut rng = StdRng::seed_from_u64(42);
        let result = sample_batch(&instrs, 10, &mut rng).unwrap();
        for shot in 0..10 {
            assert!(!result.measurements.get(0, shot),
                "gate {gate} cubed should give 0 on |0>");
        }
    }
}

#[test]
fn frame_exotic_h_gates_noiseless_deterministic() {
    for gate in &["H_NXY", "H_NXZ", "H_NYZ"] {
        let circuit = format!("R 0\n{gate} 0\n{gate} 0\nM 0");
        let instrs = parse_lines(&circuit).unwrap();
        let mut rng = StdRng::seed_from_u64(42);
        let result = sample_batch(&instrs, 10, &mut rng).unwrap();
        for shot in 0..10 {
            assert!(!result.measurements.get(0, shot),
                "gate {gate} squared should give 0 on |0>");
        }
    }
}

// ========== Error analyzer exotic gates coverage (src/error_analyzer.rs) ==========

#[test]
fn error_analyzer_c_xyz_family() {
    for gate in &["C_XYZ", "C_NXYZ", "C_XNYZ", "C_XYNZ"] {
        let circuit = format!("R 0\n{gate} 0\nX_ERROR(0.1) 0\n{gate} 0\n{gate} 0\nM 0\nDETECTOR rec[-1]");
        let instrs = parse_lines(&circuit).unwrap();
        let _dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    }
}

#[test]
fn error_analyzer_c_zyx_family() {
    for gate in &["C_ZYX", "C_NZYX", "C_ZNYX", "C_ZYNX"] {
        let circuit = format!("R 0 1\n{gate} 0\nCX 0 1\nX_ERROR(0.1) 0\n{gate} 0\n{gate} 0\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]");
        let instrs = parse_lines(&circuit).unwrap();
        let result = ErrorAnalyzer::circuit_to_dem(&instrs);
        assert!(result.is_err(), "expected default gauge rejection for {gate}");
    }
}

#[test]
fn error_analyzer_h_nxy() {
    let circuit = "R 0 1\nH_NXY 0\nCX 0 1\nX_ERROR(0.1) 0\nH_NXY 0\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]";
    let instrs = parse_lines(circuit).unwrap();
    let _dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
}

#[test]
fn error_analyzer_h_nxz() {
    let circuit = "R 0 1\nH_NXZ 0\nCX 0 1\nX_ERROR(0.1) 0\nH_NXZ 0\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]";
    let instrs = parse_lines(circuit).unwrap();
    let result = ErrorAnalyzer::circuit_to_dem(&instrs);
    assert!(result.is_err());
}

#[test]
fn error_analyzer_h_nyz() {
    let circuit = "R 0 1\nH_NYZ 0\nCX 0 1\nX_ERROR(0.1) 0\nH_NYZ 0\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]";
    let instrs = parse_lines(circuit).unwrap();
    let result = ErrorAnalyzer::circuit_to_dem(&instrs);
    assert!(result.is_err());
}

// ========== Stats coverage (src/stats.rs) ==========

#[test]
fn stats_num_measurements_mxx() {
    let instrs = parse_lines("MXX 0 1 2 3").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 2);
}

#[test]
fn stats_num_measurements_myy() {
    let instrs = parse_lines("MYY 0 1").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 1);
}

#[test]
fn stats_num_measurements_mzz() {
    let instrs = parse_lines("MZZ 0 1 2 3 4 5").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 3);
}

#[test]
fn stats_num_measurements_mpad() {
    let instrs = parse_lines("MPAD(5) 0\nMPAD 0 1 0 1").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 5);
}

#[test]
fn stats_num_measurements_heralded() {
    let instrs = parse_lines("HERALDED_ERASE(0.1) 0 1 2").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 3);
}

#[test]
fn stats_num_measurements_heralded_pauli() {
    let instrs = parse_lines("HERALDED_PAULI_CHANNEL_1(0.1,0,0,0) 0").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 1);
}

#[test]
fn stats_num_measurements_non_measurement() {
    let instrs = parse_lines("H 0\nCX 0 1\nDEPOLARIZE1(0.01) 0").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 0);
}

#[test]
fn stats_num_measurements_mrx_mry_mrz() {
    let instrs = parse_lines("MRX 0\nMRY 1\nMRZ 2").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 3);
}

#[test]
fn stats_num_qubits_with_repeat() {
    let instrs = parse_lines("REPEAT 5 {\n  H 3\n}").unwrap();
    assert_eq!(stats::num_qubits(&instrs), 4);
}

#[test]
fn stats_num_qubits_rec_targets_ignored() {
    let instrs = parse_lines("M 0\nDETECTOR rec[-1]").unwrap();
    assert_eq!(stats::num_qubits(&instrs), 1);
}

#[test]
fn stats_num_observables_in_repeat() {
    let instrs = parse_lines("REPEAT 3 {\n  M 0\n  OBSERVABLE_INCLUDE(2) rec[-1]\n}").unwrap();
    assert_eq!(stats::num_observables(&instrs), 3);
}

#[test]
fn stats_num_observables_empty() {
    let instrs = parse_lines("H 0\nM 0").unwrap();
    assert_eq!(stats::num_observables(&instrs), 0);
}

#[test]
fn stats_num_qubits_pauli_targets() {
    let instrs = parse_lines("MPP X0*Y5 Z3").unwrap();
    assert_eq!(stats::num_qubits(&instrs), 6);
}

// ========== CLI run_gen coverage (src/cli.rs) ==========

#[test]
fn cli_run_gen_direct() {
    let mut buf = Vec::new();
    rstim::cli::run_gen("repetition_code", "memory", 3, 2, 0.001, &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("R "));
    assert!(s.contains("CX "));
    assert!(s.contains("M "));
    assert!(s.contains("DETECTOR"));
    assert!(s.contains("OBSERVABLE_INCLUDE"));
}

#[test]
fn cli_run_gen_noiseless() {
    let mut buf = Vec::new();
    rstim::cli::run_gen("repetition_code", "memory", 3, 1, 0.0, &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(!s.contains("DEPOLARIZE"));
}

#[test]
fn cli_run_gen_unknown_code() {
    let mut buf = Vec::new();
    let result = rstim::cli::run_gen("surface_code", "memory", 3, 1, 0.0, &mut buf);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("unknown code/task"));
}

#[test]
fn cli_run_gen_via_dispatch() {
    use clap::Parser;
    let cli = rstim::cli::Cli::parse_from([
        "rstim", "gen",
        "--code", "repetition_code",
        "--task", "memory",
        "--distance", "3",
        "--rounds", "1",
    ]);
    let result = rstim::cli::run(cli);
    assert!(result.is_ok());
}

// ========== Transforms coverage (src/transforms.rs) ==========

#[test]
fn inverse_c_xyz_gates() {
    let instrs = parse_lines("C_XYZ 0\nC_ZYX 1").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv[0].name().unwrap(), "C_XYZ");
    assert_eq!(inv[1].name().unwrap(), "C_ZYX");
}

#[test]
fn inverse_h_n_gates() {
    let instrs = parse_lines("H_NXY 0\nH_NXZ 1\nH_NYZ 2").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv[0].name().unwrap(), "H_NYZ");
    assert_eq!(inv[1].name().unwrap(), "H_NXZ");
    assert_eq!(inv[2].name().unwrap(), "H_NXY");
}

#[test]
fn inverse_cxswap_swapcx() {
    let instrs = parse_lines("CXSWAP 0 1").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv[0].name().unwrap(), "SWAPCX");

    let instrs2 = parse_lines("SWAPCX 0 1").unwrap();
    let inv2 = transforms::inverse(&instrs2).unwrap();
    assert_eq!(inv2[0].name().unwrap(), "CXSWAP");
}

#[test]
fn inverse_czswap() {
    let instrs = parse_lines("CZSWAP 0 1").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv[0].name().unwrap(), "CZSWAP");
}

#[test]
fn inverse_fails_on_reset() {
    let instrs = parse_lines("R 0").unwrap();
    assert!(transforms::inverse(&instrs).is_err());
}

#[test]
fn inverse_fails_on_annotation() {
    let instrs = parse_lines("M 0\nDETECTOR rec[-1]").unwrap();
    assert!(transforms::inverse(&instrs).is_err());
}

#[test]
fn inverse_iswap() {
    let instrs = parse_lines("ISWAP 0 1").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv[0].name().unwrap(), "ISWAP_DAG");
}

#[test]
fn inverse_sqrt_z() {
    let instrs = parse_lines("SQRT_Z 0\nSQRT_Z_DAG 1").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv[0].name().unwrap(), "S");
    assert_eq!(inv[1].name().unwrap(), "S_DAG");
}

#[test]
fn inverse_c_negated_variants() {
    for (gate, expected) in [
        ("C_NXYZ", "C_XYNZ"), ("C_XYNZ", "C_NXYZ"),
        ("C_XNYZ", "C_ZNYX"), ("C_ZNYX", "C_XNYZ"),
        ("C_NZYX", "C_ZYNX"), ("C_ZYNX", "C_NZYX"),
    ] {
        let instrs = parse_lines(&format!("{gate} 0")).unwrap();
        let inv = transforms::inverse(&instrs).unwrap();
        assert_eq!(inv[0].name().unwrap(), expected, "inverse of {gate} should be {expected}");
    }
}

#[test]
fn without_noise_empty_repeat_removed() {
    let instrs = parse_lines("REPEAT 3 {\n  X_ERROR(0.1) 0\n}").unwrap();
    let clean = transforms::without_noise(&instrs);
    assert!(clean.is_empty());
}

#[test]
fn inverse_fails_on_unknown() {
    let instrs = vec![StimInstr::Op {
        name: "TOTALLY_UNKNOWN_GATE".to_string(),
        tag: None,
        args: vec![],
        targets: vec![StimTarget::Qubit(0)],
    }];
    assert!(transforms::inverse(&instrs).is_err());
}

// ========== qubit_index coverage (src/ir.rs) ==========

#[test]
fn qubit_index_rec_returns_none() {
    let t = StimTarget::Rec(-1);
    assert_eq!(t.qubit_index(), None);
}

#[test]
fn qubit_index_combiner_returns_none() {
    let t = StimTarget::Combiner;
    assert_eq!(t.qubit_index(), None);
}

#[test]
fn qubit_index_qubit() {
    let t = StimTarget::Qubit(5);
    assert_eq!(t.qubit_index(), Some(5));
}

#[test]
fn qubit_index_qubit_inv() {
    let t = StimTarget::QubitInv(3);
    assert_eq!(t.qubit_index(), Some(3));
}

#[test]
fn qubit_index_pauli() {
    let t = StimTarget::Pauli { qubit: 7, basis: PauliBasis::X, inverted: false };
    assert_eq!(t.qubit_index(), Some(7));
}
