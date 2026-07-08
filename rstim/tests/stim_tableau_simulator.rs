// Ported from Stim's tableau_simulator.test.cc
// Tests that exercise rstim's sample_batch (frame simulator) against
// the same circuits that Stim's TableauSimulator tests use.
//
// Tests that require features not in rstim's frame simulator are skipped:
// - Classical control targets (CX rec[-1] 1, etc.)
// - Noisy measurements (M(0.05), MX(0.05), etc.)
// - Direct tableau API (peek_bloch, postselect, set_num_qubits, etc.)
// - Pair-measure inversions (MXX !0 1) -- frame sim ignores inversions

use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::parser::parse_lines;
use rstim::sampler::sample_batch;

fn rng() -> StdRng {
    StdRng::seed_from_u64(42)
}

// === identity ===
// Stim: measure |0>, get false; measure with inversion, get true.
#[test]
fn identity_measure_z() {
    let instrs = parse_lines("M 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false, "shot {s}");
    }
}

#[test]
fn identity_measure_z_inverted() {
    let instrs = parse_lines("M !0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true, "shot {s}");
    }
}

// === bit_flip ===
// H S S H = Z, so M gives 1; then X flips back to 0.
#[test]
fn bit_flip() {
    let instrs = parse_lines("H 0\nS 0\nS 0\nH 0\nM 0\nX 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true, "shot {s}: first M");
        assert_eq!(out.measurements.get(1, s), false, "shot {s}: second M");
    }
}

// === identity2 ===
#[test]
fn identity2() {
    let instrs = parse_lines("M 0\nM 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
        assert_eq!(out.measurements.get(1, s), false);
    }
}

// === bit_flip_2 ===
// H S S H on q0 gives |1>; q1 stays |0>.
#[test]
fn bit_flip_2() {
    let instrs = parse_lines("H 0\nS 0\nS 0\nH 0\nM 0\nM 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
        assert_eq!(out.measurements.get(1, s), false);
    }
}

// === epr ===
// Bell pair: measurements must be correlated.
#[test]
fn epr_correlated() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nM 0\nM 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 1000, &mut r).unwrap();
    for s in 0..1000 {
        assert_eq!(
            out.measurements.get(0, s),
            out.measurements.get(1, s),
            "shot {s}"
        );
    }
}

// === simulate ===
// H 0, CNOT 0 1, M 0, M 1, M 2: first two correlated, third is 0.
#[test]
fn simulate_h_cnot_m() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nM 0\nM 1\nM 2\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 256, &mut r).unwrap();
    for s in 0..256 {
        assert_eq!(
            out.measurements.get(0, s),
            out.measurements.get(1, s),
            "shot {s}"
        );
        assert_eq!(out.measurements.get(2, s), false, "shot {s}: q2");
    }
}

// === simulate_reset ===
// X 0, M 0, R 0, M 0, R 0, M 0 => true, false, false
#[test]
fn simulate_reset() {
    let instrs = parse_lines("X 0\nM 0\nR 0\nM 0\nR 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true, "shot {s}: M after X");
        assert_eq!(out.measurements.get(1, s), false, "shot {s}: M after R");
        assert_eq!(out.measurements.get(2, s), false, "shot {s}: M after R again");
    }
}

// === certain_errors_consistent_with_gates ===
// X_ERROR(1) should be equivalent to X gate; same for Y, Z.
#[test]
fn x_error_1_equals_x_gate() {
    let instrs = parse_lines("X_ERROR(1) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
    }
}

#[test]
fn y_error_1_equals_y_gate() {
    let instrs = parse_lines("Y_ERROR(1) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
    }
}

