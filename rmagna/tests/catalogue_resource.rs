use rmagna::catalogue::{
    CatalogueLimits, CataloguePreparationOptions, CliffordIndicator, PhaseCatalogue,
    PhaseDecompositionInput, PreparedCataloguePhase,
};
use rmagna::gf2::BitVector;
use rmagna::phase::{ParityPhase, PhaseCoreLimits, PhaseError};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
fn bits(n: usize, x: usize) -> BitVector {
    BitVector::from_indices(
        n,
        (0..n.min(usize::BITS as usize)).filter(|j| x >> j & 1 != 0),
    )
    .unwrap()
}
fn zero(n: usize) -> ParityPhase {
    ParityPhase::new(n, 0, []).unwrap()
}
fn catalogue() -> PhaseCatalogue {
    let a = ScalarArithmetic::default();
    let mut db = PhaseCatalogue::new();
    let limits = CatalogueLimits::default();
    db.insert(
        PhaseDecompositionInput {
            dimension: 0,
            phase_table: vec![0],
            terms: vec![CliffordIndicator {
                coefficient: ExactScalar::one(),
                constraints: vec![],
                phase: zero(0),
            }],
        },
        &limits,
        &a,
    )
    .unwrap();
    db.insert(
        PhaseDecompositionInput {
            dimension: 2,
            phase_table: vec![0, 1, 1, 2],
            terms: vec![
                CliffordIndicator {
                    coefficient: ExactScalar::one(),
                    constraints: vec![(bits(2, 3), false)],
                    phase: ParityPhase::new(2, 0, [(bits(2, 1), 2)]).unwrap(),
                },
                CliffordIndicator {
                    coefficient: a.omega(1, 0).unwrap(),
                    constraints: vec![(bits(2, 3), true)],
                    phase: zero(2),
                },
            ],
        },
        &limits,
        &a,
    )
    .unwrap();
    db.insert(
        PhaseDecompositionInput {
            dimension: 3,
            phase_table: vec![0, 0, 0, 0, 0, 0, 0, 4],
            terms: vec![
                CliffordIndicator {
                    coefficient: ExactScalar::one(),
                    constraints: vec![],
                    phase: zero(3),
                },
                CliffordIndicator {
                    coefficient: a.integer(-2).unwrap(),
                    constraints: (0..3).map(|j| (bits(3, 1 << j), true)).collect(),
                    phase: zero(3),
                },
            ],
        },
        &limits,
        &a,
    )
    .unwrap();
    db
}
fn ccz(n: usize, coordinates: [usize; 3], global: u8) -> ParityPhase {
    ParityPhase::new(
        n,
        global,
        (1_usize..8).map(|mask| {
            (
                BitVector::from_indices(
                    n,
                    (0..3)
                        .filter(|j| mask >> j & 1 != 0)
                        .map(|j| coordinates[j]),
                )
                .unwrap(),
                if mask.count_ones() % 2 == 1 { 1 } else { 7 },
            )
        }),
    )
    .unwrap()
}
fn direct(phase: &ParityPhase, q: &QuadraticForm, coefficient: &ExactScalar) -> ExactScalar {
    let a = ScalarArithmetic::default();
    let mut total = ExactScalar::zero();
    for word in 0..1 << phase.num_variables() {
        let x = bits(phase.num_variables(), word);
        let exponent = phase.evaluate(&x).unwrap() + 2 * q.evaluate(&x).unwrap();
        total = a
            .add(&total, &a.omega(i64::from(exponent), 0).unwrap())
            .unwrap();
    }
    a.multiply(coefficient, &total).unwrap()
}
fn amplitudes(phase: &ParityPhase, boundary: &AffineKet) -> Vec<ExactScalar> {
    let a = ScalarArithmetic::default();
    let mut total = vec![ExactScalar::zero(); 1 << boundary.width()];
    for word in 0..1 << phase.num_variables() {
        let x = bits(phase.num_variables(), word);
        let mut label = boundary.offset().clone();
        for j in x.ones() {
            label.xor_assign(&boundary.columns()[j]).unwrap();
        }
        let index = label.ones().map(|j| 1 << j).sum::<usize>();
        let value = a
            .multiply(
                boundary.coefficient(),
                &a.omega(
                    i64::from(
                        phase.evaluate(&x).unwrap() + 2 * boundary.phase().evaluate(&x).unwrap(),
                    ),
                    0,
                )
                .unwrap(),
            )
            .unwrap();
        total[index] = a.add(&total[index], &value).unwrap();
    }
    total
}
#[test]
fn overlapping_and_paired_resources_keep_exact_scalar_and_labelled_updates_after_source_drop() {
    let a = ScalarArithmetic::default();
    let reference = ccz(3, [0, 1, 2], 3);
    let prepared = {
        let db = catalogue();
        PreparedCataloguePhase::new(&reference, &db, &CataloguePreparationOptions::default(), &a)
            .unwrap()
            .unwrap()
    };
    assert_eq!(prepared.quotient_rank(), 3);
    assert_eq!(prepared.num_terms(), 2);
    assert_eq!(prepared.geometry_words(), 28);
    assert_eq!(prepared.canonical_phase_table(), [0, 0, 0, 0, 0, 0, 0, 4]);
    let reference_terms = reference
        .terms()
        .iter()
        .map(|(axis, c)| (axis.clone(), *c))
        .collect::<Vec<_>>();
    drop(reference);
    let mut seed = 731_u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..128 {
        let updated = ParityPhase::new(
            3,
            (next() & 7) as u8,
            reference_terms
                .iter()
                .map(|(axis, c)| (axis.clone(), if next() & 1 == 0 { *c } else { (8 - c) & 7 })),
        )
        .unwrap();
        let linear = (0..3).map(|_| (next() & 3) as u8).collect();
        let upper = (0..3)
            .map(|i| BitVector::from_indices(3, (i + 1..3).filter(|_| next() & 1 != 0)).unwrap())
            .collect();
        let q = QuadraticForm::new((next() & 3) as u8, linear, upper).unwrap();
        let coefficient = a
            .scalar([3.into(), (-2).into(), 1.into(), 0.into()], -2)
            .unwrap();
        assert_eq!(
            prepared.evaluate(&updated, &q, &coefficient, &a).unwrap(),
            direct(&updated, &q, &coefficient)
        );
        let boundary = AffineKet::new(
            bits(2, 2),
            vec![bits(2, 1), bits(2, 1), bits(2, 2)],
            q,
            coefficient,
        )
        .unwrap();
        let expected = amplitudes(&updated, &boundary);
        let terms = prepared.expand(&updated, &boundary, &a).unwrap();
        assert!(terms.len() <= 2);
        for (word, expected) in expected.iter().enumerate() {
            let mut actual = ExactScalar::zero();
            for term in &terms {
                actual = a
                    .add(
                        &actual,
                        &term
                            .prepare()
                            .unwrap()
                            .amplitude(&bits(2, word), &a)
                            .unwrap(),
                    )
                    .unwrap();
            }
            assert_eq!(&actual, expected);
        }
    }
    let reference = ParityPhase::new(2, 5, [(bits(2, 1), 1), (bits(2, 2), 1)]).unwrap();
    let prepared = PreparedCataloguePhase::new(
        &reference,
        &catalogue(),
        &CataloguePreparationOptions::default(),
        &a,
    )
    .unwrap()
    .unwrap();
    assert_eq!(prepared.quotient_rank(), 2);
    assert_eq!(prepared.num_terms(), 2);
    assert_eq!(prepared.geometry_words(), 18);
    for mask in 0..4 {
        let shifted = reference
            .restrict(&bits(2, mask), &[bits(2, 1), bits(2, 2)])
            .unwrap();
        let q = QuadraticForm::new(3, vec![1, 3], vec![bits(2, 2), bits(2, 0)]).unwrap();
        assert_eq!(
            prepared
                .evaluate(&shifted, &q, &ExactScalar::one(), &a)
                .unwrap(),
            direct(&shifted, &q, &ExactScalar::one())
        );
    }
}
#[test]
fn wide_small_quotients_preserve_nonzero_origins_and_complex_radical_sums() {
    let a = ScalarArithmetic::default();
    for n in [65, 129] {
        let coordinates = [0, n / 2, n - 1];
        let reference = ccz(n, coordinates, 3);
        let prepared = PreparedCataloguePhase::new(
            &reference,
            &catalogue(),
            &CataloguePreparationOptions::default(),
            &a,
        )
        .unwrap()
        .unwrap();
        assert_eq!(prepared.quotient_rank(), 3);
        assert_eq!(prepared.num_terms(), 2);
        let q = QuadraticForm::new(
            1,
            (0..n)
                .map(|j| u8::from(!coordinates.contains(&j)))
                .collect(),
            vec![BitVector::zeros(n).unwrap(); n],
        )
        .unwrap();
        let coefficient = a.omega(7, -1).unwrap();
        let radical = a.omega((n - 3) as i64, (n - 3) as i64).unwrap();
        let expected = a
            .multiply(
                &a.multiply(&a.integer(6).unwrap(), &a.omega(5, 0).unwrap())
                    .unwrap(),
                &a.multiply(&coefficient, &radical).unwrap(),
            )
            .unwrap();
        assert_eq!(
            prepared.evaluate(&reference, &q, &coefficient, &a).unwrap(),
            expected
        );
        let origin = BitVector::from_indices(n, [1]).unwrap();
        let columns = (0..n)
            .map(|j| BitVector::from_indices(n, coordinates.contains(&j).then_some(j)).unwrap())
            .collect();
        let boundary =
            AffineKet::new(origin.clone(), columns, q.clone(), coefficient.clone()).unwrap();
        let terms = prepared.expand(&reference, &boundary, &a).unwrap();
        for mask in 0_usize..8 {
            let mut word = origin.clone();
            for (j, &coordinate) in coordinates.iter().enumerate() {
                if mask >> j & 1 != 0 {
                    word.set(coordinate, true).unwrap();
                }
            }
            let phase = 3 + u8::from(mask == 7) * 4 + 2;
            let expected = a
                .multiply(
                    &a.multiply(&coefficient, &radical).unwrap(),
                    &a.omega(i64::from(phase), 0).unwrap(),
                )
                .unwrap();
            let mut actual = ExactScalar::zero();
            for term in &terms {
                actual = a
                    .add(
                        &actual,
                        &term.prepare().unwrap().amplitude(&word, &a).unwrap(),
                    )
                    .unwrap();
            }
            assert_eq!(actual, expected);
        }
    }
}
#[test]
fn no_match_limits_signature_errors_and_zero_coefficients_have_separate_contracts() {
    let a = ScalarArithmetic::default();
    let phase = ccz(3, [0, 1, 2], 0);
    let db = catalogue();
    let options = CataloguePreparationOptions::default();
    for (field, bad) in [
        (
            "catalogue_preparation.max_terms",
            CataloguePreparationOptions {
                max_terms: 1,
                ..options
            },
        ),
        (
            "catalogue_preparation.max_geometry_words",
            CataloguePreparationOptions {
                max_geometry_words: 27,
                ..options
            },
        ),
    ] {
        assert!(
            matches!(PreparedCataloguePhase::new(&phase,&db,&bad,&a),Err(PhaseError::ResourceLimit{option,..})if option==field)
        );
    }
    assert!(matches!(
        PreparedCataloguePhase::new(
            &phase,
            &db,
            &CataloguePreparationOptions {
                core: PhaseCoreLimits {
                    max_table_values: 7,
                    ..Default::default()
                },
                ..options
            },
            &a
        ),
        Err(PhaseError::CoreLimit {
            option: "phase_core.max_table_values",
            ..
        })
    ));
    let prepared = PreparedCataloguePhase::new(
        &phase,
        &db,
        &CataloguePreparationOptions {
            max_geometry_words: 28,
            max_terms: 2,
            ..options
        },
        &a,
    )
    .unwrap()
    .unwrap();
    let q = QuadraticForm::new(0, vec![0; 3], vec![bits(3, 0); 3]).unwrap();
    let tiny = ScalarArithmetic::new(1).unwrap();
    assert_eq!(
        prepared
            .evaluate(&phase, &q, &ExactScalar::zero(), &tiny)
            .unwrap(),
        ExactScalar::zero()
    );
    assert!(matches!(
        prepared.evaluate(&phase, &q, &ExactScalar::one(), &tiny),
        Err(PhaseError::Scalar(_))
    ));
    let mut terms = phase
        .terms()
        .iter()
        .map(|(axis, c)| (axis.clone(), *c))
        .collect::<Vec<_>>();
    terms.push((bits(3, 1), 1));
    let wrong = ParityPhase::new(3, 0, terms).unwrap();
    assert!(matches!(
        prepared.evaluate(&wrong, &q, &ExactScalar::zero(), &tiny),
        Err(PhaseError::NotClifford)
    ));
    let boundary = AffineKet::new(
        bits(2, 1),
        vec![bits(2, 1), bits(2, 2), bits(2, 3)],
        q,
        ExactScalar::zero(),
    )
    .unwrap();
    assert!(
        prepared
            .expand(&phase, &boundary, &tiny)
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        prepared.expand(&wrong, &boundary, &tiny),
        Err(PhaseError::NotClifford)
    ));
    let same_dimension_miss =
        ParityPhase::new(2, 0, [(bits(2, 1), 1), (bits(2, 2), 1), (bits(2, 3), 7)]).unwrap();
    assert!(
        PreparedCataloguePhase::new(&same_dimension_miss, &db, &options, &a)
            .unwrap()
            .is_none()
    );
    let no_match = ParityPhase::new(1, 0, [(bits(1, 1), 1)]).unwrap();
    assert!(
        PreparedCataloguePhase::new(&no_match, &db, &options, &a)
            .unwrap()
            .is_none()
    );
    assert!(
        PreparedCataloguePhase::new(
            &phase,
            &PhaseCatalogue::new(),
            &CataloguePreparationOptions {
                max_terms: 0,
                max_geometry_words: 0,
                core: PhaseCoreLimits {
                    max_dimension: 0,
                    max_table_values: 0,
                    max_work: 0
                }
            },
            &a
        )
        .unwrap()
        .is_none()
    );
    let constant = ParityPhase::new(0, 5, []).unwrap();
    let zero = PreparedCataloguePhase::new(
        &constant,
        &db,
        &CataloguePreparationOptions {
            max_geometry_words: 0,
            max_terms: 1,
            ..options
        },
        &a,
    )
    .unwrap()
    .unwrap();
    assert_eq!(zero.geometry_words(), 0);
    assert_eq!(
        zero.evaluate(
            &constant,
            &QuadraticForm::new(3, vec![], vec![]).unwrap(),
            &ExactScalar::one(),
            &a
        )
        .unwrap(),
        a.omega(3, 0).unwrap()
    );
}

