use num_rational::BigRational as R;
use num_traits::{One, Zero};
use rmagna::field::{ExactField, FieldArithmetic, FieldError};
use rmagna::scalar::ScalarArithmetic;
use std::cmp::Ordering;
type Q = [R; 2];
type C = [Q; 2];
fn qadd(a: &Q, b: &Q) -> Q {
    [&a[0] + &b[0], &a[1] + &b[1]]
}
fn qneg(a: &Q) -> Q {
    [-&a[0], -&a[1]]
}
fn qmul(a: &Q, b: &Q) -> Q {
    [
        &a[0] * &b[0] + R::from_integer(2.into()) * &a[1] * &b[1],
        &a[0] * &b[1] + &a[1] * &b[0],
    ]
}
fn qinv(a: &Q) -> Q {
    let norm = &a[0] * &a[0] - R::from_integer(2.into()) * &a[1] * &a[1];
    [&a[0] / &norm, -&a[1] / norm]
}
fn cmul(a: &C, b: &C) -> C {
    [
        qadd(&qmul(&a[0], &b[0]), &qneg(&qmul(&a[1], &b[1]))),
        qadd(&qmul(&a[0], &b[1]), &qmul(&a[1], &b[0])),
    ]
}
fn cdiv(a: &C, b: &C) -> C {
    let norm = qadd(&qmul(&b[0], &b[0]), &qmul(&b[1], &b[1]));
    let inverse = qinv(&norm);
    let top = cmul(a, &[b[0].clone(), qneg(&b[1])]);
    [qmul(&top[0], &inverse), qmul(&top[1], &inverse)]
}
fn embedding(value: &ExactField) -> C {
    let [a, b, c, d] = value.coefficients();
    [
        [a.clone(), (b - d) / R::from_integer(2.into())],
        [c.clone(), (b + d) / R::from_integer(2.into())],
    ]
}
fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
#[test]
fn rational_cyclotomic_operations_match_an_independent_complex_quadratic_field() {
    let arithmetic = FieldArithmetic::new(4096);
    let mut seed = 20261025_u64;
    for _ in 0..128 {
        let mut value = || {
            arithmetic
                .value(std::array::from_fn(|_| {
                    R::new(
                        (next(&mut seed) as i64 % 2001 - 1000).into(),
                        (next(&mut seed) % 31 + 1).into(),
                    )
                }))
                .unwrap()
        };
        let a = value();
        let b = value();
        let x = embedding(&a);
        let y = embedding(&b);
        assert_eq!(
            embedding(&arithmetic.add(&a, &b).unwrap()),
            [qadd(&x[0], &y[0]), qadd(&x[1], &y[1])]
        );
        assert_eq!(
            embedding(&arithmetic.subtract(&a, &b).unwrap()),
            [qadd(&x[0], &qneg(&y[0])), qadd(&x[1], &qneg(&y[1]))]
        );
        assert_eq!(
            embedding(&arithmetic.multiply(&a, &b).unwrap()),
            cmul(&x, &y)
        );
        assert_eq!(embedding(&a.conjugate()), [x[0].clone(), qneg(&x[1])]);
        if !b.is_zero() {
            assert_eq!(embedding(&arithmetic.divide(&a, &b).unwrap()), cdiv(&x, &y));
        }
    }
}
#[test]
fn normalized_histories_and_tiny_weighted_errors_keep_exact_values() {
    let a = FieldArithmetic::default();
    let scalar = ScalarArithmetic::default();
    let history = a
        .from_scalar(
            &scalar
                .scalar([2.into(), 1.into(), 0.into(), (-1).into()], -2)
                .unwrap(),
        )
        .unwrap();
    let conditional = a.divide(&history, &history).unwrap();
    assert_eq!(conditional, ExactField::one());
    let third = R::new(1.into(), 3.into());
    let weighted = a.scale(&conditional, &third).unwrap();
    assert_eq!(weighted, a.from_rational(third.clone()).unwrap());
    let tiny = a
        .from_scalar(
            &scalar
                .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1200)
                .unwrap(),
        )
        .unwrap();
    let success = a.subtract(&ExactField::one(), &tiny).unwrap();
    let error = a.subtract(&ExactField::one(), &success).unwrap();
    assert_eq!(error, tiny);
    assert_eq!(
        a.divide(&a.scale(&tiny, &third).unwrap(), &tiny).unwrap(),
        weighted
    );
    assert_eq!(
        a.multiply(&a.divide(&ExactField::one(), &history).unwrap(), &history)
            .unwrap(),
        ExactField::one()
    );
}
#[test]
fn canonical_rationals_raw_inputs_and_resource_limits_are_explicit() {
    let a = FieldArithmetic::new(128);
    let raw = R::new_raw(6.into(), (-9).into());
    assert_eq!(
        a.from_rational(raw).unwrap(),
        a.from_rational(R::new((-2).into(), 3.into())).unwrap()
    );
    assert_eq!(
        a.from_rational(R::new_raw(1.into(), 0.into())),
        Err(FieldError::ZeroDenominator)
    );
    assert_eq!(
        a.divide(&ExactField::one(), &ExactField::zero()),
        Err(FieldError::DivisionByZero)
    );
    assert!(matches!(
        FieldArithmetic::new(1).from_rational(R::new(1.into(), 3.into())),
        Err(FieldError::ResourceLimit { .. })
    ));
    assert_eq!(
        FieldArithmetic::new(0).from_rational(R::zero()).unwrap(),
        ExactField::zero()
    );
    let scalar = ScalarArithmetic::default();
    for power in [i64::MIN, i64::MAX] {
        let value = scalar
            .scalar([1.into(), 0.into(), 0.into(), 0.into()], power)
            .unwrap();
        assert!(matches!(
            a.from_scalar(&value),
            Err(FieldError::ResourceLimit { .. })
        ));
    }
    let large = FieldArithmetic::default()
        .from_rational(R::new((rmagna::BigInt::one() << 200) - 1, 1.into()))
        .unwrap();
    assert!(matches!(
        a.multiply(&large, &large),
        Err(FieldError::ResourceLimit { .. })
    ));
}
#[test]
fn exact_real_order_handles_rational_denominators_and_severe_pell_cancellation() {
    let a = FieldArithmetic::default();
    for x in -10..=10 {
        for y in -10..=10 {
            let value = a
                .value([
                    R::new(x.into(), 7.into()),
                    R::new(y.into(), 11.into()),
                    R::zero(),
                    R::new((-y).into(), 11.into()),
                ])
                .unwrap();
            let approx = f64::from(x) / 7. + 2_f64.sqrt() * f64::from(y) / 11.;
            assert_eq!(value.real_sign().unwrap(), approx.partial_cmp(&0.).unwrap());
        }
    }
    let mut p = rmagna::BigInt::one();
    let mut q = rmagna::BigInt::one();
    for index in 0..1024 {
        let old = p.clone();
        p = &p + (&q << 1);
        q = old + q;
        let value = a
            .value([
                R::new(p.clone(), 3.into()),
                R::new(-q.clone(), 3.into()),
                R::zero(),
                R::new(q.clone(), 3.into()),
            ])
            .unwrap();
        assert_eq!(
            value.real_sign().unwrap(),
            if index % 2 == 0 {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        );
    }
    let imaginary = a
        .value([R::zero(), R::zero(), R::one(), R::zero()])
        .unwrap();
    assert_eq!(imaginary.real_sign(), Err(FieldError::NonReal));
}

#[test]
fn joint_history_contributions_apply_noise_weight_after_exact_normalization() {
    let a = FieldArithmetic::default();
    let scalar = ScalarArithmetic::default();
    let history = scalar
        .scalar([2.into(), 1.into(), 0.into(), (-1).into()], -4)
        .unwrap();
    let likelihood = R::new(7.into(), 13.into());
    assert_eq!(
        a.contribution(&history, &history, &likelihood).unwrap(),
        a.from_rational(likelihood.clone()).unwrap()
    );
    assert_eq!(
        a.contribution(&rmagna::scalar::ExactScalar::zero(), &history, &likelihood)
            .unwrap(),
        ExactField::zero()
    );
    assert_eq!(
        a.contribution(&history, &rmagna::scalar::ExactScalar::zero(), &likelihood),
        Err(FieldError::DivisionByZero)
    );
}
