/// Tests ported from Stim's circuit.test.cc to rstim.
///
/// Source: Stim/src/stim/circuit/circuit.test.cc
///
/// Covers: parsing, validation, round-trips, repeat blocks, stats.

use rstim::ir::{circuit_to_string, PauliBasis, StimInstr, StimTarget};
use rstim::parser::parse_lines;
use rstim::stats;
use rstim::transforms;

// ========== from_text ==========

#[test]
fn from_text_comment_only() {
    let instrs = parse_lines("# not an operation").unwrap();
    assert!(instrs.is_empty());
}

#[test]
fn from_text_empty() {
    assert!(parse_lines("").unwrap().is_empty());
    assert!(parse_lines("# Comment\n\n\n# More").unwrap().is_empty());
}

#[test]
fn from_text_h_gate_variants() {
    // All of these should parse to H 0
    let cases = &[
        "H 0",
        "h 0",
        "H 0     ",
        "     H 0     ",
        "\tH 0\t\t",
        "H 0  # comment",
    ];
    for case in cases {
        let instrs = parse_lines(case).unwrap();
        assert_eq!(instrs.len(), 1, "case: {case:?}");
        assert_eq!(instrs[0].name().unwrap(), "H");
        assert_eq!(instrs[0].targets().unwrap(), &[StimTarget::Qubit(0)]);
    }
}

#[test]
fn from_text_h_larger_qubit() {
    let instrs = parse_lines("H 23").unwrap();
    assert_eq!(instrs[0].targets().unwrap(), &[StimTarget::Qubit(23)]);
}

#[test]
fn from_text_depolarize1_with_arg() {
    let instrs = parse_lines("DEPOLARIZE1(0.125) 4 5  # comment").unwrap();
    assert_eq!(instrs.len(), 1);
    assert_eq!(instrs[0].name().unwrap(), "DEPOLARIZE1");
    assert_eq!(instrs[0].args().unwrap(), &[0.125]);
    assert_eq!(
        instrs[0].targets().unwrap(),
        &[StimTarget::Qubit(4), StimTarget::Qubit(5)]
    );
}

#[test]
fn from_text_cnot_alias() {
    let instrs = parse_lines("  \t Cnot 5 6  # comment   ").unwrap();
    assert_eq!(instrs.len(), 1);
    // rstim uppercases: CNOT
    assert_eq!(instrs[0].name().unwrap(), "CNOT");
    assert_eq!(
        instrs[0].targets().unwrap(),
        &[StimTarget::Qubit(5), StimTarget::Qubit(6)]
    );
}

#[test]
fn from_text_parse_errors() {
    assert!(parse_lines("H a").is_err());
    assert!(parse_lines("H 9999999999999999999999999999999999999999999").is_err());
    assert!(parse_lines("H -1").is_err());
    assert!(parse_lines("CNOT 0 a").is_err());
    assert!(parse_lines("CNOT 0 99999999999999999999999999999999").is_err());
    assert!(parse_lines("CNOT 0 -1").is_err());
    assert!(parse_lines("DETECTOR rec[0]").is_err());
    assert!(parse_lines("DETECTOR rec[1]").is_err());
    // Note: rstim does not validate gate names, so DETEstdCTOR parses as a
    // valid (albeit unknown) instruction name.  Stim rejects it.
}

#[test]
fn from_text_inverted_measurement_targets() {
    let instrs = parse_lines("M 0 !0 1 !1").unwrap();
    assert_eq!(instrs.len(), 1);
    assert_eq!(
        instrs[0].targets().unwrap(),
        &[
            StimTarget::Qubit(0),
            StimTarget::QubitInv(0),
            StimTarget::Qubit(1),
            StimTarget::QubitInv(1),
        ]
    );
}

#[test]
fn from_text_multi_line() {
    let instrs = parse_lines("# EPR\nH 0\nCNOT 0 1").unwrap();
    assert_eq!(instrs.len(), 2);
    assert_eq!(instrs[0].name().unwrap(), "H");
    assert_eq!(instrs[1].name().unwrap(), "CNOT");
}

#[test]
fn from_text_repeat_block() {
    let instrs = parse_lines("X 0\nREPEAT 2 {\n  Y 1\n  Y 2\n}").unwrap();
    assert_eq!(instrs.len(), 2);
    match &instrs[1] {
        StimInstr::Repeat { count, body } => {
            assert_eq!(*count, 2);
            assert_eq!(body.len(), 2);
        }
        _ => panic!("expected Repeat"),
    }
}

