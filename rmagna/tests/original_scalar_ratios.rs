use num_bigint::BigInt;
use num_traits::{One, Zero};
use rmagna::field::{FieldArithmetic, FieldError};
use rmagna::numeric::{
    NumericError, NumericOptions, ScalarRatioError, ScientificReal, approximate_real,
    approximate_scalar_ratio,
};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};

fn real(a: impl Into<BigInt>, b: impl Into<BigInt>, power: i64) -> ExactScalar {
    let b = b.into();
    ScalarArithmetic::default()
        .scalar([a.into(), b.clone(), BigInt::zero(), -b], power)
        .unwrap()
}
fn native(value: ScientificReal) -> (u64, i64) {
    if value.mantissa == 0.0 {
        assert_eq!(value.mantissa.to_bits(), 0);
        assert_eq!(value.exponent, 0);
        (0, 0)
    } else {
        assert!((1.0..2.0).contains(&value.mantissa.abs()));
        (
            (value.mantissa * 0.5).to_bits(),
            value.exponent.checked_add(1).unwrap(),
        )
    }
}
fn ratio(n: &ExactScalar, d: &ExactScalar) -> ScientificReal {
    approximate_scalar_ratio(n, d, NumericOptions::default()).unwrap()
}

// Whole contracts from pinned Magna NativeFileRuntimeTests in
// tests/core/test_file_native.py. Full original methods run separately through
// its identified existing helper; attribution and independent Pell bits are in
// data/original-scalar-ratio-reference.json.
#[test]
fn original_scaled_scalar_ratios_preserve_underflow_and_sign() {
    let one = ExactScalar::one();
    for (n, d, m, e) in [
        (real(9, 0, -1160), one.clone(), 0.5625_f64, -1156),
        (real(-9, 0, -1160), one.clone(), -0.5625, -1156),
        (
            real(1, 0, -1_000_000),
            real(1, 0, 1_000_000),
            0.5,
            -1_999_999,
        ),
        (ExactScalar::zero(), one.clone(), 0.0, 0),
        (one, real(1, 0, -1_000_000), 0.5, 1_000_001),
    ] {
        let options = NumericOptions { max_work_bits: 512 };
        let got = approximate_scalar_ratio(&n, &d, options).unwrap();
        assert_eq!(native(got), (m.to_bits(), e));
    }
    let expanded = real(1, 0, -1_000_000);
    assert!(matches!(
        FieldArithmetic::new(512).from_scalar(&expanded),
        Err(FieldError::ResourceLimit { limit: 512, .. })
    ));
    assert_eq!(
        approximate_scalar_ratio(
            &expanded,
            &ExactScalar::one(),
            NumericOptions { max_work_bits: 512 }
        )
        .unwrap(),
        ScientificReal {
            mantissa: 1.0,
            exponent: -1_000_000
        }
    );
}

