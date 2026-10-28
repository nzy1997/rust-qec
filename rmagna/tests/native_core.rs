use rmagna::BigInt;
use rmagna::gf2::BitVector;
use rmagna::quadratic::{QuadraticError, QuadraticForm};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::{AffineKet, StabilizerError};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn fixture() -> Value {
    assert_eq!(
        sha(include_bytes!("fixtures/native_core.json")),
        "88bce1aea06e05f220b3deb78a5e4df8a0c644bfc6cb4f1598b6068878e5cde6"
    );
    let value: Value = serde_json::from_str(include_str!("fixtures/native_core.json")).unwrap();
    let manifest: Value = serde_json::from_str(include_str!(
        "../../benchmarks/magna_reproduction/reference.json"
    ))
    .unwrap();
    assert_eq!(value["schema"], "rstim.magna-native-core.v1");
    assert_eq!(value["commit"], "56847478c840395735146ef1e1ccdc3d8aedc10f");
    assert_eq!(
        value["native_sha256"],
        "c602a7270cc64c02123c5403147c000c8bf431160a3286fc0b2dca52b27545da"
    );
    assert_eq!(
        value["reference_sha256"],
        sha(include_bytes!(
            "../../benchmarks/magna_reproduction/reference.json"
        ))
    );
    assert_eq!(
        value["test_source_sha256"],
        manifest["upstream"]["files"]["tests/core/test_native_core.py"]
    );
    assert_eq!(
        value["producer_sha256"],
        sha(include_bytes!(
            "../../benchmarks/magna_reproduction/native_core_reference.py"
        ))
    );
    assert_eq!(
        value["seeds"],
        serde_json::json!([26091461, 26091462, 26091463])
    );
    for (name, count) in [
        ("contractions", 162),
        ("overlaps", 92),
        ("zero_overlap_skips", 8),
        ("gauss", 28),
        ("scalars", 100),
        ("boundaries", 90),
        ("malformed", 4),
    ] {
        assert_eq!(value[name].as_array().unwrap().len(), count, "{name}");
    }
    value
}

fn usize_value(value: &Value) -> usize {
    usize::try_from(value.as_u64().unwrap()).unwrap()
}

fn vector(width: usize, value: &Value) -> BitVector {
    let integer: BigInt = value.as_str().unwrap().parse().unwrap();
    assert!(integer >= BigInt::from(0));
    let (_, mut words) = integer.to_u64_digits();
    words.resize(width.div_ceil(64).max(words.len()), 0);
    BitVector::from_words(width, words).unwrap()
}

fn scalar(value: &Value, arithmetic: &ScalarArithmetic) -> ExactScalar {
    let coefficients = value["coefficients"].as_array().unwrap();
    assert_eq!(coefficients.len(), 4);
    arithmetic
        .scalar(
            std::array::from_fn(|j| coefficients[j].as_str().unwrap().parse().unwrap()),
            value["power"].as_i64().unwrap(),
        )
        .unwrap()
}

