//! Assertion-specific witnesses for the two finite arithmetic contracts in
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
//! tests/core/test_history_efficiency.py. No native test-only builder is run.

use num_bigint::BigInt;
use num_rational::BigRational;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::statistics::{EfficiencyStatus, HistoryEfficiency};

fn rational(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

fn field(x: BigRational) -> ExactField {
    FieldArithmetic::default().from_rational(x).unwrap()
}

fn summarize(rows: &[[BigRational; 4]]) -> rmagna::statistics::EfficiencySummary {
    let mut state = HistoryEfficiency::new(FieldArithmetic::default()).unwrap();
    for row in rows {
        let [a, e, w, l] = row.clone().map(field);
        state.push(&a, Some(&e), &w, &l).unwrap();
    }
    state.summarize().unwrap()
}

#[test]
fn original_weighted_residuals_include_noise_likelihood_and_zero_draw() {
    let rows = [(1, 0, 2, 1), (3, 2, 5, 3), (4, 4, 3, 6), (0, 0, 0, 1)].map(|(a, e, w, l)| {
        [
            rational(a, 5),
            rational(e, 7),
            rational(w, 3),
            rational(l, 4),
        ]
    });

    // Direct centered finite sums in Q, independent of the production field's
    // moment accumulation and its uncentered summary recurrence.
    let sum_a: BigRational = rows.iter().map(|row| &row[0]).sum();
    let sum_e: BigRational = rows.iter().map(|row| &row[1]).sum();
    let center = sum_e / &sum_a;
    let residuals = rows.each_ref().map(|row| {
        let delta = &row[1] - &center * &row[0];
        &delta * &delta
    });
    assert_eq!(center, rational(15, 28));
    assert_eq!(
        residuals,
        [
            rational(9, 784),
            rational(1, 784),
            rational(1, 49),
            rational(0, 1)
        ]
    );
    let actual: BigRational = residuals.iter().sum();
    let reference: BigRational = rows
        .iter()
        .zip(&residuals)
        .filter(|(row, _)| row[2] != rational(0, 1))
        .map(|(row, residual)| &row[3] * residual / &row[2])
        .sum();
    let factor = rational(4, 1) / (&sum_a * &sum_a);
    assert_eq!(actual, rational(13, 392));
    assert_eq!(reference, rational(1113, 31360));
    assert_eq!(&actual / &reference, rational(1040, 1113));
    assert_eq!(&factor * &actual, rational(325, 6272));
    assert_eq!(&factor * &reference, rational(5565, 100352));

    let result = summarize(&rows);
    assert_eq!(result.status, EfficiencyStatus::Available);
    assert_eq!(result.status.as_str(), "available");
    assert_eq!((result.completed, result.zero_weights), (4, 1));
    assert_eq!(result.kappa, Some(field(actual.clone() / &reference)));
    assert_eq!(result.variance_coefficient, Some(field(&factor * actual)));
    assert_eq!(
        result.exact_marginal_variance_coefficient,
        Some(field(factor * reference))
    );
    assert_eq!(result.effective_sample_size, field(rational(50, 19)));
    assert_eq!(result.mean_weight, Some(field(rational(5, 6))));
    assert_eq!(result.rho, Some(field(rational(38, 25))));

    // A reference residual that omits L must fail this contract.
    let missing_l: BigRational = rows
        .iter()
        .zip(&residuals)
        .filter(|(row, _)| row[2] != rational(0, 1))
        .map(|(row, residual)| residual / &row[2])
        .sum();
    let wrong_kappa = rational(13, 392) / missing_l;
    assert_eq!(wrong_kappa, rational(260, 301));
    assert_ne!(result.kappa, Some(field(wrong_kappa)));
}

#[test]
fn original_binary_extreme_scales_and_two_zero_draws_preserve_diagnostics() {
    for shift in [-10000_i32, 0, 10000] {
        // Construct 2^shift directly, independently of sqrt(2) scalar encoding.
        let power = BigInt::from(1) << shift.unsigned_abs() as usize;
        let scale = if shift < 0 {
            BigRational::new(1.into(), power)
        } else {
            BigRational::from_integer(power)
        };
        assert_eq!(scale.numer().bits(), if shift > 0 { 10001 } else { 1 });
        assert_eq!(scale.denom().bits(), if shift < 0 { 10001 } else { 1 });
        let rows = [
            [scale.clone(), rational(0, 1), scale.clone(), scale.clone()],
            [scale.clone(), scale.clone(), scale.clone(), scale.clone()],
        ];
        let result = summarize(&rows);
        assert_eq!(result.status, EfficiencyStatus::Available);
        assert_eq!(result.status.as_str(), "available");
        assert_eq!((result.completed, result.zero_weights), (2, 0));
        assert_eq!(result.kappa, Some(ExactField::one()));
        assert_eq!(result.variance_coefficient, Some(field(rational(1, 4))));
        assert_eq!(
            result.exact_marginal_variance_coefficient,
            Some(field(rational(1, 4)))
        );
        assert_eq!(result.effective_sample_size, field(rational(2, 1)));
        assert_eq!(result.rho, Some(ExactField::one()));
        assert_eq!(result.mean_weight, Some(field(scale)));
    }

    let zero = [
        rational(0, 1),
        rational(0, 1),
        rational(0, 1),
        rational(1, 1),
    ];
    let result = summarize(&[zero.clone(), zero]);
    assert_eq!(result.status, EfficiencyStatus::ZeroAcceptance);
    assert_eq!(result.status.as_str(), "zero_acceptance");
    assert_eq!((result.completed, result.zero_weights), (2, 2));
    assert_eq!(result.kappa, None);
    assert_eq!(result.rho, None);
    assert_eq!(result.effective_sample_size, ExactField::zero());
    assert_eq!(result.mean_weight, Some(ExactField::zero()));
    assert_eq!(result.variance_coefficient, None);
    assert_eq!(result.exact_marginal_variance_coefficient, None);
}