#[test]
fn inconsistent_and_zero_terms_keep_counts_without_geometry_and_errors_do_not_poison_reuse() {
    let a = ScalarArithmetic::default();
    let mut db = PhaseCatalogue::new();
    let entry = PhaseDecompositionInput {
        dimension: 3,
        phase_table: vec![0, 0, 0, 0, 0, 0, 0, 4],
        terms: vec![
            CliffordIndicator {
                coefficient: ExactScalar::one(),
                constraints: vec![],
                phase: zero(3),
            },
            CliffordIndicator {
                coefficient: a.integer(-2).unwrap(),
                constraints: (0..3).map(|j| (bits(3, 1 << j), true)).collect(),
                phase: zero(3),
            },
            CliffordIndicator {
                coefficient: a.omega(3, 0).unwrap(),
                constraints: vec![(bits(3, 1), true), (bits(3, 1), false)],
                phase: zero(3),
            },
            CliffordIndicator {
                coefficient: ExactScalar::zero(),
                constraints: vec![],
                phase: zero(3),
            },
        ],
    };
    db.insert(entry, &CatalogueLimits::default(), &a).unwrap();
    let phase = ccz(3, [0, 1, 2], 0);
    let resource = PreparedCataloguePhase::new(
        &phase,
        &db,
        &CataloguePreparationOptions {
            max_terms: 4,
            max_geometry_words: 28,
            ..Default::default()
        },
        &a,
    )
    .unwrap()
    .unwrap();
    assert_eq!(resource.num_terms(), 4);
    assert_eq!(resource.geometry_words(), 28);
    let q = QuadraticForm::new(0, vec![0; 3], vec![bits(3, 0); 3]).unwrap();
    assert_eq!(
        resource
            .evaluate(&phase, &q, &ExactScalar::one(), &a)
            .unwrap(),
        a.integer(6).unwrap()
    );
    let invalid_phase = zero(2);
    assert!(matches!(
        resource.evaluate(&invalid_phase, &q, &ExactScalar::zero(), &a),
        Err(PhaseError::DimensionMismatch)
    ));
    let q2 = QuadraticForm::new(0, vec![0; 2], vec![bits(2, 0); 2]).unwrap();
    assert!(matches!(
        resource.evaluate(&phase, &q2, &ExactScalar::zero(), &a),
        Err(PhaseError::DimensionMismatch)
    ));
    let invalid_boundary = AffineKet::new(
        bits(2, 0),
        vec![bits(2, 1), bits(2, 2)],
        q2,
        ExactScalar::zero(),
    )
    .unwrap();
    assert!(matches!(
        resource.expand(&phase, &invalid_boundary, &a),
        Err(PhaseError::DimensionMismatch)
    ));
    let big = a
        .integer((num_bigint::BigInt::from(1) << 512_usize) + 1)
        .unwrap();
    assert!(matches!(
        resource.evaluate(&phase, &q, &big, &ScalarArithmetic::new(16).unwrap()),
        Err(PhaseError::Scalar(_))
    ));
    assert_eq!(
        resource
            .evaluate(&phase, &q, &ExactScalar::one(), &a)
            .unwrap(),
        a.integer(6).unwrap()
    );
    let mut huge = PhaseCatalogue::new();
    huge.insert(
        PhaseDecompositionInput {
            dimension: 0,
            phase_table: vec![0],
            terms: vec![
                CliffordIndicator {
                    coefficient: big.clone(),
                    constraints: vec![],
                    phase: zero(0),
                },
                CliffordIndicator {
                    coefficient: big.negated(),
                    constraints: vec![],
                    phase: zero(0),
                },
                CliffordIndicator {
                    coefficient: ExactScalar::one(),
                    constraints: vec![],
                    phase: zero(0),
                },
            ],
        },
        &CatalogueLimits::default(),
        &a,
    )
    .unwrap();
    assert!(matches!(
        PreparedCataloguePhase::new(
            &zero(0),
            &huge,
            &CataloguePreparationOptions::default(),
            &ScalarArithmetic::new(16).unwrap()
        ),
        Err(PhaseError::Scalar(_))
    ));
}

