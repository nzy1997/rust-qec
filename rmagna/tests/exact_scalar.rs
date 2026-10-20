use num_traits::{One, ToPrimitive, Zero};
use rmagna::BigInt;
use rmagna::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use std::cmp::Ordering;
use std::collections::HashSet;

fn value(arithmetic: &ScalarArithmetic, c: [i64; 4], power: i64) -> ExactScalar {
    arithmetic.scalar(c.map(BigInt::from), power).unwrap()
}

#[test]
fn exact_large_integer_witnesses_match_the_pinned_upstream() {
    let arithmetic = ScalarArithmetic::default();
    let data: serde_json::Value =
        serde_json::from_str(include_str!("data/scalar-reference.json")).unwrap();
    assert_eq!(data["schema"], "rmagna.scalar-reference.v1");
    assert_eq!(
        data["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    let vectors = data["vectors"].as_array().unwrap();
    assert_eq!(vectors.len(), 16);
    let decode = |v: &serde_json::Value| {
        arithmetic
            .scalar(
                std::array::from_fn(|i| {
                    v["coefficients"][i]
                        .as_str()
                        .unwrap()
                        .parse::<BigInt>()
                        .unwrap()
                }),
                v["power"].as_i64().unwrap(),
            )
            .unwrap()
    };
    let check = |actual: &ExactScalar, expected: &serde_json::Value| {
        assert_eq!(actual.power(), expected["power"].as_i64().unwrap());
        for (i, c) in actual.coefficients().iter().enumerate() {
            assert_eq!(c.to_string(), expected["coefficients"][i].as_str().unwrap());
        }
    };
    for vector in vectors {
        let a = decode(&vector["left"]);
        let b = decode(&vector["right"]);
        check(&a, &vector["normalized_left"]);
        check(&b, &vector["normalized_right"]);
        check(&arithmetic.add(&a, &b).unwrap(), &vector["sum"]);
        check(&arithmetic.multiply(&a, &b).unwrap(), &vector["product"]);
        check(&a.conjugate(), &vector["conjugate_left"]);
    }
}

#[test]
fn normalization_preserves_value_and_canonical_hash_identity() {
    let arithmetic = ScalarArithmetic::default();
    let a = value(&arithmetic, [6, -10, 14, -22], -23);
    let b = value(&arithmetic, [3, -5, 7, -11], -22);
    assert_eq!(a, b);
    assert_eq!(HashSet::from([a, b]).len(), 1);
    let zero = value(&arithmetic, [0; 4], i64::MIN);
    assert_eq!(zero, ExactScalar::zero());
    assert_eq!(zero.power(), 0);
    let shifted = BigInt::from(-3) << 4097_usize;
    let large = arithmetic
        .scalar([shifted, 0.into(), 0.into(), 0.into()], -4097)
        .unwrap();
    assert_eq!(large, arithmetic.integer(-3).unwrap());
}

#[test]
fn omega_group_conjugation_and_square_root_scales_are_exact() {
    let arithmetic = ScalarArithmetic::default();
    for phase in -16_i64..=16 {
        for other in -16_i64..=16 {
            for scale in [-5, -2, -1, 0, 1, 2, 5] {
                let a = arithmetic.omega(phase, scale).unwrap();
                let b = arithmetic.omega(other, -scale).unwrap();
                assert_eq!(
                    arithmetic.multiply(&a, &b).unwrap(),
                    arithmetic.omega(phase + other, 0).unwrap()
                );
                assert_eq!(a.conjugate(), arithmetic.omega(-phase, scale).unwrap());
                assert_eq!(
                    arithmetic.norm_squared(&a).unwrap(),
                    value(&arithmetic, [1, 0, 0, 0], scale)
                );
            }
        }
    }
    assert_eq!(arithmetic.omega(i64::MIN, 0).unwrap(), ExactScalar::one());
    assert_eq!(
        arithmetic.omega(i64::MAX, 0).unwrap(),
        arithmetic.omega(7, 0).unwrap()
    );
    for power in [i64::MIN, i64::MIN + 1, i64::MAX - 1, i64::MAX] {
        for phase in 0..8 {
            assert_eq!(
                arithmetic
                    .norm_squared(&arithmetic.omega(phase, power).unwrap())
                    .unwrap(),
                value(&arithmetic, [1, 0, 0, 0], power)
            );
        }
    }
}

#[test]
fn interference_and_tiny_accepted_error_survive_exact_subtraction() {
    let arithmetic = ScalarArithmetic::default();
    let one = ExactScalar::one();
    let half = value(&arithmetic, [1, 0, 0, 0], -1);
    let omega = arithmetic.omega(1, 0).unwrap();
    let plus = arithmetic
        .multiply(&half, &arithmetic.add(&one, &omega).unwrap())
        .unwrap();
    let minus = arithmetic
        .multiply(&half, &arithmetic.subtract(&one, &omega).unwrap())
        .unwrap();
    assert_eq!(
        arithmetic
            .add(
                &arithmetic.norm_squared(&plus).unwrap(),
                &arithmetic.norm_squared(&minus).unwrap()
            )
            .unwrap(),
        one
    );
    let mut cancelled = ExactScalar::zero();
    for phase in 0..8 {
        cancelled = arithmetic
            .add(&cancelled, &arithmetic.omega(phase, 0).unwrap())
            .unwrap();
    }
    assert_eq!(cancelled, ExactScalar::zero());
    let rare = value(&arithmetic, [1, 0, 0, 0], -1200);
    let success = arithmetic.subtract(&one, &rare).unwrap();
    assert_eq!(arithmetic.subtract(&one, &success).unwrap(), rare);
    assert_eq!(success.coefficients()[0].bits(), 1200);
}

#[test]
fn pell_convergents_keep_the_correct_sign_after_thousands_of_bits_cancel() {
    let arithmetic = ScalarArithmetic::default();
    let (mut a, mut b) = (BigInt::one(), BigInt::zero());
    for n in 1..=2048 {
        (a, b) = (&a + (&b << 1_usize), &a + &b);
        if n <= 10 || n % 127 == 0 || n == 2048 {
            let tiny = arithmetic
                .scalar([a.clone(), -&b, 0.into(), b.clone()], -9000)
                .unwrap();
            assert_eq!(
                tiny.real_sign().unwrap(),
                if n % 2 == 0 {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            );
            let large = arithmetic
                .scalar([a.clone(), b.clone(), 0.into(), -&b], 9000)
                .unwrap();
            assert_eq!(
                arithmetic.multiply(&tiny, &large).unwrap(),
                arithmetic.integer(if n % 2 == 0 { 1 } else { -1 }).unwrap()
            );
        }
    }
    assert!(a.bits() > 2048);
    assert_eq!(ExactScalar::zero().real_sign().unwrap(), Ordering::Equal);
    assert_eq!(
        arithmetic.omega(1, 0).unwrap().real_sign(),
        Err(ScalarError::NonReal)
    );
}

fn complex(value: &ExactScalar) -> (f64, f64) {
    let c = value.coefficients().each_ref().map(|c| c.to_f64().unwrap());
    let t = std::f64::consts::FRAC_1_SQRT_2;
    let scale = 2_f64.powi(value.power().try_into().unwrap());
    (
        (c[0] + t * (c[1] - c[3])) * scale,
        (c[2] + t * (c[1] + c[3])) * scale,
    )
}

#[test]
fn mixed_coefficients_agree_with_an_independent_complex_embedding() {
    let arithmetic = ScalarArithmetic::default();
    let mut state = 0x23456abcdef_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % 31) as i64 - 15
    };
    for _ in 0..512 {
        let a = value(&arithmetic, std::array::from_fn(|_| next()), next());
        let b = value(&arithmetic, std::array::from_fn(|_| next()), next());
        let (ar, ai) = complex(&a);
        let (br, bi) = complex(&b);
        for (actual, expected) in [
            (
                complex(&arithmetic.multiply(&a, &b).unwrap()),
                (ar * br - ai * bi, ar * bi + ai * br),
            ),
            (
                complex(&arithmetic.add(&a, &b).unwrap()),
                (ar + br, ai + bi),
            ),
        ] {
            let bound = 2e-14 * (ar.abs() + ai.abs() + 1.0) * (br.abs() + bi.abs() + 1.0);
            assert!(
                (actual.0 - expected.0).abs() <= bound,
                "{actual:?} != {expected:?}"
            );
            assert!(
                (actual.1 - expected.1).abs() <= bound,
                "{actual:?} != {expected:?}"
            );
        }
        let norm = arithmetic.norm_squared(&a).unwrap();
        assert!(norm.is_real());
        assert_ne!(norm.real_sign().unwrap(), Ordering::Less);
        assert_eq!(
            arithmetic.subtract(&a, &b).unwrap(),
            arithmetic.add(&a, &b.negated()).unwrap()
        );
    }
}

#[test]
fn limits_and_exponent_boundaries_return_errors_without_wrapping_or_allocating_huge_shifts() {
    assert_eq!(
        ScalarArithmetic::new(0).unwrap_err(),
        ScalarError::EmptyBitBudget
    );
    let arithmetic = ScalarArithmetic::new(64).unwrap();
    let low = value(&arithmetic, [1, 0, 0, 0], i64::MIN);
    let high = value(&arithmetic, [1, 0, 0, 0], i64::MAX);
    assert_eq!(
        arithmetic.add(&low, &high),
        Err(ScalarError::CoefficientSizeOverflow)
    );
    assert_eq!(
        ScalarArithmetic::new(u64::MAX).unwrap().add(&low, &high),
        Err(ScalarError::CoefficientSizeOverflow)
    );
    assert!(matches!(
        arithmetic.add(&ExactScalar::one(), &value(&arithmetic, [1, 0, 0, 0], 1000)),
        Err(ScalarError::CoefficientLimit { .. })
    ));
    assert_eq!(
        arithmetic.add(&high, &high),
        Err(ScalarError::ExponentOverflow)
    );
    assert_eq!(
        arithmetic.multiply(&high, &value(&arithmetic, [1, 0, 0, 0], 1)),
        Err(ScalarError::ExponentOverflow)
    );
    assert_eq!(
        arithmetic.multiply(&low, &ExactScalar::zero()).unwrap(),
        ExactScalar::zero()
    );
    assert_eq!(arithmetic.add(&high, &ExactScalar::zero()).unwrap(), high);
    let x = value(&arithmetic, [0, 1, 0, -1], i64::MIN);
    let y = value(&arithmetic, [0, 1, 0, -1], -1);
    assert_eq!(arithmetic.multiply(&x, &y).unwrap(), low);
    assert!(matches!(
        arithmetic.integer((BigInt::one() << 64_usize) + 1),
        Err(ScalarError::CoefficientLimit { .. })
    ));
    let larger = ScalarArithmetic::new(512)
        .unwrap()
        .integer((BigInt::one() << 300_usize) + 1)
        .unwrap();
    assert!(matches!(
        arithmetic.add(&larger, &ExactScalar::zero()),
        Err(ScalarError::CoefficientLimit { .. })
    ));
}
