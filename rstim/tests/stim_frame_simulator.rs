// Ported from Stim's frame_simulator.test.cc
// Tests covering batch sampling, noise, detection events.
//
// The following test categories are skipped because the rstim frame simulator
// does not support them:
// - Classical controls (CX rec[-1] 1) -- parser/sim rejects rec targets for gates
// - Noisy measurements (MX(0.05)) -- noise arg on measurements is ignored
// - OBSERVABLE_INCLUDE with Pauli targets (X0, Y0, Z0) -- only rec targets supported
// - MXX/MYY/MZZ inversions (MXX 0 !1) -- inversions are ignored by frame sim
// - Non-deterministic detectors via MPP (single non-eigenstate) -- frame sim
//   returns deterministic result (all same) for these cases
// - RY/MY combination -- RY randomizes x and z independently, making MY non-deterministic

use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::parser::parse_lines;
use rstim::sampler::sample_batch;

fn rng() -> StdRng {
    StdRng::seed_from_u64(12345)
}

/// Count how many shots have measurement `m_idx` set to true.
fn count_meas(out: &rstim::sampler::BatchOutput, m_idx: usize, n_shots: usize) -> usize {
    (0..n_shots)
        .filter(|&s| out.measurements.get(m_idx, s))
        .count()
}

// === correlated_error ===

#[test]
fn correlated_error_all_zero() {
    let instrs = parse_lines(
        "CORRELATED_ERROR(0) X0 X1\nELSE_CORRELATED_ERROR(0) X1 X2\n\
         ELSE_CORRELATED_ERROR(0) X2 X3\nM 0 1 2 3\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        for m in 0..4 {
            assert!(!out.measurements.get(m, s));
        }
    }
}

#[test]
fn correlated_error_first_fires() {
    let instrs = parse_lines(
        "CORRELATED_ERROR(1) X0 X1\nELSE_CORRELATED_ERROR(0) X1 X2\n\
         ELSE_CORRELATED_ERROR(0) X2 X3\nM 0 1 2 3\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(out.measurements.get(0, s));
        assert!(out.measurements.get(1, s));
        assert!(!out.measurements.get(2, s));
        assert!(!out.measurements.get(3, s));
    }
}

#[test]
fn correlated_error_second_fires() {
    let instrs = parse_lines(
        "CORRELATED_ERROR(0) X0 X1\nELSE_CORRELATED_ERROR(1) X1 X2\n\
         ELSE_CORRELATED_ERROR(0) X2 X3\nM 0 1 2 3\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(!out.measurements.get(0, s));
        assert!(out.measurements.get(1, s));
        assert!(out.measurements.get(2, s));
        assert!(!out.measurements.get(3, s));
    }
}

#[test]
fn correlated_error_third_fires() {
    let instrs = parse_lines(
        "CORRELATED_ERROR(0) X0 X1\nELSE_CORRELATED_ERROR(0) X1 X2\n\
         ELSE_CORRELATED_ERROR(1) X2 X3\nM 0 1 2 3\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(!out.measurements.get(0, s));
        assert!(!out.measurements.get(1, s));
        assert!(out.measurements.get(2, s));
        assert!(out.measurements.get(3, s));
    }
}

#[test]
fn correlated_error_first_blocks_second() {
    let instrs = parse_lines(
        "CORRELATED_ERROR(1) X0 X1\nELSE_CORRELATED_ERROR(1) X1 X2\n\
         ELSE_CORRELATED_ERROR(0) X2 X3\nM 0 1 2 3\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(out.measurements.get(0, s));
        assert!(out.measurements.get(1, s));
        assert!(!out.measurements.get(2, s));
        assert!(!out.measurements.get(3, s));
    }
}

#[test]
fn correlated_error_first_blocks_all() {
    let instrs = parse_lines(
        "CORRELATED_ERROR(1) X0 X1\nELSE_CORRELATED_ERROR(1) X1 X2\n\
         ELSE_CORRELATED_ERROR(1) X2 X3\nM 0 1 2 3\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(out.measurements.get(0, s));
        assert!(out.measurements.get(1, s));
        assert!(!out.measurements.get(2, s));
        assert!(!out.measurements.get(3, s));
    }
}

