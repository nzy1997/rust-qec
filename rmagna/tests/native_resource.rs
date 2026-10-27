use rmagna::catalogue::{PhaseInjectionOptions, builtin_phase_catalogue};
use rmagna::gf2::BitVector;
use rmagna::phase::{
    NativePhaseOptions, NativeRouteCounts, ParityPhase, PhaseError, PreparedPhaseResource,
};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
fn bits(n: usize, x: usize) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|j| x >> j & 1 != 0)).unwrap()
}
fn injected_phase() -> ParityPhase {
    ParityPhase::new(
        7,
        1,
        [
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
        ]
        .map(|(x, c)| (bits(7, x), c)),
    )
    .unwrap()
}
#[test]
fn owned_native_resources_preserve_every_route_complex_updates_and_dependent_labels() {
    let a = ScalarArithmetic::default();
    let ccz = ParityPhase::new(
        3,
        3,
        (1_usize..8).map(|m| (bits(3, m), if m.count_ones() % 2 == 1 { 1 } else { 7 })),
    )
    .unwrap();
    let product = ParityPhase::new(3, 5, (0..3).map(|j| (bits(3, 1 << j), 1))).unwrap();
    let mut observed = NativeRouteCounts::default();
    for (reference, database, injection, expected) in [
        (ccz, true, false, "catalogue"),
        (product.clone(), true, false, "positive_t_product"),
        (product, false, false, "walsh"),
        (injected_phase(), true, true, "injected"),
    ] {
        let mut options = NativePhaseOptions::default();
        if injection {
            options.injection = Some(Default::default());
        }
        let resource = {
            let db = database.then(|| builtin_phase_catalogue().unwrap().clone());
            PreparedPhaseResource::new_native(&reference, 256, options, db.as_ref(), &a).unwrap()
        };
        let frozen = resource.native_resource().unwrap();
        let actual = if frozen.injection_receipt().is_some_and(|r| r.selected) {
            "injected"
        } else {
            frozen.receipt().route.name()
        };
        assert_eq!(actual, expected);
        observed.record(&resource);
        let n = reference.num_variables();
        let q = QuadraticForm::new(
            3,
            (0..n).map(|j| (j % 4) as u8).collect(),
            (0..n)
                .map(|j| {
                    BitVector::from_indices(n, (j + 1..n).filter(|k| (j + k) % 2 == 1)).unwrap()
                })
                .collect(),
        )
        .unwrap();
        let coefficient = a
            .scalar([1.into(), (-2).into(), 3.into(), 1.into()], -2)
            .unwrap();
        for update in 0..4 {
            let mut terms = reference.terms().to_vec();
            terms.push((bits(n, (1 << n) - 1), 2 * update));
            let updated = ParityPhase::new(n, (reference.constant() + update) & 7, terms).unwrap();
            let boundary = AffineKet::new(
                bits(3, 1),
                (0..n).map(|j| bits(3, [1, 3, 1, 0][j % 4])).collect(),
                q.clone(),
                coefficient.clone(),
            )
            .unwrap();
            let mut expected = ExactScalar::zero();
            let mut labels = vec![ExactScalar::zero(); 8];
            for x in 0..1 << n {
                let x = bits(n, x);
                let value = a
                    .multiply(
                        &coefficient,
                        &a.omega(
                            i64::from(updated.evaluate(&x).unwrap() + 2 * q.evaluate(&x).unwrap()),
                            0,
                        )
                        .unwrap(),
                    )
                    .unwrap();
                expected = a.add(&expected, &value).unwrap();
                let mut label = boundary.offset().clone();
                for j in x.ones() {
                    label.xor_assign(&boundary.columns()[j]).unwrap();
                }
                let index: usize = label.ones().map(|j| 1 << j).sum();
                labels[index] = a.add(&labels[index], &value).unwrap();
            }
            assert_eq!(
                resource.evaluate(&updated, &q, &coefficient, &a).unwrap(),
                expected
            );
            let expanded = resource.expand(&updated, &boundary, &a).unwrap();
            for (word, expected) in labels.iter().enumerate() {
                let actual = expanded.iter().fold(ExactScalar::zero(), |sum, ket| {
                    a.add(
                        &sum,
                        &ket.prepare()
                            .unwrap()
                            .amplitude(&bits(3, word), &a)
                            .unwrap(),
                    )
                    .unwrap()
                });
                assert_eq!(&actual, expected);
            }
        }
        let mut changed = reference.terms().to_vec();
        changed.push((bits(n, 1), 1));
        let changed = ParityPhase::new(n, 0, changed).unwrap();
        assert_eq!(
            resource
                .evaluate(&reference, &q, &ExactScalar::zero(), &a)
                .unwrap(),
            ExactScalar::zero()
        );
        assert!(matches!(
            resource.evaluate(&changed, &q, &ExactScalar::zero(), &a),
            Err(PhaseError::NotClifford)
        ));
        assert!(resource.check_term_limit(resource.num_terms()).is_ok());
        assert!(resource.check_term_limit(resource.num_terms() - 1).is_err());
    }
    assert_eq!(
        observed,
        NativeRouteCounts {
            direct: 1,
            product: 1,
            walsh: 1,
            injected: 1,
            injection_budget_skips: 0
        }
    );
}
#[test]
fn optional_candidate_bounds_do_not_admit_an_oversized_frozen_fallback() {
    let a = ScalarArithmetic::default();
    let phase = injected_phase();
    let db = Some(builtin_phase_catalogue().unwrap());
    let mut options = NativePhaseOptions {
        injection: Some(Default::default()),
        ..Default::default()
    };
    options.selection.construction.max_terms = 64;
    options.selection.preparation.max_terms = 8;
    let resource = PreparedPhaseResource::new_native(&phase, 8, options, db, &a).unwrap();
    assert_eq!(resource.num_terms(), 8);
    assert!(
        resource
            .native_resource()
            .unwrap()
            .injection_receipt()
            .unwrap()
            .selected
    );
    options.injection = Some(PhaseInjectionOptions {
        max_work: 0,
        ..Default::default()
    });
    assert!(PreparedPhaseResource::new_native(&phase, 8, options, db, &a).is_err());
    options.selection.preparation.max_terms = 64;
    let resource = PreparedPhaseResource::new_native(&phase, 64, options, db, &a).unwrap();
    assert_eq!(resource.num_terms(), 64);
    let mut counts = NativeRouteCounts::default();
    counts.record(&resource);
    assert_eq!(
        counts,
        NativeRouteCounts {
            walsh: 1,
            injection_budget_skips: 1,
            ..Default::default()
        }
    );
}
