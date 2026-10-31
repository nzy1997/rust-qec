//! Finite original static scalar-network contracts (MPL-2.0 upstream inputs).
//! Magna 56847478c840395735146ef1e1ccdc3d8aedc10f,
//! tests/core/test_boundary_sampling.py, including the direct unused-T task.
use rmagna::clifford::{CliffordCircuit, CliffordGate};
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    OriginalBoundaryOptions, OriginalBoundaryTask, OriginalError, OriginalFault, OriginalGate as G,
    OriginalInstrument, OriginalLimits, OriginalNetworkLimits, OriginalNetworkOptions,
    OriginalNetworkResource, OriginalOperation, OriginalPauli as P, OriginalRecordParity as Parity,
    OriginalTraceAssignment, OriginalTraceOptions, OriginalTracePreparationLimits, ReadoutBasis,
};
use rmagna::noise::SplitMix64;
use rmagna::pauli::PauliWord;
use rmagna::phase::NativePhaseOptions;
use rmagna::records::CompletedRecordError;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::{CompilerStrategy, ExpansionLimits, UnitaryError};

fn bits(width: usize, indices: impl IntoIterator<Item = usize>) -> BitVector {
    BitVector::from_indices(width, indices).unwrap()
}
fn op(id: impl Into<String>, gate: G) -> OriginalOperation {
    OriginalOperation {
        id: id.into(),
        gate,
    }
}
fn initial(width: usize, plus: usize) -> AffineKet {
    let zero = CliffordCircuit::computational(bits(width, [])).unwrap();
    CliffordCircuit::new(width, (0..plus).map(CliffordGate::H))
        .unwrap()
        .evolve(&zero, &ScalarArithmetic::default())
        .unwrap()
        .unwrap()
}
fn native(rank: usize) -> OriginalNetworkOptions<'static> {
    let mut resource = NativePhaseOptions::default();
    resource.selection.max_quotient_rank = rank;
    OriginalNetworkOptions {
        resource: OriginalNetworkResource::Native(resource),
        max_terms: 1 << rank,
        ..Default::default()
    }
}
fn strategies() -> [CompilerStrategy; 2] {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
}

