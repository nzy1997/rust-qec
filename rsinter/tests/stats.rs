#[allow(unused_imports)]
use rsinter::stats::{Fit, log_binomial, log_factorial, fit_binomial, shot_error_rate_to_piece_error_rate};

#[test]
fn log_factorial_base_cases() {
    assert_eq!(log_factorial(0), 0.0);
    assert_eq!(log_factorial(1), 0.0);
    assert!((log_factorial(2) - 2.0_f64.ln()).abs() < 1e-10);
}

#[test]
fn log_binomial_fair_coin() {
    // P(50 heads in 100 flips of fair coin) ~ exp(-2.53)
    let result = log_binomial(0.5, 100, 50);
    assert!((result - (-2.5308762)).abs() < 0.01);
}

#[test]
fn log_binomial_edge_p0() {
    // P(hits>0 | p=0) = -inf
    assert!(log_binomial(0.0, 100, 1).is_infinite());
    assert!(log_binomial(0.0, 100, 1) < 0.0);
}

#[test]
fn log_binomial_edge_p1() {
    // P(misses>0 | p=1) = -inf
    assert!(log_binomial(1.0, 100, 99).is_infinite());
}

#[test]
fn log_binomial_all_hits_p1() {
    // P(100 hits | p=1) = 1, ln(1) = 0
    assert!((log_binomial(1.0, 100, 100) - 0.0).abs() < 1e-6);
}

#[test]
fn fit_binomial_zero_shots() {
    let f = fit_binomial(0, 0, 1000.0);
    assert_eq!(f.best, Some(0.5));
    assert_eq!(f.low, Some(0.0));
    assert_eq!(f.high, Some(1.0));
}

#[test]
fn fit_binomial_100m_shots_2_hits() {
    // sinter: Fit(low=2e-10, best=2e-08, high=1.259e-07)
    let f = fit_binomial(100_000_000, 2, 1000.0);
    assert!((f.best.unwrap() - 2e-8).abs() < 1e-10);
    assert!(f.low.unwrap() < f.best.unwrap());
    assert!(f.high.unwrap() > f.best.unwrap());
}

#[test]
fn fit_binomial_10_shots_5_hits() {
    // sinter: Fit(low=0.202, best=0.5, high=0.798)
    let f = fit_binomial(10, 5, 9.0);
    assert!((f.best.unwrap() - 0.5).abs() < 1e-6);
    assert!((f.low.unwrap() - 0.202).abs() < 0.01);
    assert!((f.high.unwrap() - 0.798).abs() < 0.01);
}

#[test]
fn piece_error_rate_identity() {
    // pieces=1 -> same rate
    let r = shot_error_rate_to_piece_error_rate(0.1, 1.0);
    assert!((r - 0.1).abs() < 1e-10);
}

#[test]
fn piece_error_rate_2_pieces() {
    // sinter: 0.05278640450004207
    let r = shot_error_rate_to_piece_error_rate(0.1, 2.0);
    assert!((r - 0.05278640450004207).abs() < 1e-8);
}

#[test]
fn piece_error_rate_100_pieces() {
    // sinter: 1.000000082740371e-11
    let r = shot_error_rate_to_piece_error_rate(1e-9, 100.0);
    assert!((r - 1e-11).abs() < 1e-13);
}
