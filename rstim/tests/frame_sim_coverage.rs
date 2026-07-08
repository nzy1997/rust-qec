use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::executor::reference_sample;
use rstim::parser::parse_lines;
use rstim::sampler::sample_batch;
use rstim::sim::bit_table::BitTable;
use rstim::sim::frame::FrameSimulator;
use rstim::sim::measure_record_batch::MeasureRecordBatch;

fn rng() -> StdRng {
    StdRng::seed_from_u64(12345)
}

// ========== Two-qubit gate tests ==========

#[test]
fn frame_cy_gate() {
    // CY|10> = i|11>; measuring both gives 1,1
    let instrs = parse_lines("X 0\nCY 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
        assert_eq!(out.measurements.get(1, s), true);
    }
}

#[test]
fn frame_cz_gate() {
    // CZ flips phase on |11>, test via H sandwich
    // H1 CZ H1 = CNOT(0,1): |10> -> |11>
    let instrs = parse_lines("X 0\nH 1\nCZ 0 1\nH 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
        assert_eq!(out.measurements.get(1, s), true);
    }
}

#[test]
fn frame_xcx_gate() {
    // XCX = H(a) CNOT(a,b) H(a). On |00>, XCX = I on computational basis.
    // On |+0>, XCX makes them entangled. Use: H 0, XCX 0 1, M 0 1 -> correlated.
    let instrs = parse_lines("H 0\nXCX 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 256, &mut r).unwrap();
    // XCX on |+0> entangles; both outcomes should be correlated
    for s in 0..256 {
        assert_eq!(out.measurements.get(0, s), out.measurements.get(1, s));
    }
}

#[test]
fn frame_xcy_gate() {
    // Just run the gate to exercise the code path
    let instrs = parse_lines("X 0\nXCY 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    assert_eq!(out.measurements.num_major(), 2);
}

#[test]
fn frame_xcz_gate() {
    // XCZ(a,b) = CNOT(b,a). X 0, XCZ 0 1 -> unchanged
    let instrs = parse_lines("X 1\nXCZ 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
        assert_eq!(out.measurements.get(1, s), true);
    }
}

#[test]
fn frame_ycx_gate() {
    let instrs = parse_lines("X 0\nYCX 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    assert_eq!(out.measurements.num_major(), 2);
}

#[test]
fn frame_ycy_gate() {
    let instrs = parse_lines("X 0\nYCY 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    assert_eq!(out.measurements.num_major(), 2);
}

#[test]
fn frame_ycz_gate() {
    let instrs = parse_lines("X 0\nYCZ 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    assert_eq!(out.measurements.num_major(), 2);
}

#[test]
fn frame_swap_gate() {
    // SWAP |10> = |01>
    let instrs = parse_lines("X 0\nSWAP 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
        assert_eq!(out.measurements.get(1, s), true);
    }
}

#[test]
fn frame_iswap_gate() {
    // ISWAP is like SWAP with phase; exercise the code path
    let instrs = parse_lines("X 0\nISWAP 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        // ISWAP swaps and adds phase; |10> -> i|01>
        assert_eq!(out.measurements.get(0, s), false);
        assert_eq!(out.measurements.get(1, s), true);
    }
}

#[test]
fn frame_iswap_dag_gate() {
    let instrs = parse_lines("X 0\nISWAP_DAG 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
        assert_eq!(out.measurements.get(1, s), true);
    }
}

#[test]
fn frame_cxswap_gate() {
    // CXSWAP = CX(b,a) CX(a,b)
    let instrs = parse_lines("X 0\nCXSWAP 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    assert_eq!(out.measurements.num_major(), 2);
}

#[test]
fn frame_swapcx_gate() {
    let instrs = parse_lines("X 0\nSWAPCX 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    assert_eq!(out.measurements.num_major(), 2);
}

#[test]
fn frame_czswap_gate() {
    let instrs = parse_lines("X 0\nCZSWAP 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    // CZSWAP |10> = CZ then SWAP -> |01> (CZ does nothing to |10>)
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
        assert_eq!(out.measurements.get(1, s), true);
    }
}

// ========== Single-qubit gate tests ==========

#[test]
fn frame_s_gate() {
    // S maps |+> -> |i+> (Y eigenstate); S then H then M gives deterministic 0
    // Actually S|+> = |i>; H|i> = ... Let's just test S*S = Z: X 0, S 0, S 0, M 0 -> M gives 1 (since S^2=Z, X 0 Z gives -|1>)
    let instrs = parse_lines("X 0\nS 0\nS 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
    }
}

#[test]
fn frame_sqrt_x_gate() {
    // SQRT_X^2 = X: |0> -> SQRT_X^2 -> |1>
    let instrs = parse_lines("SQRT_X 0\nSQRT_X 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
    }
}

#[test]
fn frame_sqrt_y_gate() {
    // SQRT_Y^2 = Y: |0> -> SQRT_Y^2 -> i|1>
    let instrs = parse_lines("SQRT_Y 0\nSQRT_Y 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
    }
}

#[test]
fn frame_h_xy_gate() {
    // H_XY maps Z -> -Z; on |0> the stabilizer becomes -Z so measurement gives 1
    let instrs = parse_lines("H_XY 0\nH_XY 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

#[test]
fn frame_h_yz_gate() {
    let instrs = parse_lines("H_YZ 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

// ========== MX, MY, MRX, MRY measurement tests ==========

#[test]
fn frame_mx_measurement() {
    // H|0> = |+>, MX should give 0 deterministically
    let instrs = parse_lines("H 0\nMX 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

#[test]
fn frame_mx_after_z_gate() {
    // Z|+> = |->, MX on |-> should give 1
    let instrs = parse_lines("H 0\nZ 0\nMX 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
    }
}

#[test]
fn frame_my_measurement() {
    // S|+> = |i>, MY should give 0 deterministically
    let instrs = parse_lines("H 0\nS 0\nMY 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

#[test]
fn frame_mrx_measurement() {
    // H|0> = |+>, MRX gives 0 and resets to |+>
    let instrs = parse_lines("H 0\nMRX 0\nMRX 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
        assert_eq!(out.measurements.get(1, s), false);
    }
}

#[test]
fn frame_mry_measurement() {
    // Exercise MRY code path
    let instrs = parse_lines("MRY 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), ref_sample[0]);
    }
}

// ========== RX, RY reset tests ==========

#[test]
fn frame_rx_reset() {
    // RX resets to |+>, MX should give 0
    let instrs = parse_lines("X 0\nRX 0\nMX 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

#[test]
fn frame_ry_reset() {
    // Exercise RY code path via FrameSimulator directly
    let instrs = parse_lines("RY 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
    let mut r = rng();
    let mut frame = FrameSimulator::new(1, 64);
    frame.run(&instrs, &ref_sample, &mut r).unwrap();
    assert_eq!(frame.m_record.len(), 1);
}

// ========== Noise channel tests ==========

#[test]
fn frame_y_error_deterministic() {
    // Y_ERROR(1) flips both X and Z frames; detected via Z measurement
    let instrs = parse_lines("Y_ERROR(1) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
    }
}

#[test]
fn frame_depolarize2_statistical() {
    // DEPOLARIZE2(1.0) always applies a random two-qubit Pauli (from 15 non-identity)
    // 8/15 have X or Y on qubit 0 (flip Z measurement) ≈ 53.3%
    let instrs = parse_lines("DEPOLARIZE2(1.0) 0 1\nM 0 1\n").unwrap();
    let n = 10000;
    let mut r = rng();
    let out = sample_batch(&instrs, n, &mut r).unwrap();
    let flipped_0: usize = (0..n).filter(|&s| out.measurements.get(0, s)).count();
    let flipped_1: usize = (0..n).filter(|&s| out.measurements.get(1, s)).count();
    let rate_0 = flipped_0 as f64 / n as f64;
    let rate_1 = flipped_1 as f64 / n as f64;
    assert!((rate_0 - 8.0 / 15.0).abs() < 0.05, "q0 flip rate: {rate_0}");
    assert!((rate_1 - 8.0 / 15.0).abs() < 0.05, "q1 flip rate: {rate_1}");
}

#[test]
fn frame_pauli_channel_1_x() {
    // PAULI_CHANNEL_1(1,0,0): always X error
    let instrs = parse_lines("PAULI_CHANNEL_1(1,0,0) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
    }
}

#[test]
fn frame_pauli_channel_1_y() {
    // PAULI_CHANNEL_1(0,1,0): always Y error (flips Z measurement)
    let instrs = parse_lines("PAULI_CHANNEL_1(0,1,0) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
    }
}

#[test]
fn frame_pauli_channel_1_z() {
    // PAULI_CHANNEL_1(0,0,1): always Z error (no flip in Z measurement)
    let instrs = parse_lines("PAULI_CHANNEL_1(0,0,1) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

#[test]
fn frame_pauli_channel_2_deterministic() {
    // PAULI_CHANNEL_2 with all weight on IX (index 0): always X on qubit b
    // The 15 paulis are: IX, IY, IZ, XI, XX, XY, XZ, YI, YX, YY, YZ, ZI, ZX, ZY, ZZ
    let instrs =
        parse_lines("PAULI_CHANNEL_2(1,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false); // I on qubit 0
        assert_eq!(out.measurements.get(1, s), true); // X on qubit 1
    }
}

#[test]
fn frame_pauli_channel_2_xi() {
    // All weight on XI (index 3)
    let instrs =
        parse_lines("PAULI_CHANNEL_2(0,0,0,1,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true); // X on qubit 0
        assert_eq!(out.measurements.get(1, s), false); // I on qubit 1
    }
}

#[test]
fn frame_heralded_pauli_channel_1() {
    // HERALDED_PAULI_CHANNEL_1(0,1,0,0): always herald, always X
    let instrs = parse_lines("HERALDED_PAULI_CHANNEL_1(0,1,0,0) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true); // herald bit = 1
        assert_eq!(out.measurements.get(1, s), true); // X error flips Z measurement
    }
}

#[test]
fn frame_heralded_pauli_channel_1_z_error() {
    // HERALDED_PAULI_CHANNEL_1(0,0,0,1): always herald, always Z
    let instrs = parse_lines("HERALDED_PAULI_CHANNEL_1(0,0,0,1) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true); // herald bit = 1
        assert_eq!(out.measurements.get(1, s), false); // Z error doesn't flip Z measurement
    }
}

#[test]
fn frame_heralded_pauli_channel_1_false_positive() {
    // HERALDED_PAULI_CHANNEL_1(1,0,0,0): always herald but I error (false positive)
    let instrs = parse_lines("HERALDED_PAULI_CHANNEL_1(1,0,0,0) 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true); // herald bit = 1
        assert_eq!(out.measurements.get(1, s), false); // I error = no flip
    }
}

// ========== SPP / SPP_DAG tests ==========

#[test]
fn frame_spp_z_then_measure() {
    // SPP Z0 applies S (in frame picture: z ^= x). On |0>, should be no effect on Z measurement.
    let instrs = parse_lines("SPP Z0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

#[test]
fn frame_spp_dag_z_then_measure() {
    let instrs = parse_lines("SPP_DAG Z0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

#[test]
fn frame_spp_x_product() {
    // SPP X0 on |0>: S in X basis
    let instrs = parse_lines("SPP X0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

#[test]
fn frame_spp_y_product() {
    let instrs = parse_lines("SPP Y0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

#[test]
fn frame_spp_multi_qubit() {
    // SPP X0*Z1 — multi-qubit SPP product
    let instrs = parse_lines("SPP X0*Z1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
        assert_eq!(out.measurements.get(1, s), false);
    }
}

// ========== MXX, MYY, MZZ tests ==========

#[test]
fn frame_mxx_bell() {
    // H 0, CNOT 0 1 -> Bell state. MXX should give 0 (correlated in X basis)
    let instrs = parse_lines("H 0\nCNOT 0 1\nMXX 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

#[test]
fn frame_myy_bell() {
    // Y⊗Y eigenvalue on |Φ+⟩ is -1, so MYY gives 1
    let instrs = parse_lines("H 0\nCNOT 0 1\nMYY 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![true]);
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
    }
}

#[test]
fn frame_mzz_product_state() {
    // |00> -> MZZ = 0 (both in Z=0)
    let instrs = parse_lines("MZZ 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

#[test]
fn frame_mzz_antiparallel() {
    // |10> -> MZZ = 1 (anti-parallel)
    let instrs = parse_lines("X 0\nMZZ 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![true]);
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
    }
}

// ========== MPAD test ==========

#[test]
fn frame_mpad() {
    let instrs = parse_lines("MPAD 0 1 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
        assert_eq!(out.measurements.get(1, s), true);
        assert_eq!(out.measurements.get(2, s), false);
    }
}

// ========== MPP with various bases ==========

#[test]
fn frame_mpp_xx_bell() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nMPP X0*X1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

#[test]
fn frame_mpp_yy_bell() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nMPP Y0*Y1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), ref_sample[0]);
    }
}

#[test]
fn frame_mpp_single_x() {
    // MPP X0 on |+> should give 0
    let instrs = parse_lines("H 0\nMPP X0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

#[test]
fn frame_mpp_single_y() {
    // S H |0> = |i>, MPP Y0 should give 0
    let instrs = parse_lines("H 0\nS 0\nMPP Y0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

// ========== Reference sample coverage for two-qubit gates ==========

#[test]
fn ref_sample_cy() {
    let instrs = parse_lines("X 0\nCY 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![true, true]);
}

#[test]
fn ref_sample_cz() {
    // CZ|1+> = |1-> -> H on q1 -> |11>
    let instrs = parse_lines("X 0\nH 1\nCZ 0 1\nH 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![true, true]);
}

#[test]
fn ref_sample_xcx() {
    let instrs = parse_lines("XCX 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, false]);
}

#[test]
fn ref_sample_xcy() {
    let instrs = parse_lines("XCY 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, false]);
}

#[test]
fn ref_sample_xcz() {
    let instrs = parse_lines("XCZ 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, false]);
}

#[test]
fn ref_sample_ycx() {
    let instrs = parse_lines("YCX 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, false]);
}

#[test]
fn ref_sample_ycy() {
    let instrs = parse_lines("YCY 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, false]);
}

#[test]
fn ref_sample_ycz() {
    let instrs = parse_lines("YCZ 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, false]);
}

#[test]
fn ref_sample_swap() {
    let instrs = parse_lines("X 0\nSWAP 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, true]);
}

#[test]
fn ref_sample_iswap() {
    let instrs = parse_lines("X 0\nISWAP 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, true]);
}

#[test]
fn ref_sample_iswap_dag() {
    let instrs = parse_lines("X 0\nISWAP_DAG 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, true]);
}

#[test]
fn ref_sample_cxswap() {
    let instrs = parse_lines("CXSWAP 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, false]);
}

#[test]
fn ref_sample_swapcx() {
    let instrs = parse_lines("SWAPCX 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, false]);
}

#[test]
fn ref_sample_czswap() {
    let instrs = parse_lines("CZSWAP 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, false]);
}

// ========== Reference sample for MX, MY, MRX, MRY, RX, RY ==========

#[test]
fn ref_sample_mx() {
    // |+> = H|0>, MX should give 0
    let instrs = parse_lines("H 0\nMX 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
}

#[test]
fn ref_sample_my() {
    let instrs = parse_lines("H 0\nS 0\nMY 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
}

#[test]
fn ref_sample_mrx() {
    let instrs = parse_lines("H 0\nMRX 0\nMRX 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, false]);
}

#[test]
fn ref_sample_mry() {
    let instrs = parse_lines("H 0\nS 0\nMRY 0\nMRY 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, false]);
}

#[test]
fn ref_sample_rx_reset() {
    let instrs = parse_lines("X 0\nRX 0\nMX 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
}

#[test]
fn ref_sample_ry_reset() {
    let instrs = parse_lines("X 0\nRY 0\nMY 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
}

// ========== Reference sample for SPP ==========

#[test]
fn ref_sample_spp() {
    // SPP Z0 on |0> is like S on |0> = |0>, M gives 0
    let instrs = parse_lines("SPP Z0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
}

#[test]
fn ref_sample_spp_dag() {
    let instrs = parse_lines("SPP_DAG Z0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
}

// ========== Reference sample for MXX, MYY, MZZ ==========

#[test]
fn ref_sample_mxx() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nMXX 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
}

#[test]
fn ref_sample_myy() {
    // Y⊗Y eigenvalue on |Φ+⟩ is -1
    let instrs = parse_lines("H 0\nCNOT 0 1\nMYY 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![true]);
}

#[test]
fn ref_sample_mzz() {
    let instrs = parse_lines("MZZ 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
}

// ========== Reference sample for HERALDED_PAULI_CHANNEL_1 ==========

#[test]
fn ref_sample_heralded_pauli_channel_1() {
    let instrs = parse_lines("HERALDED_PAULI_CHANNEL_1(0.1,0.1,0.1,0.1) 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, false]);
}

// ========== BitTable toggle and randomize_row ==========

#[test]
fn bit_table_toggle() {
    let mut bt = BitTable::new(2, 128);
    assert_eq!(bt.get(0, 5), false);
    bt.toggle(0, 5);
    assert_eq!(bt.get(0, 5), true);
    bt.toggle(0, 5);
    assert_eq!(bt.get(0, 5), false);
}

#[test]
fn bit_table_randomize_row() {
    let mut bt = BitTable::new(2, 128);
    let mut r = rng();
    bt.randomize_row(0, &mut r);
    let mut any_set = false;
    for i in 0..128 {
        if bt.get(0, i) {
            any_set = true;
            break;
        }
    }
    assert!(any_set, "randomized row should have at least one set bit");
    // Row 1 should still be all zeros
    for i in 0..128 {
        assert_eq!(bt.get(1, i), false);
    }
}

// ========== MeasureRecordBatch push_zeros and xor_lookback_into ==========

#[test]
fn measure_record_batch_push_zeros() {
    let mut mrb = MeasureRecordBatch::new(64);
    mrb.push_zeros();
    for s in 0..64 {
        assert_eq!(mrb.lookback(1, s), false);
    }
    assert_eq!(mrb.len(), 1);
}

#[test]
fn measure_record_batch_xor_lookback_into() {
    let mut mrb = MeasureRecordBatch::new(64);
    let mut row = BitTable::new(1, 64);
    row.set(0, 0, true);
    row.set(0, 3, true);
    mrb.push_row(row.row_words(0));
    let mut dest = vec![0u64; mrb.words_per_row()];
    mrb.xor_lookback_into(1, &mut dest);
    assert_eq!((dest[0] >> 0) & 1, 1);
    assert_eq!((dest[0] >> 3) & 1, 1);
    assert_eq!((dest[0] >> 1) & 1, 0);
    // XOR again should cancel
    mrb.xor_lookback_into(1, &mut dest);
    assert_eq!(dest[0], 0);
}

// ========== TICK, QUBIT_COORDS, SHIFT_COORDS (no-ops but exercise paths) ==========

#[test]
fn frame_tick_and_coords() {
    let instrs = parse_lines("QUBIT_COORDS(1,2) 0\nSHIFT_COORDS(0,0,1)\nTICK\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

// ========== I_ERROR and II_ERROR in frame sim ==========

#[test]
fn frame_i_error_ii_error() {
    let instrs = parse_lines("I_ERROR(0.1) 0\nII_ERROR(0.1) 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
        assert_eq!(out.measurements.get(1, s), false);
    }
}

// ========== Complex circuit: surface code style with detectors ==========

#[test]
fn frame_detector_repeat_circuit() {
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
    // Noiseless: all detectors should be 0 and observable should be 0
    for s in 0..128 {
        for d in 0..out.detections.num_major() {
            assert_eq!(out.detections.get(d, s), false, "det {d} shot {s}");
        }
        for o in 0..out.observable_flips.num_major() {
            assert_eq!(out.observable_flips.get(o, s), false, "obs {o} shot {s}");
        }
    }
}

// ========== Correlated error with Y and Z targets in frame sim ==========

#[test]
fn frame_correlated_error_yz_targets() {
    let instrs = parse_lines("CORRELATED_ERROR(1) Y0 Z1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        // Y error on q0 flips Z measurement, Z error on q1 doesn't
        assert_eq!(out.measurements.get(0, s), true);
        assert_eq!(out.measurements.get(1, s), false);
    }
}

// ========== S_DAG / SQRT_Z_DAG aliases ==========

#[test]
fn frame_s_dag_gate() {
    let instrs = parse_lines("S_DAG 0\nS_DAG 0\nX 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
    }
}

#[test]
fn frame_sqrt_z_dag_alias() {
    let instrs = parse_lines("SQRT_Z_DAG 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

#[test]
fn frame_sqrt_z_alias() {
    let instrs = parse_lines("SQRT_Z 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

#[test]
fn frame_sqrt_x_dag_alias() {
    let instrs = parse_lines("SQRT_X_DAG 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

#[test]
fn frame_sqrt_y_dag_alias() {
    let instrs = parse_lines("SQRT_Y_DAG 0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), false);
    }
}

// ========== ZCX, ZCY, ZCZ aliases ==========

#[test]
fn frame_zcx_alias() {
    let instrs = parse_lines("X 0\nZCX 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
        assert_eq!(out.measurements.get(1, s), true);
    }
}

#[test]
fn frame_zcy_alias() {
    let instrs = parse_lines("X 0\nZCY 0 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
        assert_eq!(out.measurements.get(1, s), true);
    }
}

#[test]
fn frame_zcz_alias() {
    let instrs = parse_lines("X 0\nH 1\nZCZ 0 1\nH 1\nM 0 1\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
        assert_eq!(out.measurements.get(1, s), true);
    }
}

// ========== E alias for CORRELATED_ERROR ==========

#[test]
fn frame_e_alias() {
    let instrs = parse_lines("E(1) X0\nM 0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
    }
}

// ========== Inverted measurements ==========

#[test]
fn frame_inverted_mz() {
    // !M on |0> gives 1 (inverted)
    let instrs = parse_lines("M !0\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        assert_eq!(out.measurements.get(0, s), true);
    }
}

// ========== Observable with multiple rec targets ==========

#[test]
fn frame_observable_multiple_recs() {
    let instrs = parse_lines("X 0\nM 0 1\nOBSERVABLE_INCLUDE(0) rec[-1] rec[-2]\n").unwrap();
    let mut r = rng();
    let out = sample_batch(&instrs, 64, &mut r).unwrap();
    for s in 0..64 {
        // rec[-2] matches the deterministic reference sample, so the parity stays false.
        assert_eq!(out.observable_flips.get(0, s), false);
    }
}

// ========== Frame sim directly with FrameSimulator to exercise detections/observable_flips ==========

#[test]
fn frame_sim_detections_accessor() {
    let instrs = parse_lines("M 0\nDETECTOR rec[-1]\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut r = rng();
    let mut frame = FrameSimulator::new(1, 64);
    frame.run(&instrs, &ref_sample, &mut r).unwrap();
    let det = frame.detections();
    for s in 0..64 {
        assert_eq!(det.get(0, s), false);
    }
}

#[test]
fn frame_sim_observable_flips_accessor() {
    let instrs = parse_lines("X 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut r = rng();
    let mut frame = FrameSimulator::new(1, 64);
    frame.run(&instrs, &ref_sample, &mut r).unwrap();
    let obs = frame.observable_flips();
    for s in 0..64 {
        assert_eq!(obs.get(0, s), false);
    }
}
