//! Exact original query-reuse fixtures, using literal fixed-record branch sums.
use super::{bits, original_op, preparations};
use rmagna::{
    clifford::{CliffordCircuit, CliffordGate},
    instrument::*,
    pauli::PauliWord,
    scalar::{ExactScalar, ScalarArithmetic},
    unitary::CompilerStrategy,
};
fn parity(names: &[&str]) -> OriginalRecordParity {
    OriginalRecordParity {
        records: names.iter().map(|s| (*s).into()).collect(),
        inverted: false,
    }
}
fn word(width: usize, x: usize, z: usize) -> PauliWord {
    PauliWord::new(
        bits(width, x),
        bits(width, z),
        (2 * (x & z).count_ones()) as u8,
    )
    .unwrap()
}
fn source(shared: bool, a: &ScalarArithmetic) -> OriginalInstrument {
    use OriginalGate as G;
    let ops = if shared {
        vec![
            original_op(
                "t",
                G::Phase {
                    wires: vec![0],
                    turns: 1,
                },
            ),
            original_op(
                "s",
                G::Phase {
                    wires: vec![1],
                    turns: 2,
                },
            ),
            original_op("ccz", G::Ccz(0, 1, 2)),
            original_op(
                "a",
                G::Measure {
                    factors: vec![(2, OriginalPauli::X)],
                    inverted: false,
                    record: "a".into(),
                },
            ),
            original_op("h", G::H(1)),
            original_op(
                "u",
                G::Phase {
                    wires: vec![1],
                    turns: 1,
                },
            ),
            original_op(
                "b",
                G::Measure {
                    factors: vec![(2, OriginalPauli::Z)],
                    inverted: false,
                    record: "b".into(),
                },
            ),
            original_op("drop", G::Discard(2)),
        ]
    } else {
        vec![
            original_op("ccz", G::Ccz(0, 1, 2)),
            original_op("cx", G::Cx(0, 2)),
            original_op(
                "m",
                G::Destructive {
                    wire: 2,
                    basis: ReadoutBasis::X,
                    record: "r".into(),
                },
            ),
        ]
    };
    OriginalInstrument::new(
        vec![0, 1, 2],
        vec![0, 1],
        CliffordCircuit::new(
            3,
            [CliffordGate::H(0), CliffordGate::H(1), CliffordGate::H(2)],
        )
        .unwrap()
        .evolve(&CliffordCircuit::computational(bits(3, 0)).unwrap(), a)
        .unwrap()
        .unwrap(),
        ops,
        vec![],
        OriginalLimits::default(),
        a,
    )
    .unwrap()
}
#[test]
fn full_shared_effect_event_and_fault_fixture_matches_every_independent_fixed_record_sum() {
    let a = ScalarArithmetic::default();
    let mut cells = 0;
    let mut zeros = 0;
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let instrument = source(true, &a);
        let trace = instrument.compile_trace(compiler, &a).unwrap();
        // Retaining b is represented by an explicit final scalar event row.
        // Every b cell is checked, including contradictory repeated a events.
        let layouts = [
            vec![parity(&["a"])],
            vec![parity(&["a"]), parity(&["a"]), parity(&["b"])],
            vec![parity(&["b"])],
        ];
        let options = OriginalNetworkOptions {
            max_terms: 256,
            ..Default::default()
        };
        let mut plans = vec![];
        for events in &layouts {
            for x in 0..4 {
                for z in 0..4 {
                    plans.push((
                        events,
                        x,
                        z,
                        trace
                            .prepare_network_moment(&word(4, x, z), events, &options, &a)
                            .unwrap(),
                    ));
                }
            }
        }
        let prepared = trace.query_work_statistics();
        assert_eq!(prepared.affine_closures_prepared, 48);
        assert_eq!(prepared.affine_geometry_preparations, 3);
        assert_eq!(prepared.affine_geometry_reuses, 45);
        assert_eq!(prepared.live_affine_geometries, Some(3));
        for changed in [false, true] {
            let faults = if changed {
                vec![OriginalFault {
                    after: "ccz".into(),
                    wire: 0,
                    pauli: OriginalPauli::Y,
                }]
            } else {
                vec![]
            };
            let flips = if changed { vec!["a".into()] } else { vec![] };
            let view = trace
                .with_assignment(
                    &OriginalTraceAssignment {
                        faults: faults.clone(),
                        report_flips: flips.clone(),
                    },
                    &a,
                )
                .unwrap();
            let mut branches = vec![];
            for assignment in 0..4 {
                let true_a = assignment & 1 != 0;
                let true_b = assignment & 2 != 0;
                let branch = instrument
                    .compile(
                        &OriginalAssignment {
                            true_values: vec![("a".into(), true_a), ("b".into(), true_b)],
                            faults: faults.clone(),
                            report_flips: flips.clone(),
                        },
                        compiler,
                        &a,
                    )
                    .unwrap();
                branches.push(([true_a ^ changed, true_b], branch));
            }
            for (events, x, z, plan) in &plans {
                for rhs in 0..1 << events.len() {
                    let mut expected = ExactScalar::zero();
                    for (reports, branch) in &branches {
                        if events.iter().enumerate().all(|(j, e)| {
                            reports[usize::from(e.records[0] == "b")] == (rhs >> j & 1 != 0)
                        }) {
                            expected = a
                                .add(
                                    &expected,
                                    &branch.output_moment(&word(2, *x, *z), &a).unwrap(),
                                )
                                .unwrap();
                        }
                    }
                    let actual = plan
                        .evaluate(&view, &bits(events.len(), rhs), &a)
                        .unwrap()
                        .value;
                    assert_eq!(
                        actual, expected,
                        "shared={changed},x={x},z={z},rhs={rhs},events={events:?}"
                    );
                    zeros += usize::from(actual.is_zero());
                    cells += 1;
                }
            }
        }
        let warm = trace.query_work_statistics();
        assert_eq!(preparations(warm), preparations(prepared));
        assert_eq!(warm.live_affine_geometries, Some(3));
        assert_eq!(warm.cold_evaluations, 0);
        // A fresh source owns an isolated geometry and independently selected
        // resources; check all 48 specifications in the same original order.
        let view = trace.with_assignment(&Default::default(), &a).unwrap();
        for (events, x, z, plan) in &plans {
            let isolated = instrument.compile_trace(compiler, &a).unwrap();
            let fresh = isolated
                .prepare_network_moment(&word(4, *x, *z), events, &options, &a)
                .unwrap();
            let isolated_view = isolated.with_assignment(&Default::default(), &a).unwrap();
            assert_eq!(isolated.query_work_statistics().affine_geometry_reuses, 0);
            assert_eq!(
                plan.resource().quotient_rank(),
                fresh.resource().quotient_rank()
            );
            assert_eq!(plan.resource().num_terms(), fresh.resource().num_terms());
            assert_eq!(
                plan.evaluate(&view, &bits(events.len(), 0), &a)
                    .unwrap()
                    .value,
                fresh
                    .evaluate(&isolated_view, &bits(events.len(), 0), &a)
                    .unwrap()
                    .value
            );
        }
    }
    assert_eq!(cells, 768);
    assert!(zeros > 0);
    println!(
        "768 exact shared-query cells/96 isolated source plans: complete original T,S,CCZ,a,H,T,b,DISCARD fixture, two fault/report frames, every Pauli/RHS/retained-b cell including zero pairings; 48 plans share three actual geometries and perform no hot construction"
    );
}
#[test]
fn original_ccz_destructive_trace_reuses_all_fault_rhs_effects_and_threaded_batches() {
    let a = ScalarArithmetic::default();
    let mut cells = 0;
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let instrument = source(false, &a);
        let trace = instrument.compile_trace(compiler, &a).unwrap();
        let rows = [parity(&["r"])];
        let options = OriginalNetworkOptions {
            max_terms: 256,
            ..Default::default()
        };
        let effects = [(0, 0), (1, 0), (1, 1), (0, 1)];
        let plans = effects
            .iter()
            .map(|&(x, z)| {
                trace
                    .prepare_network_moment(&word(3, x, z), &rows, &options, &a)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let prepared = trace.query_work_statistics();
        assert_eq!(prepared.affine_geometry_preparations, 1);
        assert_eq!(prepared.affine_geometry_reuses, 3);
        for fault in [
            None,
            Some(OriginalPauli::X),
            Some(OriginalPauli::Y),
            Some(OriginalPauli::Z),
        ] {
            let faults = fault
                .map(|pauli| {
                    vec![OriginalFault {
                        after: "ccz".into(),
                        wire: 0,
                        pauli,
                    }]
                })
                .unwrap_or_default();
            let view = trace
                .with_assignment(
                    &OriginalTraceAssignment {
                        faults: faults.clone(),
                        report_flips: vec![],
                    },
                    &a,
                )
                .unwrap();
            for rhs in 0..2 {
                let literal = instrument
                    .compile(
                        &OriginalAssignment {
                            true_values: vec![("r".into(), rhs != 0)],
                            faults: faults.clone(),
                            report_flips: vec![],
                        },
                        compiler,
                        &a,
                    )
                    .unwrap();
                for (&(x, z), plan) in effects.iter().zip(&plans) {
                    let expected = literal.output_moment(&word(2, x, z), &a).unwrap();
                    for _ in 0..2 {
                        assert_eq!(
                            plan.evaluate(&view, &bits(1, rhs), &a).unwrap().value,
                            expected
                        );
                        cells += 1;
                    }
                }
            }
        }
        assert_eq!(
            preparations(trace.query_work_statistics()),
            preparations(prepared)
        );
        let before = trace.query_work_statistics();
        assert!(
            trace
                .prepare_network_moment(&word(3, 4, 0), &rows, &options, &a)
                .is_err()
        );
        assert!(
            plans[0]
                .evaluate(
                    &trace.with_assignment(&Default::default(), &a).unwrap(),
                    &bits(0, 0),
                    &a
                )
                .is_err()
        );
        assert_eq!(
            preparations(trace.query_work_statistics()),
            preparations(before)
        );
        let contradictory = trace
            .prepare_network_probability(&[rows[0].clone(), rows[0].clone()], &options, &a)
            .unwrap();
        assert!(
            contradictory
                .evaluate(
                    &trace.with_assignment(&Default::default(), &a).unwrap(),
                    &bits(2, 2),
                    &a
                )
                .unwrap()
                .value
                .is_zero()
        );
        let no_events = effects
            .iter()
            .map(|&(x, z)| {
                trace
                    .prepare_network_moment(&word(3, x, z), &[], &options, &a)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let assignments = [
            OriginalTraceAssignment::default(),
            OriginalTraceAssignment {
                report_flips: vec!["r".into()],
                ..Default::default()
            },
            OriginalTraceAssignment {
                faults: vec![OriginalFault {
                    after: "ccz".into(),
                    wire: 1,
                    pauli: OriginalPauli::Z,
                }],
                ..Default::default()
            },
        ];
        let expected = assignments
            .iter()
            .map(|assignment| {
                let view = trace.with_assignment(assignment, &a).unwrap();
                no_events
                    .iter()
                    .map(|p| p.evaluate(&view, &bits(0, 0), &a).unwrap().value)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let prepared = trace.query_work_statistics();
        std::thread::scope(|scope| {
            let handles = (0..12)
                .map(|i| {
                    let assignment = &assignments[i % 3];
                    let trace = &trace;
                    let plans = &no_events;
                    let a = &a;
                    scope.spawn(move || {
                        let view = trace.with_assignment(assignment, a).unwrap();
                        plans
                            .iter()
                            .map(|p| p.evaluate(&view, &bits(0, 0), a).unwrap().value)
                            .collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>();
            for (i, handle) in handles.into_iter().enumerate() {
                assert_eq!(handle.join().unwrap(), expected[i % 3]);
            }
        });
        assert_eq!(
            preparations(trace.query_work_statistics()),
            preparations(prepared)
        );
    }
    assert_eq!(cells, 128);
    println!(
        "128 repeated original CCZ/CX/MX_DESTRUCT fault/RHS/effect cells and 24 actual threaded batches match literal branch sums; warm validation and contradictory events remain exact"
    );
}
