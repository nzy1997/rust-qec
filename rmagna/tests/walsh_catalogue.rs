use rmagna::catalogue::{
    CatalogueLimits, CataloguePreparationOptions, PhaseCatalogue, PreparedCataloguePhase,
};
use rmagna::gf2::BitVector;
use rmagna::phase::{CanonicalPhaseCore, ParityPhase, PhaseCoreLimits};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use sha2::{Digest, Sha256};

fn bits(width: usize, word: usize) -> BitVector {
    BitVector::from_indices(width, (0..width).filter(|j| word >> j & 1 != 0)).unwrap()
}
// Literal character sums in Z[omega]/(omega^4+1), without a butterfly or
// production decomposition. Small integer numerators are exact before scaling.
fn literal_coefficients(
    dimension: usize,
    table: &[u8],
    a: &ScalarArithmetic,
) -> Vec<(usize, ExactScalar)> {
    (0..table.len())
        .filter_map(|mask| {
            let mut numerator = [0_i64; 4];
            for (word, &phase) in table.iter().enumerate() {
                let sign = if (mask & word).count_ones() % 2 == 0 {
                    1
                } else {
                    -1
                };
                numerator[usize::from(phase % 4)] += if phase < 4 { sign } else { -sign };
            }
            let coefficient = a
                .scalar(numerator.map(Into::into), -(dimension as i64))
                .unwrap();
            (!coefficient.is_zero()).then_some((mask, coefficient))
        })
        .collect()
}
fn verify_table(dimension: usize, table: &[u8]) {
    let a = ScalarArithmetic::default();
    let expected = literal_coefficients(dimension, table, &a);
    let catalogue = PhaseCatalogue::from_walsh_table(
        dimension,
        table,
        1 << 24,
        &CatalogueLimits::default(),
        &a,
    )
    .unwrap();
    let entry = catalogue.get(dimension, table).unwrap();
    assert_eq!(entry.dimension(), dimension);
    assert_eq!(entry.phase_table(), table);
    assert_eq!(entry.terms().len(), expected.len());
    for (term, (mask, coefficient)) in entry.terms().iter().zip(&expected) {
        assert_eq!(
            &term.coefficient, coefficient,
            "table {table:?}, mask {mask}"
        );
        assert!(term.constraints.is_empty());
        assert_eq!(term.phase.constant(), 0);
        assert_eq!(term.phase.num_variables(), dimension);
        if *mask == 0 {
            assert!(term.phase.terms().is_empty());
        } else {
            assert_eq!(term.phase.terms(), &[(bits(dimension, *mask), 4)]);
        }
    }
    let parities = expected.iter().filter(|(mask, _)| *mask != 0).count();
    let usage = catalogue.usage();
    assert_eq!(usage.entries, 1);
    assert_eq!(usage.table_values, table.len());
    assert_eq!(usage.terms, expected.len());
    assert_eq!(usage.constraint_rows, 0);
    assert_eq!(usage.parity_rows, parities);
    assert_eq!(
        usage.verification_work,
        table.len() as u128 * (1 + expected.len() + parities) as u128
    );
}

#[test]
fn all_small_tables_and_complex_coupled_tables_match_literal_exact_character_sums() {
    let mut cases = 0;
    for dimension in 0..=2 {
        let count = 1_usize << dimension;
        for mut encoded in 0..8_usize.pow(count as u32) {
            let table = (0..count)
                .map(|_| {
                    let phase = (encoded & 7) as u8;
                    encoded >>= 3;
                    phase
                })
                .collect::<Vec<_>>();
            verify_table(dimension, &table);
            cases += 1;
        }
    }
    for dimension in 3..=6 {
        let count = 1_usize << dimension;
        let tables = [
            (0..count)
                .map(|word| (word.count_ones() % 8) as u8)
                .collect::<Vec<_>>(),
            (0..count)
                .map(|word| ((8 - word.count_ones() % 8) % 8) as u8)
                .collect(),
            (0..count)
                .map(|word| (4 * ((word & 5).count_ones() % 2)) as u8)
                .collect(),
            (0..count)
                .map(|word| {
                    ((3 + word.count_ones()
                        + 2 * ((word & 3).count_ones() % 2)
                        + 4 * u32::from(word & 7 == 7))
                        % 8) as u8
                })
                .collect(),
        ];
        for table in tables {
            verify_table(dimension, &table);
            cases += 1;
        }
        for index in 0..12 {
            let table = (0..count)
                .map(|word| {
                    Sha256::digest(format!("walsh945:{dimension}:{index}:{word}").as_bytes())[0] % 8
                })
                .collect::<Vec<_>>();
            verify_table(dimension, &table);
            cases += 1;
        }
    }
    assert_eq!(cases, 4232);
}