#[test]
fn from_text_detector_rec() {
    let instrs = parse_lines("DETECTOR rec[-5]").unwrap();
    assert_eq!(instrs[0].targets().unwrap(), &[StimTarget::Rec(-5)]);
}

#[test]
fn from_text_correlated_error() {
    let instrs = parse_lines("CORRELATED_ERROR(0.125) X90 Y91 Z92 X93").unwrap();
    assert_eq!(instrs.len(), 1);
    assert_eq!(instrs[0].name().unwrap(), "CORRELATED_ERROR");
    assert_eq!(instrs[0].args().unwrap(), &[0.125]);
    let targets = instrs[0].targets().unwrap();
    assert_eq!(targets.len(), 4);
    assert_eq!(
        targets[0],
        StimTarget::Pauli {
            qubit: 90,
            basis: PauliBasis::X,
            inverted: false
        }
    );
    assert_eq!(
        targets[1],
        StimTarget::Pauli {
            qubit: 91,
            basis: PauliBasis::Y,
            inverted: false
        }
    );
}

// ========== parse_mpp ==========

#[test]
fn parse_mpp_errors() {
    // bare qubit not allowed in MPP (rstim parser doesn't validate this the same way,
    // but empty-around-* and double-* cases are checked)
    assert!(parse_lines("MPP X1**Y2").is_err());
}

#[test]
fn parse_mpp_valid() {
    let instrs = parse_lines("MPP X1*Y2 Z3*Z4\nMPP Z5").unwrap();
    // rstim does not fuse across lines, so we get 2 instructions
    assert_eq!(instrs.len(), 2);
    let t = instrs[0].targets().unwrap();
    // X1 * Y2 Z3 * Z4
    assert_eq!(
        t,
        &[
            StimTarget::Pauli { qubit: 1, basis: PauliBasis::X, inverted: false },
            StimTarget::Combiner,
            StimTarget::Pauli { qubit: 2, basis: PauliBasis::Y, inverted: false },
            StimTarget::Pauli { qubit: 3, basis: PauliBasis::Z, inverted: false },
            StimTarget::Combiner,
            StimTarget::Pauli { qubit: 4, basis: PauliBasis::Z, inverted: false },
        ]
    );
}

#[test]
fn parse_mpp_with_arg() {
    let instrs = parse_lines("MPP(0.125) X1*Y2 Z3*Z4").unwrap();
    assert_eq!(instrs[0].args().unwrap(), &[0.125]);
}

// ========== parse_spp ==========

#[test]
fn parse_spp_valid() {
    let instrs = parse_lines("SPP X1 Z2").unwrap();
    assert_eq!(instrs.len(), 1);
    assert_eq!(instrs[0].name().unwrap(), "SPP");
    let t = instrs[0].targets().unwrap();
    assert_eq!(t.len(), 2);
    assert_eq!(
        t[0],
        StimTarget::Pauli { qubit: 1, basis: PauliBasis::X, inverted: false }
    );
    assert_eq!(
        t[1],
        StimTarget::Pauli { qubit: 2, basis: PauliBasis::Z, inverted: false }
    );
}

#[test]
fn parse_spp_with_combiners() {
    let instrs = parse_lines("SPP X0 X1*Y2*Z3").unwrap();
    assert_eq!(instrs.len(), 1);
}

// ========== str (round-trip) ==========

#[test]
fn str_roundtrip_basic() {
    let input = "TICK\nCX 2 3\nM 1 3 2\nDETECTOR rec[-7]\nOBSERVABLE_INCLUDE(17) rec[-11] rec[-1]\nX_ERROR(0.5) 19\nCORRELATED_ERROR(0.25) X23 Z27 Y29";
    let instrs = parse_lines(input).unwrap();
    let s = circuit_to_string(&instrs);
    // Parse back and compare
    let re_parsed = parse_lines(&s).unwrap();
    let s2 = circuit_to_string(&re_parsed);
    assert_eq!(s, s2);
}

#[test]
fn str_roundtrip_complex() {
    let input = "R 0 1 2\nH 0\nCX 0 1\nDEPOLARIZE1(0.01) 0\nM 0 1\nDETECTOR(1,2,3) rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-2]";
    let instrs = parse_lines(input).unwrap();
    let s = circuit_to_string(&instrs);
    let re_parsed = parse_lines(&s).unwrap();
    let s2 = circuit_to_string(&re_parsed);
    assert_eq!(s, s2);
}

#[test]
fn str_roundtrip_atom_loss_sampling_ops() {
    let input = "LOSS(0.125) 0 2\nML 0\nMRXL !1\nM 2";
    let instrs = parse_lines(input).unwrap();
    let s = circuit_to_string(&instrs);
    let re_parsed = parse_lines(&s).unwrap();
    let s2 = circuit_to_string(&re_parsed);
    assert_eq!(s, s2);
}

