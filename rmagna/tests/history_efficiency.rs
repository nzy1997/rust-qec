use num_rational::BigRational;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::statistics::{EfficiencyStatus, HistoryEfficiency, StatisticsError};

fn r(n: i64, d: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(BigRational::new(n.into(), d.into()))
        .unwrap()
}
fn oracle(
    rows: &[[ExactField; 4]],
) -> (ExactField, ExactField, ExactField, ExactField, ExactField) {
    let f = FieldArithmetic::default();
    let sum = |values: Vec<ExactField>| {
        values
            .into_iter()
            .try_fold(ExactField::zero(), |s, x| f.add(&s, &x))
            .unwrap()
    };
    let a = sum(rows.iter().map(|v| v[0].clone()).collect());
    let e = sum(rows.iter().map(|v| v[1].clone()).collect());
    let ratio = f.divide(&e, &a).unwrap();
    let residuals = rows
        .iter()
        .map(|v| {
            let d = f
                .subtract(&v[1], &f.multiply(&ratio, &v[0]).unwrap())
                .unwrap();
            f.multiply(&d, &d).unwrap()
        })
        .collect::<Vec<_>>();
    let actual = sum(residuals.clone());
    let reference = sum(rows
        .iter()
        .zip(residuals)
        .filter(|(v, _)| !v[2].is_zero())
        .map(|(v, d)| f.multiply(&f.divide(&v[3], &v[2]).unwrap(), &d).unwrap())
        .collect());
    let factor = f
        .divide(
            &f.from_rational(BigRational::from_integer(rows.len().into()))
                .unwrap(),
            &f.multiply(&a, &a).unwrap(),
        )
        .unwrap();
    let w = sum(rows.iter().map(|v| v[2].clone()).collect());
    let w2 = sum(rows
        .iter()
        .map(|v| f.multiply(&v[2], &v[2]).unwrap())
        .collect());
    (
        f.divide(&actual, &reference).unwrap(),
        f.multiply(&factor, &actual).unwrap(),
        f.multiply(&factor, &reference).unwrap(),
        f.divide(&f.multiply(&w, &w).unwrap(), &w2).unwrap(),
        f.divide(
            &w,
            &f.from_rational(BigRational::from_integer(rows.len().into()))
                .unwrap(),
        )
        .unwrap(),
    )
}
fn native_rows() -> Vec<[ExactField; 4]> {
    [(1, 0, 2, 1), (3, 2, 5, 3), (4, 4, 3, 6), (0, 0, 0, 1)]
        .into_iter()
        .map(|(a, e, w, l)| [r(a, 5), r(e, 7), r(w, 3), r(l, 4)])
        .collect()
}
fn accumulate(rows: &[[ExactField; 4]], f: FieldArithmetic) -> HistoryEfficiency {
    let mut state = HistoryEfficiency::new(f).unwrap();
    for [a, e, w, l] in rows {
        state.push(a, Some(e), w, l).unwrap();
    }
    state
}

#[test]
fn weighted_residuals_and_noise_correction_match_independent_full_list_sums() {
    let f = FieldArithmetic::default();
    let rows = native_rows();
    let summary = accumulate(&rows, f).summarize().unwrap();
    let (kappa, actual, reference, ess, mean) = oracle(&rows);
    assert_eq!(summary.status, EfficiencyStatus::Available);
    assert_eq!(summary.status.as_str(), "available");
    assert_eq!((summary.completed, summary.zero_weights), (4, 1));
    assert_eq!(summary.kappa, Some(kappa));
    assert_eq!(summary.variance_coefficient, Some(actual));
    assert_eq!(summary.exact_marginal_variance_coefficient, Some(reference));
    assert_eq!(summary.effective_sample_size, ess);
    assert_eq!(summary.mean_weight, Some(mean));
    assert_eq!(
        summary.rho,
        Some(f.divide(&r(4, 1), &summary.effective_sample_size).unwrap())
    );
    // Omitting L from the reference residual is observably a different diagnostic.
    let wrong = rows
        .iter()
        .map(|v| [v[0].clone(), v[1].clone(), v[2].clone(), ExactField::one()])
        .collect::<Vec<_>>();
    assert_ne!(summary.kappa, Some(oracle(&wrong).0));
}

