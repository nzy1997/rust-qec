//! Complete original Magna finite ratio-rounding loops, pinned in the portable fixture.
use num_bigint::BigInt;
use num_rational::BigRational;
use rmagna::field::FieldArithmetic;
use rmagna::numeric::{NumericOptions, approximate_real};
use serde_json::Value;

fn fixture() -> Value {
    let data: Value =
        serde_json::from_str(include_str!("data/original-ratio-rounding.json")).unwrap();
    assert_eq!(data["schema"], "rstim.magna-original-ratio-rounding.v1");
    assert_eq!(
        data["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    data
}

fn integer(row: &Value, name: &str) -> BigInt {
    row[name].as_str().unwrap().parse().unwrap()
}

fn check(n: BigInt, d: BigInt, power: i64, expected: &Value, context: &str) {
    // Apply the *entire* binary exponent to the exact input before export.
    // Applying it only to the output would bypass the extreme-input contract.
    let rational = if power >= 0 {
        BigRational::new(n << usize::try_from(power).unwrap(), d)
    } else {
        BigRational::new(n, d << usize::try_from(power.unsigned_abs()).unwrap())
    };
    let exact = FieldArithmetic::default().from_rational(rational).unwrap();
    let result = approximate_real(&exact, NumericOptions::default()).unwrap();
    assert!((1.0..2.0).contains(&result.scientific.mantissa.abs()));
    // Both normalizations are exact powers of two: Rust [1,2), native [0.5,1).
    assert_eq!(
        (result.scientific.mantissa / 2.0).to_bits(),
        u64::from_str_radix(expected["mantissa_bits"].as_str().unwrap(), 16).unwrap(),
        "{context}: mantissa"
    );
    assert_eq!(
        result.scientific.exponent + 1,
        expected["exponent"].as_i64().unwrap(),
        "{context}: exponent"
    );
}

#[test]
fn rational_ratios_against_python_fraction_rounding() {
    let data = fixture();
    let group = &data["rational_ratios"];
    assert_eq!(group["seed"], 20260927);
    assert_eq!(
        group["method"],
        "test_noise_numeric_fastpaths.NoiseNumericFastpathTests.test_rational_ratios_against_python_fraction_rounding"
    );
    let rows = group["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 350);
    for (index, row) in rows.iter().enumerate() {
        check(
            integer(row, "n"),
            integer(row, "d"),
            row["a"].as_i64().unwrap() - row["b"].as_i64().unwrap(),
            &row["expected"],
            &format!("original rational case {index}"),
        );
    }
}

#[test]
fn large_rational_rounding_and_extreme_exponents() {
    let data = fixture();
    let group = &data["large_rounding"];
    assert_eq!(group["seed"], 5641);
    assert_eq!(
        group["method"],
        "test_weight_fast_paths.WeightFastPathTests.test_large_rational_rounding_and_extreme_exponents"
    );
    let rows = group["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 237);
    assert_eq!(rows.iter().filter(|r| r["kind"] == "midpoint").count(), 27);
    assert!(rows.iter().any(|r| integer(r, "n").bits() >= 4999));
    let mut comparisons = 0;
    for (index, row) in rows.iter().enumerate() {
        let variants = row["variants"].as_array().unwrap();
        assert_eq!(variants.len(), 3);
        for (variant, (sign, power)) in variants.iter().zip([(1, 0), (-1, -99999), (1, 99999)]) {
            assert_eq!(variant["sign"], sign);
            assert_eq!(variant["power"], power);
            check(
                integer(row, "n") * sign,
                integer(row, "d"),
                power,
                &variant["expected"],
                &format!("original large case {index}, sign {sign}, power {power}"),
            );
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 711);
}
