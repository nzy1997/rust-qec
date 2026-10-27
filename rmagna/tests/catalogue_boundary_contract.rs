use rmagna::catalogue::{
    CatalogueBoundaryOptions, FrozenPhasePlan, PhaseSelectionOptions, PreparedCatalogueBoundary,
    builtin_phase_catalogue,
};
use rmagna::gf2::BitVector;
use rmagna::phase::{ParityPhase, PhaseError};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::{AffineKet, StabilizerError};
use std::sync::Arc;

fn bits(n: usize, word: usize) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|j| word >> j & 1 != 0)).unwrap()
}
fn reference() -> ParityPhase {
    ParityPhase::new(5, 0, (0..4).map(|j| (bits(5, 1 << j), 1))).unwrap()
}
fn boundary(cancel: bool, coupled: bool, coefficient: ExactScalar) -> AffineKet {
    let mut upper = (0..5).map(|_| bits(5, 0)).collect::<Vec<_>>();
    if coupled {
        upper[0] = bits(5, 1 << 4);
    }
    AffineKet::new(
        bits(2, 1),
        vec![bits(2, 1), bits(2, 2), bits(2, 3), bits(2, 0), bits(2, 0)],
        QuadraticForm::new(0, vec![0, 0, 0, 0, if cancel { 2 } else { 0 }], upper).unwrap(),
        coefficient,
    )
    .unwrap()
}
fn selected_resource(a: &ScalarArithmetic) -> FrozenPhasePlan {
    FrozenPhasePlan::new(
        &reference(),
        Some(builtin_phase_catalogue().unwrap()),
        &PhaseSelectionOptions::default(),
        a,
    )
    .unwrap()
}
fn assert_limit(result: Result<PreparedCatalogueBoundary, PhaseError>, name: &str, expected: u128) {
    match result {
        Err(PhaseError::ResourceLimit {
            option,
            required,
            limit,
        }) => {
            assert_eq!(option, name);
            assert_eq!(required, expected);
            assert!(required > limit);
        }
        _ => panic!("expected named construction admission failure"),
    }
}

#[test]
fn admission_endpoints_precede_construction_and_failures_leave_the_plan_unchanged() {
    let a = ScalarArithmetic::new(8192).unwrap();
    let phase = reference();
    let resource = selected_resource(&a);
    let input = boundary(false, false, ExactScalar::one());
    let prepared = resource
        .prepare_boundary(&phase, &input, &CatalogueBoundaryOptions::default(), &a)
        .unwrap();
    let usage = prepared.usage();
    assert_eq!(usage.terms, 4);
    assert_eq!(usage.consistent_terms, 4);
    assert_eq!(usage.shared_maps, 1);
    let exact = CatalogueBoundaryOptions {
        max_terms: usage.terms,
        max_geometry_words: usage.geometry_words,
        max_construction_work: usage.construction_work,
    };
    assert_eq!(
        resource
            .prepare_boundary(&phase, &input, &exact, &a)
            .unwrap()
            .usage(),
        usage
    );
    assert_limit(
        resource.prepare_boundary(
            &phase,
            &input,
            &CatalogueBoundaryOptions {
                max_terms: usage.terms - 1,
                ..exact
            },
            &a,
        ),
        "catalogue_boundary.max_terms",
        usage.terms as u128,
    );
    assert_limit(
        resource.prepare_boundary(
            &phase,
            &input,
            &CatalogueBoundaryOptions {
                max_geometry_words: usage.geometry_words - 1,
                ..exact
            },
            &a,
        ),
        "catalogue_boundary.max_geometry_words",
        usage.geometry_words as u128,
    );
    assert_limit(
        resource.prepare_boundary(
            &phase,
            &input,
            &CatalogueBoundaryOptions {
                max_construction_work: usage.construction_work - 1,
                ..exact
            },
            &a,
        ),
        "catalogue_boundary.max_construction_work",
        usage.construction_work,
    );
    let before = prepared.expand(&phase, &input, &a).unwrap();
    let small = ScalarArithmetic::new(1).unwrap();
    assert!(
        prepared
            .expand(
                &phase,
                &boundary(false, false, a.integer(3).unwrap()),
                &small
            )
            .is_err()
    );
    assert!(
        resource
            .prepare_boundary(
                &phase,
                &boundary(false, false, a.integer(3).unwrap()),
                &exact,
                &small
            )
            .is_err()
    );
    let zero = ExactScalar::zero();
    let wrong = ParityPhase::new(5, 0, [(bits(5, 1), 1)]).unwrap();
    assert!(matches!(
        prepared.expand(&wrong, &boundary(false, false, zero.clone()), &a),
        Err(PhaseError::NotClifford)
    ));
    let changed = AffineKet::new(
        bits(2, 1),
        vec![bits(2, 2); 5],
        input.phase().clone(),
        zero.clone(),
    )
    .unwrap();
    assert!(matches!(
        prepared.expand(&phase, &changed, &a),
        Err(PhaseError::Stabilizer(
            StabilizerError::IncompatibleBoundaryMap
        ))
    ));
    let changed =
        AffineKet::new(bits(3, 1), vec![bits(3, 1); 5], input.phase().clone(), zero).unwrap();
    assert!(matches!(
        prepared.expand(&phase, &changed, &a),
        Err(PhaseError::Stabilizer(
            StabilizerError::IncompatibleBoundaryMap
        ))
    ));
    let after = prepared.expand(&phase, &input, &a).unwrap();
    assert_eq!(before.states, after.states);
    assert_eq!(before.statistics, after.statistics);
    assert_eq!(prepared.usage(), usage);
}

