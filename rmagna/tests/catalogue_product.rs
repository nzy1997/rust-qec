use rmagna::catalogue::{
    CatalogueLimits, CataloguePreparationOptions, CliffordIndicator, PhaseCatalogue,
    PhaseDecompositionInput, PreparedCataloguePhase, builtin_phase_catalogue,
};
use rmagna::gf2::BitVector;
use rmagna::phase::ParityPhase;
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};

fn table(n: usize) -> Vec<u8> {
    (0_usize..1 << n)
        .map(|word| (word.count_ones() % 8) as u8)
        .collect()
}
fn copy_factor(n: usize) -> PhaseDecompositionInput {
    let entry = builtin_phase_catalogue()
        .unwrap()
        .get(n, &table(n))
        .unwrap();
    PhaseDecompositionInput {
        dimension: n,
        phase_table: entry.phase_table().to_vec(),
        terms: entry.terms().to_vec(),
    }
}
fn verify(catalogue: &PhaseCatalogue, n: usize) {
    let a = ScalarArithmetic::default();
    let entry = catalogue.get(n, &table(n)).unwrap();
    for word in 0_usize..1 << n {
        let x = BitVector::from_indices(n, (0..n).filter(|j| word >> j & 1 != 0)).unwrap();
        let mut actual = ExactScalar::zero();
        for term in entry.terms() {
            if term
                .constraints
                .iter()
                .all(|(row, bit)| row.dot(&x).unwrap() == *bit)
            {
                let value = a
                    .multiply(
                        &term.coefficient,
                        &a.omega(term.phase.evaluate(&x).unwrap().into(), 0).unwrap(),
                    )
                    .unwrap();
                actual = a.add(&actual, &value).unwrap();
            }
        }
        let mut expected = ExactScalar::one();
        for j in 0..n {
            if word >> j & 1 != 0 {
                expected = a.multiply(&expected, &a.omega(1, 0).unwrap()).unwrap();
            }
        }
        assert_eq!(actual, expected, "dimension {n}, assignment {word}");
    }
}
#[test]
fn paired_t_products_reconstruct_all_assignments_with_disjoint_factor_coordinates() {
    let a = ScalarArithmetic::default();
    let source = builtin_phase_catalogue().unwrap().clone();
    let before = source.usage();
    for n in 0_usize..=8 {
        let product = source
            .paired_t_product(n, &CatalogueLimits::default(), &a)
            .unwrap()
            .unwrap();
        assert_eq!(product.usage().entries, 1);
        let entry = product.get(n, &table(n)).unwrap();
        assert_eq!(entry.terms().len(), 1 << n.div_ceil(2));
        for term in entry.terms() {
            assert_eq!(term.constraints.len(), n.div_ceil(2));
            for (j, (row, _)) in term.constraints.iter().enumerate() {
                assert_eq!(
                    row.ones().collect::<Vec<_>>(),
                    (2 * j..(2 * j + 2).min(n)).collect::<Vec<_>>()
                );
            }
        }
        verify(&product, n);
        let phase = ParityPhase::new(
            n,
            0,
            (0..n).map(|j| (BitVector::from_indices(n, [j]).unwrap(), 1)),
        )
        .unwrap();
        let resource = PreparedCataloguePhase::new(
            &phase,
            &product,
            &CataloguePreparationOptions::default(),
            &a,
        )
        .unwrap()
        .unwrap();
        let mut expected = ExactScalar::one();
        let single = a.add(&ExactScalar::one(), &a.omega(1, 0).unwrap()).unwrap();
        for _ in 0..n {
            expected = a.multiply(&expected, &single).unwrap();
        }
        assert_eq!(
            resource
                .evaluate(
                    &phase,
                    &QuadraticForm::new(0, vec![0; n], vec![BitVector::zeros(n).unwrap(); n])
                        .unwrap(),
                    &ExactScalar::one(),
                    &a
                )
                .unwrap(),
            expected
        );
    }
    assert_eq!(source.usage(), before);
    let product = source
        .paired_t_product(5, &CatalogueLimits::default(), &a)
        .unwrap()
        .unwrap();
    drop(source);
    verify(&product, 5);
}
#[test]
fn missing_required_factors_are_explicit_without_alternative_grouping_or_mutation() {
    let a = ScalarArithmetic::default();
    let limits = CatalogueLimits::default();
    let empty = PhaseCatalogue::new();
    verify(&empty.paired_t_product(0, &limits, &a).unwrap().unwrap(), 0);
    assert!(empty.paired_t_product(1, &limits, &a).unwrap().is_none());
    let mut one = PhaseCatalogue::new();
    one.insert(copy_factor(1), &limits, &a).unwrap();
    assert!(one.paired_t_product(2, &limits, &a).unwrap().is_none());
    let mut pair = PhaseCatalogue::new();
    pair.insert(copy_factor(2), &limits, &a).unwrap();
    let before = pair.usage();
    assert!(pair.paired_t_product(3, &limits, &a).unwrap().is_none());
    verify(&pair.paired_t_product(4, &limits, &a).unwrap().unwrap(), 4);
    assert_eq!(pair.usage(), before);
    let mut wrong = copy_factor(1);
    wrong.phase_table = vec![0, 0];
    wrong.terms = vec![CliffordIndicator {
        coefficient: ExactScalar::one(),
        constraints: vec![],
        phase: ParityPhase::new(1, 0, []).unwrap(),
    }];
    let mut other = PhaseCatalogue::new();
    other.insert(wrong, &limits, &a).unwrap();
    assert!(other.paired_t_product(1, &limits, &a).unwrap().is_none());
}
fn extended_source() -> PhaseCatalogue {
    let a = ScalarArithmetic::default();
    let mut source = PhaseCatalogue::new();
    for n in 1..=2 {
        let mut input = copy_factor(n);
        for term in &mut input.terms {
            term.phase = ParityPhase::new(
                n,
                (term.phase.constant() + 2) & 7,
                term.phase.terms().to_vec(),
            )
            .unwrap();
            term.coefficient = a
                .multiply(&term.coefficient, &a.omega(-2, 0).unwrap())
                .unwrap();
        }
        let extra = CliffordIndicator {
            coefficient: a
                .scalar([3.into(), 0.into(), 5.into(), 0.into()], 0)
                .unwrap(),
            constraints: vec![],
            phase: ParityPhase::new(n, 6, []).unwrap(),
        };
        input.terms.push(extra.clone());
        input.terms.push(CliffordIndicator {
            coefficient: extra.coefficient.negated(),
            ..extra.clone()
        });
        input.terms.push(CliffordIndicator {
            coefficient: ExactScalar::zero(),
            ..extra.clone()
        });
        let row = BitVector::from_indices(n, [0]).unwrap();
        input.terms.push(CliffordIndicator {
            constraints: vec![(row.clone(), false), (row, true)],
            ..extra
        });
        source
            .insert(input, &CatalogueLimits::default(), &a)
            .unwrap();
    }
    source
}
#[test]
fn complex_overlapping_cancelled_zero_and_inconsistent_terms_remain_exact() {
    let source = extended_source();
    for n in [1, 2, 3, 4, 5] {
        let product = source
            .paired_t_product(n, &CatalogueLimits::default(), &ScalarArithmetic::default())
            .unwrap()
            .unwrap();
        assert_eq!(product.usage().terms, 6_usize.pow(n.div_ceil(2) as u32));
        verify(&product, n);
    }
}
#[test]
fn output_limits_have_exact_endpoints_and_failures_preserve_source_and_old_products() {
    let a = ScalarArithmetic::default();
    let source = builtin_phase_catalogue().unwrap();
    let product = source
        .paired_t_product(4, &CatalogueLimits::default(), &a)
        .unwrap()
        .unwrap();
    let u = product.usage();
    let limits = CatalogueLimits {
        max_entries: 1,
        max_dimension: 4,
        max_table_values: 16,
        max_terms: u.terms,
        max_constraint_rows: u.constraint_rows,
        max_parity_rows: u.parity_rows,
        max_verification_work: u.verification_work,
        ..Default::default()
    };
    assert!(u.parity_rows > 0);
    let before = source.usage();
    assert_eq!(
        source
            .paired_t_product(4, &limits, &a)
            .unwrap()
            .unwrap()
            .usage(),
        u
    );
    for (field, bad) in [
        (
            "max_entries",
            CatalogueLimits {
                max_entries: 0,
                ..limits
            },
        ),
        (
            "max_dimension",
            CatalogueLimits {
                max_dimension: 3,
                ..limits
            },
        ),
        (
            "max_table_values",
            CatalogueLimits {
                max_table_values: 15,
                ..limits
            },
        ),
        (
            "max_terms",
            CatalogueLimits {
                max_terms: u.terms - 1,
                ..limits
            },
        ),
        (
            "max_constraint_rows",
            CatalogueLimits {
                max_constraint_rows: u.constraint_rows - 1,
                ..limits
            },
        ),
        (
            "max_parity_rows",
            CatalogueLimits {
                max_parity_rows: u.parity_rows - 1,
                ..limits
            },
        ),
        (
            "max_verification_work",
            CatalogueLimits {
                max_verification_work: u.verification_work - 1,
                ..limits
            },
        ),
    ] {
        let error = source.paired_t_product(4, &bad, &a).unwrap_err();
        assert!(error.field.contains(field), "{field}: {error}");
        assert_eq!(source.usage(), before);
        verify(&product, 4);
    }
    assert!(
        source
            .paired_t_product(usize::MAX, &CatalogueLimits::default(), &a)
            .unwrap_err()
            .field
            .contains("max_dimension")
    );
    assert!(
        source
            .paired_t_product(
                0,
                &CatalogueLimits {
                    max_terms: 0,
                    ..Default::default()
                },
                &a
            )
            .unwrap_err()
            .field
            .contains("max_terms")
    );
    let source = extended_source();
    let before = source.usage();
    let error = source
        .paired_t_product(
            4,
            &CatalogueLimits::default(),
            &ScalarArithmetic::new(1).unwrap(),
        )
        .unwrap_err();
    assert!(error.field.contains("coefficient"), "{error}");
    assert_eq!(source.usage(), before);
}