#[test]
fn exact_marginal_weights_have_kappa_one_and_common_scale_does_not_change_variance() {
    let f = FieldArithmetic::default();
    let rows = native_rows()
        .into_iter()
        .map(|mut row| {
            row[2] = row[3].clone();
            row
        })
        .collect::<Vec<_>>();
    let baseline = accumulate(&rows, f).summarize().unwrap();
    assert_eq!(baseline.kappa, Some(ExactField::one()));
    assert_eq!(
        baseline.variance_coefficient,
        baseline.exact_marginal_variance_coefficient
    );
    for binary_exponent in [-10000, 0, 10000] {
        let scale = f
            .from_scalar(
                &rmagna::scalar::ScalarArithmetic::default()
                    // omega's second exponent is a power of sqrt(2).
                    .omega(0, 2 * binary_exponent)
                    .unwrap(),
            )
            .unwrap();
        let scaled = rows
            .iter()
            .map(|row| row.each_ref().map(|x| f.multiply(x, &scale).unwrap()))
            .collect::<Vec<_>>();
        let summary = accumulate(&scaled, f).summarize().unwrap();
        assert_eq!(summary.kappa, baseline.kappa);
        assert_eq!(summary.variance_coefficient, baseline.variance_coefficient);
        assert_eq!(
            summary.exact_marginal_variance_coefficient,
            baseline.exact_marginal_variance_coefficient
        );
        assert_eq!(
            summary.effective_sample_size,
            baseline.effective_sample_size
        );
        assert_eq!(summary.rho, baseline.rho);
        assert_eq!(
            summary.mean_weight,
            Some(
                f.multiply(baseline.mean_weight.as_ref().unwrap(), &scale)
                    .unwrap()
            )
        );
    }
}

#[test]
fn irrational_contributions_and_disjoint_merges_match_the_centered_oracle() {
    let f = FieldArithmetic::default();
    let root = f
        .value([0, 1, 0, -1].map(|n| BigRational::new(n.into(), 4.into())))
        .unwrap();
    let rows = vec![
        [
            root.clone(),
            f.subtract(&root, &r(1, 4)).unwrap(),
            r(6, 5),
            r(1, 4),
        ],
        [r(1, 2), r(1, 3), r(7, 5), r(5, 3)],
        [r(0, 1), r(0, 1), r(0, 1), r(1, 1)],
        [r(1, 3), r(1, 7), r(9, 4), r(2, 7)],
    ];
    let baseline = accumulate(&rows, f).summarize().unwrap();
    let wanted = oracle(&rows);
    assert_eq!(baseline.kappa, Some(wanted.0));
    assert_eq!(baseline.variance_coefficient, Some(wanted.1));
    assert_eq!(baseline.exact_marginal_variance_coefficient, Some(wanted.2));
    for split in 0..=rows.len() {
        let mut first = accumulate(&rows[..split], f);
        first.merge(&accumulate(&rows[split..], f)).unwrap();
        assert_eq!(first.summarize().unwrap(), baseline);
    }
}