#[test]
fn zero_coefficients_use_no_retained_terms_or_rows_and_zero_dimension_is_owned() {
    let a = ScalarArithmetic::default();
    let limits = CatalogueLimits {
        max_terms: 1,
        max_constraint_rows: 0,
        max_parity_rows: 0,
        ..Default::default()
    };
    let mut table = vec![3; 8];
    let constant = PhaseCatalogue::from_walsh_table(3, &table, 24, &limits, &a).unwrap();
    table.fill(0);
    let entry = constant.get(3, &[3; 8]).unwrap();
    assert_eq!(entry.terms().len(), 1);
    assert_eq!(entry.terms()[0].coefficient, a.omega(3, 0).unwrap());
    assert!(entry.terms()[0].phase.terms().is_empty());
    assert_eq!(constant.usage().parity_rows, 0);
    let character = PhaseCatalogue::from_walsh_table(
        2,
        &[0, 4, 4, 0],
        8,
        &CatalogueLimits {
            max_terms: 1,
            max_parity_rows: 1,
            max_constraint_rows: 0,
            ..Default::default()
        },
        &a,
    )
    .unwrap();
    let entry = character.get(2, &[0, 4, 4, 0]).unwrap();
    assert_eq!(entry.terms().len(), 1);
    assert_eq!(entry.terms()[0].coefficient, ExactScalar::one());
    assert_eq!(entry.terms()[0].phase.terms(), &[(bits(2, 3), 4)]);
    for phase in 0..8 {
        let zero = PhaseCatalogue::from_walsh_table(0, &[phase], 0, &limits, &a).unwrap();
        let term = &zero.get(0, &[phase]).unwrap().terms()[0];
        assert_eq!(term.coefficient, a.omega(phase.into(), 0).unwrap());
        assert_eq!(term.phase.num_variables(), 0);
        assert!(term.constraints.is_empty());
    }
}

#[test]
fn construction_and_certificate_limits_have_exact_endpoints_and_preserve_old_results() {
    let a = ScalarArithmetic::default();
    let table = [0, 1, 7, 2, 5, 0, 3, 6];
    let original =
        PhaseCatalogue::from_walsh_table(3, &table, 24, &CatalogueLimits::default(), &a).unwrap();
    let usage = original.usage();
    assert!(usage.terms > 1 && usage.parity_rows > 0);
    let limits = CatalogueLimits {
        max_entries: 1,
        max_dimension: 3,
        max_table_values: 8,
        max_terms: usage.terms,
        max_constraint_rows: 0,
        max_parity_rows: usage.parity_rows,
        max_verification_work: usage.verification_work,
        ..Default::default()
    };
    assert_eq!(
        PhaseCatalogue::from_walsh_table(3, &table, 24, &limits, &a)
            .unwrap()
            .usage(),
        usage
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
                max_dimension: 2,
                ..limits
            },
        ),
        (
            "max_table_values",
            CatalogueLimits {
                max_table_values: 7,
                ..limits
            },
        ),
        (
            "max_terms",
            CatalogueLimits {
                max_terms: usage.terms - 1,
                ..limits
            },
        ),
        (
            "max_parity_rows",
            CatalogueLimits {
                max_parity_rows: usage.parity_rows - 1,
                ..limits
            },
        ),
        (
            "max_verification_work",
            CatalogueLimits {
                max_verification_work: usage.verification_work - 1,
                ..limits
            },
        ),
    ] {
        let error = PhaseCatalogue::from_walsh_table(3, &table, 24, &bad, &a).unwrap_err();
        assert!(error.field.contains(field), "{field}: {error}");
        assert_eq!(original.usage(), usage);
        assert_eq!(original.get(3, &table).unwrap().phase_table(), table);
    }
    assert_eq!(
        PhaseCatalogue::from_walsh_table(3, &table, 23, &limits, &a)
            .unwrap_err()
            .field,
        "catalogue.walsh.max_transform_work"
    );
    assert_eq!(
        PhaseCatalogue::from_walsh_table(1, &[0], 2, &CatalogueLimits::default(), &a)
            .unwrap_err()
            .field,
        "catalogue.walsh.core_phase_mod8"
    );
    assert_eq!(
        PhaseCatalogue::from_walsh_table(1, &[0, 8], 2, &CatalogueLimits::default(), &a)
            .unwrap_err()
            .field,
        "catalogue.walsh.core_phase_mod8.1"
    );
    let unbounded_shape = CatalogueLimits {
        max_dimension: usize::MAX,
        max_table_values: usize::MAX,
        ..Default::default()
    };
    assert_eq!(
        PhaseCatalogue::from_walsh_table(
            usize::BITS as usize,
            &[],
            u128::MAX,
            &unbounded_shape,
            &a
        )
        .unwrap_err()
        .field,
        "catalogue.dimension"
    );
    assert!(
        PhaseCatalogue::from_walsh_table(usize::MAX, &[], 0, &CatalogueLimits::default(), &a)
            .unwrap_err()
            .field
            .contains("max_dimension")
    );
    let limited = ScalarArithmetic::new(1).unwrap();
    assert!(
        PhaseCatalogue::from_walsh_table(
            2,
            &[0, 0, 0, 1],
            8,
            &CatalogueLimits::default(),
            &limited
        )
        .unwrap_err()
        .field
        .starts_with("catalogue.walsh.")
    );
    assert_eq!(original.usage(), usage);
    verify_table(3, &table);
}

