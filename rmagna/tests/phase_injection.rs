use rmagna::{
    catalogue::{
        CatalogueLimits, FrozenPhasePlan, PhaseCatalogue, PhaseInjectionOptions,
        PhaseSelectionOptions,
    },
    gf2::BitVector,
    phase::ParityPhase,
    quadratic::QuadraticForm,
    scalar::{ExactScalar, ScalarArithmetic},
    stabilizer::AffineKet,
};
use serde_json::{Value, json};
fn bits(n: usize, x: u64) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|j| x >> j & 1 != 0)).unwrap()
}
fn mask(x: &BitVector) -> u64 {
    x.words().first().copied().unwrap_or(0)
}
fn fraction(v: &Value, a: &ScalarArithmetic) -> ExactScalar {
    a.scalar(
        std::array::from_fn(|j| {
            let n: i64 = v[j][0].as_str().unwrap().parse().unwrap();
            let d: i64 = v[j][1].as_str().unwrap().parse().unwrap();
            assert_eq!(4 % d, 0);
            (n * (4 / d)).into()
        }),
        -2,
    )
    .unwrap()
}
fn dependency_case(a: &ScalarArithmetic) -> (ParityPhase, PhaseCatalogue) {
    let data: Value =
        serde_json::from_str(include_str!("fixtures/phase_injection_native.json")).unwrap();
    let row = data["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["case"]["name"] == "random-6-1")
        .unwrap();
    let n = row["case"]["n"].as_u64().unwrap() as usize;
    let phase = ParityPhase::new(
        n,
        row["case"]["constant"].as_u64().unwrap() as u8,
        row["case"]["terms"].as_array().unwrap().iter().map(|p| {
            (
                bits(n, p[0].as_u64().unwrap()),
                p[1].as_u64().unwrap() as u8,
            )
        }),
    )
    .unwrap();
    let catalogue =
        PhaseCatalogue::from_json(&row["database"].to_string(), &CatalogueLimits::default(), a)
            .unwrap();
    (phase, catalogue)
}

#[test]
fn dependent_groups_enumerate_eight_supports_and_preserve_every_original_amplitude() {
    let a = ScalarArithmetic::new(512).unwrap();
    let (reference, catalogue) = dependency_case(&a);
    let mut options = PhaseSelectionOptions::default();
    options.preparation.max_terms = 8;
    options.construction.max_terms = 64;
    let injection = PhaseInjectionOptions {
        max_terms: 8,
        ..Default::default()
    };
    let plan =
        FrozenPhasePlan::new_with_injection(&reference, Some(&catalogue), &options, &injection, &a)
            .unwrap();
    let receipt = plan.injection_receipt().unwrap();
    assert_eq!(receipt.original_terms, 64);
    assert_eq!(receipt.groups.len(), 12);
    assert_eq!(receipt.constraint_rank, 3);
    assert_eq!(receipt.candidate_terms, Some(8));
    assert!(receipt.selected);
    assert_eq!(plan.num_terms(), 8);
    let cartesian = 1_u128 << receipt.groups.len();
    assert!(cartesian > injection.max_terms as u128);
    assert!(receipt.geometry_words < cartesian * plan.quotient_rank() as u128);
    let n = reference.num_variables();
    let q = QuadraticForm::new(0, vec![0; n], vec![bits(n, 0); n]).unwrap();
    let ket = AffineKet::new(
        bits(n, 0),
        (0..n).map(|j| bits(n, 1 << j)).collect(),
        q,
        ExactScalar::one(),
    )
    .unwrap();
    let expanded = plan.expand(&reference, &ket, &a).unwrap();
    drop(catalogue);
    for word in 0..1_u64 << n {
        let y = bits(n, word);
        let sum = expanded.iter().fold(ExactScalar::zero(), |sum, ket| {
            a.add(&sum, &ket.prepare().unwrap().amplitude(&y, &a).unwrap())
                .unwrap()
        });
        assert_eq!(
            sum,
            a.omega(reference.evaluate(&y).unwrap().into(), 0).unwrap(),
            "original word {word}"
        );
    }
}

#[test]
fn optional_budgets_skip_but_final_caps_and_arithmetic_remain_hard_errors() {
    let a = ScalarArithmetic::new(512).unwrap();
    let (reference, catalogue) = dependency_case(&a);
    let source_usage = catalogue.usage();
    let options = PhaseSelectionOptions::default();
    let old = FrozenPhasePlan::new(&reference, Some(&catalogue), &options, &a).unwrap();
    assert_eq!(old.num_terms(), 64);
    let selected = FrozenPhasePlan::new_with_injection(
        &reference,
        Some(&catalogue),
        &options,
        &PhaseInjectionOptions::default(),
        &a,
    )
    .unwrap();
    let receipt = selected.injection_receipt().unwrap();
    assert!(receipt.selected);
    let endpoint = PhaseInjectionOptions {
        max_work: receipt.work_bound,
        max_geometry_words: receipt.geometry_words as usize,
        max_terms: 8,
    };
    let exact =
        FrozenPhasePlan::new_with_injection(&reference, Some(&catalogue), &options, &endpoint, &a)
            .unwrap();
    assert!(exact.injection_receipt().unwrap().selected);
    for (field, below) in [
        (
            "phase_injection.max_work",
            PhaseInjectionOptions {
                max_work: endpoint.max_work - 1,
                ..endpoint
            },
        ),
        (
            "phase_injection.max_geometry_words",
            PhaseInjectionOptions {
                max_geometry_words: endpoint.max_geometry_words - 1,
                ..endpoint
            },
        ),
        (
            "phase_injection.max_terms",
            PhaseInjectionOptions {
                max_terms: 7,
                ..endpoint
            },
        ),
    ] {
        let skipped =
            FrozenPhasePlan::new_with_injection(&reference, Some(&catalogue), &options, &below, &a)
                .unwrap();
        assert_eq!(skipped.num_terms(), 64);
        assert!(!skipped.injection_receipt().unwrap().selected);
        assert_eq!(
            skipped.injection_receipt().unwrap().budget_skip.as_deref(),
            Some(field)
        );
        let mut capped = options;
        capped.preparation.max_terms = 8;
        let failure =
            FrozenPhasePlan::new_with_injection(&reference, Some(&catalogue), &capped, &below, &a)
                .unwrap_err();
        assert_eq!(failure.field, "catalogue_preparation.max_terms");
        let success = FrozenPhasePlan::new_with_injection(
            &reference,
            Some(&catalogue),
            &capped,
            &endpoint,
            &a,
        )
        .unwrap();
        assert_eq!(success.num_terms(), 8);
        assert_eq!(catalogue.usage(), source_usage);
    }
    let constraints = selected.receipt().certification.constraint_rows;
    assert_eq!(constraints, 24);
    let mut exact_constraints = options;
    exact_constraints.construction.max_constraint_rows = constraints;
    assert!(
        FrozenPhasePlan::new_with_injection(
            &reference,
            Some(&catalogue),
            &exact_constraints,
            &endpoint,
            &a
        )
        .unwrap()
        .injection_receipt()
        .unwrap()
        .selected
    );
    exact_constraints.construction.max_constraint_rows -= 1;
    let skipped = FrozenPhasePlan::new_with_injection(
        &reference,
        Some(&catalogue),
        &exact_constraints,
        &endpoint,
        &a,
    )
    .unwrap();
    assert_eq!(
        skipped.injection_receipt().unwrap().budget_skip.as_deref(),
        Some("catalogue.max_constraint_rows")
    );
    assert_eq!(skipped.num_terms(), 64);
    let tiny = ScalarArithmetic::new(1).unwrap();
    assert!(
        FrozenPhasePlan::new_with_injection(
            &reference,
            Some(&catalogue),
            &options,
            &endpoint,
            &tiny
        )
        .is_err()
    );
    assert_eq!(catalogue.usage(), source_usage);
    assert_eq!(old.num_terms(), 64);
    let n = reference.num_variables();
    let zero = QuadraticForm::new(0, vec![0; n], vec![bits(n, 0); n]).unwrap();
    assert_eq!(
        old.evaluate(&reference, &zero, &ExactScalar::one(), &a)
            .unwrap(),
        selected
            .evaluate(&reference, &zero, &ExactScalar::one(), &a)
            .unwrap()
    );
}
#[test]
fn pinned_native_injection_and_complex_transports() {
    let data: Value =
        serde_json::from_str(include_str!("fixtures/phase_injection_native.json")).unwrap();
    let a = ScalarArithmetic::new(512).unwrap();
    let prefactor = a
        .scalar([1.into(), 1.into(), 1.into(), (-1).into()], -2)
        .unwrap();
    let mut points = 0;
    let mut comparisons = 0;
    let mut labels = 0;
    for row in data["cases"].as_array().unwrap() {
        let case = &row["case"];
        let n = case["n"].as_u64().unwrap() as usize;
        let constant = case["constant"].as_u64().unwrap() as u8;
        let terms: Vec<_> = case["terms"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| {
                (
                    bits(n, pair[0].as_u64().unwrap()),
                    pair[1].as_u64().unwrap() as u8,
                )
            })
            .collect();
        let reference = ParityPhase::new(n, constant, terms.clone()).unwrap();
        let catalogue = if row["database"].is_null() {
            None
        } else {
            Some(
                PhaseCatalogue::from_json(
                    &row["database"].to_string(),
                    &CatalogueLimits::default(),
                    &a,
                )
                .unwrap(),
            )
        };
        let baseline = FrozenPhasePlan::new(
            &reference,
            catalogue.as_ref(),
            &PhaseSelectionOptions::default(),
            &a,
        )
        .unwrap();
        assert_eq!(
            baseline.num_terms() as u64,
            row["original_terms"].as_u64().unwrap(),
            "baseline count {}",
            case["name"]
        );
        assert_eq!(
            baseline
                .signature_axes()
                .iter()
                .map(mask)
                .collect::<Vec<_>>(),
            row["axes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            baseline
                .section_lifts()
                .iter()
                .map(mask)
                .collect::<Vec<_>>(),
            row["lifts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            baseline
                .certificate()
                .phase_table()
                .iter()
                .map(|&v| u64::from(v))
                .collect::<Vec<_>>(),
            row["table"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .collect::<Vec<_>>()
        );
        // The native injected terms must reconstruct every exact core phase.
        for y in 0..row["table"].as_array().unwrap().len() {
            let mut sum = ExactScalar::zero();
            for term in row["resource"]["exact_entries"].as_array().unwrap() {
                if term[1].as_array().unwrap().iter().all(|pair| {
                    ((pair[0].as_u64().unwrap() & y as u64).count_ones() & 1)
                        == pair[1].as_u64().unwrap() as u32
                }) {
                    let coeff = a
                        .scalar(
                            std::array::from_fn(|j| term[0][0][j].as_i64().unwrap().into()),
                            term[0][1].as_i64().unwrap(),
                        )
                        .unwrap();
                    let e: i64 = term[2]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|pair| {
                            2 * pair[1].as_i64().unwrap()
                                * i64::from((pair[0].as_u64().unwrap() & y as u64).count_ones() & 1)
                        })
                        .sum();
                    sum = a
                        .add(&sum, &a.multiply(&coeff, &a.omega(e, 0).unwrap()).unwrap())
                        .unwrap();
                }
            }
            assert_eq!(sum, a.omega(row["table"][y].as_i64().unwrap(), 0).unwrap());
            points += 1;
        }
        let plan = FrozenPhasePlan::new_with_injection(
            &reference,
            catalogue.as_ref(),
            &PhaseSelectionOptions::default(),
            &PhaseInjectionOptions::default(),
            &a,
        )
        .unwrap();
        let receipt = plan.injection_receipt().unwrap();
        assert_eq!(
            receipt.original_terms as u64,
            row["original_terms"].as_u64().unwrap()
        );
        assert_eq!(
            receipt.odd_rotations as u64,
            row["rotations"].as_u64().unwrap()
        );
        assert_eq!(
            receipt
                .groups
                .iter()
                .map(|group| group.iter().map(mask).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            row["groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(|group| group
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap())
                    .collect::<Vec<_>>())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            receipt.candidate_terms.unwrap_or(0) as u64,
            row["candidate_terms"].as_u64().unwrap()
        );
        assert_eq!(
            receipt.constraint_rank,
            row["constraint_rows"].as_array().unwrap().len()
        );
        assert_eq!(receipt.selected, row["selected"].as_bool().unwrap());
        assert_eq!(receipt.constructed, receipt.selected);
        assert!(receipt.budget_skip.is_none());
        assert_eq!(
            plan.num_terms(),
            row["resource"]["exact_entries"].as_array().unwrap().len()
        );
        for (actual, expected) in plan
            .certificate()
            .terms()
            .iter()
            .zip(row["resource"]["exact_entries"].as_array().unwrap())
        {
            assert_eq!(
                actual.coefficient,
                a.scalar(
                    std::array::from_fn(|j| expected[0][0][j].as_i64().unwrap().into()),
                    expected[0][1].as_i64().unwrap()
                )
                .unwrap()
            );
            assert_eq!(
                actual
                    .constraints
                    .iter()
                    .map(|(v, b)| (mask(v), u64::from(*b)))
                    .collect::<Vec<_>>(),
                expected[1]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| (p[0].as_u64().unwrap(), p[1].as_u64().unwrap()))
                    .collect::<Vec<_>>()
            );
            assert_eq!(actual.phase.constant(), 0);
            assert_eq!(
                actual
                    .phase
                    .terms()
                    .iter()
                    .map(|(v, c)| (mask(v), u64::from(*c) / 2))
                    .collect::<Vec<_>>(),
                expected[2]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|p| p[0].as_u64().unwrap() != 0)
                    .map(|p| (p[0].as_u64().unwrap(), p[1].as_u64().unwrap()))
                    .collect::<Vec<_>>()
            );
        }
        drop(catalogue);
        for update in 0..4_usize {
            let mut updated = terms.clone();
            updated.push((bits(n, (1 << n) - 1), 2 * update as u8));
            let updated = ParityPhase::new(n, (constant + update as u8) & 7, updated).unwrap();
            let boundary = QuadraticForm::new(
                0,
                (0..n).map(|j| ((j + update) % 4) as u8).collect(),
                (0..n)
                    .map(|j| {
                        BitVector::from_indices(n, (j + 1..n).filter(|k| (j + k) % 2 == 1)).unwrap()
                    })
                    .collect(),
            )
            .unwrap();
            let expected = fraction(&row["expected_sums"][update], &a);
            assert_eq!(
                plan.evaluate(&updated, &boundary, &prefactor, &a).unwrap(),
                expected,
                "injected scalar {} update{update}",
                case["name"]
            );
            comparisons += 1;
            let ket = AffineKet::new(
                bits(3, 1),
                (0..n).map(|j| bits(3, [1, 3, 1, 0][j % 4])).collect(),
                boundary,
                prefactor.clone(),
            )
            .unwrap();
            let expansions = plan.expand(&updated, &ket, &a).unwrap();
            for word in 0..8_u64 {
                let actual = expansions.iter().fold(ExactScalar::zero(), |sum, ket| {
                    a.add(
                        &sum,
                        &ket.prepare()
                            .unwrap()
                            .amplitude(&bits(3, word), &a)
                            .unwrap(),
                    )
                    .unwrap()
                });
                assert_eq!(
                    actual,
                    fraction(&row["expected_labels"][update][word as usize], &a),
                    "injected dependent amplitude {} update{update} word{word}",
                    case["name"]
                );
                labels += 1;
            }
            if !row["axes"].as_array().unwrap().is_empty() {
                let changed = ParityPhase::new(n, 0, []).unwrap();
                assert!(
                    plan.evaluate(&changed, ket.phase(), &ExactScalar::zero(), &a)
                        .is_err()
                );
                let zero = AffineKet::new(
                    ket.offset().clone(),
                    ket.columns().to_vec(),
                    ket.phase().clone(),
                    ExactScalar::zero(),
                )
                .unwrap();
                assert!(plan.expand(&changed, &zero, &a).is_err());
            }
        }
    }
    assert!(comparisons >= 400 && labels >= 3200);
    println!(
        "{}",
        json!({"passed":true,"cases":data["cases"].as_array().unwrap().len(),"native_core_pointwise":points,"parent_scalar_comparisons":comparisons,"dependent_labelled_amplitudes":labels,"source_catalogue_dropped":true,"scope":"Exact selected indicators and complex transports checked against preimplementation parent/native expectations."})
    );
}
