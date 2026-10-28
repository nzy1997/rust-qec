// Assertion-role translations from itpyi/magna at 56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_phase_quotient_optimization.py (MIT). Expected truth tables
// were calculated by its pure integer oracle before compiling this counterpart.
use rmagna::BigInt;
use rmagna::gf2::BitVector;
use rmagna::phase::{
    CanonicalPhaseCore, ParityPhase, PhaseCoreLimits, PhaseError, PreparedPhaseSum,
};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;

fn fixture() -> Value {
    let raw = include_bytes!("fixtures/phase_quotient_literals.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(raw)),
        "e92715315ab9cff366c2b341909e505e32fcd096a20ed324cf3f5f9b42e666df"
    );
    let f: Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(f["schema"], "rstim.phase-quotient-literals.v1");
    assert_eq!(
        f["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(
        f["upstream_module_sha256"],
        "77c7db4efbcc00f2026a7df617fd6cfb86d1364b137f374e5f566876f2ce74fc"
    );
    f
}
fn bits(n: usize, x: usize) -> BitVector {
    BitVector::from_indices(
        n,
        (0..n.min(usize::BITS as usize)).filter(|&j| x >> j & 1 != 0),
    )
    .unwrap()
}
fn phase(row: &Value) -> ParityPhase {
    let n = row["n"].as_u64().unwrap() as usize;
    ParityPhase::new(
        n,
        0,
        row["terms"].as_array().unwrap().iter().map(|v| {
            (
                bits(n, v[0].as_u64().unwrap() as usize),
                v[1].as_i64().unwrap().rem_euclid(8) as u8,
            )
        }),
    )
    .unwrap()
}
fn boundary(n: usize) -> QuadraticForm {
    QuadraticForm::new(0, vec![0; n], vec![bits(n, 0); n]).unwrap()
}
fn literal_sum(row: &Value, extra_power: i64) -> ExactScalar {
    ScalarArithmetic::default()
        .scalar(
            std::array::from_fn(|j| BigInt::from(row["sum_coefficients"][j].as_i64().unwrap())),
            extra_power,
        )
        .unwrap()
}
fn geometry(p: &ParityPhase) -> (Vec<BitVector>, Vec<u8>) {
    let q = p.quotient().unwrap();
    let transformed = p.restrict(&bits(p.num_variables(), 0), q.basis()).unwrap();
    let core = CanonicalPhaseCore::new(&transformed, &PhaseCoreLimits::default()).unwrap();
    (q.basis().to_vec(), core.phase_table().to_vec())
}
fn check_values(row: &Value) {
    let p = phase(row);
    let n = p.num_variables();
    let values = row["values"].as_array().unwrap();
    assert_eq!(values.len(), 1 << n);
    let q = p.quotient().unwrap();
    let resource = p.restrict(&bits(n, 0), q.complement()).unwrap();
    let lifted = ParityPhase::new(
        n,
        resource.constant(),
        resource
            .terms()
            .iter()
            .map(|(a, c)| (BitVector::from_indices(n, a.ones()).unwrap(), *c)),
    )
    .unwrap();
    let transformed = p.restrict(&bits(n, 0), q.basis()).unwrap();
    let (global, correction) = transformed.subtract(&lifted).unwrap().clifford().unwrap();
    let mut physical_images = Vec::with_capacity(values.len());
    for (x, value) in values.iter().enumerate() {
        assert_eq!(
            p.evaluate(&bits(n, x)).unwrap(),
            value.as_u64().unwrap() as u8
        );
        let mut image = bits(n, 0);
        for j in bits(n, x).ones() {
            image.xor_assign(&q.basis()[j]).unwrap();
        }
        let physical = image.ones().fold(0, |a, j| a | (1 << j));
        physical_images.push(physical);
        let core_value = resource
            .evaluate(&bits(q.rank(), x & ((1 << q.rank()) - 1)))
            .unwrap();
        assert_eq!(
            (core_value + global + 2 * correction.evaluate(&bits(n, x)).unwrap()) & 7,
            values[physical].as_u64().unwrap() as u8
        );
    }
    physical_images.sort_unstable();
    assert_eq!(
        physical_images,
        (0..values.len()).collect::<Vec<_>>(),
        "quotient reconstruction must cover every physical input"
    );
}

#[test]
fn original_139_small_signatures_reconstruct_literal_values_and_radicals() {
    let f = fixture();
    let rows = f["small"].as_array().unwrap();
    assert_eq!(rows.len(), 139);
    let mut count = [0; 4];
    for row in rows {
        check_values(row);
        let p = phase(row);
        let n = p.num_variables();
        let size = 1 << n;
        let q = p.quotient().unwrap();
        assert_eq!(row["selection"].as_u64().unwrap() as usize, count[n]);
        count[n] += 1;
        assert_eq!(q.rank(), row["rank"].as_u64().unwrap() as usize);
        let values = row["values"].as_array().unwrap();
        let expected = (0..size)
            .filter(|&u| {
                (0..size).all(|v| {
                    (0..size).all(|w| {
                        (0_usize..8)
                            .map(|k| {
                                let x = if k & 1 != 0 { u } else { 0 }
                                    ^ if k & 2 != 0 { v } else { 0 }
                                    ^ if k & 4 != 0 { w } else { 0 };
                                let value = values[x].as_i64().unwrap();
                                if k.count_ones() & 1 != 0 {
                                    value
                                } else {
                                    -value
                                }
                            })
                            .sum::<i64>()
                            .rem_euclid(8)
                            == 0
                    })
                })
            })
            .collect::<Vec<_>>();
        let mut actual = (0..1 << q.radical().len())
            .map(|word| {
                let mut x = bits(n, 0);
                for j in bits(q.radical().len(), word).ones() {
                    x.xor_assign(&q.radical()[j]).unwrap();
                }
                x.ones().fold(0, |a, j| a | (1 << j))
            })
            .collect::<Vec<_>>();
        actual.sort_unstable();
        assert_eq!(actual, expected);
    }
    assert_eq!(count, [1, 2, 8, 128]);
}

#[test]
fn original_signed_even_duplicate_updates_reuse_owned_preparation() {
    let f = fixture();
    let base = phase(&f["base"]);
    let original = base.clone();
    let expected_geometry = geometry(&base);
    let arithmetic = ScalarArithmetic::default();
    let prepared = PreparedPhaseSum::new(&base, 16, &arithmetic).unwrap();
    let original_basis = prepared.quotient().basis().to_vec();
    let original_terms = prepared.num_terms();
    assert_eq!(f["updates"].as_array().unwrap().len(), 16);
    for _ in 0..2 {
        for (word, row) in f["updates"].as_array().unwrap().iter().enumerate() {
            assert_eq!(row["word"], word);
            check_values(row);
            let changed = phase(row);
            assert_eq!(geometry(&changed), expected_geometry);
            assert_eq!(
                prepared
                    .evaluate(&changed, &boundary(4), &ExactScalar::one(), &arithmetic)
                    .unwrap(),
                literal_sum(row, 0)
            );
            assert_eq!(base, original);
            assert_eq!(prepared.quotient().basis(), original_basis);
            assert_eq!(prepared.num_terms(), original_terms);
        }
    }
    assert_eq!(
        prepared
            .evaluate(&base, &boundary(4), &ExactScalar::one(), &arithmetic)
            .unwrap(),
        literal_sum(&f["base"], 0)
    );
    check_values(&f["cancelled"]);
    let cancelled = phase(&f["cancelled"]);
    assert_eq!(cancelled.quotient().unwrap().rank(), 0);
    assert_eq!(cancelled.terms(), &[(bits(4, 10), 6)]);
    let plan = PreparedPhaseSum::new(&cancelled, 1, &arithmetic).unwrap();
    assert_eq!(
        plan.evaluate(&cancelled, &boundary(4), &ExactScalar::one(), &arithmetic)
            .unwrap(),
        literal_sum(&f["cancelled"], 0)
    );
}

fn low_rank(n: usize) -> ParityPhase {
    ParityPhase::new(n, 0, [(bits(n, 1), 7), (bits(n, 2), 3), (bits(n, 3), 1)]).unwrap()
}
fn check_low_rank(n: usize, expected: &ExactScalar) -> Arc<PreparedPhaseSum> {
    let p = low_rank(n);
    let a = ScalarArithmetic::default();
    let prepared = Arc::new(PreparedPhaseSum::new(&p, 4, &a).unwrap());
    assert_eq!(prepared.quotient().rank(), 2);
    assert_eq!(prepared.num_terms(), 4);
    assert_eq!(
        prepared
            .evaluate(&p, &boundary(n), &ExactScalar::one(), &a)
            .unwrap(),
        *expected
    );
    prepared
}
#[test]
fn original_dimension_churn_and_concurrent_queries_preserve_owned_values() {
    let f = fixture();
    let expected3 = literal_sum(&f["low_rank"], 1);
    let old = check_low_rank(3, &expected3);
    let old_basis = old.quotient().basis().to_vec();
    for n in 4..=14 {
        check_low_rank(n, &literal_sum(&f["low_rank"], (n - 2) as i64));
    }
    let replacement = check_low_rank(3, &expected3);
    assert_eq!(replacement.quotient().basis(), old_basis);
    assert_eq!(
        old.evaluate(
            &low_rank(3),
            &boundary(3),
            &ExactScalar::one(),
            &ScalarArithmetic::default()
        )
        .unwrap(),
        expected3
    );
    let jobs = [3, 4, 65, 129]
        .into_iter()
        .map(|n| {
            (
                n,
                check_low_rank(n, &literal_sum(&f["low_rank"], (n - 2) as i64)),
                literal_sum(&f["low_rank"], (n - 2) as i64),
            )
        })
        .collect::<Vec<_>>();
    std::thread::scope(|scope| {
        let handles = (0..3)
            .flat_map(|_| jobs.iter())
            .map(|(n, plan, expected)| {
                scope.spawn(move || {
                    for _ in 0..3 {
                        let p = low_rank(*n);
                        let a = ScalarArithmetic::default();
                        assert_eq!(
                            plan.evaluate(&p, &boundary(*n), &ExactScalar::one(), &a)
                                .unwrap(),
                            *expected
                        );
                        let fresh = PreparedPhaseSum::new(&p, 4, &a).unwrap();
                        assert_eq!(fresh.quotient().basis(), plan.quotient().basis());
                    }
                })
            })
            .collect::<Vec<_>>();
        for h in handles {
            h.join().unwrap();
        }
    });
}

#[test]
fn original_70_singletons_keep_identity_lifts_and_exact_clifford_remainder() {
    let identity = (0..70)
        .map(|j| BitVector::from_indices(70, [j]).unwrap())
        .collect::<Vec<_>>();
    let p = ParityPhase::new(
        70,
        0,
        identity
            .iter()
            .enumerate()
            .map(|(j, a)| (a.clone(), 1 + 2 * (j % 4) as u8)),
    )
    .unwrap();
    let q = p.quotient().unwrap();
    assert_eq!(q.rank(), 70);
    assert!(q.radical().is_empty());
    assert_eq!(q.complement(), identity);
    let odd = ParityPhase::new(70, 0, identity.iter().map(|a| (a.clone(), 1))).unwrap();
    let (global, remainder) = p.subtract(&odd).unwrap().clifford().unwrap();
    assert_eq!(global, 0);
    assert_eq!(
        remainder.linear(),
        (0..70).map(|j| (j % 4) as u8).collect::<Vec<_>>()
    );
    assert!(remainder.upper().iter().all(BitVector::is_zero));
    // Both primal lifts and dual singleton axes are identity: the dot matrix is literal I.
    for (j, lift) in q.complement().iter().enumerate() {
        assert_eq!(lift.ones().collect::<Vec<_>>(), vec![j]);
        assert_eq!(odd.evaluate(lift).unwrap(), 1);
        for (k, axis) in identity.iter().enumerate() {
            assert_eq!(axis.dot(lift).unwrap(), j == k);
        }
    }
    for word in [vec![], vec![0, 63, 64, 69], (0..70).collect()] {
        let x = BitVector::from_indices(70, word.clone()).unwrap();
        let expected = word.iter().map(|j| 1 + 2 * (j % 4)).sum::<usize>() as u8 & 7;
        assert_eq!(p.evaluate(&x).unwrap(), expected);
        assert_eq!(
            (odd.evaluate(&x).unwrap() + 2 * remainder.evaluate(&x).unwrap()) & 7,
            expected
        );
    }
    assert!(matches!(
        PreparedPhaseSum::new(&p, 16, &ScalarArithmetic::default()),
        Err(PhaseError::TermLimit {
            rank: 70,
            limit: 16
        })
    ));
}

#[test]
fn original_invalid_axes_fail_before_reusing_a_valid_owned_preparation() {
    let p = ParityPhase::new(3, 0, [(bits(3, 1), 1)]).unwrap();
    let a = ScalarArithmetic::default();
    let plan = PreparedPhaseSum::new(&p, 2, &a).unwrap();
    let before = plan
        .evaluate(&p, &boundary(3), &ExactScalar::one(), &a)
        .unwrap();
    let basis = plan.quotient().basis().to_vec();
    assert!(BitVector::from_words(3, vec![8]).is_err());
    assert!(BitVector::from_indices(3, [3]).is_err());
    for terms in [
        vec![(bits(4, 8), 1)],
        vec![(bits(3, 1), 1), (bits(4, 8), 2)],
    ] {
        assert!(matches!(
            ParityPhase::new(3, 0, terms),
            Err(PhaseError::DimensionMismatch)
        ));
        assert_eq!(
            plan.evaluate(&p, &boundary(3), &ExactScalar::one(), &a)
                .unwrap(),
            before
        );
    }
    assert!(matches!(
        plan.evaluate(&low_rank(4), &boundary(3), &ExactScalar::one(), &a),
        Err(PhaseError::DimensionMismatch)
    ));
    assert!(matches!(
        plan.evaluate(&p, &boundary(4), &ExactScalar::one(), &a),
        Err(PhaseError::DimensionMismatch)
    ));
    let incompatible = ParityPhase::new(3, 0, [(bits(3, 2), 1)]).unwrap();
    assert!(matches!(
        plan.evaluate(&incompatible, &boundary(3), &ExactScalar::one(), &a),
        Err(PhaseError::NotClifford)
    ));
    assert_eq!(plan.quotient().basis(), basis);
    assert_eq!(
        plan.evaluate(&p, &boundary(3), &ExactScalar::one(), &a)
            .unwrap(),
        before
    );
}