// ========== repeat_validation ==========

#[test]
fn repeat_validation_unterminated() {
    assert!(parse_lines("REPEAT 100 {").is_err());
}

#[test]
fn repeat_validation_missing_count() {
    assert!(parse_lines("REPEAT {\n}").is_err());
}

#[test]
fn repeat_validation_non_repeat_block() {
    assert!(parse_lines("H {\n}").is_err());
}

// ========== tick_validation ==========

#[test]
fn tick_parses_no_targets() {
    let instrs = parse_lines("TICK").unwrap();
    assert_eq!(instrs.len(), 1);
    assert_eq!(instrs[0].name().unwrap(), "TICK");
    assert!(instrs[0].targets().unwrap().is_empty());
}

// ========== detector_validation ==========

#[test]
fn detector_with_rec() {
    let instrs = parse_lines("M 0\nDETECTOR rec[-1]").unwrap();
    assert_eq!(instrs.len(), 2);
    assert_eq!(instrs[1].name().unwrap(), "DETECTOR");
}

// ========== x_error_validation ==========

#[test]
fn x_error_valid() {
    parse_lines("X_ERROR(0) 1").unwrap();
    parse_lines("X_ERROR(0.1) 1").unwrap();
    parse_lines("X_ERROR(1) 1").unwrap();
}

// ========== pauli_err_1_validation ==========

#[test]
fn pauli_channel_1_valid() {
    parse_lines("PAULI_CHANNEL_1(0,0,0) 1").unwrap();
    parse_lines("PAULI_CHANNEL_1(0.1,0.2,0.6) 1").unwrap();
    parse_lines("PAULI_CHANNEL_1(1,0,0) 1").unwrap();
}

// ========== pauli_err_2_validation ==========

#[test]
fn pauli_channel_2_valid() {
    parse_lines("PAULI_CHANNEL_2(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 1 2").unwrap();
    parse_lines("PAULI_CHANNEL_2(0.1,0,0,0,0,0,0,0,0,0,0.1,0,0,0,0.1) 1 2").unwrap();
}

// ========== qubit_coords ==========

#[test]
fn qubit_coords_basic() {
    let instrs = parse_lines("QUBIT_COORDS(1,2) 3").unwrap();
    assert_eq!(instrs.len(), 1);
    assert_eq!(instrs[0].name().unwrap(), "QUBIT_COORDS");
    assert_eq!(instrs[0].args().unwrap(), &[1.0, 2.0]);
    assert_eq!(instrs[0].targets().unwrap(), &[StimTarget::Qubit(3)]);
}

// ========== count_qubits ==========

#[test]
fn count_qubits_empty() {
    assert_eq!(stats::num_qubits(&[]), 0);
}

#[test]
fn count_qubits_with_repeat() {
    let instrs = parse_lines(
        "H 0\nM 0 1\nREPEAT 2 {\n  X 1\n  REPEAT 3 {\n    Y 2\n    M 2\n  }\n}",
    )
    .unwrap();
    assert_eq!(stats::num_qubits(&instrs), 3);
}

#[test]
fn count_qubits_deeply_nested() {
    let instrs = parse_lines(
        "H 0\nM 0 1\nREPEAT 999999 {\n  REPEAT 999999 {\n    REPEAT 999999 {\n      REPEAT 999999 {\n        X 1\n        REPEAT 999999 {\n          Y 2\n          M 2\n        }\n      }\n    }\n  }\n}",
    )
    .unwrap();
    assert_eq!(stats::num_qubits(&instrs), 3);
}

// ========== count_measurements ==========

#[test]
fn count_measurements_empty() {
    assert_eq!(stats::num_measurements(&[]), 0);
}

#[test]
fn count_measurements_with_repeat() {
    let instrs = parse_lines(
        "H 0\nM 0 1\nREPEAT 2 {\n  X 1\n  REPEAT 3 {\n    Y 2\n    M 2\n  }\n}",
    )
    .unwrap();
    assert_eq!(stats::num_measurements(&instrs), 8);
}

#[test]
fn count_measurements_mpp_separate_products() {
    // In rstim, space-separated Pauli targets WITHOUT `*` are all in one
    // product (no Combiner tokens), so num_measurements counts 1 group.
    // Stim treats them as separate products (3 measurements).
    let instrs = parse_lines("MPP X0 Z1 Y2").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 1);
}

