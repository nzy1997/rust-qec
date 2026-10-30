use super::*;
use crate::{gf2::FACTORIZATION_CALLS, quadratic::QuadraticForm};

fn bits(n: usize, indices: &[usize]) -> BitVector {
    BitVector::from_indices(n, indices.iter().copied()).unwrap()
}
fn calls() -> usize {
    FACTORIZATION_CALLS.with(|n| n.get())
}
fn reset() {
    FACTORIZATION_CALLS.with(|n| n.set(0));
}

#[test]
fn retained_pair_execution_performs_no_new_gf2_factorization() {
    let a = ScalarArithmetic::default();
    let row = bits(2, &[1]);
    let spec = CompletedRecordSpec::new(
        2,
        std::slice::from_ref(&row),
        &[row.clone(), row.clone()],
        &bits(2, &[]),
        Default::default(),
    )
    .unwrap();
    let ket = AffineKet::new(
        bits(2, &[1]),
        vec![bits(2, &[0]), bits(2, &[1])],
        QuadraticForm::new(1, vec![1, 2], vec![bits(2, &[1]), bits(2, &[])]).unwrap(),
        a.omega(1, -2).unwrap(),
    )
    .unwrap();
    let terms = vec![ket.clone(), ket.conjugate()];
    reset();
    let plan = spec.prepare(&terms, Default::default(), &a).unwrap();
    assert!(calls() >= 8);
    assert_eq!(plan.statistics().support_factorizations, 4);
    assert_eq!(plan.statistics().pairing_factorizations, 4);
    for event in [bits(2, &[]), bits(2, &[0, 1]), bits(2, &[1]), bits(2, &[])] {
        for (x, z, sign) in [
            (&[][..], &[][..], 0),
            (&[0][..], &[][..], 0),
            (&[0][..], &[0][..], 2),
            (&[][..], &[0][..], 4),
        ] {
            let word = PauliWord::new(bits(2, x), bits(2, z), sign).unwrap();
            reset();
            let warm = plan.moment(&event, &word, &a).unwrap();
            assert_eq!(calls(), 0, "warm execution called a real GF2 constructor");
            assert_eq!(warm.work.support_factorizations, 0);
            assert_eq!(warm.work.pairing_factorizations, 0);
            assert_eq!(warm.work.support_plan_applications, 4);
            let cold_spec = CompletedRecordSpec::new(
                2,
                &[bits(2, &[1])],
                &[bits(2, &[1]), bits(2, &[1])],
                &event,
                Default::default(),
            )
            .unwrap();
            let cold = cold_spec.moment(&terms, &word, &a).unwrap();
            assert!(
                calls() > 0,
                "negative control must detect actual cold constructors"
            );
            for v in [bits(1, &[]), bits(1, &[0])] {
                assert_eq!(
                    warm.distribution.value(&v, &a).unwrap(),
                    cold.value(&v, &a).unwrap()
                );
            }
        }
    }
}

#[test]
fn inconsistent_events_and_wide_fixed_support_keep_real_plans() {
    let a = ScalarArithmetic::default();
    let n = 32769;
    let row = bits(n, &[32768]);
    let spec = CompletedRecordSpec::new(
        n,
        &[],
        &[row.clone(), row],
        &bits(2, &[0]),
        Default::default(),
    )
    .unwrap();
    let terms = [AffineKet::new(
        bits(n, &[32768]),
        vec![],
        QuadraticForm::new(0, vec![], vec![]).unwrap(),
        ExactScalar::one(),
    )
    .unwrap()];
    reset();
    assert!(matches!(
        spec.prepare(
            &terms,
            PreparedRecordLimits {
                max_geometry_words: 0,
                ..Default::default()
            },
            &a
        ),
        Err(CompletedRecordError::ResourceLimit {
            option: "max_prepared_geometry_words",
            ..
        })
    ));
    assert_eq!(
        calls(),
        0,
        "retained storage must be admitted before factoring"
    );
    let plan = spec.prepare(&terms, Default::default(), &a).unwrap();
    assert!(calls() > 0);
    assert_eq!(plan.statistics().support_factorizations, 1);
    assert_eq!(plan.statistics().pairing_factorizations, 1);
    for (outcome, expected) in [
        (bits(2, &[0]), ExactScalar::zero()),
        (bits(2, &[0, 1]), ExactScalar::one()),
    ] {
        reset();
        let q = plan.probability(&outcome, &a).unwrap();
        assert_eq!(calls(), 0);
        assert_eq!(q.distribution.total(&a).unwrap(), expected);
    }
}

