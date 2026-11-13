// SPDX-License-Identifier: MPL-2.0
// Complete finite scenarios from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_deferred_feedback.py::DeferredFeedbackTests: signed Pauli
// products/report fault anchors, literal joint events, Native literal joint
// events, and explicit unsupported controls. Literal matrices below are an
// independent oracle. Supplemental packed-width and admission cases are Rust
// contracts, without private cache, Native layout or random-stream claims.
use rmagna::clifford::{CliffordCircuit, CliffordGate};
use rmagna::gf2::BitVector;
use rmagna::instrument::*;
use rmagna::pauli::PauliWord;
use rmagna::phase::NativePhaseOptions;
use rmagna::scalar::{ExactScalar as V, ScalarArithmetic};
use rmagna::unitary::{CompilerStrategy, OwnedStabilizerExpansion};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
fn bits(n: usize, value: usize) -> BitVector {
    BitVector::from_indices(
        n,
        (0..n.min(usize::BITS as usize)).filter(|j| value >> j & 1 != 0),
    )
    .unwrap()
}
fn word(n: usize, x: usize, z: usize) -> PauliWord {
    PauliWord::new(bits(n, x), bits(n, z), (2 * (x & z).count_ones() % 8) as u8).unwrap()
}
fn strategies() -> [CompilerStrategy; 2] {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
}
fn initial(n: usize, plus: &[usize], a: &ScalarArithmetic) -> rmagna::stabilizer::AffineKet {
    CliffordCircuit::new(n, plus.iter().map(|&q| CliffordGate::H(q)))
        .unwrap()
        .evolve(&CliffordCircuit::computational(bits(n, 0)).unwrap(), a)
        .unwrap()
        .unwrap()
}
fn op(id: &str, gate: OriginalGate) -> OriginalOperation {
    OriginalOperation {
        id: id.into(),
        gate,
    }
}
fn measure(
    id: &str,
    name: &str,
    factors: &[(i64, OriginalPauli)],
    inverted: bool,
) -> OriginalOperation {
    op(
        id,
        OriginalGate::Measure {
            factors: factors.to_vec(),
            inverted,
            record: name.into(),
        },
    )
}
fn r(name: &str) -> ControlExpression {
    ControlExpression::record(name.into(), Default::default()).unwrap()
}
fn e(kind: ControlOperator, args: Vec<ControlExpression>) -> ControlExpression {
    ControlExpression::compose(kind, args, Default::default()).unwrap()
}
fn cg(
    kind: ControlGateKind,
    wires: &[i64],
    exponent: Option<ControlExpression>,
    when: ControlExpression,
) -> ControlGate {
    ControlGate::new(kind, wires.to_vec(), exponent, when, Default::default()).unwrap()
}
fn declaration(id: &str, parents: &[&str], gates: Vec<ControlGate>) -> OriginalDeclaredControl {
    OriginalDeclaredControl::new(
        id.into(),
        parents.iter().map(|s| s.to_string()).collect(),
        "literal-v1".into(),
        ControlRule::new(gates, Default::default()).unwrap(),
    )
    .unwrap()
}
fn fault(after: &str, wire: i64, pauli: OriginalPauli) -> OriginalFault {
    OriginalFault {
        after: after.into(),
        wire,
        pauli,
    }
}
fn flips(mask: usize) -> Vec<String> {
    (0..2)
        .filter(|j| mask >> j & 1 != 0)
        .map(|j| if j == 0 { "a" } else { "b" }.into())
        .collect()
}
fn amplitude(side: &OwnedStabilizerExpansion, physical: &BitVector, a: &ScalarArithmetic) -> V {
    side.entries()
        .iter()
        .try_fold(V::zero(), |v, k| {
            a.add(&v, &k.prepare().unwrap().amplitude(physical, a).unwrap())
        })
        .unwrap()
}
fn event(records: &[&str], value: bool) -> OriginalRecordEvent {
    OriginalRecordEvent {
        parity: OriginalRecordParity {
            records: records.iter().map(|s| s.to_string()).collect(),
            inverted: false,
        },
        value,
    }
}
fn network_options() -> OriginalNetworkOptions<'static> {
    OriginalNetworkOptions {
        resource: OriginalNetworkResource::Native(NativePhaseOptions::default()),
        ..Default::default()
    }
}
// Literal state vectors never call a production simulator for an expected value.
fn h(v: &mut [V], q: usize, a: &ScalarArithmetic) {
    let c = a
        .scalar([0.into(), 1.into(), 0.into(), (-1).into()], -1)
        .unwrap();
    for i in 0..v.len() {
        if i >> q & 1 == 0 {
            let j = i ^ (1 << q);
            let x = v[i].clone();
            let y = v[j].clone();
            v[i] = a.multiply(&c, &a.add(&x, &y).unwrap()).unwrap();
            v[j] = a.multiply(&c, &a.subtract(&x, &y).unwrap()).unwrap();
        }
    }
}
fn phase(v: &mut [V], mask: usize, turns: u8, a: &ScalarArithmetic) {
    let c = a.omega(turns.into(), 0).unwrap();
    for (i, x) in v.iter_mut().enumerate() {
        if (i & mask).count_ones() % 2 == 1 {
            *x = a.multiply(x, &c).unwrap();
        }
    }
}
fn pauli(v: &[V], x: usize, z: usize, a: &ScalarArithmetic) -> Vec<V> {
    let c = a.omega(i64::from(2 * (x & z).count_ones() % 8), 0).unwrap();
    let mut out = vec![V::zero(); v.len()];
    for (i, t) in v.iter().enumerate() {
        let t = a.multiply(t, &c).unwrap();
        out[i ^ x] = if (i & z).count_ones().is_multiple_of(2) {
            t
        } else {
            t.negated()
        };
    }
    out
}
fn apply(v: &mut Vec<V>, x: usize, z: usize, a: &ScalarArithmetic) {
    *v = pauli(v, x, z, a);
}
fn project(
    v: &[V],
    x: usize,
    z: usize,
    negative: bool,
    outcome: usize,
    a: &ScalarArithmetic,
) -> Vec<V> {
    let pv = pauli(v, x, z, a);
    let half = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
        .unwrap();
    v.iter()
        .zip(pv)
        .map(|(u, p)| {
            a.multiply(
                &half,
                &if (outcome != 0) ^ negative {
                    a.subtract(u, &p).unwrap()
                } else {
                    a.add(u, &p).unwrap()
                },
            )
            .unwrap()
        })
        .collect()
}
fn moment(v: &[V], x: usize, z: usize, a: &ScalarArithmetic) -> V {
    v.iter()
        .zip(pauli(v, x, z, a))
        .try_fold(V::zero(), |s, (u, p)| {
            a.add(&s, &a.multiply(&u.conjugate(), &p)?)
        })
        .unwrap()
}
fn joint_branch(av: usize, bv: usize, flip: usize, f: usize, a: &ScalarArithmetic) -> Vec<V> {
    let half = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
        .unwrap();
    let mut v = (0..8)
        .map(|i| if i & 1 == 0 { half.clone() } else { V::zero() })
        .collect::<Vec<_>>();
    h(&mut v, 0, a);
    if f == 1 {
        apply(&mut v, 1, 1, a);
    }
    phase(&mut v, 2, 1, a);
    v = project(&v, 0, 1, false, av, a);
    if f == 2 {
        apply(&mut v, 1, 0, a);
    }
    h(&mut v, 0, a);
    v = project(&v, 1, 0, false, bv, a);
    // Canonical destructive-X readout stores the true bit in this Z-basis wire.
    h(&mut v, 0, a);
    let ar = av ^ (flip & 1);
    let br = bv ^ ((flip >> 1) & 1);
    if ar != 0 {
        apply(&mut v, 2, 2, a);
    }
    if ar ^ br == 0 {
        phase(&mut v, 6, 6, a);
    }
    if br != 0 {
        apply(&mut v, 4, 0, a);
    }
    phase(&mut v, 4, 2, a);
    if f == 3 {
        apply(&mut v, 0, 2, a);
    }
    v
}
fn joint_instrument(a: &ScalarArithmetic) -> OriginalInstrument {
    let gates = vec![
        cg(ControlGateKind::Y, &[1], None, r("a")),
        cg(
            ControlGateKind::Phase,
            &[1, 2],
            Some(ControlExpression::constant(6)),
            e(
                ControlOperator::Not,
                vec![e(ControlOperator::Xor, vec![r("a"), r("b")])],
            ),
        ),
        cg(ControlGateKind::X, &[2], None, r("b")),
        cg(
            ControlGateKind::S,
            &[2],
            None,
            ControlExpression::constant(1),
        ),
    ];
    OriginalInstrument::new_with_declared_controls(
        vec![0, 1, 2],
        vec![1, 2],
        initial(3, &[1, 2], a),
        vec![
            op("h", OriginalGate::H(0)),
            op(
                "t",
                OriginalGate::Phase {
                    wires: vec![1],
                    turns: 1,
                },
            ),
            measure("m0", "a", &[(0, OriginalPauli::Z)], false),
            op("h2", OriginalGate::H(0)),
            op(
                "m1",
                OriginalGate::Destructive {
                    wire: 0,
                    basis: ReadoutBasis::X,
                    record: "b".into(),
                },
            ),
            op("c", OriginalGate::Control),
        ],
        vec![declaration("c", &["a", "b"], gates)],
        Default::default(),
        a,
    )
    .unwrap()
}
fn joint_events(compact: bool) {
    let a = ScalarArithmetic::default();
    let ir = joint_instrument(&a);
    let mut count = 0;
    for compiler in strategies() {
        let trace = ir.compile_trace(compiler, &a).unwrap();
        for report_flips in [
            vec![],
            vec!["a".into()],
            vec!["a".into(), "a".into(), "b".into()],
        ] {
            let flip = report_flips
                .iter()
                .fold(0, |v, s: &String| v ^ if s == "a" { 1 } else { 2 });
            for f in 0..4 {
                let faults = match f {
                    0 => vec![],
                    1 => vec![fault("h", 0, OriginalPauli::Y)],
                    2 => vec![fault("m0", 0, OriginalPauli::X)],
                    _ => vec![fault("c", 1, OriginalPauli::Z)],
                };
                let assignment = OriginalTraceAssignment {
                    faults,
                    report_flips: report_flips.clone(),
                };
                let view = trace.with_assignment(&assignment, &a).unwrap();
                assert_eq!(trace.physical_width(), 4);
                for selected in [None, Some(0), Some(1)] {
                    let reported = selected.map_or(0, |v| v ^ ((flip & 1) ^ ((flip >> 1) & 1)));
                    let events =
                        selected.map_or(vec![], |_| vec![event(&["a", "b"], reported != 0)]);
                    let parities = events.iter().map(|e| e.parity.clone()).collect::<Vec<_>>();
                    let rhs = bits(events.len(), reported);
                    for (x, z) in [(0, 0), (1, 0), (1, 1), (3, 2)] {
                        let mut expected = V::zero();
                        for av in 0..2 {
                            for bv in 0..2 {
                                if selected.is_none_or(|v| av ^ bv == v) {
                                    expected = a
                                        .add(
                                            &expected,
                                            &moment(
                                                &joint_branch(av, bv, flip, f, &a),
                                                x << 1,
                                                z << 1,
                                                &a,
                                            ),
                                        )
                                        .unwrap();
                                }
                            }
                        }
                        let w = word(2, x, z);
                        let actual = if compact {
                            let extended = PauliWord::new(
                                BitVector::from_indices(trace.network_width(), w.x().ones())
                                    .unwrap(),
                                BitVector::from_indices(trace.network_width(), w.z().ones())
                                    .unwrap(),
                                w.phase(),
                            )
                            .unwrap();
                            trace
                                .prepare_network_moment(
                                    &extended,
                                    &parities,
                                    &network_options(),
                                    &a,
                                )
                                .unwrap()
                                .evaluate(&view, &rhs, &a)
                                .unwrap()
                                .value
                        } else {
                            view.output_moment(&[], &events, &w, &Default::default(), &a)
                                .unwrap()
                                .distribution
                                .total(&a)
                                .unwrap()
                        };
                        assert_eq!(
                            actual, expected,
                            "compact={compact}/{compiler:?}/flip{flip}/fault{f}/event{selected:?}/X{x}Z{z}"
                        );
                        count += 1;
                    }
                }
                if !compact {
                    let side = view.expansion(&Default::default(), &a).unwrap();
                    let mut expected = vec![V::zero(); 16];
                    for av in 0..2 {
                        for bv in 0..2 {
                            for (i, t) in joint_branch(av, bv, flip, f, &a).into_iter().enumerate()
                            {
                                let j = i | (av << 3);
                                expected[j] = a.add(&expected[j], &t).unwrap();
                            }
                        }
                    }
                    for (i, want) in expected.into_iter().enumerate() {
                        assert_eq!(amplitude(&side, &bits(4, i), &a), want);
                    }
                }
            }
        }
    }
    assert_eq!(count, 288);
}
#[test]
fn complete_literal_joint_events_and_output_observables() {
    joint_events(false);
}
#[test]
fn complete_native_literal_joint_events_and_output_observables() {
    joint_events(true);
}
#[test]
fn complete_signed_pauli_products_and_report_fault_anchors() {
    let a = ScalarArithmetic::default();
    let ir = OriginalInstrument::new_with_declared_controls(
        vec![0, 1],
        vec![0, 1],
        initial(2, &[], &a),
        vec![
            op("h", OriginalGate::H(0)),
            measure(
                "m",
                "a",
                &[(0, OriginalPauli::Y), (1, OriginalPauli::Z)],
                true,
            ),
            op("c", OriginalGate::Control),
            measure(
                "n",
                "b",
                &[(0, OriginalPauli::X), (1, OriginalPauli::X)],
                false,
            ),
        ],
        vec![declaration(
            "c",
            &["a"],
            vec![cg(ControlGateKind::Y, &[1], None, r("a"))],
        )],
        Default::default(),
        &a,
    )
    .unwrap();
    for compiler in strategies() {
        let trace = ir.compile_trace(compiler, &a).unwrap();
        for flip in 0..4 {
            let view = trace
                .with_assignment(
                    &OriginalTraceAssignment {
                        faults: vec![fault("m", 0, OriginalPauli::Z)],
                        report_flips: flips(flip),
                    },
                    &a,
                )
                .unwrap();
            let mut coherent = vec![V::zero(); 16];
            for av in 0..2 {
                for bv in 0..2 {
                    let mut v = vec![V::one(), V::zero(), V::zero(), V::zero()];
                    h(&mut v, 0, &a);
                    v = project(&v, 1, 3, true, av, &a);
                    apply(&mut v, 0, 1, &a);
                    if av ^ (flip & 1) != 0 {
                        apply(&mut v, 2, 2, &a);
                    }
                    v = project(&v, 3, 0, false, bv, &a);
                    for (i, t) in v.iter().enumerate() {
                        coherent[i | (av << 2) | (bv << 3)] = t.clone();
                    }
                    let reported = (av | (bv << 1)) ^ flip;
                    let events = [
                        event(&["a"], reported & 1 != 0),
                        event(&["b"], reported & 2 != 0),
                    ];
                    let parities = events.iter().map(|e| e.parity.clone()).collect::<Vec<_>>();
                    for (x, z) in [(0, 0), (1, 1), (2, 0)] {
                        let want = moment(&v, x, z, &a);
                        let w = word(2, x, z);
                        assert_eq!(
                            view.output_moment(&[], &events, &w, &Default::default(), &a)
                                .unwrap()
                                .distribution
                                .total(&a)
                                .unwrap(),
                            want
                        );
                        let extended = word(4, x, z);
                        assert_eq!(
                            trace
                                .prepare_network_moment(
                                    &extended,
                                    &parities,
                                    &network_options(),
                                    &a
                                )
                                .unwrap()
                                .evaluate(&view, &bits(2, reported), &a)
                                .unwrap()
                                .value,
                            want
                        );
                    }
                }
            }
            let side = view.expansion(&Default::default(), &a).unwrap();
            for (i, want) in coherent.into_iter().enumerate() {
                // Signed a is stored as its physical, uninverted Pauli product.
                let physical = i ^ 4;
                assert_eq!(
                    amplitude(&side, &bits(4, physical), &a),
                    want,
                    "{compiler:?}/flip{flip}/basis{i}"
                );
            }
        }
    }
}
#[test]
fn complete_explicit_unsupported_controls_do_not_invoke_callbacks() {
    let a = ScalarArithmetic::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let operations = vec![
        measure("m", "a", &[(0, OriginalPauli::Z)], false),
        op("c", OriginalGate::Control),
    ];
    let opaque = OriginalInstrument::new(
        vec![0],
        vec![0],
        initial(1, &[], &a),
        operations.clone(),
        vec![OriginalControl {
            operation: "c".into(),
            parents: vec!["a".into()],
            version: "opaque-v1".into(),
            callback: Arc::new(move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(vec![])
            }),
        }],
        Default::default(),
        &a,
    )
    .unwrap();
    for compiler in strategies() {
        assert!(matches!(
            opaque.compile_trace(compiler, &a),
            Err(OriginalError::Metadata {
                kind: "static trace requires resolved controls",
                ..
            })
        ));
    }
    for gate in [
        cg(ControlGateKind::T, &[0], None, r("a")),
        cg(
            ControlGateKind::Z,
            &[0],
            None,
            e(ControlOperator::And, vec![r("a"), r("a")]),
        ),
    ] {
        let ir = OriginalInstrument::new_with_declared_controls(
            vec![0],
            vec![0],
            initial(1, &[], &a),
            operations.clone(),
            vec![declaration("c", &["a"], vec![gate])],
            Default::default(),
            &a,
        )
        .unwrap();
        for compiler in strategies() {
            assert!(matches!(
                ir.compile_trace(compiler, &a),
                Err(OriginalError::DeclaredControl {
                    error: ControlRuleError::UnsupportedFeedback(_),
                    ..
                })
            ));
        }
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
#[test]
fn wide_wires_high_records_report_flips_faults_and_empty_anchors_remain_exact() {
    let a = ScalarArithmetic::default();
    for n in [65usize, 129] {
        let mut operations = vec![op("h", OriginalGate::H(0))];
        for j in 0..65 {
            operations.push(measure(
                &format!("m{j}"),
                &format!("r{j}"),
                &[(0, OriginalPauli::Z)],
                false,
            ));
        }
        for id in ["empty-before", "c", "empty-after"] {
            operations.push(op(id, OriginalGate::Control));
        }
        let declarations = vec![
            declaration("empty-before", &[], vec![]),
            declaration(
                "c",
                &["r64"],
                vec![cg(ControlGateKind::X, &[(n - 1) as i64], None, r("r64"))],
            ),
            declaration("empty-after", &[], vec![]),
        ];
        let limits = OriginalLimits {
            max_emitted_operations: 0,
            ..Default::default()
        };
        let ir = OriginalInstrument::new_with_declared_controls(
            (0..n as i64).collect(),
            (0..n as i64).rev().collect(),
            initial(n, &[], &a),
            operations.clone(),
            declarations.clone(),
            limits,
            &a,
        )
        .unwrap();
        for compiler in strategies() {
            let trace = ir.compile_trace(compiler, &a).unwrap();
            assert_eq!(trace.physical_width(), n + 65);
            assert_eq!(trace.outputs()[0], (n - 1) as i64);
            assert!(trace.environment().is_empty());
            assert_eq!(trace.records()[64], "r64");
            let event = OriginalRecordParity {
                records: vec!["r64".into()],
                inverted: false,
            };
            let w = PauliWord::new(
                BitVector::zeros(trace.network_width()).unwrap(),
                BitVector::from_indices(trace.network_width(), [0]).unwrap(),
                0,
            )
            .unwrap();
            let prepared = trace
                .prepare_network_moment(&w, std::slice::from_ref(&event), &network_options(), &a)
                .unwrap();
            for list in [vec![], vec!["r64".into()], vec!["r64".into(), "r64".into()]] {
                let flip = list.len() % 2;
                for y in [false, true] {
                    let faults = if y {
                        vec![fault("c", (n - 1) as i64, OriginalPauli::Y)]
                    } else {
                        vec![]
                    };
                    let view = trace
                        .with_assignment(
                            &OriginalTraceAssignment {
                                faults,
                                report_flips: list.clone(),
                            },
                            &a,
                        )
                        .unwrap();
                    let side = view.expansion(&Default::default(), &a).unwrap();
                    for av in 0..2 {
                        let reported = av ^ flip;
                        let want = a
                            .scalar(
                                [
                                    if reported ^ usize::from(y) == 0 {
                                        1.into()
                                    } else {
                                        (-1).into()
                                    },
                                    0.into(),
                                    0.into(),
                                    0.into(),
                                ],
                                -1,
                            )
                            .unwrap();
                        assert_eq!(
                            prepared
                                .evaluate(&view, &bits(1, reported), &a)
                                .unwrap()
                                .value,
                            want
                        );
                        let output = word(n, 0, 1);
                        assert_eq!(
                            view.output_moment(
                                &[],
                                &[OriginalRecordEvent {
                                    parity: event.clone(),
                                    value: reported != 0
                                }],
                                &output,
                                &Default::default(),
                                &a
                            )
                            .unwrap()
                            .distribution
                            .total(&a)
                            .unwrap(),
                            want
                        );
                        let physical = BitVector::from_indices(
                            n + 65,
                            (0..n + 65).filter(|&q| {
                                if q == 0 {
                                    av != 0
                                } else if q == n - 1 {
                                    reported ^ usize::from(y) != 0
                                } else {
                                    q >= n && av != 0
                                }
                            }),
                        )
                        .unwrap();
                        let want_amp = a
                            .omega(
                                if y {
                                    if reported == 0 { 2 } else { 6 }
                                } else {
                                    0
                                },
                                -1,
                            )
                            .unwrap();
                        assert_eq!(amplitude(&side, &physical, &a), want_amp);
                    }
                }
            }
        }
        let narrow = OriginalLimits {
            max_physical_slots: n + 64,
            ..limits
        };
        let source = OriginalInstrument::new_with_declared_controls(
            (0..n as i64).collect(),
            (0..n as i64).rev().collect(),
            initial(n, &[], &a),
            operations,
            declarations,
            narrow,
            &a,
        )
        .unwrap();
        assert!(
            matches!(source.compile_trace(CompilerStrategy::ParityCopies,&a),Err(OriginalError::ResourceLimit{option:"max_physical_slots",limit,required})if limit==n+64&&required==(n+65)as u128)
        );
    }
}
fn cx(v: &mut [V], control: usize, target: usize) {
    for i in 0..v.len() {
        if i >> control & 1 != 0 && i >> target & 1 == 0 {
            v.swap(i, i ^ (1 << target));
        }
    }
}
fn cz(v: &mut [V], control: usize, target: usize) {
    for (i, t) in v.iter_mut().enumerate() {
        if i >> control & 1 != 0 && i >> target & 1 != 0 {
            *t = t.negated();
        }
    }
}
#[test]
fn affine_clifford_families_even_variable_phases_and_inactive_exponents_match_matrices() {
    let a = ScalarArithmetic::default();
    let guard = e(
        ControlOperator::Xor,
        vec![e(ControlOperator::Xor, vec![r("a"), r("a")]), r("a")],
    );
    let exponent = e(
        ControlOperator::Add,
        vec![
            ControlExpression::constant(3),
            e(
                ControlOperator::Add,
                vec![
                    e(
                        ControlOperator::Multiply,
                        vec![ControlExpression::constant(4), r("a")],
                    ),
                    e(
                        ControlOperator::Multiply,
                        vec![
                            ControlExpression::constant(2),
                            e(ControlOperator::Not, vec![r("a")]),
                        ],
                    ),
                ],
            ),
        ],
    );
    let mut gates = vec![];
    for (kind, wires) in [
        (ControlGateKind::I, vec![999]),
        (ControlGateKind::X, vec![1]),
        (ControlGateKind::Y, vec![2]),
        (ControlGateKind::Z, vec![1]),
        (ControlGateKind::H, vec![2]),
        (ControlGateKind::Cx, vec![1, 2]),
        (ControlGateKind::Cz, vec![2, 1]),
    ] {
        gates.push(cg(kind, &wires, None, guard.clone()));
    }
    gates.push(cg(
        ControlGateKind::Phase,
        &[1, 2],
        Some(exponent),
        ControlExpression::constant(1),
    ));
    gates.push(cg(
        ControlGateKind::T,
        &[0],
        None,
        ControlExpression::constant(1),
    ));
    gates.push(cg(
        ControlGateKind::Phase,
        &[1],
        Some(e(ControlOperator::And, vec![r("a"), r("a")])),
        ControlExpression::constant(0),
    ));
    let ir = OriginalInstrument::new_with_declared_controls(
        vec![0, 1, 2],
        vec![2, 1, 0],
        initial(3, &[1, 2], &a),
        vec![
            op("h", OriginalGate::H(0)),
            op(
                "t",
                OriginalGate::Phase {
                    wires: vec![1],
                    turns: 1,
                },
            ),
            measure("m", "a", &[(0, OriginalPauli::Z)], false),
            op("c", OriginalGate::Control),
        ],
        vec![declaration("c", &["a"], gates)],
        Default::default(),
        &a,
    )
    .unwrap();
    let reverse = |x: usize| ((x & 1) << 2) | (x & 2) | ((x & 4) >> 2);
    for compiler in strategies() {
        let trace = ir.compile_trace(compiler, &a).unwrap();
        for flip in 0..2 {
            let report_flips = if flip == 0 { vec![] } else { vec!["a".into()] };
            let view = trace
                .with_assignment(
                    &OriginalTraceAssignment {
                        report_flips,
                        ..Default::default()
                    },
                    &a,
                )
                .unwrap();
            let mut coherent = vec![V::zero(); 16];
            for av in 0..2 {
                let half = a
                    .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
                    .unwrap();
                let mut v = (0..8)
                    .map(|i| if i & 1 == 0 { half.clone() } else { V::zero() })
                    .collect::<Vec<_>>();
                h(&mut v, 0, &a);
                phase(&mut v, 2, 1, &a);
                v = project(&v, 0, 1, false, av, &a);
                let reported = av ^ flip;
                if reported != 0 {
                    apply(&mut v, 2, 0, &a);
                    apply(&mut v, 4, 4, &a);
                    apply(&mut v, 0, 2, &a);
                    h(&mut v, 2, &a);
                    cx(&mut v, 1, 2);
                    cz(&mut v, 2, 1);
                }
                phase(&mut v, 6, if reported == 0 { 5 } else { 7 }, &a);
                phase(&mut v, 1, 1, &a);
                for (i, t) in v.iter().enumerate() {
                    coherent[i | (av << 3)] = t.clone();
                }
                for (x, z) in [
                    (0, 0),
                    (1, 0),
                    (1, 1),
                    (3, 2),
                    (7, 7),
                    (4, 5),
                    (2, 7),
                    (5, 3),
                ] {
                    let want = moment(&v, reverse(x), reverse(z), &a);
                    let w = word(3, x, z);
                    let events = [event(&["a"], reported != 0)];
                    assert_eq!(
                        view.output_moment(&[], &events, &w, &Default::default(), &a)
                            .unwrap()
                            .distribution
                            .total(&a)
                            .unwrap(),
                        want
                    );
                    assert_eq!(
                        trace
                            .prepare_network_moment(
                                &word(4, x, z),
                                &[events[0].parity.clone()],
                                &network_options(),
                                &a
                            )
                            .unwrap()
                            .evaluate(&view, &bits(1, reported), &a)
                            .unwrap()
                            .value,
                        want
                    );
                }
            }
            let side = view.expansion(&Default::default(), &a).unwrap();
            for (i, want) in coherent.into_iter().enumerate() {
                assert_eq!(amplitude(&side, &bits(4, i), &a), want);
            }
        }
    }
}
#[test]
fn declared_target_geometry_masks_and_lowering_limits_are_admitted_before_work() {
    let a = ScalarArithmetic::default();
    let source = |limits| {
        OriginalInstrument::new_with_declared_controls(
            vec![0, 1, 2],
            vec![0, 1, 2],
            initial(3, &[], &a),
            vec![
                measure("m", "a", &[(0, OriginalPauli::Z)], false),
                op("c", OriginalGate::Control),
            ],
            vec![declaration(
                "c",
                &["a"],
                vec![cg(ControlGateKind::Cx, &[1, 2], None, r("a"))],
            )],
            limits,
            &a,
        )
        .unwrap()
    };
    let ir = source(OriginalLimits::default());
    let compiler = CompilerStrategy::ParityCopies;
    assert!(matches!(
        ir.compile_trace_with_declared_limits(
            compiler,
            OriginalDeclaredTraceLimits {
                max_gate_targets: 1,
                ..Default::default()
            },
            &a
        ),
        Err(OriginalError::DeclaredControl {
            error: ControlRuleError::ResourceLimit {
                limit: 1,
                required: 2,
                ..
            },
            ..
        })
    ));
    assert!(matches!(
        ir.compile_trace_with_declared_limits(
            compiler,
            OriginalDeclaredTraceLimits {
                max_target_occurrences: 2,
                ..Default::default()
            },
            &a
        ),
        Err(OriginalError::ResourceLimit {
            option: "max_declared_target_occurrences",
            limit: 2,
            required: 3
        })
    ));
    let required = match ir.compile_trace_with_declared_limits(
        compiler,
        OriginalDeclaredTraceLimits {
            max_preparation_words: 0,
            ..Default::default()
        },
        &a,
    ) {
        Err(OriginalError::ResourceLimit {
            option: "max_declared_preparation_words",
            limit: 0,
            required,
        }) => usize::try_from(required).unwrap(),
        _ => panic!("expected preparation refusal"),
    };
    assert!(required > 0);
    assert!(
        matches!(ir.compile_trace_with_declared_limits(compiler,OriginalDeclaredTraceLimits{max_preparation_words:required-1,..Default::default()},&a),Err(OriginalError::ResourceLimit{option:"max_declared_preparation_words",required:r,..})if r==required as u128)
    );
    let trace = ir
        .compile_trace_with_declared_limits(
            compiler,
            OriginalDeclaredTraceLimits {
                max_preparation_words: required,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
    let original = trace.with_assignment(&Default::default(), &a).unwrap();
    let query = trace
        .prepare_network_probability(&[], &Default::default(), &a)
        .unwrap();
    assert_eq!(
        query.evaluate(&original, &bits(0, 0), &a).unwrap().value,
        V::one()
    );
    assert!(
        trace
            .with_assignment(
                &OriginalTraceAssignment {
                    report_flips: vec!["missing".into()],
                    ..Default::default()
                },
                &a
            )
            .is_err()
    );
    assert_eq!(
        query.evaluate(&original, &bits(0, 0), &a).unwrap().value,
        V::one()
    );
    let exact = ir
        .compile_trace_with_declared_limits(
            compiler,
            OriginalDeclaredTraceLimits {
                max_target_occurrences: 3,
                max_preparation_words: required,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
    assert!(matches!(
        exact.with_assignment(
            &OriginalTraceAssignment {
                faults: vec![fault("c", 1, OriginalPauli::I)],
                ..Default::default()
            },
            &a
        ),
        Err(OriginalError::ResourceLimit {
            option: "max_declared_target_occurrences",
            limit: 3,
            required: 4
        })
    ));
    assert!(
        matches!(exact.with_assignment(&OriginalTraceAssignment{report_flips:vec!["a".into()],..Default::default()},&a),Err(OriginalError::ResourceLimit{option:"max_declared_preparation_words",required:n,..})if n>required as u128)
    );
    let limits = OriginalLimits {
        max_faults: 0,
        ..Default::default()
    };
    let narrow = source(limits).compile_trace(compiler, &a).unwrap();
    assert!(matches!(
        narrow.with_assignment(
            &OriginalTraceAssignment {
                faults: vec![fault("c", 1, OriginalPauli::I)],
                ..Default::default()
            },
            &a
        ),
        Err(OriginalError::ResourceLimit {
            option: "max_faults",
            limit: 0,
            required: 1
        })
    ));
    let limits = OriginalLimits {
        max_gates: 0,
        ..Default::default()
    };
    assert!(matches!(
        source(limits).compile_trace(compiler, &a),
        Err(OriginalError::ResourceLimit {
            option: "max_gates",
            limit: 0,
            required: 1
        })
    ));
}