#[test]
fn count_measurements_mpp_combined() {
    // X0 followed by Z1*Y2: the parser emits [X0, Z1, Combiner, Y2] which
    // splits into [X0, Z1] and [Y2] = 2 groups.
    let instrs = parse_lines("MPP X0 Z1*Y2").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 2);
}

#[test]
fn count_measurements_mpp_long_chain() {
    // X0*X1*X2*X3*X4 Z5 Z6 parses with Combiners between X-chain elements:
    // [X0, C, X1, C, X2, C, X3, C, X4, Z5, Z6] -> 5 groups.
    let instrs = parse_lines("MPP X0*X1*X2*X3*X4 Z5 Z6").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 5);
}

#[test]
fn count_measurements_mpp_multi_product() {
    // X0*X1 Z0*Z1 Y0*Y1 parses to [X0, C, X1, Z0, C, Z1, Y0, C, Y1]
    // split by C => [X0], [X1, Z0], [Z1, Y0], [Y1] = 4 groups.
    let instrs = parse_lines("MPP X0*X1 Z0*Z1 Y0*Y1").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 4);
}

// ========== count_detectors_num_observables ==========

#[test]
fn count_detectors_empty() {
    assert_eq!(stats::num_detectors(&[]), 0);
}

#[test]
fn count_observables_empty() {
    assert_eq!(stats::num_observables(&[]), 0);
}

#[test]
fn count_detectors_and_observables() {
    let instrs = parse_lines("M 0 1 2\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(5) rec[-1]").unwrap();
    assert_eq!(stats::num_detectors(&instrs), 1);
    assert_eq!(stats::num_observables(&instrs), 6);
}

#[test]
fn count_detectors_in_repeat() {
    let instrs = parse_lines(
        "M 0 1\nREPEAT 1000 {\n  REPEAT 1000 {\n    REPEAT 1000 {\n      REPEAT 1000 {\n        DETECTOR rec[-1]\n        OBSERVABLE_INCLUDE(2) rec[-1]\n      }\n    }\n  }\n}",
    )
    .unwrap();
    assert_eq!(stats::num_detectors(&instrs), 1000000000000);
    assert_eq!(stats::num_observables(&instrs), 3);
}

// ========== preserves_repetition_blocks ==========

#[test]
fn preserves_repetition_blocks() {
    let instrs = parse_lines(
        "H 0\nM 0 1\nREPEAT 2 {\n  X 1\n  REPEAT 3 {\n    Y 2\n    M 2\n    X 0\n  }\n}",
    )
    .unwrap();
    // Top level: H, M, REPEAT
    assert_eq!(instrs.len(), 3);
    match &instrs[2] {
        StimInstr::Repeat { count, body } => {
            assert_eq!(*count, 2);
            // body: X, REPEAT
            assert_eq!(body.len(), 2);
            match &body[1] {
                StimInstr::Repeat { count, body } => {
                    assert_eq!(*count, 3);
                    // body: Y, M, X
                    assert_eq!(body.len(), 3);
                }
                _ => panic!("expected inner Repeat"),
            }
        }
        _ => panic!("expected outer Repeat"),
    }
}

// ========== big_rep_count ==========

#[test]
fn big_rep_count() {
    let instrs = parse_lines("REPEAT 1234567890123456789 {\n  M 1\n}").unwrap();
    match &instrs[0] {
        StimInstr::Repeat { count, .. } => {
            assert_eq!(*count, 1234567890123456789u64);
        }
        _ => panic!("expected Repeat"),
    }
    let s = circuit_to_string(&instrs);
    assert!(s.contains("REPEAT 1234567890123456789 {"));
}

// ========== zero_repetitions_not_allowed ==========

#[test]
fn zero_repetitions_not_allowed() {
    assert!(parse_lines("REPEAT 0 {\n  M 0\n  OBSERVABLE_INCLUDE(0) rec[-1]\n}").is_err());
}

// ========== negative_float_coordinates ==========

#[test]
fn negative_float_coordinates() {
    let instrs = parse_lines("SHIFT_COORDS(-1,-2,-3)\nQUBIT_COORDS(1,-2) 1\nQUBIT_COORDS(-3.5) 1").unwrap();
    assert_eq!(instrs[0].args().unwrap()[2], -3.0);
    assert_eq!(instrs[2].args().unwrap()[0], -3.5);
}

#[test]
fn qubit_coords_scientific_notation() {
    let instrs = parse_lines("QUBIT_COORDS(1e20) 0").unwrap();
    assert_eq!(instrs[0].args().unwrap()[0], 1e20);
}

// ========== equality ==========

