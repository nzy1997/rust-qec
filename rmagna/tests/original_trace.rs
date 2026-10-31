//! Original static trace contracts and independent exact matrix oracles.
use rmagna::clifford::CliffordCircuit;
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    OriginalAssignment, OriginalControl, OriginalError, OriginalFault, OriginalGate as G,
    OriginalInstrument, OriginalLimits, OriginalNetworkOptions, OriginalOperation,
    OriginalPauli as P, OriginalRecordEvent as Event, OriginalRecordParity as Parity,
    OriginalTraceAssignment, OriginalTraceOptions, ReadoutBasis,
};
use rmagna::pauli::PauliWord;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::CompilerStrategy;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn bits(n: usize, v: usize) -> BitVector {
    BitVector::from_indices(
        n,
        (0..n.min(usize::BITS as usize)).filter(|j| v >> j & 1 != 0),
    )
    .unwrap()
}
fn op(id: &str, gate: G) -> OriginalOperation {
    OriginalOperation {
        id: id.into(),
        gate,
    }
}
fn parity(names: &[&str]) -> Parity {
    Parity {
        records: names.iter().map(|s| s.to_string()).collect(),
        inverted: false,
    }
}
fn zero(n: usize) -> AffineKet {
    CliffordCircuit::computational(bits(n, 0)).unwrap()
}
fn plus(n: usize) -> AffineKet {
    let a = ScalarArithmetic::default();
    CliffordCircuit::new(n, (0..n).map(rmagna::clifford::CliffordGate::H))
        .unwrap()
        .evolve(&zero(n), &a)
        .unwrap()
        .unwrap()
}
fn ir(
    inputs: Vec<i64>,
    outputs: Vec<i64>,
    initial: AffineKet,
    ops: Vec<OriginalOperation>,
    limits: OriginalLimits,
) -> OriginalInstrument {
    OriginalInstrument::new(
        inputs,
        outputs,
        initial,
        ops,
        vec![],
        limits,
        &ScalarArithmetic::default(),
    )
    .unwrap()
}
fn strategies() -> [CompilerStrategy; 2] {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
}
fn pword(n: usize, x: usize, z: usize) -> PauliWord {
    PauliWord::new(bits(n, x), bits(n, z), 2 * ((x & z).count_ones() as u8 % 2)).unwrap()
}

