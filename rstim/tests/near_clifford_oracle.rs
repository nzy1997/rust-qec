#[path = "support/near_clifford_oracle.rs"]
mod oracle;

use oracle::{Amp, DenseOracle};

fn assert_close(actual: Amp, expected: Amp) {
    let error = ((actual.re - expected.re).powi(2) + (actual.im - expected.im).powi(2)).sqrt();
    assert!(
        error < 1e-12,
        "actual={actual:?}, expected={expected:?}, error={error}"
    );
}

#[test]
fn near_clifford_two_t_oracle_known_answer() {
    let mut state = DenseOracle::new(2);
    state.h(0);
    state.cx(0, 1);
    state.t(0);
    state.h(0);
    state.t(0);

    let w = std::f64::consts::FRAC_1_SQRT_2;
    let expected = [
        Amp::new(0.5, 0.0),
        Amp::new(w / 2.0, w / 2.0),
        Amp::new(w / 2.0, w / 2.0),
        Amp::new(0.0, -0.5),
    ];
    for (actual, expected) in state.amplitudes().iter().zip(expected) {
        assert_close(*actual, expected);
        assert!((actual.norm_sqr() - 0.25).abs() < 1e-12);
    }
    assert!((state.pauli_xx_expectation(0, 1) - 0.5).abs() < 1e-12);
    assert!((state.even_x_parity_probability(0, 1) - 0.75).abs() < 1e-12);
}

#[test]
fn near_clifford_two_t_oracle_gauge_coefficients() {
    let mut state = DenseOracle::new(2);
    state.h(0);
    state.cx(0, 1);
    state.t(0);
    state.h(0);
    state.t(0);

    let phi = [0.5, 0.5, 0.5, -0.5];
    let global = Amp::new(
        std::f64::consts::FRAC_1_SQRT_2,
        -std::f64::consts::FRAC_1_SQRT_2,
    );
    let c = (std::f64::consts::PI / 8.0).cos();
    let s = (std::f64::consts::PI / 8.0).sin();
    let expected = [
        Amp::new(c * c, 0.0),
        Amp::new(0.0, -c * s),
        Amp::new(0.0, -s * c),
        Amp::new(-s * s, 0.0),
    ];
    for (sector, expected) in expected.into_iter().enumerate() {
        let a = (sector >> 1) & 1;
        let b = sector & 1;
        let projected =
            state
                .amplitudes()
                .iter()
                .enumerate()
                .fold(Amp::default(), |sum, (index, amp)| {
                    let q0 = index >> 1;
                    let q1 = index & 1;
                    let sign = if (a * q1 + b * q0) & 1 == 0 {
                        1.0
                    } else {
                        -1.0
                    };
                    sum + *amp * (phi[index] * sign)
                });
        assert_close(projected * global, expected);
    }
}

#[test]
fn near_clifford_oracle_t_then_t_dag_is_identity() {
    let mut state = DenseOracle::new(3);
    state.h(0);
    state.cx(0, 2);
    let before = state.amplitudes().to_vec();
    state.t(0);
    state.t_dag(0);
    for (actual, expected) in state.amplitudes().iter().zip(before) {
        assert_close(*actual, expected);
    }
}
