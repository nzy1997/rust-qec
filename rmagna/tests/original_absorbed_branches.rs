//! Real original-IR witnesses for projection before resource selection.
#[allow(dead_code)]
mod common;
use common::{bits, dense_gate};
use rmagna::clifford::CliffordGate as C;
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    OriginalAssignment, OriginalBranch, OriginalBranchCertificateOptions as Options, OriginalFault,
    OriginalGate as G, OriginalInstrument, OriginalLimits, OriginalNetworkResource,
    OriginalOperation, OriginalPauli, ReadoutBasis,
};
use rmagna::phase::NativePhaseOptions;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::CompilerStrategy;

fn op(id: &str, gate: G) -> OriginalOperation {
    OriginalOperation {
        id: id.into(),
        gate,
    }
}
fn phase(q: i64, turns: i64) -> G {
    G::Phase {
        wires: vec![q],
        turns,
    }
}
fn strategies() -> [CompilerStrategy; 2] {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
}
fn instrument(a: &ScalarArithmetic) -> OriginalInstrument {
    let input = AffineKet::new(
        bits(3, 0),
        (0..3).map(|q| bits(3, 1 << q)).collect(),
        rmagna::quadratic::QuadraticForm::new(0, vec![0; 3], vec![bits(3, 0); 3]).unwrap(),
        a.omega(0, -3).unwrap(),
    )
    .unwrap();
    let mut limits = OriginalLimits::default();
    limits.expansion.max_side_terms = 1;
    OriginalInstrument::new(
        vec![0, 1, 2],
        vec![0, 1],
        input,
        vec![
            op("t0", phase(0, 1)),
            op("t1", phase(1, 1)),
            op("t2", phase(2, 1)),
            op("tdg2", phase(2, -1)),
            op("cz0", G::Cz(0, 2)),
            op("cz1", G::Cz(1, 2)),
            op(
                "m",
                G::Destructive {
                    wire: 2,
                    basis: ReadoutBasis::X,
                    record: "m".into(),
                },
            ),
        ],
        vec![],
        limits,
        a,
    )
    .unwrap()
}
fn assignment(outcome: bool, fault: bool) -> OriginalAssignment {
    OriginalAssignment {
        true_values: vec![("m".into(), outcome)],
        faults: if fault {
            vec![OriginalFault {
                after: "t2".into(),
                wire: 2,
                pauli: OriginalPauli::X,
            }]
        } else {
            vec![]
        },
        ..Default::default()
    }
}
fn options(rank: usize, terms: usize, pairs: usize) -> Options<'static> {
    let mut policy = NativePhaseOptions::default();
    policy.selection.max_quotient_rank = rank;
    let mut options = Options {
        resource: OriginalNetworkResource::Native(policy),
        max_quotient_rank: rank,
        max_terms: terms,
        ..Default::default()
    };
    options.completed.max_pairs = pairs;
    options
}
fn sum(terms: &[AffineKet], word: &BitVector, a: &ScalarArithmetic) -> ExactScalar {
    terms.iter().fold(ExactScalar::zero(), |v, k| {
        a.add(&v, &k.prepare().unwrap().amplitude(word, a).unwrap())
            .unwrap()
    })
}
fn oracle(fault: bool, a: &ScalarArithmetic) -> Vec<ExactScalar> {
    let mut values = vec![ExactScalar::zero(); 8];
    values[0] = ExactScalar::one();
    for q in 0..3 {
        dense_gate(C::H(q), &mut values, a);
    }
    for q in 0..3 {
        for (x, v) in values.iter_mut().enumerate() {
            if x >> q & 1 != 0 {
                *v = a.multiply(v, &a.omega(1, 0).unwrap()).unwrap();
            }
        }
    }
    if fault {
        dense_gate(C::X(2), &mut values, a);
    }
    for (x, v) in values.iter_mut().enumerate() {
        if x & 4 != 0 {
            *v = a.multiply(v, &a.omega(-1, 0).unwrap()).unwrap();
        }
    }
    dense_gate(C::Cz { a: 0, b: 2 }, &mut values, a);
    dense_gate(C::Cz { a: 1, b: 2 }, &mut values, a);
    dense_gate(C::H(2), &mut values, a);
    values
}
fn compare(
    branch: &OriginalBranch,
    fault: bool,
    outcome: bool,
    options: &Options<'_>,
    a: &ScalarArithmetic,
) {
    let cert = branch.compile_after_absorption(options, a).unwrap();
    let expected = oracle(fault, a);
    assert_eq!(cert.probability(), &a.omega(0, -2).unwrap());
    let mut norm = ExactScalar::zero();
    for y in 0..4 {
        let value = sum(cert.states(), &bits(2, y), a);
        assert_eq!(
            value,
            expected[y + 4 * usize::from(outcome)],
            "fault={fault} outcome={outcome} y={y}"
        );
        assert_eq!(branch.amplitude(&bits(2, y), 4096, a).unwrap(), value);
        norm = a
            .add(&norm, &a.multiply(&value.conjugate(), &value).unwrap())
            .unwrap();
    }
    assert_eq!(&norm, cert.probability());
    assert_eq!(cert.branch().outputs(), &[0, 1]);
    assert!(cert.branch().environment().is_empty());
    assert_eq!(cert.statistics().side_entries, cert.states().len());
    assert_eq!(
        cert.statistics().probability.pair_work,
        cert.states().len().pow(2)
    );
}
#[test]
fn complete_original_ir_absorbs_before_branch_budget_and_fault_restores_rank() {
    let a = ScalarArithmetic::default();
    let ir = instrument(&a);
    for strategy in strategies() {
        for outcome in [false, true] {
            let source = ir
                .compile(&assignment(outcome, false), strategy, &a)
                .unwrap();
            // The existing route still selects the source resource first.
            assert!(source.expansion(&a).is_err());
            let strict = options(0, 1, 1);
            compare(&source, false, outcome, &strict, &a);
            let cert = source.compile_after_absorption(&strict, &a).unwrap();
            assert_eq!(cert.statistics().quotient_rank, Some(0));
            assert_eq!(cert.statistics().certified_terms, 1);
            assert_eq!(cert.states().len(), 1);
            assert!(cert.absorption().diagnostics().clifford_contractions > 0);

            let expensive = ir
                .compile(&assignment(outcome, true), strategy, &a)
                .unwrap();
            assert!(matches!(
                expensive.compile_after_absorption(&strict, &a),
                Err(rmagna::instrument::OriginalError::ResourceLimit {
                    option: "branch.max_quotient_rank",
                    required: 2,
                    ..
                })
            ));
            let relaxed = options(2, 4, 16);
            compare(&expensive, true, outcome, &relaxed, &a);
            assert_eq!(
                expensive
                    .compile_after_absorption(&relaxed, &a)
                    .unwrap()
                    .statistics()
                    .quotient_rank,
                Some(2)
            );
            // A failed query does not poison either collected branch.
            compare(&source, false, outcome, &strict, &a);
        }
    }
}
#[test]
fn bounded_refusals_leave_the_actual_collected_branch_reusable() {
    let a = ScalarArithmetic::default();
    let ir = instrument(&a);
    for strategy in strategies() {
        let source = ir.compile(&assignment(false, false), strategy, &a).unwrap();
        let valid = options(0, 1, 1);
        let mut cases = Vec::new();
        let mut x = valid;
        x.max_raw_path_bits = 0;
        cases.push(x);
        let mut x = valid;
        x.max_projection_rows = 0;
        cases.push(x);
        let mut x = valid;
        x.max_geometry_words = 0;
        cases.push(x);
        let mut x = valid;
        x.absorption.max_stages = 0;
        cases.push(x);
        let mut x = valid;
        x.absorption.max_work_units = 0;
        cases.push(x);
        let mut x = valid;
        x.absorption.max_geometry_words = 0;
        cases.push(x);
        let mut x = valid;
        x.max_terms = 0;
        cases.push(x);
        let mut x = valid;
        x.completed.max_pairs = 0;
        cases.push(x);
        for refused in cases {
            assert!(source.compile_after_absorption(&refused, &a).is_err());
            compare(&source, false, false, &valid, &a);
        }
    }
}