#[test]
fn original_ccz_inherits_two_side_entries_and_four_pairs() {
    let a = ScalarArithmetic::default();
    let o = OriginalTraceOptions::default();
    let limits = OriginalLimits {
        expansion: rmagna::unitary::ExpansionLimits {
            max_side_terms: 2,
            max_pairs: 4,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut ops = vec![op("ccz", G::Ccz(0, 1, 2))];
    for j in 0..3 {
        ops.push(op(
            &format!("m{j}"),
            G::Destructive {
                wire: j,
                basis: ReadoutBasis::X,
                record: format!("r{j}"),
            },
        ));
    }
    let instrument = ir(vec![0, 1, 2], vec![], plus(3), ops, limits);
    let keep = vec![parity(&["r0"]), parity(&["r1"]), parity(&["r2"])];
    for strategy in strategies() {
        let trace = instrument.compile_trace(strategy, &a).unwrap();
        for flips in [vec![], vec!["r1".into()]] {
            let flipped = !flips.is_empty();
            let view = trace
                .with_assignment(
                    &OriginalTraceAssignment {
                        report_flips: flips,
                        ..Default::default()
                    },
                    &a,
                )
                .unwrap();
            let q = view.probability(&keep, &[], &o, &a).unwrap();
            assert_eq!(q.resource_expansion(), "inherited-product");
            let prepared = view
                .prepare_records(&keep, &[], &o, Default::default(), &a)
                .unwrap();
            let warm = prepared.probability(&bits(0, 0), &a).unwrap();
            assert_eq!(prepared.statistics().ordered_pairs, 4);
            assert_eq!(prepared.statistics().support_factorizations, 4);
            assert_eq!(prepared.statistics().pairing_factorizations, 4);
            assert_eq!(warm.side, q.side);
            assert_eq!(warm.work.support_factorizations, 0);
            assert_eq!(warm.work.pairing_factorizations, 0);
            assert_eq!(q.side.side_entries, 2);
            assert_eq!(q.distribution.statistics().pair_work, 4);
            assert!(q.distribution.statistics().retained_terms <= 4);
            assert_eq!(q.distribution.total(&a).unwrap(), ExactScalar::one());
            for word in 0..8 {
                let expected = a
                    .scalar(
                        [
                            if word ^ if flipped { 2 } else { 0 } == 0 {
                                9
                            } else {
                                1
                            },
                            0,
                            0,
                            0,
                        ]
                        .map(Into::into),
                        -4,
                    )
                    .unwrap();
                assert_eq!(q.distribution.value(&bits(3, word), &a).unwrap(), expected);
                assert_eq!(
                    warm.distribution.value(&bits(3, word), &a).unwrap(),
                    expected
                );
            }
        }
    }
}

#[test]
fn original_entangled_trace_is_distinct_from_postselection() {
    let a = ScalarArithmetic::default();
    let o = OriginalTraceOptions::default();
    let initial = CliffordCircuit::new(2, [rmagna::clifford::CliffordGate::H(0)])
        .unwrap()
        .evolve(&zero(2), &a)
        .unwrap()
        .unwrap();
    let instrument = ir(
        vec![0, 1],
        vec![0],
        initial,
        vec![
            op(
                "t",
                G::Phase {
                    wires: vec![0],
                    turns: 1,
                },
            ),
            op("cx", G::Cx(0, 1)),
            op(
                "m",
                G::Destructive {
                    wire: 1,
                    basis: ReadoutBasis::X,
                    record: "a".into(),
                },
            ),
        ],
        OriginalLimits::default(),
    );
    for s in strategies() {
        let trace = instrument.compile_trace(s, &a).unwrap();
        let v = trace
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        assert_eq!(
            v.probability(&[], &[], &o, &a)
                .unwrap()
                .distribution
                .total(&a)
                .unwrap(),
            ExactScalar::one()
        );
        for (x, z) in [(1, 0), (1, 1), (0, 1)] {
            let word = pword(1, x, z);
            assert_eq!(
                v.output_moment(&[], &[], &word, &o, &a)
                    .unwrap()
                    .distribution
                    .total(&a)
                    .unwrap(),
                ExactScalar::zero()
            );
            let values = [false, true].map(|bit| {
                v.output_moment(
                    &[],
                    &[Event {
                        parity: parity(&["a"]),
                        value: bit,
                    }],
                    &word,
                    &o,
                    &a,
                )
                .unwrap()
                .distribution
                .total(&a)
                .unwrap()
            });
            assert_eq!(a.add(&values[0], &values[1]).unwrap(), ExactScalar::zero());
            if x != 0 {
                assert_ne!(values[0], ExactScalar::zero());
            }
        }
    }
}

#[test]
fn static_records_preserve_signed_measurements_redundancies_and_vacuum() {
    let a = ScalarArithmetic::default();
    let o = OriginalTraceOptions::default();
    let instrument = ir(
        vec![-7],
        vec![-7],
        zero(1),
        vec![
            op(
                "negative",
                G::Measure {
                    factors: vec![(-7, P::Z)],
                    inverted: true,
                    record: "a".into(),
                },
            ),
            op(
                "empty",
                G::Measure {
                    factors: vec![],
                    inverted: true,
                    record: "b".into(),
                },
            ),
        ],
        OriginalLimits::default(),
    );
    let trace = instrument
        .compile_trace(CompilerStrategy::ParityCopies, &a)
        .unwrap();
    let v = trace
        .with_assignment(
            &OriginalTraceAssignment {
                report_flips: vec!["a".into(), "a".into(), "b".into()],
                ..Default::default()
            },
            &a,
        )
        .unwrap();
    let keep = [
        parity(&["a"]),
        parity(&["a"]),
        parity(&["a", "a"]),
        parity(&["b"]),
    ];
    let q = v.probability(&keep, &[], &o, &a).unwrap();
    assert_eq!(
        q.distribution.value(&bits(4, 3), &a).unwrap(),
        ExactScalar::one()
    );
    assert_eq!(
        q.distribution.value(&bits(4, 1), &a).unwrap(),
        ExactScalar::zero()
    );
    let e = parity(&["a"]);
    assert_eq!(
        v.probability(
            &[],
            &[
                Event {
                    parity: e.clone(),
                    value: true
                },
                Event {
                    parity: e,
                    value: false
                }
            ],
            &o,
            &a
        )
        .unwrap()
        .distribution
        .total(&a)
        .unwrap(),
        ExactScalar::zero()
    );
    let vacuum = ir(vec![], vec![], zero(0), vec![], OriginalLimits::default())
        .compile_trace(CompilerStrategy::ParityCopies, &a)
        .unwrap();
    let v = vacuum
        .with_assignment(&OriginalTraceAssignment::default(), &a)
        .unwrap();
    let q = v
        .probability(
            &[Parity {
                inverted: true,
                ..Default::default()
            }],
            &[],
            &o,
            &a,
        )
        .unwrap();
    assert_eq!(
        q.distribution.value(&bits(1, 1), &a).unwrap(),
        ExactScalar::one()
    );
    assert_eq!(
        v.output_moment(&[], &[], &pword(0, 0, 0), &o, &a)
            .unwrap()
            .distribution
            .total(&a)
            .unwrap(),
        ExactScalar::one()
    );
}

fn sum_amplitude(terms: &[AffineKet], n: usize, word: usize, a: &ScalarArithmetic) -> ExactScalar {
    terms.iter().fold(ExactScalar::zero(), |sum, k| {
        a.add(
            &sum,
            &k.prepare().unwrap().amplitude(&bits(n, word), a).unwrap(),
        )
        .unwrap()
    })
}
#[test]
fn sparse_lifetimes_keep_fault_order_global_phases_and_immutable_replacement() {
    let a = ScalarArithmetic::default();
    let o = OriginalTraceOptions::default();
    let instrument = ir(
        vec![9],
        vec![9],
        zero(1),
        vec![
            op("first", G::Identity(vec![])),
            op("second", G::Identity(vec![])),
            op(
                "reset",
                G::Reset {
                    wire: 9,
                    plus: false,
                },
            ),
            op(
                "m",
                G::Destructive {
                    wire: 9,
                    basis: ReadoutBasis::Z,
                    record: "a".into(),
                },
            ),
            op(
                "new",
                G::Prepare {
                    wire: 9,
                    plus: false,
                },
            ),
        ],
        OriginalLimits::default(),
    );
    for s in strategies() {
        let trace = instrument.compile_trace(s, &a).unwrap();
        assert_eq!(trace.environment()[0].wire, 9);
        assert_eq!(trace.outputs(), [9]);
        let faults = vec![
            OriginalFault {
                after: "second".into(),
                wire: 9,
                pauli: P::Z,
            },
            OriginalFault {
                after: "first".into(),
                wire: 9,
                pauli: P::X,
            },
            OriginalFault {
                after: "reset".into(),
                wire: 9,
                pauli: P::Y,
            },
        ];
        let v = trace
            .with_assignment(
                &OriginalTraceAssignment {
                    faults,
                    ..Default::default()
                },
                &a,
            )
            .unwrap();
        let e = v.expansion(&o, &a).unwrap();
        // Chronological X then Z gives -|1>; reset's fresh Y adds i|1>.
        // Slots: old9 environment0, new9 readout1, prepared9 output2.
        assert_eq!(sum_amplitude(e.entries(), 3, 3, &a), a.omega(6, 0).unwrap());
        assert_eq!(
            v.probability(&[parity(&["a"])], &[], &o, &a)
                .unwrap()
                .distribution
                .value(&bits(1, 1), &a)
                .unwrap(),
            ExactScalar::one()
        );
        let clean = trace
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        assert_eq!(
            sum_amplitude(clean.expansion(&o, &a).unwrap().entries(), 3, 0, &a),
            ExactScalar::one()
        );
        assert!(matches!(
            trace.with_assignment(
                &OriginalTraceAssignment {
                    faults: vec![OriginalFault {
                        after: "m".into(),
                        wire: 9,
                        pauli: P::X
                    }],
                    ..Default::default()
                },
                &a
            ),
            Err(OriginalError::Lifetime { .. })
        ));
        assert!(
            trace
                .with_assignment(
                    &OriginalTraceAssignment {
                        faults: vec![OriginalFault {
                            after: "m".into(),
                            wire: 100,
                            pauli: P::I
                        }],
                        ..Default::default()
                    },
                    &a
                )
                .is_ok()
        );
    }
}

#[test]
fn static_trace_rejects_controls_without_invoking_callbacks() {
    let a = ScalarArithmetic::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let instrument = OriginalInstrument::new(
        vec![],
        vec![],
        zero(0),
        vec![op("control", G::Control)],
        vec![OriginalControl {
            operation: "control".into(),
            parents: vec![],
            version: "v1".into(),
            callback: Arc::new(move |_| {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(vec![])
            }),
        }],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    assert!(matches!(
        instrument.compile_trace(CompilerStrategy::ParityCopies, &a),
        Err(OriginalError::Metadata {
            kind: "static trace requires resolved controls",
            ..
        })
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn static_trace_limits_errors_leave_reusable_references() {
    let a = ScalarArithmetic::default();
    let o = OriginalTraceOptions::default();
    let build = |limits| {
        ir(
            vec![0],
            vec![],
            plus(1),
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
            limits,
        )
        .compile_trace(CompilerStrategy::ParityCopies, &a)
        .unwrap()
    };
    let trace = build(OriginalLimits::default());
    let v = trace
        .with_assignment(&OriginalTraceAssignment::default(), &a)
        .unwrap();
    let q = [parity(&["a"])];
    assert!(matches!(
        v.probability(
            &q,
            &[],
            &OriginalTraceOptions {
                max_record_occurrences: 0,
                ..Default::default()
            },
            &a
        ),
        Err(OriginalError::ResourceLimit {
            option: "max_record_occurrences",
            ..
        })
    ));
    assert!(v.probability(&[parity(&["unknown"])], &[], &o, &a).is_err());
    assert!(
        v.output_moment(
            &[],
            &[],
            &PauliWord::new(bits(0, 0), bits(0, 0), 1).unwrap(),
            &o,
            &a
        )
        .is_err()
    );
    assert_eq!(
        v.probability(&q, &[], &o, &a)
            .unwrap()
            .distribution
            .total(&a)
            .unwrap(),
        ExactScalar::one()
    );
    for (name, limits) in [
        (
            "max_pairs",
            OriginalLimits {
                expansion: rmagna::unitary::ExpansionLimits {
                    max_pairs: 3,
                    ..Default::default()
                },
                ..Default::default()
            },
        ),
        (
            "max_record_rows",
            OriginalLimits {
                completed: rmagna::records::CompletedRecordLimits {
                    max_record_rows: 0,
                    ..Default::default()
                },
                ..Default::default()
            },
        ),
        (
            "max_event_rows",
            OriginalLimits {
                completed: rmagna::records::CompletedRecordLimits {
                    max_event_rows: 0,
                    ..Default::default()
                },
                ..Default::default()
            },
        ),
    ] {
        let trace = build(limits);
        let view = trace
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        let events = if name == "max_event_rows" {
            vec![Event {
                parity: parity(&["a"]),
                value: false,
            }]
        } else {
            vec![]
        };
        assert!(
            matches!(view.probability(&q,&events,&o,&a),Err(OriginalError::ResourceLimit {option,..}) if option==name)
        );
    }
}

fn json_scalar(v: &Value, a: &ScalarArithmetic) -> ExactScalar {
    a.scalar(
        std::array::from_fn(|j| v["coefficients"][j].as_i64().unwrap().into()),
        v["power"].as_i64().unwrap(),
    )
    .unwrap()
}
fn json_initial(v: &Value, a: &ScalarArithmetic) -> AffineKet {
    let columns = v["columns"].as_array().unwrap();
    let n = v["width"].as_u64().unwrap() as usize;
    AffineKet::new(
        bits(n, v["base"].as_u64().unwrap() as usize),
        columns
            .iter()
            .map(|x| bits(n, x.as_u64().unwrap() as usize))
            .collect(),
        rmagna::quadratic::QuadraticForm::new(
            0,
            v["linear"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as u8)
                .collect(),
            v["upper"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| bits(columns.len(), x.as_u64().unwrap() as usize))
                .collect(),
        )
        .unwrap(),
        a.omega(v["phase"].as_i64().unwrap(), v["power"].as_i64().unwrap())
            .unwrap(),
    )
    .unwrap()
}
fn json_gate(v: &Value) -> G {
    let q = v["qubits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| q.as_i64().unwrap())
        .collect::<Vec<_>>();
    match v["kind"].as_str().unwrap() {
        "H" => G::H(q[0]),
        "S" | "T" | "PHASE" => G::Phase {
            wires: q,
            turns: v["exponent"].as_i64().unwrap(),
        },
        "CX" => G::Cx(q[0], q[1]),
        "CZ" => G::Cz(q[0], q[1]),
        "CCZ" => G::Ccz(q[0], q[1], q[2]),
        "MP" => G::Measure {
            factors: q
                .into_iter()
                .zip(v["paulis"].as_str().unwrap().chars().map(|p| match p {
                    'X' => P::X,
                    'Y' => P::Y,
                    'Z' => P::Z,
                    _ => panic!("Pauli"),
                }))
                .collect(),
            inverted: v["sign"].as_i64().unwrap() != 0,
            record: v["record"].as_str().unwrap().into(),
        },
        "RESET_PLUS" => G::Reset {
            wire: q[0],
            plus: true,
        },
        "MX_DESTRUCT" => G::Destructive {
            wire: q[0],
            basis: ReadoutBasis::X,
            record: v["record"].as_str().unwrap().into(),
        },
        kind => panic!("unexpected original gate {kind}"),
    }
}
// Dense vector oracle uses literal 2x2/permutation matrices and scalar ring
// operations only: no Clifford evolution, affine paths, PauliWord, quotient,
// stabilizer expansion, native query or coherent compiler enters this oracle.
fn dense_pauli(v: &[ExactScalar], slot: usize, p: P, a: &ScalarArithmetic) -> Vec<ExactScalar> {
    let mut out = vec![ExactScalar::zero(); v.len()];
    for (i, c) in v.iter().enumerate() {
        let mut phase = if matches!(p, P::Y) { 2 } else { 0 };
        if matches!(p, P::Y | P::Z) && i >> slot & 1 != 0 {
            phase += 4;
        }
        let j = if matches!(p, P::X | P::Y) {
            i ^ (1 << slot)
        } else {
            i
        };
        out[j] = a.multiply(c, &a.omega(phase, 0).unwrap()).unwrap();
    }
    out
}
fn dense_h(v: &[ExactScalar], q: usize, a: &ScalarArithmetic) -> Vec<ExactScalar> {
    (0..v.len())
        .map(|i| {
            let lo = i & !(1 << q);
            let hi = lo | (1 << q);
            let sum = if i >> q & 1 == 0 {
                a.add(&v[lo], &v[hi])
            } else {
                a.subtract(&v[lo], &v[hi])
            }
            .unwrap();
            a.multiply(&sum, &a.omega(0, -1).unwrap()).unwrap()
        })
        .collect()
}
fn dense_branch(
    ops: &[OriginalOperation],
    fault: &OriginalFault,
    values: [bool; 2],
    a: &ScalarArithmetic,
) -> (Vec<ExactScalar>, BTreeMap<i64, usize>) {
    let mut v = vec![ExactScalar::zero(); 8];
    // Literal amplitudes for base5, columns(3,6), linear(1,3), cross2,
    // coefficient omega7/2; independent of either affine input coordinate basis.
    for (word, phase) in [(0, 3), (3, 5), (5, 7), (6, 1)] {
        v[word] = a.omega(phase, -2).unwrap();
    }
    let mut live = BTreeMap::from([(0, 0), (1, 1), (2, 2)]);
    for op in ops {
        match &op.gate {
            G::H(q) => v = dense_h(&v, live[q], a),
            G::Phase { wires, turns } => {
                for (i, c) in v.iter_mut().enumerate() {
                    if wires.iter().fold(0, |p, q| p ^ ((i >> live[q]) & 1)) != 0 {
                        *c = a.multiply(c, &a.omega(*turns, 0).unwrap()).unwrap();
                    }
                }
            }
            G::Cx(c, t) => {
                let mut out = vec![ExactScalar::zero(); v.len()];
                for (i, x) in v.iter().enumerate() {
                    out[if i >> live[c] & 1 != 0 {
                        i ^ (1 << live[t])
                    } else {
                        i
                    }] = x.clone();
                }
                v = out;
            }
            G::Cz(q, r) => {
                for (i, c) in v.iter_mut().enumerate() {
                    if i >> live[q] & 1 != 0 && i >> live[r] & 1 != 0 {
                        *c = a.subtract(&ExactScalar::zero(), c).unwrap();
                    }
                }
            }
            G::Ccz(q, r, s) => {
                for (i, c) in v.iter_mut().enumerate() {
                    if i >> live[q] & 1 != 0 && i >> live[r] & 1 != 0 && i >> live[s] & 1 != 0 {
                        *c = a.subtract(&ExactScalar::zero(), c).unwrap();
                    }
                }
            }
            G::Measure {
                factors, inverted, ..
            } => {
                let mut transformed = v.clone();
                for (wire, p) in factors {
                    transformed = dense_pauli(&transformed, live[wire], *p, a);
                }
                for (c, p) in v.iter_mut().zip(transformed) {
                    *c = if values[0] ^ inverted {
                        a.subtract(c, &p)
                    } else {
                        a.add(c, &p)
                    }
                    .unwrap();
                    *c = a
                        .multiply(
                            c,
                            &a.scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
                                .unwrap(),
                        )
                        .unwrap();
                }
            }
            G::Reset { wire, plus: true } => {
                let old = v.len();
                let slot = old.trailing_zeros() as usize;
                let mut out = vec![ExactScalar::zero(); 2 * old];
                for (i, c) in v.iter().enumerate() {
                    let c = a.multiply(c, &a.omega(0, -1).unwrap()).unwrap();
                    out[i] = c.clone();
                    out[old + i] = c;
                }
                v = out;
                live.insert(*wire, slot);
            }
            G::Destructive {
                wire,
                basis: ReadoutBasis::X,
                ..
            } => {
                let slot = live.remove(wire).unwrap();
                v = dense_h(&v, slot, a);
                for (i, c) in v.iter_mut().enumerate() {
                    if (i >> slot & 1 != 0) != values[1] {
                        *c = ExactScalar::zero();
                    }
                }
            }
            _ => panic!("seed oracle gate"),
        }
        if op.id == fault.after {
            v = dense_pauli(&v, live[&fault.wire], fault.pauli, a);
        }
    }
    (v, live)
}
fn dense_moment(
    v: &[ExactScalar],
    live: &BTreeMap<i64, usize>,
    x: usize,
    z: usize,
    a: &ScalarArithmetic,
) -> ExactScalar {
    let p = match (x, z) {
        (0, 0) => P::I,
        (1, 0) => P::X,
        (1, 1) => P::Y,
        (0, 1) => P::Z,
        _ => panic!("one output word"),
    };
    // Output order is original(1,0); masks refer to its first wire.
    let transformed = dense_pauli(v, live[&1], p, a);
    v.iter()
        .zip(transformed)
        .fold(ExactScalar::zero(), |sum, (l, r)| {
            a.add(&sum, &a.multiply(&l.conjugate(), &r).unwrap())
                .unwrap()
        })
}

#[test]
fn original_seeded_static_instruments_all_trials_events_moments_and_input_bases() {
    let a = ScalarArithmetic::default();
    let o = OriginalTraceOptions::default();
    let fixtures: Value =
        serde_json::from_str(include_str!("fixtures/original_static_seed26092381.json")).unwrap();
    assert_eq!(fixtures["seed"], 26092381);
    assert_eq!(
        fixtures["original_method_source_sha256"],
        "3dcbb502019f061474dd4beb2dede81c224d73e417f0f724158d67df0bbfcd58"
    );
    let trials = fixtures["trials"].as_array().unwrap();
    assert_eq!(trials.len(), 12);
    for trial in trials {
        let operations = trial["operations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| op(v["id"].as_str().unwrap(), json_gate(v)))
            .collect::<Vec<_>>();
        assert_eq!(operations.len(), 15);
        let f = &trial["faults"][0];
        let fault = OriginalFault {
            after: f["after"].as_str().unwrap().into(),
            wire: f["qubit"].as_i64().unwrap(),
            pauli: match f["pauli"].as_str().unwrap() {
                "X" => P::X,
                "Y" => P::Y,
                "Z" => P::Z,
                _ => panic!("fault"),
            },
        };
        let flips = trial["report_flips"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        // Independent literal dense matrix check of every original branch table.
        for row in trial["table"].as_array().unwrap() {
            let values = [
                row["true_values"][0].as_u64().unwrap() != 0,
                row["true_values"][1].as_u64().unwrap() != 0,
            ];
            let (v, live) = dense_branch(&operations, &fault, values, &a);
            for (j, (x, z)) in [(0, 0), (1, 0), (1, 1), (0, 1)].into_iter().enumerate() {
                assert_eq!(
                    dense_moment(&v, &live, x, z, &a),
                    json_scalar(&row["values"][j], &a),
                    "native vs literal trial={},values={values:?},moment={j}",
                    trial["trial"]
                );
            }
        }
        for basis in ["initial", "equivalent"] {
            let instrument = ir(
                vec![0, 1, 2],
                vec![1, 0],
                json_initial(&trial[basis], &a),
                operations.clone(),
                OriginalLimits::default(),
            );
            for strategy in strategies() {
                let trace = instrument.compile_trace(strategy, &a).unwrap();
                assert_eq!(trace.outputs(), [1, 0]);
                assert_eq!(trace.environment()[0].wire, 1);
                let view = trace
                    .with_assignment(
                        &OriginalTraceAssignment {
                            faults: vec![fault.clone()],
                            report_flips: flips.clone(),
                        },
                        &a,
                    )
                    .unwrap();
                // Literal amplitude comparisons retain global phases and the
                // original output/environment order, for all complete branches.
                for row in trial["table"].as_array().unwrap() {
                    let values = [
                        row["true_values"][0].as_u64().unwrap() != 0,
                        row["true_values"][1].as_u64().unwrap() != 0,
                    ];
                    let branch = instrument
                        .compile(
                            &OriginalAssignment {
                                true_values: vec![("a".into(), values[0]), ("b".into(), values[1])],
                                faults: vec![fault.clone()],
                                report_flips: flips.clone(),
                            },
                            strategy,
                            &a,
                        )
                        .unwrap();
                    let (dense, _) = dense_branch(&operations, &fault, values, &a);
                    for w in 0..8 {
                        let index = ((w & 1) << 3)
                            | ((w & 2) >> 1)
                            | ((w & 4) >> 1)
                            | ((values[1] as usize) << 2);
                        assert_eq!(
                            branch.amplitude(&bits(3, w), 65536, &a).unwrap(),
                            dense[index],
                            "literal amplitude trial={},basis={basis},word={w}",
                            trial["trial"]
                        );
                    }
                }
                let network_events = [parity(&["a"]), parity(&["a", "b"])];
                let network_probability = trace
                    .prepare_network_probability(
                        &network_events,
                        &OriginalNetworkOptions::default(),
                        &a,
                    )
                    .unwrap();
                let network_moments = [(0, 0), (1, 0), (1, 1), (0, 1)]
                    .into_iter()
                    .map(|(x, z)| {
                        trace
                            .prepare_network_moment(
                                &pword(trace.network_width(), x, z),
                                &network_events,
                                &OriginalNetworkOptions::default(),
                                &a,
                            )
                            .unwrap()
                    })
                    .collect::<Vec<_>>();
                let prepared = view
                    .prepare_records(
                        &[parity(&["a"])],
                        &[Event {
                            parity: parity(&["a", "b"]),
                            value: false,
                        }],
                        &o,
                        rmagna::records::PreparedRecordLimits::default(),
                        &a,
                    )
                    .unwrap();
                assert_eq!(
                    prepared.statistics().ordered_pairs,
                    prepared.side_statistics().side_entries.pow(2)
                );
                assert_eq!(
                    prepared.statistics().support_factorizations,
                    prepared.statistics().ordered_pairs
                );
                assert_eq!(
                    prepared.statistics().pairing_factorizations,
                    prepared.statistics().ordered_pairs
                );
                for parity_bit in [false, true] {
                    let events = [Event {
                        parity: parity(&["a", "b"]),
                        value: parity_bit,
                    }];
                    let keep = [parity(&["a"])];
                    let probability = view.probability(&keep, &events, &o, &a).unwrap();
                    let warm_probability = prepared
                        .probability(&bits(1, parity_bit as usize), &a)
                        .unwrap();
                    assert_eq!(warm_probability.work.support_factorizations, 0);
                    assert_eq!(warm_probability.work.pairing_factorizations, 0);
                    assert_eq!(warm_probability.side, probability.side);

                    assert_eq!(
                        probability.distribution.statistics().source_entries,
                        probability.side.side_entries
                    );
                    assert_eq!(
                        probability.distribution.statistics().pair_work,
                        probability.side.side_entries.pow(2)
                    );
                    for (moment, (x, z)) in [(0, 0), (1, 0), (1, 1), (0, 1)].into_iter().enumerate()
                    {
                        let signed = if moment == 0 {
                            None
                        } else {
                            Some(
                                view.output_moment(&keep, &events, &pword(2, x, z), &o, &a)
                                    .unwrap(),
                            )
                        };
                        let warm_moment = prepared
                            .output_moment(&bits(1, parity_bit as usize), &pword(2, x, z), &a)
                            .unwrap();
                        assert_eq!(warm_moment.work.support_factorizations, 0);
                        assert_eq!(warm_moment.work.pairing_factorizations, 0);
                        for bit in [false, true] {
                            let expected = trial["table"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .filter(|r| {
                                    (r["reported"][0].as_u64().unwrap() != 0) == bit
                                        && ((r["reported"][0].as_u64().unwrap()
                                            ^ r["reported"][1].as_u64().unwrap())
                                            != 0)
                                            == parity_bit
                                })
                                .fold(ExactScalar::zero(), |sum, r| {
                                    a.add(&sum, &json_scalar(&r["values"][moment], &a)).unwrap()
                                });
                            let actual = match &signed {
                                None => probability.distribution.value(&bits(1, bit as usize), &a),
                                Some(q) => q.distribution.value(&bits(1, bit as usize), &a),
                            }
                            .unwrap();
                            assert_eq!(
                                warm_moment
                                    .distribution
                                    .value(&bits(1, bit as usize), &a)
                                    .unwrap(),
                                expected
                            );
                            if moment == 0 {
                                assert_eq!(
                                    warm_probability
                                        .distribution
                                        .value(&bits(1, bit as usize), &a)
                                        .unwrap(),
                                    expected
                                );
                            }
                            assert_eq!(
                                actual, expected,
                                "trial={},basis={basis},strategy={strategy:?},parity={parity_bit},bit={bit},moment={moment}",
                                trial["trial"]
                            );
                            let network_outcomes =
                                bits(2, bit as usize | ((parity_bit as usize) << 1));
                            assert_eq!(
                                network_moments[moment]
                                    .evaluate(&view, &network_outcomes, &a,)
                                    .unwrap()
                                    .value,
                                expected,
                                "global network trial={},basis={basis},strategy={strategy:?},parity={parity_bit},bit={bit},moment={moment}",
                                trial["trial"]
                            );
                            if moment == 0 {
                                assert_eq!(
                                    network_probability
                                        .evaluate(&view, &network_outcomes, &a,)
                                        .unwrap()
                                        .value,
                                    expected
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn original_trace_cross_word_records_and_explicit_catalogue_selection() {
    let a = ScalarArithmetic::default();
    let ops = (0..65)
        .map(|q| {
            op(
                &format!("m{q}"),
                G::Destructive {
                    wire: q,
                    basis: ReadoutBasis::Z,
                    record: format!("r{q}"),
                },
            )
        })
        .collect();
    let trace = ir(
        (0..65).collect(),
        vec![],
        zero(65),
        ops,
        OriginalLimits::default(),
    )
    .compile_trace(CompilerStrategy::ParityCopies, &a)
    .unwrap();
    let view = trace
        .with_assignment(
            &OriginalTraceAssignment {
                report_flips: vec!["r64".into()],
                ..Default::default()
            },
            &a,
        )
        .unwrap();
    let keep = (0..65)
        .map(|j| parity(&[&format!("r{j}")]))
        .collect::<Vec<_>>();
    let catalogue = rmagna::catalogue::PhaseCatalogue::new();
    for o in [
        OriginalTraceOptions::default(),
        OriginalTraceOptions {
            catalogue: Some(&catalogue),
            ..Default::default()
        },
    ] {
        let q = view.probability(&keep, &[], &o, &a).unwrap();
        let plan = view
            .prepare_records(&keep, &[], &o, Default::default(), &a)
            .unwrap();
        let warm = plan.probability(&bits(0, 0), &a).unwrap();
        assert_eq!(
            warm.distribution
                .value(&BitVector::from_indices(65, [64]).unwrap(), &a)
                .unwrap(),
            ExactScalar::one()
        );
        assert_eq!(
            warm.distribution
                .value(&BitVector::zeros(65).unwrap(), &a)
                .unwrap(),
            ExactScalar::zero()
        );
        assert_eq!(warm.side, q.side);
        assert_eq!(
            q.side.resource_kind,
            rmagna::phase::PhaseResourceKind::CliffordFibers
        );
        assert_eq!(
            q.distribution
                .value(&BitVector::from_indices(65, [64]).unwrap(), &a)
                .unwrap(),
            ExactScalar::one()
        );
        assert_eq!(
            q.distribution
                .value(&BitVector::zeros(65).unwrap(), &a)
                .unwrap(),
            ExactScalar::zero()
        );
    }
}

#[test]
fn original_trace_raw_path_pair_and_query_limits_are_typed_and_reusable() {
    let a = ScalarArithmetic::default();
    let o = OriginalTraceOptions::default();
    let keep = [parity(&["a"])];
    let build = |l| {
        ir(
            vec![0],
            vec![],
            plus(1),
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
            l,
        )
        .compile_trace(CompilerStrategy::ParityCopies, &a)
        .unwrap()
    };
    for (name, completed) in [
        (
            "max_pair_paths",
            rmagna::records::CompletedRecordLimits {
                max_pair_paths: 0,
                ..Default::default()
            },
        ),
        (
            "max_pairs",
            rmagna::records::CompletedRecordLimits {
                max_pairs: 3,
                ..Default::default()
            },
        ),
        (
            "max_source_entries",
            rmagna::records::CompletedRecordLimits {
                max_source_entries: 1,
                ..Default::default()
            },
        ),
    ] {
        let trace = build(OriginalLimits {
            completed,
            ..Default::default()
        });
        let view = trace
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        assert!(
            matches!(view.probability(&keep,&[],&o,&a),Err(OriginalError::Completed(rmagna::records::CompletedRecordError::ResourceLimit {option,..})) if option==name)
        );
        assert_eq!(view.expansion(&o, &a).unwrap().entries().len(), 2);
    }
    let trace = build(OriginalLimits {
        expansion: rmagna::unitary::ExpansionLimits {
            max_path_bits: 0,
            ..Default::default()
        },
        ..Default::default()
    });
    let view = trace
        .with_assignment(&OriginalTraceAssignment::default(), &a)
        .unwrap();
    assert!(matches!(
        view.probability(&keep, &[], &o, &a),
        Err(OriginalError::Unitary(
            rmagna::unitary::UnitaryError::ExpansionResourceLimit {
                option: "max_path_bits",
                ..
            }
        ))
    ));
    let trace = build(OriginalLimits {
        completed: rmagna::records::CompletedRecordLimits {
            max_query_rows: 0,
            ..Default::default()
        },
        ..Default::default()
    });
    let view = trace
        .with_assignment(&OriginalTraceAssignment::default(), &a)
        .unwrap();
    let q = view.probability(&keep, &[], &o, &a).unwrap();
    assert!(q.distribution.value(&bits(1, 0), &a).is_err());
    assert_eq!(q.distribution.total(&a).unwrap(), ExactScalar::one());
}

#[test]
fn original_trace_admits_query_name_bytes_before_copying_error_identities() {
    let a = ScalarArithmetic::default();
    let o = OriginalTraceOptions::default();
    let trace = ir(
        vec![0],
        vec![],
        zero(1),
        vec![op(
            "m",
            G::Destructive {
                wire: 0,
                basis: ReadoutBasis::Z,
                record: "a".into(),
            },
        )],
        OriginalLimits {
            max_metadata_bytes: 1024,
            ..Default::default()
        },
    )
    .compile_trace(CompilerStrategy::ParityCopies, &a)
    .unwrap();
    let view = trace
        .with_assignment(&OriginalTraceAssignment::default(), &a)
        .unwrap();
    let unknown = Parity {
        records: vec!["unknown".repeat(160)],
        inverted: false,
    };
    for (keep, events) in [
        (vec![unknown.clone()], vec![]),
        (
            vec![],
            vec![Event {
                parity: unknown,
                value: false,
            }],
        ),
    ] {
        assert!(matches!(
            view.probability(&keep, &events, &o, &a),
            Err(OriginalError::ResourceLimit {
                option: "max_metadata_bytes",
                limit: 1024,
                ..
            })
        ));
    }
    let q = view.probability(&[parity(&["a"])], &[], &o, &a).unwrap();
    assert_eq!(
        q.distribution.value(&bits(1, 0), &a).unwrap(),
        ExactScalar::one()
    );
    assert_eq!(
        q.distribution.value(&bits(1, 1), &a).unwrap(),
        ExactScalar::zero()
    );
}

#[test]
fn original_prepared_support_offsets_redundancies_and_budget_failures() {
    use rmagna::instrument::OriginalTracePreparationLimits;
    use rmagna::records::{CompletedRecordError, CompletedRecordSpec, PreparedRecordLimits};
    let a = ScalarArithmetic::default();
    let o = OriginalTraceOptions::default();
    let instrument = ir(
        vec![0],
        vec![],
        plus(1),
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
        OriginalLimits::default(),
    );
    for strategy in strategies() {
        let trace = instrument.compile_trace(strategy, &a).unwrap();
        for (flip, inverted) in [(false, false), (true, false), (false, true), (true, true)] {
            let assignment = OriginalTraceAssignment {
                report_flips: if flip { vec!["a".into()] } else { vec![] },
                ..Default::default()
            };
            let view = trace.with_assignment(&assignment, &a).unwrap();
            let event = Event {
                parity: Parity {
                    inverted,
                    ..parity(&["a"])
                },
                value: false,
            };
            let events = [event.clone(), event];
            let plan = view
                .prepare_records(&[], &events, &o, Default::default(), &a)
                .unwrap();
            assert_eq!(plan.side_statistics().quotient_rank, 1);
            assert_eq!(plan.side_statistics().side_entries, 2);
            assert_eq!(plan.statistics().ordered_pairs, 4);
            assert_eq!(plan.statistics().support_factorizations, 4);
            assert_eq!(plan.statistics().pairing_factorizations, 4);
            let mut masses = Vec::new();
            for bit in [false, true, false] {
                let values = bits(2, if bit { 3 } else { 0 });
                let q = plan.probability(&values, &a).unwrap();
                assert_eq!(q.resource_expansion(), "inherited-product");
                assert_eq!(q.work.support_factorizations, 0);
                assert_eq!(q.work.pairing_factorizations, 0);
                assert_eq!(q.work.support_plan_applications, 4);
                assert!(q.work.pairing_plan_applications > 0);
                let value = q.distribution.total(&a).unwrap();
                let true_bit = bit ^ flip ^ inverted;
                let s: i64 = if true_bit { -1 } else { 1 };
                assert_eq!(value, a.scalar([2, s, 0, -s].map(Into::into), -2).unwrap());
                let old = instrument
                    .compile(
                        &OriginalAssignment {
                            true_values: vec![("a".into(), true_bit)],
                            report_flips: assignment.report_flips.clone(),
                            ..Default::default()
                        },
                        strategy,
                        &a,
                    )
                    .unwrap();
                assert_eq!(value, old.probability(&a).unwrap());
                masses.push(value);
            }
            assert_eq!(a.add(&masses[0], &masses[1]).unwrap(), ExactScalar::one());
            assert_eq!(masses[0], masses[2]);
            assert_eq!(
                plan.probability(&bits(2, 1), &a)
                    .unwrap()
                    .distribution
                    .total(&a)
                    .unwrap(),
                ExactScalar::zero()
            );
            assert!(matches!(
                plan.probability(&bits(1, 0), &a),
                Err(OriginalError::DimensionMismatch)
            ));
            assert!(matches!(
                plan.output_moment(&bits(2, 0), &pword(1, 0, 0), &a),
                Err(OriginalError::DimensionMismatch)
            ));
            assert!(matches!(
                plan.output_moment(
                    &bits(2, 0),
                    &PauliWord::new(bits(0, 0), bits(0, 0), 1).unwrap(),
                    &a
                ),
                Err(OriginalError::Completed(
                    CompletedRecordError::NonHermitianPauli
                ))
            ));
            assert!(matches!(
                view.prepare_records_with_limits(
                    &[],
                    &events,
                    &o,
                    OriginalTracePreparationLimits {
                        max_source_quotient_rank: 0,
                        ..Default::default()
                    },
                    &a
                ),
                Err(OriginalError::Unitary(
                    rmagna::unitary::UnitaryError::ExpansionResourceLimit {
                        option: "max_source_quotient_rank",
                        required: 1,
                        ..
                    }
                ))
            ));
            assert!(matches!(
                view.prepare_records(
                    &[],
                    &[],
                    &o,
                    PreparedRecordLimits {
                        max_reduced_pair_paths: 0,
                        ..Default::default()
                    },
                    &a
                ),
                Err(OriginalError::Completed(
                    CompletedRecordError::ResourceLimit {
                        option: "max_reduced_pair_paths",
                        ..
                    }
                ))
            ));
            assert!(matches!(
                view.prepare_records(
                    &[],
                    &events,
                    &o,
                    PreparedRecordLimits {
                        max_geometry_words: 0,
                        ..Default::default()
                    },
                    &a
                ),
                Err(OriginalError::Completed(
                    CompletedRecordError::ResourceLimit {
                        option: "max_prepared_geometry_words",
                        ..
                    }
                ))
            ));
            assert_eq!(
                plan.probability(&bits(2, 0), &a)
                    .unwrap()
                    .distribution
                    .total(&a)
                    .unwrap(),
                masses[0]
            );
            // Rust's bit-vector type rejects a nonbinary/out-of-width encoding.
            assert!(BitVector::from_words(1, vec![2]).is_err());
            let side = view.expansion(&o, &a).unwrap();
            let row = bits(trace.physical_width(), 1);
            let spec = CompletedRecordSpec::new(
                trace.physical_width(),
                &[],
                &[row],
                &bits(1, 0),
                Default::default(),
            )
            .unwrap();
            let physical = spec
                .prepare(side.entries(), Default::default(), &a)
                .unwrap();
            assert!(matches!(
                physical.moment(&bits(1, 0), &pword(trace.physical_width(), 1, 0), &a),
                Err(CompletedRecordError::NonCommutingPauli {
                    kind: "event",
                    index: 0
                })
            ));
            assert!(matches!(
                physical.probability(&bits(2, 0), &a),
                Err(CompletedRecordError::DimensionMismatch)
            ));
        }
    }
}