#[test]
fn twenty_unused_original_t_subcircuits_trace_before_native_rank_and_terms() {
    let a = ScalarArithmetic::default();
    let n = 20;
    let mut operations = Vec::new();
    for j in 0..n {
        operations.push(op(
            format!("t{j}"),
            G::Phase {
                wires: vec![j as i64],
                turns: 1,
            },
        ));
        operations.push(op(format!("h{j}"), G::H(j as i64)));
        operations.push(op(
            format!("m{j}"),
            G::Destructive {
                wire: j as i64,
                basis: ReadoutBasis::Z,
                record: format!("m{j}"),
            },
        ));
    }
    operations.push(op(
        "fixed",
        G::Destructive {
            wire: n as i64,
            basis: ReadoutBasis::Z,
            record: "fixed".into(),
        },
    ));
    let instrument = OriginalInstrument::new(
        (0..=n as i64).collect(),
        vec![],
        initial(n + 1, n),
        operations,
        vec![],
        OriginalLimits {
            expansion: ExpansionLimits {
                max_side_terms: 1,
                ..Default::default()
            },
            ..Default::default()
        },
        &a,
    )
    .unwrap();
    for strategy in strategies() {
        let trace = instrument.compile_trace(strategy, &a).unwrap();
        assert_eq!(trace.records().len(), 21);
        assert_eq!(trace.network_width(), 21);
        let probability = trace
            .prepare_network_probability(&[], &native(0), &a)
            .unwrap();
        let identity = trace
            .prepare_network_moment(
                &PauliWord::identity(trace.network_width()).unwrap(),
                &[],
                &native(0),
                &a,
            )
            .unwrap();
        let assignment = OriginalTraceAssignment {
            faults: vec![OriginalFault {
                after: "t3".into(),
                wire: 3,
                pauli: P::Y,
            }],
            report_flips: vec!["m4".into(), "fixed".into()],
        };
        let view = trace.with_assignment(&assignment, &a).unwrap();
        let service = trace
            .prepare_boundary_task(
                &OriginalBoundaryTask::default(),
                &OriginalBoundaryOptions {
                    network: native(0),
                    ..Default::default()
                },
                &a,
            )
            .unwrap();
        assert_eq!(
            (
                service.layout().postselection_rank(),
                service.layout().feedback_rank(),
                service.layout().trace_rank()
            ),
            (0, 0, 21)
        );
        assert!(service.layout().compatible());
        assert!(service.required_reports().is_empty());
        let mut rng = SplitMix64::new(2);
        let state = rng.state();
        let draw = service.sample(&view, &mut rng, &a).unwrap();
        assert_eq!(rng.state(), state);
        assert_eq!(draw.mass, ExactScalar::one());
        assert_eq!(draw.acceptance, ExactScalar::one());
        assert!(draw.moments.is_empty());
        assert!(draw.reports.is_empty());
        assert_eq!(draw.final_resources.len(), 1);
        assert!(
            draw.final_resources
                .iter()
                .all(|r| (r.quotient_rank, r.constructive_terms) == (0, 1))
        );
        for query in [
            probability.evaluate(&view, &bits(0, []), &a).unwrap(),
            identity.evaluate(&view, &bits(0, []), &a).unwrap(),
        ] {
            assert_eq!(query.value, ExactScalar::one());
            assert_eq!((query.quotient_rank, query.terms), (0, 1));
        }
        match view.prepare_records_with_limits(
            &[],
            &[],
            &OriginalTraceOptions::default(),
            OriginalTracePreparationLimits {
                max_source_quotient_rank: 0,
                ..Default::default()
            },
            &a,
        ) {
            Err(OriginalError::Unitary(UnitaryError::ExpansionResourceLimit {
                option: "max_source_quotient_rank",
                limit: 0,
                required,
            })) => assert_eq!(required, 20),
            _ => panic!("the explicit side must still reject its twenty-dimensional source"),
        }
        let restored = trace
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        let q = probability.evaluate(&restored, &bits(0, []), &a).unwrap();
        assert_eq!(
            (q.value, q.quotient_rank, q.terms),
            (ExactScalar::one(), 0, 1)
        );
        assert!(matches!(
            probability.evaluate(&view, &bits(1, []), &a),
            Err(OriginalError::DimensionMismatch)
        ));
        assert_eq!(
            probability.evaluate(&view, &bits(0, []), &a).unwrap().value,
            ExactScalar::one()
        );
    }
}