fn exact_sum(
    phase: &ParityPhase,
    boundary: &AffineKet,
    label: Option<&BitVector>,
    a: &ScalarArithmetic,
) -> ExactScalar {
    let mut result = ExactScalar::zero();
    for word in 0..1_usize << phase.num_variables() {
        let x = bits(phase.num_variables(), word);
        if let Some(label) = label {
            let mut value = boundary.offset().clone();
            for j in x.ones() {
                value.xor_assign(&boundary.columns()[j]).unwrap();
            }
            if value != *label {
                continue;
            }
        }
        let exponent = phase.evaluate(&x).unwrap() + 2 * boundary.phase().evaluate(&x).unwrap();
        result = a
            .add(&result, &a.omega(exponent.into(), 0).unwrap())
            .unwrap();
    }
    a.multiply(&result, boundary.coefficient()).unwrap()
}

#[test]
fn prepared_scalar_and_dependent_labelled_amplitudes_match_literal_original_domain_sums() {
    let a = ScalarArithmetic::default();
    let reference = ParityPhase::new(
        4,
        3,
        [
            (bits(4, 1), 1),
            (bits(4, 6), 7),
            (bits(4, 5), 3),
            (bits(4, 8), 2),
        ],
    )
    .unwrap();
    let quotient = reference.quotient().unwrap();
    assert_eq!(quotient.rank(), 3);
    let reduced = reference
        .restrict(&bits(4, 0), quotient.complement())
        .unwrap();
    let core = CanonicalPhaseCore::new(&reduced, &PhaseCoreLimits::default()).unwrap();
    let catalogue = PhaseCatalogue::from_walsh_table(
        core.dimension(),
        core.phase_table(),
        1 << 16,
        &CatalogueLimits::default(),
        &a,
    )
    .unwrap();
    let options = CataloguePreparationOptions::default();
    let prepared = PreparedCataloguePhase::new(&reference, &catalogue, &options, &a)
        .unwrap()
        .unwrap();
    assert_eq!(prepared.quotient_rank(), 3);
    assert!(prepared.geometry_words() > 0);
    let geometry_error = PreparedCataloguePhase::new(
        &reference,
        &catalogue,
        &CataloguePreparationOptions {
            max_geometry_words: 0,
            ..options
        },
        &a,
    )
    .err()
    .unwrap();
    assert!(geometry_error.to_string().contains("max_geometry_words"));
    drop(catalogue);
    let q = QuadraticForm::new(
        1,
        vec![1, 2, 3, 0],
        vec![bits(4, 10), bits(4, 4), bits(4, 8), bits(4, 0)],
    )
    .unwrap();
    let coefficient = a
        .scalar([1.into(), 1.into(), 1.into(), (-1).into()], -2)
        .unwrap();
    let boundary = AffineKet::new(
        bits(3, 4),
        vec![bits(3, 1), bits(3, 2), bits(3, 3), bits(3, 0)],
        q,
        coefficient,
    )
    .unwrap();
    for global in 0..4 {
        let updated = ParityPhase::new(
            4,
            (reference.constant() + 2 * global) % 8,
            reference
                .terms()
                .iter()
                .cloned()
                .chain([(bits(4, 9), 2 * global)]),
        )
        .unwrap();
        assert_eq!(
            prepared
                .evaluate(&updated, boundary.phase(), boundary.coefficient(), &a)
                .unwrap(),
            exact_sum(&updated, &boundary, None, &a)
        );
        let expansions = prepared.expand(&updated, &boundary, &a).unwrap();
        for label in 0..8 {
            let y = bits(3, label);
            let actual = expansions.iter().fold(ExactScalar::zero(), |sum, ket| {
                a.add(&sum, &ket.prepare().unwrap().amplitude(&y, &a).unwrap())
                    .unwrap()
            });
            assert_eq!(
                actual,
                exact_sum(&updated, &boundary, Some(&y), &a),
                "update {global}, label {label}"
            );
        }
    }
    let changed = ParityPhase::new(4, 0, []).unwrap();
    assert!(
        prepared
            .evaluate(&changed, boundary.phase(), &ExactScalar::zero(), &a)
            .is_err()
    );
    let zero_boundary = AffineKet::new(
        boundary.offset().clone(),
        boundary.columns().to_vec(),
        boundary.phase().clone(),
        ExactScalar::zero(),
    )
    .unwrap();
    assert!(prepared.expand(&changed, &zero_boundary, &a).is_err());
    assert_eq!(
        prepared
            .evaluate(&reference, boundary.phase(), boundary.coefficient(), &a)
            .unwrap(),
        exact_sum(&reference, &boundary, None, &a)
    );
}
