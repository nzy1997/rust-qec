use rmagna::catalogue::{
    CatalogueLimits, CataloguePreparationOptions, CliffordIndicator, PhaseDecompositionInput,
    PreparedCataloguePhase, builtin_phase_catalogue,
};
use rmagna::gf2::BitVector;
use rmagna::phase::ParityPhase;
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use std::collections::BTreeSet;
fn bits(n: usize, word: usize) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|j| word >> j & 1 != 0)).unwrap()
}
// Independent complete-table finite differences, not the production axis
// expansion or constructor's affine-hyperplane search.
fn coefficients(table: &[u8]) -> Vec<u8> {
    (0_usize..table.len())
        .map(|mask| {
            let mut value = 0_i64;
            let mut sub = mask;
            loop {
                let sign = if (mask.count_ones() - sub.count_ones()) % 2 == 0 {
                    1
                } else {
                    -1
                };
                value += sign * i64::from(table[sub]);
                if sub == 0 {
                    break;
                }
                sub = (sub - 1) & mask;
            }
            value.rem_euclid(8) as u8
        })
        .collect()
}
fn canonical_table(table: &[u8]) -> Vec<u8> {
    let mut coefficients = coefficients(table);
    for (mask, coefficient) in coefficients.iter_mut().enumerate() {
        *coefficient &= match mask.count_ones() {
            0 => 0,
            1 => 1,
            2 => 3,
            _ => 7,
        };
    }
    (0..table.len())
        .map(|word| {
            coefficients
                .iter()
                .enumerate()
                .filter(|(mask, _)| word & mask == *mask)
                .map(|(_, c)| usize::from(*c))
                .sum::<usize>() as u8
                & 7
        })
        .collect()
}
fn phase_from_table(n: usize, table: &[u8]) -> ParityPhase {
    let polynomial = coefficients(table);
    let mut terms = Vec::new();
    for (mask, &coefficient) in polynomial.iter().enumerate().skip(1) {
        if coefficient == 0 {
            continue;
        }
        let degree = mask.count_ones();
        assert!(degree <= 3);
        assert_eq!(coefficient % (1 << (degree - 1)), 0);
        let scale = coefficient >> (degree - 1);
        let mut sub = mask;
        while sub != 0 {
            terms.push((
                bits(n, sub),
                if sub.count_ones() % 2 == 1 {
                    scale
                } else {
                    (8 - scale) & 7
                },
            ));
            sub = (sub - 1) & mask;
        }
    }
    let phase = ParityPhase::new(n, polynomial[0], terms).unwrap();
    for (word, &expected) in table.iter().enumerate() {
        assert_eq!(phase.evaluate(&bits(n, word)).unwrap(), expected);
    }
    phase
}
fn expected_keys() -> BTreeSet<(usize, Vec<u8>)> {
    let mut expected = BTreeSet::from([(0, vec![0]), (1, vec![0, 0]), (1, vec![0, 1])]);
    for a in 0..=1 {
        for b in 0..=1 {
            for c in [0, 2] {
                expected.insert((
                    2,
                    (0..4)
                        .map(|word| {
                            let x = word & 1;
                            let y = (word >> 1) & 1;
                            (a * x + b * y + c * x * y) as u8
                        })
                        .collect(),
                ));
            }
        }
    }
    expected.insert((3, vec![0, 0, 0, 0, 0, 0, 0, 4]));
    for transform in 0..3 {
        let table = (0..16)
            .map(|word| {
                let x = (0..4).map(|j| (word >> j) & 1).collect::<Vec<_>>();
                let (a, b, c, d) = match transform {
                    0 => (x[0], x[1], x[2], x[3]),
                    1 => (x[1], x[2], x[3], x[0] ^ x[1]),
                    _ => (x[1], x[2], x[1] ^ x[3], x[0] ^ x[1]),
                };
                ((a + d + 4 * a * b * c + 4 * b * c * d) & 7) as u8
            })
            .collect::<Vec<_>>();
        expected.insert((4, canonical_table(&table)));
    }
    expected
}
#[test]
fn all_fifteen_builtin_keys_and_exact_indicator_values_match_independent_polynomials() {
    let catalogue = builtin_phase_catalogue().unwrap();
    let keys = catalogue
        .entries()
        .map(|e| (e.dimension(), e.phase_table().to_vec()))
        .collect::<BTreeSet<_>>();
    assert_eq!(keys, expected_keys());
    assert_eq!(keys.len(), 15);
    assert_eq!(catalogue.usage().entries, 15);
    assert_eq!(catalogue.usage().table_values, 93);
    assert_eq!(catalogue.usage().terms, 27);
    assert_eq!(catalogue.usage().constraint_rows, 25);
    let arithmetic = ScalarArithmetic::default();
    let mut overlapping = 0;
    let mut partitioned = 0;
    for entry in catalogue.entries() {
        let mut has_overlap = false;
        for (word, &expected) in entry.phase_table().iter().enumerate() {
            let x = bits(entry.dimension(), word);
            let mut actual = ExactScalar::zero();
            let mut active = 0;
            for term in entry.terms() {
                term.phase.clifford().unwrap();
                if term
                    .constraints
                    .iter()
                    .all(|(row, bit)| row.dot(&x).unwrap() == *bit)
                {
                    active += 1;
                    actual = arithmetic
                        .add(
                            &actual,
                            &arithmetic
                                .multiply(
                                    &term.coefficient,
                                    &arithmetic
                                        .omega(i64::from(term.phase.evaluate(&x).unwrap()), 0)
                                        .unwrap(),
                                )
                                .unwrap(),
                        )
                        .unwrap();
                }
            }
            has_overlap |= active > 1;
            assert_eq!(
                actual,
                arithmetic.omega(i64::from(expected), 0).unwrap(),
                "n={} x={word}",
                entry.dimension()
            );
            assert!(active >= 1);
        }
        if has_overlap {
            overlapping += 1;
        } else {
            partitioned += 1;
        }
    }
    assert_eq!(overlapping, 1);
    assert_eq!(partitioned, 14);
}
#[test]
fn shared_catalogue_is_thread_safe_and_cloned_extensions_do_not_mutate_it() {
    let catalogue = builtin_phase_catalogue().unwrap();
    let pointer = catalogue as *const _ as usize;
    std::thread::scope(|scope| {
        let handles = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    let c = builtin_phase_catalogue().unwrap();
                    assert_eq!(c.usage().terms, 27);
                    c as *const _ as usize
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            assert_eq!(handle.join().unwrap(), pointer);
        }
    });
    let mut extended = catalogue.clone();
    let arithmetic = ScalarArithmetic::default();
    let table = (0..8).map(|word| (word & 1) as u8).collect::<Vec<_>>();
    extended
        .insert(
            PhaseDecompositionInput {
                dimension: 3,
                phase_table: table.clone(),
                terms: [false, true]
                    .into_iter()
                    .map(|bit| CliffordIndicator {
                        coefficient: arithmetic.omega(i64::from(bit), 0).unwrap(),
                        constraints: vec![(bits(3, 1), bit)],
                        phase: ParityPhase::new(3, 0, []).unwrap(),
                    })
                    .collect(),
            },
            &CatalogueLimits::default(),
            &arithmetic,
        )
        .unwrap();
    assert!(extended.get(3, &table).is_some());
    assert!(catalogue.get(3, &table).is_none());
    assert_eq!(extended.usage().entries, 16);
    assert_eq!(builtin_phase_catalogue().unwrap().usage().entries, 15);
}
#[test]
fn generated_certificates_reuse_exact_sums_and_dependent_output_amplitudes_under_updates() {
    let catalogue = builtin_phase_catalogue().unwrap();
    let arithmetic = ScalarArithmetic::default();
    let coefficient = arithmetic
        .scalar([3.into(), (-2).into(), 1.into(), 0.into()], -2)
        .unwrap();
    let mut cases = 0;
    for entry in catalogue.entries() {
        let n = entry.dimension();
        let reference = phase_from_table(n, entry.phase_table());
        let prepared = PreparedCataloguePhase::new(
            &reference,
            catalogue,
            &CataloguePreparationOptions::default(),
            &arithmetic,
        )
        .unwrap()
        .unwrap();
        for seed in 0..8 {
            let updated = ParityPhase::new(
                n,
                seed as u8,
                reference.terms().iter().map(|(axis, c)| {
                    (
                        axis.clone(),
                        (c + 2 * ((axis.ones().sum::<usize>() + seed) % 4) as u8) & 7,
                    )
                }),
            )
            .unwrap();
            let q = QuadraticForm::new(
                (seed & 3) as u8,
                (0..n).map(|j| ((j + seed) & 3) as u8).collect(),
                (0..n)
                    .map(|i| {
                        BitVector::from_indices(n, (i + 1..n).filter(|j| (i + j + seed) % 2 == 0))
                            .unwrap()
                    })
                    .collect(),
            )
            .unwrap();
            let boundary = AffineKet::new(
                bits(2, 1),
                (0..n).map(|j| bits(2, [1, 1, 2, 0][j])).collect(),
                q.clone(),
                coefficient.clone(),
            )
            .unwrap();
            let mut expected = vec![ExactScalar::zero(); 4];
            for word in 0..1 << n {
                let x = bits(n, word);
                let mut label = boundary.offset().clone();
                for j in x.ones() {
                    label.xor_assign(&boundary.columns()[j]).unwrap();
                }
                let index = label.ones().map(|j| 1 << j).sum::<usize>();
                let value = arithmetic
                    .multiply(
                        &coefficient,
                        &arithmetic
                            .omega(
                                i64::from(
                                    updated.evaluate(&x).unwrap() + 2 * q.evaluate(&x).unwrap(),
                                ),
                                0,
                            )
                            .unwrap(),
                    )
                    .unwrap();
                expected[index] = arithmetic.add(&expected[index], &value).unwrap();
            }
            let sum = expected
                .iter()
                .try_fold(ExactScalar::zero(), |total, v| arithmetic.add(&total, v))
                .unwrap();
            assert_eq!(
                prepared
                    .evaluate(&updated, &q, &coefficient, &arithmetic)
                    .unwrap(),
                sum
            );
            let terms = prepared.expand(&updated, &boundary, &arithmetic).unwrap();
            assert!(terms.len() <= prepared.num_terms());
            for (word, value) in expected.iter().enumerate() {
                let mut actual = ExactScalar::zero();
                for term in &terms {
                    actual = arithmetic
                        .add(
                            &actual,
                            &term
                                .prepare()
                                .unwrap()
                                .amplitude(&bits(2, word), &arithmetic)
                                .unwrap(),
                        )
                        .unwrap();
                }
                assert_eq!(&actual, value, "n={n} seed={seed} output={word}");
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 120);
}