#[test]
fn reported_events_and_record_pauli_signs_replace_assignments() {
    let a = ScalarArithmetic::default();
    let instrument = OriginalInstrument::new(
        vec![0],
        vec![],
        initial(1, 1),
        vec![
            op(
                "t",
                G::Phase {
                    wires: vec![0],
                    turns: 1,
                },
            ),
            op(
                "a",
                G::Destructive {
                    wire: 0,
                    basis: ReadoutBasis::X,
                    record: "a".into(),
                },
            ),
        ],
        vec![],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    let event = Parity {
        records: vec!["a".into()],
        inverted: false,
    };
    let options = OriginalNetworkOptions {
        catalogue: Some(rmagna::catalogue::builtin_phase_catalogue().unwrap()),
        max_terms: 2,
        ..native(2)
    };
    let p0 = a.scalar([2, 1, 0, -1].map(Into::into), -2).unwrap();
    let p1 = a.scalar([2, -1, 0, 1].map(Into::into), -2).unwrap();
    let y = a.scalar([0, -1, 0, 1].map(Into::into), -1).unwrap();
    for strategy in strategies() {
        let trace = instrument.compile_trace(strategy, &a).unwrap();
        let probability = trace
            .prepare_network_probability(&[event.clone(), event.clone()], &options, &a)
            .unwrap();
        let no_catalogue = OriginalNetworkOptions {
            catalogue: None,
            ..options
        };
        match trace.prepare_network_probability(std::slice::from_ref(&event), &no_catalogue, &a) {
            Err(OriginalError::Unitary(error)) => assert!(error.to_string().contains("terms")),
            _ => panic!("strict native selection must not silently substitute fibers"),
        }
        let walsh = trace
            .prepare_network_probability(
                std::slice::from_ref(&event),
                &OriginalNetworkOptions {
                    max_terms: 4,
                    ..no_catalogue
                },
                &a,
            )
            .unwrap();
        let plain = trace
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        let q = walsh.evaluate(&plain, &bits(1, []), &a).unwrap();
        assert_eq!((q.value, q.quotient_rank, q.terms), (p0.clone(), 2, 4));
        let rank_zero = OriginalNetworkOptions {
            catalogue: options.catalogue,
            max_terms: 2,
            ..native(0)
        };
        match trace.prepare_network_probability(std::slice::from_ref(&event), &rank_zero, &a) {
            Err(OriginalError::Unitary(error)) => assert!(
                error
                    .to_string()
                    .contains("phase_selection.max_quotient_rank")
            ),
            _ => panic!("the requested T readout must not pass the rank-zero cap"),
        }
        let z = PauliWord::new(bits(1, []), bits(1, [0]), 0).unwrap();
        let z_event = trace
            .prepare_network_moment(&z, std::slice::from_ref(&event), &options, &a)
            .unwrap();
        let y_word = PauliWord::new(bits(1, [0]), bits(1, [0]), 2).unwrap();
        let y_query = trace
            .prepare_network_moment(&y_word, &[], &options, &a)
            .unwrap();
        for flipped in [false, true, false] {
            let view = trace
                .with_assignment(
                    &OriginalTraceAssignment {
                        report_flips: if flipped { vec!["a".into()] } else { vec![] },
                        ..Default::default()
                    },
                    &a,
                )
                .unwrap();
            for value in [false, true] {
                let expected = if value ^ flipped { &p1 } else { &p0 };
                let q = probability
                    .evaluate(&view, &bits(2, if value { vec![0, 1] } else { vec![] }), &a)
                    .unwrap();
                assert_eq!(&q.value, expected);
                assert_eq!((q.quotient_rank, q.terms), (2, 2));
                let moment = z_event
                    .evaluate(&view, &bits(1, if value { vec![0] } else { vec![] }), &a)
                    .unwrap();
                assert_eq!(
                    moment.value,
                    if value {
                        expected.negated()
                    } else {
                        expected.clone()
                    }
                );
            }
            assert_eq!(
                probability
                    .evaluate(&view, &bits(2, [0]), &a)
                    .unwrap()
                    .value,
                ExactScalar::zero()
            );
            assert_eq!(
                y_query.evaluate(&view, &bits(0, []), &a).unwrap().value,
                if flipped { y.negated() } else { y.clone() }
            );
        }
        let x = PauliWord::new(bits(1, [0]), bits(1, []), 0).unwrap();
        assert!(matches!(
            trace.prepare_network_moment(&x, std::slice::from_ref(&event), &options, &a),
            Err(OriginalError::Completed(
                CompletedRecordError::NonCommutingPauli {
                    kind: "event",
                    index: 0,
                }
            ))
        ));
        let wrong_phase = PauliWord::new(bits(1, [0]), bits(1, [0]), 0).unwrap();
        assert!(matches!(
            trace.prepare_network_moment(&wrong_phase, &[], &options, &a),
            Err(OriginalError::Completed(
                CompletedRecordError::NonHermitianPauli
            ))
        ));
    }
}

#[test]
fn network_admission_limits_and_unknown_names_leave_plans_reusable() {
    let a = ScalarArithmetic::default();
    let instrument = OriginalInstrument::new(
        vec![0],
        vec![],
        initial(1, 1),
        vec![op(
            "a",
            G::Destructive {
                wire: 0,
                basis: ReadoutBasis::Z,
                record: "a".into(),
            },
        )],
        vec![],
        OriginalLimits {
            max_metadata_bytes: 512,
            ..Default::default()
        },
        &a,
    )
    .unwrap();
    let event = Parity {
        records: vec!["a".into()],
        inverted: false,
    };
    for strategy in strategies() {
        let trace = instrument.compile_trace(strategy, &a).unwrap();
        let plan = trace
            .prepare_network_probability(std::slice::from_ref(&event), &native(0), &a)
            .unwrap();
        let words = usize::try_from(plan.geometry_words()).unwrap();
        let base = native(0);
        let cases = [
            (
                OriginalNetworkOptions {
                    max_record_occurrences: 0,
                    ..base
                },
                "max_record_occurrences",
                1,
            ),
            (
                OriginalNetworkOptions {
                    limits: OriginalNetworkLimits {
                        max_event_rows: 0,
                        ..base.limits
                    },
                    ..base
                },
                "max_event_rows",
                1,
            ),
            (
                OriginalNetworkOptions {
                    limits: OriginalNetworkLimits {
                        max_joint_paths: 1,
                        ..base.limits
                    },
                    ..base
                },
                "max_network_joint_paths",
                2,
            ),
            (
                OriginalNetworkOptions {
                    limits: OriginalNetworkLimits {
                        max_geometry_words: words - 1,
                        ..base.limits
                    },
                    ..base
                },
                "max_network_geometry_words",
                words as u128,
            ),
        ];
        let view = trace
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        let half = a.scalar([1, 0, 0, 0].map(Into::into), -1).unwrap();
        for (options, expected_option, expected_required) in cases {
            match trace.prepare_network_probability(std::slice::from_ref(&event), &options, &a) {
                Err(OriginalError::ResourceLimit {
                    option, required, ..
                }) => {
                    assert_eq!(option, expected_option);
                    assert_eq!(required, expected_required);
                }
                _ => panic!("each original network admission must fail with its own typed quota"),
            }
            assert_eq!(plan.evaluate(&view, &bits(1, []), &a).unwrap().value, half);
        }
        let exact = OriginalNetworkOptions {
            limits: OriginalNetworkLimits {
                max_geometry_words: words,
                ..base.limits
            },
            ..base
        };
        let admitted = trace
            .prepare_network_probability(std::slice::from_ref(&event), &exact, &a)
            .unwrap();
        assert_eq!(admitted.geometry_words(), words as u128);
        assert_eq!(
            admitted.evaluate(&view, &bits(1, [0]), &a).unwrap().value,
            half
        );
        let oversized = Parity {
            records: vec!["x".repeat(513)],
            inverted: false,
        };
        assert!(matches!(
            trace.prepare_network_probability(&[oversized], &base, &a),
            Err(OriginalError::ResourceLimit {
                option: "max_metadata_bytes",
                limit: 512,
                ..
            })
        ));
        let unknown = Parity {
            records: vec!["unknown".into()],
            inverted: false,
        };
        assert!(matches!(
            trace.prepare_network_probability(&[unknown], &base, &a),
            Err(OriginalError::Metadata {
                kind: "unknown record parity",
                ..
            })
        ));
        let too_many = Parity {
            records: vec!["a".into(); 3],
            inverted: false,
        };
        assert!(matches!(
            trace.prepare_network_probability(
                &[too_many],
                &OriginalNetworkOptions {
                    max_record_occurrences: 2,
                    ..base
                },
                &a
            ),
            Err(OriginalError::ResourceLimit {
                option: "max_record_occurrences",
                required: 3,
                ..
            })
        ));
        assert!(matches!(
            plan.evaluate(&view, &bits(2, []), &a),
            Err(OriginalError::DimensionMismatch)
        ));
        assert_eq!(plan.evaluate(&view, &bits(1, [0]), &a).unwrap().value, half);
    }
}

#[test]
fn vacuum_constant_events_and_foreign_programs_are_exact() {
    let a = ScalarArithmetic::default();
    let instrument = OriginalInstrument::new(
        vec![],
        vec![],
        initial(0, 0),
        vec![],
        vec![],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    for strategy in strategies() {
        let trace = instrument.compile_trace(strategy, &a).unwrap();
        let view = trace
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        let events = [
            Parity::default(),
            Parity {
                inverted: true,
                ..Default::default()
            },
        ];
        let probability = trace
            .prepare_network_probability(&events, &native(0), &a)
            .unwrap();
        let moment = trace
            .prepare_network_moment(&PauliWord::identity(0).unwrap(), &events, &native(0), &a)
            .unwrap();
        for (indices, expected) in [
            (vec![1], ExactScalar::one()),
            (vec![], ExactScalar::zero()),
            (vec![0, 1], ExactScalar::zero()),
            (vec![0], ExactScalar::zero()),
        ] {
            let rhs = bits(2, indices);
            assert_eq!(
                probability.evaluate(&view, &rhs, &a).unwrap().value,
                expected
            );
            assert_eq!(moment.evaluate(&view, &rhs, &a).unwrap().value, expected);
        }
        let zero_terms = OriginalNetworkOptions {
            max_terms: 0,
            ..native(0)
        };
        match trace.prepare_network_probability(&[], &zero_terms, &a) {
            Err(OriginalError::Unitary(error)) => assert!(error.to_string().contains("terms")),
            _ => panic!("a nonzero vacuum resource still requires one admitted term"),
        }
        let other = instrument.compile_trace(strategy, &a).unwrap();
        let foreign = other
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        assert!(matches!(
            probability.evaluate(&foreign, &bits(2, [1]), &a),
            Err(OriginalError::Unitary(UnitaryError::IncompatibleProgram))
        ));
        assert_eq!(
            probability
                .evaluate(&view, &bits(2, [1]), &a)
                .unwrap()
                .value,
            ExactScalar::one()
        );
        assert!(matches!(
            trace.prepare_network_moment(&PauliWord::identity(1).unwrap(), &[], &native(0), &a),
            Err(OriginalError::DimensionMismatch)
        ));
    }
}

#[test]
fn inverted_record_y_and_z_factors_preserve_complex_correlations_and_faults() {
    let a = ScalarArithmetic::default();
    let state = CliffordCircuit::new(
        1,
        [CliffordGate::H(0), CliffordGate::S { qubit: 0, turns: 1 }],
    )
    .unwrap()
    .evolve(&initial(1, 0), &a)
    .unwrap()
    .unwrap();
    let instrument = OriginalInstrument::new(
        vec![0],
        vec![0],
        state,
        vec![op(
            "negative",
            G::Measure {
                factors: vec![(0, P::Z)],
                inverted: true,
                record: "a".into(),
            },
        )],
        vec![],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    for strategy in strategies() {
        let trace = instrument.compile_trace(strategy, &a).unwrap();
        assert_eq!(trace.network_width(), 2);
        // The physical state is (|00> + i|11>)/sqrt(2): both XY and ZZ are +1.
        // The originally inverted reported record conjugates each by record X.
        let xy = PauliWord::new(bits(2, [0, 1]), bits(2, [1]), 2).unwrap();
        let zz = PauliWord::new(bits(2, []), bits(2, [0, 1]), 0).unwrap();
        let xy_query = trace
            .prepare_network_moment(&xy, &[], &native(0), &a)
            .unwrap();
        let zz_query = trace
            .prepare_network_moment(&zz, &[], &native(0), &a)
            .unwrap();
        for (flipped, faulted) in [(false, false), (true, false), (true, true), (false, false)] {
            let view = trace
                .with_assignment(
                    &OriginalTraceAssignment {
                        report_flips: if flipped { vec!["a".into()] } else { vec![] },
                        faults: if faulted {
                            vec![OriginalFault {
                                after: "negative".into(),
                                wire: 0,
                                pauli: P::Z,
                            }]
                        } else {
                            vec![]
                        },
                    },
                    &a,
                )
                .unwrap();
            let xy = xy_query.evaluate(&view, &bits(0, []), &a).unwrap();
            let zz = zz_query.evaluate(&view, &bits(0, []), &a).unwrap();
            assert_eq!(
                xy.value,
                if flipped ^ faulted {
                    ExactScalar::one()
                } else {
                    ExactScalar::one().negated()
                }
            );
            assert_eq!(
                zz.value,
                if flipped {
                    ExactScalar::one()
                } else {
                    ExactScalar::one().negated()
                }
            );
            assert_eq!((xy.quotient_rank, xy.terms), (0, 1));
            assert_eq!((zz.quotient_rank, zz.terms), (0, 1));
        }
        let unknown_fault = OriginalTraceAssignment {
            faults: vec![OriginalFault {
                after: "missing".into(),
                wire: 0,
                pauli: P::Y,
            }],
            ..Default::default()
        };
        assert!(matches!(
            trace.with_assignment(&unknown_fault, &a),
            Err(OriginalError::Metadata {
                kind: "unknown fault anchor",
                ..
            })
        ));
        let restored = trace
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        assert_eq!(
            xy_query
                .evaluate(&restored, &bits(0, []), &a)
                .unwrap()
                .value,
            ExactScalar::one().negated()
        );
    }
}

#[test]
fn packed_output_order_and_wide_physical_equality_storage_are_admitted_honestly() {
    let a = ScalarArithmetic::default();
    for width in [65, 129] {
        let wires = (0..width as i64).collect::<Vec<_>>();
        let instrument = OriginalInstrument::new(
            wires.clone(),
            wires.into_iter().rev().collect(),
            CliffordCircuit::computational(bits(width, [0])).unwrap(),
            vec![],
            vec![],
            OriginalLimits::default(),
            &a,
        )
        .unwrap();
        for strategy in strategies() {
            let trace = instrument.compile_trace(strategy, &a).unwrap();
            let view = trace
                .with_assignment(&OriginalTraceAssignment::default(), &a)
                .unwrap();
            let probability = trace
                .prepare_network_probability(&[], &native(0), &a)
                .unwrap();
            assert_eq!(
                probability.evaluate(&view, &bits(0, []), &a).unwrap().value,
                ExactScalar::one()
            );
            let words = usize::try_from(probability.geometry_words()).unwrap();
            for (index, expected) in [
                (0, ExactScalar::one()),
                (width - 1, ExactScalar::one().negated()),
            ] {
                let z = PauliWord::new(bits(width, []), bits(width, [index]), 0).unwrap();
                let moment = trace
                    .prepare_network_moment(&z, &[], &native(0), &a)
                    .unwrap();
                assert_eq!(
                    moment.evaluate(&view, &bits(0, []), &a).unwrap().value,
                    expected
                );
            }
            let base = native(0);
            let refused = OriginalNetworkOptions {
                limits: OriginalNetworkLimits {
                    max_geometry_words: words - 1,
                    ..base.limits
                },
                ..base
            };
            assert!(matches!(
                trace.prepare_network_probability(&[], &refused, &a),
                Err(OriginalError::ResourceLimit {
                    option: "max_network_geometry_words",
                    ..
                })
            ));
            let recovered = OriginalNetworkOptions {
                limits: OriginalNetworkLimits {
                    max_geometry_words: words,
                    ..base.limits
                },
                ..base
            };
            assert_eq!(
                trace
                    .prepare_network_probability(&[], &recovered, &a)
                    .unwrap()
                    .evaluate(&view, &bits(0, []), &a)
                    .unwrap()
                    .value,
                ExactScalar::one()
            );
        }
    }
    // This reference is valid and has no path variables. The new scalar closure
    // still needs quadratic physical-equality storage, so its default quota must
    // refuse before the existing packed-row factory allocates those matrices.
    let width = 4097;
    let wires = (0..width as i64).collect::<Vec<_>>();
    let instrument = OriginalInstrument::new(
        wires.clone(),
        wires,
        initial(width, 0),
        vec![],
        vec![],
        OriginalLimits {
            expansion: ExpansionLimits {
                max_geometry_words: usize::MAX,
                ..Default::default()
            },
            ..Default::default()
        },
        &a,
    )
    .unwrap();
    for strategy in strategies() {
        let trace = instrument.compile_trace(strategy, &a).unwrap();
        assert!(
            matches!(trace.prepare_network_probability(&[], &native(0), &a),
            Err(OriginalError::ResourceLimit { option: "max_network_geometry_words", required, .. })
                if required > (1 << 24))
        );
    }
}