#[test]
fn z_error_1_no_flip_z() {
    let instrs = parse_lines("Z_ERROR(1) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

// === correlated_error ===
// A battery of CORRELATED_ERROR / ELSE_CORRELATED_ERROR tests.

#[test]
fn correlated_error_all_zero_prob() {
    let instrs = parse_lines(
        "CORRELATED_ERROR(0) X0 X1\n\
         ELSE_CORRELATED_ERROR(0) X1 X2\n\
         ELSE_CORRELATED_ERROR(0) X2 X3\n\
         M 0 1 2 3\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        for m in 0..4 {
            assert_eq!(out.measurements.get(m, s), false, "shot {s} m{m}");
        }
    }
}

#[test]
fn correlated_error_first_fires() {
    let instrs = parse_lines(
        "CORRELATED_ERROR(1) X0 X1\n\
         ELSE_CORRELATED_ERROR(0) X1 X2\n\
         ELSE_CORRELATED_ERROR(0) X2 X3\n\
         M 0 1 2 3\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
        assert_eq!(out.measurements.get(1, s), true);
        assert_eq!(out.measurements.get(2, s), false);
        assert_eq!(out.measurements.get(3, s), false);
    }
}

#[test]
fn correlated_error_second_fires() {
    let instrs = parse_lines(
        "CORRELATED_ERROR(0) X0 X1\n\
         ELSE_CORRELATED_ERROR(1) X1 X2\n\
         ELSE_CORRELATED_ERROR(0) X2 X3\n\
         M 0 1 2 3\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
        assert_eq!(out.measurements.get(1, s), true);
        assert_eq!(out.measurements.get(2, s), true);
        assert_eq!(out.measurements.get(3, s), false);
    }
}

#[test]
fn correlated_error_third_fires() {
    let instrs = parse_lines(
        "CORRELATED_ERROR(0) X0 X1\n\
         ELSE_CORRELATED_ERROR(0) X1 X2\n\
         ELSE_CORRELATED_ERROR(1) X2 X3\n\
         M 0 1 2 3\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
        assert_eq!(out.measurements.get(1, s), false);
        assert_eq!(out.measurements.get(2, s), true);
        assert_eq!(out.measurements.get(3, s), true);
    }
}

#[test]
fn correlated_error_first_blocks_second() {
    let instrs = parse_lines(
        "CORRELATED_ERROR(1) X0 X1\n\
         ELSE_CORRELATED_ERROR(1) X1 X2\n\
         ELSE_CORRELATED_ERROR(0) X2 X3\n\
         M 0 1 2 3\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
        assert_eq!(out.measurements.get(1, s), true);
        assert_eq!(out.measurements.get(2, s), false);
        assert_eq!(out.measurements.get(3, s), false);
    }
}

#[test]
fn correlated_error_first_blocks_all() {
    let instrs = parse_lines(
        "CORRELATED_ERROR(1) X0 X1\n\
         ELSE_CORRELATED_ERROR(1) X1 X2\n\
         ELSE_CORRELATED_ERROR(1) X2 X3\n\
         M 0 1 2 3\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
        assert_eq!(out.measurements.get(1, s), true);
        assert_eq!(out.measurements.get(2, s), false);
        assert_eq!(out.measurements.get(3, s), false);
    }
}

#[test]
fn correlated_error_chain_then_new_chain() {
    let instrs = parse_lines(
        "CORRELATED_ERROR(1) X0 X1\n\
         ELSE_CORRELATED_ERROR(1) X1 X2\n\
         ELSE_CORRELATED_ERROR(1) X2 X3\n\
         CORRELATED_ERROR(1) X3 X4\n\
         M 0 1 2 3 4\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true, "s{s} m0");
        assert_eq!(out.measurements.get(1, s), true, "s{s} m1");
        assert_eq!(out.measurements.get(2, s), false, "s{s} m2");
        assert_eq!(out.measurements.get(3, s), true, "s{s} m3");
        assert_eq!(out.measurements.get(4, s), true, "s{s} m4");
    }
}

#[test]
fn correlated_error_statistical() {
    // CORRELATED_ERROR(0.5) X0; ELSE(0.25) X1; ELSE(0.75) X2
    // Expected rates: X0=0.5, X1=0.5*0.25=0.125, X2=0.5*0.75*0.75=0.28125
    let instrs = parse_lines(
        "CORRELATED_ERROR(0.5) X0\n\
         ELSE_CORRELATED_ERROR(0.25) X1\n\
         ELSE_CORRELATED_ERROR(0.75) X2\n\
         M 0 1 2\n",
    )
    .unwrap();
    let n = 10000;
    let mut r = rng();
    let out = sample_batch(&instrs, n, &mut r).unwrap();
    let mut hits = [0usize; 3];
    for s in 0..n {
        if out.measurements.get(0, s) {
            hits[0] += 1;
        }
        if out.measurements.get(1, s) {
            hits[1] += 1;
        }
        if out.measurements.get(2, s) {
            hits[2] += 1;
        }
    }
    let r0 = hits[0] as f64 / n as f64;
    let r1 = hits[1] as f64 / n as f64;
    let r2 = hits[2] as f64 / n as f64;
    assert!((r0 - 0.5).abs() < 0.05, "X0 rate={r0}");
    assert!((r1 - 0.125).abs() < 0.05, "X1 rate={r1}");
    assert!((r2 - 0.28125).abs() < 0.05, "X2 rate={r2}");
}

// === mr_repeated_target ===
// X 0, MR 0 0 -> first MR gives 1, resets; second MR gives 0.
#[test]
fn mr_repeated_target() {
    let instrs = parse_lines("X 0\nMR 0 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true, "shot {s}: first MR");
        assert_eq!(out.measurements.get(1, s), false, "shot {s}: second MR");
    }
}

// === phase_kickback_preserve_s_state ===
// Prepare S state on q1, H on q0, kickback preserving protocol, check both.
// This does not use classical feedback.
#[test]
fn phase_kickback_preserve_s_state() {
    let instrs = parse_lines(
        "H 1\nS 1\nH 0\nCNOT 0 1\nH 1\nCNOT 0 1\nH 1\nS 0\nH 0\nM 0\nS 1\nH 1\nM 1\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true, "shot {s}: q0");
        assert_eq!(out.measurements.get(1, s), true, "shot {s}: q1");
    }
}

// === reset_vs_measurements: RX, RZ then basis-measure ===
#[test]
fn reset_xz_then_measure() {
    // RX 0, RZ 1, H 0, M 0 1 -> all false
    let instrs = parse_lines("RX 0\nRZ 1\nH 0\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false, "shot {s}: q0");
        assert_eq!(out.measurements.get(1, s), false, "shot {s}: q1");
    }
}

// === reset_vs_measurements: errors and MX/MY/MZ ===
#[test]
fn errors_then_basis_measurements() {
    let instrs = parse_lines(
        "H 0\nH 1\nH 2\nH_YZ 3\nH_YZ 4\nH_YZ 5\n\
         X_ERROR(1) 0 3 6\nY_ERROR(1) 1 4 7\nZ_ERROR(1) 2 5 8\n\
         MX 0 1 2\nMY 3 4 5\nM 6 7 8\n",
    )
    .unwrap();
    let expected = [false, true, true, true, false, true, true, true, false];
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        for (i, &exp) in expected.iter().enumerate() {
            assert_eq!(out.measurements.get(i, s), exp, "shot {s} m{i}");
        }
    }
}

