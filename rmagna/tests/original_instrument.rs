//! Complete original UnifiedTests witnesses and independent literal amplitudes.
use rmagna::clifford::CliffordCircuit;
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    OriginalAssignment as Assignment, OriginalCallbackError as CallbackError,
    OriginalControl as Control, OriginalError as Error, OriginalFault as Fault,
    OriginalGate as Gate, OriginalInstrument as Instrument, OriginalLimits as Limits,
    OriginalOperation as Operation, OriginalOrigin as Origin, OriginalPauli as P, ReadoutBasis,
};
use rmagna::pauli::PauliWord;
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::{CompilerStrategy, UnitaryError};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn bits(width: usize, mask: usize) -> BitVector {
    BitVector::from_indices(
        width,
        (0..width.min(usize::BITS as usize)).filter(|j| mask >> j & 1 != 0),
    )
    .unwrap()
}
fn zero(width: usize) -> AffineKet {
    CliffordCircuit::computational(bits(width, 0)).unwrap()
}
fn op(id: &str, gate: Gate) -> Operation {
    Operation {
        id: id.into(),
        gate,
    }
}
fn phase(wires: &[i64], turns: i64) -> Gate {
    Gate::Phase {
        wires: wires.to_vec(),
        turns,
    }
}
fn assignment(values: &[(&str, bool)]) -> Assignment {
    Assignment {
        true_values: values.iter().map(|(n, v)| (n.to_string(), *v)).collect(),
        ..Default::default()
    }
}
fn scalar(a: &ScalarArithmetic, c: [i32; 4], power: i64) -> ExactScalar {
    a.scalar(c.map(Into::into), power).unwrap()
}
fn strategies() -> [CompilerStrategy; 2] {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
}
fn sum_amplitude(terms: &[AffineKet], word: &BitVector, a: &ScalarArithmetic) -> ExactScalar {
    terms.iter().fold(ExactScalar::zero(), |v, k| {
        a.add(&v, &k.prepare().unwrap().amplitude(word, a).unwrap())
            .unwrap()
    })
}
fn input_small(a: &ScalarArithmetic) -> AffineKet {
    AffineKet::new(
        bits(2, 1),
        vec![bits(2, 1), bits(2, 2)],
        QuadraticForm::new(0, vec![1, 3], vec![bits(2, 2), bits(2, 0)]).unwrap(),
        a.omega(3, -2).unwrap(),
    )
    .unwrap()
}
fn small(a: &ScalarArithmetic) -> Instrument {
    Instrument::new(
        vec![9, 2],
        vec![9],
        input_small(a),
        vec![
            op("0", Gate::H(9)),
            op("1", phase(&[9], 1)),
            op("2", phase(&[2], -2)),
            op("3", Gate::Cx(9, 2)),
            op(
                "4",
                Gate::Pauli {
                    wire: 9,
                    pauli: P::Y,
                },
            ),
            op(
                "5",
                Gate::Measure {
                    factors: vec![(9, P::Y), (2, P::X)],
                    inverted: false,
                    record: "a".into(),
                },
            ),
            op("6", Gate::Control),
            op(
                "7",
                Gate::Measure {
                    factors: vec![(9, P::Z)],
                    inverted: false,
                    record: "b".into(),
                },
            ),
            op(
                "8",
                Gate::Reset {
                    wire: 2,
                    plus: true,
                },
            ),
            op("9", phase(&[2], -1)),
            op(
                "10",
                Gate::Destructive {
                    wire: 2,
                    basis: ReadoutBasis::X,
                    record: "c".into(),
                },
            ),
        ],
        vec![Control {
            operation: "6".into(),
            parents: vec!["a".into()],
            version: "test-v1".into(),
            callback: Arc::new(|p| Ok(vec![phase(&[9], 1 + 2 * i64::from(p.get("a")?))])),
        }],
        Limits::default(),
        a,
    )
    .unwrap()
}

