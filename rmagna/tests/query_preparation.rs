use rmagna::circuit::{BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    CompileOptions, FaultSelection, InstrumentError, InstrumentQueryLayout as Q, QueryPreparation,
    QueryPreparationOutcome as O, compile_affine, compile_unitary,
};
use rmagna::pauli::PauliWord;
use rmagna::phase::PhaseError;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::unitary::{CompilerStrategy, DensityBoundary, DensityLayout, UnitaryError};
fn bits(width: usize, value: usize) -> BitVector {
    BitVector::from_indices(
        width,
        (0..width.min(usize::BITS as usize)).filter(|j| value >> j & 1 != 0),
    )
    .unwrap()
}
fn source() -> rmagna::circuit::Circuit {
    parse_circuit(
        "H 0\nT 0\nH 0\nCX 0 1\nM(1/7) 1\nDETECTOR rec[-1]\nPOSTSELECT det[0]\nX_ERROR(1/4) 0",
        &ParseOptions::default(),
    )
    .unwrap()
}
fn options(mode: QueryPreparation) -> CompileOptions {
    CompileOptions {
        probability_preparation: mode,
        density_preparation: mode,
        moment_preparation: mode,
        ..Default::default()
    }
}
#[test]
fn all_static_layouts_prime_the_same_families_for_faulted_nonzero_outcomes() {
    let a = ScalarArithmetic::default();
    let target = compile_unitary(
        &parse_circuit("H 0\nCX 0 1", &ParseOptions::default()).unwrap(),
        &CompileOptions::default(),
        &a,
    )
    .unwrap();
    let target_rows = [bits(2, 1)];
    let r = [BitExpr::atom(BitAtom::Record(0))];
    let z = PauliWord::new(bits(1, 0), bits(1, 1), 2).unwrap();
    let p0 = a
        .add(&a.omega(0, -2).unwrap(), &a.omega(0, -3).unwrap())
        .unwrap();
    let p1 = a.subtract(&ExactScalar::one(), &p0).unwrap();
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for mode in [
            QueryPreparation::Trace,
            QueryPreparation::FixedFibers(Default::default()),
        ] {
            let mut config = options(mode);
            config.compiler_strategy = strategy;
            let p = compile_affine(&source(), &config, &a).unwrap();
            assert_eq!(
                p.prepared_probability_statistics().unwrap().cached_queries,
                0
            );
            assert_eq!(p.prepare_query(Q::Acceptance, 256, &a).unwrap(), O::Created);
            assert_eq!(
                p.prepare_query(Q::ReportedProbability { parities: &r }, 256, &a)
                    .unwrap(),
                O::Reused
            );
            assert_eq!(
                p.prepare_query(Q::AcceptedProbability { parities: &r }, 256, &a)
                    .unwrap(),
                O::Created
            );
            assert_eq!(
                p.prepare_query(Q::ReportedProbability { parities: &[] }, 256, &a)
                    .unwrap(),
                O::Created
            );
            let density = Q::TargetDensity {
                outputs: &[0],
                target: &target,
                target_layout: DensityLayout {
                    outputs: &[1],
                    event_rows: &target_rows,
                },
            };
            assert_eq!(density.kind(), "target_density");
            assert_eq!(p.prepare_query(density, 256, &a).unwrap(), O::Created);
            assert_eq!(
                p.prepare_query(
                    Q::PauliMoment {
                        outputs: &[0],
                        word: &z
                    },
                    256,
                    &a
                )
                .unwrap(),
                O::Created
            );
            assert_eq!(p.prepare_query(Q::ReportedSupport, 0, &a).unwrap(), O::Cold);
            let before = (
                p.prepared_probability_statistics().unwrap(),
                p.prepared_density_statistics().unwrap(),
                p.prepared_moment_statistics().unwrap(),
            );
            assert_eq!(before.0.cached_queries, 3);
            assert_eq!(before.1.cached_queries, 1);
            assert_eq!(before.2.cached_queries, 1);
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
            let flip = site(7);
            let x = site(4);
            let fault = |site| FaultSelection {
                site,
                alternative: 0,
            };
            for faults in [
                vec![],
                vec![fault(flip)],
                vec![fault(x)],
                vec![fault(x), fault(flip)],
            ] {
                let report = usize::from(faults.iter().any(|f| f.site == flip));
                let state = report ^ usize::from(faults.iter().any(|f| f.site == x));
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
                    let density = p
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
                        .unwrap();
                    assert_eq!(
                        density.value,
                        if outcome == state {
                            a.multiply(&accepted, &a.omega(0, -2).unwrap()).unwrap()
                        } else {
                            ExactScalar::zero()
                        }
                    );
                }
                let moment = a
                    .multiply(
                        &accepted,
                        &a.omega(if state == 0 { 2 } else { 6 }, 0).unwrap(),
                    )
                    .unwrap();
                assert_eq!(
                    p.pauli_moment(&[0], &z, &faults, 256, &a).unwrap().value,
                    moment
                );
                assert_eq!(
                    p.reported_probability(&[], &bits(0, 0), &faults, 256, &a)
                        .unwrap()
                        .value,
                    ExactScalar::one()
                );
                // Structural support excludes the POSTSELECT predicate.
                let support = p.reported_support(&r, &faults, &a).unwrap();
                assert_eq!((support.width(), support.dimension()), (1, 1));
            }
            assert_eq!(p.prepare_query(density, 256, &a).unwrap(), O::Reused);
            assert_eq!(
                p.prepare_query(Q::ReportedSupport, 256, &a).unwrap(),
                O::Cold
            );
            assert_eq!(
                before,
                (
                    p.prepared_probability_statistics().unwrap(),
                    p.prepared_density_statistics().unwrap(),
                    p.prepared_moment_statistics().unwrap(),
                )
            );
        }
    }
}
#[test]
fn fixed_caps_apply_to_new_and_reused_resources_without_changing_zero_queries() {
    let a = ScalarArithmetic::default();
    let r = [BitExpr::atom(BitAtom::Record(0))];
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let mut config = options(QueryPreparation::FixedFibers(Default::default()));
        config.compiler_strategy = strategy;
        config.max_prepared_probabilities = 2;
        config.max_prepared_densities = 1;
        config.max_prepared_moments = 1;
        let p = compile_affine(&source(), &config, &a).unwrap();
        let z = PauliWord::new(bits(1, 0), bits(1, 1), 0).unwrap();
        let target = compile_unitary(
            &parse_circuit("I 0", &ParseOptions::default()).unwrap(),
            &CompileOptions::default(),
            &a,
        )
        .unwrap();
        let layout = DensityLayout {
            outputs: &[0],
            event_rows: &[],
        };
        for query in [
            Q::AcceptedProbability { parities: &r },
            Q::PauliMoment {
                outputs: &[0],
                word: &z,
            },
            Q::TargetDensity {
                outputs: &[0],
                target: &target,
                target_layout: layout,
            },
        ] {
            assert!(p.prepare_query(query, 0, &a).is_err());
            assert_eq!(p.prepare_query(query, 256, &a).unwrap(), O::Created);
            assert!(matches!(
                p.prepare_query(query, 0, &a),
                Err(InstrumentError::Unitary(UnitaryError::Phase(
                    PhaseError::FiberTermLimit { limit: 0, .. }
                )))
            ));
            assert_eq!(p.prepare_query(query, 256, &a).unwrap(), O::Reused);
        }
        // A cached family may still answer an impossible event without enumerating
        // its fixed phase resource, although preparing that resource needs a budget.
        assert_eq!(
            p.accepted_probability(&r, &bits(1, 1), &[], 0, &a)
                .unwrap()
                .value,
            ExactScalar::zero()
        );
        assert_eq!(p.prepare_query(Q::Acceptance, 256, &a).unwrap(), O::Created);
        assert!(matches!(
            p.prepare_query(Q::ReportedProbability { parities: &[] }, 256, &a),
            Err(InstrumentError::ResourceLimit {
                option: "max_prepared_probabilities",
                limit: 2,
                required: 3
            })
        ));
        let other = PauliWord::new(bits(1, 0), bits(1, 1), 4).unwrap();
        assert!(matches!(
            p.prepare_query(
                Q::PauliMoment {
                    outputs: &[0],
                    word: &other
                },
                256,
                &a
            ),
            Err(InstrumentError::ResourceLimit {
                option: "max_prepared_moments",
                limit: 1,
                required: 2
            })
        ));
        let independent = compile_unitary(
            &parse_circuit("I 0", &ParseOptions::default()).unwrap(),
            &CompileOptions::default(),
            &a,
        )
        .unwrap();
        assert!(matches!(
            p.prepare_query(
                Q::TargetDensity {
                    outputs: &[0],
                    target: &independent,
                    target_layout: layout
                },
                256,
                &a
            ),
            Err(InstrumentError::ResourceLimit {
                option: "max_prepared_densities",
                limit: 1,
                required: 2
            })
        ));
        assert_eq!(
            p.prepared_probability_statistics().unwrap().cached_queries,
            2
        );
        assert_eq!(p.prepared_density_statistics().unwrap().cached_queries, 1);
        assert_eq!(p.prepared_moment_statistics().unwrap().cached_queries, 1);
    }
}
#[test]
fn cold_policies_validate_layouts_and_report_disabled_families_explicitly() {
    let a = ScalarArithmetic::default();
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let p = compile_affine(
            &source(),
            &CompileOptions {
                compiler_strategy: strategy,
                max_prepared_probabilities: 0,
                max_prepared_densities: 0,
                max_prepared_moments: 0,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
        let target = compile_unitary(
            &parse_circuit("I 0 1", &ParseOptions::default()).unwrap(),
            &CompileOptions::default(),
            &a,
        )
        .unwrap();
        let z = PauliWord::new(bits(1, 0), bits(1, 1), 0).unwrap();
        for q in [
            Q::Acceptance,
            Q::ReportedProbability { parities: &[] },
            Q::AcceptedProbability { parities: &[] },
            Q::PauliMoment {
                outputs: &[0],
                word: &z,
            },
            Q::TargetDensity {
                outputs: &[0],
                target: &target,
                target_layout: DensityLayout {
                    outputs: &[1],
                    event_rows: &[],
                },
            },
        ] {
            assert_eq!(p.prepare_query(q, 0, &a).unwrap(), O::Cold);
        }
        let invalid = BitExpr::atom(BitAtom::Record(999));
        assert!(matches!(
            p.prepare_query(
                Q::ReportedProbability {
                    parities: &[invalid]
                },
                0,
                &a
            ),
            Err(InstrumentError::RecordOutOfBounds { .. })
        ));
        assert!(matches!(
            p.prepare_query(
                Q::PauliMoment {
                    outputs: &[0, 0],
                    word: &PauliWord::identity(2).unwrap()
                },
                0,
                &a
            ),
            Err(InstrumentError::Unitary(UnitaryError::RepeatedOutput))
        ));
        for layout in [
            DensityLayout {
                outputs: &[0, 1],
                event_rows: &[],
            },
            DensityLayout {
                outputs: &[0, 0],
                event_rows: &[],
            },
            DensityLayout {
                outputs: &[0],
                event_rows: &[bits(1, 0)],
            },
        ] {
            assert!(
                p.prepare_query(
                    Q::TargetDensity {
                        outputs: &[0],
                        target: &target,
                        target_layout: layout
                    },
                    0,
                    &a
                )
                .is_err()
            );
        }
        assert_eq!(
            p.prepared_probability_statistics().unwrap().cached_queries,
            0
        );
        assert_eq!(p.prepared_density_statistics().unwrap().cached_queries, 0);
        assert_eq!(p.prepared_moment_statistics().unwrap().cached_queries, 0);
        // Conservative structural support has no retained numerical phase family.
        assert_eq!(p.prepare_query(Q::ReportedSupport, 0, &a).unwrap(), O::Cold);
        let support = p
            .reported_support(&[BitExpr::atom(BitAtom::Record(0))], &[], &a)
            .unwrap();
        assert_eq!((support.width(), support.dimension()), (1, 1));
    }
}

#[test]
fn explicit_preparation_preserves_nonzero_term_boundaries_and_selected_policy() {
    let a = ScalarArithmetic::default();
    let source = parse_circuit(
        "H 0\nT 0\nH 0\nT 0\nH 0\nI 1\nI 2",
        &ParseOptions::default(),
    )
    .unwrap();
    let z = PauliWord::new(bits(3, 0), bits(3, 1), 0).unwrap();
    let half = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
        .unwrap();
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for mode in [
            QueryPreparation::Cold,
            QueryPreparation::Trace,
            QueryPreparation::FixedFibers(Default::default()),
        ] {
            let p = compile_affine(
                &source,
                &CompileOptions {
                    compiler_strategy: strategy,
                    moment_preparation: mode,
                    ..Default::default()
                },
                &a,
            )
            .unwrap();
            let q = Q::PauliMoment {
                outputs: &[0, 1, 2],
                word: &z,
            };
            let fixed = matches!(mode, QueryPreparation::FixedFibers(_));
            let cold = matches!(mode, QueryPreparation::Cold);
            let limit = |error| {
                assert!(matches!(
                    error,
                    InstrumentError::Unitary(UnitaryError::Phase(PhaseError::FiberTermLimit {
                        coordinates: 1,
                        quotient_rank: 2,
                        limit: 1
                    }))
                ))
            };
            match p.prepare_query(q, 1, &a) {
                Err(error) => {
                    assert!(fixed);
                    limit(error);
                }
                Ok(outcome) => assert_eq!(outcome, if cold { O::Cold } else { O::Created }),
            }
            assert_eq!(
                p.prepared_moment_statistics().unwrap().cached_queries,
                usize::from(!cold && !fixed)
            );
            assert_eq!(
                p.prepare_query(q, 2, &a).unwrap(),
                if cold {
                    O::Cold
                } else if fixed {
                    O::Created
                } else {
                    O::Reused
                }
            );
            assert_eq!(
                p.pauli_moment(&[0, 1, 2], &z, &[], 2, &a).unwrap().value,
                half
            );
            limit(p.pauli_moment(&[0, 1, 2], &z, &[], 1, &a).unwrap_err());
            if fixed {
                limit(p.prepare_query(q, 1, &a).unwrap_err());
            } else {
                assert_eq!(
                    p.prepare_query(q, 1, &a).unwrap(),
                    if cold { O::Cold } else { O::Reused }
                );
            }
            let stats = p.prepared_moment_statistics().unwrap();
            assert_eq!(
                (stats.cached_queries, stats.fixed_resources),
                (usize::from(!cold), usize::from(fixed))
            );
        }
    }
}