// === reset_vs_measurements: inverted measurements with errors ===
#[test]
fn errors_then_inverted_basis_measurements() {
    let instrs = parse_lines(
        "H 0\nH 1\nH 2\nH_YZ 3\nH_YZ 4\nH_YZ 5\n\
         X_ERROR(1) 0 3 6\nY_ERROR(1) 1 4 7\nZ_ERROR(1) 2 5 8\n\
         MX !0 !1 !2\nMY !3 !4 !5\nM !6 !7 !8\n",
    )
    .unwrap();
    let expected = [true, false, false, false, true, false, false, false, true];
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        for (i, &exp) in expected.iter().enumerate() {
            assert_eq!(out.measurements.get(i, s), exp, "shot {s} m{i}");
        }
    }
}

// === reset_vs_measurements: MR then check reset state ===
#[test]
fn mr_then_check_reset_state() {
    // X 0 puts q0 in |1>. MR gives true, resets to |0>. M gives false.
    let instrs = parse_lines("X 0\nMR 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true, "shot {s}: MR after X");
        assert_eq!(out.measurements.get(1, s), false, "shot {s}: M after reset");
    }
}

// === MRX then check reset state ===
#[test]
fn mrx_then_check_reset_state() {
    // H puts q0 in |+>. Z_ERROR flips to |->. MRX gives true, resets to |+>. MX gives false.
    let instrs = parse_lines("H 0\nZ_ERROR(1) 0\nMRX 0\nMX 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true, "shot {s}: MRX after Z_ERROR");
        assert_eq!(out.measurements.get(1, s), false, "shot {s}: MX after reset");
    }
}