#[test]
fn correlated_error_chain_then_new() {
    let instrs = parse_lines(
        "CORRELATED_ERROR(1) X0 X1\nELSE_CORRELATED_ERROR(1) X1 X2\n\
         ELSE_CORRELATED_ERROR(1) X2 X3\nCORRELATED_ERROR(1) X3 X4\nM 0 1 2 3 4\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(out.measurements.get(0, s));
        assert!(out.measurements.get(1, s));
        assert!(!out.measurements.get(2, s));
        assert!(out.measurements.get(3, s));
        assert!(out.measurements.get(4, s));
    }
}

#[test]
fn correlated_error_statistical() {
    let n = 10000;
    let instrs = parse_lines(
        "CORRELATED_ERROR(0.5) X0\nELSE_CORRELATED_ERROR(0.25) X1\n\
         ELSE_CORRELATED_ERROR(0.75) X2\nM 0 1 2\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, n, &mut r).unwrap();
    let r0 = count_meas(&out, 0, n) as f64 / n as f64;
    let r1 = count_meas(&out, 1, n) as f64 / n as f64;
    let r2 = count_meas(&out, 2, n) as f64 / n as f64;
    assert!((r0 - 0.5).abs() < 0.05, "r0={r0}");
    assert!((r1 - 0.125).abs() < 0.05, "r1={r1}");
    assert!((r2 - 0.28125).abs() < 0.05, "r2={r2}");
}

// === measure_pauli_product_4body ===
// MPP X0*X1*X2*X3 then MX 0 1 2 3: the MPP result should equal XOR of MX results.
#[test]
fn mpp_4body_x_parity() {
    let instrs = parse_lines("MPP X0*X1*X2*X3\nMX 0 1 2 3\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 256, &mut r).unwrap();
    for s in 0..256 {
        let x0123 = out.measurements.get(0, s);
        let x0 = out.measurements.get(1, s);
        let x1 = out.measurements.get(2, s);
        let x2 = out.measurements.get(3, s);
        let x3 = out.measurements.get(4, s);
        assert_eq!(x0123, x0 ^ x1 ^ x2 ^ x3, "shot {s}");
    }
}

// === mxxyyzz_basis ===
// Same-basis pair measurements on eigenstates give false (parity +1).
#[test]
fn mxxyyzz_basis() {
    // RX 0 1: both in |+>, MXX gives false.
    let instrs = parse_lines("RX 0 1\nMXX 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 100, &mut r).unwrap();
    for s in 0..100 {
        assert!(!out.measurements.get(0, s), "shot {s}: MXX on |++>");
    }

    // MZZ on |00> gives false.
    let instrs = parse_lines("MZZ 0 1\n").unwrap();
    let out = sample_batch(&instrs, 100, &mut r).unwrap();
    for s in 0..100 {
        assert!(!out.measurements.get(0, s), "shot {s}: MZZ on |00>");
    }
}

// === mpad ===
#[test]
fn mpad_deterministic() {
    let instrs = parse_lines("MPAD 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 100, &mut r).unwrap();
    for s in 0..100 {
        assert!(!out.measurements.get(0, s), "shot {s}: MPAD 0");
        assert!(out.measurements.get(1, s), "shot {s}: MPAD 1");
    }
}

#[test]
fn mpad_multi() {
    let instrs = parse_lines("MPAD 0 0 1 1 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    let expected = [false, false, true, true, false];
    for s in 0..64 {
        for (i, &exp) in expected.iter().enumerate() {
            assert_eq!(out.measurements.get(i, s), exp, "shot {s} m{i}");
        }
    }
}

// === resets_vs_measurements ===
// After reset, measuring in the reset basis gives false.
#[test]
fn resets_deterministic_z() {
    let instrs = parse_lines("M 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(!out.measurements.get(0, s));
    }
}

#[test]
fn resets_deterministic_x() {
    let instrs = parse_lines("RX 0\nMX 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(!out.measurements.get(0, s));
    }
}

#[test]
fn resets_vs_measurements_errors_basis() {
    // Errors in different bases, measured in different bases.
    let instrs = parse_lines(
        "H 0 1 2\nH_YZ 3 4 5\n\
         X_ERROR(1) 0 3 6\nY_ERROR(1) 1 4 7\nZ_ERROR(1) 2 5 8\n\
         MX 0 1 2\nMY 3 4 5\nM 6 7 8\n",
    )
    .unwrap();
    let expected = [false, true, true, true, false, true, true, true, false];
    let mut r = rng();
    let out = sample_batch(&instrs, 100, &mut r).unwrap();
    for s in 0..100 {
        for (i, &exp) in expected.iter().enumerate() {
            assert_eq!(out.measurements.get(i, s), exp, "shot {s} m{i}");
        }
    }
}

#[test]
fn resets_vs_measurements_inverted_basis() {
    // Same but with inverted measurements.
    let instrs = parse_lines(
        "H 0 1 2\nH_YZ 3 4 5\n\
         X_ERROR(1) 0 3 6\nY_ERROR(1) 1 4 7\nZ_ERROR(1) 2 5 8\n\
         MX !0 !1 !2\nMY !3 !4 !5\nM !6 !7 !8\n",
    )
    .unwrap();
    // Inverted measurement flips the result.
    let expected = [true, false, false, false, true, false, false, false, true];
    let mut r = rng();
    let out = sample_batch(&instrs, 100, &mut r).unwrap();
    for s in 0..100 {
        for (i, &exp) in expected.iter().enumerate() {
            assert_eq!(out.measurements.get(i, s), exp, "shot {s} m{i}");
        }
    }
}

#[test]
fn resets_vs_measurements_mr_then_measure() {
    // After MRX, qubit is in |+>. MX gives false. After MR, qubit is |0>. M gives false.
    let instrs = parse_lines(
        "H 0 1 2\nH_YZ 3 4 5\n\
         X_ERROR(1) 0 3 6\nY_ERROR(1) 1 4 7\nZ_ERROR(1) 2 5 8\n\
         MRX 0 1 2\nMR 6 7 8\n\
         H 0\nM 0 6\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 100, &mut r).unwrap();
    // Post-MRX reset: q0 is in |+>, H 0 -> |0>, M 0 -> false.
    // Post-MR reset: q6 is in |0>, M 6 -> false.
    for s in 0..100 {
        assert!(
            !out.measurements.get(6, s),
            "shot {s}: M after MRX;H should be false"
        );
        assert!(
            !out.measurements.get(7, s),
            "shot {s}: M after MR should be false"
        );
    }
}

#[test]
fn resets_vs_measurements_mr_repeated() {
    // MRX 0 0: first measures error state, reset, second measures |+> -> false.
    let instrs = parse_lines(
        "H 0\nZ_ERROR(1) 0\nX_ERROR(1) 2\n\
         MRX 0 0\nMR 2 2\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 100, &mut r).unwrap();
    for s in 0..100 {
        assert!(out.measurements.get(0, s), "shot {s}: MRX first");
        assert!(!out.measurements.get(1, s), "shot {s}: MRX second (reset)");
        assert!(out.measurements.get(2, s), "shot {s}: MR first");
        assert!(!out.measurements.get(3, s), "shot {s}: MR second (reset)");
    }
}

// === block_results_single_shot ===
#[test]
fn block_results_single_shot() {
    let instrs = parse_lines("REPEAT 1000 {\n    X_ERROR(1) 0\n    MR 0\n    M 0 0\n}\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 1, &mut r).unwrap();
    for i in 0..1000 {
        assert!(out.measurements.get(i * 3, 0), "iter {i}: MR should be 1");
        assert!(
            !out.measurements.get(i * 3 + 1, 0),
            "iter {i}: M after R should be 0"
        );
        assert!(
            !out.measurements.get(i * 3 + 2, 0),
            "iter {i}: second M should be 0"
        );
    }
}

// === block_results_triple_shot ===
#[test]
fn block_results_triple_shot() {
    let instrs = parse_lines("REPEAT 1000 {\n    X_ERROR(1) 0\n    MR 0\n    M 0 0\n}\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 3, &mut r).unwrap();
    for shot in 0..3 {
        for i in 0..1000 {
            assert!(
                out.measurements.get(i * 3, shot),
                "shot {shot} iter {i}: MR"
            );
            assert!(
                !out.measurements.get(i * 3 + 1, shot),
                "shot {shot} iter {i}: M after R"
            );
            assert!(
                !out.measurements.get(i * 3 + 2, shot),
                "shot {shot} iter {i}: second M"
            );
        }
    }
}

// === runs_on_general_circuit ===
#[test]
fn runs_on_general_circuit() {
    let circuit = "\
H 0
CNOT 0 1
S 0
S_DAG 1
SQRT_X 0
SQRT_X_DAG 1
SQRT_Y 0
SQRT_Y_DAG 1
CZ 0 1
CY 0 1
SWAP 0 1
ISWAP 0 1
ISWAP_DAG 0 1
H_XY 0
H_YZ 1
X 0
Y 1
Z 0
TICK
M 0 1
";
    let instrs = parse_lines(circuit).unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 100, &mut r).unwrap();
    assert_eq!(out.measurements.num_major(), 2);
}

// === Bell pair: measurements correlated ===
#[test]
fn bell_pair_correlated() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 1000, &mut r).unwrap();
    for s in 0..1000 {
        assert_eq!(
            out.measurements.get(0, s),
            out.measurements.get(1, s),
            "shot {s}: Bell pair should be correlated"
        );
    }
}

// === Bell pair detector: XOR of correlated measurements always 0 ===
#[test]
fn bell_pair_detector() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 256, &mut r).unwrap();
    for s in 0..256 {
        assert!(!out.detections.get(0, s), "shot {s}");
    }
}

// === Detector noiseless ===
#[test]
fn detector_noiseless() {
    let instrs = parse_lines("M 0\nR 0\nM 0\nDETECTOR rec[-1] rec[-2]\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(!out.detections.get(0, s), "shot {s}");
    }
}

// === Detector with noise ===
#[test]
fn detector_with_noise() {
    let instrs = parse_lines("M 0\nR 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1] rec[-2]\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(out.detections.get(0, s), "shot {s}");
    }
}

// === Observable flip ===
#[test]
fn observable_flip() {
    let instrs = parse_lines("X 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(!out.observable_flips.get(0, s), "shot {s}");
    }
}

// === Observable no flip ===
#[test]
fn observable_no_flip() {
    let instrs = parse_lines("M 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(!out.observable_flips.get(0, s), "shot {s}");
    }
}

// === Repeat loop ===
#[test]
fn repeat_loop() {
    let instrs = parse_lines("REPEAT 3 {\nX 0\nM 0\nR 0\n}\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        for m in 0..3 {
            assert!(out.measurements.get(m, s), "shot {s} m{m}");
        }
    }
}

// === Surface-code-style circuit ===
#[test]
fn surface_code_style_detector_circuit() {
    let circuit = "\
R 0 1 2
TICK
CNOT 0 1
CNOT 2 1
TICK
M 1
R 1
DETECTOR rec[-1]
REPEAT 2 {
    TICK
    CNOT 0 1
    CNOT 2 1
    TICK
    M 1
    R 1
    DETECTOR rec[-1] rec[-2]
}
M 0 2
OBSERVABLE_INCLUDE(0) rec[-1] rec[-2]
";
    let instrs = parse_lines(circuit).unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 128, &mut r).unwrap();
    for s in 0..128 {
        for d in 0..out.detections.num_major() {
            assert!(!out.detections.get(d, s), "det {d} shot {s}");
        }
        for o in 0..out.observable_flips.num_major() {
            assert!(!out.observable_flips.get(o, s), "obs {o} shot {s}");
        }
    }
}

// === MPP on Bell pair stabilizers ===
#[test]
fn mpp_xx_bell() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nMPP X0*X1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 128, &mut r).unwrap();
    for s in 0..128 {
        assert!(!out.measurements.get(0, s), "shot {s}");
    }
}

