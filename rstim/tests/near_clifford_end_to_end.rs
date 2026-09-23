#[path = "support/near_clifford_oracle.rs"]
mod oracle;

use oracle::{DenseOracle, WeightedTableauOracle};
use rand::{Rng, SeedableRng, rngs::StdRng};
use rstim::ir::{StimInstr, StimTarget};
use rstim::near_clifford::{ActiveState, CliffordGate, MeasurementBasis, NearCliffordExecutor};

#[test]
fn near_clifford_two_t_known_answer() {
    let mut state = ActiveState::new(2, 2);
    let mut oracle = DenseOracle::new(2);
    let mut weighted = WeightedTableauOracle::new(2);
    state.apply_clifford(CliffordGate::H(0)).unwrap();
    oracle.h(0);
    weighted.h(0);
    state.apply_clifford(CliffordGate::CX(0, 1)).unwrap();
    oracle.cx(0, 1);
    weighted.cx(0, 1);
    state.t(0).unwrap();
    oracle.t(0);
    weighted.t(0);
    state.apply_clifford(CliffordGate::H(0)).unwrap();
    oracle.h(0);
    weighted.h(0);
    state.t(0).unwrap();
    oracle.t(0);
    weighted.t(0);

    let frame = state.frame_snapshot();
    assert_eq!(frame.x[2], [false, true]);
    assert_eq!(frame.z[2], [true, false]);
    assert_eq!(frame.x[3], [true, false]);
    assert_eq!(frame.z[3], [false, true]);
    assert_eq!(frame.phase[2..], [0, 0]);
    assert_eq!(state.active_rank(), 2);
    let c = (std::f64::consts::PI / 8.0).cos();
    let s = (std::f64::consts::PI / 8.0).sin();
    // The stored virtual |11> vector is -Z1 Z0|phi>; rephase that sector
    // into the sign-sector convention fixed by issue #739.
    let raw = state.coefficients();
    let sign_sectors = [raw[0], raw[2], raw[1], raw[3] * -1.0];
    for (actual, (re, im)) in
        sign_sectors
            .iter()
            .zip([(c * c, 0.0), (0.0, -c * s), (0.0, -s * c), (-s * s, 0.0)])
    {
        assert!((actual.re - re).abs() < 1e-12, "{actual:?} != {re}+{im}i");
        assert!((actual.im - im).abs() < 1e-12, "{actual:?} != {re}+{im}i");
    }
    assert_eq!(weighted.term_count(), 4);
    weighted.assert_stabilizer_witnesses();
    let weighted_amplitudes = weighted.amplitudes();
    let amplitudes = oracle.amplitudes();
    for (weighted, dense) in weighted_amplitudes.iter().zip(amplitudes) {
        assert!((weighted.re - dense.re).abs() < 1e-12);
        assert!((weighted.im - dense.im).abs() < 1e-12);
    }
    for (sector, (a, b)) in
        sign_sectors
            .iter()
            .zip([(false, false), (false, true), (true, false), (true, true)])
    {
        let mut projected_re = 0.0;
        let mut projected_im = 0.0;
        for (index, amplitude) in amplitudes.iter().enumerate() {
            let phi = if index == 3 { -0.5 } else { 0.5 };
            let sign = if (a && index & 1 != 0) ^ (b && index & 2 != 0) {
                -1.0
            } else {
                1.0
            };
            projected_re += phi * sign * amplitude.re;
            projected_im += phi * sign * amplitude.im;
        }
        let phase = state.global_phase().conj();
        let projected = rstim::near_clifford::ComplexAmp::new(projected_re, projected_im) * phase;
        assert!((projected.re - sector.re).abs() < 1e-12);
        assert!((projected.im - sector.im).abs() < 1e-12);
    }
    let root_half = std::f64::consts::FRAC_1_SQRT_2 / 2.0;
    for (actual, (re, im)) in amplitudes.iter().zip([
        (0.5, 0.0),
        (root_half, root_half),
        (root_half, root_half),
        (0.0, -0.5),
    ]) {
        assert!((actual.re - re).abs() < 1e-12);
        assert!((actual.im - im).abs() < 1e-12);
    }
    assert!((oracle.even_x_parity_probability(0, 1) - 0.75).abs() < 1e-12);
    let mut rng = StdRng::seed_from_u64(739);
    let mut even = 0;
    for _ in 0..4096 {
        let mut shot = state.clone();
        let a = shot.measure(0, MeasurementBasis::X, &mut rng).unwrap();
        let b = shot.measure(1, MeasurementBasis::X, &mut rng).unwrap();
        even += usize::from(a == b);
    }
    assert!((even as f64 / 4096.0 - 0.75).abs() < 0.03);
    let error =
        NearCliffordExecutor::compile_text("H 0\nCX 0 1\nROT_Z(0.4487989505128276) 0\nH 0\nT 0")
            .unwrap_err();
    assert!(error.contains("unsupported gate ROT_Z"), "{error}");
    let error =
        NearCliffordExecutor::compile_text("H 0\nCX 0 1\nRZ(0.4487989505128276) 0\nH 0\nT 0")
            .unwrap_err();
    assert!(error.contains("RZ takes no arguments"), "{error}");
}

