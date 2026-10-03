use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::{ActiveState, CliffordGate, MeasurementBasis};

#[test]
fn full_width_magic_ghz_measurements_preserve_conditional_born_probabilities() {
    // This exact full-width witness has state
    // (|0...0> + exp(i*pi/4)|1...1>)/sqrt(2). Measuring X/Y on
    // all but one qubit leaves an independently predictable relative phase.
    for width in [2, 63, 64, 65, 127, 128, 129, 193] {
        for seed in [739, 20261003, 20261004] {
            let mut state = ActiveState::new(width, 1);
            state.apply_clifford(CliffordGate::H(0)).unwrap();
            state.t(0).unwrap();
            for q in 1..width {
                state.apply_clifford(CliffordGate::CX(0, q)).unwrap();
            }
            let mut rng = StdRng::seed_from_u64(seed);
            let mut phase = std::f64::consts::FRAC_PI_4;
            // Reverse record order exercises dormant pivots on both sides of
            // the active coordinate and densely supported physical Paulis.
            for q in (1..width).rev() {
                let basis = if (q + seed as usize) % 2 == 0 {
                    MeasurementBasis::X
                } else {
                    MeasurementBasis::Y
                };
                let (zero, one) = state.measurement_probabilities(q, basis).unwrap();
                assert!((zero - 0.5).abs() < 1e-10 && (one - 0.5).abs() < 1e-10);
                let outcome = if q % 3 == 0 {
                    state.measure_reset(q, basis, &mut rng).unwrap()
                } else {
                    state.measure(q, basis, &mut rng).unwrap()
                };
                if basis == MeasurementBasis::Y {
                    phase -= std::f64::consts::FRAC_PI_2;
                }
                if outcome {
                    phase += std::f64::consts::PI;
                }
                // Keep the analytic phase bounded to avoid accumulated trig
                // argument-reduction error in the independent reference.
                phase = phase.rem_euclid(2.0 * std::f64::consts::PI);
            }
            for (basis, expected) in [
                (MeasurementBasis::X, (1.0 + phase.cos()) / 2.0),
                (MeasurementBasis::Y, (1.0 + phase.sin()) / 2.0),
                (MeasurementBasis::Z, 0.5),
            ] {
                let (zero, one) = state.measurement_probabilities(0, basis).unwrap();
                assert!(
                    (zero - expected).abs() < 1e-10,
                    "width={width} seed={seed} basis={basis:?}"
                );
                assert!((one - (1.0 - expected)).abs() < 1e-10);
            }
        }
    }
}