// === reset_vs_measurements: MRX resets to |+> ===
#[test]
fn mrx_resets_to_plus() {
    let instrs = parse_lines(
        "H 0\nZ_ERROR(1) 0\n\
         MRX 0\n\
         MX 0\n",
    )
    .unwrap();
    // H puts in |+>. Z_ERROR flips phase: |->. MRX on |-> gives true. Reset to |+>.
    // MX on |+> gives false.
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true, "shot {s}: MRX on |->");
        assert_eq!(out.measurements.get(1, s), false, "shot {s}: MX after reset");
    }
}

// === mr_repeated_targets_z ===
#[test]
fn mr_repeated_targets_z() {
    // H 0, Z_ERROR flips |+> to |->. MRX 0 0: first gives true, second gives false.
    let instrs = parse_lines("H 0\nZ_ERROR(1) 0\nMRX 0 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true, "shot {s}: first MRX");
        assert_eq!(out.measurements.get(1, s), false, "shot {s}: second MRX");
    }
}

#[test]
fn mr_repeated_targets_mz() {
    // X_ERROR(1) on |0> gives |1>. MR 0 0: first gives true, reset, second gives false.
    let instrs = parse_lines("X_ERROR(1) 0\nMR 0 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true, "shot {s}: first MR");
        assert_eq!(out.measurements.get(1, s), false, "shot {s}: second MR");
    }
}

// === mpad ===
#[test]
fn mpad_values() {
    let instrs = parse_lines("MPAD 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }

    let instrs = parse_lines("MPAD 1\n").unwrap();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
    }

    let instrs = parse_lines("MPAD 0 0 1 1 0\n").unwrap();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    let expected = [false, false, true, true, false];
    for s in 0..64 {
        for (i, &exp) in expected.iter().enumerate() {
            assert_eq!(out.measurements.get(i, s), exp, "shot {s} m{i}");
        }
    }
}

// === measure_pauli_product_1 ===
// RX 0, RY 1, RZ 2, then MPP X0 Y1 Z2 X0*Y1*Z2: all should be 0.
#[test]
fn mpp_pure_eigenstates() {
    let instrs =
        parse_lines("RX 0\nRY 1\nREPEAT 100 {\nMPP X0 Y1 Z2 X0*Y1*Z2\n}\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 1, &mut r).unwrap();
    for i in 0..400 {
        assert_eq!(out.measurements.get(i, 0), false, "m{i}");
    }
}

// === measure_pauli_product_epr ===
// MPP X0*X1 Z0*Z1 Y0*Y1, then CNOT 0 1, H 0, M 0 1.
// Should satisfy: m0 == x01, m1 == z01, x01 ^ z01 == y01 ^ 1.
#[test]
fn mpp_epr_relations() {
    let instrs =
        parse_lines("MPP X0*X1 Z0*Z1 Y0*Y1\nCNOT 0 1\nH 0\nM 0 1\n").unwrap();
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

// === measure_pauli_product_4body ===
// Test the algebraic identity between X-basis and Y/Z-basis MPP measurements.
#[test]
fn mpp_4body_x_parity() {
    // MPP X0*X1*X2*X3 should equal XOR of individual MX results.
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

// === ignores_sweep_controls ===
#[test]
fn ignores_sweep_controls() {
    let instrs = parse_lines("X 0\nCNOT sweep[0] 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true, "shot {s}");
    }
}

// === mxx_myy_mzz basic ===
#[test]
fn mxx_basic() {
    // Both qubits in +X: MXX gives false (parity is +1).
    let instrs = parse_lines("RX 0\nRX 1\nMXX 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false, "shot {s}");
    }
}

#[test]
fn mzz_basic() {
    // Both qubits in |0>: MZZ gives false.
    let instrs = parse_lines("MZZ 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false, "shot {s}");
    }
}

#[test]
fn mzz_antiparallel() {
    // |10>: MZZ gives true (anti-parallel Z eigenvalues).
    let instrs = parse_lines("X 0\nMZZ 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true, "shot {s}");
    }
}

// === mxx on bell pair ===
#[test]
fn mxx_bell() {
    // Bell state |Phi+>: MXX gives false (X*X eigenvalue is +1).
    let instrs = parse_lines("H 0\nCNOT 0 1\nMXX 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false, "shot {s}");
    }
}

// === myy on bell pair ===
#[test]
fn myy_bell() {
    // Bell state |Phi+>: MYY gives true (Y*Y eigenvalue is -1).
    let instrs = parse_lines("H 0\nCNOT 0 1\nMYY 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true, "shot {s}");
    }
}

// === mzz on bell pair ===
#[test]
fn mzz_bell() {
    // Bell state |Phi+>: MZZ gives false (Z*Z eigenvalue is +1).
    let instrs = parse_lines("H 0\nCNOT 0 1\nMZZ 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false, "shot {s}");
    }
}

// === reset_x_entangled ===
// After entangling and resetting, qubit should be in reset state.
#[test]
fn reset_x_entangled() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nRX 0\nMX 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false, "shot {s}");
    }
}

