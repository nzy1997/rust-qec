// SPDX-License-Identifier: MPL-2.0
// Source-bound ownership cases from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f:
// test_native_batch.py, test_native_global_fastpath.py and test_simulator_optimization.py.
// Full license: fixtures/original_many_record_input.LICENSE.
#[path = "support/build_artifacts.rs"]
mod build_artifacts;
use rmagna::{
    clifford::CliffordCircuit,
    gf2::BitVector,
    instrument::*,
    scalar::{ExactScalar, ScalarArithmetic},
    unitary::CompilerStrategy,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
fn bits(n: usize, x: usize) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|j| x >> j & 1 != 0)).unwrap()
}
fn instrument(
    turns: i64,
    computational: Option<bool>,
    control: Option<Arc<AtomicUsize>>,
) -> OriginalInstrument {
    let a = ScalarArithmetic::default();
    let initial = if let Some(one) = computational {
        CliffordCircuit::computational(bits(1, usize::from(one))).unwrap()
    } else {
        rmagna::stabilizer::AffineKet::new(
            bits(1, 0),
            vec![bits(1, 1)],
            rmagna::quadratic::QuadraticForm::new(0, vec![0], vec![bits(1, 0)]).unwrap(),
            a.omega(0, -1).unwrap(),
        )
        .unwrap()
    };
    let mut ops = Vec::new();
    if computational.is_none() || turns != 0 {
        ops.push(OriginalOperation {
            id: "t".into(),
            gate: OriginalGate::Phase {
                wires: vec![0],
                turns,
            },
        });
    }
    ops.push(OriginalOperation {
        id: "m".into(),
        gate: OriginalGate::Measure {
            factors: vec![(
                0,
                if computational.is_some() {
                    OriginalPauli::Z
                } else {
                    OriginalPauli::X
                },
            )],
            inverted: false,
            record: "m".into(),
        },
    });
    let controls = if let Some(calls) = control {
        ops.push(OriginalOperation {
            id: "c".into(),
            gate: OriginalGate::Control,
        });
        vec![OriginalControl {
            operation: "c".into(),
            parents: vec!["m".into()],
            version: "v1".into(),
            callback: Arc::new(move |r| {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(vec![if r.get("m")? {
                    OriginalGate::Pauli {
                        wire: 0,
                        pauli: OriginalPauli::X,
                    }
                } else {
                    OriginalGate::Identity(vec![0])
                }])
            }),
        }]
    } else {
        vec![]
    };
    OriginalInstrument::new(
        vec![0],
        vec![0],
        initial,
        ops,
        controls,
        OriginalLimits::default(),
        &a,
    )
    .unwrap()
}
fn plan(turns: i64, base: Option<bool>, calls: Option<Arc<AtomicUsize>>) -> OriginalAssignmentPlan {
    OriginalAssignmentPlan::from_owned(
        instrument(turns, base, calls),
        CompilerStrategy::ParityCopies,
        ScalarArithmetic::default(),
    )
}
fn batch_plan(hadamard: bool) -> OriginalAssignmentPlan {
    let a = ScalarArithmetic::default();
    let ir = OriginalInstrument::new(
        vec![0],
        vec![0],
        CliffordCircuit::computational(bits(1, 0)).unwrap(),
        vec![
            OriginalOperation {
                id: "h".into(),
                gate: if hadamard {
                    OriginalGate::H(0)
                } else {
                    OriginalGate::Identity(vec![0])
                },
            },
            OriginalOperation {
                id: "t".into(),
                gate: OriginalGate::Phase {
                    wires: vec![0],
                    turns: 1,
                },
            },
            OriginalOperation {
                id: "m".into(),
                gate: OriginalGate::Measure {
                    factors: vec![(0, OriginalPauli::Z)],
                    inverted: false,
                    record: "m".into(),
                },
            },
        ],
        vec![],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    OriginalAssignmentPlan::from_owned(ir, CompilerStrategy::ParityCopies, a)
}
fn half() -> ExactScalar {
    ScalarArithmetic::default()
        .scalar([1, 0, 0, 0].map(Into::into), -1)
        .unwrap()
}
fn ordinary(bit: bool) -> OriginalAssignment {
    OriginalAssignment {
        true_values: vec![("m".into(), bit)],
        ..Default::default()
    }
}
fn fault(p: OriginalPauli) -> OriginalFault {
    OriginalFault {
        after: "t".into(),
        wire: 0,
        pauli: p,
    }
}
fn mass(high: bool) -> ExactScalar {
    ScalarArithmetic::default()
        .scalar(
            if high { [2, 1, 0, -1] } else { [2, -1, 0, 1] }.map(Into::into),
            -2,
        )
        .unwrap()
}
#[test]
fn original_empty_fault_sharing_cannot_bypass_foreign_snapshot_validation() {
    let local = plan(1, None, None);
    let foreign = plan(3, None, None);
    let first = local
        .compact(bits(1, 0), OriginalSharedFaults::empty())
        .unwrap();
    let second = foreign
        .compact(bits(1, 0), OriginalSharedFaults::empty())
        .unwrap();
    assert!(
        first
            .shared_faults()
            .shares_storage_with(second.shared_faults())
    );
    let mut batch = local.probabilities([first, second]);
    assert_eq!(batch.next().unwrap().unwrap(), mass(true));
    assert!(
        batch
            .next()
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("different IR snapshot")
    );
    assert!(batch.next().is_none());
    assert_eq!(
        batch.statistics(),
        OriginalAssignmentBatchStatistics {
            inputs: 2,
            fault_group_snapshots: 1,
            compiled_branches: 1,
            values: 1,
            collected_callback_decisions: 0
        }
    );
    assert!(
        local
            .compact(bits(2, 0), OriginalSharedFaults::empty())
            .is_err()
    );
}
#[test]
fn original_shared_fault_group_keeps_records_report_errors_and_independent_snapshots() {
    let source = batch_plan(true);
    let group = source
        .shared_faults(vec![fault(OriginalPauli::Y)], vec!["m".into()])
        .unwrap();
    let a = source.compact(bits(1, 1), group.clone()).unwrap();
    let b = source.compact(bits(1, 0), group.clone()).unwrap();
    assert!(a.shared_faults().shares_storage_with(b.shared_faults()));
    assert_eq!(a.word(), &bits(1, 1));
    assert_eq!(b.word(), &bits(1, 0));
    assert_eq!(group.faults()[0].pauli, OriginalPauli::Y);
    assert_eq!(group.report_flips(), &["m"]);
    let mut batch = source.probabilities([a.clone(), b.clone()]);
    assert_eq!(
        batch.by_ref().collect::<Result<Vec<_>, _>>().unwrap(),
        vec![half(); 2]
    );
    assert_eq!(batch.statistics().fault_group_snapshots, 1);
    for (input, bit) in [(a, true), (b, false)] {
        let expected = source
            .probability(OriginalAssignment {
                faults: vec![fault(OriginalPauli::Y)],
                report_flips: vec!["m".into()],
                ..ordinary(bit)
            })
            .unwrap();
        assert_eq!(source.probability(input).unwrap(), expected);
    }
    let cancelled = source
        .shared_faults(vec![], vec!["m".into(), "m".into()])
        .unwrap();
    assert!(cancelled.report_flips().is_empty());
    assert!(cancelled.shares_storage_with(&OriginalSharedFaults::empty()));
    assert!(
        source
            .shared_faults(vec![], vec!["unknown".into()])
            .is_err()
    );
    let replacement = batch_plan(false);
    let original = source.compact(bits(1, 0), group).unwrap();
    assert!(replacement.probability(original).is_err());
}
#[test]
fn original_ordinary_faults_are_copied_again_after_each_mutating_yield() {
    let source = plan(1, None, None);
    let mut assignment = Arc::new(OriginalAssignment {
        faults: vec![fault(OriginalPauli::I)],
        ..ordinary(false)
    });
    let mut index = 0;
    let inputs = std::iter::from_fn(|| {
        let p = match index {
            0 => OriginalPauli::I,
            1 => OriginalPauli::Z,
            _ => return None,
        };
        // Fails if the evaluator retains even one previous ordinary input.
        Arc::get_mut(&mut assignment)
            .expect("consumed input must be released")
            .faults[0]
            .pauli = p;
        index += 1;
        Some(Arc::clone(&assignment))
    });
    let mut batch = source.probabilities(inputs);
    assert_eq!(
        batch.by_ref().collect::<Result<Vec<_>, _>>().unwrap(),
        vec![mass(true), mass(false)]
    );
    assert_eq!(batch.statistics().fault_group_snapshots, 2);
    assert_eq!(batch.statistics().compiled_branches, 2);
}
#[test]
fn original_no_feedback_stream_releases_all_sixty_four_consumed_assignments() {
    let source = plan(1, None, None);
    let weak = Arc::new(Mutex::new(Vec::<std::sync::Weak<OriginalAssignment>>::new()));
    let refs = weak.clone();
    let inputs = (0..64).map(move |i| {
        let assignment = Arc::new(ordinary(i % 2 != 0));
        let mut refs = refs.lock().unwrap();
        refs.push(Arc::downgrade(&assignment));
        assert!(refs.iter().filter(|r| r.upgrade().is_some()).count() <= 1);
        assignment
    });
    let mut batch = source.probabilities(inputs);
    let values = batch.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(values.len(), 64);
    assert_eq!(batch.statistics().inputs, 64);
    assert_eq!(batch.statistics().values, 64);
    for (j, value) in values.iter().enumerate() {
        assert_eq!(*value, mass(j % 2 == 0));
    }
    assert!(weak.lock().unwrap().iter().all(|r| r.upgrade().is_none()));
}
#[test]
fn original_lazy_batches_capture_one_source_while_next_single_query_uses_replacement() {
    let mut active = plan(0, Some(false), None);
    let captured = active.clone();
    let mut index = 0;
    let inputs = std::iter::from_fn(|| {
        if index == 2 {
            return None;
        }
        if index == 1 {
            active = plan(0, Some(true), None);
        }
        index += 1;
        Some(ordinary(false))
    });
    assert_eq!(
        captured
            .probabilities(inputs)
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        vec![ExactScalar::one(); 2]
    );
    assert_eq!(
        active.probability(ordinary(false)).unwrap(),
        ExactScalar::zero()
    );
    assert_eq!(
        active
            .probabilities([ordinary(false), ordinary(false)])
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        vec![ExactScalar::zero(); 2]
    );
}
#[test]
fn original_compact_batch_retains_owner_while_callbacks_execute_on_every_assignment() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut active = plan(0, Some(false), Some(calls.clone()));
    let captured = active.clone();
    let a = captured
        .compact(bits(1, 0), OriginalSharedFaults::empty())
        .unwrap();
    let mut index = 0;
    let inputs = std::iter::from_fn(|| {
        if index == 2 {
            return None;
        }
        if index == 1 {
            active = plan(0, Some(true), Some(calls.clone()));
        }
        index += 1;
        Some(a.clone())
    });
    let mut batch = captured.probabilities(inputs);
    assert_eq!(
        batch.by_ref().collect::<Result<Vec<_>, _>>().unwrap(),
        vec![ExactScalar::one(); 2]
    );
    assert_eq!(batch.statistics().collected_callback_decisions, 2);
    assert_eq!(
        captured.probability(ordinary(false)).unwrap(),
        ExactScalar::one()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert!(
        active
            .probability(a)
            .unwrap_err()
            .to_string()
            .contains("different IR snapshot")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[test]
fn original_noise_layout_rejects_all_four_bad_code_rows_and_cancels_duplicate_reports() {
    use serde_json::json;
    let source = batch_plan(true);
    let layout = OriginalCompactNoiseLayout::new(
        &source,
        vec![vec![
            OriginalCompactNoiseChannel::report("m".into()).unwrap(),
        ]],
    )
    .unwrap();
    for row in [
        vec![(json!(0), json!(2))],
        vec![(json!(1), json!(1))],
        vec![(json!(-1), json!(1))],
        vec![(json!(0), json!(true))],
    ] {
        let mut stream = layout.assignments([row], vec![bits(1, 0)]).unwrap();
        assert!(stream.next().unwrap().is_err());
        assert!(stream.next().is_none());
    }
    let a = layout
        .assignments(
            [vec![(json!(0), json!(1)), (json!(0), json!(1))]],
            vec![bits(1, 0)],
        )
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(a.word(), &bits(1, 0));
    assert!(a.shared_faults().report_flips().is_empty());
    assert_eq!(source.probability(a).unwrap(), half());
    assert!(
        source
            .compact(bits(2, 2), OriginalSharedFaults::empty())
            .is_err()
    );
    assert!(layout.assignments([vec![]], vec![bits(2, 2)]).is_err());
    eprintln!("noise codes:4 original bad rows;duplicate report cancellation;foreign word bounds");
}
#[test]
fn original_noise_rows_snapshot_before_yield_and_share_true_record_inversions() {
    use serde_json::json;
    let source = batch_plan(true);
    let physical = OriginalCompactNoiseChannel::pauli_site(
        "t",
        &[0],
        &[
            vec![OriginalPauli::X],
            vec![OriginalPauli::Y],
            vec![OriginalPauli::Z],
        ],
    )
    .unwrap();
    let layout = OriginalCompactNoiseLayout::new(
        &source,
        vec![
            physical,
            vec![OriginalCompactNoiseChannel::report("m".into()).unwrap()],
        ],
    )
    .unwrap();
    let row = Arc::new(Mutex::new(vec![(json!(0), json!(3)), (json!(1), json!(1))]));
    let entered = Arc::new(AtomicUsize::new(0));
    let observed = row.clone();
    let count = entered.clone();
    let rows = std::iter::from_fn(move || {
        if count.fetch_add(1, Ordering::SeqCst) == 0 {
            Some(observed.lock().unwrap().clone())
        } else {
            None
        }
    });
    let mut stream = layout
        .assignments(rows, vec![bits(1, 0), bits(1, 1)])
        .unwrap();
    let a = stream.next().unwrap().unwrap();
    row.lock().unwrap().clear();
    let b = stream.next().unwrap().unwrap();
    assert_eq!(entered.load(Ordering::SeqCst), 1);
    assert!(a.shared_faults().shares_storage_with(b.shared_faults()));
    assert_eq!((a.word(), b.word()), (&bits(1, 1), &bits(1, 0)));
    assert_eq!(a.shared_faults().faults(), &[fault(OriginalPauli::Y)]);
    assert_eq!(a.shared_faults().report_flips(), &["m"]);
    let actual = source
        .probabilities([a.clone(), b.clone()])
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let expected = (0..2)
        .map(|j| {
            source
                .probability(OriginalAssignment {
                    faults: vec![fault(OriginalPauli::Y)],
                    report_flips: vec!["m".into()],
                    ..ordinary(j == 0)
                })
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert_eq!(actual, vec![half(); 2]);
    assert!(batch_plan(false).probability(a).is_err());
    assert!(stream.next().is_none());
    assert_eq!(entered.load(Ordering::SeqCst), 2);
    eprintln!(
        "immutable row:IXZY code3=Y;reported0,1->true1,0;one decoded row;shared group survives caller mutation"
    );
}
#[test]
fn compact_codebooks_preserve_ixzy_single_channel_codes_and_reject_ambiguity() {
    use serde_json::json;
    let source = batch_plan(true);
    let channels = OriginalCompactNoiseChannel::pauli_site(
        "t",
        &[0],
        &[
            vec![OriginalPauli::X],
            vec![OriginalPauli::Y],
            vec![OriginalPauli::Z],
        ],
    )
    .unwrap();
    let layout = OriginalCompactNoiseLayout::new(&source, vec![channels]).unwrap();
    for (code, pauli) in [
        (1, OriginalPauli::X),
        (2, OriginalPauli::Z),
        (3, OriginalPauli::Y),
    ] {
        assert_eq!(
            layout
                .decode_json_pairs(&[(json!(0), json!(code))])
                .unwrap()
                .faults(),
            &[fault(pauli)]
        );
    }
    let channels =
        OriginalCompactNoiseChannel::pauli_site("t", &[0], &[vec![OriginalPauli::Z]]).unwrap();
    let layout = OriginalCompactNoiseLayout::new(&source, vec![channels]).unwrap();
    assert_eq!(
        layout
            .decode_json_pairs(&[(json!(0), json!(1))])
            .unwrap()
            .faults(),
        &[fault(OriginalPauli::Z)]
    );
    assert!(layout.decode_json_pairs(&[(json!(0), json!(2))]).is_err());
    let channel = |pauli| OriginalCompactNoiseChannel {
        code: bits(1, 1),
        faults: vec![fault(pauli)],
        report_flips: vec![],
    };
    assert!(
        OriginalCompactNoiseLayout::new(
            &source,
            vec![vec![channel(OriginalPauli::X), channel(OriginalPauli::Z)]]
        )
        .is_err()
    );
    assert!(
        OriginalCompactNoiseLayout::new(
            &source,
            vec![vec![channel(OriginalPauli::X), channel(OriginalPauli::X)]]
        )
        .is_ok()
    );
    assert!(
        OriginalCompactNoiseChannel::pauli_site("t", &[0, 0], &[vec![OriginalPauli::X; 2]])
            .is_err()
    );
    assert!(OriginalCompactNoiseChannel::pauli_site("t", &[0], &[vec![]]).is_err());
}
#[test]
fn typed_noise_codes_cross_two_words_without_truncating_the_last_physical_wire() {
    let a = ScalarArithmetic::default();
    let wires = (0..65).collect::<Vec<_>>();
    let ir = OriginalInstrument::new(
        wires.clone(),
        wires.clone(),
        CliffordCircuit::computational(BitVector::zeros(65).unwrap()).unwrap(),
        vec![OriginalOperation {
            id: "t".into(),
            gate: OriginalGate::Identity(wires.clone()),
        }],
        vec![],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    let source = OriginalAssignmentPlan::from_owned(ir, CompilerStrategy::ParityCopies, a);
    let mut x = vec![OriginalPauli::I; 65];
    x[64] = OriginalPauli::X;
    let mut z = vec![OriginalPauli::I; 65];
    z[64] = OriginalPauli::Z;
    let channels = OriginalCompactNoiseChannel::pauli_site("t", &wires, &[x, z]).unwrap();
    let layout = OriginalCompactNoiseLayout::new(&source, vec![channels]).unwrap();
    for (bit, pauli) in [(128, OriginalPauli::X), (129, OriginalPauli::Z)] {
        let code = BitVector::from_indices(130, [bit]).unwrap();
        let group = layout
            .decode(&[OriginalNoiseCodeSelection { site: 0, code }])
            .unwrap();
        assert_eq!(
            group.faults(),
            &[OriginalFault {
                after: "t".into(),
                wire: 64,
                pauli
            }]
        );
        assert!(group.report_flips().is_empty());
    }
    assert!(
        layout
            .decode(&[OriginalNoiseCodeSelection {
                site: 0,
                code: BitVector::from_indices(131, [130]).unwrap()
            }])
            .is_err()
    );
}

#[test]
fn original_probability_queries_generate_no_input_key_but_explicit_compile_does() {
    use serde_json::json;
    // Rust exposes no probability-value cache: both upstream disabled-cache
    // configurations map to this explicit no-cache session contract.
    for _disabled_kind in ["entries", "bytes"] {
        let source = batch_plan(true);
        assert_eq!(source.input_key_generations(), 0);
        assert_eq!(
            source
                .probabilities([ordinary(false), ordinary(false), ordinary(false)])
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            vec![half(); 3]
        );
        assert!(
            source
                .probability(OriginalRecordAssignment {
                    records: vec![("m".into(), json!(2))],
                    ..Default::default()
                })
                .is_err()
        );
        assert_eq!(source.input_key_generations(), 0);
        let certificate = source.compile(ordinary(false)).unwrap();
        assert_eq!(
            certificate.input_fingerprint(),
            "c9eab50d7c6176234bb7db7f4ef9e7155fa502ba2930947fd8db467596752689"
        );
        assert_eq!(certificate.probability(), &half());
        assert_eq!(source.input_key_generations(), 1);
        let mut exported = certificate.to_json();
        exported["input"]["true_values"][0][1] = json!(true);
        assert_eq!(
            certificate.to_json()["input"]["true_values"][0][1],
            json!(false)
        );
        assert_eq!(source.probability(ordinary(false)).unwrap(), half());
        assert_eq!(source.input_key_generations(), 1);
    }
    let source = batch_plan(true);
    source.probability(ordinary(false)).unwrap();
    assert_eq!(source.input_key_generations(), 0);
}
#[test]
fn original_unexpected_kernel_error_propagates_once_without_another_backend_attempt() {
    let a = ScalarArithmetic::default();
    let ir = OriginalInstrument::new(
        vec![0],
        vec![0],
        CliffordCircuit::computational(bits(1, 0)).unwrap(),
        vec![],
        vec![],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    let source = OriginalAssignmentPlan::from_owned(ir, CompilerStrategy::ParityCopies, a);
    let count = AtomicUsize::new(0);
    let injected = OriginalError::Scalar(rmagna::scalar::ScalarError::ExponentOverflow);
    let found = source.compile_with_kernel(OriginalAssignment::default(), |branch, _| {
        count.fetch_add(1, Ordering::SeqCst);
        assert_eq!(branch.outputs(), &[0]);
        assert!(branch.records().is_empty());
        Err(injected.clone())
    });
    let error = match found {
        Err(e) => e,
        Ok(_) => panic!("injected kernel failure was swallowed"),
    };
    assert_eq!(error, injected);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(source.input_key_generations(), 0);
}
fn ownership_fixture(
    controlled: bool,
    mismatch: bool,
    calls: Arc<AtomicUsize>,
) -> OriginalAssignmentPlan {
    let a = ScalarArithmetic::default();
    let initial = rmagna::stabilizer::AffineKet::new(
        bits(1, 0),
        vec![bits(1, 1)],
        rmagna::quadratic::QuadraticForm::new(0, vec![0], vec![bits(1, 0)]).unwrap(),
        a.omega(0, -1).unwrap(),
    )
    .unwrap();
    let mut ops = vec![OriginalOperation {
        id: "m".into(),
        gate: OriginalGate::Measure {
            factors: vec![(0, OriginalPauli::Z)],
            inverted: false,
            record: "m".into(),
        },
    }];
    let controls = if controlled {
        ops.push(OriginalOperation {
            id: "c".into(),
            gate: OriginalGate::Control,
        });
        vec![OriginalControl {
            operation: "c".into(),
            parents: vec!["m".into()],
            version: "owned-rule-v1".into(),
            callback: Arc::new(move |r| {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(vec![OriginalGate::Phase {
                    wires: vec![0],
                    turns: 1 + if r.get("m")? {
                        if mismatch { 1 } else { 6 }
                    } else {
                        0
                    },
                }])
            }),
        }]
    } else {
        vec![]
    };
    ops.push(OriginalOperation {
        id: "n".into(),
        gate: OriginalGate::Measure {
            factors: vec![(0, OriginalPauli::X)],
            inverted: false,
            record: "n".into(),
        },
    });
    OriginalAssignmentPlan::from_owned(
        OriginalInstrument::new(
            vec![0],
            vec![0],
            initial,
            ops,
            controls,
            OriginalLimits::default(),
            &a,
        )
        .unwrap(),
        CompilerStrategy::ParityCopies,
        a,
    )
}
#[test]
fn original_certificate_keeps_both_captured_inputs_before_the_evaluation_kernel_returns() {
    for controlled in [false, true] {
        let calls = Arc::new(AtomicUsize::new(0));
        let source = ownership_fixture(controlled, false, calls.clone());
        let original = Arc::new(Mutex::new(OriginalAssignment {
            true_values: vec![("m".into(), true), ("n".into(), false)],
            faults: vec![OriginalFault {
                after: "m".into(),
                wire: 0,
                pauli: OriginalPauli::Y,
            }],
            report_flips: vec![],
        }));
        let expected = source.compile(original.lock().unwrap().clone()).unwrap();
        let captured = original.lock().unwrap().clone();
        let changed = original.clone();
        let actual = source
            .compile_with_kernel(captured, move |branch, a| {
                let result = branch.probability(a)?;
                let mut value = changed.lock().unwrap();
                value.true_values[0].1 = false;
                value.faults[0].pauli = OriginalPauli::X;
                Ok(result)
            })
            .unwrap();
        assert_eq!(actual.to_json(), expected.to_json());
        assert_eq!(actual.assignment().true_values[0], ("m".into(), true));
        assert_eq!(actual.assignment().faults[0].pauli, OriginalPauli::Y);
        assert_eq!(
            actual.probability(),
            &ScalarArithmetic::default()
                .scalar([1, 0, 0, 0].map(Into::into), -2)
                .unwrap()
        );
        for y in [0, 1] {
            assert_eq!(
                actual
                    .branch()
                    .amplitude(&bits(1, y), 1024, &ScalarArithmetic::default())
                    .unwrap(),
                ScalarArithmetic::default().omega(6, -3).unwrap()
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), if controlled { 2 } else { 0 });
        assert_eq!(original.lock().unwrap().faults[0].pauli, OriginalPauli::X);
    }
}
#[test]
fn original_literal_policy_retains_packed_values_and_never_replays_the_phase_rule() {
    let calls = Arc::new(AtomicUsize::new(0));
    let source = ownership_fixture(true, true, calls.clone());
    let original = Arc::new(Mutex::new(OriginalAssignment {
        true_values: vec![("m".into(), true), ("n".into(), false)],
        ..Default::default()
    }));
    let expected = source.compile(original.lock().unwrap().clone()).unwrap();
    calls.store(0, Ordering::SeqCst);
    let captured = original.lock().unwrap().clone();
    let changed = original.clone();
    let actual = source
        .compile_with_kernel(captured, move |branch, a| {
            let value = branch.probability(a)?;
            let mut input = changed.lock().unwrap();
            input.true_values[0].1 = false;
            input.true_values[1].1 = true;
            Ok(value)
        })
        .unwrap();
    assert_eq!(actual.to_json(), expected.to_json());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(actual.branch().decisions().len(), 1);
    for y in [0, 1] {
        assert_eq!(
            actual
                .branch()
                .amplitude(&bits(1, y), 1024, &ScalarArithmetic::default())
                .unwrap(),
            ScalarArithmetic::default().omega(2, -3).unwrap()
        );
    }
}
fn environment_tensor(labels: usize) -> rmagna::phase::CertifiedPathTensor<()> {
    use rmagna::phase::*;
    CertifiedPathTensor::new(
        PhasePathTensor::new(
            rmagna::stabilizer::AffineKet::new(
                BitVector::zeros(labels).unwrap(),
                vec![],
                rmagna::quadratic::QuadraticForm::new(0, vec![], vec![]).unwrap(),
                ExactScalar::one(),
            )
            .unwrap(),
            ParityPhase::new(0, 0, []).unwrap(),
        )
        .unwrap(),
    )
}
#[test]
fn original_environment_unicode_nul_and_non_bmp_labels_survive_both_tensor_services() {
    use rmagna::phase::*;
    let a = ScalarArithmetic::default();
    // Repository policy substitutes a non-Han multibyte symbol for the source
    // label's script-specific code point; all encoding/ownership cases remain.
    let labels = vec![
        "\0".to_string(),
        "before\0after".to_string(),
        "Ω-é-\0-🙂".to_string(),
    ];
    let encoded = serde_json::to_vec(&labels).unwrap();
    let source =
        NamedPhasePathTensor::from_json_environment(environment_tensor(3), 0, &encoded, 1024)
            .unwrap();
    let absorbed = source.absorb_boundaries(Default::default(), &a).unwrap();
    assert_eq!(absorbed.environment(), labels);
    assert_eq!(absorbed.output_count(), 0);
    let mut decomposed = source
        .decompose(1, Default::default(), None, Some(Default::default()), &a)
        .unwrap();
    assert_eq!(decomposed.environment, labels);
    assert_eq!(decomposed.output_count, 0);
    assert_eq!(decomposed.decomposition.report.constructive_terms, 1);
    let states = decomposed
        .decomposition
        .resource
        .as_ref()
        .unwrap()
        .expand(
            decomposed.decomposition.tensor.tensor().phase(),
            decomposed.decomposition.tensor.tensor().boundary(),
            &a,
        )
        .unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(
        states[0]
            .prepare()
            .unwrap()
            .amplitude(&bits(3, 0), &a)
            .unwrap(),
        ExactScalar::one()
    );
    for y in 1..8 {
        assert_eq!(
            states[0]
                .prepare()
                .unwrap()
                .amplitude(&bits(3, y), &a)
                .unwrap(),
            ExactScalar::zero()
        );
    }
    decomposed.environment[1].clear();
    assert_eq!(source.environment(), labels);
    assert_eq!(absorbed.environment(), labels);
    assert_eq!(
        serde_json::from_slice::<Vec<String>>(&serde_json::to_vec(source.environment()).unwrap())
            .unwrap(),
        labels
    );
}
#[test]
fn original_unpaired_environment_surrogate_rejects_at_the_shared_json_import_boundary() {
    use rmagna::phase::*;
    for raw in [
        br#"["\ud800"]"#.as_slice(),
        b"[\"\xed\xa0\x80\"]".as_slice(),
    ] {
        let error =
            NamedPhasePathTensor::from_json_environment(environment_tensor(1), 0, raw, 1024)
                .err()
                .expect("invalid Unicode cannot enter either service");
        assert!(matches!(error, NamedPathTensorError::Json(_)));
        assert!(error.to_string().contains("environment label JSON"));
    }
    assert!(matches!(
        NamedPhasePathTensor::from_json_environment(environment_tensor(1), 0, b"[\"x\"]", 0),
        Err(NamedPathTensorError::JsonLimit { .. })
    ));
    assert!(NamedPhasePathTensor::new(environment_tensor(1), 0, vec![]).is_err());
    assert!(NamedPhasePathTensor::new(environment_tensor(1), 1, vec!["x".into()]).is_err());
}
#[test]
fn original_parent_list_replacement_changes_source_identity_and_uses_new_callback_inputs() {
    let mut parents = vec!["a".to_string()];
    let make = |parents: &[String]| {
        let a = ScalarArithmetic::default();
        let measure = |name: &str| OriginalOperation {
            id: name.into(),
            gate: OriginalGate::Measure {
                factors: vec![(0, OriginalPauli::Z)],
                inverted: false,
                record: name.into(),
            },
        };
        let ir = OriginalInstrument::new(
            vec![0],
            vec![0],
            CliffordCircuit::computational(bits(1, 0)).unwrap(),
            vec![
                measure("a"),
                measure("b"),
                OriginalOperation {
                    id: "c".into(),
                    gate: OriginalGate::Control,
                },
                measure("m"),
            ],
            vec![OriginalControl {
                operation: "c".into(),
                parents: parents.to_vec(),
                version: "v1".into(),
                callback: Arc::new(|r| {
                    Ok(vec![if r.entries().iter().any(|(name, _)| name == "b") {
                        OriginalGate::Pauli {
                            wire: 0,
                            pauli: OriginalPauli::X,
                        }
                    } else {
                        OriginalGate::Identity(vec![0])
                    }])
                }),
            }],
            OriginalLimits::default(),
            &a,
        )
        .unwrap();
        OriginalAssignmentPlan::from_owned(ir, CompilerStrategy::ParityCopies, a)
    };
    let assignment = OriginalAssignment {
        true_values: vec![
            ("a".into(), false),
            ("b".into(), false),
            ("m".into(), false),
        ],
        ..Default::default()
    };
    let first = make(&parents);
    let c = first.compile(assignment.clone()).unwrap();
    assert_eq!(c.probability(), &ExactScalar::one());
    parents.push("b".into());
    let replacement = make(&parents);
    let d = replacement.compile(assignment.clone()).unwrap();
    assert_eq!(d.probability(), &ExactScalar::zero());
    assert!(!c.shares_source_snapshot_with(&d));
    assert_eq!(
        c.branch().decisions()[0].inputs.entries(),
        &[("a".into(), false)]
    );
    assert_eq!(
        d.branch().decisions()[0].inputs.entries(),
        &[("a".into(), false), ("b".into(), false)]
    );
    assert_eq!(first.probability(assignment).unwrap(), ExactScalar::one());
}
#[test]
fn compact_inputs_and_certificates_retain_identity_without_retaining_source_callbacks() {
    let marker = Arc::new(());
    let weak = Arc::downgrade(&marker);
    let retained = marker.clone();
    drop(marker);
    let a = ScalarArithmetic::default();
    let ir = OriginalInstrument::new(
        vec![0],
        vec![0],
        CliffordCircuit::computational(bits(1, 0)).unwrap(),
        vec![OriginalOperation {
            id: "c".into(),
            gate: OriginalGate::Control,
        }],
        vec![OriginalControl {
            operation: "c".into(),
            parents: vec![],
            version: "1".into(),
            callback: Arc::new(move |_| {
                let _ = &retained;
                Ok(vec![OriginalGate::Identity(vec![0])])
            }),
        }],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    let source = OriginalAssignmentPlan::from_owned(ir, CompilerStrategy::ParityCopies, a);
    let input = source
        .compact(bits(0, 0), OriginalSharedFaults::empty())
        .unwrap();
    let certificate = source.compile(input.clone()).unwrap();
    assert!(weak.upgrade().is_some());
    drop(source);
    assert!(weak.upgrade().is_none());
    assert_eq!(certificate.probability(), &ExactScalar::one());
    assert_eq!(
        certificate.branch().amplitude(&bits(1, 0), 1, &a).unwrap(),
        ExactScalar::one()
    );
    assert_eq!(input.word(), &bits(0, 0));
}
#[test]
fn assignment_quota_refusals_precede_owned_capture_and_any_kernel_callback() {
    let a = ScalarArithmetic::default();
    let limits = OriginalLimits {
        max_faults: 0,
        ..Default::default()
    };
    let ir = OriginalInstrument::new(
        vec![0],
        vec![0],
        CliffordCircuit::computational(bits(1, 0)).unwrap(),
        vec![],
        vec![],
        limits,
        &a,
    )
    .unwrap();
    let source = OriginalAssignmentPlan::from_owned(ir, CompilerStrategy::ParityCopies, a);
    let calls = AtomicUsize::new(0);
    let result = source.compile_with_kernel(
        OriginalAssignment {
            faults: vec![fault(OriginalPauli::X)],
            ..Default::default()
        },
        |_, _| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(ExactScalar::one())
        },
    );
    assert!(matches!(
        result,
        Err(OriginalError::ResourceLimit {
            option: "max_faults",
            limit: 0,
            required: 1
        })
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(source.input_key_generations(), 0);
}

#[test]
fn original_public_trace_boundary_is_available_as_a_typed_rust_service() {
    use rmagna::phase::PathTraceBoundary;
    let a = ScalarArithmetic::default();
    let raw = environment_tensor(0);
    let word = rmagna::pauli::PauliWord::identity(0).unwrap();
    let traced = raw
        .tensor()
        .trace(
            PathTraceBoundary {
                retained_rows: &[],
                event_rows: &[],
                outcomes: &bits(0, 0),
                word: &word,
            },
            Default::default(),
            &a,
        )
        .unwrap();
    let resource = traced
        .prepare_native_resource(1, Default::default(), None, &a)
        .unwrap()
        .unwrap();
    let states = resource
        .expand(traced.phase(), traced.boundary(), &a)
        .unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(
        states[0]
            .prepare()
            .unwrap()
            .amplitude(&bits(0, 0), &a)
            .unwrap(),
        ExactScalar::one()
    );
}
#[test]
fn original_compiled_misuses_and_legacy_private_entries_are_rejected_downstream() {
    use std::{path::PathBuf, process::Command};
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let artifacts = build_artifacts::rmagna().unwrap();
    let library = &artifacts.library;
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = Scratch(std::env::temp_dir().join(format!(
        "rmagna-snapshot-types-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    )));
    std::fs::create_dir(&dir.0).unwrap();
    let cases = [
        (
            "valid",
            r#"use rmagna::{file_api::{FileTask,CompiledFileTask,FileSamplerComparisonOptions},task::TaskRunOptions,instrument::OriginalAssignmentPlan}; fn valid(source:&FileTask,compiled:&CompiledFileTask,plan:&OriginalAssignmentPlan){let _=source.compile();let _=compiled.run(&TaskRunOptions::default());let _=source.compare_samplers(&FileSamplerComparisonOptions::default());let _=plan.input_key_generations();let _=rmagna::noise::SparseNoiseAction::Pauli{operation:"a".into(),factors:vec![(0,rmagna::circuit::Basis::Y)]};}"#,
            None,
        ),
        (
            "compile_twice",
            r#"use rmagna::file_api::CompiledFileTask;fn invalid(compiled:&CompiledFileTask){compiled.compile();}"#,
            Some(("E0599", "compile")),
        ),
        (
            "compiled_pilot",
            r#"use rmagna::file_api::{CompiledFileTask,FileSamplerComparisonOptions};fn invalid(compiled:&CompiledFileTask){compiled.compare_samplers(&FileSamplerComparisonOptions::default());}"#,
            Some(("E0599", "compare_samplers")),
        ),
        (
            "old_batch",
            r#"use rmagna::evaluate_batch;"#,
            Some(("E0432", "evaluate_batch")),
        ),
        (
            "old_reference",
            r#"use rmagna::boundary_trace_reference_test;"#,
            Some(("E0432", "boundary_trace_reference_test")),
        ),
        (
            "invalid_pauli",
            r#"use rmagna::{noise::SparseNoiseAction,circuit::Basis};fn invalid(){let _=SparseNoiseAction::Pauli{operation:"a".into(),factors:vec![(0,Basis::Q)]};}"#,
            Some(("E0599", "Q")),
        ),
        (
            "negative_qubit",
            r#"use rmagna::{noise::SparseNoiseAction,circuit::Basis};fn invalid(){let _=SparseNoiseAction::Pauli{operation:"a".into(),factors:vec![(-1,Basis::X)]};}"#,
            Some(("E0600", "usize")),
        ),
    ];
    for (name, code, error) in cases {
        let input = dir.0.join(format!("{name}.rs"));
        std::fs::write(&input, code).unwrap();
        let mut command = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
        for directory in &artifacts.dependencies {
            command
                .arg("-L")
                .arg(format!("dependency={}", directory.display()));
        }
        let output = command
            .args([
                "--edition=2024",
                "--crate-type=lib",
                "--crate-name=snapshot_contract_probe",
                "--emit=metadata",
            ])
            .arg("--extern")
            .arg(format!("rmagna={}", library.display()))
            .arg(&input)
            .arg("-o")
            .arg(dir.0.join(format!("{name}.rmeta")))
            .output()
            .unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        if let Some((code, keyword)) = error {
            assert!(!output.status.success(), "{name} was incorrectly accepted");
            assert!(
                stderr.contains(code) && stderr.contains(keyword),
                "{name}: {stderr}"
            );
        } else {
            assert!(
                output.status.success(),
                "matching current rlib positive control failed: {stderr}"
            );
        }
    }
    if std::env::var_os("RMAGNA_SNAPSHOT_LAYOUT_CHILD").is_none() {
        let executable = std::env::current_exe().unwrap();
        let already_in_deps = executable
            .ancestors()
            .any(|p| p.file_name().is_some_and(|n| n == "deps"));
        let parent = artifacts
            .profile
            .join(if already_in_deps { "build" } else { "deps" });
        std::fs::create_dir_all(&parent).unwrap();
        let relocated = Scratch(parent.join(format!(
            "rmagna-snapshot-layout-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        )));
        let directory = if already_in_deps {
            relocated.0.join("out")
        } else {
            relocated.0.clone()
        };
        std::fs::create_dir_all(&directory).unwrap();
        let binary =
            directory.join("snapshot-layout-probe".to_owned() + std::env::consts::EXE_SUFFIX);
        std::fs::copy(&executable, &binary).unwrap();
        let copied_library = Scratch(artifacts.profile.join("build").join(format!(
            "rmagna-snapshot-library-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        )));
        let output = copied_library.0.join("out");
        std::fs::create_dir_all(&output).unwrap();
        std::fs::copy(library, output.join(library.file_name().unwrap())).unwrap();
        let child = Command::new(&binary)
            .env("RMAGNA_SNAPSHOT_LAYOUT_CHILD", "1")
            .args([
                "--exact",
                "original_compiled_misuses_and_legacy_private_entries_are_rejected_downstream",
                "--nocapture",
            ])
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "relocated Cargo layout failed: stdout={} stderr={}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
    }
    eprintln!(
        "downstream types:current-library positive control;two compiled-object misuses,two legacy entries,two sparse-action misuses;actual deps/build-output layouts;no Cargo/environment copies"
    );
}
#[test]
fn original_sampler_reconfiguration_is_an_explicit_fresh_preparation() {
    use rmagna::{
        circuit::{ParseOptions, parse_circuit},
        field::ExactField,
        noise::SplitMix64,
        runtime::{ControllerRegistry, HistorySampler},
        task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task},
    };
    let parse = ParseOptions::default();
    let circuit = parse_circuit("H 0\nM 0\n", &parse).unwrap();
    let modules = parse_task(
        r#"{"sample_records":[0]}"#,
        std::path::Path::new("."),
        &TaskOptions::default(),
    )
    .unwrap()
    .load_modules(&circuit, &parse)
    .unwrap();
    let mut options = EvaluationOptions::default();
    options.runtime.history_sampler = HistorySampler::ExactMarginal;
    let old =
        TaskEvaluator::new(&circuit, &modules, &ControllerRegistry::default(), options).unwrap();
    options.runtime.history_sampler = HistorySampler::AffineSupport;
    let replacement =
        TaskEvaluator::new(&circuit, &modules, &ControllerRegistry::default(), options).unwrap();
    let draw = replacement.sample(&mut SplitMix64::new(1)).unwrap();
    assert_eq!(draw.history_proposal_mass, Some(half()));
    assert_eq!(draw.accepted, ExactField::one());
    assert_eq!(draw.history_mass, half());
    assert_eq!(
        old.sample(&mut SplitMix64::new(1)).unwrap().accepted,
        ExactField::one()
    );
    assert_eq!(circuit.num_records(), 1);
}
#[test]
fn original_borrowed_and_owned_traces_keep_all_sixty_four_control_fault_event_queries() {
    use rmagna::pauli::PauliWord;
    use sha2::{Digest, Sha256};
    let raw = include_bytes!("fixtures/original_snapshot_queries.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(raw)),
        "66e61acaba96d4286f8e51c90a1e1e6c77fbbcd1cfaa74bcc6e09576416cbac9"
    );
    let fixture: serde_json::Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(
        fixture["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(fixture["original_queries"], 128);
    assert_eq!(
        fixture["generator_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../benchmarks/magna_reproduction/original_snapshot_queries.py"
            ))
        )
    );
    let rows = fixture["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 64);
    let a = ScalarArithmetic::default();
    let sqrt2over4 = a.scalar([0, 1, 0, -1].map(Into::into), -2).unwrap();
    let make = |limit| {
        let ops = vec![
            OriginalOperation {
                id: "p0".into(),
                gate: OriginalGate::Prepare {
                    wire: 0,
                    plus: false,
                },
            },
            OriginalOperation {
                id: "p1".into(),
                gate: OriginalGate::Prepare {
                    wire: 1,
                    plus: false,
                },
            },
            OriginalOperation {
                id: "h0".into(),
                gate: OriginalGate::H(0),
            },
            OriginalOperation {
                id: "h1".into(),
                gate: OriginalGate::H(1),
            },
            OriginalOperation {
                id: "t".into(),
                gate: OriginalGate::Phase {
                    wires: vec![0],
                    turns: 1,
                },
            },
            OriginalOperation {
                id: "cx".into(),
                gate: OriginalGate::Cx(0, 1),
            },
            OriginalOperation {
                id: "m".into(),
                gate: OriginalGate::Measure {
                    factors: vec![(1, OriginalPauli::Z)],
                    inverted: false,
                    record: "m0".into(),
                },
            },
            OriginalOperation {
                id: "c".into(),
                gate: OriginalGate::Control,
            },
            OriginalOperation {
                id: "discard".into(),
                gate: OriginalGate::Discard(1),
            },
        ];
        let expression = ControlExpression::record("m0".into(), Default::default()).unwrap();
        let gate = ControlGate::new(
            ControlGateKind::Y,
            vec![0],
            None,
            expression,
            Default::default(),
        )
        .unwrap();
        let declaration = OriginalDeclaredControl::new(
            "c".into(),
            vec!["m0".into()],
            "1".into(),
            ControlRule::new(vec![gate], Default::default()).unwrap(),
        )
        .unwrap();
        let mut limits = OriginalLimits::default();
        limits.max_path_bits = limit;
        let ir = OriginalInstrument::new_with_declared_controls(
            vec![],
            vec![0],
            CliffordCircuit::computational(bits(0, 0)).unwrap(),
            ops,
            vec![declaration],
            limits,
            &a,
        )
        .unwrap();
        ir
    };
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let ir = make(4096);
        let public = OriginalAssignmentPlan::new(&ir, strategy, a);
        let owned = OriginalAssignmentPlan::from_owned(ir.clone(), strategy, a);
        let public_trace = public.trace().unwrap();
        let owned_trace = owned.trace().unwrap();
        assert_eq!(public_trace.records(), &["m0"]);
        assert_eq!(public_trace.outputs(), &[0]);
        assert_eq!(public_trace.environment().len(), 1);
        // An explicit caller edit creates another source. Invalidating its H
        // target must affect that source, while both captured traces stay valid.
        let mut operations = ir.operations().to_vec();
        operations[2].gate = OriginalGate::H(99);
        let replaced = ir.with_operations(operations, &a).unwrap();
        assert!(matches!(
            OriginalAssignmentPlan::from_owned(replaced, strategy, a).trace(),
            Err(OriginalError::Lifetime {
                wire: 99,
                expected_live: true
            })
        ));
        drop(public);
        drop(owned);
        drop(ir);
        for (index, row) in rows.iter().enumerate() {
            let pauli = match row["pauli"].as_str().unwrap() {
                "I" => OriginalPauli::I,
                "X" => OriginalPauli::X,
                "Y" => OriginalPauli::Y,
                "Z" => OriginalPauli::Z,
                _ => panic!("invalid fixture"),
            };
            assert_eq!(row["pauli"], ["I", "X", "Y", "Z"][index / 16]);
            assert_eq!(row["flipped"], index / 8 % 2 != 0);
            assert_eq!(row["outcome"], index / 4 % 2 != 0);
            let x = row["x"].as_u64().unwrap() as usize;
            let z = row["z"].as_u64().unwrap() as usize;
            let bit = row["outcome"].as_bool().unwrap();
            assert_eq!((x, z), [(0, 0), (1, 0), (0, 1), (1, 1)][index % 4]);
            let assignment = OriginalTraceAssignment {
                faults: vec![fault(pauli)],
                report_flips: if row["flipped"].as_bool().unwrap() {
                    vec!["m0".into()]
                } else {
                    vec![]
                },
            };
            let event = OriginalRecordEvent {
                parity: OriginalRecordParity {
                    records: vec!["m0".into()],
                    inverted: false,
                },
                value: bit,
            };
            let word =
                PauliWord::new(bits(1, x), bits(1, z), if x & z != 0 { 2 } else { 0 }).unwrap();
            let first = public_trace
                .with_assignment(&assignment, &a)
                .unwrap()
                .output_moment(
                    &[],
                    &[event.clone()],
                    &word,
                    &OriginalTraceOptions::default(),
                    &a,
                )
                .unwrap();
            let second = owned_trace
                .with_assignment(&assignment, &a)
                .unwrap()
                .output_moment(&[], &[event], &word, &OriginalTraceOptions::default(), &a)
                .unwrap();
            assert_eq!(
                first.distribution.contributions(),
                second.distribution.contributions()
            );
            let coefficients: [rmagna::BigInt; 4] = std::array::from_fn(|j| {
                row["expected"]["coefficients"][j].as_i64().unwrap().into()
            });
            let expected = a
                .scalar(coefficients, row["expected"]["power"].as_i64().unwrap())
                .unwrap();
            let literal = match (x, z) {
                (0, 0) => half(),
                (0, 1) => ExactScalar::zero(),
                (1, 0) => {
                    let negative = matches!(pauli, OriginalPauli::Y | OriginalPauli::Z) ^ bit;
                    if negative {
                        a.multiply(&sqrt2over4, &a.omega(4, 0).unwrap()).unwrap()
                    } else {
                        sqrt2over4.clone()
                    }
                }
                (1, 1) => {
                    if matches!(pauli, OriginalPauli::X | OriginalPauli::Z) {
                        a.multiply(&sqrt2over4, &a.omega(4, 0).unwrap()).unwrap()
                    } else {
                        sqrt2over4.clone()
                    }
                }
                _ => unreachable!(),
            };
            assert_eq!(expected, literal, "Native/literal {index}");
            assert_eq!(
                first.distribution.total(&a).unwrap(),
                expected,
                "borrowed {index}"
            );
            assert_eq!(
                second.distribution.total(&a).unwrap(),
                expected,
                "owned {index}"
            );
        }
        let ir = make(0);
        let public = OriginalAssignmentPlan::new(&ir, strategy, a);
        let owned = OriginalAssignmentPlan::from_owned(ir, strategy, a);
        assert!(matches!(
            public.trace(),
            Err(OriginalError::Unitary(
                rmagna::unitary::UnitaryError::RawPathLimit { limit: 0, .. }
            ))
        ));
        assert!(matches!(
            owned.trace(),
            Err(OriginalError::Unitary(
                rmagna::unitary::UnitaryError::RawPathLimit { limit: 0, .. }
            ))
        ));
    }
    eprintln!(
        "owned/borrowed:original64query schedule x2compilers x2plans;Native+literal signs;four quota refusals"
    );
}
fn sparse_native_factors(value: &serde_json::Value) -> Vec<(usize, rmagna::circuit::Basis)> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|gate| {
            let basis = match gate["kind"].as_str().unwrap() {
                "X" => rmagna::circuit::Basis::X,
                "Y" => rmagna::circuit::Basis::Y,
                "Z" => rmagna::circuit::Basis::Z,
                _ => panic!("non-Pauli captured action"),
            };
            assert_eq!(gate["qubits"].as_array().unwrap().len(), 1);
            (gate["qubits"][0].as_u64().unwrap() as usize, basis)
        })
        .collect()
}
#[test]
fn original_sparse_actions_match_all_hundred_dense_draws_and_remain_isolated() {
    use num_rational::BigRational;
    use rmagna::{
        circuit::{ParseOptions, parse_circuit},
        instrument::{CompileOptions, FaultSelection, compile_affine},
        noise::{
            NoiseModel, NoiseOptions, NoiseSampler, NoiseSampling, SparseNoiseAction,
            SparseNoiseOptions, SparseNoiseSampler, SplitMix64, original_noise_categories,
        },
    };
    use sha2::{Digest, Sha256};
    use std::collections::{BTreeMap, BTreeSet};
    let raw = include_bytes!("fixtures/original_sparse_ownership.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(raw)),
        "b65daed64c008f1ff65d71495c6bde070cf2192238c3731e3419d9c57f5e4e5f"
    );
    let fixture: serde_json::Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(
        fixture["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(fixture["original_dense_draws"], 100);
    assert_eq!(fixture["original_sparse_draws"], 200);
    assert_eq!(
        fixture["generator_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../benchmarks/magna_reproduction/original_sparse_ownership.py"
            ))
        )
    );
    let circuit=parse_circuit("DEPOLARIZE2(1/3) 0 1\nCORRELATED_ERROR(1/5) X0 Z1\nELSE_CORRELATED_ERROR(1/7) Y0\nM(1/4) 0 1",&ParseOptions::default()).unwrap();
    let instrument = compile_affine(
        &circuit,
        &CompileOptions::default(),
        &ScalarArithmetic::default(),
    )
    .unwrap();
    let policy = NoiseSampling::Importance {
        proposal_scale: BigRational::new(3.into(), 2.into()),
    };
    let dense = NoiseSampler::configured(
        NoiseModel::physical(&instrument, &NoiseOptions::default()).unwrap(),
        &policy,
        &BTreeSet::new(),
        NoiseOptions::default(),
    )
    .unwrap();
    let mut categories = original_noise_categories(&circuit, &Default::default()).unwrap();
    let sparse =
        SparseNoiseSampler::from_categories(&categories, &policy, SparseNoiseOptions::default())
            .unwrap();
    let native_actions = fixture["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|site| {
            site.as_array()
                .unwrap()
                .iter()
                .map(|entry| {
                    let operation = entry[0].as_str().unwrap();
                    if operation == "report" {
                        SparseNoiseAction::ReportFlip {
                            record: entry[1].as_str().unwrap().into(),
                        }
                    } else {
                        SparseNoiseAction::Pauli {
                            operation: operation.into(),
                            factors: sparse_native_factors(&entry[1]),
                        }
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(native_actions.len(), sparse.actions().len());
    let dense_rows = fixture["dense"]["rows"].as_array().unwrap();
    let sparse_rows = fixture["sparse"].as_array().unwrap();
    let seeds = fixture["dense"]["seeds"].as_array().unwrap();
    assert_eq!(
        (dense_rows.len(), sparse_rows.len(), seeds.len()),
        (100, 100, 100)
    );
    let mut originals = Vec::new();
    for index in 0..100 {
        let selected = dense_rows[index]["choices"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .filter_map(|(site, choice)| {
                let choice = choice.as_i64().unwrap();
                if choice < 0 {
                    return None;
                }
                let action = &native_actions[site][choice as usize];
                let alternative = sparse.actions()[site]
                    .iter()
                    .position(|a| a == action)
                    .expect("explicit action-order mapping");
                Some(FaultSelection { site, alternative })
            })
            .collect::<Vec<_>>();
        let replay = sparse.prescribed(&selected).unwrap();
        let expected = &sparse_rows[index];
        let gates = expected["gates"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, value)| (name.clone(), sparse_native_factors(value)))
            .collect::<BTreeMap<_, _>>();
        let flips = expected["report_flips"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        let weight = expected["weight"]
            .as_str()
            .unwrap()
            .parse::<BigRational>()
            .unwrap();
        assert_eq!(replay.gates, gates, "Native actions {index}");
        assert_eq!(replay.report_flips, flips, "Native reports {index}");
        assert_eq!(
            replay.noise.likelihood_ratio, weight,
            "Native weight {index}"
        );
        assert_eq!(replay.noise.entropy_words, 0);
        let seed = seeds[index].as_u64().unwrap();
        let mut dense_rng = SplitMix64::new(seed);
        let mut sparse_rng = SplitMix64::new(seed);
        let baseline = dense.draw(&mut dense_rng).unwrap();
        let actual = sparse.draw(&mut sparse_rng).unwrap();
        assert_eq!(actual.noise, baseline);
        assert_eq!(sparse_rng.state(), dense_rng.state());
        originals.push(actual);
    }
    let empty = sparse.prescribed(&[]).unwrap();
    assert_eq!(
        empty.noise.physical_probability,
        BigRational::new(9.into(), 35.into())
    );
    assert_eq!(
        empty.noise.proposal_probability,
        BigRational::new(185.into(), 1792.into())
    );
    assert_eq!(
        empty.noise.likelihood_ratio,
        BigRational::new(2304.into(), 925.into())
    );
    for category in &mut categories {
        for channel in &mut category.channels {
            if let rmagna::noise::OriginalNoiseAction::Pauli { factors } = &mut channel.action {
                factors.clear();
            }
        }
    }
    for (index, seed) in seeds.iter().enumerate() {
        assert_eq!(
            sparse
                .draw(&mut SplitMix64::new(seed.as_u64().unwrap()))
                .unwrap(),
            originals[index]
        );
    }
    eprintln!(
        "sparse ownership:all100Native choices/weights/actions;own100dense+200sparse seeded draws;state equality;caller mutation isolation;exact empty mass"
    );
}
#[test]
fn original_direct_sparse_action_table_owns_nested_factors_and_rejects_shape_errors() {
    use num_rational::BigRational;
    use rmagna::{
        circuit::Basis,
        noise::{
            NoiseModel, NoiseOptions, NoiseSampling, SparseNoiseAction, SparseNoiseError,
            SparseNoiseOptions, SparseNoiseSampler, SplitMix64,
        },
        probability::Probability,
    };
    let model = || {
        NoiseModel::new(
            vec![vec![
                Probability::from_rational(BigRational::from_integer(0.into())).unwrap(),
                Probability::from_rational(BigRational::from_integer(1.into())).unwrap(),
            ]],
            &NoiseOptions::default(),
        )
        .unwrap()
    };
    let mut table = vec![vec![SparseNoiseAction::Pauli {
        operation: "a".into(),
        factors: vec![(0, Basis::Y)],
    }]];
    let sampler = SparseNoiseSampler::new(
        model(),
        &table,
        &NoiseSampling::Physical,
        Default::default(),
    )
    .unwrap();
    if let SparseNoiseAction::Pauli { factors, .. } = &mut table[0][0] {
        factors.clear();
    }
    let row = sampler.draw(&mut SplitMix64::new(7)).unwrap();
    assert_eq!(row.gates.get("a").unwrap(), &vec![(0, Basis::Y)]);
    assert!(row.report_flips.is_empty());
    assert_eq!(
        row.noise.likelihood_ratio,
        BigRational::from_integer(1.into())
    );
    assert!(matches!(
        SparseNoiseSampler::new(model(), &[], &NoiseSampling::Physical, Default::default()),
        Err(SparseNoiseError::Shape { site: None })
    ));
    assert!(matches!(
        SparseNoiseSampler::new(
            model(),
            &[vec![]],
            &NoiseSampling::Physical,
            Default::default()
        ),
        Err(SparseNoiseError::Shape { site: Some(0) })
    ));
    let options = SparseNoiseOptions {
        max_name_bytes: 0,
        ..Default::default()
    };
    assert!(matches!(
        SparseNoiseSampler::new(
            model(),
            sampler.actions(),
            &NoiseSampling::Physical,
            options
        ),
        Err(SparseNoiseError::Limit {
            option: "sparse_noise.max_name_bytes",
            ..
        })
    ));
}

#[test]
fn original_partial_feedback_incidence_matches_all_162_queries() {
    use rmagna::circuit::{
        Basis, BitAtom, FeedbackTemplate, FeedbackTemplateError, FeedbackTemplateOptions, Gate,
        ParseOptions, parse_circuit,
    };
    use std::collections::{BTreeMap, BTreeSet};
    let text = "H 0\nM 0\nDETECTOR rec[-1]\nCONTROL[bits] det[-1] -> ctrl[0] ctrl[1]\nCONTROL[XOR](1) ctrl[0] ctrl[1] -> ctrl[2]\nCX ctrl[0] 1\nCS ctrl[2] 1\nCZ ctrl[1] 2\nDETECTOR rec[-1]\nCONTROL[bits] det[-1] -> ctrl[0] ctrl[1]\nCONTROL[XOR] ctrl[0] ctrl[1] ctrl[2] -> ctrl[3]\nCY ctrl[3] 2\nCONTROL[NOT] ctrl[3] -> ctrl[4]\nPOSTSELECT ctrl[4] ctrl[2]\n";
    let circuit = parse_circuit(
        text,
        &ParseOptions {
            controllers: BTreeSet::from(["bits".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    let template = FeedbackTemplate::new(&circuit, Default::default()).unwrap();
    use sha2::{Digest, Sha256};
    let raw = include_bytes!("fixtures/original_feedback_incidence.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(raw)),
        "c9869bedd5d18d688e44edafd4f293bb3351466bff131511ce404f3a76236928"
    );
    let original: serde_json::Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(
        original["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(
        original["generator_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../benchmarks/magna_reproduction/original_feedback_incidence.py"
            ))
        )
    );
    let atom = |value: &serde_json::Value| {
        assert_eq!(value[0], "ctrl");
        BitAtom::Decision {
            call: value[2].as_u64().unwrap() as usize,
            bit: value[1].as_u64().unwrap() as usize,
        }
    };
    let axis = |s: &str| match s {
        "X" => Basis::X,
        "Y" => Basis::Y,
        "Z" => Basis::Z,
        _ => panic!("invalid captured axis"),
    };
    assert_eq!(template.wire_count(), 6);
    assert_eq!(template.updates().len(), 6);
    assert_eq!(original["template"]["wire_count"], 6);
    let mut terms = BTreeSet::new();
    for (update, row) in template
        .updates()
        .iter()
        .zip(original["template"]["updates"].as_array().unwrap())
    {
        assert_eq!(update.wire, row["wire"].as_u64().unwrap() as usize);
        assert_eq!(update.pauli, axis(row["pauli"].as_str().unwrap()));
        assert_eq!(
            update.selection_offset,
            row["offset"].as_u64().unwrap() != 0
        );
        assert_eq!(
            update.decisions,
            row["terms"].as_array().unwrap().iter().map(atom).collect()
        );
        terms.extend(update.decisions.iter().cloned());
    }
    assert_eq!(terms.len(), 4);
    assert_eq!(template.virtual_controls().len(), 1);
    let control = &template.virtual_controls()[0];
    assert_eq!(
        (
            control.operation,
            control.wire,
            control.gate,
            control.targets.as_slice()
        ),
        (6, 3, Gate::S, &[1][..])
    );
    assert!(template.updates()[1].guard.constant); // XOR(1), retained by fixed CS gate
    assert!(!template.updates()[4].guard.constant); // NOT/XOR constants go to selection offset
    let cases = original["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 162);
    for (case_index, row) in cases.iter().enumerate() {
        let values = row["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| (atom(&v[0]), v[1].as_u64().unwrap() != 0))
            .collect::<BTreeMap<_, _>>();
        let prefix = row["prefix"].as_str().unwrap();
        let mapping = (0..template.wire_count())
            .map(|w| (w, w + 17))
            .collect::<BTreeMap<_, _>>();
        let wires = if prefix.is_empty() {
            None
        } else {
            Some(&mapping)
        };
        let faults = template.faults(&values, prefix, wires).unwrap();
        let expected = row["faults"].as_array().unwrap();
        assert_eq!(faults.len(), expected.len(), "case {case_index}");
        for (fault, expected) in faults.iter().zip(expected) {
            let update = &template.updates()[expected["row"].as_u64().unwrap() as usize];
            assert_eq!(fault.after, format!("{prefix}op{}", update.operation));
            assert_eq!(fault.wire, expected["wire"].as_u64().unwrap() as usize);
            assert_eq!(fault.pauli, axis(expected["pauli"].as_str().unwrap()));
        }
        // Independent XOR scan, without the service's reverse incidence index.
        let scan = template
            .updates()
            .iter()
            .filter(|u| {
                u.decisions.iter().fold(u.selection_offset, |b, t| {
                    b ^ values.get(t).copied().unwrap_or(false)
                })
            })
            .map(|u| u.operation)
            .collect::<Vec<_>>();
        assert_eq!(
            faults
                .iter()
                .map(|f| f
                    .after
                    .strip_prefix(prefix)
                    .unwrap()
                    .strip_prefix("op")
                    .unwrap()
                    .parse::<usize>()
                    .unwrap())
                .collect::<Vec<_>>(),
            scan
        );
    }
    assert!(matches!(
        template.faults(&BTreeMap::new(), "", Some(&BTreeMap::new())),
        Err(FeedbackTemplateError::MissingWire(5))
    ));
    for options in [
        FeedbackTemplateOptions {
            max_wires: 5,
            ..Default::default()
        },
        FeedbackTemplateOptions {
            max_updates: 5,
            ..Default::default()
        },
        FeedbackTemplateOptions {
            max_incidences: 0,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            FeedbackTemplate::new(&circuit, options),
            Err(FeedbackTemplateError::Limit { .. })
        ));
    }
    eprintln!(
        "feedback incidence:4 versioned decisions x81 partial assignments x2 prefix/wire maps;all6 rows,CS virtual control,selection constants,3 quotas,missing map"
    );
}

#[test]
fn original_trace_cache_obeys_source_and_budget_identity_and_weak_eviction() {
    use rmagna::pauli::PauliWord;
    let a = ScalarArithmetic::default();
    let make = |decision: i32| {
        let x_when = ControlExpression::compose(
            ControlOperator::Xor,
            vec![
                ControlExpression::constant(decision),
                ControlExpression::record("m".into(), Default::default()).unwrap(),
            ],
            Default::default(),
        )
        .unwrap();
        let gates = vec![
            ControlGate::new(
                ControlGateKind::X,
                vec![1],
                None,
                x_when,
                Default::default(),
            )
            .unwrap(),
            ControlGate::new(
                ControlGateKind::Phase,
                vec![1],
                Some(ControlExpression::constant(2)),
                ControlExpression::constant(decision),
                Default::default(),
            )
            .unwrap(),
        ];
        let ir = OriginalInstrument::new_with_declared_controls(
            vec![0, 1],
            vec![1],
            CliffordCircuit::computational(bits(2, 0)).unwrap(),
            vec![
                OriginalOperation {
                    id: "m".into(),
                    gate: OriginalGate::Measure {
                        factors: vec![(0, OriginalPauli::Z)],
                        inverted: false,
                        record: "m".into(),
                    },
                },
                OriginalOperation {
                    id: "c".into(),
                    gate: OriginalGate::Control,
                },
                OriginalOperation {
                    id: "discard".into(),
                    gate: OriginalGate::Discard(0),
                },
            ],
            vec![
                OriginalDeclaredControl::new(
                    "c".into(),
                    vec!["m".into()],
                    "echo-specialized-v1".into(),
                    ControlRule::new(gates, Default::default()).unwrap(),
                )
                .unwrap(),
            ],
            OriginalLimits::default(),
            &a,
        )
        .unwrap();
        OriginalAssignmentPlan::from_owned(ir, CompilerStrategy::ParityCopies, a)
    };
    let zero_source = make(0);
    let one_source = make(1);
    let moment = |trace: &OriginalStaticTrace| {
        trace
            .with_assignment(&Default::default(), &a)
            .unwrap()
            .output_moment(
                &[],
                &[],
                &PauliWord::new(bits(1, 0), bits(1, 1), 0).unwrap(),
                &OriginalTraceOptions::default(),
                &a,
            )
            .unwrap()
            .distribution
            .total(&a)
            .unwrap()
    };
    let mut cache = OriginalTraceCache::new(1);
    let zero = cache.prepare(&zero_source, 1 << 20).unwrap();
    let zero_ref = Arc::downgrade(&zero);
    assert!(Arc::ptr_eq(
        &zero,
        &cache.prepare(&zero_source, 1 << 20).unwrap()
    ));
    let one = cache.prepare(&one_source, 8192).unwrap();
    assert!(!Arc::ptr_eq(&zero, &one));
    assert_eq!(cache.len(), 1);
    assert_eq!(moment(&zero), ExactScalar::one());
    assert_eq!(moment(&one), a.omega(4, 0).unwrap());
    drop(zero);
    assert!(zero_ref.upgrade().is_none());
    let again = cache.prepare(&zero_source, 1 << 20).unwrap();
    assert_eq!(moment(&again), ExactScalar::one());
    assert_eq!(
        cache.statistics(),
        OriginalTraceCacheStatistics {
            compilation_attempts: 3,
            compilations: 3,
            hits: 1,
            evictions: 2
        }
    );
    // Same source, different admitted budget must prepare again as well.
    let budget_changed = cache.prepare(&zero_source, 8192).unwrap();
    assert!(!Arc::ptr_eq(&again, &budget_changed));
    assert_eq!(cache.statistics().compilations, 4);
    let mut disabled = OriginalTraceCache::new(0);
    let first = disabled.prepare(&zero_source, 8192).unwrap();
    let first_ref = Arc::downgrade(&first);
    drop(first);
    assert!(first_ref.upgrade().is_none());
    disabled.prepare(&zero_source, 8192).unwrap();
    assert!(disabled.is_empty());
    assert_eq!(disabled.statistics().compilations, 2);
    // Admission failure cannot return a more generously prepared cached trace.
    let phased = plan(1, None, None);
    assert!(cache.prepare(&phased, 0).is_err());
    assert_eq!(cache.statistics().compilations, 4);
    assert_eq!(cache.statistics().compilation_attempts, 5);
    assert_eq!(moment(&budget_changed), ExactScalar::one());
    eprintln!(
        "trace cache:original specialized echo/XOR/CS zero->one->zero;three real compilations;external +/-Z;Weak expiry;budget change;disabled retention;no fallback on refusal"
    );
}

#[test]
fn original_explicit_literal_policy_compiles_once_without_structural_preparation() {
    let a = ScalarArithmetic::default();
    let initial = CliffordCircuit::new(1, [rmagna::clifford::CliffordGate::H(0)])
        .unwrap()
        .evolve(&CliffordCircuit::computational(bits(1, 0)).unwrap(), &a)
        .unwrap()
        .unwrap();
    let ir = OriginalInstrument::new(
        vec![0],
        vec![0],
        initial,
        vec![],
        vec![],
        Default::default(),
        &a,
    )
    .unwrap();
    let source = OriginalAssignmentPlan::from_owned(ir, CompilerStrategy::ParityCopies, a);
    assert_eq!(
        source.execution_policy(),
        OriginalAssignmentExecutionPolicy::LiteralPerAssignment
    );
    let mut batch = source.probabilities([OriginalAssignment::default()]);
    assert_eq!(batch.next().unwrap().unwrap(), ExactScalar::one());
    assert!(batch.next().is_none());
    assert_eq!(batch.statistics().compiled_branches, 1);
    assert_eq!(batch.statistics().values, 1);
    assert_eq!(source.input_key_generations(), 0);
    let certificate = source.compile(OriginalAssignment::default()).unwrap();
    assert_eq!(certificate.probability(), &ExactScalar::one());
    for x in 0..2 {
        assert_eq!(
            certificate
                .branch()
                .amplitude(&bits(1, x), 256, &a)
                .unwrap(),
            a.omega(0, -1).unwrap()
        );
    }
    eprintln!(
        "preparation policy:explicit literal;one actual branch compilation;no structure cache/native preparation attempt or hidden fallback"
    );
}

#[test]
fn original_guarded_supplied_noise_retains_owned_actions_after_inputs_are_dropped() {
    use rmagna::{
        circuit::{ParseOptions, parse_circuit},
        noise::SplitMix64,
        pauli::PauliWord,
        runtime::{CausalProgram, CausalStrategy, ControllerRegistry, RuntimeOptions},
    };
    let a = ScalarArithmetic::default();
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        let circuit = parse_circuit(
            "CONTROL[XOR](1) -> ctrl[0]\nX_ERROR(1/10) ctrl[0] 1\n",
            &ParseOptions::default(),
        )
        .unwrap();
        let program = CausalProgram::new(
            &circuit,
            &ControllerRegistry::default(),
            RuntimeOptions {
                strategy,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
        let site = program.potential_site(1).unwrap();
        let mut faults = vec![FaultSelection {
            site,
            alternative: 0,
        }];
        let history = program
            .resolve_supplied_noise(
                &[],
                &faults,
                &num_rational::BigRational::from_integer(1.into()),
                &mut SplitMix64::new(1),
                &a,
            )
            .unwrap();
        faults.clear();
        drop(program);
        drop(circuit);
        assert_eq!(
            history.faults(),
            &[FaultSelection {
                site,
                alternative: 0
            }]
        );
        let found = history
            .instrument()
            .pauli_moment(
                &[1],
                &PauliWord::new(bits(1, 0), bits(1, 1), 0).unwrap(),
                history.faults(),
                256,
                &a,
            )
            .unwrap()
            .value;
        assert_eq!(found, a.omega(4, 0).unwrap());
    }
    eprintln!(
        "guarded supplied noise:constant XOR guard,X fault;both causal compilers;caller fault replacement/source drop;retained physical -Z;arbitrary Python extras remain unsupported"
    );
}

#[test]
fn compact_noise_empty_report_schedule_never_consumes_input_rows() {
    let source = batch_plan(true);
    let layout = OriginalCompactNoiseLayout::new(&source, vec![]).unwrap();
    let pulls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&pulls);
    let rows = std::iter::from_fn(move || {
        observed.fetch_add(1, Ordering::SeqCst);
        Some(vec![])
    });
    let mut stream = layout.assignments(rows, vec![]).unwrap();
    assert!(stream.next().is_none());
    assert!(stream.next().is_none());
    assert_eq!(pulls.load(Ordering::SeqCst), 0);
}

// Complete F2 ownership/refusal methods use the authoritative declared rule;
// the legacy opaque-callback/literal witnesses above remain independent.
fn fallback_positive_ket() -> rmagna::stabilizer::AffineKet {
    rmagna::stabilizer::AffineKet::new(
        bits(1, 0),
        vec![bits(1, 1)],
        rmagna::quadratic::QuadraticForm::new(0, vec![0], vec![bits(1, 0)]).unwrap(),
        ScalarArithmetic::default().omega(0, -1).unwrap(),
    )
    .unwrap()
}
fn fallback_positive(strategy: CompilerStrategy, limits: OriginalLimits) -> OriginalAssignmentPlan {
    let a = ScalarArithmetic::default();
    let initial = fallback_positive_ket();
    OriginalAssignmentPlan::from_owned(
        OriginalInstrument::new(vec![0], vec![0], initial, vec![], vec![], limits, &a).unwrap(),
        strategy,
        a,
    )
}
fn fallback_declared(
    slope: i32,
    strategy: CompilerStrategy,
    nonlinear: bool,
    limits: OriginalLimits,
) -> OriginalAssignmentPlan {
    let a = ScalarArithmetic::default();
    let l = ControlRuleLimits::default();
    let bit = ControlExpression::record("m".into(), l).unwrap();
    let exponent = ControlExpression::compose(
        ControlOperator::Add,
        vec![
            ControlExpression::constant(1),
            ControlExpression::compose(
                ControlOperator::Multiply,
                vec![ControlExpression::constant(slope), bit.clone()],
                l,
            )
            .unwrap(),
        ],
        l,
    )
    .unwrap();
    let when = if nonlinear {
        ControlExpression::compose(ControlOperator::And, vec![bit.clone(), bit], l).unwrap()
    } else {
        ControlExpression::constant(1)
    };
    let declaration = OriginalDeclaredControl::new(
        "c".into(),
        vec!["m".into()],
        "owned-rule-v1".into(),
        ControlRule::new(
            vec![ControlGate::new(ControlGateKind::T, vec![0], Some(exponent), when, l).unwrap()],
            l,
        )
        .unwrap(),
    )
    .unwrap();
    // The independently declared source has the same positive ket, MZ/CONTROL/MX
    // operations and rule 1 + record_bit as the pinned original refusal method.
    let ir = OriginalInstrument::new_with_declared_controls(
        vec![0],
        vec![0],
        fallback_positive_ket(),
        vec![
            OriginalOperation {
                id: "m".into(),
                gate: OriginalGate::Measure {
                    factors: vec![(0, OriginalPauli::Z)],
                    inverted: false,
                    record: "m".into(),
                },
            },
            OriginalOperation {
                id: "c".into(),
                gate: OriginalGate::Control,
            },
            OriginalOperation {
                id: "n".into(),
                gate: OriginalGate::Measure {
                    factors: vec![(0, OriginalPauli::X)],
                    inverted: false,
                    record: "n".into(),
                },
            },
        ],
        vec![declaration],
        limits,
        &a,
    )
    .unwrap();
    OriginalAssignmentPlan::from_owned(ir, strategy, a)
}
fn fallback_options() -> OriginalAssignmentExecutionOptions<'static> {
    OriginalAssignmentExecutionOptions {
        max_structure_geometry_words: 0,
        ..Default::default()
    }
}
#[test]
fn original_opt_in_preparation_refusal_compiles_the_empty_positive_branch_once() {
    let a = ScalarArithmetic::default();
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let source = fallback_positive(strategy, Default::default());
        let expected = source.compile(OriginalAssignment::default()).unwrap();
        let mut executor = source.executor(
            OriginalAssignmentExecutionPolicy::PrepareOrLiteral,
            fallback_options(),
        );
        assert_eq!(
            executor.execution_policy(),
            OriginalAssignmentExecutionPolicy::PrepareOrLiteral
        );
        assert_eq!(
            source.execution_policy(),
            OriginalAssignmentExecutionPolicy::LiteralPerAssignment
        );
        let found = executor.compile(OriginalAssignment::default()).unwrap();
        assert_eq!(
            found.route(),
            OriginalAssignmentExecutionRoute::PreparationFallback
        );
        assert!(
            matches!(found.fallback_reason(), Some(OriginalAssignmentFallbackReason::PreparationGeometry {
            limit:0,required }) if *required > 0)
        );
        assert_eq!(
            found.literal_certificate().unwrap().to_json(),
            expected.to_json()
        );
        assert_eq!(found.probability(), &ExactScalar::one());
        for y in 0..2 {
            assert_eq!(
                found
                    .literal_certificate()
                    .unwrap()
                    .branch()
                    .amplitude(&bits(1, y), 256, &a)
                    .unwrap(),
                a.omega(0, -1).unwrap()
            );
        }
        println!("F2 empty positive exact mass/amplitudes matched before work assertions");
        assert_eq!(
            found.statistics(),
            OriginalAssignmentExecutionStatistics {
                preparation_attempts: 1,
                literal_compilation_attempts: 1,
                literal_compilations: 1,
                ..Default::default()
            }
        );
        assert_eq!(executor.statistics(), found.statistics());
        println!(
            "F2 preparation budget {strategy:?}: reason={:?}; work={:?}; probability=1; both branch amplitudes=1/sqrt(2)",
            found.fallback_reason(),
            found.statistics()
        );
        // The same source/options with an explicit literal policy never prepare.
        let mut literal = source.executor(
            OriginalAssignmentExecutionPolicy::LiteralPerAssignment,
            fallback_options(),
        );
        let found = literal.compile(OriginalAssignment::default()).unwrap();
        assert_eq!(found.route(), OriginalAssignmentExecutionRoute::Literal);
        assert!(found.fallback_reason().is_none());
        assert_eq!(found.statistics().preparation_attempts, 0);
        assert_eq!(found.statistics().literal_compilations, 1);
        assert_eq!(
            found.literal_certificate().unwrap().to_json(),
            expected.to_json()
        );
    }
}
#[test]
fn original_opt_in_odd_phase_refusal_keeps_packed_inputs_and_collected_rule() {
    let a = ScalarArithmetic::default();
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for with_metadata in [false, true] {
            let source = fallback_declared(1, strategy, false, Default::default());
            let mut original = Arc::new(OriginalAssignment {
                true_values: vec![("m".into(), true), ("n".into(), false)],
                faults: if with_metadata {
                    vec![OriginalFault {
                        after: "m".into(),
                        wire: 0,
                        pauli: OriginalPauli::Y,
                    }]
                } else {
                    vec![]
                },
                report_flips: if with_metadata {
                    vec!["m".into(), "n".into(), "m".into()]
                } else {
                    vec![]
                },
            });
            let expected = source.compile(Arc::clone(&original)).unwrap();
            let boundary_calls = AtomicUsize::new(0);
            let mut executor = source.executor(
                OriginalAssignmentExecutionPolicy::PrepareOrLiteral,
                Default::default(),
            );
            let found = executor
                .compile_with_boundary(Arc::clone(&original), |scratch, reason| {
                    boundary_calls.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(
                        reason,
                        Some(&OriginalAssignmentFallbackReason::ControlledOddPhase {
                            operation: "c".into()
                        })
                    );
                    scratch.true_values[0].1 = false;
                    scratch.true_values[1].1 = true;
                    scratch.faults = vec![OriginalFault {
                        after: "c".into(),
                        wire: 0,
                        pauli: OriginalPauli::X,
                    }];
                    scratch.report_flips = vec!["m".into()];
                    *Arc::get_mut(&mut original).expect("input Arc released before boundary") =
                        scratch.clone();
                    Ok(())
                })
                .unwrap();
            assert_eq!(boundary_calls.load(Ordering::SeqCst), 1);
            assert_eq!(
                found.route(),
                OriginalAssignmentExecutionRoute::PreparationFallback
            );
            assert_eq!(found.probability(), expected.probability());
            println!("F2 exact mass matched before ownership/work assertions");
            assert_eq!(
                found.literal_certificate().unwrap().to_json(),
                expected.to_json()
            );
            assert_eq!(found.assignment().true_values[0], ("m".into(), true));
            assert_eq!(found.decisions()[0].inputs.entries(), &[("m".into(), true)]);
            assert_eq!(found.decisions().len(), 1);
            assert_eq!(
                found.statistics(),
                OriginalAssignmentExecutionStatistics {
                    preparation_attempts: 1,
                    control_evaluations: 1,
                    control_completions: 1,
                    literal_compilation_attempts: 1,
                    literal_compilations: 1,
                    boundary_observations: 1,
                    ..Default::default()
                }
            );
            for y in 0..2 {
                let amplitude = found
                    .literal_certificate()
                    .unwrap()
                    .branch()
                    .amplitude(&bits(1, y), 1024, &a)
                    .unwrap();
                assert_eq!(
                    amplitude,
                    expected.branch().amplitude(&bits(1, y), 1024, &a).unwrap()
                );
                if !with_metadata {
                    assert_eq!(amplitude, a.omega(2, -3).unwrap());
                }
            }
            let mutated = source.compile(Arc::clone(&original)).unwrap();
            assert_ne!(
                mutated.to_json(),
                expected.to_json(),
                "mutated inputs must be distinguishable even when their mass agrees"
            );
            // Returned observations are owned and do not include later oracle work.
            assert_eq!(found.statistics().control_evaluations, 1);
            println!(
                "F2 odd phase {strategy:?} metadata={with_metadata}: reason={:?}; work={:?}; frozen exact branch and probability match; zero rule replay",
                found.fallback_reason(),
                found.statistics()
            );
        }
    }
}
#[test]
fn original_opt_in_global_success_matches_literal_without_literal_compilation() {
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let source = fallback_positive(strategy, Default::default());
        let mut executor = source.executor(
            OriginalAssignmentExecutionPolicy::PrepareOrLiteral,
            Default::default(),
        );
        let value = executor.compile(OriginalAssignment::default()).unwrap();
        assert_eq!(
            value.route(),
            OriginalAssignmentExecutionRoute::PreparedGlobal
        );
        assert!(value.literal_certificate().is_none() && value.fallback_reason().is_none());
        assert_eq!(value.probability(), &ExactScalar::one());
        assert_eq!(
            value.statistics(),
            OriginalAssignmentExecutionStatistics {
                preparation_attempts: 1,
                prepared_sources: 1,
                prepared_queries: 1,
                prepared_evaluations: 1,
                ..Default::default()
            }
        );
        let source = fallback_declared(6, strategy, false, Default::default());
        let mut executor = source.executor(
            OriginalAssignmentExecutionPolicy::PrepareOrLiteral,
            Default::default(),
        );
        for word in 0..4 {
            for pauli in [
                OriginalPauli::I,
                OriginalPauli::X,
                OriginalPauli::Y,
                OriginalPauli::Z,
            ] {
                for flip in [false, true] {
                    let input = OriginalAssignment {
                        true_values: vec![("m".into(), word & 1 != 0), ("n".into(), word & 2 != 0)],
                        faults: vec![OriginalFault {
                            after: "m".into(),
                            wire: 0,
                            pauli,
                        }],
                        report_flips: if flip { vec!["m".into()] } else { vec![] },
                    };
                    let expected = source.compile(input.clone()).unwrap();
                    let found = executor.compile(input).unwrap();
                    assert_eq!(
                        found.route(),
                        OriginalAssignmentExecutionRoute::PreparedGlobal
                    );
                    assert_eq!(found.probability(), expected.probability());
                    assert_eq!(found.decisions(), expected.branch().decisions());
                    assert_eq!(
                        found.statistics(),
                        OriginalAssignmentExecutionStatistics {
                            preparation_attempts: 1,
                            prepared_sources: 1,
                            prepared_queries: 1,
                            prepared_evaluations: 1,
                            control_evaluations: 1,
                            control_completions: 1,
                            ..Default::default()
                        }
                    );
                }
            }
        }
    }
    // Analytic single-wire masses distinguish true/reported coordinates, signed
    // readouts and faults. X commutes with MX; Y and Z interchange its masses.
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for inverted in [false, true] {
            let raw = instrument(1, None, None);
            let mut operations = raw.operations().to_vec();
            if let OriginalGate::Measure { inverted: flag, .. } =
                &mut operations.last_mut().unwrap().gate
            {
                *flag = inverted;
            } else {
                panic!("expected original MX fixture");
            }
            let source = OriginalAssignmentPlan::from_owned(
                raw.with_operations(operations, &ScalarArithmetic::default())
                    .unwrap(),
                strategy,
                ScalarArithmetic::default(),
            );
            let mut executor = source.executor(
                OriginalAssignmentExecutionPolicy::PrepareOrLiteral,
                Default::default(),
            );
            for high in [false, true] {
                for flip in [false, true] {
                    for pauli in [
                        OriginalPauli::I,
                        OriginalPauli::X,
                        OriginalPauli::Y,
                        OriginalPauli::Z,
                    ] {
                        let found = executor
                            .compile(OriginalAssignment {
                                true_values: vec![("m".into(), !high ^ inverted)],
                                faults: vec![fault(pauli)],
                                report_flips: if flip { vec!["m".into()] } else { vec![] },
                            })
                            .unwrap();
                        let swapped = matches!(pauli, OriginalPauli::Y | OriginalPauli::Z);
                        assert_eq!(found.probability(), &mass(high ^ swapped));
                        assert_eq!(
                            found.route(),
                            OriginalAssignmentExecutionRoute::PreparedGlobal
                        );
                        assert_eq!(found.statistics().literal_compilation_attempts, 0);
                    }
                }
            }
        }
    }
    // A separate measured wire drives T^(1+2*reported_m) on |+>. Its MX mass
    // changes with feedback, unlike the pinned single-wire 1/4 fixture. The
    // independent oracle is one fair MZ times (1 +/- 1/sqrt(2))/2.
    let a = ScalarArithmetic::default();
    let l = ControlRuleLimits::default();
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for inverted_m in [false, true] {
            for inverted_n in [false, true] {
                let exponent = ControlExpression::compose(
                    ControlOperator::Add,
                    vec![
                        ControlExpression::constant(1),
                        ControlExpression::compose(
                            ControlOperator::Multiply,
                            vec![
                                ControlExpression::constant(2),
                                ControlExpression::record("m".into(), l).unwrap(),
                            ],
                            l,
                        )
                        .unwrap(),
                    ],
                    l,
                )
                .unwrap();
                let declaration = OriginalDeclaredControl::new(
                    "c".into(),
                    vec!["m".into()],
                    "correlated-v1".into(),
                    ControlRule::new(
                        vec![
                            ControlGate::new(
                                ControlGateKind::T,
                                vec![1],
                                Some(exponent),
                                ControlExpression::constant(1),
                                l,
                            )
                            .unwrap(),
                        ],
                        l,
                    )
                    .unwrap(),
                )
                .unwrap();
                let initial = rmagna::stabilizer::AffineKet::new(
                    bits(2, 0),
                    vec![bits(2, 1), bits(2, 2)],
                    rmagna::quadratic::QuadraticForm::new(
                        0,
                        vec![0, 0],
                        vec![bits(2, 0), bits(2, 0)],
                    )
                    .unwrap(),
                    a.omega(0, -2).unwrap(),
                )
                .unwrap();
                let raw = OriginalInstrument::new_with_declared_controls(
                    vec![0, 1],
                    vec![0, 1],
                    initial,
                    vec![
                        OriginalOperation {
                            id: "m".into(),
                            gate: OriginalGate::Measure {
                                factors: vec![(0, OriginalPauli::Z)],
                                inverted: inverted_m,
                                record: "m".into(),
                            },
                        },
                        OriginalOperation {
                            id: "c".into(),
                            gate: OriginalGate::Control,
                        },
                        OriginalOperation {
                            id: "n".into(),
                            gate: OriginalGate::Measure {
                                factors: vec![(1, OriginalPauli::X)],
                                inverted: inverted_n,
                                record: "n".into(),
                            },
                        },
                    ],
                    vec![declaration],
                    Default::default(),
                    &a,
                )
                .unwrap();
                let source = OriginalAssignmentPlan::from_owned(raw, strategy, a);
                let mut executor = source.executor(
                    OriginalAssignmentExecutionPolicy::PrepareOrLiteral,
                    Default::default(),
                );
                for word in 0..4 {
                    for flip_m in [false, true] {
                        for flip_n in [false, true] {
                            let m = word & 1 != 0;
                            let n = word & 2 != 0;
                            let input = OriginalAssignment {
                                true_values: vec![("m".into(), m), ("n".into(), n)],
                                report_flips: [("m", flip_m), ("n", flip_n)]
                                    .into_iter()
                                    .filter_map(|(r, on)| on.then_some(r.into()))
                                    .collect(),
                                ..Default::default()
                            };
                            let expected = a
                                .multiply(&half(), &mass((n ^ inverted_n) == (m ^ flip_m)))
                                .unwrap();
                            assert_eq!(
                                source.compile(input.clone()).unwrap().probability(),
                                &expected
                            );
                            let found = executor.compile(input).unwrap();
                            assert_eq!(found.probability(), &expected);
                            assert_eq!(
                                found.route(),
                                OriginalAssignmentExecutionRoute::PreparedGlobal
                            );
                            assert_eq!(found.statistics().control_evaluations, 1);
                            assert_eq!(found.statistics().literal_compilation_attempts, 0);
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn original_opt_in_fallback_propagates_non_whitelisted_and_boundary_errors_once() {
    let source = fallback_positive(CompilerStrategy::ParityCopies, Default::default());
    let errors = [
        OriginalError::Scalar(rmagna::scalar::ScalarError::ExponentOverflow),
        OriginalError::Unitary(rmagna::unitary::UnitaryError::IncompatibleProgram),
        OriginalError::ResourceLimit {
            option: "max_geometry_words",
            limit: 0,
            required: 2096,
        },
        OriginalError::DeclaredControl {
            operation: "c".into(),
            error: ControlRuleError::UnsupportedFeedback(
                "controlled odd phase requires complex feedback",
            ),
        },
    ];
    for options in [
        OriginalAssignmentExecutionOptions::default(),
        fallback_options(),
    ] {
        for injected in &errors {
            let mut executor =
                source.executor(OriginalAssignmentExecutionPolicy::PrepareOrLiteral, options);
            let calls = AtomicUsize::new(0);
            let found = executor.compile_with_boundary(OriginalAssignment::default(), |_, _| {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(injected.clone())
            });
            let error = match found {
                Err(e) => e,
                Ok(_) => panic!("boundary failure was swallowed"),
            };
            assert_eq!(error, *injected);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(executor.statistics().literal_compilation_attempts, 0);
            assert_eq!(executor.statistics().preparation_attempts, 1);
            assert_eq!(executor.statistics().boundary_observations, 1);
            assert!(!executor.statistics().saturated);
        }
    }
    // Query/resource refusal is not a source-preparation geometry refusal.
    let mut options = OriginalAssignmentExecutionOptions::default();
    options.network.max_terms = 0;
    let mut executor =
        source.executor(OriginalAssignmentExecutionPolicy::PrepareOrLiteral, options);
    let called = AtomicUsize::new(0);
    assert!(
        executor
            .compile_with_boundary(OriginalAssignment::default(), |_, _| {
                called.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .is_err()
    );
    assert_eq!(called.load(Ordering::SeqCst), 0);
    assert_eq!(executor.statistics().prepared_sources, 1);
    assert_eq!(executor.statistics().literal_compilation_attempts, 0);
    assert!(executor.last_fallback_reason().is_none());
    // Initial-state admission fits 2112 words, but the measured two-record
    // coherent dilation needs more. Neither cap selection may bypass this
    // immutable source limit, even when preparation's separate cap is zero.
    let mut limits = OriginalLimits::default();
    limits.expansion.max_geometry_words = 2112;
    let source = fallback_declared(6, CompilerStrategy::ParityCopies, false, limits);
    for options in [
        OriginalAssignmentExecutionOptions::default(),
        fallback_options(),
    ] {
        let mut executor =
            source.executor(OriginalAssignmentExecutionPolicy::PrepareOrLiteral, options);
        assert!(matches!(executor.compile(OriginalAssignment {
            true_values: vec![("m".into(), true),("n".into(), false)], ..Default::default()
        }), Err(OriginalError::ResourceLimit {option:"max_geometry_words", required, ..}) if required>2112));
        assert_eq!(executor.statistics().literal_compilation_attempts, 0);
        assert!(executor.last_fallback_reason().is_none());
    }
    let source = fallback_declared(6, CompilerStrategy::ParityCopies, true, Default::default());
    let mut executor = source.executor(
        OriginalAssignmentExecutionPolicy::PrepareOrLiteral,
        Default::default(),
    );
    assert!(matches!(
        executor.compile(OriginalAssignment {
            true_values: vec![("m".into(), true), ("n".into(), false)],
            ..Default::default()
        }),
        Err(OriginalError::DeclaredControl {
            error: ControlRuleError::UnsupportedFeedback(_),
            ..
        })
    ));
    assert_eq!(executor.statistics().control_evaluations, 1);
    assert_eq!(executor.statistics().preparation_attempts, 1);
    assert_eq!(executor.statistics().literal_compilation_attempts, 0);
    assert!(executor.last_fallback_reason().is_none());
}
#[test]
fn original_opt_in_fallback_validates_before_preparation_and_never_retries_literal_failure() {
    let source = fallback_declared(1, CompilerStrategy::ParityCopies, false, Default::default());
    let base = OriginalAssignment {
        true_values: vec![("m".into(), true), ("n".into(), false)],
        ..Default::default()
    };
    let mut inputs = vec![OriginalAssignment::default()];
    let mut value = base.clone();
    value.true_values.push(("m".into(), false));
    inputs.push(value);
    let mut value = base.clone();
    value.report_flips.push("unknown".into());
    inputs.push(value);
    let mut value = base.clone();
    value.faults.push(OriginalFault {
        after: "unknown".into(),
        wire: 0,
        pauli: OriginalPauli::X,
    });
    inputs.push(value);
    let mut value = base.clone();
    value.faults.push(OriginalFault {
        after: "m".into(),
        wire: 99,
        pauli: OriginalPauli::X,
    });
    inputs.push(value);
    let mut executor = source.executor(
        OriginalAssignmentExecutionPolicy::PrepareOrLiteral,
        Default::default(),
    );
    for input in inputs {
        assert!(executor.compile(input).is_err());
        assert_eq!(
            executor.statistics(),
            OriginalAssignmentExecutionStatistics::default()
        );
    }
    let foreign = fallback_declared(1, CompilerStrategy::ParityCopies, false, Default::default())
        .compact(bits(2, 1), OriginalSharedFaults::empty())
        .unwrap();
    assert!(matches!(
        executor.compile(foreign),
        Err(OriginalError::Metadata {
            kind: "compact assignment belongs to a different IR snapshot",
            ..
        })
    ));
    assert_eq!(
        executor.statistics(),
        OriginalAssignmentExecutionStatistics::default()
    );
    let local = source
        .compact(bits(2, 1), OriginalSharedFaults::empty())
        .unwrap();
    assert_eq!(
        executor.compile(local).unwrap().route(),
        OriginalAssignmentExecutionRoute::PreparationFallback
    );
    assert_eq!(executor.statistics().literal_compilations, 1);
    // A literal probability failure after the one successful branch compilation
    // propagates, preserving the original pair cap rather than raising it.
    let mut limits = OriginalLimits::default();
    limits.completed.max_pairs = 0;
    let source = fallback_positive(CompilerStrategy::ParityCopies, limits);
    let expected = source.compile(OriginalAssignment::default()).err().unwrap();
    let mut executor = source.executor(
        OriginalAssignmentExecutionPolicy::PrepareOrLiteral,
        fallback_options(),
    );
    let error = match executor.compile(OriginalAssignment::default()) {
        Err(e) => e,
        Ok(_) => panic!("literal pair cap was bypassed"),
    };
    assert_eq!(error, expected);
    assert_eq!(executor.statistics().literal_compilation_attempts, 1);
    assert_eq!(executor.statistics().literal_compilations, 1);
    assert_eq!(executor.statistics().preparation_attempts, 1);
    assert_eq!(source.input_key_generations(), 0);
}

#[test]
fn original_opt_in_collection_preserves_chronological_errors_and_callback_failures() {
    let a = ScalarArithmetic::default();
    for dead_wire in [false, true] {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&calls);
        let mut ops = vec![];
        if dead_wire {
            ops.extend([
                OriginalOperation {
                    id: "drop".into(),
                    gate: OriginalGate::Discard(0),
                },
                OriginalOperation {
                    id: "invalid".into(),
                    gate: OriginalGate::Pauli {
                        wire: 0,
                        pauli: OriginalPauli::X,
                    },
                },
            ]);
        }
        ops.push(OriginalOperation {
            id: "c".into(),
            gate: OriginalGate::Control,
        });
        let source = OriginalAssignmentPlan::from_owned(
            OriginalInstrument::new(
                vec![0],
                if dead_wire { vec![] } else { vec![0] },
                CliffordCircuit::computational(bits(1, 0)).unwrap(),
                ops,
                vec![OriginalControl {
                    operation: "c".into(),
                    parents: vec![],
                    version: "fails-once".into(),
                    callback: Arc::new(move |_| {
                        observed.fetch_add(1, Ordering::SeqCst);
                        Err(OriginalCallbackError::Failed("sentinel".into()))
                    }),
                }],
                Default::default(),
                &a,
            )
            .unwrap(),
            CompilerStrategy::ParityCopies,
            a,
        );
        let expected = source.compile(OriginalAssignment::default()).err().unwrap();
        calls.store(0, Ordering::SeqCst);
        let mut executor = source.executor(
            OriginalAssignmentExecutionPolicy::PrepareOrLiteral,
            fallback_options(),
        );
        let error = match executor.compile(OriginalAssignment::default()) {
            Err(e) => e,
            Ok(_) => panic!("collection failure was swallowed"),
        };
        assert_eq!(error, expected);
        assert_eq!(calls.load(Ordering::SeqCst), usize::from(!dead_wire));
        assert_eq!(
            executor.statistics().control_evaluations,
            u64::from(!dead_wire)
        );
        assert_eq!(executor.statistics().control_completions, 0);
        assert_eq!(executor.statistics().preparation_attempts, 0);
        assert_eq!(executor.statistics().literal_compilation_attempts, 0);
        assert!(!executor.statistics().saturated);
    }
}