#[test]
fn original_small_instruments_all_records() {
    let a = ScalarArithmetic::default();
    let ir = small(&a);
    // Literal vectors in output9, old2-environment order. Each row is scaled
    // by 2^-4. Derived with four-component rational cyclotomic matrix algebra,
    // without affine/quadratic, GF(2), path or native compiler evaluation.
    let expected = [
        [[0, -2, 0, 2], [0, 0, 0, 0], [-4, -2, 0, 2], [0, 0, 0, 0]],
        [[0, 2, -4, 2], [0, 0, 0, 0], [0, -2, 0, -2], [0, 0, 0, 0]],
        [[0, 0, 0, 0], [2, 4, 2, 0], [0, 0, 0, 0], [2, 0, 2, 0]],
        [[0, 0, 0, 0], [-2, 0, 2, 0], [0, 0, 0, 0], [2, 0, -2, 4]],
        [[0, 2, 4, 2], [0, 0, 0, 0], [0, -2, 0, -2], [0, 0, 0, 0]],
        [[0, -2, 0, 2], [0, 0, 0, 0], [4, -2, 0, 2], [0, 0, 0, 0]],
        [[0, 0, 0, 0], [-2, 0, -2, 0], [0, 0, 0, 0], [2, 4, 2, 0]],
        [[0, 0, 0, 0], [-2, 0, 2, -4], [0, 0, 0, 0], [-2, 0, 2, 0]],
    ];
    for strategy in strategies() {
        let mut total = ExactScalar::zero();
        for (i, vector) in expected.iter().enumerate() {
            let mut v = assignment(&[("a", i & 4 != 0), ("b", i & 2 != 0), ("c", i & 1 != 0)]);
            v.report_flips.push("a".into());
            let branch = ir.compile(&v, strategy, &a).unwrap();
            assert_eq!(branch.outputs(), &[9]);
            assert_eq!(branch.width(), 2);
            assert_eq!(
                branch
                    .environment()
                    .iter()
                    .map(|e| e.wire)
                    .collect::<Vec<_>>(),
                [2]
            );
            assert_eq!(
                branch.environment()[0].retired_by,
                Origin::Operation("8".into())
            );
            assert_eq!(
                branch.decisions()[0].inputs.entries(),
                &[("a".into(), i & 4 == 0)]
            );
            assert_eq!(
                branch
                    .records()
                    .iter()
                    .map(|r| (r.true_value, r.reported_value))
                    .collect::<Vec<_>>(),
                [
                    (i & 4 != 0, i & 4 == 0),
                    (i & 2 != 0, i & 2 != 0),
                    (i & 1 != 0, i & 1 != 0)
                ]
            );
            let terms = branch.expansion(&a).unwrap();
            let mut norm = ExactScalar::zero();
            for (word, coeff) in vector.iter().enumerate() {
                let expected = scalar(&a, *coeff, -4);
                assert_eq!(
                    branch.amplitude(&bits(2, word), 65536, &a).unwrap(),
                    expected,
                    "assignment={i},word={word},strategy={strategy:?}"
                );
                assert_eq!(sum_amplitude(&terms, &bits(2, word), &a), expected);
                norm = a.add(&norm, &a.norm_squared(&expected).unwrap()).unwrap();
            }
            assert_eq!(branch.probability(&a).unwrap(), norm);
            total = a.add(&total, &norm).unwrap();
            let z = PauliWord::new(bits(1, 0), bits(1, 1), 0).unwrap();
            let signed = if i & 2 == 0 {
                norm.clone()
            } else {
                a.subtract(&ExactScalar::zero(), &norm).unwrap()
            };
            assert_eq!(branch.output_moment(&z, &a).unwrap(), signed);
            // The completed output Pauli need not commute with the old MZ row.
            assert_eq!(
                branch
                    .output_moment(&PauliWord::new(bits(1, 1), bits(1, 0), 0).unwrap(), &a)
                    .unwrap(),
                ExactScalar::zero()
            );
        }
        assert_eq!(total, ExactScalar::one());
    }
}

