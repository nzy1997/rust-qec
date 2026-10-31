//! Original BoundarySampling finite static contracts at pinned56847478.
use rmagna::clifford::{CliffordCircuit, CliffordGate};
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    OriginalBoundaryLimits as Limits, OriginalBoundaryOptions as Options,
    OriginalBoundaryQuery as Query, OriginalBoundaryTask as Task, OriginalError, OriginalGate as G,
    OriginalInstrument, OriginalLimits, OriginalNetworkOptions, OriginalNetworkResource,
    OriginalOperation, OriginalRecordEvent as Event, OriginalRecordLayout as Layout,
    OriginalRecordParity as Parity, OriginalTraceAssignment, ReadoutBasis,
};
use rmagna::noise::RandomSource;
use rmagna::pauli::PauliWord;
use rmagna::phase::NativePhaseOptions;
use rmagna::records::CompletedRecordError;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::unitary::{CompilerStrategy, UnitaryError};
use std::collections::BTreeSet;

fn bits(n: usize, ones: impl IntoIterator<Item = usize>) -> BitVector {
    BitVector::from_indices(n, ones).unwrap()
}
fn event(names: &[&str], inverted: bool, value: bool) -> Event {
    Event {
        parity: Parity {
            records: names.iter().map(|s| s.to_string()).collect(),
            inverted,
        },
        value,
    }
}
fn op(id: impl Into<String>, gate: G) -> OriginalOperation {
    OriginalOperation {
        id: id.into(),
        gate,
    }
}
fn strategies() -> [CompilerStrategy; 2] {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
}
fn native(rank: usize, terms: usize) -> Options<'static> {
    let mut resource = NativePhaseOptions::default();
    resource.selection.max_quotient_rank = rank;
    Options {
        network: OriginalNetworkOptions {
            resource: OriginalNetworkResource::Native(resource),
            catalogue: Some(rmagna::catalogue::builtin_phase_catalogue().unwrap()),
            max_terms: terms,
            ..Default::default()
        },
        ..Default::default()
    }
}
struct NeverRandom;
impl RandomSource for NeverRandom {
    fn next_u64(&mut self) -> u64 {
        panic!("static direct service read RNG")
    }
}

#[test]
fn original_virtual_record_coordinates_and_redundant_postselection() {
    let names = ["a", "b", "c", "unused"].map(str::to_string);
    let events = [
        event(&["a", "b", "c"], false, false),
        event(&["b", "c", "a"], false, false),
    ];
    let layout = Layout::new(&names, &events, Limits::default()).unwrap();
    assert_eq!(
        (
            layout.postselection_rank(),
            layout.feedback_rank(),
            layout.trace_rank()
        ),
        (1, 0, 3)
    );
    assert!(layout.compatible());
    assert_eq!(layout.postselection_values().unwrap(), &bits(1, []));
    let mut images = BTreeSet::new();
    for word in 0..16 {
        let x = bits(4, (0..4).filter(|i| word & (1 << i) != 0));
        let y = layout.to_virtual(&x).unwrap();
        assert_eq!(layout.to_records(&y).unwrap(), x);
        images.insert(y.words().to_vec());
    }
    assert_eq!(images.len(), 16);
    let a = ScalarArithmetic::default();
    let ir = OriginalInstrument::new(
        vec![0, 1, 2, 3],
        vec![],
        CliffordCircuit::computational(bits(4, [])).unwrap(),
        names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                op(
                    format!("m{i}"),
                    G::Destructive {
                        wire: i as i64,
                        basis: ReadoutBasis::Z,
                        record: name.clone(),
                    },
                )
            })
            .collect(),
        vec![],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    for strategy in strategies() {
        let trace = ir.compile_trace(strategy, &a).unwrap();
        let task = Task {
            postselection: events.to_vec(),
            ..Default::default()
        };
        let service = trace
            .prepare_boundary_task(&task, &native(0, 1), &a)
            .unwrap();
        let layout = service.layout();
        assert_eq!(
            (
                layout.postselection_rank(),
                layout.feedback_rank(),
                layout.trace_rank()
            ),
            (1, 0, 3)
        );
        assert!(layout.compatible());
        let mut images = BTreeSet::new();
        for word in 0..16 {
            let x = bits(4, (0..4).filter(|i| word & (1 << i) != 0));
            let y = layout.to_virtual(&x).unwrap();
            assert_eq!(layout.to_records(&y).unwrap(), x);
            images.insert(y.words().to_vec());
        }
        assert_eq!(images.len(), 16);
        let view = trace
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        assert_eq!(
            service
                .sample(&view, &mut NeverRandom, &a)
                .unwrap()
                .acceptance,
            ExactScalar::one()
        );
    }
    let bad = Layout::new(
        &["a".into(), "b".into()],
        &[
            event(&["a", "b"], false, false),
            event(&["b", "a"], false, true),
        ],
        Limits::default(),
    )
    .unwrap();
    assert!(!bad.compatible());
    assert!(bad.postselection_values().is_none());
    assert_eq!(
        (
            bad.postselection_rank(),
            bad.feedback_rank(),
            bad.trace_rank()
        ),
        (1, 0, 1)
    );
}