#[test]
fn unavailable_cases_distinguish_no_metric_no_draws_zero_acceptance_and_zero_residual() {
    let f = FieldArithmetic::default();
    let mut state = HistoryEfficiency::new(f).unwrap();
    let empty = state.summarize().unwrap();
    assert_eq!(empty.status, EfficiencyStatus::InsufficientSamples);
    assert!(empty.mean_weight.is_none());
    assert!(empty.rho.is_none());
    for i in 0..2 {
        state
            .push(&r(0, 1), Some(&r(0, 1)), &r(0, 1), &r(1, 1))
            .unwrap();
        let s = state.summarize().unwrap();
        assert_eq!(
            s.status,
            if i == 0 {
                EfficiencyStatus::InsufficientSamples
            } else {
                EfficiencyStatus::ZeroAcceptance
            }
        );
        assert!(s.kappa.is_none());
        assert!(s.effective_sample_size.is_zero());
        assert!(s.rho.is_none());
        assert_eq!(s.zero_weights, i + 1);
    }
    let mut state = HistoryEfficiency::new(f).unwrap();
    for _ in 0..2 {
        state
            .push(&r(1, 1), Some(&r(1, 3)), &r(2, 1), &r(1, 1))
            .unwrap();
    }
    let s = state.summarize().unwrap();
    assert_eq!(s.status, EfficiencyStatus::ZeroResidualVariance);
    assert!(
        s.kappa.is_none()
            && s.variance_coefficient.is_none()
            && s.exact_marginal_variance_coefficient.is_none()
    );
    let mut missing = HistoryEfficiency::new(f).unwrap();
    missing.push(&r(1, 1), None, &r(1, 1), &r(1, 1)).unwrap();
    state.merge(&missing).unwrap();
    assert_eq!(
        state.summarize().unwrap().status,
        EfficiencyStatus::NoErrorMetric
    );
}

#[test]
fn invalid_or_excessive_updates_and_count_overflows_are_atomic() {
    let f = FieldArithmetic::default();
    let mut state = accumulate(&native_rows(), f);
    let before = state.summarize().unwrap();
    for (row, expected) in [
        (
            [r(-1, 1), r(0, 1), r(1, 1), r(1, 1)],
            StatisticsError::InvalidProbabilityContribution,
        ),
        (
            [r(1, 1), r(2, 1), r(1, 1), r(1, 1)],
            StatisticsError::InvalidProbabilityContribution,
        ),
        (
            [r(1, 1), r(0, 1), r(-1, 1), r(1, 1)],
            StatisticsError::NegativeWeight,
        ),
        (
            [r(1, 1), r(0, 1), r(1, 1), r(-1, 1)],
            StatisticsError::NegativeNoiseLikelihood,
        ),
        (
            [r(1, 1), r(0, 1), r(0, 1), r(1, 1)],
            StatisticsError::ZeroLikelihoodContribution,
        ),
        (
            [r(1, 1), r(0, 1), r(1, 1), r(0, 1)],
            StatisticsError::ZeroLikelihoodContribution,
        ),
    ] {
        assert_eq!(
            state.push(&row[0], Some(&row[1]), &row[2], &row[3]),
            Err(expected)
        );
        assert_eq!(state.summarize().unwrap(), before);
    }
    let imaginary = f
        .value([0, 0, 1, 0].map(|n| BigRational::from_integer(n.into())))
        .unwrap();
    assert!(matches!(
        state.push(&r(1, 1), Some(&imaginary), &r(1, 1), &r(1, 1)),
        Err(StatisticsError::Field(_))
    ));
    assert_eq!(state.summarize().unwrap(), before);
    let mut bounded = HistoryEfficiency::new(FieldArithmetic::new(16)).unwrap();
    bounded
        .push(&r(1, 1), Some(&r(0, 1)), &r(1, 1), &r(1, 1))
        .unwrap();
    let before = bounded.summarize().unwrap();
    assert!(matches!(
        bounded.push(&r(1, 1), Some(&r(0, 1)), &r(1, 1), &r(1, 100_000)),
        Err(StatisticsError::Field(_))
    ));
    assert_eq!(bounded.summarize().unwrap(), before);
    let large_reference = accumulate(&[[r(1, 1), r(1, 3), r(1, 1), r(1, 100_000)]], f);
    assert!(matches!(
        bounded.merge(&large_reference),
        Err(StatisticsError::Field(_))
    ));
    assert_eq!(bounded.summarize().unwrap(), before);
    let mut huge = HistoryEfficiency::new(f).unwrap();
    huge.push(&r(0, 1), Some(&r(0, 1)), &r(0, 1), &r(1, 1))
        .unwrap();
    for _ in 0..63 {
        huge.merge(&huge.clone()).unwrap();
    }
    let before = huge.summarize().unwrap();
    assert_eq!(
        huge.merge(&huge.clone()),
        Err(StatisticsError::CountOverflow)
    );
    assert_eq!(huge.summarize().unwrap(), before);
}
