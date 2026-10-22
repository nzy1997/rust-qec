use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::numeric::{Binary64, NumericError, NumericOptions, approximate_real, approximate_sqrt};

fn field(x: BigRational) -> ExactField {
    FieldArithmetic::default().from_rational(x).unwrap()
}
fn dyadic(n: impl Into<BigInt>, exponent: i32) -> ExactField {
    let n = n.into();
    field(if exponent >= 0 {
        BigRational::from_integer(n << exponent as usize)
    } else {
        BigRational::new(n, BigInt::one() << exponent.unsigned_abs() as usize)
    })
}
fn finite_bits(x: Binary64) -> u64 {
    match x {
        Binary64::Finite(f) => f.to_bits(),
        _ => panic!("expected finite binary64, got {x:?}"),
    }
}

#[test]
fn literal_ieee_normal_ties_and_signed_rational_witnesses_round_to_even() {
    let o = NumericOptions::default();
    for (x, bits) in [
        (dyadic(1, 0), 0x3ff0000000000000),
        (dyadic((1_u64 << 53) + 1, -53), 0x3ff0000000000000),
        (dyadic((1_u64 << 53) + 3, -53), 0x3ff0000000000002),
        (dyadic((1_u64 << 54) - 1, -53), 0x4000000000000000),
        (
            field(BigRational::new(1.into(), 3.into())),
            0x3fd5555555555555,
        ),
        (
            field(BigRational::new(1.into(), 10.into())),
            0x3fb999999999999a,
        ),
    ] {
        for negative in [false, true] {
            let x = if negative { x.negated() } else { x.clone() };
            let result = approximate_real(&x, o).unwrap();
            assert_eq!(
                finite_bits(result.binary64),
                bits | (u64::from(negative) << 63)
            );
            assert!(
                result.scientific.mantissa.abs() >= 1.0 && result.scientific.mantissa.abs() < 2.0
            );
            assert_eq!(result.scientific.mantissa.is_sign_negative(), negative);
        }
    }
    let zero = approximate_real(&ExactField::zero(), o).unwrap();
    assert_eq!(finite_bits(zero.binary64), 0);
    assert_eq!(zero.scientific.mantissa.to_bits(), 0);
    assert_eq!(zero.scientific.exponent, 0);
}

#[test]
fn literal_ieee_subnormal_and_overflow_boundaries_classify_signed_nonzero_values() {
    let o = NumericOptions::default();
    for (x, bits) in [
        (dyadic(1, -1074), 1),
        (dyadic(3, -1076), 1),
        (dyadic(3, -1075), 2),
        (dyadic(5, -1075), 2),
        (dyadic((1_u64 << 53) - 1, -1075), 0x0010000000000000),
        (dyadic((1_u64 << 53) - 3, -1075), 0x000ffffffffffffe),
        (dyadic((1_u64 << 53) - 1, 971), 0x7fefffffffffffff),
        (
            dyadic(((BigInt::from((1_u64 << 54) - 1)) << 45) - 1, 925),
            0x7fefffffffffffff,
        ),
    ] {
        let f = FieldArithmetic::default();
        let square = f.multiply(&x, &x).unwrap();
        assert_eq!(
            finite_bits(approximate_sqrt(&square, o).unwrap().binary64),
            bits
        );
        for negative in [false, true] {
            let value = if negative { x.negated() } else { x.clone() };
            assert_eq!(
                finite_bits(approximate_real(&value, o).unwrap().binary64),
                bits | u64::from(negative) << 63
            );
        }
    }
    for (x, overflow) in [
        (dyadic(1, -1075), false),
        (dyadic(1, -2000), false),
        (dyadic((1_u64 << 54) - 1, 970), true),
        (dyadic(1, 2000), true),
    ] {
        for negative in [false, true] {
            let value = if negative { x.negated() } else { x.clone() };
            let got = approximate_real(&value, o).unwrap();
            assert_eq!(
                got.binary64,
                if overflow {
                    Binary64::Overflow { negative }
                } else {
                    Binary64::Underflow { negative }
                }
            );
            assert!(got.scientific.mantissa.abs() >= 1.0);
        }
    }
}

#[test]
fn direct_subnormal_rounding_is_independent_of_scientific_double_rounding() {
    // Just above half the smallest subnormal: rounding the normalized
    // mantissa first would discard the 2^-1200 term and incorrectly give zero.
    let f = FieldArithmetic::default();
    let x = f.add(&dyadic(1, -1075), &dyadic(1, -1200)).unwrap();
    let out = approximate_real(&x, NumericOptions::default()).unwrap();
    assert_eq!(out.scientific.mantissa, 1.0);
    assert_eq!(out.scientific.exponent, -1075);
    assert_eq!(finite_bits(out.binary64), 1);
    let below = f.subtract(&dyadic(1, -1075), &dyadic(1, -1200)).unwrap();
    assert_eq!(
        approximate_real(&below, NumericOptions::default())
            .unwrap()
            .binary64,
        Binary64::Underflow { negative: false }
    );
}