#[test]
fn original_output_environment_observables() {
    let a = ScalarArithmetic::default();
    let ir = Instrument::new(
        vec![0, 1],
        vec![0, 1],
        zero(2),
        vec![
            op("h", Gate::H(0)),
            op("cx", Gate::Cx(0, 1)),
            op(
                "reset",
                Gate::Reset {
                    wire: 1,
                    plus: false,
                },
            ),
        ],
        vec![],
        Limits::default(),
        &a,
    )
    .unwrap();
    for strategy in strategies() {
        let branch = ir.compile(&Assignment::default(), strategy, &a).unwrap();
        let terms = branch.expansion(&a).unwrap();
        assert_eq!(branch.width(), 3);
        assert_eq!(branch.environment()[0].wire, 1);
        for word in 0..8 {
            let expected = if word == 0 || word == 5 {
                a.omega(0, -1).unwrap()
            } else {
                ExactScalar::zero()
            };
            assert_eq!(
                branch.amplitude(&bits(3, word), 1024, &a).unwrap(),
                expected
            );
            assert_eq!(sum_amplitude(&terms, &bits(3, word), &a), expected);
        }
        assert_eq!(branch.probability(&a).unwrap(), ExactScalar::one());
        assert_eq!(
            branch
                .output_moment(&PauliWord::new(bits(2, 0), bits(2, 1), 0).unwrap(), &a)
                .unwrap(),
            ExactScalar::zero()
        );
        assert_eq!(
            branch.target_weight(&[zero(2)], &a).unwrap(),
            scalar(&a, [1, 0, 0, 0], -1)
        );
        let plus = AffineKet::new(
            bits(2, 0),
            vec![bits(2, 1), bits(2, 2)],
            QuadraticForm::new(0, vec![0, 0], vec![bits(2, 0); 2]).unwrap(),
            a.omega(0, -2).unwrap(),
        )
        .unwrap();
        assert_eq!(
            branch
                .target_weight(std::slice::from_ref(&plus), &a)
                .unwrap(),
            scalar(&a, [1, 0, 0, 0], -2)
        );
        let minus = AffineKet::new(
            plus.offset().clone(),
            plus.columns().to_vec(),
            plus.phase().clone(),
            a.subtract(&ExactScalar::zero(), plus.coefficient())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            branch.target_weight(&[plus, minus], &a).unwrap(),
            ExactScalar::zero()
        );
        let sub = AffineKet::new(
            bits(2, 0),
            vec![],
            QuadraticForm::new(0, vec![], vec![]).unwrap(),
            a.omega(3, -2).unwrap(),
        )
        .unwrap();
        assert_eq!(
            branch.target_weight(&[sub], &a).unwrap(),
            scalar(&a, [1, 0, 0, 0], -3)
        );
        assert!(matches!(
            branch.output_moment(&PauliWord::new(bits(3, 4), bits(3, 0), 0).unwrap(), &a),
            Err(Error::DimensionMismatch)
        ));
        assert!(matches!(
            branch.target_weight(&[zero(1)], &a),
            Err(Error::DimensionMismatch)
        ));
        assert!(matches!(
            branch.amplitude(&bits(2, 0), 1024, &a),
            Err(Error::DimensionMismatch)
        ));
    }
}