#[test]
fn zero_branch_skips_resource_selection_under_zero_term_and_pair_budgets() {
    let a = ScalarArithmetic::default();
    let ir = OriginalInstrument::new(
        vec![7],
        vec![],
        rmagna::clifford::CliffordCircuit::computational(bits(1, 0)).unwrap(),
        vec![op(
            "m",
            G::Destructive {
                wire: 7,
                basis: ReadoutBasis::Z,
                record: "m".into(),
            },
        )],
        vec![],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    for strategy in strategies() {
        let impossible = ir
            .compile(
                &OriginalAssignment {
                    true_values: vec![("m".into(), true)],
                    ..Default::default()
                },
                strategy,
                &a,
            )
            .unwrap();
        for resource in [
            OriginalNetworkResource::default(),
            OriginalNetworkResource::Native(NativePhaseOptions::default()),
        ] {
            let mut zero = options(0, 0, 0);
            zero.resource = resource;
            zero.absorption.max_stages = 0;
            zero.absorption.max_work_units = 0;
            let cert = impossible.compile_after_absorption(&zero, &a).unwrap();
            assert_eq!(cert.probability(), &ExactScalar::zero());
            assert!(cert.states().is_empty());
            assert!(cert.resource().is_none());
            assert_eq!(cert.statistics().quotient_rank, None);
            assert_eq!(cert.statistics().certified_terms, 0);
            assert_eq!(cert.statistics().probability.pair_work, 0);
        }
        let possible = ir
            .compile(
                &OriginalAssignment {
                    true_values: vec![("m".into(), false)],
                    ..Default::default()
                },
                strategy,
                &a,
            )
            .unwrap();
        assert_eq!(
            possible
                .compile_after_absorption(&options(0, 1, 1), &a)
                .unwrap()
                .probability(),
            &ExactScalar::one()
        );
    }
}

#[test]
fn stored_feedback_and_chronological_environment_preserve_global_phase() {
    use rmagna::instrument::{OriginalControl, OriginalOrigin};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let a = ScalarArithmetic::default();
    for strategy in strategies() {
        for m in [false, true] {
            let calls = Arc::new(AtomicUsize::new(0));
            let counter = calls.clone();
            let input = AffineKet::new(
                bits(3, 0),
                vec![bits(3, 1), bits(3, 4)],
                rmagna::quadratic::QuadraticForm::new(0, vec![0, 0], vec![bits(2, 0); 2]).unwrap(),
                a.omega(3, -2).unwrap(),
            )
            .unwrap();
            let ir = OriginalInstrument::new(
                vec![0, 1, 2],
                vec![1],
                input,
                vec![
                    op(
                        "read",
                        G::Measure {
                            factors: vec![(0, OriginalPauli::Z)],
                            inverted: false,
                            record: "m".into(),
                        },
                    ),
                    op("feedback", G::Control),
                    op(
                        "reset",
                        G::Reset {
                            wire: 0,
                            plus: false,
                        },
                    ),
                    op("discard", G::Discard(2)),
                    op(
                        "retire",
                        G::Destructive {
                            wire: 0,
                            basis: ReadoutBasis::Z,
                            record: "r".into(),
                        },
                    ),
                ],
                vec![OriginalControl {
                    operation: "feedback".into(),
                    parents: vec!["m".into()],
                    version: "certificate-feedback-v1".into(),
                    callback: Arc::new(move |inputs| {
                        counter.fetch_add(1, Ordering::SeqCst);
                        let mut gates = vec![phase(0, 1)];
                        if inputs.get("m")? {
                            gates.push(G::Pauli {
                                wire: 1,
                                pauli: OriginalPauli::X,
                            });
                        }
                        Ok(gates)
                    }),
                }],
                OriginalLimits::default(),
                &a,
            )
            .unwrap();
            let branch = ir
                .compile(
                    &OriginalAssignment {
                        true_values: vec![("m".into(), m), ("r".into(), false)],
                        report_flips: vec!["m".into()],
                        ..Default::default()
                    },
                    strategy,
                    &a,
                )
                .unwrap();
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            let invalid = Options {
                max_geometry_words: 0,
                ..Default::default()
            };
            assert!(branch.compile_after_absorption(&invalid, &a).is_err());
            for resource in [
                OriginalNetworkResource::default(),
                OriginalNetworkResource::Native(NativePhaseOptions::default()),
            ] {
                let opts = Options {
                    resource,
                    ..Default::default()
                };
                let cert = branch.compile_after_absorption(&opts, &a).unwrap();
                assert_eq!(cert.probability(), &a.omega(0, -2).unwrap());
                assert_eq!(cert.branch().outputs(), &[1]);
                assert_eq!(
                    cert.branch()
                        .environment()
                        .iter()
                        .map(|e| e.wire)
                        .collect::<Vec<_>>(),
                    [0, 2]
                );
                assert_eq!(
                    cert.branch().environment()[0].retired_by,
                    OriginalOrigin::Operation("reset".into())
                );
                assert_eq!(
                    cert.branch().environment()[1].retired_by,
                    OriginalOrigin::Operation("discard".into())
                );
                assert_eq!(
                    cert.branch().decisions()[0].version,
                    "certificate-feedback-v1"
                );
                assert_eq!(
                    cert.branch().decisions()[0].inputs.entries(),
                    &[("m".into(), !m)]
                );
                assert_eq!(cert.branch().records()[0].true_value, m);
                assert_eq!(cert.branch().records()[0].reported_value, !m);
                let mut norm = ExactScalar::zero();
                // Each surviving environment2 label has amplitude
                // omega^(3+m)/2; the reset environment stores the true m,
                // while the controlled output follows the flipped report.
                for word in 0..8 {
                    let expected =
                        if word & 1 == usize::from(!m) && ((word >> 1) & 1) == usize::from(m) {
                            a.omega(3 + i64::from(m), -2).unwrap()
                        } else {
                            ExactScalar::zero()
                        };
                    let actual = sum(cert.states(), &bits(3, word), &a);
                    assert_eq!(actual, expected, "m={m} word={word}");
                    norm = a
                        .add(&norm, &a.multiply(&actual.conjugate(), &actual).unwrap())
                        .unwrap();
                }
                assert_eq!(&norm, cert.probability());
                assert_eq!(calls.load(Ordering::SeqCst), 1);
            }
        }
    }
}

#[test]
fn scalar_refusal_preserves_a_later_complete_valid_certificate() {
    let a = ScalarArithmetic::default();
    let input = AffineKet::new(
        bits(1, 0),
        vec![bits(1, 1)],
        rmagna::quadratic::QuadraticForm::new(0, vec![0], vec![bits(1, 0)]).unwrap(),
        a.omega(0, -1).unwrap(),
    )
    .unwrap();
    let ir = OriginalInstrument::new(
        vec![0],
        vec![],
        input,
        vec![
            op("t", phase(0, 1)),
            op(
                "m",
                G::Destructive {
                    wire: 0,
                    basis: ReadoutBasis::X,
                    record: "m".into(),
                },
            ),
        ],
        vec![],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    for strategy in strategies() {
        let source = ir
            .compile(
                &OriginalAssignment {
                    true_values: vec![("m".into(), false)],
                    ..Default::default()
                },
                strategy,
                &a,
            )
            .unwrap();
        let limited = ScalarArithmetic::new(1).unwrap();
        assert!(
            source
                .compile_after_absorption(&Options::default(), &limited)
                .is_err()
        );
        let cert = source
            .compile_after_absorption(&Options::default(), &a)
            .unwrap();
        // |(1+omega)/2|^2 = (2+sqrt(2))/4, without normalization.
        let expected = a
            .add(&a.omega(0, -2).unwrap(), &a.omega(0, -3).unwrap())
            .unwrap();
        assert_eq!(cert.probability(), &expected);
    }
}

#[test]
fn actual_temporal_fault_keeps_trace_probability_one_half() {
    use rmagna::instrument::{
        OriginalNetworkOptions, OriginalRecordParity, OriginalTraceAssignment,
    };
    let a = ScalarArithmetic::default();
    let ir = instrument(&a);
    for strategy in strategies() {
        let trace = ir.compile_trace(strategy, &a).unwrap();
        let parities = [OriginalRecordParity {
            records: vec!["m".into()],
            inverted: false,
        }];
        let probability = trace
            .prepare_network_probability(&parities, &OriginalNetworkOptions::default(), &a)
            .unwrap();
        for fault in [false, true] {
            let assignment = assignment(false, fault);
            let view = trace
                .with_assignment(
                    &OriginalTraceAssignment {
                        faults: assignment.faults,
                        report_flips: vec![],
                    },
                    &a,
                )
                .unwrap();
            for outcome in [0, 1] {
                assert_eq!(
                    probability
                        .evaluate(&view, &bits(1, outcome), &a)
                        .unwrap()
                        .value,
                    a.omega(0, -2).unwrap()
                );
            }
        }
    }
}
