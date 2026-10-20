use num_rational::BigRational;
use rmagna::BigInt;
use rmagna::probability::{
    OccurrenceThreshold as Threshold, Probability, ProbabilityError, ProbabilityParser,
};
use std::{collections::HashSet, str::FromStr};

#[test]
fn exact_decimal_scientific_and_fraction_tokens_have_canonical_values() {
    for spellings in [
        vec!["0", "-0", "+0.00", "0e+50", "0/7"],
        vec!["1", "1.", ".1e1", "10e-1", "-3/-3"],
        vec![
            "0.125",
            ".1250",
            "125e-3",
            "1.25e-1",
            "1/8",
            "-3/-24",
            "1_000/8_000",
            "0.1_25",
            "1_25e-3",
        ],
        vec![
            "1e-30",
            "0.000000000000000000000000000001",
            "1/1000000000000000000000000000000",
        ],
    ] {
        let values = spellings
            .iter()
            .map(|s| Probability::from_str(s).unwrap())
            .collect::<Vec<_>>();
        assert!(values.windows(2).all(|pair| pair[0] == pair[1]));
        assert_eq!(values.into_iter().collect::<HashSet<_>>().len(), 1);
    }
    let tenth: Probability = "  +1 / 10  ".parse().unwrap();
    assert_eq!(tenth.to_string(), "1/10");
    assert_eq!(tenth.complement().to_string(), "9/10");
    assert_eq!(
        tenth
            .scaled(&BigRational::new(2.into(), 3.into()))
            .unwrap()
            .to_string(),
        "1/15"
    );
    assert_eq!(
        Probability::checked_sum(["1/3".parse().unwrap(), "2/3".parse().unwrap()]).unwrap(),
        Probability::one()
    );
    assert_eq!(
        Probability::checked_sum(["2/3".parse().unwrap(), "2/3".parse().unwrap()]),
        Err(ProbabilityError::OutOfRange)
    );
}

#[test]
fn small_integer_fractions_and_decimal_scales_match_integer_identities() {
    for denominator in 1..=128 {
        for numerator in 0..=denominator {
            let text = format!("{numerator}/{denominator}");
            let value: Probability = text.parse().unwrap();
            assert_eq!(
                value.value() * BigRational::from_integer(denominator.into()),
                BigRational::from_integer(numerator.into())
            );
            assert_eq!(
                value.value() + value.complement().value(),
                BigRational::from_integer(1.into())
            );
        }
    }
    for digits in 1..=128 {
        let value: Probability = format!("1e-{digits}").parse().unwrap();
        assert_eq!(value.value().numer(), &BigInt::from(1));
        assert_eq!(value.value().denom(), &BigInt::from(10).pow(digits));
    }
}

#[test]
fn threshold_rounding_keeps_endpoints_and_exact_low_probability_values_separate() {
    assert_eq!(Probability::zero().threshold_64(), Threshold::Never);
    assert_eq!(Probability::one().threshold_64(), Threshold::Always);
    assert!(!Threshold::Never.accepts(0));
    assert!(Threshold::Always.accepts(u64::MAX));
    let half: Probability = "1/2".parse().unwrap();
    assert_eq!(half.threshold_64(), Threshold::Below(1 << 63));
    assert!(half.threshold_64().accepts((1 << 63) - 1));
    assert!(!half.threshold_64().accepts(1 << 63));
    let denominator = BigInt::from(1) << 64_usize;
    let unit = Probability::from_ratio(1.into(), denominator.clone()).unwrap();
    assert_eq!(unit.threshold_64(), Threshold::Below(1));
    let tiny = Probability::from_ratio(1.into(), &denominator * 2).unwrap();
    assert_ne!(tiny, Probability::zero());
    assert_eq!(tiny.threshold_64(), Threshold::Never);
    let below_one = Probability::from_ratio(&denominator - 1, &denominator + 1).unwrap();
    assert_eq!(below_one.threshold_64(), Threshold::Below(u64::MAX - 1));
    // A fractional cutoff rounds downward, even though the exact rational
    // likelihood/query value retains its additional half-unit.
    let fractional = Probability::from_ratio(3.into(), denominator * 2).unwrap();
    assert_eq!(fractional.threshold_64(), Threshold::Below(1));
    assert_ne!(fractional, unit);
}

#[test]
fn malformed_numbers_invalid_raw_ratios_and_excessive_resources_fail() {
    for text in [
        "",
        ".",
        "+",
        "1e",
        "1e+-1",
        "1e1e1",
        "1..0",
        "NaN",
        "inf",
        "0x0",
        "1/2/3",
        "1.0/2",
        "1/2.0",
        "1 + 0",
        "0.1 trailing",
        "_10",
        "10_",
        "1__0",
        "1_.0",
        "1._0",
        "1e_0",
    ] {
        assert_eq!(
            text.parse::<Probability>(),
            Err(ProbabilityError::InvalidSyntax),
            "{text}"
        );
    }
    for text in ["-1/3", "1.0001", "1e1", "2"] {
        assert_eq!(
            text.parse::<Probability>(),
            Err(ProbabilityError::OutOfRange)
        );
    }
    assert_eq!(
        "1/0".parse::<Probability>(),
        Err(ProbabilityError::ZeroDenominator)
    );
    assert_eq!(
        Probability::from_rational(BigRational::new_raw(1.into(), 0.into())),
        Err(ProbabilityError::ZeroDenominator)
    );
    let raw = Probability::from_rational(BigRational::new_raw((-2).into(), (-4).into())).unwrap();
    assert_eq!(raw, "1/2".parse().unwrap());
    assert_eq!(
        raw.scaled(&BigRational::new_raw(1.into(), 0.into())),
        Err(ProbabilityError::ZeroDenominator)
    );
    assert_eq!(
        ProbabilityParser::new(0, 1).unwrap_err(),
        ProbabilityError::EmptyTokenBudget
    );
    let limited = ProbabilityParser::new(8, 3).unwrap();
    assert!(matches!(
        limited.parse("0.0000001"),
        Err(ProbabilityError::TokenLimit { .. })
    ));
    for text in ["1e-4", "1e4", ".0001", "1e999999"] {
        assert_eq!(
            limited.parse(text),
            Err(ProbabilityError::DecimalPowerLimit { limit: 3 })
        );
    }
    assert_eq!(limited.parse("1e-3").unwrap(), "1/1000".parse().unwrap());
    assert!(
        ProbabilityParser::default()
            .parse("1e9999999999999999999999999999999")
            .is_err()
    );
    let zero_power = ProbabilityParser::new(16, 0).unwrap();
    assert_eq!(zero_power.parse("1"), Ok(Probability::one()));
    assert!(zero_power.parse(".5").is_err());
}
