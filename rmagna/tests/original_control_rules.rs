// SPDX-License-Identifier: MPL-2.0
// Complete DeclaredExpressionNativeTests::test_value_conversion_and_inactive_exponent
// from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_deferred_feedback.py. Expected arithmetic/payloads are literal.
use num_bigint::BigInt;
use rmagna::clifford::{CliffordCircuit, CliffordGate};
use rmagna::gf2::BitVector;
use rmagna::instrument::*;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::unitary::CompilerStrategy;
use std::collections::{BTreeMap, BTreeSet};

fn map(a: i32, b: i32) -> BTreeMap<String, i32> {
    BTreeMap::from([("a".into(), a), ("b".into(), b)])
}
fn compose(op: ControlOperator, a: ControlExpression, b: ControlExpression) -> ControlExpression {
    ControlExpression::compose(op, vec![a, b], Default::default()).unwrap()
}
#[test]
fn original_declared_expression_converts_large_values_and_skips_inactive_exponents() {
    let limits = ControlRuleLimits::default();
    let a = ControlExpression::record("a".into(), limits).unwrap();
    let b = ControlExpression::record("b".into(), limits).unwrap();
    let not_b = ControlExpression::compose(ControlOperator::Not, vec![b.clone()], limits).unwrap();
    let large: BigInt = (BigInt::from(1) << 200usize) + 3;
    let expression = compose(
        ControlOperator::Add,
        compose(
            ControlOperator::Multiply,
            ControlExpression::constant(2),
            compose(ControlOperator::Xor, a.clone(), not_b),
        ),
        ControlExpression::constant(large),
    );
    assert_eq!(expression, expression.clone());
    assert_eq!(
        expression.records(),
        &BTreeSet::from(["a".into(), "b".into()])
    );
    for x in 0..=1 {
        for y in 0..=1 {
            assert_eq!(
                expression.evaluate(&map(x, y)).unwrap(),
                (2 * (x ^ (1 - y)) + 3) % 8
            );
        }
    }
    let gate = ControlGate::new(ControlGateKind::Phase, vec![0], Some(b), a, limits).unwrap();
    let inactive = BTreeMap::from([("a".into(), i32::from(false))]);
    assert_eq!(gate.evaluate(&inactive).unwrap(), None);
    let rule = ControlRule::new(vec![gate.clone()], limits).unwrap();
    assert_eq!(
        rule.evaluate(&map(i32::from(true), 1)).unwrap(),
        vec![ControlEmission {
            kind: ControlGateKind::Phase,
            wires: vec![0],
            exponent: Some(1)
        }]
    );
    assert!(
        matches!(gate.evaluate(&BTreeMap::from([("a".into(),1)])), Err(ControlRuleError::MissingRecord(name)) if name=="b")
    );
}
#[test]
fn declared_control_arithmetic_keeps_bitwise_truth_order_and_admission_contracts() {
    let limits = ControlRuleLimits::default();
    let a = ControlExpression::record("a".into(), limits).unwrap();
    let b = ControlExpression::record("b".into(), limits).unwrap();
    for x in [i32::MIN, -9, -1, 0, 1, 2, 7, i32::MAX] {
        for y in [i32::MIN, -8, -1, 0, 1, 4, 7, i32::MAX] {
            for (op, expected) in [
                (ControlOperator::Xor, x ^ y),
                (ControlOperator::And, x & y),
                (ControlOperator::Or, x | y),
                (
                    ControlOperator::Add,
                    (i64::from(x) + i64::from(y)).rem_euclid(8) as i32,
                ),
                (
                    ControlOperator::Multiply,
                    (i64::from(x) * i64::from(y)).rem_euclid(8) as i32,
                ),
            ] {
                assert_eq!(
                    compose(op, a.clone(), b.clone())
                        .evaluate(&map(x, y))
                        .unwrap(),
                    expected
                );
            }
            assert_eq!(
                ControlExpression::compose(ControlOperator::Not, vec![a.clone()], limits)
                    .unwrap()
                    .evaluate(&map(x, y))
                    .unwrap(),
                i32::from(x == 0)
            );
        }
    }
    for value in [-1000i32, -9, -8, -1, 0, 1, 7, 8, 999] {
        assert_eq!(
            ControlExpression::constant(value)
                .evaluate(&BTreeMap::new())
                .unwrap(),
            value.rem_euclid(8)
        );
    }
    let eager = compose(
        ControlOperator::And,
        ControlExpression::constant(0),
        b.clone(),
    );
    assert!(
        matches!(eager.evaluate(&BTreeMap::new()),Err(ControlRuleError::MissingRecord(name)) if name=="b")
    );
    assert!(
        ControlExpression::compose(ControlOperator::Not, vec![a.clone(), b.clone()], limits)
            .is_err()
    );
    assert!(
        ControlExpression::compose(
            ControlOperator::Add,
            vec![a.clone(), b.clone()],
            ControlRuleLimits {
                max_expression_nodes: 2,
                ..limits
            }
        )
        .is_err()
    );
    assert!(
        ControlExpression::compose(
            ControlOperator::Add,
            vec![a.clone(), b],
            ControlRuleLimits {
                max_expression_depth: 1,
                ..limits
            }
        )
        .is_err()
    );
    assert!(
        ControlExpression::record(
            "long".into(),
            ControlRuleLimits {
                max_record_bytes: 3,
                ..limits
            }
        )
        .is_err()
    );
    assert!(
        ControlGate::new(
            ControlGateKind::X,
            vec![0],
            Some(a.clone()),
            ControlExpression::constant(1),
            limits
        )
        .is_err()
    );
    assert!(
        ControlGate::new(
            ControlGateKind::Cx,
            vec![0, 0],
            None,
            ControlExpression::constant(1),
            limits
        )
        .is_err()
    );
    assert!(
        ControlGate::new(
            ControlGateKind::Phase,
            vec![],
            None,
            ControlExpression::constant(1),
            limits
        )
        .is_err()
    );
    let x = ControlGate::new(
        ControlGateKind::X,
        vec![2],
        None,
        ControlExpression::constant(2),
        limits,
    )
    .unwrap();
    let z = ControlGate::new(
        ControlGateKind::Z,
        vec![2],
        None,
        ControlExpression::constant(1),
        limits,
    )
    .unwrap();
    assert!(
        ControlRule::new(
            vec![x.clone(), z.clone()],
            ControlRuleLimits {
                max_gates: 1,
                ..limits
            }
        )
        .is_err()
    );
    assert!(
        ControlRule::new(
            vec![x.clone(), z.clone()],
            ControlRuleLimits {
                max_rule_targets: 1,
                ..limits
            }
        )
        .is_err()
    );
    assert!(
        ControlRule::new(
            vec![x.clone(), z.clone()],
            ControlRuleLimits {
                max_rule_expression_nodes: 1,
                ..limits
            }
        )
        .is_err()
    );
    assert_eq!(
        ControlRule::new(vec![x, z], limits)
            .unwrap()
            .evaluate(&BTreeMap::new())
            .unwrap()
            .iter()
            .map(|g| g.kind)
            .collect::<Vec<_>>(),
        vec![ControlGateKind::X, ControlGateKind::Z]
    );
}
#[test]
fn declared_rule_is_retained_as_authoritative_callback_and_uses_reported_parents() {
    let l = ControlRuleLimits::default();
    let record = ControlExpression::record("a".into(), l).unwrap();
    let exponent = compose(
        ControlOperator::Add,
        ControlExpression::constant(1),
        compose(
            ControlOperator::Multiply,
            ControlExpression::constant(6),
            record,
        ),
    );
    let rule = ControlRule::new(
        vec![
            ControlGate::new(
                ControlGateKind::Phase,
                vec![1],
                Some(exponent),
                ControlExpression::constant(1),
                l,
            )
            .unwrap(),
        ],
        l,
    )
    .unwrap();
    assert!(OriginalDeclaredControl::new("c".into(), vec![], "v1".into(), rule.clone()).is_err());
    let declaration = OriginalDeclaredControl::new(
        "c".into(),
        vec!["a".into(), "a".into()],
        "v1".into(),
        rule.clone(),
    )
    .unwrap();
    assert_eq!(declaration.operation(), "c");
    assert_eq!(declaration.parents(), &["a", "a"]);
    assert_eq!(declaration.version(), "v1");
    assert_eq!(declaration.rule(), &rule);
    let a = ScalarArithmetic::default();
    let initial = CliffordCircuit::new(2, [CliffordGate::H(0), CliffordGate::H(1)])
        .unwrap()
        .evolve(
            &CliffordCircuit::computational(BitVector::zeros(2).unwrap()).unwrap(),
            &a,
        )
        .unwrap()
        .unwrap();
    let operations = vec![
        OriginalOperation {
            id: "m".into(),
            gate: OriginalGate::Measure {
                factors: vec![(0, OriginalPauli::Z)],
                inverted: false,
                record: "a".into(),
            },
        },
        OriginalOperation {
            id: "c".into(),
            gate: OriginalGate::Control,
        },
    ];
    let ir = OriginalInstrument::new_with_declared_controls(
        vec![0, 1],
        vec![1, 0],
        initial.clone(),
        operations.clone(),
        vec![declaration],
        Default::default(),
        &a,
    )
    .unwrap();
    let saved = ir.clone();
    assert_eq!(ir.control_rule("c"), Some(&rule));
    assert!(ir.control_rule("m").is_none());
    drop(rule);
    drop(ir);
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for bit in [false, true] {
            for flips in [vec![], vec!["a".into()], vec!["a".into(), "a".into()]] {
                let reported = bit ^ (!flips.len().is_multiple_of(2));
                let turns = 1 + 6 * i64::from(reported);
                let branch = saved
                    .compile(
                        &OriginalAssignment {
                            true_values: vec![("a".into(), bit)],
                            report_flips: flips,
                            faults: vec![OriginalFault {
                                after: "c".into(),
                                wire: 1,
                                pauli: OriginalPauli::Y,
                            }],
                        },
                        strategy,
                        &a,
                    )
                    .unwrap();
                assert_eq!(
                    branch.decisions()[0].inputs.entries(),
                    &[("a".into(), reported), ("a".into(), reported)]
                );
                assert_eq!(branch.decisions()[0].emitted_operations, 1);
                assert_eq!(branch.outputs(), &[1, 0]);
                assert!(
                    matches!(&branch.origins()[2],OriginalOrigin::ControlEmission{control,ordinal:0}if control=="c")
                );
                assert!(
                    matches!(&branch.origins()[3],OriginalOrigin::Fault{after,ordinal:0}if after=="c")
                );
                for label in 0..4 {
                    let want = if label >> 1 != usize::from(bit) {
                        ExactScalar::zero()
                    } else if label & 1 == 0 {
                        a.omega(turns + 6, -2).unwrap()
                    } else {
                        a.omega(2, -2).unwrap()
                    };
                    let word =
                        BitVector::from_indices(2, (0..2).filter(|j| label >> j & 1 != 0)).unwrap();
                    assert_eq!(branch.amplitude(&word, 64, &a).unwrap(), want);
                }
                assert_eq!(branch.probability(&a).unwrap(), a.omega(0, -2).unwrap());
            }
        }
    }
    // Looking up declarations must not probe or reinterpret an opaque callback.
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let opaque = OriginalInstrument::new(
        vec![0, 1],
        vec![1, 0],
        initial,
        operations,
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
    assert!(opaque.control_rule("c").is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