fn ket(value: &Value, arithmetic: &ScalarArithmetic) -> AffineKet {
    let width = usize_value(&value["width"]);
    let columns = value["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| vector(width, c))
        .collect::<Vec<_>>();
    let linear = value["linear"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| u8::try_from(v.as_u64().unwrap()).unwrap())
        .collect();
    let upper = value["upper"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| vector(columns.len(), c))
        .collect();
    AffineKet::new(
        vector(width, &value["base"]),
        columns,
        QuadraticForm::new(0, linear, upper).unwrap(),
        arithmetic
            .omega(
                value["phase"].as_i64().unwrap(),
                value["power"].as_i64().unwrap(),
            )
            .unwrap(),
    )
    .unwrap()
}

fn check_record(value: &Value, arithmetic: &ScalarArithmetic) {
    let original = ket(&value["raw"], arithmetic);
    let contracted = original.contract(arithmetic).unwrap();
    assert_eq!(contracted.is_none(), value["zero"].as_bool().unwrap());
    let expected_norm = scalar(&value["norm"], arithmetic);
    assert_eq!(original.norm_squared(arithmetic).unwrap(), expected_norm);
    assert_eq!(
        contracted
            .as_ref()
            .map(|k| k.norm_squared(arithmetic).unwrap())
            .unwrap_or_else(ExactScalar::zero),
        expected_norm
    );
    let before = original.prepare().unwrap();
    let after = contracted.as_ref().map(|k| k.prepare().unwrap());
    for amplitude in value["amplitudes"].as_array().unwrap() {
        let word = vector(original.width(), &amplitude["word"]);
        let expected = scalar(&amplitude["value"], arithmetic);
        assert_eq!(
            before.amplitude(&word, arithmetic).unwrap(),
            expected,
            "raw word {word:?}"
        );
        assert_eq!(
            after
                .as_ref()
                .map(|p| p.amplitude(&word, arithmetic).unwrap())
                .unwrap_or_else(ExactScalar::zero),
            expected,
            "contracted word {word:?}"
        );
    }
}

#[test]
fn original_random_contractions_preserve_every_dense_amplitude_and_norm() {
    let data = fixture();
    let arithmetic = ScalarArithmetic::default();
    let rows = data["contractions"].as_array().unwrap();
    let mut counts = [0; 9];
    for row in rows {
        counts[row["raw"]["columns"].as_array().unwrap().len()] += 1;
        check_record(row, &arithmetic);
    }
    assert_eq!(counts, [18; 9]);
    assert!(rows.iter().any(|r| r["zero"] == true));
    assert!(rows.iter().any(|r| r["zero"] == false));
}

#[test]
fn original_overlap_gauss_cancellation_and_300_bit_scalar_contracts() {
    let data = fixture();
    let arithmetic = ScalarArithmetic::default();
    let mut attempts = BTreeSet::new();
    for row in data["overlaps"].as_array().unwrap() {
        assert!(attempts.insert(usize_value(&row["attempt"])));
        let expected = scalar(&row["expected"], &arithmetic);
        for representation in ["raw", "contracted"] {
            let left = ket(&row[representation][0], &arithmetic);
            let right = ket(&row[representation][1], &arithmetic);
            assert_eq!(left.overlap(&right, &arithmetic).unwrap(), expected);
            assert_eq!(
                right.overlap(&left, &arithmetic).unwrap(),
                expected.conjugate()
            );
            assert_eq!(
                left.contract(&arithmetic)
                    .unwrap()
                    .unwrap()
                    .overlap(&right.contract(&arithmetic).unwrap().unwrap(), &arithmetic)
                    .unwrap(),
                expected
            );
        }
    }
    for row in data["zero_overlap_skips"].as_array().unwrap() {
        assert!(attempts.insert(usize_value(&row["attempt"])));
        let side = usize_value(&row["zero_side"]);
        let inputs = row["attempted_inputs"].as_array().unwrap();
        assert_eq!(inputs.len(), side + 1);
        for (index, input) in inputs.iter().enumerate() {
            let k = ket(input, &arithmetic);
            assert_eq!(k.contract(&arithmetic).unwrap().is_none(), index == side);
            if index == side {
                assert_eq!(k.norm_squared(&arithmetic).unwrap(), ExactScalar::zero());
            }
        }
    }
    assert_eq!(attempts, (0..100).collect());
    let mut gauss_cases = BTreeSet::new();
    for row in data["gauss"].as_array().unwrap() {
        let n = usize_value(&row["paths"]);
        let l = u8::try_from(row["linear"].as_u64().unwrap()).unwrap();
        assert!(gauss_cases.insert((n, l)));
        let phase =
            QuadraticForm::new(0, vec![l; n], vec![BitVector::zeros(n).unwrap(); n]).unwrap();
        let coefficient = arithmetic.omega(3, 0).unwrap();
        let original = AffineKet::new(
            BitVector::zeros(0).unwrap(),
            vec![BitVector::zeros(0).unwrap(); n],
            phase.clone(),
            coefficient.clone(),
        )
        .unwrap();
        let expected = scalar(&row["expected"], &arithmetic);
        let unit = arithmetic
            .add(
                &ExactScalar::one(),
                &arithmetic.omega(2 * i64::from(l), 0).unwrap(),
            )
            .unwrap();
        let mut product = coefficient.clone();
        for _ in 0..n {
            product = arithmetic.multiply(&product, &unit).unwrap();
        }
        assert_eq!(product, expected);
        assert_eq!(
            arithmetic
                .multiply(&coefficient, &phase.gauss_sum(&arithmetic).unwrap())
                .unwrap(),
            expected
        );
        let contracted = original.contract(&arithmetic).unwrap();
        assert_eq!(contracted.is_none(), expected.is_zero());
        assert_eq!(
            contracted
                .map(|k| k
                    .prepare()
                    .unwrap()
                    .amplitude(&BitVector::zeros(0).unwrap(), &arithmetic)
                    .unwrap())
                .unwrap_or_else(ExactScalar::zero),
            expected
        );
    }
    assert_eq!(
        gauss_cases,
        [63, 64, 65, 127, 128, 129, 257]
            .into_iter()
            .flat_map(|n| (0..4).map(move |l| (n, l)))
            .collect()
    );
    for row in data["scalars"].as_array().unwrap() {
        let a = scalar(&row["a"], &arithmetic);
        let b = scalar(&row["b"], &arithmetic);
        assert_eq!(
            arithmetic.add(&a, &b).unwrap(),
            scalar(&row["sum"], &arithmetic)
        );
        assert_eq!(
            arithmetic.multiply(&a, &b).unwrap(),
            scalar(&row["product"], &arithmetic)
        );
    }
}

#[test]
fn original_packed_width_boundaries_preserve_dependent_column_amplitudes() {
    let data = fixture();
    let arithmetic = ScalarArithmetic::default();
    let mut counts = std::collections::BTreeMap::new();
    for row in data["boundaries"].as_array().unwrap() {
        let raw = &row["raw"];
        let width = usize_value(&raw["width"]);
        *counts.entry(width).or_insert(0) += 1;
        let columns = raw["columns"].as_array().unwrap();
        assert_eq!(columns.len(), 6);
        let mut dependent = vector(width, &columns[0]);
        dependent.xor_assign(&vector(width, &columns[1])).unwrap();
        assert_eq!(dependent, vector(width, &columns[4]));
        assert!(vector(width, &columns[5]).is_zero());
        check_record(row, &arithmetic);
    }
    assert_eq!(
        counts,
        [0, 1, 63, 64, 65, 127, 128, 129, 257]
            .into_iter()
            .map(|n| (n, 10))
            .collect()
    );
}

#[test]
fn original_malformed_roles_are_rust_constructor_errors_or_unsigned_unrepresentable() {
    let data = fixture();
    let roles = data["malformed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["role"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        roles,
        [
            "column_outside_width",
            "quadratic_diagonal",
            "negative_base",
            "dimension_mismatch"
        ]
        .into_iter()
        .collect()
    );
    assert!(
        data["malformed"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["native_exception"] == "ValueError")
    );
    assert!(BitVector::from_words(1, vec![2]).is_err());
    assert_eq!(
        QuadraticForm::new(0, vec![0], vec![BitVector::from_words(1, vec![1]).unwrap()]),
        Err(QuadraticError::InvalidUpperTriangle)
    );
    let zero_phase = QuadraticForm::new(0, vec![], vec![]).unwrap();
    assert_eq!(
        AffineKet::new(
            BitVector::zeros(1).unwrap(),
            vec![BitVector::from_words(1, vec![1]).unwrap()],
            zero_phase,
            ExactScalar::one()
        ),
        Err(StabilizerError::DimensionMismatch)
    );
    // The Python tuple's base=-1 cannot be represented by Vec<u64> words.
    // Rust has no tuple compatibility or fallback API for this internal role.
    assert!("-1".parse::<u64>().is_err());
}
