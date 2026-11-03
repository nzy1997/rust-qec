// SPDX-License-Identifier: MPL-2.0
// Finite assertions adapted from itpyi/magna commit
// 56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_native_global_phase.py (SHA256
// b9d596e34dd777a83ac9aa8feb81e4a246d56bd4fae9db9be663f091927d6b50).
// Original catalogue: fixtures/original_phase_decompositions.json.
// License: fixtures/original_many_record_input.LICENSE.
use rmagna::catalogue::{CatalogueLimits, PhaseCatalogue, PhaseSelectionRoute};
use rmagna::gf2::{BitVector, Gf2Error};
use rmagna::phase::{ParityPhase, PhasePathTensor};
use rmagna::quadratic::{QuadraticError, QuadraticForm};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use serde_json::Value;
use sha2::{Digest, Sha256};

const DATABASE: &str = include_str!("fixtures/original_phase_decompositions.json");
const DATABASE_SHA256: &str = "81b0487232c624b202c1ecd53123980ec29ada377599395c0c18f92d98ccac76";

fn bits(n: usize, value: usize) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|&j| value >> j & 1 != 0)).unwrap()
}

fn zero_quadratic(n: usize) -> QuadraticForm {
    QuadraticForm::new(0, vec![0; n], vec![bits(n, 0); n]).unwrap()
}

fn product(n: usize, a: &ScalarArithmetic) -> PhasePathTensor {
    PhasePathTensor::new(
        AffineKet::new(
            bits(n, 0),
            (0..n).map(|j| bits(n, 1 << j)).collect(),
            zero_quadratic(n),
            a.omega(0, -(n as i64)).unwrap(),
        )
        .unwrap(),
        ParityPhase::new(n, 3, (0..n).map(|j| (bits(n, 1 << j), 1))).unwrap(),
    )
    .unwrap()
}

fn assert_products(database: &PhaseCatalogue, a: &ScalarArithmetic) {
    let usage = database.usage();
    let mut amplitudes = 0;
    for n in 1..=6 {
        let tensor = product(n, a);
        let original = tensor.clone();
        let resource = tensor
            .prepare_native_resource(256, Default::default(), Some(database), a)
            .unwrap()
            .unwrap();
        assert!(matches!(
            resource.native_resource().unwrap().receipt().route,
            PhaseSelectionRoute::CertifiedCatalogue | PhaseSelectionRoute::PositiveTProduct
        ));
        let states = resource
            .expand(tensor.phase(), tensor.boundary(), a)
            .unwrap();
        assert!(!states.is_empty());
        let prepared = states
            .iter()
            .map(|state| state.prepare().unwrap())
            .collect::<Vec<_>>();
        for y in 0..1_usize << n {
            let actual = prepared.iter().fold(ExactScalar::zero(), |sum, state| {
                a.add(&sum, &state.amplitude(&bits(n, y), a).unwrap())
                    .unwrap()
            });
            // Literal original phase and normalization, with no quotient or
            // resource used to derive the expected complex value.
            let expected = a.omega(3 + i64::from(y.count_ones()), -(n as i64)).unwrap();
            assert_eq!(actual, expected, "n={n}, label={y}");
            amplitudes += 1;
        }
        // Every ordered pair is required: individual term norms omit coherent
        // interference between the actual emitted stabilizer states.
        let mut norm = ExactScalar::zero();
        for left in &states {
            for right in &states {
                norm = a.add(&norm, &left.overlap(right, a).unwrap()).unwrap();
            }
        }
        assert_eq!(norm, ExactScalar::one(), "n={n}");
        assert_eq!(tensor, original);
        assert_eq!(database.usage(), usage);
    }
    assert_eq!(amplitudes, 126);
}

