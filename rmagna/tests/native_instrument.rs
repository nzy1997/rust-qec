use rmagna::circuit::{BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    CompileOptions, FaultSelection, InstrumentQueryLayout as Q, QueryPreparation,
    QueryPreparationOutcome as O, compile_affine, compile_unitary,
};
use rmagna::pauli::PauliWord;
use rmagna::phase::{NativePhaseOptions, PhaseResourceKind};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::unitary::{CompilerStrategy, DensityBoundary, DensityLayout};
fn bits(n: usize, x: usize) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|j| x >> j & 1 != 0)).unwrap()
}
fn total(s: rmagna::phase::NativeRouteCounts) -> usize {
    s.direct + s.product + s.walsh + s.injected
}
#[test]
fn explicit_native_families_retain_the_faulted_projected_queries_and_labelled_parent() {
    let a = ScalarArithmetic::default();
    let source = parse_circuit(
        "H 0\nT 0\nH 0\nCX 0 1\nM(1/7) 1\nDETECTOR rec[-1]\nPOSTSELECT det[0]\nX_ERROR(1/4) 0",
        &ParseOptions::default(),
    )
    .unwrap();
    let target = compile_unitary(
        &parse_circuit("H 0\nCX 0 1", &ParseOptions::default()).unwrap(),
        &CompileOptions::default(),
        &a,
    )
    .unwrap();
    let target_rows = [bits(2, 1)];
    let r = [BitExpr::atom(BitAtom::Record(0))];
    let z = PauliWord::new(bits(1, 0), bits(1, 1), 2).unwrap();
    let zero_word = PauliWord::new(bits(1, 1), bits(1, 0), 0).unwrap();
    let p0 = a
        .add(&a.omega(0, -2).unwrap(), &a.omega(0, -3).unwrap())
        .unwrap();
    let p1 = a.subtract(&ExactScalar::one(), &p0).unwrap();
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for injection in [None, Some(Default::default())] {
            let mode = QueryPreparation::Native(NativePhaseOptions {
                injection,
                ..Default::default()
            });
            assert!(mode.is_fixed());
            let p = compile_affine(
                &source,
                &CompileOptions {
                    compiler_strategy: strategy,
                    probability_preparation: mode,
                    density_preparation: mode,
                    moment_preparation: mode,
                    ..Default::default()
                },
                &a,
            )
            .unwrap();
            let density = Q::TargetDensity {
                outputs: &[0],
                target: &target,
                target_layout: DensityLayout {
                    outputs: &[1],
                    event_rows: &target_rows,
                },
            };
            for query in [
                Q::Acceptance,
                Q::AcceptedProbability { parities: &r },
                density,
                Q::PauliMoment {
                    outputs: &[0],
                    word: &z,
                },
                Q::ReportedSupport,
            ] {
                assert_eq!(p.prepare_query(query, 256, &a).unwrap(), O::Created);
                assert_eq!(p.prepare_query(query, 256, &a).unwrap(), O::Reused);
                assert!(p.prepare_query(query, 0, &a).is_err());
            }
            assert_eq!(
                p.prepare_query(
                    Q::PauliMoment {
                        outputs: &[0],
                        word: &zero_word
                    },
                    256,
                    &a
                )
                .unwrap(),
                O::Created
            );
            let before = (
                p.prepared_probability_statistics().unwrap(),
                p.prepared_density_statistics().unwrap(),
                p.prepared_moment_statistics().unwrap(),
            );
            assert_eq!(before.0.cached_queries, 2);
            assert_eq!(before.1.cached_queries, 1);
            assert_eq!(before.2.cached_queries, 2);
            assert_eq!(before.2.fixed_resources, 2);
            for s in [before.0, before.1, before.2] {
                assert_eq!(total(s.native_routes), s.fixed_resources);
                assert_eq!(s.catalogue_resources, 0);
            }
            let support = p.prepared_support_statistics().unwrap().unwrap();
            assert_eq!(support.resource_kind, PhaseResourceKind::NativeSelection);
            assert_eq!(total(support.native_routes), 1);
            let site = |denominator: i64| {
                p.noise_sites()
                    .iter()
                    .position(|s| {
                        s.alternatives().iter().any(|v| {
                            v.probability().value()
                                == &num_rational::BigRational::new(1.into(), denominator.into())
                        })
                    })
                    .unwrap()
            };
            let report_site = site(7);
            let physical_site = site(4);
            for fault_mask in 0..4 {
                let faults: Vec<_> = [(report_site, 1), (physical_site, 2)]
                    .into_iter()
                    .filter(|(_, bit)| fault_mask & bit != 0)
                    .map(|(site, _)| FaultSelection {
                        site,
                        alternative: 0,
                    })
                    .collect();
                let report = fault_mask & 1;
                let physical = (fault_mask >> 1) & 1;
                let accepted = if report == 0 { p0.clone() } else { p1.clone() };
                assert_eq!(p.acceptance(&faults, 256, &a).unwrap().value, accepted);
                for outcome in 0..2 {
                    assert_eq!(
                        p.reported_probability(&r, &bits(1, outcome), &faults, 256, &a)
                            .unwrap()
                            .value,
                        if outcome ^ report == 0 {
                            p0.clone()
                        } else {
                            p1.clone()
                        }
                    );
                    assert_eq!(
                        p.accepted_probability(&r, &bits(1, outcome), &faults, 256, &a)
                            .unwrap()
                            .value,
                        if outcome == 0 {
                            accepted.clone()
                        } else {
                            ExactScalar::zero()
                        }
                    );
                    let value = p
                        .target_density_weight(
                            &[0],
                            &target,
                            DensityBoundary {
                                outputs: &[1],
                                event_rows: &target_rows,
                                outcomes: &bits(1, outcome),
                            },
                            &faults,
                            256,
                            &a,
                        )
                        .unwrap()
                        .value;
                    assert_eq!(
                        value,
                        if outcome == report ^ physical {
                            a.multiply(&accepted, &a.omega(0, -2).unwrap()).unwrap()
                        } else {
                            ExactScalar::zero()
                        }
                    );
                }
                assert_eq!(
                    p.pauli_moment(&[0], &z, &faults, 256, &a).unwrap().value,
                    a.multiply(
                        &accepted,
                        &a.omega(if report ^ physical == 0 { 2 } else { 6 }, 0)
                            .unwrap()
                    )
                    .unwrap()
                );
                assert_eq!(
                    p.pauli_moment(&[0], &zero_word, &faults, 256, &a)
                        .unwrap()
                        .value,
                    ExactScalar::zero()
                );
                let terms = p.reported_support_terms(&r, &faults, 256, &a).unwrap();
                assert!(!terms.is_empty());
                assert!(terms.iter().all(|term| term.width() == 1));
            }
            assert_eq!(
                before,
                (
                    p.prepared_probability_statistics().unwrap(),
                    p.prepared_density_statistics().unwrap(),
                    p.prepared_moment_statistics().unwrap()
                )
            );
            assert_eq!(Some(support), p.prepared_support_statistics().unwrap());
        }
    }
}
#[test]
fn native_construction_errors_never_publish_a_fiber_fallback() {
    let a = ScalarArithmetic::default();
    let source = parse_circuit("H 0\nT 0\nH 0\nM 0", &ParseOptions::default()).unwrap();
    let mut options = NativePhaseOptions::default();
    options.selection.max_core_table_bytes = 0;
    let mode = QueryPreparation::Native(options);
    let p = compile_affine(
        &source,
        &CompileOptions {
            probability_preparation: mode,
            density_preparation: mode,
            moment_preparation: mode,
            ..Default::default()
        },
        &a,
    )
    .unwrap();
    assert!(p.prepare_query(Q::Acceptance, 256, &a).is_err());
    assert_eq!(
        p.prepared_probability_statistics().unwrap().cached_queries,
        0
    );
    assert!(p.prepare_query(Q::ReportedSupport, 256, &a).is_err());
    assert!(p.prepared_support_statistics().unwrap().is_none());
}
