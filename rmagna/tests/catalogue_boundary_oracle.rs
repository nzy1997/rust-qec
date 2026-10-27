use rmagna::catalogue::{
    CatalogueBoundaryOptions, CatalogueLimits, CataloguePreparationOptions, CliffordIndicator,
    FrozenPhasePlan, PhaseCatalogue, PhaseDecompositionInput, PhaseInjectionOptions,
    PhaseSelectionOptions, PreparedCataloguePhase, builtin_phase_catalogue,
};
use rmagna::gf2::BitVector;
use rmagna::phase::ParityPhase;
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use std::sync::Arc;

fn bits(n: usize, word: usize) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|j| word >> j & 1 != 0)).unwrap()
}
fn phase(n: usize, constant: u8, terms: &[(usize, u8)]) -> ParityPhase {
    ParityPhase::new(n, constant, terms.iter().map(|&(m, c)| (bits(n, m), c))).unwrap()
}
fn quadratic(n: usize, update: usize) -> QuadraticForm {
    QuadraticForm::new(
        ((update + 1) & 3) as u8,
        (0..n).map(|j| ((j + update) & 3) as u8).collect(),
        (0..n)
            .map(|i| {
                BitVector::from_indices(
                    n,
                    (i + 1..n).filter(|j| (i + j + update).is_multiple_of(3)),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn every_original_complex_label_survives_shared_maps_and_arbitrary_clifford_updates() {
    let a = ScalarArithmetic::new(8192).unwrap();
    let builtin = builtin_phase_catalogue().unwrap();
    let cases = [
        (
            "direct",
            4,
            5,
            vec![(1, 1), (2, 1), (4, 1), (3, 7), (5, 7), (6, 7), (7, 1)],
            false,
            true,
        ),
        (
            "product",
            5,
            2,
            (0..5).map(|j| (1 << j, 1)).collect(),
            false,
            true,
        ),
        (
            "walsh",
            4,
            7,
            vec![(3, 7), (5, 3), (9, 1), (15, 2)],
            false,
            false,
        ),
        (
            "injected",
            7,
            1,
            vec![
                (2, 3),
                (3, 3),
                (11, 3),
                (15, 7),
                (18, 7),
                (19, 7),
                (21, 5),
                (33, 5),
                (50, 1),
                (52, 1),
                (56, 7),
                (57, 7),
                (64, 2),
            ],
            true,
            true,
        ),
    ];
    let mut comparisons = 0;
    let mut recomputations = 0;
    let mut reuses = 0;
    for (name, n, c, terms, injection, database) in cases {
        let reference = phase(n, c, &terms);
        let resource = if injection {
            FrozenPhasePlan::new_with_injection(
                &reference,
                Some(builtin),
                &PhaseSelectionOptions::default(),
                &PhaseInjectionOptions::default(),
                &a,
            )
        } else {
            FrozenPhasePlan::new(
                &reference,
                database.then_some(builtin),
                &PhaseSelectionOptions::default(),
                &a,
            )
        }
        .unwrap();
        if injection {
            assert!(resource.injection_receipt().unwrap().selected);
        }
        let mut plans = Vec::new();
        for layout in 0..3 {
            let width = [3, 2, 0][layout];
            let columns = (0..n)
                .map(|j| match layout {
                    0 => [3, 1, 2, 0, 3, 0, 1][j % 7],
                    1 => {
                        if j % 3 == 0 {
                            0
                        } else {
                            3
                        }
                    }
                    _ => 0,
                })
                .collect::<Vec<_>>();
            let boundary = AffineKet::new(
                bits(width, 0),
                columns.iter().map(|&w| bits(width, w)).collect(),
                quadratic(n, 0),
                ExactScalar::zero(),
            )
            .unwrap();
            let prepared = resource
                .prepare_boundary(
                    &reference,
                    &boundary,
                    &CatalogueBoundaryOptions::default(),
                    &a,
                )
                .unwrap();
            plans.push((width, columns, prepared));
        }
        drop(resource);
        for (width, columns, prepared) in plans {
            for update in 0..3 {
                let mut changed = terms.clone();
                if update > 0 {
                    changed.push(((1 << n) - 1, if update == 1 { 2 } else { 6 }));
                }
                let constant = (c + 2 * update as u8) & 7;
                let updated = phase(n, constant, &changed);
                let q = quadratic(n, update);
                let mut offsets = vec![0, (1 << width) - 1, usize::from(width > 0)];
                offsets.sort();
                offsets.dedup();
                for offset in offsets {
                    for nonzero in [false, true] {
                        let coefficient = if nonzero {
                            a.omega(3, -4).unwrap()
                        } else {
                            ExactScalar::zero()
                        };
                        let boundary = AffineKet::new(
                            bits(width, offset),
                            columns.iter().map(|&w| bits(width, w)).collect(),
                            q.clone(),
                            coefficient,
                        )
                        .unwrap();
                        let result = prepared.expand(&updated, &boundary, &a).unwrap();
                        reuses += result.statistics.gaussian_reuses;
                        recomputations += result.statistics.gaussian_recomputations;
                        if nonzero {
                            assert_eq!(
                                result.statistics.map_pullbacks,
                                prepared.usage().shared_maps
                            );
                            assert_eq!(
                                result.statistics.shifted_terms,
                                prepared.usage().consistent_terms
                            );
                            assert_eq!(
                                result.statistics.gaussian_reuses
                                    + result.statistics.gaussian_recomputations,
                                result.statistics.shifted_terms
                            );
                        } else {
                            assert_eq!(result.statistics, Default::default());
                        }
                        let states = result
                            .states
                            .iter()
                            .map(|k| k.prepare().unwrap())
                            .collect::<Vec<_>>();
                        let mut expected = vec![[0_i64; 4]; 1 << width];
                        if nonzero {
                            for word in 0..1_usize << n {
                                let label = columns
                                    .iter()
                                    .enumerate()
                                    .filter(|(j, _)| word >> j & 1 != 0)
                                    .fold(offset, |label, (_, &column)| label ^ column);
                                let parity =
                                    changed.iter().fold(constant as usize, |e, &(mask, c)| {
                                        e + c as usize * ((word & mask).count_ones() as usize & 1)
                                    });
                                let linear = (0..n)
                                    .map(|j| ((j + update) & 3) * (word >> j & 1))
                                    .sum::<usize>();
                                let cross = (0..n)
                                    .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
                                    .filter(|&(i, j)| {
                                        (i + j + update).is_multiple_of(3)
                                            && word >> i & 1 != 0
                                            && word >> j & 1 != 0
                                    })
                                    .count();
                                let exponent =
                                    (3 + parity + 2 * ((update + 1) & 3) + 2 * linear + 4 * cross)
                                        & 7;
                                expected[label][exponent & 3] += if exponent < 4 { 1 } else { -1 };
                            }
                        }
                        for (word, coefficients) in expected.into_iter().enumerate() {
                            let expected = a.scalar(coefficients.map(Into::into), -2).unwrap();
                            let actual = states.iter().fold(ExactScalar::zero(), |value, k| {
                                a.add(&value, &k.amplitude(&bits(width, word), &a).unwrap())
                                    .unwrap()
                            });
                            assert_eq!(
                                actual, expected,
                                "{name}, update {update}, offset {offset}, label {word}"
                            );
                            comparisons += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 888);
    assert!(reuses > 0 && recomputations > 0);
}

#[test]
fn overlapping_and_inconsistent_indicators_keep_their_exact_cancellation() {
    let a = ScalarArithmetic::new(8192).unwrap();
    let builtin = builtin_phase_catalogue().unwrap();
    let original = builtin.get(2, &[0, 1, 1, 2]).unwrap();
    let mut terms = original.terms().to_vec();
    let positive = terms[0].clone();
    let mut negative = positive.clone();
    negative.coefficient = negative.coefficient.negated();
    terms.extend([
        positive,
        negative,
        CliffordIndicator {
            coefficient: ExactScalar::one(),
            constraints: vec![(bits(2, 1), false), (bits(2, 1), true)],
            phase: phase(2, 0, &[]),
        },
    ]);
    let mut catalogue = PhaseCatalogue::new();
    catalogue
        .insert(
            PhaseDecompositionInput {
                dimension: 2,
                phase_table: vec![0, 1, 1, 2],
                terms,
            },
            &CatalogueLimits::default(),
            &a,
        )
        .unwrap();
    let reference = phase(2, 0, &[(1, 1), (2, 1)]);
    let resource = Arc::new(
        PreparedCataloguePhase::new(
            &reference,
            &catalogue,
            &CataloguePreparationOptions::default(),
            &a,
        )
        .unwrap()
        .unwrap(),
    );
    let boundary = AffineKet::new(
        bits(2, 1),
        vec![bits(2, 3), bits(2, 3)],
        quadratic(2, 0),
        a.omega(3, -4).unwrap(),
    )
    .unwrap();
    let prepared = resource
        .prepare_boundary(
            &reference,
            &boundary,
            &CatalogueBoundaryOptions::default(),
            &a,
        )
        .unwrap();
    drop(catalogue);
    drop(resource);
    assert_eq!(prepared.usage().terms, 5);
    assert_eq!(prepared.usage().consistent_terms, 4);
    assert!(prepared.usage().shared_maps < prepared.usage().consistent_terms);
    let output = prepared.expand(&reference, &boundary, &a).unwrap();
    for label in 0..4 {
        let mut expected = [0_i64; 4];
        for word in 0..4_usize {
            let mapped = 1 ^ if word.count_ones() & 1 != 0 { 3 } else { 0 };
            if label == mapped {
                let exponent = (3 + word.count_ones() as usize + 2 * (1 + (word >> 1 & 1))) & 7;
                expected[exponent & 3] += if exponent < 4 { 1 } else { -1 };
            }
        }
        let actual = output.states.iter().fold(ExactScalar::zero(), |sum, k| {
            a.add(
                &sum,
                &k.prepare().unwrap().amplitude(&bits(2, label), &a).unwrap(),
            )
            .unwrap()
        });
        assert_eq!(actual, a.scalar(expected.map(Into::into), -2).unwrap());
    }
}