#[test]
fn original_scaled_scalar_ratios_exact_cancellation() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("data/original-scalar-ratio-reference.json")).unwrap();
    assert_eq!(
        reference["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(reference["pell_steps"], 120);
    let (mut a, mut b) = (BigInt::one(), BigInt::zero());
    for _ in 0..120 {
        (a, b) = (&a * 3 + &b * 4, &a * 2 + &b * 3);
    }
    assert_eq!(&a * &a - ((&b * &b) << 1_usize), BigInt::one());
    assert_eq!(a.to_string(), reference["a"].as_str().unwrap());
    assert_eq!(b.to_string(), reference["b"].as_str().unwrap());
    let small = real(a.clone(), -&b, -500_000);
    let large = real(a, b, 500_000);
    for ((n, d), expected) in [(&small, &large), (&large, &small)]
        .into_iter()
        .zip(reference["ratios"].as_array().unwrap())
    {
        let bits =
            u64::from_str_radix(expected["Rust_mantissa_bits"].as_str().unwrap(), 16).unwrap();
        let exponent = expected["Rust_exponent"].as_i64().unwrap();
        let got = ratio(n, d);
        assert_eq!(got.mantissa.to_bits(), bits);
        assert_eq!(got.exponent, exponent);
        assert_eq!(
            native(got),
            (
                u64::from_str_radix(expected["native_mantissa_bits"].as_str().unwrap(), 16)
                    .unwrap(),
                expected["native_exponent"].as_i64().unwrap(),
            )
        );
        let negative = ratio(&n.negated(), d);
        assert_eq!(negative.mantissa.to_bits(), bits | (1_u64 << 63));
        assert_eq!(negative.exponent, exponent);
    }
}

#[test]
fn original_scaled_scalar_ratios_round_half_even() {
    for (n, bits, exponent) in [
        ((1_u64 << 53) + 1, 0x3fe0000000000000, 0),
        ((1_u64 << 53) + 3, 0x3fe0000000000002, 0),
        ((1_u64 << 54) - 1, 0x3fe0000000000000, 1),
    ] {
        let input = real(n, 0, -54);
        let got = ratio(&input, &ExactScalar::one());
        assert_eq!(native(got), (bits, exponent));
        let negative = ratio(&input.negated(), &ExactScalar::one());
        assert_eq!(native(negative), (bits | (1_u64 << 63), exponent));
    }
}

#[test]
fn original_scaled_scalar_ratios_reject_nonreal_and_nonpositive_denominator() {
    let one = ExactScalar::one();
    for d in [ExactScalar::zero(), real(-1, 0, 0)] {
        assert_eq!(
            approximate_scalar_ratio(&one, &d, NumericOptions::default()),
            Err(ScalarRatioError::NonPositiveDenominator)
        );
        // A zero numerator never hides an invalid denominator.
        assert_eq!(
            approximate_scalar_ratio(&ExactScalar::zero(), &d, NumericOptions::default()),
            Err(ScalarRatioError::NonPositiveDenominator)
        );
    }
    let imaginary = ScalarArithmetic::default()
        .scalar([0.into(), 0.into(), 1.into(), 0.into()], 0)
        .unwrap();
    for (n, d) in [
        (&imaginary, &one),
        (&one, &imaginary),
        (&ExactScalar::zero(), &imaginary),
    ] {
        assert_eq!(
            approximate_scalar_ratio(n, d, NumericOptions::default()),
            Err(ScalarRatioError::Numeric(NumericError::NonReal))
        );
    }
    assert_eq!(
        ratio(&one, &one),
        ScientificReal {
            mantissa: 1.0,
            exponent: 0
        }
    );
}

#[test]
fn coefficient_and_intermediate_work_refusals_preserve_inputs_and_allow_retry() {
    let one = ExactScalar::one();
    let wide = real((BigInt::one() << 255_usize) + 1, 1, -1_000_000);
    let before = wide.clone();
    for (n, d) in [(&wide, &one), (&one, &wide), (&ExactScalar::zero(), &wide)] {
        assert!(matches!(
            approximate_scalar_ratio(n, d, NumericOptions { max_work_bits: 32 }),
            Err(ScalarRatioError::Numeric(NumericError::ResourceLimit { limit: 32, required }))
                if required >= 256
        ));
    }
    assert_eq!(wide, before);
    assert!(ratio(&wide, &one).mantissa.is_finite());
    let n = real((1_u64 << 30) + 1, 1, 0);
    let d = real((1_u64 << 30) + 3, -1, 0);
    let before = (n.clone(), d.clone());
    assert!(matches!(
        approximate_scalar_ratio(&n, &d, NumericOptions { max_work_bits: 32 }),
        Err(ScalarRatioError::Numeric(NumericError::ResourceLimit { limit: 32, required }))
            if required > 32
    ));
    assert_eq!((n.clone(), d.clone()), before);
    let f = FieldArithmetic::default();
    let exact = f
        .divide(&f.from_scalar(&n).unwrap(), &f.from_scalar(&d).unwrap())
        .unwrap();
    assert_eq!(
        ratio(&n, &d),
        approximate_real(&exact, NumericOptions::default())
            .unwrap()
            .scientific
    );
    assert!(matches!(
        approximate_scalar_ratio(&one, &one, NumericOptions { max_work_bits: 0 }),
        Err(ScalarRatioError::Numeric(NumericError::ResourceLimit {
            limit: 0,
            ..
        }))
    ));
    assert!(ratio(&one, &one).mantissa == 1.0);
}

#[test]
fn final_wide_exponent_admission_handles_cancellation_zero_and_rounding_carry() {
    // The power difference alone is outside i64. The rounded coefficient
    // exponent rescues both ends before the single final i64 conversion.
    assert_eq!(
        ratio(&real(1, 0, i64::MAX), &real(3, 0, -1)).exponent,
        i64::MAX - 1
    );
    assert_eq!(
        ratio(&real(3, 0, i64::MIN), &real(1, 0, 1)),
        ScientificReal {
            mantissa: 1.5,
            exponent: i64::MIN
        }
    );
    for (n, d) in [
        (real(1, 0, i64::MAX), real(1, 0, -1)),
        (real(1, 0, i64::MIN), real(1, 0, 1)),
        (
            real((1_u64 << 54) - 1, 0, i64::MAX - 53),
            ExactScalar::one(),
        ),
        (real(1, 0, i64::MAX), real(1, 0, i64::MIN)),
    ] {
        assert_eq!(
            approximate_scalar_ratio(&n, &d, NumericOptions::default()),
            Err(ScalarRatioError::ExponentOverflow)
        );
    }
    assert_eq!(
        ratio(
            &real((1_u64 << 54) - 1, 0, i64::MAX - 54),
            &ExactScalar::one()
        ),
        ScientificReal {
            mantissa: 1.0,
            exponent: i64::MAX
        }
    );
    assert_eq!(
        ratio(&ExactScalar::zero(), &real(1, 0, i64::MIN)),
        ScientificReal {
            mantissa: 0.0,
            exponent: 0
        }
    );
    assert_eq!(
        ratio(&real(1, 0, i64::MAX), &real(1, 0, i64::MAX)),
        ScientificReal {
            mantissa: 1.0,
            exponent: 0
        }
    );
}

#[test]
fn conjugate_norm_signs_and_cancellation_match_exact_small_field_quotients() {
    let f = FieldArithmetic::default();
    for (a, b, c, d) in [
        (1, 0, 0, 1),
        (1, 0, -1, 1),
        (1, 0, 3, -2),
        (-1, 1, 3, 2),
        (3, -2, 3, 2),
        (3, 2, 3, -2),
        (-3, 2, 5, -3),
        (0, 1, 3, 1),
        (1, -1, 1, 1),
    ] {
        let n = real(a, b, 0);
        let den = real(c, d, 0);
        let exact = f
            .divide(&f.from_scalar(&n).unwrap(), &f.from_scalar(&den).unwrap())
            .unwrap();
        assert_eq!(
            ratio(&n, &den),
            approximate_real(&exact, NumericOptions::default())
                .unwrap()
                .scientific
        );
    }
    assert_eq!(
        approximate_scalar_ratio(
            &ExactScalar::one(),
            &real(1, -1, 0),
            NumericOptions::default()
        ),
        Err(ScalarRatioError::NonPositiveDenominator)
    );
}