#[test]
fn noise_feedback_and_qec_syndrome_use_one_coherent_shot() {
    let circuit = NearCliffordExecutor::compile_text(
        "H 0\nCX 0 1\nT 0\nR 2\nX_ERROR(1) 1\nCX 0 2\nCX 1 2\nM 2\nDETECTOR rec[-1]\nCX rec[-1] 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-3]",
    )
    .unwrap();
    let mut rng = StdRng::seed_from_u64(739);
    for shot in circuit.sample(128, &mut rng).unwrap() {
        assert_eq!(shot.measurements.len(), 3);
        assert_eq!(shot.measurements[0], true);
        assert_eq!(shot.measurements[1], shot.measurements[2]);
        assert_eq!(shot.detectors, [true, false]);
        assert_eq!(shot.observables, [(0, true)]);
    }
}

#[test]
fn sweep_feedback_and_observable_events_follow_stim_record_order() {
    let circuit = NearCliffordExecutor::compile_text(
        "QUBIT_COORDS(1,2) 0\nTICK\nCX sweep[0] 0\nM 0\nDETECTOR(5) rec[-1]\nOBSERVABLE_INCLUDE(3) rec[-1]\nSHIFT_COORDS(1)\nOBSERVABLE_INCLUDE(3) rec[-1]",
    )
    .unwrap();
    let mut rng = StdRng::seed_from_u64(7);
    let zero = circuit.run(&mut rng).unwrap();
    assert_eq!(zero.measurements, [false]);
    assert_eq!(zero.detectors, [false]);
    assert_eq!(zero.observables, [(3, false), (3, false)]);
    let one = circuit.run_with_sweep(&[true], &mut rng).unwrap();
    assert_eq!(one.measurements, [true]);
    assert_eq!(one.detectors, [true]);
    assert_eq!(one.observables, [(3, true), (3, true)]);
    for shot in circuit.sample_with_sweep(3, &[true], &mut rng).unwrap() {
        assert_eq!(shot, one);
    }
}

#[test]
fn cy_pair_and_cz_record_feedback_apply_the_intended_pauli() {
    let pair = NearCliffordExecutor::compile_text("X 0\nCY 0 1\nM 1").unwrap();
    let feedback = NearCliffordExecutor::compile_text("X 0\nM 0\nH 1\nCZ rec[-1] 1\nMX 1").unwrap();
    let mut rng = StdRng::seed_from_u64(739);
    assert_eq!(pair.run(&mut rng).unwrap().measurements, [true]);
    assert_eq!(feedback.run(&mut rng).unwrap().measurements, [true, true]);
}

