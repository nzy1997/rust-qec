unsafe extern "C" {
    fn lgamma(x: f64) -> f64;
}

/// Binomial confidence interval result.
#[derive(Debug, Clone, PartialEq)]
pub struct Fit {
    pub low: Option<f64>,
    pub best: Option<f64>,
    pub high: Option<f64>,
}

/// ln(n!) via lgamma(n+1)
pub fn log_factorial(n: u64) -> f64 {
    unsafe { lgamma(n as f64 + 1.0) }
}

/// ln(P(hits = B(n, p))) -- log-probability of binomial outcome.
pub fn log_binomial(p: f64, n: u64, hits: u64) -> f64 {
    let p = p.clamp(0.0, 1.0);
    let misses = n - hits;

    if hits > 0 && p == 0.0 {
        return f64::NEG_INFINITY;
    }
    if misses > 0 && p == 1.0 {
        return f64::NEG_INFINITY;
    }

    let mut result = 0.0;
    if p > 0.0 {
        result += p.ln() * hits as f64;
    }
    if p < 1.0 {
        result += (1.0 - p).ln() * misses as f64;
    }
    result += log_factorial(n) - log_factorial(misses) - log_factorial(hits);
    result
}

/// Integer binary search over monotonically ascending function.
fn binary_search(func: impl Fn(i64) -> f64, min_x: i64, max_x: i64, target: f64) -> i64 {
    let mut lo = min_x;
    let mut hi = max_x;
    while hi > lo + 1 {
        let mid = lo + (hi - lo) / 2;
        let v = func(mid);
        if v < target {
            lo = mid;
        } else if v > target {
            hi = mid;
        } else {
            return mid;
        }
    }
    let fhi = func(hi);
    let flo = func(lo);
    let dhi = if fhi == target { 0.0 } else { (fhi - target).abs() };
    let dlo = if flo == target { 0.0 } else { (flo - target).abs() };
    if dhi < dlo { hi } else { lo }
}

/// Determine hypothesis probabilities compatible with the given hit ratio.
/// Uses binary search over log_binomial likelihoods.
pub fn fit_binomial(num_shots: u64, num_hits: u64, max_likelihood_factor: f64) -> Fit {
    if num_shots == 0 {
        return Fit { low: Some(0.0), best: Some(0.5), high: Some(1.0) };
    }
    let best_p = num_hits as f64 / num_shots as f64;
    let log_ml = log_binomial(best_p, num_shots, num_hits);
    let target = log_ml - max_likelihood_factor.ln();
    let acc: i64 = 100;

    let low = binary_search(
        |exp_err| log_binomial(exp_err as f64 / (acc as f64 * num_shots as f64), num_shots, num_hits),
        0,
        num_hits as i64 * acc,
        target,
    );
    let high = binary_search(
        |exp_err| -log_binomial(exp_err as f64 / (acc as f64 * num_shots as f64), num_shots, num_hits),
        num_hits as i64 * acc,
        num_shots as i64 * acc,
        -target,
    );

    Fit {
        best: Some(best_p),
        low: Some(low as f64 / (acc as f64 * num_shots as f64)),
        high: Some(high as f64 / (acc as f64 * num_shots as f64)),
    }
}

/// Convert per-shot error rate to per-piece (per-round) error rate.
/// Inverse of: shot_rate = 1 - (1 - 2*piece_rate)^pieces / 2
pub fn shot_error_rate_to_piece_error_rate(shot_error_rate: f64, pieces: f64) -> f64 {
    assert!((0.0..=1.0).contains(&shot_error_rate));
    assert!(pieces > 0.0);
    if pieces == 1.0 {
        return shot_error_rate;
    }
    if shot_error_rate > 0.5 {
        return 1.0 - shot_error_rate_to_piece_error_rate(1.0 - shot_error_rate, pieces);
    }
    let randomize_rate = 2.0 * shot_error_rate;
    let round_randomize_rate = 1.0 - (1.0 - randomize_rate).powf(1.0 / pieces);
    let round_error_rate = round_randomize_rate / 2.0;
    if round_error_rate == 0.0 {
        return shot_error_rate / pieces;
    }
    round_error_rate
}