fn counted_control(name: &str, parents: &[&str], calls: &Arc<AtomicUsize>) -> Control {
    let calls = calls.clone();
    Control {
        operation: name.into(),
        parents: parents.iter().map(|p| p.to_string()).collect(),
        version: "same-version".into(),
        callback: Arc::new(move |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(vec![])
        }),
    }
}
fn admission_ir(calls: &Arc<AtomicUsize>, a: &ScalarArithmetic) -> Instrument {
    Instrument::new(
        vec![9],
        vec![9],
        zero(1),
        vec![
            op("c", Gate::Control),
            op(
                "m",
                Gate::Measure {
                    factors: vec![(9, P::Z)],
                    inverted: false,
                    record: "r".into(),
                },
            ),
        ],
        vec![counted_control("c", &[], calls)],
        Limits::default(),
        a,
    )
    .unwrap()
}
#[test]
fn assignment_admission_precedes_every_callback_and_remains_reusable() {
    let a = ScalarArithmetic::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let ir = admission_ir(&calls, &a);
    let good = assignment(&[("r", false)]);
    let bad = [
        Assignment::default(),
        assignment(&[("r", false), ("r", true)]),
        assignment(&[("other", false)]),
        Assignment {
            report_flips: vec!["unknown".into()],
            ..good.clone()
        },
        Assignment {
            faults: vec![Fault {
                after: "unknown".into(),
                wire: 9,
                pauli: P::I,
            }],
            ..good.clone()
        },
    ];
    for v in bad {
        assert!(ir.compile(&v, strategies()[0], &a).is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
    let first = ir.compile(&good, strategies()[0], &a).unwrap();
    let mut flipped = good.clone();
    flipped.report_flips = vec!["r".into(), "r".into()];
    let second = ir.compile(&flipped, strategies()[0], &a).unwrap();
    assert!(!second.records()[0].reported_value);
    assert_eq!(first.probability(&a).unwrap(), ExactScalar::one());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let impossible = ir
        .compile(&assignment(&[("r", true)]), strategies()[0], &a)
        .unwrap();
    assert_eq!(impossible.probability(&a).unwrap(), ExactScalar::zero());
    assert_eq!(
        impossible.amplitude(&bits(1, 0), 1024, &a).unwrap(),
        ExactScalar::zero()
    );
    assert_eq!(
        ir.compile(&good, strategies()[0], &a)
            .unwrap()
            .probability(&a)
            .unwrap(),
        ExactScalar::one()
    );
}

#[test]
fn identities_initial_state_and_parents_are_admitted_before_callbacks() {
    let a = ScalarArithmetic::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let create = |inputs, outputs, initial, ops, controls| {
        Instrument::new(
            inputs,
            outputs,
            initial,
            ops,
            controls,
            Limits::default(),
            &a,
        )
    };
    assert!(create(vec![0, 0], vec![0], zero(2), vec![], vec![]).is_err());
    assert!(create(vec![0], vec![0, 0], zero(1), vec![], vec![]).is_err());
    assert!(create(vec![0], vec![0], zero(2), vec![], vec![]).is_err());
    for coefficient in [ExactScalar::zero(), a.omega(0, -1).unwrap()] {
        let initial = AffineKet::new(
            bits(1, 0),
            vec![],
            QuadraticForm::new(0, vec![], vec![]).unwrap(),
            coefficient,
        )
        .unwrap();
        assert!(
            create(
                vec![0],
                vec![0],
                initial,
                vec![op("c", Gate::Control)],
                vec![counted_control("c", &[], &calls)]
            )
            .is_err()
        );
    }
    let dependent = AffineKet::new(
        bits(1, 0),
        vec![bits(1, 1), bits(1, 1)],
        QuadraticForm::new(0, vec![0, 0], vec![bits(2, 0); 2]).unwrap(),
        a.omega(0, -2).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        create(vec![0], vec![0], dependent, vec![], vec![]),
        Err(Error::Unitary(UnitaryError::DependentStabilizerInput))
    ));
    let measure = || Gate::Measure {
        factors: vec![(0, P::Z)],
        inverted: false,
        record: "r".into(),
    };
    for ops in [
        vec![op("", Gate::Identity(vec![]))],
        vec![op("same", Gate::H(0)), op("same", Gate::H(0))],
        vec![op("a", measure()), op("b", measure())],
        vec![op("a", Gate::Cx(0, 0))],
        vec![op("a", phase(&[0, 0], 1))],
        vec![op(
            "a",
            Gate::Measure {
                factors: vec![(0, P::I)],
                inverted: false,
                record: "r".into(),
            },
        )],
    ] {
        assert!(create(vec![0], vec![0], zero(1), ops, vec![]).is_err());
    }
    assert!(
        create(
            vec![0],
            vec![0],
            zero(1),
            vec![op("c", Gate::Control)],
            vec![]
        )
        .is_err()
    );
    assert!(
        create(
            vec![0],
            vec![0],
            zero(1),
            vec![],
            vec![counted_control("c", &[], &calls)]
        )
        .is_err()
    );
    let c = counted_control("c", &[], &calls);
    assert!(
        create(
            vec![0],
            vec![0],
            zero(1),
            vec![op("c", Gate::Control)],
            vec![c.clone(), c.clone()]
        )
        .is_err()
    );
    let mut c = c;
    c.version.clear();
    assert!(
        create(
            vec![0],
            vec![0],
            zero(1),
            vec![op("c", Gate::Control)],
            vec![c]
        )
        .is_err()
    );
    assert!(
        create(
            vec![0],
            vec![0],
            zero(1),
            vec![op("c", Gate::Control), op("m", measure())],
            vec![counted_control("c", &["r"], &calls)]
        )
        .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn callbacks_are_fresh_ordered_scoped_and_previous_branches_are_frozen() {
    let a = ScalarArithmetic::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let n = calls.clone();
    let control = Control {
        operation: "c".into(),
        parents: vec!["r".into(), "r".into()],
        version: "unchanged".into(),
        callback: Arc::new(move |p| {
            assert_eq!(p.entries(), &[("r".into(), true), ("r".into(), true)]);
            assert_eq!(
                p.get("hidden"),
                Err(CallbackError::UndeclaredParent("hidden".into()))
            );
            let turn = n.fetch_add(1, Ordering::SeqCst) as i64;
            Ok(vec![phase(&[9], turn)])
        }),
    };
    let initial = CliffordCircuit::computational(bits(1, 1)).unwrap();
    let ir = Instrument::new(
        vec![9],
        vec![9],
        initial,
        vec![
            op(
                "m",
                Gate::Measure {
                    factors: vec![(9, P::Z)],
                    inverted: false,
                    record: "r".into(),
                },
            ),
            op("c", Gate::Control),
        ],
        vec![control],
        Limits::default(),
        &a,
    )
    .unwrap();
    let v = assignment(&[("r", true)]);
    let first = ir.compile(&v, strategies()[0], &a).unwrap();
    let second = ir.compile(&v, strategies()[0], &a).unwrap();
    assert_eq!(
        first.amplitude(&bits(1, 1), 1024, &a).unwrap(),
        ExactScalar::one()
    );
    assert_eq!(
        second.amplitude(&bits(1, 1), 1024, &a).unwrap(),
        a.omega(1, 0).unwrap()
    );
    assert_eq!(first.probability(&a).unwrap(), ExactScalar::one());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(first.decisions()[0].inputs.entries().len(), 2);
}

#[test]
fn callbacks_can_change_lifetimes_and_preserve_chronological_environments() {
    let a = ScalarArithmetic::default();
    let control = Control {
        operation: "c".into(),
        parents: vec![],
        version: "v".into(),
        callback: Arc::new(|_| {
            Ok(vec![
                Gate::Reset {
                    wire: -9,
                    plus: true,
                },
                Gate::Discard(2),
                Gate::Prepare {
                    wire: 2,
                    plus: false,
                },
                Gate::Discard(-9),
                Gate::Prepare {
                    wire: -9,
                    plus: false,
                },
                Gate::Pauli {
                    wire: -9,
                    pauli: P::X,
                },
            ])
        }),
    };
    let ir = Instrument::new(
        vec![-9, 2],
        vec![2, -9],
        zero(2),
        vec![op("c", Gate::Control)],
        vec![control],
        Limits::default(),
        &a,
    )
    .unwrap();
    for s in strategies() {
        let b = ir.compile(&Assignment::default(), s, &a).unwrap();
        assert_eq!(b.outputs(), &[2, -9]);
        assert_eq!(
            b.environment().iter().map(|e| e.wire).collect::<Vec<_>>(),
            [-9, 2, -9]
        );
        assert_eq!(
            b.environment()[0].retired_by,
            Origin::ControlEmission {
                control: "c".into(),
                ordinal: 0
            }
        );
        assert_eq!(b.width(), 5);
        assert_eq!(b.probability(&a).unwrap(), ExactScalar::one());
        let terms = b.expansion(&a).unwrap();
        for word in 0..32 {
            let expected = if word == 2 || word == 18 {
                a.omega(0, -1).unwrap()
            } else {
                ExactScalar::zero()
            };
            assert_eq!(b.amplitude(&bits(5, word), 1024, &a).unwrap(), expected);
            assert_eq!(sum_amplitude(&terms, &bits(5, word), &a), expected);
        }
    }
}

#[test]
fn ordered_faults_identity_faults_and_generated_looking_user_ids() {
    let a = ScalarArithmetic::default();
    let ir = Instrument::new(
        vec![9],
        vec![9],
        zero(1),
        vec![
            op("c/0", Gate::Identity(vec![])),
            op("c", Gate::Control),
            op("c/fault/0", Gate::Identity(vec![9])),
        ],
        vec![Control {
            operation: "c".into(),
            parents: vec![],
            version: "v".into(),
            callback: Arc::new(|_| Ok(vec![Gate::Identity(vec![])])),
        }],
        Limits::default(),
        &a,
    )
    .unwrap();
    for (ps, exponent) in [([P::Y, P::Z], 6), ([P::Z, P::Y], 2)] {
        let v = Assignment {
            faults: ps
                .into_iter()
                .map(|pauli| Fault {
                    after: "c".into(),
                    wire: 9,
                    pauli,
                })
                .collect(),
            ..Default::default()
        };
        let b = ir.compile(&v, strategies()[0], &a).unwrap();
        assert_eq!(
            b.amplitude(&bits(1, 1), 1024, &a).unwrap(),
            a.omega(exponent, 0).unwrap()
        );
        assert_eq!(
            b.origins(),
            &[
                Origin::Operation("c/0".into()),
                Origin::Operation("c".into()),
                Origin::ControlEmission {
                    control: "c".into(),
                    ordinal: 0
                },
                Origin::Fault {
                    after: "c".into(),
                    ordinal: 0
                },
                Origin::Fault {
                    after: "c".into(),
                    ordinal: 1
                },
                Origin::Operation("c/fault/0".into())
            ]
        );
    }
    let ir = Instrument::new(
        vec![9],
        vec![],
        zero(1),
        vec![op(
            "d",
            Gate::Destructive {
                wire: 9,
                basis: ReadoutBasis::Z,
                record: "r".into(),
            },
        )],
        vec![],
        Limits::default(),
        &a,
    )
    .unwrap();
    let mut v = assignment(&[("r", false)]);
    v.faults.push(Fault {
        after: "d".into(),
        wire: 999,
        pauli: P::I,
    });
    let b = ir.compile(&v, strategies()[0], &a).unwrap();
    assert_eq!(b.width(), 0);
    assert!(b.environment().is_empty());
    assert_eq!(
        b.amplitude(&bits(0, 0), 1024, &a).unwrap(),
        ExactScalar::one()
    );
    v.faults[0].pauli = P::Y;
    assert!(matches!(
        ir.compile(&v, strategies()[0], &a),
        Err(Error::Lifetime { .. })
    ));
    v.faults.clear();
    assert_eq!(
        ir.compile(&v, strategies()[0], &a)
            .unwrap()
            .probability(&a)
            .unwrap(),
        ExactScalar::one()
    );
}

#[test]
fn signed_empty_and_y_measurements_and_destructive_x_preserve_phases() {
    let a = ScalarArithmetic::default();
    for (factors, inverted, input_phase, expected_bit) in [
        (vec![], false, None, false),
        (vec![], true, None, true),
        (vec![(9, P::Y)], false, Some(2), false),
        (vec![(9, P::Y)], true, Some(2), true),
    ] {
        let input = if let Some(p) = input_phase {
            AffineKet::new(
                bits(1, 0),
                vec![bits(1, 1)],
                QuadraticForm::new(0, vec![1], vec![bits(1, 0)]).unwrap(),
                a.omega(p - 2, -1).unwrap(),
            )
            .unwrap()
        } else {
            zero(1)
        };
        let ir = Instrument::new(
            vec![9],
            vec![9],
            input,
            vec![op(
                "m",
                Gate::Measure {
                    factors,
                    inverted,
                    record: "r".into(),
                },
            )],
            vec![],
            Limits::default(),
            &a,
        )
        .unwrap();
        for s in strategies() {
            for bit in [false, true] {
                let b = ir.compile(&assignment(&[("r", bit)]), s, &a).unwrap();
                assert_eq!(
                    b.probability(&a).unwrap(),
                    if bit == expected_bit {
                        ExactScalar::one()
                    } else {
                        ExactScalar::zero()
                    }
                );
            }
        }
    }
    let ir = Instrument::new(
        vec![9],
        vec![],
        CliffordCircuit::computational(bits(1, 1)).unwrap(),
        vec![op(
            "x",
            Gate::Destructive {
                wire: 9,
                basis: ReadoutBasis::X,
                record: "r".into(),
            },
        )],
        vec![],
        Limits::default(),
        &a,
    )
    .unwrap();
    for bit in [false, true] {
        let b = ir
            .compile(&assignment(&[("r", bit)]), strategies()[0], &a)
            .unwrap();
        assert_eq!(
            b.amplitude(&bits(0, 0), 1024, &a).unwrap(),
            a.omega(if bit { 4 } else { 0 }, -1).unwrap()
        );
        assert_eq!(b.probability(&a).unwrap(), scalar(&a, [1, 0, 0, 0], -1));
    }
}

#[test]
fn parity_phase_ccz_input_order_and_global_phase_are_exact() {
    let a = ScalarArithmetic::default();
    for turn in [i64::MIN, -9, -2, -1, 0, 1, 2, 7, 8, 9] {
        for word in 0..8 {
            for s in strategies() {
                let input = AffineKet::new(
                    bits(3, word),
                    vec![],
                    QuadraticForm::new(0, vec![], vec![]).unwrap(),
                    a.omega(3, 0).unwrap(),
                )
                .unwrap();
                let ir = Instrument::new(
                    vec![90, -2, 7],
                    vec![7, 90, -2],
                    input,
                    vec![
                        op("empty", phase(&[], turn)),
                        op("p", phase(&[90, 7], turn)),
                        op("ccz", Gate::Ccz(90, -2, 7)),
                    ],
                    vec![],
                    Limits::default(),
                    &a,
                )
                .unwrap();
                let b = ir.compile(&Assignment::default(), s, &a).unwrap();
                let output = ((word >> 2) & 1) | ((word & 1) << 1) | ((word & 2) << 1);
                let exponent = 3
                    + turn.rem_euclid(8) * i64::from(((word & 1) ^ ((word >> 2) & 1)) != 0)
                    + if word == 7 { 4 } else { 0 };
                assert_eq!(
                    b.amplitude(&bits(3, output), 1024, &a).unwrap(),
                    a.omega(exponent, 0).unwrap()
                );
                assert_eq!(b.probability(&a).unwrap(), ExactScalar::one());
            }
        }
    }
}

#[test]
fn lifetime_and_output_errors_are_typed_and_callbacks_can_recover() {
    let a = ScalarArithmetic::default();
    for gates in [
        vec![Gate::H(999)],
        vec![Gate::Prepare {
            wire: 9,
            plus: false,
        }],
        vec![Gate::Discard(9), Gate::H(9)],
        vec![Gate::Identity(vec![999])],
    ] {
        let ir = Instrument::new(
            vec![9],
            vec![9],
            zero(1),
            gates
                .into_iter()
                .enumerate()
                .map(|(i, g)| op(&i.to_string(), g))
                .collect(),
            vec![],
            Limits::default(),
            &a,
        )
        .unwrap();
        assert!(matches!(
            ir.compile(&Assignment::default(), strategies()[0], &a),
            Err(Error::Lifetime { .. })
        ));
    }
    let ir = Instrument::new(
        vec![9],
        vec![],
        zero(1),
        vec![],
        vec![],
        Limits::default(),
        &a,
    )
    .unwrap();
    assert!(matches!(
        ir.compile(&Assignment::default(), strategies()[0], &a),
        Err(Error::OutputMismatch)
    ));
    let calls = Arc::new(AtomicUsize::new(0));
    let n = calls.clone();
    let c = Control {
        operation: "c".into(),
        parents: vec![],
        version: "v".into(),
        callback: Arc::new(move |_| match n.fetch_add(1, Ordering::SeqCst) {
            0 => Err(CallbackError::Failed("intentional".into())),
            1 => Ok(vec![Gate::Measure {
                factors: vec![],
                inverted: false,
                record: "x".into(),
            }]),
            2 => Ok(vec![Gate::Control]),
            3 => Ok(vec![Gate::Discard(9)]),
            4 => Ok(vec![Gate::Cx(9, 9)]),
            _ => Ok(vec![]),
        }),
    };
    let ir = Instrument::new(
        vec![9],
        vec![9],
        zero(1),
        vec![op("c", Gate::Control)],
        vec![c],
        Limits::default(),
        &a,
    )
    .unwrap();
    assert!(matches!(
        ir.compile(&Assignment::default(), strategies()[0], &a),
        Err(Error::Callback { .. })
    ));
    for _ in 0..4 {
        assert!(
            ir.compile(&Assignment::default(), strategies()[0], &a)
                .is_err()
        );
    }
    assert_eq!(
        ir.compile(&Assignment::default(), strategies()[0], &a)
            .unwrap()
            .probability(&a)
            .unwrap(),
        ExactScalar::one()
    );
}

#[test]
fn finite_quotas_reject_before_callbacks_or_lowering_and_allow_reuse() {
    let a = ScalarArithmetic::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let c = counted_control("c", &[], &calls);
    for l in [
        Limits {
            max_operations: 0,
            ..Limits::default()
        },
        Limits {
            max_metadata_bytes: 1,
            ..Limits::default()
        },
        Limits {
            max_physical_slots: 0,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            Instrument::new(
                vec![9],
                vec![9],
                zero(1),
                vec![op("c", Gate::Control)],
                vec![c.clone()],
                l,
                &a
            ),
            Err(Error::ResourceLimit { .. })
        ));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let calls2 = Arc::new(AtomicUsize::new(0));
    let n = calls2.clone();
    let c = Control {
        operation: "c".into(),
        parents: vec![],
        version: "v".into(),
        callback: Arc::new(move |_| {
            if n.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(vec![Gate::H(9); 2])
            } else {
                Ok(vec![])
            }
        }),
    };
    let ir = Instrument::new(
        vec![9],
        vec![9],
        zero(1),
        vec![op("c", Gate::Control)],
        vec![c],
        Limits {
            max_emitted_operations: 1,
            max_faults: 0,
            ..Limits::default()
        },
        &a,
    )
    .unwrap();
    let v = Assignment {
        faults: vec![Fault {
            after: "c".into(),
            wire: 9,
            pauli: P::I,
        }],
        ..Default::default()
    };
    assert!(ir.compile(&v, strategies()[0], &a).is_err());
    assert_eq!(calls2.load(Ordering::SeqCst), 0);
    assert!(matches!(
        ir.compile(&Assignment::default(), strategies()[0], &a),
        Err(Error::ResourceLimit {
            option: "max_emitted_operations",
            ..
        })
    ));
    assert_eq!(
        ir.compile(&Assignment::default(), strategies()[0], &a)
            .unwrap()
            .probability(&a)
            .unwrap(),
        ExactScalar::one()
    );
    for l in [
        Limits {
            max_gates: 0,
            ..Limits::default()
        },
        Limits {
            max_path_bits: 0,
            ..Limits::default()
        },
    ] {
        let ir = Instrument::new(
            vec![9],
            vec![9],
            zero(1),
            vec![op("h", Gate::H(9))],
            vec![],
            l,
            &a,
        )
        .unwrap();
        assert!(
            ir.compile(&Assignment::default(), strategies()[0], &a)
                .is_err()
        );
    }
    let ir = Instrument::new(
        vec![9],
        vec![9],
        zero(1),
        vec![op(
            "reset",
            Gate::Reset {
                wire: 9,
                plus: false,
            },
        )],
        vec![],
        Limits {
            max_physical_slots: 1,
            ..Limits::default()
        },
        &a,
    )
    .unwrap();
    assert!(
        ir.compile(&Assignment::default(), strategies()[0], &a)
            .is_err()
    );
    let mut l = Limits::default();
    l.expansion.max_side_terms = 0;
    let ir = Instrument::new(vec![9], vec![9], zero(1), vec![], vec![], l, &a).unwrap();
    let b = ir
        .compile(&Assignment::default(), strategies()[0], &a)
        .unwrap();
    assert!(b.expansion(&a).is_err());
    assert_eq!(
        b.amplitude(&bits(1, 0), 1024, &a).unwrap(),
        ExactScalar::one()
    );
    let vacuum = Instrument::new(
        vec![],
        vec![],
        zero(0),
        vec![],
        vec![],
        Limits::default(),
        &a,
    )
    .unwrap()
    .compile(&Assignment::default(), strategies()[0], &a)
    .unwrap();
    assert_eq!(vacuum.probability(&a).unwrap(), ExactScalar::one());
    assert_eq!(vacuum.target_weight(&[], &a).unwrap(), ExactScalar::zero());
    assert_eq!(
        vacuum.target_weight(&[zero(0)], &a).unwrap(),
        ExactScalar::one()
    );
}

#[test]
fn target_pair_geometry_and_parent_quotas_fail_explicitly() {
    let a = ScalarArithmetic::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let c = counted_control("c", &["r", "r"], &calls);
    let ops = vec![
        op(
            "m",
            Gate::Measure {
                factors: vec![],
                inverted: false,
                record: "r".into(),
            },
        ),
        op("c", Gate::Control),
    ];
    assert!(matches!(
        Instrument::new(
            vec![9],
            vec![9],
            zero(1),
            ops,
            vec![c],
            Limits {
                max_parents: 1,
                ..Limits::default()
            },
            &a
        ),
        Err(Error::ResourceLimit {
            option: "max_parents",
            ..
        })
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    for which in 0..3 {
        let mut limits = Limits::default();
        if which == 0 {
            limits.completed.max_source_entries = 1;
        }
        if which == 1 {
            limits.completed.max_pairs = 1;
        }
        let ir = Instrument::new(vec![9], vec![9], zero(1), vec![], vec![], limits, &a).unwrap();
        let b = ir
            .compile(&Assignment::default(), strategies()[0], &a)
            .unwrap();
        if which < 2 {
            assert!(b.target_weight(&[zero(1), zero(1)], &a).is_err());
        } else {
            // Large hidden target coordinates must fail before tensoring them,
            // even though their one-wire physical boundary is small.
            let rank = limits.expansion.max_path_bits + 1;
            let target = AffineKet::new(
                bits(1, 0),
                vec![bits(1, 0); rank],
                QuadraticForm::new(
                    0,
                    vec![0; rank],
                    vec![BitVector::zeros(rank).unwrap(); rank],
                )
                .unwrap(),
                ExactScalar::one(),
            )
            .unwrap();
            assert!(matches!(
                b.target_weight(&[target], &a),
                Err(Error::ResourceLimit {
                    option: "max_path_bits",
                    ..
                })
            ));
        }
        assert_eq!(b.probability(&a).unwrap(), ExactScalar::one());
    }
}
