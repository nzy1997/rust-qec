#[path = "support/near_clifford_oracle.rs"]
mod oracle;

use oracle::DenseOracle;
use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::{ActiveState, CliffordGate, MeasurementBasis, NearCliffordExecutor};

#[test]
fn fixed_active_axis_retires_and_later_t_matches_oracle() {
    let mut seen = [false; 2];
    for seed in 0..64 {
        let mut active = ActiveState::new(1, 1);
        let mut oracle = DenseOracle::new(1);
        active.apply_clifford(CliffordGate::H(0)).unwrap();
        oracle.h(0);
        active.t(0).unwrap();
        oracle.t(0);
        let mut rng = StdRng::seed_from_u64(seed);
        let outcome = active.measure(0, MeasurementBasis::X, &mut rng).unwrap();
        oracle.collapse(0, 'X', outcome);
        seen[outcome as usize] = true;
        assert_eq!(active.retire_fixed_axes(), 1);
        assert_eq!(active.active_rank(), 0);
        assert_eq!(active.origin(), &[outcome]);

        active.apply_clifford(CliffordGate::H(0)).unwrap();
        oracle.h(0);
        active.t(0).unwrap();
        oracle.t(0);
        let actual = active.coefficients()[0] * active.global_phase();
        let expected = oracle.amplitudes()[outcome as usize];
        assert!((actual.re - expected.re).abs() < 1e-12, "seed={seed}");
        assert!((actual.im - expected.im).abs() < 1e-12, "seed={seed}");
        assert!(oracle.amplitudes()[(!outcome) as usize].norm_sqr() < 1e-12);
    }
    assert_eq!(seen, [true, true]);
}

#[test]
fn entangled_measure_reset_preserves_record_and_other_qubit() {
    let circuit = NearCliffordExecutor::compile_text("H 0\nCX 0 1\nMR 0\nT 1\nM 0 1").unwrap();
    let mut rng = StdRng::seed_from_u64(739);
    let shots = circuit.sample(100, &mut rng).unwrap();
    assert!(shots.iter().any(|shot| shot.measurements[0]));
    assert!(shots.iter().any(|shot| !shot.measurements[0]));
    for shot in shots {
        assert_eq!(shot.measurements.len(), 3);
        assert!(!shot.measurements[1], "reset target must be |0>");
        assert_eq!(shot.measurements[2], shot.measurements[0]);
    }
}

#[test]
fn repeated_mid_circuit_measurement_and_reset_work() {
    let circuit = NearCliffordExecutor::compile_text("REPEAT 3 {\n  H 0\n  MR 0\n}\nM 0").unwrap();
    let mut rng = StdRng::seed_from_u64(123);
    for shot in circuit.sample(32, &mut rng).unwrap() {
        assert_eq!(shot.measurements.len(), 4);
        assert!(!shot.measurements[3]);
    }
}

#[test]
fn x_and_y_resets_prepare_positive_eigenstates() {
    for (reset, measure) in [("RX", "MX"), ("RY", "MY")] {
        let circuit =
            NearCliffordExecutor::compile_text(&format!("H 0\nT 0\n{reset} 0\n{measure} 0"))
                .unwrap();
        let mut rng = StdRng::seed_from_u64(70);
        let shots = circuit.sample(32, &mut rng).unwrap();
        assert!(
            shots.iter().all(|shot| shot.measurements == [false]),
            "{reset}"
        );
    }
}

#[test]
fn pure_clifford_resets_do_not_consume_active_rank() {
    let mut circuit_text = String::new();
    for q in 0..24 {
        circuit_text.push_str(&format!("RX {q}\nMX {q}\n"));
    }
    let circuit = NearCliffordExecutor::compile_text(&circuit_text).unwrap();
    let mut rng = StdRng::seed_from_u64(739);
    let shot = circuit.run(&mut rng).unwrap();
    assert_eq!(shot.measurements, vec![false; 24]);
}

#[test]
fn pure_clifford_y_measurement_and_retired_origin_rebase() {
    let mut pure = ActiveState::new(1, 0);
    pure.apply_clifford(CliffordGate::H(0)).unwrap();
    pure.apply_clifford(CliffordGate::S(0)).unwrap();
    let mut rng = StdRng::seed_from_u64(739);
    assert!(!pure.measure(0, MeasurementBasis::Y, &mut rng).unwrap());
    assert!(!pure.measure(0, MeasurementBasis::Y, &mut rng).unwrap());

    for rotate_to_y in [false, true] {
        let mut found_negative_x = false;
        for seed in 0..64 {
            let mut state = ActiveState::new(2, 1);
            state.apply_clifford(CliffordGate::H(0)).unwrap();
            state.t(0).unwrap();
            let mut rng = StdRng::seed_from_u64(seed);
            if !state.measure(0, MeasurementBasis::X, &mut rng).unwrap() {
                continue;
            }
            assert_eq!(state.retire_fixed_axes(), 1);
            assert_eq!(state.origin(), &[true, false]);
            state.apply_clifford(CliffordGate::H(0)).unwrap();
            if rotate_to_y {
                state.apply_clifford(CliffordGate::S(0)).unwrap();
            }
            assert!(!state.measure(1, MeasurementBasis::Z, &mut rng).unwrap());
            assert_eq!(state.origin(), &[false, false]);
            assert!(state.measure(0, MeasurementBasis::Z, &mut rng).unwrap());
            found_negative_x = true;
            break;
        }
        assert!(found_negative_x);
    }
}