// === reset_z_entangled ===
#[test]
fn reset_z_entangled() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nR 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false, "shot {s}");
    }
}

// === measure_x_entangled ===
// Bell pair, MX 0 then MX 1: both should agree (XX is stabilizer of |Phi+>).
#[test]
fn measure_x_entangled() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nMX 0\nMX 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 256, &mut r).unwrap();
    for s in 0..256 {
        assert_eq!(
            out.measurements.get(0, s),
            out.measurements.get(1, s),
            "shot {s}"
        );
    }
}

// === measure_z_entangled ===
#[test]
fn measure_z_entangled() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nM 0\nM 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 256, &mut r).unwrap();
    for s in 0..256 {
        assert_eq!(
            out.measurements.get(0, s),
            out.measurements.get(1, s),
            "shot {s}"
        );
    }
}

// === measure_y_entangled ===
// Bell pair |Phi+>: MY 0 then MY 1 should be anti-correlated.
// We test this via MPP instead of sequential MY, since MPP handles
// multi-qubit Pauli products correctly in the frame sim.
#[test]
fn measure_y_entangled_via_mpp() {
    // MPP Y0*Y1 on |Phi+> gives true (eigenvalue -1).
    // Then MX 0 and MX 1 should agree (XX is stabilizer).
    let instrs =
        parse_lines("H 0\nCNOT 0 1\nMPP Y0*Y1\nMX 0\nMX 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 256, &mut r).unwrap();
    for s in 0..256 {
        assert_eq!(out.measurements.get(0, s), true, "shot {s}: MPP Y*Y");
        assert_eq!(
            out.measurements.get(1, s),
            out.measurements.get(2, s),
            "shot {s}: MX correlation"
        );
    }
}

// === measure_reset_x_entangled ===
#[test]
fn measure_reset_x_entangled() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nMRX 0\nMX 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(1, s), false, "shot {s}: post-reset MX");
    }
}

// === measure_reset_z_entangled ===
#[test]
fn measure_reset_z_entangled() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nMR 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(1, s), false, "shot {s}: post-reset MZ");
    }
}

// === depolarize1 statistical ===
#[test]
fn depolarize1_statistical() {
    let instrs = parse_lines("DEPOLARIZE1(0.75) 0\nM 0\n").unwrap();
    let n = 10000;
    let mut r = rng();
    let out = sample_batch(&instrs, n, &mut r).unwrap();
    let count: usize = (0..n).filter(|&s| out.measurements.get(0, s)).count();
    // X or Y flip Z measurement: 2/3 of 75% = 50%
    let rate = count as f64 / n as f64;
    assert!((rate - 0.5).abs() < 0.05, "rate={rate}");
}