#[test]
fn complex_overlapping_certificates_match_literal_finite_and_wide_radical_values() {
    let a = ScalarArithmetic::default();
    let scalar = |values: [i64; 4], power| a.scalar(values.map(Into::into), power).unwrap();
    let powers = [[1, 0, 0, 0], [-1, 1, 0, 0], [1, -2, 1, 0], [-1, 3, -3, 1]];
    let mut terms = (0_usize..8)
        .map(|mask| CliffordIndicator {
            coefficient: scalar(powers[mask.count_ones() as usize], 0),
            constraints: (0..3)
                .filter(|j| mask >> j & 1 != 0)
                .map(|j| (bits(3, 1 << j), true))
                .collect(),
            phase: zero(3),
        })
        .collect::<Vec<_>>();
    for sign in [1, -1] {
        terms.push(CliffordIndicator {
            coefficient: scalar([sign, sign, 2 * sign, -sign], -2),
            constraints: vec![(bits(3, 5), true)],
            phase: ParityPhase::new(3, 0, [(bits(3, 3), 4)]).unwrap(),
        });
    }
    terms.push(CliffordIndicator {
        coefficient: a
            .integer((num_bigint::BigInt::from(1) << 129_usize) + 3)
            .unwrap(),
        constraints: vec![(bits(3, 6), false), (bits(3, 6), true)],
        phase: zero(3),
    });
    let mut db = PhaseCatalogue::new();
    db.insert(
        PhaseDecompositionInput {
            dimension: 3,
            phase_table: vec![0, 1, 1, 2, 1, 2, 2, 3],
            terms,
        },
        &CatalogueLimits::default(),
        &a,
    )
    .unwrap();
    // Literal values come from direct eight-assignment cyclotomic sums.
    let cases = [
        (
            5,
            vec![(1, 1), (2, 1), (4, 1)],
            [-3, -3, 5, -7],
            [[1, -2, 3, -4], [-6, 2, -2, -4], [0; 4], [2, -3, 4, 1]],
        ),
        (
            6,
            vec![(1, 7), (2, 3), (4, 5), (3, 2), (5, 6)],
            [-3, 11, -9, -1],
            [
                [-2, 3, -4, -1],
                [-4, 6, -2, 2],
                [2, 4, -6, 2],
                [1, -2, 3, -4],
            ],
        ),
        (
            7,
            vec![(1, 1), (2, 7), (4, 7)],
            [-9, -1, 9, -7],
            [[0; 4], [-4, -1, 2, -3], [-3, 4, 1, -2], [-2, -4, 6, -2]],
        ),
    ];
    for (n, active, geometry) in [
        (3, [0, 1, 2], 98),
        (65, [1, 32, 63], 4110),
        (129, [1, 64, 127], 12152),
    ] {
        let lift = |mask: usize| {
            BitVector::from_indices(
                n,
                active
                    .into_iter()
                    .enumerate()
                    .filter_map(|(j, coordinate)| (mask >> j & 1 != 0).then_some(coordinate)),
            )
            .unwrap()
        };
        let reference = ParityPhase::new(n, 5, [(lift(1), 1), (lift(2), 1), (lift(4), 1)]).unwrap();
        let options = CataloguePreparationOptions {
            max_terms: 11,
            max_geometry_words: geometry,
            core: PhaseCoreLimits {
                max_dimension: 3,
                max_table_values: 8,
                max_work: 35,
            },
        };
        assert!(matches!(
            PreparedCataloguePhase::new(
                &reference,
                &db,
                &options,
                &ScalarArithmetic::new(129).unwrap()
            ),
            Err(PhaseError::Scalar(_))
        ));
        assert!(matches!(
            PreparedCataloguePhase::new(
                &reference,
                &db,
                &CataloguePreparationOptions { max_geometry_words: geometry - 1, ..options },
                &a
            ),
            Err(PhaseError::ResourceLimit { option: "catalogue_preparation.max_geometry_words", required, .. })
                if required > (geometry - 1) as u128
        ));
        let prepared = PreparedCataloguePhase::new(&reference, &db, &options, &a)
            .unwrap()
            .unwrap();
        assert_eq!(prepared.num_terms(), 11);
        assert_eq!(prepared.geometry_words(), geometry);
        assert_eq!(prepared.quotient_rank(), 3);
        assert_eq!(prepared.canonical_phase_table(), [0, 1, 1, 2, 1, 2, 2, 3]);
        let mut linear = vec![1; n];
        for (coordinate, coefficient) in active.into_iter().zip([1, 3, 2]) {
            linear[coordinate] = coefficient;
        }
        let mut upper = vec![BitVector::zeros(n).unwrap(); n];
        upper[active[0]].set(active[2], true).unwrap();
        upper[active[1]].set(active[2], true).unwrap();
        let q = QuadraticForm::new(1, linear, upper).unwrap();
        let alpha = scalar([1, 2, -3, 1], -3);
        let columns = (0..n)
            .map(|j| {
                let column = active
                    .iter()
                    .position(|coordinate| *coordinate == j)
                    .map_or(0, |k| [1, 3, 1][k]);
                bits(2, column)
            })
            .collect();
        let boundary = AffineKet::new(bits(2, 2), columns, q.clone(), alpha.clone()).unwrap();
        // Inactive coordinates factor as (1+i)^(n-3).
        let radical = if n == 3 {
            ExactScalar::one()
        } else {
            a.scalar(
                [0.into(), 0.into(), (-1).into(), 0.into()],
                ((n - 3) / 2) as i64,
            )
            .unwrap()
        };
        for (global, phase_terms, expected_scalar, expected_amplitudes) in &cases {
            let updated = ParityPhase::new(
                n,
                *global,
                phase_terms.iter().map(|(mask, c)| (lift(*mask), *c)),
            )
            .unwrap();
            let expected = a.multiply(&scalar(*expected_scalar, -3), &radical).unwrap();
            assert_eq!(
                prepared.evaluate(&updated, &q, &alpha, &a).unwrap(),
                expected
            );
            let expanded = prepared.expand(&updated, &boundary, &a).unwrap();
            for (word, coefficients) in expected_amplitudes.iter().enumerate() {
                let mut actual = ExactScalar::zero();
                for term in &expanded {
                    actual = a
                        .add(
                            &actual,
                            &term
                                .prepare()
                                .unwrap()
                                .amplitude(&bits(2, word), &a)
                                .unwrap(),
                        )
                        .unwrap();
                }
                assert_eq!(
                    actual,
                    a.multiply(&scalar(*coefficients, -3), &radical).unwrap()
                );
            }
        }
        assert_eq!(prepared.geometry_words(), geometry);
    }
}
