// SPDX-License-Identifier: MPL-2.0
// Complete itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f method
// tests/core/test_deferred_feedback.py::DeferredFeedbackTests::test_native_wide_trace_and_explicit_resource_failure.
// Supplemental small exact expectations are analytical, independent of network
// evaluation. No full-side expansion or fallback can establish this contract.
use rmagna::catalogue::{PhaseSelectionOptions, builtin_phase_catalogue};
use rmagna::clifford::{CliffordCircuit, CliffordGate};
use rmagna::gf2::BitVector;
use rmagna::instrument::*;
use rmagna::pauli::PauliWord;
use rmagna::phase::NativePhaseOptions;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::unitary::CompilerStrategy;
fn bits(n: usize, indices: &[usize]) -> BitVector {
    BitVector::from_indices(n, indices.iter().copied()).unwrap()
}
fn word(n: usize, x: &[usize], z: &[usize]) -> PauliWord {
    PauliWord::new(
        bits(n, x),
        bits(n, z),
        (2 * x.iter().filter(|i| z.contains(i)).count() % 8) as u8,
    )
    .unwrap()
}
fn op(id: &str, gate: OriginalGate) -> OriginalOperation {
    OriginalOperation {
        id: id.into(),
        gate,
    }
}
fn native<'a>(
    catalogue: Option<&'a rmagna::catalogue::PhaseCatalogue>,
    rank: usize,
    terms: usize,
) -> OriginalNetworkOptions<'a> {
    OriginalNetworkOptions {
        resource: OriginalNetworkResource::Native(NativePhaseOptions {
            selection: PhaseSelectionOptions {
                max_quotient_rank: rank,
                ..Default::default()
            },
            ..Default::default()
        }),
        catalogue,
        max_terms: terms,
        ..Default::default()
    }
}
fn compilers() -> [CompilerStrategy; 2] {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
}
#[test]
fn complete_native_wide_trace_and_explicit_resource_failure() {
    let a = ScalarArithmetic::default();
    let catalogue = builtin_phase_catalogue().unwrap();
    let options = native(Some(catalogue), 8, 256);
    let half = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
        .unwrap();
    let isqrt = a.omega(0, -1).unwrap();
    let expected = a.multiply(&half, &isqrt).unwrap();
    let n = 129;
    let initial = CliffordCircuit::new(n, (0..n).map(CliffordGate::H))
        .unwrap()
        .evolve(&CliffordCircuit::computational(bits(n, &[])).unwrap(), &a)
        .unwrap()
        .unwrap();
    let ir = OriginalInstrument::new(
        (0..n as i64).collect(),
        (0..n as i64).collect(),
        initial,
        (0..n)
            .map(|j| {
                op(
                    &format!("t{j}"),
                    OriginalGate::Phase {
                        wires: vec![j as i64],
                        turns: 1,
                    },
                )
            })
            .collect(),
        vec![],
        Default::default(),
        &a,
    )
    .unwrap();
    let small = OriginalInstrument::new(
        vec![0],
        vec![0],
        CliffordCircuit::computational(bits(1, &[])).unwrap(),
        vec![
            op("h", OriginalGate::H(0)),
            op(
                "t",
                OriginalGate::Phase {
                    wires: vec![0],
                    turns: 1,
                },
            ),
            op("h2", OriginalGate::H(0)),
        ],
        vec![],
        Default::default(),
        &a,
    )
    .unwrap();
    for compiler in compilers() {
        let trace = ir.compile_trace(compiler, &a).unwrap();
        let view = trace.with_assignment(&Default::default(), &a).unwrap();
        let row = bits(n, &[0, 1]);
        let probability = trace
            .prepare_boundary_event_probability(std::slice::from_ref(&row), &options, &a)
            .unwrap();
        assert!(probability.resource().native_resource().is_some());
        let p = probability
            .evaluate(&view, &bits(1, &[]), &a)
            .unwrap()
            .value;
        assert_eq!(p, half);
        let mut values = vec![p];
        for w in [word(n, &[128], &[]), word(n, &[128], &[128])] {
            let q = trace
                .prepare_boundary_event_moment(&w, std::slice::from_ref(&row), &options, &a)
                .unwrap();
            assert!(q.resource().native_resource().is_some());
            assert!(q.resource().quotient_rank() <= 8);
            assert!(q.resource().num_terms() <= 256);
            let result = q.evaluate(&view, &bits(1, &[]), &a).unwrap();
            assert_eq!(result.quotient_rank, q.resource().quotient_rank());
            assert_eq!(result.terms, q.resource().num_terms());
            assert_eq!(result.value, expected);
            values.push(result.value);
        }
        assert_eq!(
            a.subtract(
                &a.subtract(
                    &a.multiply(&half, &values[0]).unwrap(),
                    &a.multiply(&expected, &values[1]).unwrap()
                )
                .unwrap(),
                &a.multiply(&expected, &values[2]).unwrap()
            )
            .unwrap(),
            ExactScalar::zero()
        );
        assert!(matches!(
            trace.prepare_boundary_event_moment(
                &word(n, &[0], &[]),
                std::slice::from_ref(&row),
                &options,
                &a
            ),
            Err(OriginalError::Completed(_))
        ));
        let limited = OriginalNetworkOptions {
            limits: OriginalNetworkLimits {
                max_joint_paths: 2,
                ..options.limits
            },
            ..options
        };
        assert!(
            matches!(trace.prepare_boundary_event_probability(&[row],&limited,&a),Err(OriginalError::ResourceLimit{option:"max_network_joint_paths",limit:2,required})if required>2)
        );
        let trace = small.compile_trace(compiler, &a).unwrap();
        let view = trace.with_assignment(&Default::default(), &a).unwrap();
        let q = trace
            .prepare_boundary_event_probability(&[bits(1, &[0])], &options, &a)
            .unwrap();
        assert_eq!(
            q.evaluate(&view, &bits(1, &[]), &a).unwrap().value,
            a.scalar([2.into(), 1.into(), 0.into(), (-1).into()], -2)
                .unwrap()
        );
    }
}
#[test]
fn mixed_output_record_events_relabel_reports_and_coherent_words_once() {
    let a = ScalarArithmetic::default();
    let catalogue = builtin_phase_catalogue().unwrap();
    let options = native(Some(catalogue), 8, 256);
    let ir = OriginalInstrument::new(
        vec![0, 1],
        vec![1, 0],
        CliffordCircuit::computational(bits(2, &[])).unwrap(),
        vec![
            op("h0", OriginalGate::H(0)),
            op(
                "t0",
                OriginalGate::Phase {
                    wires: vec![0],
                    turns: 1,
                },
            ),
            op("h00", OriginalGate::H(0)),
            op(
                "m",
                OriginalGate::Measure {
                    factors: vec![(0, OriginalPauli::Z)],
                    inverted: false,
                    record: "a".into(),
                },
            ),
            op("h1", OriginalGate::H(1)),
            op(
                "t1",
                OriginalGate::Phase {
                    wires: vec![1],
                    turns: 1,
                },
            ),
        ],
        vec![],
        Default::default(),
        &a,
    )
    .unwrap();
    let isqrt = a.omega(0, -1).unwrap();
    let masses = [
        a.scalar([2.into(), 1.into(), 0.into(), (-1).into()], -2)
            .unwrap(),
        a.scalar([2.into(), (-1).into(), 0.into(), 1.into()], -2)
            .unwrap(),
    ];
    for compiler in compilers() {
        let trace = ir.compile_trace(compiler, &a).unwrap();
        for flags in 0..16 {
            let flip = flags & 1 != 0;
            let physical_x = flags & 2 != 0;
            let physical_z = flags & 4 != 0;
            let output_z = flags & 8 != 0;
            let mut assignment = OriginalTraceAssignment::default();
            if flip {
                assignment.report_flips.push("a".into());
            }
            for (active, after, wire, pauli) in [
                (physical_x, "m", 0, OriginalPauli::X),
                (physical_z, "m", 0, OriginalPauli::Z),
                (output_z, "t1", 1, OriginalPauli::Z),
            ] {
                if active {
                    assignment.faults.push(OriginalFault {
                        after: after.into(),
                        wire,
                        pauli,
                    });
                }
            }
            let view = trace.with_assignment(&assignment, &a).unwrap();
            let row = bits(3, &[1, 2]);
            let q = trace
                .prepare_boundary_event_probability(std::slice::from_ref(&row), &options, &a)
                .unwrap();
            let x = trace
                .prepare_boundary_event_moment(
                    &word(3, &[0], &[]),
                    std::slice::from_ref(&row),
                    &options,
                    &a,
                )
                .unwrap();
            let coherent = trace
                .prepare_boundary_event_moment(&word(3, &[1, 2], &[2]), &[row], &options, &a)
                .unwrap();
            for outcome in [false, true] {
                let rhs = bits(1, if outcome { &[0] } else { &[] });
                let possible = outcome == (flip ^ physical_x);
                assert_eq!(
                    q.evaluate(&view, &rhs, &a).unwrap().value,
                    if possible {
                        ExactScalar::one()
                    } else {
                        ExactScalar::zero()
                    }
                );
                assert_eq!(
                    x.evaluate(&view, &rhs, &a).unwrap().value,
                    if !possible {
                        ExactScalar::zero()
                    } else if output_z {
                        isqrt.negated()
                    } else {
                        isqrt.clone()
                    }
                );
                assert_eq!(
                    coherent.evaluate(&view, &rhs, &a).unwrap().value,
                    if !possible {
                        ExactScalar::zero()
                    } else if flip ^ physical_z {
                        isqrt.clone()
                    } else {
                        isqrt.negated()
                    }
                );
            }
            let q = trace
                .prepare_boundary_event_probability(&[bits(3, &[2])], &options, &a)
                .unwrap();
            for r in [false, true] {
                assert_eq!(
                    q.evaluate(&view, &bits(1, if r { &[0] } else { &[] }), &a)
                        .unwrap()
                        .value,
                    masses[usize::from(r ^ flip)]
                );
            }
        }
    }
}
#[test]
fn boundary_event_shapes_commutation_and_geometry_are_explicitly_admitted() {
    let a = ScalarArithmetic::default();
    let catalogue = builtin_phase_catalogue().unwrap();
    let options = native(Some(catalogue), 8, 256);
    let ir = OriginalInstrument::new(
        vec![0],
        vec![0],
        CliffordCircuit::computational(bits(1, &[])).unwrap(),
        vec![op("h", OriginalGate::H(0))],
        vec![],
        Default::default(),
        &a,
    )
    .unwrap();
    for compiler in compilers() {
        let trace = ir.compile_trace(compiler, &a).unwrap();
        let view = trace.with_assignment(&Default::default(), &a).unwrap();
        assert!(matches!(
            trace.prepare_boundary_event_probability(&[bits(2, &[0])], &options, &a),
            Err(OriginalError::DimensionMismatch)
        ));
        assert!(matches!(
            trace.prepare_boundary_event_moment(&word(2, &[], &[]), &[], &options, &a),
            Err(OriginalError::DimensionMismatch)
        ));
        let nonhermitian = PauliWord::new(bits(1, &[0]), bits(1, &[0]), 0).unwrap();
        assert!(matches!(
            trace.prepare_boundary_event_moment(&nonhermitian, &[], &options, &a),
            Err(OriginalError::Completed(_))
        ));
        assert!(matches!(
            trace.prepare_boundary_event_moment(
                &word(1, &[0], &[]),
                &[bits(1, &[0])],
                &options,
                &a
            ),
            Err(OriginalError::Completed(_))
        ));
        for (label, limits) in [
            (
                "max_network_geometry_words",
                OriginalNetworkLimits {
                    max_geometry_words: 0,
                    ..options.limits
                },
            ),
            (
                "max_event_rows",
                OriginalNetworkLimits {
                    max_event_rows: 0,
                    ..options.limits
                },
            ),
        ] {
            assert!(
                matches!(trace.prepare_boundary_event_probability(&[bits(1,&[0])],&OriginalNetworkOptions{limits,..options},&a),Err(OriginalError::ResourceLimit{option,limit:0,required})if option==label&&required>0)
            );
        }
        let q = trace
            .prepare_boundary_event_probability(&[bits(1, &[0]), bits(1, &[0])], &options, &a)
            .unwrap();
        assert_eq!(
            q.evaluate(&view, &bits(2, &[0]), &a).unwrap().value,
            ExactScalar::zero()
        );
        assert!(q.evaluate(&view, &bits(1, &[]), &a).is_err());
        assert_eq!(
            q.evaluate(&view, &bits(2, &[]), &a).unwrap().value,
            a.omega(0, -2).unwrap()
        );
        let empty = OriginalInstrument::new(
            vec![],
            vec![],
            CliffordCircuit::computational(bits(0, &[])).unwrap(),
            vec![],
            vec![],
            Default::default(),
            &a,
        )
        .unwrap();
        let trace = empty.compile_trace(compiler, &a).unwrap();
        let view = trace.with_assignment(&Default::default(), &a).unwrap();
        let options = native(Some(catalogue), 0, 1);
        let q = trace
            .prepare_boundary_event_probability(&[bits(0, &[])], &options, &a)
            .unwrap();
        assert_eq!(
            q.evaluate(&view, &bits(1, &[]), &a).unwrap().value,
            ExactScalar::one()
        );
        assert_eq!(
            q.evaluate(&view, &bits(1, &[0]), &a).unwrap().value,
            ExactScalar::zero()
        );
    }
}