#[test]
fn virtual_offsets_duplicate_names_and_packed_inverses_are_exact() {
    let affine = Layout::new(
        &["a", "b", "c", "unused"].map(str::to_string),
        &[
            event(&["a", "b"], true, false),
            event(&["b", "c"], false, true),
            event(&["c", "a"], true, true),
        ],
        Limits::default(),
    )
    .unwrap();
    assert_eq!(affine.postselection_rank(), 2);
    assert_eq!(affine.postselection_values().unwrap(), &bits(2, [1]));
    for word in 0..16 {
        let x = bits(4, (0..4).filter(|i| word & (1 << i) != 0));
        let y = affine.to_virtual(&x).unwrap();
        let (a, b, c) = (x.get(0).unwrap(), x.get(1).unwrap(), x.get(2).unwrap());
        assert_eq!(
            !y.get(0).unwrap() && y.get(1).unwrap(),
            (a ^ b) && (b ^ c) && !(a ^ c)
        );
        assert_eq!(affine.to_records(&y).unwrap(), x);
    }
    for n in [0, 1, 4, 65, 129] {
        let names = (0..n).map(|i| format!("r{i}")).collect::<Vec<_>>();
        let events = if n == 0 {
            vec![event(&[], true, true)]
        } else {
            vec![
                event(&["r0", "r0", "r0"], true, false),
                event(&["r0"], false, true),
            ]
        };
        let layout = Layout::new(&names, &events, Limits::default()).unwrap();
        assert!(layout.compatible());
        assert_eq!(layout.postselection_rank(), usize::from(n != 0));
        if n != 0 {
            assert_eq!(layout.postselection_values().unwrap(), &bits(1, [0]));
        }
        for pattern in [0, 1, 2] {
            let x = bits(n, (0..n).filter(|j| (j + pattern) % 3 == 0));
            assert_eq!(
                layout.to_records(&layout.to_virtual(&x).unwrap()).unwrap(),
                x
            );
        }
        assert!(matches!(
            layout.to_virtual(&bits(n + 1, [])),
            Err(OriginalError::DimensionMismatch)
        ));
        assert!(matches!(
            layout.to_records(&bits(n + 1, [])),
            Err(OriginalError::DimensionMismatch)
        ));
    }
    assert!(matches!(
        Layout::new(&["a".into(), "a".into()], &[], Limits::default()),
        Err(OriginalError::Metadata {
            kind: "duplicate boundary record",
            ..
        })
    ));
    assert!(matches!(
        Layout::new(
            &["a".into()],
            &[event(&["absent"], false, false)],
            Limits::default()
        ),
        Err(OriginalError::Metadata {
            kind: "unknown boundary record",
            ..
        })
    ));
}