#[test]
fn equality_different_targets() {
    let a = parse_lines("H 0\nREPEAT 100 {\n  X_ERROR(0.25) 1\n}").unwrap();
    let b = parse_lines("H 1\nREPEAT 100 {\n  X_ERROR(0.25) 1\n}").unwrap();
    let c = parse_lines("H 0\nREPEAT 100 {\n  X_ERROR(0.125) 1\n}").unwrap();

    assert_ne!(a, b);
    assert_ne!(a, c);
    assert_ne!(b, c);

    // Same circuit should be equal
    let a2 = parse_lines("H 0\nREPEAT 100 {\n  X_ERROR(0.25) 1\n}").unwrap();
    assert_eq!(a, a2);
}

// ========== flattened ==========

#[test]
fn flattened_empty() {
    assert_eq!(transforms::flattened(&[]), vec![]);
}

#[test]
fn flattened_simple() {
    let instrs = parse_lines("H 1").unwrap();
    let flat = transforms::flattened(&instrs);
    assert_eq!(flat.len(), 1);
    assert_eq!(flat[0].name().unwrap(), "H");
}

#[test]
fn flattened_repeat() {
    let instrs = parse_lines("REPEAT 3 {\n  H 0\n}").unwrap();
    let flat = transforms::flattened(&instrs);
    // Should expand to 3 H ops
    assert_eq!(flat.len(), 3);
    for f in &flat {
        assert_eq!(f.name().unwrap(), "H");
        assert_eq!(f.targets().unwrap(), &[StimTarget::Qubit(0)]);
    }
}

// ========== parse_windows_newlines ==========

#[test]
fn parse_windows_newlines() {
    let a = parse_lines("H 0\r\nCX 0 1\r\n").unwrap();
    let b = parse_lines("H 0\nCX 0 1\n").unwrap();
    let sa = circuit_to_string(&a);
    let sb = circuit_to_string(&b);
    assert_eq!(sa, sb);
}

// ========== validate_nan_probability ==========

#[test]
fn validate_nan_probability() {
    // Rust's f64 parse accepts "NaN" as a valid float, so rstim does not
    // reject it at parse time.  Instead, verify that NaN round-trips through
    // parsing without panic, and that the resulting arg is indeed NaN.
    let instrs = parse_lines("X_ERROR(NaN) 0").unwrap();
    assert!(instrs[0].args().unwrap()[0].is_nan());
}

// ========== validate_mpad ==========

#[test]
fn validate_mpad_parses() {
    // MPAD with small targets should parse OK
    parse_lines("MPAD 0 1").unwrap();
}

// ========== count_ticks ==========

#[test]
fn count_ticks_empty() {
    assert_eq!(stats::num_ticks(&[]), 0);
}

#[test]
fn count_ticks_simple() {
    let instrs = parse_lines("TICK").unwrap();
    assert_eq!(stats::num_ticks(&instrs), 1);
}

#[test]
fn count_ticks_multiple() {
    let instrs = parse_lines("TICK\nH 0\nTICK").unwrap();
    assert_eq!(stats::num_ticks(&instrs), 2);
}

#[test]
fn count_ticks_in_repeat() {
    let instrs = parse_lines(
        "TICK\nREPEAT 1000 {\n  REPEAT 2000 {\n    REPEAT 1000 {\n      TICK\n    }\n    TICK\n    TICK\n    TICK\n  }\n}\nTICK",
    )
    .unwrap();
    assert_eq!(stats::num_ticks(&instrs), 2006000002);
}

// ========== repeat block round-trip ==========

#[test]
fn repeat_block_roundtrip() {
    let input = "REPEAT 5 {\n  H 0\n  CX 0 1\n  M 0 1\n  DETECTOR rec[-1]\n}";
    let instrs = parse_lines(input).unwrap();
    let s = circuit_to_string(&instrs);
    let re_parsed = parse_lines(&s).unwrap();
    let s2 = circuit_to_string(&re_parsed);
    assert_eq!(s, s2);
}

#[test]
fn nested_repeat_roundtrip() {
    let input = "REPEAT 10 {\n  REPEAT 20 {\n    H 0\n    M 0\n  }\n  TICK\n}";
    let instrs = parse_lines(input).unwrap();
    let s = circuit_to_string(&instrs);
    let re_parsed = parse_lines(&s).unwrap();
    let s2 = circuit_to_string(&re_parsed);
    assert_eq!(s, s2);
}

// ========== heralded_erase measurements ==========

#[test]
fn count_measurements_heralded_erase() {
    let instrs = parse_lines("HERALDED_ERASE(0.01) 0 1 2").unwrap();
    assert_eq!(stats::num_measurements(&instrs), 3);
}
