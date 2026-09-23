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
fn three_active_axes_preserve_pauli_probabilities_through_sequential_measurements() {
    let mut active = ActiveState::new(3, 4);
    let mut oracle = DenseOracle::new(3);
    for q in 0..3 {
        active.apply_clifford(CliffordGate::H(q)).unwrap();
        oracle.h(q);
    }
    active.apply_clifford(CliffordGate::CX(0, 1)).unwrap();
    oracle.cx(0, 1);
    for q in 0..3 {
        active.t(q).unwrap();
        oracle.t(q);
    }
    active.apply_clifford(CliffordGate::X(2)).unwrap();
    oracle.x(2);
    active.apply_clifford(CliffordGate::H(1)).unwrap();
    oracle.h(1);
    active.t_dag(0).unwrap();
    oracle.t_dag(0);
    assert!(active.active_rank() >= 3);

    let mut rng = StdRng::seed_from_u64(739);
    for (q, basis, letter) in [
        (2, MeasurementBasis::Y, 'Y'),
        (0, MeasurementBasis::X, 'X'),
        (1, MeasurementBasis::Z, 'Z'),
    ] {
        let (zero, one) = active.measurement_probabilities(q, basis).unwrap();
        assert!((zero - oracle.measurement_probability(q, letter, false)).abs() < 1e-12);
        assert!((one - oracle.measurement_probability(q, letter, true)).abs() < 1e-12);
        let outcome = active.measure(q, basis, &mut rng).unwrap();
        oracle.collapse(q, letter, outcome);
        for check_q in 0..3 {
            for (check_basis, check_letter) in [
                (MeasurementBasis::X, 'X'),
                (MeasurementBasis::Y, 'Y'),
                (MeasurementBasis::Z, 'Z'),
            ] {
                let (actual_zero, actual_one) = active
                    .measurement_probabilities(check_q, check_basis)
                    .unwrap();
                assert!(
                    (actual_zero - oracle.measurement_probability(check_q, check_letter, false))
                        .abs()
                        < 1e-12
                );
                assert!(
                    (actual_one - oracle.measurement_probability(check_q, check_letter, true))
                        .abs()
                        < 1e-12
                );
            }
        }
    }
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
    let mut state = ActiveState::new(2, 1);
    state.apply_clifford(CliffordGate::H(0)).unwrap();
    state.t(0).unwrap();
    state.apply_clifford(CliffordGate::H(1)).unwrap();
    let before = state.coefficients().to_vec();
    let mut rng = StdRng::seed_from_u64(739);
    let mut untouched_rng = StdRng::seed_from_u64(739);
    assert!(
        state
            .measure(1, MeasurementBasis::Z, &mut rng)
            .unwrap_err()
            .contains("active-state limit")
    );
    assert_eq!(state.coefficients(), before);
    assert_eq!(state.active_rank(), 1);
    assert_eq!(rng.r#gen::<u64>(), untouched_rng.r#gen::<u64>());
}

#[test]
fn wide_tableau_pauli_rows_match_two_qubit_oracle_across_measurements() {
    fn prepare(n: usize, a: usize, b: usize) -> ActiveState {
        let mut state = ActiveState::new(n, 8);
        state.apply_clifford(CliffordGate::H(a)).unwrap();
        state.apply_clifford(CliffordGate::CX(a, b)).unwrap();
        state.apply_clifford(CliffordGate::S(b)).unwrap();
        state.t(a).unwrap();
        state.apply_clifford(CliffordGate::H(b)).unwrap();
        state.t_dag(b).unwrap();
        state
    }
    let mut wide = prepare(130, 63, 129);
    let mut narrow = prepare(2, 0, 1);
    let mut wide_rng = StdRng::seed_from_u64(739);
    let mut narrow_rng = StdRng::seed_from_u64(739);
    for (wide_q, narrow_q, basis) in [
        (129, 1, MeasurementBasis::Y),
        (63, 0, MeasurementBasis::X),
        (129, 1, MeasurementBasis::Z),
    ] {
        for (wq, nq) in [(63, 0), (129, 1)] {
            for check_basis in [
                MeasurementBasis::X,
                MeasurementBasis::Y,
                MeasurementBasis::Z,
            ] {
                let actual = wide.measurement_probabilities(wq, check_basis).unwrap();
                let expected = narrow.measurement_probabilities(nq, check_basis).unwrap();
                assert!((actual.0 - expected.0).abs() < 1e-12);
                assert!((actual.1 - expected.1).abs() < 1e-12);
            }
        }
        assert_eq!(
            wide.measure(wide_q, basis, &mut wide_rng).unwrap(),
            narrow.measure(narrow_q, basis, &mut narrow_rng).unwrap()
        );
        wide.retire_fixed_axes();
        narrow.retire_fixed_axes();
    }
    assert_eq!(wide_rng.r#gen::<u64>(), narrow_rng.r#gen::<u64>());
}
