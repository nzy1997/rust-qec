use super::*;
use crate::gf2::BitVector;
use std::cell::Cell;

fn symbolic(n: usize, width: usize) -> AffineKet {
    AffineKet::new(
        BitVector::zeros(width).unwrap(),
        vec![BitVector::zeros(width).unwrap(); n],
        QuadraticForm::new(0, vec![0; n], vec![BitVector::zeros(n).unwrap(); n]).unwrap(),
        ExactScalar::one(),
    )
    .unwrap()
}

#[test]
fn symbolic_input_is_admitted_before_its_factory_and_charged_at_exact_edges() {
    let a = ScalarArithmetic::default();
    let reference = ParityPhase::new(5, 0, []).unwrap();
    let mut options = NativePhaseOptions::default();
    options.transport.mode = NativeTransportMode::Prepared;
    let mut resource =
        PreparedPhaseResource::new_native(&reference, 256, options, None, &a).unwrap();
    let standalone = resource
        .native_resource()
        .unwrap()
        .prepare_boundary(&reference, &symbolic(5, 65), &options.transport.limits, &a)
        .unwrap()
        .usage();
    let calls = Cell::new(0);
    let plan = resource
        .prepare_native_transport(
            &reference,
            65,
            || {
                calls.set(calls.get() + 1);
                Ok(symbolic(5, 65))
            },
            &a,
        )
        .unwrap()
        .unwrap();
    let Some(NativeTransportReceipt::Prepared(usage)) = resource.native_transport_receipt() else {
        panic!("successful transport must report its combined admission");
    };
    assert_eq!(calls.get(), 1);
    assert!(usage.geometry_words > standalone.geometry_words);
    assert!(usage.construction_work > standalone.construction_work);
    assert_eq!(usage.consistent_terms, plan.usage().consistent_terms);
    assert_eq!(usage.shared_maps, plan.usage().shared_maps);
    assert_eq!(usage.terms, standalone.terms);
    for (cap, required) in [
        ("catalogue_boundary.max_terms", usage.terms as u128),
        (
            "catalogue_boundary.max_geometry_words",
            usage.geometry_words as u128,
        ),
        (
            "catalogue_boundary.max_construction_work",
            usage.construction_work,
        ),
    ] {
        for limit in [0, required - 1, required, required + 1] {
            let mut bounded = options;
            match cap {
                "catalogue_boundary.max_terms" => {
                    bounded.transport.limits.max_terms = limit as usize
                }
                "catalogue_boundary.max_geometry_words" => {
                    bounded.transport.limits.max_geometry_words = limit as usize
                }
                _ => bounded.transport.limits.max_construction_work = limit,
            }
            let mut candidate =
                PreparedPhaseResource::new_native(&reference, 256, bounded, None, &a).unwrap();
            let before = candidate.native_resource().unwrap().receipt().to_owned();
            let called = Cell::new(false);
            let result = candidate
                .prepare_native_transport(
                    &reference,
                    65,
                    || {
                        called.set(true);
                        Ok(symbolic(5, 65))
                    },
                    &a,
                )
                .unwrap();
            assert_eq!(candidate.native_resource().unwrap().receipt(), &before);
            assert_eq!(called.get(), limit >= required);
            assert_eq!(result.is_some(), limit >= required);
            if limit < required {
                assert_eq!(
                    candidate.native_transport_receipt(),
                    Some(NativeTransportReceipt::BudgetSkipped {
                        option: cap,
                        required,
                        limit,
                    })
                );
            } else {
                assert_eq!(
                    candidate.native_transport_receipt(),
                    Some(NativeTransportReceipt::Prepared(usage))
                );
            }
        }
    }
}

#[test]
fn off_skips_the_factory_and_admitted_shape_errors_stay_hard() {
    let a = ScalarArithmetic::default();
    let reference = ParityPhase::new(5, 0, []).unwrap();
    let mut resource =
        PreparedPhaseResource::new_native(&reference, 256, NativePhaseOptions::default(), None, &a)
            .unwrap();
    assert!(
        resource
            .prepare_native_transport(&reference, 3, || panic!("off must not construct input"), &a)
            .unwrap()
            .is_none()
    );
    assert_eq!(resource.native_transport_receipt(), None);
    let mut options = NativePhaseOptions::default();
    options.transport.mode = NativeTransportMode::Prepared;
    for factory in [false, true] {
        let mut resource =
            PreparedPhaseResource::new_native(&reference, 256, options, None, &a).unwrap();
        let result = resource.prepare_native_transport(
            &reference,
            3,
            || {
                if factory {
                    Err(PhaseError::DimensionMismatch)
                } else {
                    Ok(symbolic(5, 4))
                }
            },
            &a,
        );
        assert!(matches!(result, Err(PhaseError::DimensionMismatch)));
        assert_eq!(resource.native_transport_receipt(), None);
    }
    let mut resource =
        PreparedPhaseResource::new_native(&reference, 256, options, None, &a).unwrap();
    let wrong = ParityPhase::new(6, 0, []).unwrap();
    assert!(matches!(
        resource.prepare_native_transport(
            &wrong,
            3,
            || panic!("dimension mismatch precedes factory"),
            &a
        ),
        Err(PhaseError::DimensionMismatch)
    ));
}