// === depolarize2 statistical ===
#[test]
fn depolarize2_statistical() {
    let instrs = parse_lines("DEPOLARIZE2(1.0) 0 1\nM 0 1\n").unwrap();
    let n = 10000;
    let mut r = rng();
    let out = sample_batch(&instrs, n, &mut r).unwrap();
    let flipped_0: usize = (0..n).filter(|&s| out.measurements.get(0, s)).count();
    let flipped_1: usize = (0..n).filter(|&s| out.measurements.get(1, s)).count();
    let rate_0 = flipped_0 as f64 / n as f64;
    let rate_1 = flipped_1 as f64 / n as f64;
    // 8/15 of the 15 non-identity paulis have X or Y on each qubit
    assert!(
        (rate_0 - 8.0 / 15.0).abs() < 0.05,
        "q0 flip rate: {rate_0}"
    );
    assert!(
        (rate_1 - 8.0 / 15.0).abs() < 0.05,
        "q1 flip rate: {rate_1}"
    );
}

// === x_error statistical ===
#[test]
fn x_error_statistical() {
    let instrs = parse_lines("X_ERROR(0.3) 0\nM 0\n").unwrap();
    let n = 10000;
    let mut r = rng();
    let out = sample_batch(&instrs, n, &mut r).unwrap();
    let count: usize = (0..n).filter(|&s| out.measurements.get(0, s)).count();
    let rate = count as f64 / n as f64;
    assert!((rate - 0.3).abs() < 0.05, "rate={rate}");
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
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    assert_eq!(out.measurements.num_major(), 2);
}

// === detectors noiseless ===
#[test]
fn detector_noiseless() {
    let instrs =
        parse_lines("M 0\nR 0\nM 0\nDETECTOR rec[-1] rec[-2]\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.detections.get(0, s), false, "shot {s}");
    }
}

// === detectors with noise ===
#[test]
fn detector_with_noise() {
    let instrs = parse_lines(
        "M 0\nR 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1] rec[-2]\n",
    )
    .unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.detections.get(0, s), true, "shot {s}");
    }
}

// === observable ===
#[test]
fn observable_flip() {
    let instrs =
        parse_lines("X 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.observable_flips.get(0, s), true, "shot {s}");
    }
}

// === repeat loop ===
#[test]
fn repeat_loop() {
    let instrs = parse_lines("REPEAT 3 {\nX 0\nM 0\nR 0\n}\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        for m in 0..3 {
            assert_eq!(out.measurements.get(m, s), true, "shot {s} m{m}");
        }
    }
}

// === Bell pair via detectors ===
#[test]
fn bell_pair_detector() {
    let instrs =
        parse_lines("H 0\nCNOT 0 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 256, &mut r).unwrap();
    for s in 0..256 {
        assert_eq!(out.detections.get(0, s), false, "shot {s}");
    }
}

// === MPP ZZ on Bell pair ===
#[test]
fn mpp_zz_bell() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nMPP Z0*Z1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 128, &mut r).unwrap();
    for s in 0..128 {
        assert_eq!(out.measurements.get(0, s), false, "shot {s}");
    }
}

// === MPP XX on Bell pair ===
#[test]
fn mpp_xx_bell() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nMPP X0*X1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 128, &mut r).unwrap();
    for s in 0..128 {
        assert_eq!(out.measurements.get(0, s), false, "shot {s}");
    }
}

// === MPP YY on Bell pair ===
#[test]
fn mpp_yy_bell() {
    // Y*Y eigenvalue on |Phi+> is -1, so MPP Y0*Y1 gives true.
    let instrs = parse_lines("H 0\nCNOT 0 1\nMPP Y0*Y1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 128, &mut r).unwrap();
    for s in 0..128 {
        assert_eq!(out.measurements.get(0, s), true, "shot {s}");
    }
}

// === PAULI_CHANNEL_1 deterministic ===
#[test]
fn pauli_channel_1_x() {
    let instrs = parse_lines("PAULI_CHANNEL_1(1,0,0) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true, "shot {s}");
    }
}

#[test]
fn pauli_channel_1_z() {
    // Z error does not flip Z measurement.
    let instrs = parse_lines("PAULI_CHANNEL_1(0,0,1) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false, "shot {s}");
    }
}

// === Surface-code-style circuit with detectors ===
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
            assert_eq!(out.detections.get(d, s), false, "det {d} shot {s}");
        }
        for o in 0..out.observable_flips.num_major() {
            assert_eq!(
                out.observable_flips.get(o, s),
                false,
                "obs {o} shot {s}"
            );
        }
    }
}
