use rmagna::catalogue::{
    CatalogueLimits, CliffordIndicator, PhaseCatalogue, PhaseDecompositionInput,
    builtin_phase_catalogue,
};
use rmagna::fibers::FiberOptions;
use rmagna::gf2::BitVector;
use rmagna::phase::{
    ParityPhase, PhaseError, PhaseResourceKind, PhaseResourceOptions, PreparedPhaseResource,
};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
fn bits(n: usize, word: usize) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|j| word >> j & 1 != 0)).unwrap()
}
fn ccz() -> ParityPhase {
    ParityPhase::new(
        3,
        5,
        (1_usize..8).map(|mask| {
            (
                bits(3, mask),
                if mask.count_ones() % 2 == 1 { 1 } else { 7 },
            )
        }),
    )
    .unwrap()
}
fn computational() -> PhaseResourceOptions {
    PhaseResourceOptions {
        fibers: FiberOptions {
            attempts: 0,
            ..Default::default()
        },
        ..Default::default()
    }
}
fn zero(n: usize) -> QuadraticForm {
    QuadraticForm::new(0, vec![0; n], (0..n).map(|_| bits(n, 0)).collect()).unwrap()
}
#[test]
fn catalogue_and_fiber_choices_match_direct_complex_sums_and_every_dependent_label() {
    let a = ScalarArithmetic::default();
    let reference = ccz();
    let fiber = PreparedPhaseResource::new(&reference, 8, computational(), None, &a).unwrap();
    let catalogue = {
        let owned = builtin_phase_catalogue().unwrap().clone();
        PreparedPhaseResource::new(&reference, 8, computational(), Some(&owned), &a).unwrap()
    };
    assert_eq!(fiber.kind(), PhaseResourceKind::CliffordFibers);
    assert_eq!(fiber.num_terms(), 8);
    assert_eq!(fiber.clifford_fibers().unwrap().enumerated_coordinates(), 3);
    assert!(fiber.catalogue_resource().is_none());
    assert_eq!(catalogue.kind(), PhaseResourceKind::CertifiedCatalogue);
    assert_eq!(catalogue.num_terms(), 2);
    assert_eq!(
        catalogue
            .catalogue_resource()
            .unwrap()
            .canonical_phase_table(),
        [0, 0, 0, 0, 0, 0, 0, 4]
    );
    assert!(catalogue.clifford_fibers().is_none());
    let terms = reference.terms().to_vec();
    drop(reference);
    let coefficient = a
        .scalar([3.into(), (-2).into(), 1.into(), 0.into()], -2)
        .unwrap();
    for seed in 0..64 {
        let phase = ParityPhase::new(
            3,
            (seed & 7) as u8,
            terms.iter().map(|(axis, c)| {
                (
                    axis.clone(),
                    (c + 2 * ((seed + axis.ones().sum::<usize>()) % 4) as u8) & 7,
                )
            }),
        )
        .unwrap();
        let q = QuadraticForm::new(
            (seed & 3) as u8,
            (0..3).map(|j| ((seed + j) & 3) as u8).collect(),
            (0..3)
                .map(|i| {
                    BitVector::from_indices(3, (i + 1..3).filter(|j| (seed + i + j) % 2 == 0))
                        .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let boundary = AffineKet::new(
            bits(2, 2),
            vec![bits(2, 1), bits(2, 1), bits(2, 2)],
            q.clone(),
            coefficient.clone(),
        )
        .unwrap();
        let mut expected = vec![ExactScalar::zero(); 4];
        for word in 0..8 {
            let x = bits(3, word);
            let label = 2 ^ (word & 1) ^ ((word >> 1) & 1) ^ (((word >> 2) & 1) << 1);
            let value = a
                .multiply(
                    &coefficient,
                    &a.omega(
                        i64::from(phase.evaluate(&x).unwrap() + 2 * q.evaluate(&x).unwrap()),
                        0,
                    )
                    .unwrap(),
                )
                .unwrap();
            expected[label] = a.add(&expected[label], &value).unwrap();
        }
        let sum = expected
            .iter()
            .try_fold(ExactScalar::zero(), |total, value| a.add(&total, value))
            .unwrap();
        for resource in [&fiber, &catalogue] {
            assert_eq!(resource.quotient_rank(), 3);
            assert_eq!(
                resource.evaluate(&phase, &q, &coefficient, &a).unwrap(),
                sum
            );
            let expanded = resource.expand(&phase, &boundary, &a).unwrap();
            assert!(expanded.len() <= resource.num_terms());
            for (word, value) in expected.iter().enumerate() {
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
                    &actual,
                    value,
                    "strategy={:?} seed={seed} word={word}",
                    resource.kind()
                );
            }
        }
    }
    assert_eq!(catalogue.num_terms(), 2);
    assert_eq!(fiber.num_terms(), 8);
}
#[test]
fn absent_empty_and_missing_keys_fall_back_without_inventing_catalogue_diagnostics() {
    let a = ScalarArithmetic::default();
    let mut disabled = computational();
    disabled.catalogue.max_geometry_words = 0;
    disabled.catalogue.core.max_dimension = 0;
    for catalogue in [None, Some(&PhaseCatalogue::new())] {
        let plan = PreparedPhaseResource::new(&ccz(), 8, disabled, catalogue, &a).unwrap();
        assert_eq!(plan.kind(), PhaseResourceKind::CliffordFibers);
        assert_eq!(plan.num_terms(), 8);
        assert!(plan.catalogue_resource().is_none());
    }
    for n in [3, 5] {
        let phase = ParityPhase::new(n, 0, (0..n).map(|j| (bits(n, 1 << j), 1))).unwrap();
        let plan = PreparedPhaseResource::new(
            &phase,
            1 << n,
            computational(),
            Some(builtin_phase_catalogue().unwrap()),
            &a,
        )
        .unwrap();
        assert_eq!(plan.kind(), PhaseResourceKind::CliffordFibers);
        assert_eq!(plan.quotient_rank(), n);
        assert_eq!(plan.num_terms(), 1 << n);
        let value = a.add(&ExactScalar::one(), &a.omega(1, 0).unwrap()).unwrap();
        let expected = (0..n)
            .try_fold(ExactScalar::one(), |product, _| {
                a.multiply(&product, &value)
            })
            .unwrap();
        assert_eq!(
            plan.evaluate(&phase, &zero(n), &ExactScalar::one(), &a)
                .unwrap(),
            expected
        );
    }
    let phase = ParityPhase::new(0, 7, []).unwrap();
    let plan = PreparedPhaseResource::new(
        &phase,
        1,
        computational(),
        Some(builtin_phase_catalogue().unwrap()),
        &a,
    )
    .unwrap();
    assert_eq!(plan.kind(), PhaseResourceKind::CertifiedCatalogue);
    assert_eq!(plan.quotient_rank(), 0);
    assert_eq!(plan.num_terms(), 1);
    assert_eq!(
        plan.evaluate(&phase, &zero(0), &ExactScalar::one(), &a)
            .unwrap(),
        a.omega(7, 0).unwrap()
    );
}
#[test]
fn matched_construction_failures_and_tighter_caps_are_explicit_without_fallback() {
    let a = ScalarArithmetic::default();
    let db = builtin_phase_catalogue().unwrap();
    let phase = ccz();
    let error = match PreparedPhaseResource::new(&phase, 1, computational(), Some(db), &a) {
        Err(e) => e,
        Ok(_) => panic!("accepted two catalogue terms under one-term cap"),
    };
    assert!(matches!(
        error,
        PhaseError::ResourceLimit {
            option: "catalogue_preparation.max_terms",
            limit: 1,
            required: 2
        }
    ));
    let mut options = computational();
    options.catalogue.max_terms = 1;
    assert!(matches!(
        PreparedPhaseResource::new(&phase, 8, options, Some(db), &a),
        Err(PhaseError::ResourceLimit {
            option: "catalogue_preparation.max_terms",
            ..
        })
    ));
    options = computational();
    options.catalogue.max_geometry_words = 0;
    assert!(matches!(
        PreparedPhaseResource::new(&phase, 8, options, Some(db), &a),
        Err(PhaseError::ResourceLimit {
            option: "catalogue_preparation.max_geometry_words",
            ..
        })
    ));
    let mut large_coefficient = PhaseCatalogue::new();
    large_coefficient
        .insert(
            PhaseDecompositionInput {
                dimension: 3,
                phase_table: vec![0, 0, 0, 0, 0, 0, 0, 4],
                terms: vec![
                    CliffordIndicator {
                        coefficient: a.integer(3).unwrap(),
                        constraints: vec![],
                        phase: ParityPhase::new(3, 0, []).unwrap(),
                    },
                    CliffordIndicator {
                        coefficient: a.integer(-2).unwrap(),
                        constraints: vec![],
                        phase: ParityPhase::new(3, 0, []).unwrap(),
                    },
                    CliffordIndicator {
                        coefficient: a.integer(-2).unwrap(),
                        constraints: (0..3).map(|j| (bits(3, 1 << j), true)).collect(),
                        phase: ParityPhase::new(3, 0, []).unwrap(),
                    },
                ],
            },
            &CatalogueLimits::default(),
            &a,
        )
        .unwrap();
    assert!(matches!(
        PreparedPhaseResource::new(
            &phase,
            8,
            computational(),
            Some(&large_coefficient),
            &ScalarArithmetic::new(1).unwrap()
        ),
        Err(PhaseError::Scalar(_))
    ));
    let catalogue = PreparedPhaseResource::new(&phase, 2, computational(), Some(db), &a).unwrap();
    assert!(catalogue.check_term_limit(2).is_ok());
    assert!(matches!(
        catalogue.check_term_limit(1),
        Err(PhaseError::ResourceLimit {
            option: "phase_resource.max_terms",
            limit: 1,
            required: 2
        })
    ));
    assert_eq!(catalogue.kind(), PhaseResourceKind::CertifiedCatalogue);
    let fiber = PreparedPhaseResource::new(&phase, 8, computational(), None, &a).unwrap();
    assert!(matches!(
        fiber.check_term_limit(7),
        Err(PhaseError::FiberTermLimit {
            coordinates: 3,
            quotient_rank: 3,
            limit: 7
        })
    ));
    assert!(fiber.check_term_limit(8).is_ok());
    assert!(matches!(
        PreparedPhaseResource::new(&phase, 1, computational(), None, &a),
        Err(PhaseError::FiberTermLimit { .. })
    ));
}
#[test]
fn bad_signatures_and_dimensions_are_errors_even_with_a_zero_scalar() {
    let a = ScalarArithmetic::default();
    let phase = ccz();
    let mut terms = phase.terms().to_vec();
    terms.push((bits(3, 1), 1));
    let changed = ParityPhase::new(3, 5, terms).unwrap();
    for db in [None, Some(builtin_phase_catalogue().unwrap())] {
        let plan = PreparedPhaseResource::new(&phase, 8, computational(), db, &a).unwrap();
        assert!(matches!(
            plan.evaluate(&changed, &zero(3), &ExactScalar::zero(), &a),
            Err(PhaseError::NotClifford)
        ));
        assert!(matches!(
            plan.evaluate(&phase, &zero(2), &ExactScalar::zero(), &a),
            Err(PhaseError::DimensionMismatch)
        ));
        assert_eq!(
            plan.evaluate(&phase, &zero(3), &ExactScalar::zero(), &a)
                .unwrap(),
            ExactScalar::zero()
        );
        let boundary = AffineKet::new(
            bits(1, 0),
            vec![bits(1, 1); 3],
            zero(3),
            ExactScalar::zero(),
        )
        .unwrap();
        assert!(matches!(
            plan.expand(&changed, &boundary, &a),
            Err(PhaseError::NotClifford)
        ));
        assert_eq!(plan.num_terms(), if db.is_some() { 2 } else { 8 });
    }
}