#[test]
fn zero_reference_revives_with_recorded_steps_and_coupled_updates_recompute() {
    let a = ScalarArithmetic::new(8192).unwrap();
    let phase = reference();
    let resource = selected_resource(&a);
    let reference = boundary(true, false, ExactScalar::one());
    let prepared = resource
        .prepare_boundary(&phase, &reference, &CatalogueBoundaryOptions::default(), &a)
        .unwrap();
    drop(resource);
    let zero = prepared.expand(&phase, &reference, &a).unwrap();
    assert!(zero.states.is_empty());
    assert_eq!(zero.statistics.gaussian_reuses, 4);
    assert_eq!(zero.statistics.discarded_terms, 4);
    let revived = prepared
        .expand(&phase, &boundary(false, false, ExactScalar::one()), &a)
        .unwrap();
    assert!(!revived.states.is_empty());
    assert_eq!(revived.statistics.gaussian_reuses, 4);
    assert_eq!(revived.statistics.gaussian_recomputations, 0);
    assert_eq!(revived.statistics.map_pullbacks, 1);
    assert_eq!(revived.statistics.shifted_terms, 4);
    let coupled = prepared
        .expand(&phase, &boundary(false, true, ExactScalar::one()), &a)
        .unwrap();
    assert!(coupled.statistics.gaussian_recomputations > 0);
    assert_eq!(
        coupled.statistics.gaussian_recomputations + coupled.statistics.gaussian_reuses,
        4
    );
    let generic = selected_resource(&a)
        .expand(&phase, &boundary(false, true, ExactScalar::one()), &a)
        .unwrap();
    for label in 0..4 {
        let amplitude = |states: &[AffineKet]| {
            states.iter().fold(ExactScalar::zero(), |sum, k| {
                a.add(
                    &sum,
                    &k.prepare().unwrap().amplitude(&bits(2, label), &a).unwrap(),
                )
                .unwrap()
            })
        };
        assert_eq!(amplitude(&coupled.states), amplitude(&generic));
    }
}

#[test]
fn owned_transport_accepts_concurrent_queries_after_all_source_handles_drop() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<PreparedCatalogueBoundary>();
    let a = ScalarArithmetic::new(8192).unwrap();
    let phase = reference();
    let input = boundary(false, true, a.omega(3, -4).unwrap());
    let resource = selected_resource(&a);
    let plan = Arc::new(
        resource
            .prepare_boundary(&phase, &input, &CatalogueBoundaryOptions::default(), &a)
            .unwrap(),
    );
    drop(resource);
    let expected = plan.expand(&phase, &input, &a).unwrap();
    let threads = (0..4)
        .map(|_| {
            let plan = Arc::clone(&plan);
            let phase = phase.clone();
            let input = input.clone();
            std::thread::spawn(move || {
                plan.expand(&phase, &input, &ScalarArithmetic::new(8192).unwrap())
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        let actual = thread.join().unwrap();
        assert_eq!(actual.states, expected.states);
        assert_eq!(actual.statistics, expected.statistics);
    }
}