#[test]
fn original_database_exact_reconstruction_and_product_identification() {
    assert_eq!(DATABASE.len(), 12426);
    assert_eq!(format!("{:x}", Sha256::digest(DATABASE)), DATABASE_SHA256);
    let a = ScalarArithmetic::default();
    let limits = CatalogueLimits::default();
    let original: Value = serde_json::from_str(DATABASE).unwrap();
    assert_eq!(original["entries"].as_array().unwrap().len(), 15);
    let mut database = PhaseCatalogue::from_json(DATABASE, &limits, &a).unwrap();
    assert_eq!(database.usage().entries, 15);
    assert_eq!(database.entries().count(), 15);
    // Lookup and reconstruct every original certificate, including entries not
    // selected by the T-product inputs below.
    for entry in original["entries"].as_array().unwrap() {
        let n = entry["dimension"].as_u64().unwrap() as usize;
        let table = entry["core_phase_mod8"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_u64().unwrap() as u8)
            .collect::<Vec<_>>();
        let certificate = database.get(n, &table).unwrap();
        for (x, &phase) in table.iter().enumerate() {
            let input = bits(n, x);
            let mut actual = ExactScalar::zero();
            for term in certificate.terms() {
                if term
                    .constraints
                    .iter()
                    .all(|(row, bit)| row.dot(&input).unwrap() == *bit)
                {
                    actual = a
                        .add(
                            &actual,
                            &a.multiply(
                                &term.coefficient,
                                &a.omega(i64::from(term.phase.evaluate(&input).unwrap()), 0)
                                    .unwrap(),
                            )
                            .unwrap(),
                        )
                        .unwrap();
                }
            }
            assert_eq!(actual, a.omega(i64::from(phase), 0).unwrap());
        }
    }
    assert_products(&database, &a);

    let mut broken = original.clone();
    assert_eq!(broken["entries"][0]["terms"][0]["coefficient"][0], 1);
    broken["entries"][0]["terms"][0]["coefficient"][0] = Value::from(2);
    let mut duplicate = original.clone();
    let first = duplicate["entries"][0].clone();
    duplicate["entries"].as_array_mut().unwrap().push(first);
    for (document, message) in [(&broken, "does not reconstruct"), (&duplicate, "duplicate")] {
        let text = serde_json::to_string(document).unwrap();
        let saved_text = text.clone();
        let error = PhaseCatalogue::from_json(&text, &limits, &a).unwrap_err();
        assert!(error.message.contains(message), "{error}");

        // The duplicate document stages all fifteen valid entries before its
        // final rejection. No staged entry may escape into the receiver.
        let mut empty = PhaseCatalogue::new();
        let before = empty.usage();
        let error = empty.extend_json(&text, &limits, &a).unwrap_err();
        assert!(error.message.contains(message), "{error}");
        assert_eq!(empty.usage(), before);
        assert_eq!(empty.entries().count(), 0);
        empty.extend_json(DATABASE, &limits, &a).unwrap();
        assert_eq!(empty.usage(), database.usage());

        // An existing valid catalogue remains queryable after a refused import.
        let usage = database.usage();
        assert!(database.extend_json(&text, &limits, &a).is_err());
        assert_eq!(database.usage(), usage);
        assert_eq!(database.entries().count(), 15);
        assert_products(&database, &a);
        assert_eq!(text, saved_text);
    }
    assert_eq!(serde_json::from_str::<Value>(DATABASE).unwrap(), original);
    assert_eq!(format!("{:x}", Sha256::digest(DATABASE)), DATABASE_SHA256);
}

#[test]
fn original_malformed_dimensions_and_zero_phase_conventions() {
    // The original n=2, parity mask=4 is admitted through the actual public
    // packed-vector constructor, rather than truncated or forged internally.
    let mask = vec![4_u64];
    let saved_mask = mask.clone();
    assert_eq!(
        BitVector::from_words(2, mask.clone()),
        Err(Gf2Error::NonzeroPadding)
    );
    assert_eq!(mask, saved_mask);
    let valid_mask = BitVector::from_words(2, vec![2]).unwrap();
    let phase = ParityPhase::new(2, 0, [(valid_mask.clone(), 1)]).unwrap();
    let original_phase = phase.clone();
    for (x, expected) in [0, 0, 1, 1].into_iter().enumerate() {
        assert_eq!(phase.evaluate(&bits(2, x)).unwrap(), expected);
    }

    // The original one-coordinate upper diagonal is invalid at typed input
    // admission. No malformed internal tensor or certificate is manufactured.
    let linear = vec![0];
    let upper = vec![bits(1, 1)];
    let saved_linear = linear.clone();
    let saved_upper = upper.clone();
    let error = QuadraticForm::new(0, linear.clone(), upper.clone()).unwrap_err();
    assert_eq!(error, QuadraticError::InvalidUpperTriangle);
    assert!(error.to_string().contains("quadratic"));
    assert_eq!(linear, saved_linear);
    assert_eq!(upper, saved_upper);
    let valid = QuadraticForm::new(0, linear, vec![bits(1, 0)]).unwrap();
    let tensor = PhasePathTensor::new(
        AffineKet::new(bits(1, 0), vec![bits(1, 1)], valid, ExactScalar::one()).unwrap(),
        ParityPhase::new(1, 0, []).unwrap(),
    )
    .unwrap();
    let original = tensor.clone();
    let a = ScalarArithmetic::default();
    let absorbed = tensor
        .absorb_clifford_radical(Default::default(), &a)
        .unwrap();
    let output = absorbed.tensor();
    let resource = output
        .prepare_native_resource(1, Default::default(), None, &a)
        .unwrap()
        .unwrap();
    let states = resource
        .expand(output.phase(), output.boundary(), &a)
        .unwrap();
    for y in 0..2 {
        let actual = states.iter().fold(ExactScalar::zero(), |sum, state| {
            a.add(
                &sum,
                &state.prepare().unwrap().amplitude(&bits(1, y), &a).unwrap(),
            )
            .unwrap()
        });
        assert_eq!(actual, ExactScalar::one());
    }
    assert_eq!(tensor, original);
    assert_eq!(phase, original_phase);
    assert_eq!(phase.terms()[0].0, valid_mask);
    assert_eq!(
        BitVector::from_words(2, mask),
        Err(Gf2Error::NonzeroPadding)
    );
    assert_eq!(
        QuadraticForm::new(0, saved_linear, saved_upper),
        Err(QuadraticError::InvalidUpperTriangle)
    );
    assert_eq!(phase.evaluate(&bits(2, 2)).unwrap(), 1);
}