#[test]
fn depolarizing_noise_samples_nonidentity_paulis() {
    let mut rng = StdRng::seed_from_u64(739);
    let one = NearCliffordExecutor::compile_text("DEPOLARIZE1(1) 0\nM 0").unwrap();
    let flips = one
        .sample(3000, &mut rng)
        .unwrap()
        .iter()
        .filter(|shot| shot.measurements[0])
        .count();
    assert!((flips as f64 / 3000.0 - 2.0 / 3.0).abs() < 0.04);
    let two = NearCliffordExecutor::compile_text("DEPOLARIZE2(1) 0 1\nM 0 1").unwrap();
    let flips = two
        .sample(3000, &mut rng)
        .unwrap()
        .iter()
        .filter(|shot| shot.measurements.iter().any(|bit| *bit))
        .count();
    assert!((flips as f64 / 3000.0 - 0.8).abs() < 0.04);
}

#[test]
fn deterministic_noise_does_not_consume_randomness() {
    let baseline = NearCliffordExecutor::compile_text("H 0\nM 0").unwrap();
    let zero_noise = NearCliffordExecutor::compile_text("H 0\nX_ERROR(0) 0\nM 0").unwrap();
    let definite_gate = NearCliffordExecutor::compile_text("X 0\nH 1\nM 0 1").unwrap();
    let definite_noise = NearCliffordExecutor::compile_text("X_ERROR(1) 0\nH 1\nM 0 1").unwrap();
    for seed in 0..32 {
        let mut first = StdRng::seed_from_u64(seed);
        let mut second = StdRng::seed_from_u64(seed);
        assert_eq!(
            baseline.run(&mut first).unwrap(),
            zero_noise.run(&mut second).unwrap()
        );
        assert_eq!(first.r#gen::<u64>(), second.r#gen::<u64>());
        let mut first = StdRng::seed_from_u64(seed);
        let mut second = StdRng::seed_from_u64(seed);
        assert_eq!(
            definite_gate.run(&mut first).unwrap(),
            definite_noise.run(&mut second).unwrap()
        );
        assert_eq!(first.r#gen::<u64>(), second.r#gen::<u64>());
    }
}

#[test]
fn malformed_noise_feedback_and_records_fail_explicitly() {
    for circuit in [
        "X_ERROR(1.1) 0",
        "DEPOLARIZE1 0",
        "DEPOLARIZE2(0.2) 0",
        "CX rec[0] 0",
        "SWAP rec[-1] 0",
        "DETECTOR 0",
        "OBSERVABLE_INCLUDE(1.5) rec[-1]",
        "DETECTOR rec[-1]",
        "H 0\nM 0\nCX rec[-2] 0",
        "REPEAT 2 {\n  DETECTOR rec[-1]\n  M 0\n}",
        "CX 0 rec[-1]",
        "CX 0 0",
        "TICK 0",
        "T",
    ] {
        assert!(
            NearCliffordExecutor::compile_text(circuit).is_err(),
            "{circuit}"
        );
    }
    let circuit =
        NearCliffordExecutor::compile_text("M 0\nREPEAT 2 {\n  M 0\n  DETECTOR rec[-2] rec[-1]\n}")
            .unwrap();
    let mut rng = StdRng::seed_from_u64(739);
    assert_eq!(circuit.run(&mut rng).unwrap().detectors, [false, false]);
    assert!(
        NearCliffordExecutor::compile(vec![
            StimInstr::new("SHIFT_COORDS", vec![f64::NAN], vec![],)
        ])
        .is_err()
    );
    assert!(
        NearCliffordExecutor::compile(vec![
            StimInstr::new("M", vec![], vec![StimTarget::Qubit(0)]),
            StimInstr::Repeat {
                count: u64::MAX,
                body: vec![StimInstr::new("M", vec![], vec![StimTarget::Qubit(0)])],
            },
        ])
        .is_err()
    );
}
