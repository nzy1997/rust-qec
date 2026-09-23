#[path = "support/near_clifford_oracle.rs"]
mod oracle;

use oracle::{Amp, DenseOracle};
use rand::{Rng, SeedableRng, rngs::StdRng};
use rstim::near_clifford::{ActiveState, CliffordGate, ComplexAmp, MeasurementBasis};

fn setup_two_t() -> (ActiveState, DenseOracle) {
    let mut active = ActiveState::new(2, 2);
    let mut oracle = DenseOracle::new(2);
    active.apply_clifford(CliffordGate::H(0)).unwrap();
    oracle.h(0);
    active.apply_clifford(CliffordGate::CX(0, 1)).unwrap();
    oracle.cx(0, 1);
    active.t(0).unwrap();
    oracle.t(0);
    active.apply_clifford(CliffordGate::H(0)).unwrap();
    oracle.h(0);
    active.t(0).unwrap();
    oracle.t(0);
    (active, oracle)
}

fn compare_after_unframing(active: &ActiveState, oracle: &mut DenseOracle) {
    oracle.h(0);
    oracle.cx(0, 1);
    oracle.h(0);
    let mut virtual_state = vec![ComplexAmp::default(); 4];
    for (index, coefficient) in active.coefficients().iter().enumerate() {
        let mut bits = 0;
        for (axis_number, axis) in active.active_axes().iter().enumerate() {
            if index & (1 << axis_number) != 0 {
                bits ^= (axis[0] as usize) << 1 | axis[1] as usize;
            }
        }
        virtual_state[bits] = virtual_state[bits] + *coefficient * active.global_phase();
    }
    for (actual, expected) in virtual_state.into_iter().zip(oracle.amplitudes()) {
        let Amp { re, im } = *expected;
        assert!((actual.re - re).abs() < 1e-12, "{actual:?} != {expected:?}");
        assert!((actual.im - im).abs() < 1e-12, "{actual:?} != {expected:?}");
    }
}

#[test]
fn two_t_terminal_measurement_probabilities_match_oracle() {
    let (active, oracle) = setup_two_t();
    for (basis, oracle_basis) in [
        (MeasurementBasis::X, 'X'),
        (MeasurementBasis::Y, 'Y'),
        (MeasurementBasis::Z, 'Z'),
    ] {
        for q in 0..2 {
            let (zero, one) = active.measurement_probabilities(q, basis).unwrap();
            assert!((zero - oracle.measurement_probability(q, oracle_basis, false)).abs() < 1e-12);
            assert!((one - oracle.measurement_probability(q, oracle_basis, true)).abs() < 1e-12);
        }
    }
    assert!((oracle.even_x_parity_probability(0, 1) - 0.75).abs() < 1e-12);
}

#[test]
fn sequential_terminal_x_measurements_preserve_interference() {
    let (mut active, mut oracle) = setup_two_t();
    let mut rng = StdRng::seed_from_u64(739);
    let first = active.measure(0, MeasurementBasis::X, &mut rng).unwrap();
    oracle.collapse(0, 'X', first);
    let (p0, p1) = active
        .measurement_probabilities(1, MeasurementBasis::X)
        .unwrap();
    assert!((p0 - oracle.measurement_probability(1, 'X', false)).abs() < 1e-12);
    assert!((p1 - oracle.measurement_probability(1, 'X', true)).abs() < 1e-12);
    let second = active.measure(1, MeasurementBasis::X, &mut rng).unwrap();
    oracle.collapse(1, 'X', second);
    compare_after_unframing(&active, &mut oracle);
    let (repeat_zero, repeat_one) = active
        .measurement_probabilities(1, MeasurementBasis::X)
        .unwrap();
    assert!((if second { repeat_one } else { repeat_zero } - 1.0).abs() < 1e-12);
}

#[test]
fn terminal_y_and_z_collapse_match_oracle() {
    for (basis, letter) in [(MeasurementBasis::Y, 'Y'), (MeasurementBasis::Z, 'Z')] {
        let (mut active, mut oracle) = setup_two_t();
        let mut rng = StdRng::seed_from_u64(42);
        let outcome = active.measure(1, basis, &mut rng).unwrap();
        oracle.collapse(1, letter, outcome);
        compare_after_unframing(&active, &mut oracle);
    }
}

#[test]
fn terminal_measurement_rejects_out_of_range_qubit() {
    let mut active = ActiveState::new(2, 2);
    let mut rng = StdRng::seed_from_u64(1);
    assert!(
        active
            .measurement_probabilities(2, MeasurementBasis::Z)
            .is_err()
    );
    assert!(active.measure(2, MeasurementBasis::Z, &mut rng).is_err());
    assert_eq!(active.active_rank(), 0);
}

#[test]
fn two_t_sampled_x_parity_detects_interference() {
    let (prepared, _) = setup_two_t();
    let mut rng = StdRng::seed_from_u64(739);
    let mut even = 0;
    let shots = 4096;
    for _ in 0..shots {
        let mut state = prepared.clone();
        let a = state.measure(0, MeasurementBasis::X, &mut rng).unwrap();
        let b = state.measure(1, MeasurementBasis::X, &mut rng).unwrap();
        even += usize::from(a == b);
    }
    let frequency = even as f64 / shots as f64;
    assert!((frequency - 0.75).abs() < 0.03, "frequency={frequency}");
}

#[test]
fn measurement_rank_limit_does_not_consume_randomness_or_mutate_state() {
    let mut state = ActiveState::new(1, 0);
    state.apply_clifford(CliffordGate::H(0)).unwrap();
    let before = state.coefficients().to_vec();
    let mut rng = StdRng::seed_from_u64(739);
    let mut untouched_rng = StdRng::seed_from_u64(739);
    assert!(
        state
            .measure(0, MeasurementBasis::Z, &mut rng)
            .unwrap_err()
            .contains("active-state limit")
    );
    assert_eq!(state.coefficients(), before);
    assert_eq!(state.active_rank(), 0);
    assert_eq!(rng.r#gen::<u64>(), untouched_rng.r#gen::<u64>());
}