#[test]
fn irrational_and_pell_cancellation_keep_nonzero_exact_magnitudes() {
    let f = FieldArithmetic::default();
    let sqrt2 = f
        .value([0, 1, 0, -1].map(|n| BigRational::from_integer(n.into())))
        .unwrap();
    assert_eq!(
        finite_bits(
            approximate_real(&sqrt2, NumericOptions::default())
                .unwrap()
                .binary64
        ),
        0x3ff6a09e667f3bcd
    );
    // (1+sqrt(2))^128 = A+B*sqrt(2), with norm one. The conjugate
    // A-B*sqrt(2) is tiny but nonzero; its reciprocal is A+B*sqrt(2).
    let (mut a, mut b) = (BigInt::one(), BigInt::zero());
    for _ in 0..128 {
        (a, b) = (&a + (&b << 1), &a + &b);
    }
    let tiny = f
        .value([
            BigRational::from_integer(a.clone()),
            BigRational::from_integer(-&b),
            BigRational::zero(),
            BigRational::from_integer(b.clone()),
        ])
        .unwrap();
    let huge = f
        .value([
            BigRational::from_integer(a),
            BigRational::from_integer(b.clone()),
            BigRational::zero(),
            BigRational::from_integer(-b),
        ])
        .unwrap();
    assert_eq!(f.multiply(&tiny, &huge).unwrap(), ExactField::one());
    let got = approximate_real(&tiny, NumericOptions::default()).unwrap();
    assert!(got.scientific.exponent < -100);
    assert!(finite_bits(got.binary64) > 0);
    let inverse = f.divide(&ExactField::one(), &huge).unwrap();
    assert_eq!(
        approximate_real(&inverse, NumericOptions::default()).unwrap(),
        got
    );
    let decimal = field(BigRational::new(1.into(), BigInt::from(10).pow(400)));
    let out = approximate_real(&decimal, NumericOptions::default()).unwrap();
    assert_eq!(out.scientific.exponent, -1329);
    assert_eq!(out.binary64, Binary64::Underflow { negative: false });
    assert!(out.scientific.mantissa > 1.0 && out.scientific.mantissa < 2.0);
}

#[test]
fn square_roots_use_exact_squared_boundaries_including_tiny_and_huge_inputs() {
    let o = NumericOptions::default();
    for (x, bits) in [
        (dyadic(0, 0), 0),
        (dyadic(1, 0), 0x3ff0000000000000),
        (dyadic(2, 0), 0x3ff6a09e667f3bcd),
        (dyadic(1, -1), 0x3fe6a09e667f3bcd),
        (dyadic(1, -2148), 1),
        (dyadic(9, -2152), 1),
    ] {
        assert_eq!(finite_bits(approximate_sqrt(&x, o).unwrap().binary64), bits);
    }
    for (power, expected) in [
        (-2150, Binary64::Underflow { negative: false }),
        (4000, Binary64::Overflow { negative: false }),
    ] {
        let out = approximate_sqrt(&dyadic(1, power), o).unwrap();
        assert_eq!(out.binary64, expected);
        assert_eq!(out.scientific.mantissa, 1.0);
        assert_eq!(out.scientific.exponent, i64::from(power / 2));
    }
    let f = FieldArithmetic::default();
    for n in [(1_u64 << 53) + 1, (1_u64 << 53) + 3] {
        let x = dyadic(n, -53);
        let square = f.multiply(&x, &x).unwrap();
        assert_eq!(
            approximate_sqrt(&square, o).unwrap(),
            approximate_real(&x, o).unwrap()
        );
    }
}

#[test]
fn nonreal_negative_root_and_explicit_intermediate_budgets_fail() {
    let f = FieldArithmetic::default();
    let imaginary = f
        .value([0, 0, 1, 0].map(|n| BigRational::from_integer(n.into())))
        .unwrap();
    assert_eq!(
        approximate_real(&imaginary, NumericOptions::default()),
        Err(NumericError::NonReal)
    );
    assert_eq!(
        approximate_sqrt(&ExactField::one().negated(), NumericOptions::default()),
        Err(NumericError::NegativeSquareRoot)
    );
    for x in [ExactField::one(), dyadic(1, -2000)] {
        assert!(matches!(
            approximate_real(&x, NumericOptions { max_work_bits: 0 }),
            Err(NumericError::ResourceLimit { .. })
        ));
        assert!(matches!(
            approximate_real(&x, NumericOptions { max_work_bits: 32 }),
            Err(NumericError::ResourceLimit { .. })
        ));
    }
}

#[test]
fn independent_decimal_witnesses_match_both_roundings_and_square_roots() {
    let vectors: serde_json::Value =
        serde_json::from_str(include_str!("data/numeric-reference.json")).unwrap();
    let f = FieldArithmetic::default();
    for (index, v) in vectors.as_array().unwrap().iter().enumerate() {
        let integer = |name: &str| v[name].as_str().unwrap().parse::<BigInt>().unwrap();
        let a = BigRational::new(integer("a"), integer("ad"));
        let b = BigRational::new(integer("b"), integer("bd"));
        let value = f.value([a, b.clone(), BigRational::zero(), -b]).unwrap();
        for (square_root, expected) in [(false, &v["real"]), (true, &v["sqrt"])] {
            if expected.is_null() {
                continue;
            }
            let out = if square_root {
                approximate_sqrt(&value, NumericOptions::default())
            } else {
                approximate_real(&value, NumericOptions::default())
            }
            .unwrap();
            assert_eq!(
                out.scientific.mantissa.to_bits(),
                u64::from_str_radix(expected["mantissa_bits"].as_str().unwrap(), 16).unwrap(),
                "witness {index}, sqrt={square_root}"
            );
            assert_eq!(
                out.scientific.exponent,
                expected["exponent"].as_i64().unwrap(),
                "witness {index}, sqrt={square_root}"
            );
            let direct = expected["binary64"].as_str().unwrap();
            match direct {
                "underflow+" => assert_eq!(out.binary64, Binary64::Underflow { negative: false }),
                "underflow-" => assert_eq!(out.binary64, Binary64::Underflow { negative: true }),
                "overflow+" => assert_eq!(out.binary64, Binary64::Overflow { negative: false }),
                "overflow-" => assert_eq!(out.binary64, Binary64::Overflow { negative: true }),
                bits => assert_eq!(
                    finite_bits(out.binary64),
                    u64::from_str_radix(bits, 16).unwrap(),
                    "witness {index}, sqrt={square_root}"
                ),
            }
        }
    }
}