#[test]
fn original_scalar_residual_and_zero_postselection_use_actual_final_resources() {
    let a = ScalarArithmetic::default();
    let zero = CliffordCircuit::computational(bits(1, [])).unwrap();
    let plus = CliffordCircuit::new(1, [CliffordGate::H(0)])
        .unwrap()
        .evolve(&zero, &a)
        .unwrap()
        .unwrap();
    let instrument = OriginalInstrument::new(
        vec![0],
        vec![],
        plus,
        vec![
            op(
                "t",
                G::Phase {
                    wires: vec![0],
                    turns: 1,
                },
            ),
            op(
                "m",
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
    let task = Task {
        postselection: vec![event(&["a"], false, false)],
        ..Default::default()
    };
    let expected = a.scalar([2, 1, 0, -1].map(Into::into), -2).unwrap();
    let other = a.scalar([2, -1, 0, 1].map(Into::into), -2).unwrap();
    for strategy in strategies() {
        let trace = instrument.compile_trace(strategy, &a).unwrap();
        let service = trace
            .prepare_boundary_task(&task, &native(2, 2), &a)
            .unwrap();
        assert_eq!(
            (
                service.layout().postselection_rank(),
                service.layout().feedback_rank(),
                service.layout().trace_rank()
            ),
            (1, 0, 0)
        );
        assert!(service.required_reports().is_empty());
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
            let draw = service.sample(&view, &mut NeverRandom, &a).unwrap();
            assert_eq!(draw.mass, ExactScalar::one());
            assert_eq!(
                draw.acceptance,
                if flipped {
                    other.clone()
                } else {
                    expected.clone()
                }
            );
            assert!(draw.reports.is_empty());
            assert!(draw.moments.is_empty());
            assert_eq!(draw.final_resources.len(), 2);
            let receipt = &draw.final_resources[1];
            assert_eq!(receipt.query, Query::Acceptance);
            assert_eq!((receipt.quotient_rank, receipt.constructive_terms), (2, 2));
        }
        let bad = Task {
            postselection: vec![event(&["a"], false, false), event(&["a"], false, true)],
            ..Default::default()
        };
        let impossible = trace
            .prepare_boundary_task(&bad, &native(2, 2), &a)
            .unwrap();
        let view = trace
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        let draw = impossible.sample(&view, &mut NeverRandom, &a).unwrap();
        assert!(!impossible.layout().compatible());
        assert_eq!(draw.mass, ExactScalar::one());
        assert_eq!(draw.acceptance, ExactScalar::zero());
        assert_eq!(draw.final_resources.len(), 1);
        assert_eq!(draw.final_resources[0].query, Query::Mass);
        match trace.prepare_boundary_task(&task, &native(0, 2), &a) {
            Err(OriginalError::Unitary(e)) => assert!(e.to_string().contains("max_quotient_rank")),
            _ => panic!("postselected T readout must reject native rank zero"),
        }
        let mut no_database = native(2, 2);
        no_database.network.catalogue = None;
        assert!(matches!(
            trace.prepare_boundary_task(&task, &no_database, &a),
            Err(OriginalError::Unitary(_))
        ));
        assert_eq!(service.evaluate(&view, &a).unwrap().acceptance, expected);
    }
}

fn bounded_instrument(a: &ScalarArithmetic) -> OriginalInstrument {
    let initial = CliffordCircuit::new(4, [CliffordGate::H(3)])
        .unwrap()
        .evolve(&CliffordCircuit::computational(bits(4, [])).unwrap(), a)
        .unwrap()
        .unwrap();
    OriginalInstrument::new(
        vec![0, 1, 2, 3],
        vec![3],
        initial,
        (0..3)
            .map(|i| {
                op(
                    format!("m{i}"),
                    G::Destructive {
                        wire: i,
                        basis: ReadoutBasis::Z,
                        record: format!("r{i}"),
                    },
                )
            })
            .collect(),
        vec![],
        OriginalLimits::default(),
        a,
    )
    .unwrap()
}
fn bounded_task() -> Task {
    Task {
        postselection: vec![
            event(&["r0", "r1"], false, false),
            event(&["r1", "r0"], false, false),
        ],
        observables: vec![
            PauliWord::new(bits(1, [0]), bits(1, []), 0).unwrap(),
            PauliWord::new(bits(1, [0]), bits(1, []), 4).unwrap(),
        ],
    }
}

#[test]
fn boundary_counts_and_storage_limits_refuse_then_recover() {
    let a = ScalarArithmetic::default();
    let task = bounded_task();
    let options = Options::default();
    for strategy in strategies() {
        let trace = bounded_instrument(&a).compile_trace(strategy, &a).unwrap();
        let good = trace.prepare_boundary_task(&task, &options, &a).unwrap();
        let cases = [
            (
                Limits {
                    max_records: 2,
                    ..options.limits
                },
                "max_boundary_records",
                3,
            ),
            (
                Limits {
                    max_postselection_rows: 1,
                    ..options.limits
                },
                "max_boundary_postselection_rows",
                2,
            ),
            (
                Limits {
                    max_record_occurrences: 3,
                    ..options.limits
                },
                "max_boundary_record_occurrences",
                4,
            ),
            (
                Limits {
                    max_observables: 1,
                    ..options.limits
                },
                "max_boundary_observables",
                2,
            ),
            (
                Limits {
                    max_queries: 3,
                    ..options.limits
                },
                "max_boundary_queries",
                4,
            ),
            (
                Limits {
                    max_layout_words: good.layout().geometry_words() as usize - 1,
                    ..options.limits
                },
                "max_boundary_layout_words",
                good.layout().geometry_words(),
            ),
            (
                Limits {
                    max_task_words: good.geometry_words() as usize - 1,
                    ..options.limits
                },
                "max_boundary_task_words",
                good.geometry_words(),
            ),
        ];
        for (bad, name, needed) in cases {
            match trace.prepare_boundary_task(
                &task,
                &Options {
                    limits: bad,
                    ..options
                },
                &a,
            ) {
                Err(OriginalError::ResourceLimit {
                    option, required, ..
                }) => {
                    assert_eq!(option, name);
                    assert_eq!(required, needed);
                }
                _ => panic!("boundary quota {name} failed to refuse"),
            }
        }
        let exact = Limits {
            max_records: 3,
            max_postselection_rows: 2,
            max_record_occurrences: 4,
            max_observables: 2,
            max_queries: 4,
            max_layout_words: good.layout().geometry_words() as usize,
            max_task_words: good.geometry_words() as usize,
            ..options.limits
        };
        let plan = trace
            .prepare_boundary_task(
                &task,
                &Options {
                    limits: exact,
                    ..options
                },
                &a,
            )
            .unwrap();
        let view = trace
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        let draw = plan.sample(&view, &mut NeverRandom, &a).unwrap();
        assert_eq!(draw.mass, ExactScalar::one());
        assert_eq!(draw.acceptance, ExactScalar::one());
        assert_eq!(
            draw.moments,
            vec![ExactScalar::one(), ExactScalar::one().negated()]
        );
        assert_eq!(draw.final_resources.len(), 4);
        assert_eq!(
            draw.final_resources
                .iter()
                .map(|r| r.query)
                .collect::<Vec<_>>(),
            [
                Query::Mass,
                Query::Acceptance,
                Query::Moment(0),
                Query::Moment(1)
            ]
        );
        let mut bytes = 0;
        loop {
            match trace.prepare_boundary_task(
                &task,
                &Options {
                    limits: Limits {
                        max_metadata_bytes: bytes,
                        ..exact
                    },
                    ..options
                },
                &a,
            ) {
                Err(OriginalError::ResourceLimit {
                    option, required, ..
                }) => {
                    assert!(
                        ["max_boundary_metadata_bytes", "max_metadata_bytes"].contains(&option)
                    );
                    assert!(required > bytes as u128);
                    bytes = usize::try_from(required).unwrap();
                }
                Ok(_) => break,
                _ => panic!("unexpected metadata admission error"),
            }
        }
        assert!(matches!(
            trace.prepare_boundary_task(
                &task,
                &Options {
                    limits: Limits {
                        max_metadata_bytes: bytes - 1,
                        ..exact
                    },
                    ..options
                },
                &a
            ),
            Err(OriginalError::ResourceLimit { .. })
        ));
        let unknown = Task {
            postselection: vec![event(&[&"missing".repeat(1024)], false, false)],
            ..Default::default()
        };
        assert!(matches!(
            trace.prepare_boundary_task(
                &unknown,
                &Options {
                    limits: Limits {
                        max_metadata_bytes: 1024,
                        ..options.limits
                    },
                    ..options
                },
                &a
            ),
            Err(OriginalError::ResourceLimit {
                option: "max_boundary_metadata_bytes",
                ..
            })
        ));
        assert_eq!(good.evaluate(&view, &a).unwrap().moments, draw.moments);
    }
}

#[test]
fn packed_record_tasks_preserve_offsets_flips_and_virtual_roles() {
    let a = ScalarArithmetic::default();
    for n in [65, 129] {
        let input = CliffordCircuit::computational(bits(n, [0, 64, n - 1])).unwrap();
        let ir = OriginalInstrument::new(
            (0..n as i64).collect(),
            vec![],
            input,
            (0..n)
                .map(|i| {
                    op(
                        format!("m{i}"),
                        G::Destructive {
                            wire: i as i64,
                            basis: ReadoutBasis::Z,
                            record: format!("r{i}"),
                        },
                    )
                })
                .collect(),
            vec![],
            OriginalLimits::default(),
            &a,
        )
        .unwrap();
        let task = Task {
            postselection: vec![event(&["r0", "r64"], true, false)],
            ..Default::default()
        };
        for strategy in strategies() {
            let trace = ir.compile_trace(strategy, &a).unwrap();
            let plan = trace
                .prepare_boundary_task(&task, &native(0, 1), &a)
                .unwrap();
            assert_eq!(
                (
                    plan.layout().postselection_rank(),
                    plan.layout().feedback_rank(),
                    plan.layout().trace_rank()
                ),
                (1, 0, n - 1)
            );
            for flipped in [false, true, false] {
                let view = trace
                    .with_assignment(
                        &OriginalTraceAssignment {
                            report_flips: if flipped { vec!["r64".into()] } else { vec![] },
                            ..Default::default()
                        },
                        &a,
                    )
                    .unwrap();
                let draw = plan.sample(&view, &mut NeverRandom, &a).unwrap();
                assert_eq!(draw.mass, ExactScalar::one());
                assert_eq!(
                    draw.acceptance,
                    if flipped {
                        ExactScalar::one()
                    } else {
                        ExactScalar::zero()
                    }
                );
                assert_eq!(
                    draw.final_resources
                        .iter()
                        .map(|r| (r.query, r.quotient_rank, r.constructive_terms))
                        .collect::<Vec<_>>(),
                    [
                        (Query::Mass, 0, 1),
                        (Query::Acceptance, 0, usize::from(flipped))
                    ]
                );
            }
        }
    }
}

#[test]
fn vacuum_impossible_events_and_foreign_views_do_not_consume_randomness() {
    let a = ScalarArithmetic::default();
    let ir = OriginalInstrument::new(
        vec![],
        vec![],
        CliffordCircuit::computational(bits(0, [])).unwrap(),
        vec![],
        vec![],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    for strategy in strategies() {
        let trace = ir.compile_trace(strategy, &a).unwrap();
        let foreign = ir.compile_trace(strategy, &a).unwrap();
        let view = trace
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        for compatible in [false, true] {
            let task = Task {
                postselection: vec![event(&[], true, compatible)],
                observables: vec![PauliWord::new(bits(0, []), bits(0, []), 4).unwrap()],
            };
            let plan = trace
                .prepare_boundary_task(&task, &native(0, 1), &a)
                .unwrap();
            let draw = plan.sample(&view, &mut NeverRandom, &a).unwrap();
            assert_eq!(plan.layout().compatible(), compatible);
            assert_eq!(draw.mass, ExactScalar::one());
            assert_eq!(
                draw.acceptance,
                if compatible {
                    ExactScalar::one()
                } else {
                    ExactScalar::zero()
                }
            );
            assert_eq!(
                draw.moments,
                vec![if compatible {
                    ExactScalar::one().negated()
                } else {
                    ExactScalar::zero()
                }]
            );
            let wrong = foreign
                .with_assignment(&OriginalTraceAssignment::default(), &a)
                .unwrap();
            assert!(matches!(
                plan.sample(&wrong, &mut NeverRandom, &a),
                Err(OriginalError::Unitary(UnitaryError::IncompatibleProgram))
            ));
            assert_eq!(plan.evaluate(&view, &a).unwrap().moments, draw.moments);
        }
        let bad = Task {
            postselection: vec![event(&[], false, true)],
            observables: vec![PauliWord::new(bits(0, []), bits(0, []), 2).unwrap()],
        };
        assert!(matches!(
            trace.prepare_boundary_task(&bad, &Options::default(), &a),
            Err(OriginalError::Completed(
                CompletedRecordError::NonHermitianPauli
            ))
        ));
        let bad_width = Task {
            observables: vec![PauliWord::identity(1).unwrap()],
            ..Default::default()
        };
        assert!(matches!(
            trace.prepare_boundary_task(&bad_width, &Options::default(), &a),
            Err(OriginalError::DimensionMismatch)
        ));
    }
}
