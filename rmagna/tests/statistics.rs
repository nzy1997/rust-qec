use num_rational::BigRational as R;
use num_traits::{One, Zero};
use rmagna::field::{ExactField, FieldArithmetic, FieldError};
use rmagna::statistics::{ExactStatistics, StatisticsError, StatisticsOptions};

fn r(n: i64, d: i64) -> R {
    R::new(n.into(), d.into())
}
fn f(value: R) -> ExactField {
    FieldArithmetic::default().from_rational(value).unwrap()
}
fn rational(value: &ExactField) -> R {
    assert!(value.coefficients()[1..].iter().all(Zero::is_zero));
    value.coefficients()[0].clone()
}
fn toy() -> (Vec<Vec<R>>, Vec<R>) {
    (
        vec![
            vec![r(1, 1), r(1, 2), r(-1, 2)],
            vec![r(1, 2), r(0, 1), r(1, 4)],
            vec![r(0, 1), r(0, 1), r(0, 1)],
            vec![r(1, 1), r(1, 2), r(-1, 3)],
        ],
        vec![r(2, 1), r(1, 2), r(0, 1), r(3, 2)],
    )
}
#[test]
fn weighted_joint_means_covariance_and_paired_ratio_match_centered_full_list_sums() {
    let (rows, weights) = toy();
    let mut stats = ExactStatistics::new(3, StatisticsOptions::default()).unwrap();
    for (row, weight) in rows.iter().zip(&weights) {
        stats
            .push(
                &row.iter().cloned().map(f).collect::<Vec<_>>(),
                &f(weight.clone()),
            )
            .unwrap();
    }
    let summary = stats.summarize().unwrap().unwrap();
    let n = R::from_integer(rows.len().into());
    let means = (0..3)
        .map(|j| rows.iter().map(|row| &row[j]).sum::<R>() / &n)
        .collect::<Vec<_>>();
    assert_eq!(
        summary.means.iter().map(rational).collect::<Vec<_>>(),
        means
    );
    let cov = summary.covariance_of_mean.unwrap();
    for i in 0..3 {
        for j in 0..3 {
            let centered = rows
                .iter()
                .map(|row| (&row[i] - &means[i]) * (&row[j] - &means[j]))
                .sum::<R>();
            assert_eq!(rational(&cov[i][j]), centered / (&n * (&n - R::one())));
        }
    }
    assert_eq!(rational(&summary.mean_weight), R::one());
    assert_eq!(rational(&summary.effective_sample_size), r(32, 13));
    assert_eq!(summary.count, 4); // The zero-weight draw remains in the mean.
    let ratio = stats.paired_ratio(1, 0).unwrap().unwrap();
    assert_eq!(rational(&ratio.ratio), r(2, 5));
    let residuals = rows
        .iter()
        .map(|row| {
            let z = &row[1] - r(2, 5) * &row[0];
            &z * &z
        })
        .sum::<R>();
    assert_eq!(
        rational(ratio.variance.as_ref().unwrap()),
        residuals / (&n * (&n - R::one()) * &means[0] * &means[0])
    );
    assert_eq!(rational(ratio.variance.as_ref().unwrap()), r(8, 625));
    let average_of_ratios = rows
        .iter()
        .map(|row| {
            if row[0].is_zero() {
                R::zero()
            } else {
                &row[1] / &row[0]
            }
        })
        .sum::<R>()
        / n;
    assert_ne!(rational(&ratio.ratio), average_of_ratios);
    assert_eq!(
        stats.paired_ratio(0, 0).unwrap().unwrap().variance,
        Some(ExactField::zero())
    );
}
#[test]
fn disjoint_partition_merges_match_uninterrupted_streams_including_empty_partitions() {
    let (rows, weights) = toy();
    for cut in 0..=rows.len() {
        let mut whole = ExactStatistics::new(3, StatisticsOptions::default()).unwrap();
        let mut left = whole.clone();
        let mut right = whole.clone();
        for (j, (row, w)) in rows.iter().zip(&weights).enumerate() {
            let values = row.iter().cloned().map(f).collect::<Vec<_>>();
            let weight = f(w.clone());
            whole.push(&values, &weight).unwrap();
            if j < cut {
                left.push(&values, &weight).unwrap();
            } else {
                right.push(&values, &weight).unwrap();
            }
        }
        left.merge(&right).unwrap();
        assert_eq!(left.summarize().unwrap(), whole.summarize().unwrap());
        assert_eq!(
            left.paired_ratio(1, 0).unwrap(),
            whole.paired_ratio(1, 0).unwrap()
        );
    }
}
#[test]
fn real_cyclotomic_values_and_tiny_variances_remain_exact() {
    let a = FieldArithmetic::default();
    let root_two = a
        .value([R::zero(), R::one(), R::zero(), -R::one()])
        .unwrap();
    let mut stats = ExactStatistics::new(2, StatisticsOptions::default()).unwrap();
    stats
        .push(&[root_two.clone(), f(r(1, 1))], &ExactField::one())
        .unwrap();
    stats
        .push(
            &[a.scale(&root_two, &r(2, 1)).unwrap(), f(r(3, 1))],
            &ExactField::one(),
        )
        .unwrap();
    let summary = stats.summarize().unwrap().unwrap();
    assert_eq!(summary.means[0], a.scale(&root_two, &r(3, 2)).unwrap());
    let cov = summary.covariance_of_mean.unwrap();
    assert_eq!(rational(&cov[0][0]), r(1, 2));
    assert_eq!(rational(&cov[1][1]), r(1, 1));
    assert_eq!(cov[0][1], a.scale(&root_two, &r(1, 2)).unwrap());
    assert_eq!(
        stats.paired_ratio(1, 0).unwrap().unwrap().ratio,
        a.scale(&root_two, &r(2, 3)).unwrap()
    );
    assert_eq!(
        rational(&stats.paired_ratio(1, 0).unwrap().unwrap().variance.unwrap()),
        r(2, 81)
    );
    let tiny = R::new(1.into(), num_bigint::BigInt::from(10).pow(400));
    let mut stats = ExactStatistics::new(2, StatisticsOptions::default()).unwrap();
    stats
        .push(&[ExactField::one(), f(tiny.clone())], &ExactField::one())
        .unwrap();
    stats
        .push(&[ExactField::one(), f(&tiny * r(2, 1))], &ExactField::one())
        .unwrap();
    let ratio = stats.paired_ratio(1, 0).unwrap().unwrap();
    assert_eq!(rational(&ratio.ratio), &tiny * r(3, 2));
    assert_eq!(rational(&ratio.variance.unwrap()), &tiny * &tiny / r(4, 1));
}
#[test]
fn empty_singleton_zero_denominator_and_weight_only_statistics_are_explicit() {
    let mut stats = ExactStatistics::new(2, StatisticsOptions::default()).unwrap();
    assert_eq!(stats.count(), 0);
    assert_eq!(stats.num_quantities(), 2);
    assert_eq!(stats.summarize().unwrap(), None);
    assert_eq!(stats.paired_ratio(1, 0).unwrap(), None);
    stats
        .push(&[f(r(0, 1)), f(r(1, 1))], &ExactField::zero())
        .unwrap();
    let s = stats.summarize().unwrap().unwrap();
    assert_eq!(s.covariance_of_mean, None);
    assert_eq!(s.effective_sample_size, ExactField::zero());
    assert_eq!(stats.paired_ratio(1, 0).unwrap(), None);
    assert_eq!(stats.paired_ratio(0, 1).unwrap().unwrap().variance, None);
    let mut weights = ExactStatistics::new(0, StatisticsOptions::default()).unwrap();
    weights.push(&[], &f(r(3, 1))).unwrap();
    weights.push(&[], &f(r(1, 1))).unwrap();
    let s = weights.summarize().unwrap().unwrap();
    assert!(s.means.is_empty());
    assert_eq!(rational(&s.mean_weight), r(2, 1));
    assert_eq!(rational(&s.effective_sample_size), r(8, 5));
}
#[test]
fn invalid_shapes_complex_values_negative_weights_and_resource_failures_are_atomic() {
    let mut stats = ExactStatistics::new(
        1,
        StatisticsOptions {
            arithmetic: FieldArithmetic::new(5),
            ..Default::default()
        },
    )
    .unwrap();
    stats.push(&[f(r(4, 1))], &ExactField::one()).unwrap();
    let before = stats.summarize().unwrap();
    assert!(matches!(
        stats.push(&[f(r(4, 1))], &ExactField::one()),
        Err(StatisticsError::Field(FieldError::ResourceLimit { .. }))
    ));
    assert_eq!(stats.summarize().unwrap(), before);
    assert!(matches!(
        stats.push(&[], &ExactField::one()),
        Err(StatisticsError::DimensionMismatch { .. })
    ));
    assert_eq!(stats.summarize().unwrap(), before);
    let complex = FieldArithmetic::default()
        .value([R::zero(), R::zero(), R::one(), R::zero()])
        .unwrap();
    assert_eq!(
        stats.push(std::slice::from_ref(&complex), &ExactField::one()),
        Err(StatisticsError::Field(FieldError::NonReal))
    );
    assert_eq!(
        stats.push(&[ExactField::one()], &complex),
        Err(StatisticsError::Field(FieldError::NonReal))
    );
    assert_eq!(
        stats.push(&[ExactField::one()], &f(r(-1, 1))),
        Err(StatisticsError::NegativeWeight)
    );
    assert_eq!(stats.summarize().unwrap(), before);
    let mut larger = ExactStatistics::new(1, StatisticsOptions::default()).unwrap();
    larger.push(&[f(r(8, 1))], &ExactField::one()).unwrap();
    assert!(matches!(
        stats.merge(&larger),
        Err(StatisticsError::Field(FieldError::ResourceLimit { .. }))
    ));
    assert_eq!(stats.summarize().unwrap(), before);
    assert!(matches!(
        stats.merge(&ExactStatistics::new(2, StatisticsOptions::default()).unwrap()),
        Err(StatisticsError::DimensionMismatch { .. })
    ));
    assert!(matches!(
        stats.paired_ratio(1, 0),
        Err(StatisticsError::InvalidIndex { index: 1, .. })
    ));
    assert!(matches!(
        ExactStatistics::new(65, StatisticsOptions::default()),
        Err(StatisticsError::DimensionLimit { .. })
    ));
    assert!(matches!(
        ExactStatistics::new(
            usize::MAX,
            StatisticsOptions {
                max_dimensions: usize::MAX,
                ..Default::default()
            }
        ),
        Err(StatisticsError::SizeOverflow)
    ));
}
#[test]
fn count_overflow_in_push_and_merge_preserves_the_complete_accumulator() {
    let mut power = ExactStatistics::new(1, StatisticsOptions::default()).unwrap();
    power
        .push(&[ExactField::zero()], &ExactField::zero())
        .unwrap();
    let mut total = ExactStatistics::new(1, StatisticsOptions::default()).unwrap();
    for bit in 0..64 {
        total.merge(&power).unwrap();
        if bit < 63 {
            power.merge(&power.clone()).unwrap();
        }
    }
    assert_eq!(total.count(), u64::MAX);
    let before = total.summarize().unwrap();
    assert_eq!(
        total.push(&[ExactField::zero()], &ExactField::zero()),
        Err(StatisticsError::CountOverflow)
    );
    assert_eq!(total.merge(&power), Err(StatisticsError::CountOverflow));
    assert_eq!(total.summarize().unwrap(), before);
}

#[test]
fn failures_after_computing_contribution_sums_do_not_partially_update_weights() {
    let mut stats = ExactStatistics::new(
        1,
        StatisticsOptions {
            arithmetic: FieldArithmetic::new(5),
            ..Default::default()
        },
    )
    .unwrap();
    stats.push(&[ExactField::zero()], &f(r(4, 1))).unwrap();
    let inspect = |source: &ExactStatistics| {
        let mut wide = ExactStatistics::new(1, StatisticsOptions::default()).unwrap();
        wide.merge(source).unwrap();
        wide.summarize().unwrap()
    };
    let before = inspect(&stats);
    assert!(matches!(
        stats.push(&[ExactField::one()], &f(r(4, 1))),
        Err(StatisticsError::Field(FieldError::ResourceLimit { .. }))
    ));
    assert_eq!(stats.count(), 1);
    assert_eq!(inspect(&stats), before);
}