#[test]
fn zero_gaussian_and_cancelling_sources_do_not_replace_structural_preparation() {
    let a = ScalarArithmetic::default();
    let spec =
        CompletedRecordSpec::new(1, &[bits(1, &[0])], &[], &bits(0, &[]), Default::default())
            .unwrap();
    let zero = AffineKet::new(
        bits(1, &[]),
        vec![bits(1, &[])],
        QuadraticForm::new(0, vec![2], vec![bits(1, &[])]).unwrap(),
        ExactScalar::one(),
    )
    .unwrap();
    let basis = AffineKet::new(
        bits(1, &[]),
        vec![],
        QuadraticForm::new(0, vec![], vec![]).unwrap(),
        ExactScalar::one(),
    )
    .unwrap();
    let negative = AffineKet::new(
        bits(1, &[]),
        vec![],
        QuadraticForm::new(0, vec![], vec![]).unwrap(),
        a.omega(4, 0).unwrap(),
    )
    .unwrap();
    for terms in [vec![zero], vec![basis, negative]] {
        let plan = spec.prepare(&terms, Default::default(), &a).unwrap();
        assert_eq!(plan.statistics().support_factorizations, terms.len().pow(2));
        assert_eq!(plan.statistics().pairing_factorizations, terms.len().pow(2));
        reset();
        let q = plan.probability(&bits(0, &[]), &a).unwrap();
        assert_eq!(calls(), 0);
        assert_eq!(q.distribution.total(&a).unwrap(), ExactScalar::zero());
        assert!(matches!(
            plan.probability(&bits(1, &[]), &a),
            Err(CompletedRecordError::DimensionMismatch)
        ));
        assert_eq!(
            plan.probability(&bits(0, &[]), &a)
                .unwrap()
                .distribution
                .total(&a)
                .unwrap(),
            ExactScalar::zero()
        );
    }
}

#[test]
fn existing_cold_quotas_admit_before_prepared_allocations_and_leave_reuse_intact() {
    let a = ScalarArithmetic::default();
    let ket = AffineKet::new(
        bits(1, &[]),
        vec![bits(1, &[0])],
        QuadraticForm::new(0, vec![0], vec![bits(1, &[])]).unwrap(),
        ExactScalar::one(),
    )
    .unwrap();
    let terms = [ket.clone(), ket];
    let good =
        CompletedRecordSpec::new(1, &[bits(1, &[0])], &[], &bits(0, &[]), Default::default())
            .unwrap();
    let plan = good.prepare(&terms, Default::default(), &a).unwrap();
    for (name, limits) in [
        (
            "max_source_entries",
            CompletedRecordLimits {
                max_source_entries: 1,
                ..Default::default()
            },
        ),
        (
            "max_pair_paths",
            CompletedRecordLimits {
                max_pair_paths: 0,
                ..Default::default()
            },
        ),
        (
            "max_pairs",
            CompletedRecordLimits {
                max_pairs: 3,
                ..Default::default()
            },
        ),
        (
            "max_geometry_words",
            CompletedRecordLimits {
                max_geometry_words: (plan.cold_words - 1) as usize,
                ..Default::default()
            },
        ),
        (
            "max_construction_work",
            CompletedRecordLimits {
                max_construction_work: 0,
                ..Default::default()
            },
        ),
    ] {
        let limited =
            CompletedRecordSpec::new(1, &[bits(1, &[0])], &[], &bits(0, &[]), limits).unwrap();
        reset();
        assert!(
            matches!(limited.prepare(&terms,Default::default(),&a),Err(CompletedRecordError::ResourceLimit { option,.. }) if option==name)
        );
        assert_eq!(
            calls(),
            0,
            "quota must reject before the first support/retained plan"
        );
        let warm = plan.probability(&bits(0, &[]), &a).unwrap();
        assert_eq!(calls(), 0);
        assert_eq!(
            warm.distribution.total(&a).unwrap(),
            a.scalar([8, 0, 0, 0].map(Into::into), 0).unwrap()
        );
    }
}