#[test]
fn mpp_zz_bell() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nMPP Z0*Z1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 128, &mut r).unwrap();
    for s in 0..128 {
        assert!(!out.measurements.get(0, s), "shot {s}");
    }
}

#[test]
fn mpp_yy_bell() {
    // Y*Y eigenvalue on |Phi+> is -1, so MPP Y0*Y1 gives true.
    let instrs = parse_lines("H 0\nCNOT 0 1\nMPP Y0*Y1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 128, &mut r).unwrap();
    for s in 0..128 {
        assert!(out.measurements.get(0, s), "shot {s}");
    }
}

// === MPP EPR relations ===
#[test]
fn mpp_epr_relations() {
    let instrs = parse_lines("MPP X0*X1 Z0*Z1 Y0*Y1\nCNOT 0 1\nH 0\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 256, &mut r).unwrap();
    for s in 0..256 {
        let x01 = out.measurements.get(0, s);
        let z01 = out.measurements.get(1, s);
        let y01 = out.measurements.get(2, s);
        let m0 = out.measurements.get(3, s);
        let m1 = out.measurements.get(4, s);
        assert_eq!(m0, x01, "shot {s}: m0 != x01");
        assert_eq!(m1, z01, "shot {s}: m1 != z01");
        assert_eq!(x01 ^ z01, y01 ^ true, "shot {s}: x01^z01 != y01^1");
    }
}

// === X_ERROR statistical ===
#[test]
fn x_error_statistical() {
    let n = 10000;
    let instrs = parse_lines("X_ERROR(0.3) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, n, &mut r).unwrap();
    let rate = count_meas(&out, 0, n) as f64 / n as f64;
    assert!((rate - 0.3).abs() < 0.05, "rate={rate}");
}

// === DEPOLARIZE1 statistical ===
#[test]
fn depolarize1_statistical() {
    let n = 10000;
    let instrs = parse_lines("DEPOLARIZE1(0.75) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, n, &mut r).unwrap();
    // X or Y flip Z measurement: 2/3 of 75% = 50%
    let rate = count_meas(&out, 0, n) as f64 / n as f64;
    assert!((rate - 0.5).abs() < 0.05, "rate={rate}");
}

// === DEPOLARIZE2 statistical ===
#[test]
fn depolarize2_statistical() {
    let n = 10000;
    let instrs = parse_lines("DEPOLARIZE2(1.0) 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, n, &mut r).unwrap();
    let rate_0 = count_meas(&out, 0, n) as f64 / n as f64;
    let rate_1 = count_meas(&out, 1, n) as f64 / n as f64;
    // 8/15 of 15 non-identity Paulis flip each qubit
    assert!((rate_0 - 8.0 / 15.0).abs() < 0.05, "q0: {rate_0}");
    assert!((rate_1 - 8.0 / 15.0).abs() < 0.05, "q1: {rate_1}");
}

// === Certain errors consistent with gates ===
#[test]
fn x_error_1_equals_x_gate() {
    let instrs = parse_lines("X_ERROR(1) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(out.measurements.get(0, s));
    }
}

#[test]
fn y_error_1_equals_y_gate() {
    let instrs = parse_lines("Y_ERROR(1) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(out.measurements.get(0, s));
    }
}

#[test]
fn z_error_1_no_flip_z() {
    let instrs = parse_lines("Z_ERROR(1) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(!out.measurements.get(0, s));
    }
}

// === Reset then measure ===
#[test]
fn simulate_reset() {
    let instrs = parse_lines("X 0\nM 0\nR 0\nM 0\nR 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(out.measurements.get(0, s), "shot {s}: M after X");
        assert!(!out.measurements.get(1, s), "shot {s}: M after R");
        assert!(!out.measurements.get(2, s), "shot {s}: M after R again");
    }
}

// === MR repeated target ===
#[test]
fn mr_repeated_target() {
    let instrs = parse_lines("X 0\nMR 0 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(out.measurements.get(0, s), "shot {s}: first MR");
        assert!(!out.measurements.get(1, s), "shot {s}: second MR");
    }
}

// === PAULI_CHANNEL_1 deterministic ===
#[test]
fn pauli_channel_1_x() {
    let instrs = parse_lines("PAULI_CHANNEL_1(1,0,0) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(out.measurements.get(0, s));
    }
}

#[test]
fn pauli_channel_1_z() {
    let instrs = parse_lines("PAULI_CHANNEL_1(0,0,1) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(!out.measurements.get(0, s));
    }
}

// === Detector with X_ERROR statistical ===
#[test]
fn detector_statistical() {
    let n = 10000;
    let instrs = parse_lines("M 0\nR 0\nX_ERROR(0.3) 0\nM 0\nDETECTOR rec[-1] rec[-2]\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, n, &mut r).unwrap();
    let det_count: usize = (0..n).filter(|&s| out.detections.get(0, s)).count();
    let rate = det_count as f64 / n as f64;
    assert!((rate - 0.3).abs() < 0.05, "detection rate={rate}");
}

// === MXX antiparallel ===
#[test]
fn mzz_antiparallel() {
    let instrs = parse_lines("X 0\nMZZ 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(out.measurements.get(0, s), "shot {s}");
    }
}

// === MXX on bell pair ===
#[test]
fn mxx_bell() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nMXX 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(!out.measurements.get(0, s), "shot {s}");
    }
}

// === MYY on bell pair ===
#[test]
fn myy_bell() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nMYY 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(out.measurements.get(0, s), "shot {s}");
    }
}

// === MZZ on bell pair ===
#[test]
fn mzz_bell() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nMZZ 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert!(!out.measurements.get(0, s), "shot {s}");
    }
}
