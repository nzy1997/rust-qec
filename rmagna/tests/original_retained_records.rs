// SPDX-License-Identifier: MPL-2.0
// Original complete many-record scenario and deterministic input adapted from
// itpyi/magna 56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_global_sampling.py:
// test_many_record_phase_resources_cancel_without_decomposition.
// License: fixtures/original_many_record_input.LICENSE.
// Small literal matrix and Boolean boundary oracles are independent.
use rmagna::clifford::{CliffordCircuit, CliffordGate as C};
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    OriginalError, OriginalFault, OriginalGate as G, OriginalInstrument, OriginalLimits,
    OriginalNetworkLimits, OriginalNetworkOptions, OriginalNetworkResource, OriginalOperation,
    OriginalPauli as P, OriginalRecordParity, OriginalTraceAssignment, ReadoutBasis,
};
use rmagna::pauli::PauliWord;
use rmagna::phase::NativePhaseOptions;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::unitary::{CompilerStrategy, ExpansionLimits};
use sha2::{Digest, Sha256};
#[allow(dead_code)]
mod common;
use common::{bits, dense_gate};

fn options(rank: usize) -> OriginalNetworkOptions<'static> {
    let mut native = NativePhaseOptions::default();
    native.selection.max_quotient_rank = rank;
    OriginalNetworkOptions {
        resource: OriginalNetworkResource::Native(native),
        max_terms: 1 << rank,
        ..Default::default()
    }
}
enum Gate {
    H,
    T,
    Cx,
}
fn gate(id: String, gate: Gate, wires: Vec<i64>) -> OriginalOperation {
    let gate = match (gate, wires.as_slice()) {
        (Gate::H, [q]) => G::H(*q),
        (Gate::T, [q]) => G::Phase {
            wires: vec![*q],
            turns: 1,
        },
        (Gate::Cx, [c, t]) => G::Cx(*c, *t),
        _ => panic!("invalid literal test operation"),
    };
    OriginalOperation { id, gate }
}
fn measure(q: i64) -> OriginalOperation {
    OriginalOperation {
        id: format!("m{q}"),
        gate: G::Destructive {
            wire: q,
            basis: ReadoutBasis::Z,
            record: format!("r{q}"),
        },
    }
}
fn instrument(
    width: usize,
    outputs: Vec<i64>,
    operations: Vec<OriginalOperation>,
    limits: OriginalLimits,
    a: &ScalarArithmetic,
) -> OriginalInstrument {
    OriginalInstrument::new(
        (0..width as i64).collect(),
        outputs,
        CliffordCircuit::computational(BitVector::zeros(width).unwrap()).unwrap(),
        operations,
        vec![],
        limits,
        a,
    )
    .unwrap()
}
fn parity(mask: usize, inverted: bool) -> OriginalRecordParity {
    OriginalRecordParity {
        records: if mask == 0 {
            vec!["r0".into(), "r0".into()]
        } else {
            (0..3)
                .filter(|j| mask >> j & 1 != 0)
                .map(|j| format!("r{j}"))
                .collect()
        },
        inverted,
    }
}
fn hex_bit(s: &str, j: usize) -> bool {
    s.trim_start_matches("0x")
        .chars()
        .rev()
        .nth(j / 4)
        .is_some_and(|c| c.to_digit(16).unwrap() >> (j % 4) & 1 != 0)
}

#[test]
fn original_many_record_trace_cancels_resources_before_retaining_probability_labels() {
    let a = ScalarArithmetic::default();
    let data = include_bytes!("fixtures/original_many_record_input.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(data)),
        "0b165ce97bd2c21601090d61eaa5bd8940d77011dc39ca1ebf9dfb31f8ca8837"
    );
    let input: serde_json::Value = serde_json::from_slice(data).unwrap();
    assert_eq!(input["seed"], 260916);
    assert_eq!(input["width"], 129);
    assert_eq!(input["record_width"], 128);
    assert_eq!(input["normal_sqrt2_power"], -129);
    assert_eq!(input["native_kept_record_mask"], 6);
    assert_eq!(input["native_boundary_order"][0], 128);
    assert_eq!(
        input["physical_boundary_hex"].as_array().unwrap().len(),
        129
    );
    let mut boundary = (0..129)
        .map(|i| (0..129).map(|j| i == j).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut ops = vec![];
    for q in 0..129 {
        ops.push(gate(format!("h{q}"), Gate::H, vec![q]));
    }
    for q in 0..129 {
        ops.push(gate(format!("t{q}"), Gate::T, vec![q]));
    }
    let pairs = input["cnot_pairs"].as_array().unwrap();
    assert_eq!(pairs.len(), 512);
    for (i, pair) in pairs.iter().enumerate() {
        let control = pair[0].as_u64().unwrap() as usize;
        let target = pair[1].as_u64().unwrap() as usize;
        assert!(control < 128 && target < 128 && control != target);
        let source = boundary[control].clone();
        for (bit, source_bit) in boundary[target].iter_mut().zip(source) {
            *bit ^= source_bit;
        }
        ops.push(gate(
            format!("cx{i}"),
            Gate::Cx,
            vec![control as i64, target as i64],
        ));
    }
    for (row, expected) in boundary
        .iter()
        .zip(input["physical_boundary_hex"].as_array().unwrap())
    {
        let s = expected.as_str().unwrap();
        for (j, &bit) in row.iter().enumerate() {
            assert_eq!(bit, hex_bit(s, j));
        }
    }
    ops.extend((0..128).map(measure));
    let ir = instrument(
        129,
        vec![128],
        ops,
        OriginalLimits {
            expansion: ExpansionLimits {
                max_side_terms: 1,
                ..Default::default()
            },
            ..Default::default()
        },
        &a,
    );
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let prepared = ir.compile_trace(compiler, &a).unwrap();
        let network = prepared
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        let row = OriginalRecordParity {
            records: vec!["r0".into(), "r1".into()],
            inverted: false,
        };
        let query = network
            .prepare_record_probabilities(&[row], &options(0), &a)
            .unwrap();
        assert_eq!(query.phase_resource().quotient_rank(), 0);
        assert_eq!(query.phase_resource().num_terms(), 1);
        let probabilities = query.evaluate(&a).unwrap();
        let [term] = probabilities.terms() else {
            panic!("one actual probability term required")
        };
        assert_eq!(term.width(), 1);
        assert_eq!(term.columns().len(), 1);
        assert!(term.phase().linear().iter().all(|&c| c == 0));
        assert!(term.phase().upper().iter().all(BitVector::is_zero));
        let uniform = probabilities.uniform_affine(&a).unwrap().unwrap();
        assert_eq!(uniform.support().dimension(), 1);
        assert_eq!(uniform.total_mass(), &ExactScalar::one());
        let half = a
            .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
            .unwrap();
        assert_eq!(uniform.mass_per_label(), &half);
        for label in 0..2 {
            assert_eq!(
                term.prepare()
                    .unwrap()
                    .amplitude(&bits(1, label), &a)
                    .unwrap(),
                half
            );
        }
        for y in [false, true] {
            let word = PauliWord::new(
                BitVector::from_indices(prepared.network_width(), [0]).unwrap(),
                BitVector::from_indices(prepared.network_width(), if y { vec![0] } else { vec![] })
                    .unwrap(),
                if y { 2 } else { 0 },
            )
            .unwrap();
            let moment = prepared
                .prepare_network_moment(&word, &[], &options(0), &a)
                .unwrap();
            let value = moment.evaluate(&network, &bits(0, 0), &a).unwrap();
            assert_eq!(
                value.value,
                a.scalar([0.into(), 1.into(), 0.into(), (-1).into()], -1)
                    .unwrap()
            );
            assert_eq!((value.quotient_rank, value.terms), (0, 1));
        }
        assert!(matches!(
            network.prepare_record_probabilities(
                &[],
                &OriginalNetworkOptions {
                    limits: OriginalNetworkLimits {
                        max_joint_paths: 128,
                        ..Default::default()
                    },
                    ..options(0)
                },
                &a
            ),
            Err(OriginalError::ResourceLimit {
                option: "max_network_joint_paths",
                ..
            })
        ));
        let mut wide_rows = (0..128)
            .map(|j| OriginalRecordParity {
                records: vec![format!("r{j}")],
                inverted: false,
            })
            .collect::<Vec<_>>();
        wide_rows.push(OriginalRecordParity {
            records: vec!["r127".into()],
            inverted: false,
        });
        let wide_query = network
            .prepare_record_probabilities(&wide_rows, &options(0), &a)
            .unwrap();
        let wide_tensor = wide_query.evaluate(&a).unwrap();
        let wide = wide_tensor.uniform_affine(&a).unwrap().unwrap();
        assert_eq!(
            (wide.support().width(), wide.support().dimension()),
            (129, 128)
        );
        assert_eq!(wide.total_mass(), &ExactScalar::one());
        assert_eq!(
            wide.mass_per_label(),
            &a.scalar([1.into(), 0.into(), 0.into(), 0.into()], -128)
                .unwrap()
        );
        let mut calls = 0;
        let sample = wide
            .sample_bits(&mut || {
                calls += 1;
                calls % 3 == 0
            })
            .unwrap();
        assert_eq!(calls, 128);
        assert_eq!(sample.get(127).unwrap(), sample.get(128).unwrap());
    }
}

#[test]
fn retained_record_masses_match_literal_branches_with_faults_and_report_relabeling() {
    let a = ScalarArithmetic::default();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for fault in [false, true] {
            for nonuniform in [false, true] {
                let mut ops = vec![
                    gate("h0".into(), Gate::H, vec![0]),
                    gate("h1".into(), Gate::H, vec![1]),
                    gate("t0".into(), Gate::T, vec![0]),
                ];
                let mut dense = vec![ExactScalar::zero(); 8];
                dense[0] = ExactScalar::one();
                dense_gate(C::H(0), &mut dense, &a);
                dense_gate(C::H(1), &mut dense, &a);
                for (j, v) in dense.iter_mut().enumerate() {
                    if j & 1 != 0 {
                        *v = a.multiply(v, &a.omega(1, 0).unwrap()).unwrap();
                    }
                }
                if fault {
                    dense_gate(C::Z(0), &mut dense, &a);
                }
                if nonuniform {
                    ops.push(gate("last_h".into(), Gate::H, vec![0]));
                    dense_gate(C::H(0), &mut dense, &a);
                } else {
                    ops.push(gate("cx".into(), Gate::Cx, vec![0, 1]));
                    dense_gate(
                        C::Cx {
                            control: 0,
                            target: 1,
                        },
                        &mut dense,
                        &a,
                    );
                }
                ops.extend((0..3).map(measure));
                let ir = instrument(3, vec![], ops, OriginalLimits::default(), &a);
                let prepared = ir.compile_trace(compiler, &a).unwrap();
                let assignment = OriginalTraceAssignment {
                    faults: if fault {
                        vec![OriginalFault {
                            after: "t0".into(),
                            wire: 0,
                            pauli: P::Z,
                        }]
                    } else {
                        vec![]
                    },
                    report_flips: vec!["r1".into(), "r2".into()],
                };
                let network = prepared.with_assignment(&assignment, &a).unwrap();
                let before = prepared.compilation().clone();
                const INVERSION: usize = 0b101010;
                let rows = [2, 1, 2, 0, 4, 3].map(|mask| parity(mask, false));
                let rows = rows
                    .into_iter()
                    .enumerate()
                    .map(|(j, mut row)| {
                        row.inverted = INVERSION >> j & 1 != 0;
                        row
                    })
                    .collect::<Vec<_>>();
                let query = network
                    .prepare_record_probabilities(&rows, &options(4), &a)
                    .unwrap();
                let probabilities = query.evaluate(&a).unwrap();
                let mut expected = vec![ExactScalar::zero(); 64];
                for (j, value) in dense.iter().enumerate() {
                    let reported = j ^ 6;
                    let mut label = 0;
                    for (i, row) in [2usize, 1, 2, 0, 4, 3].into_iter().enumerate() {
                        label |= (((reported & row).count_ones() as usize & 1)
                            ^ ((INVERSION >> i) & 1))
                            << i;
                    }
                    expected[label] = a
                        .add(&expected[label], &a.norm_squared(value).unwrap())
                        .unwrap();
                }
                for (j, mass) in expected.iter().enumerate() {
                    let actual =
                        probabilities
                            .terms()
                            .iter()
                            .fold(ExactScalar::zero(), |v, term| {
                                a.add(
                                    &v,
                                    &term.prepare().unwrap().amplitude(&bits(6, j), &a).unwrap(),
                                )
                                .unwrap()
                            });
                    assert_eq!(
                        &actual, mass,
                        "{compiler:?} fault={fault} nonuniform={nonuniform} label={j}"
                    );
                }
                let uniform = probabilities.uniform_affine(&a).unwrap();
                if nonuniform {
                    assert!(uniform.is_none());
                    let root = a
                        .scalar([0.into(), 1.into(), 0.into(), (-1).into()], -2)
                        .unwrap();
                    let half = a
                        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
                        .unwrap();
                    let expected_zero = if fault {
                        a.subtract(&half, &root)
                    } else {
                        a.add(&half, &root)
                    }
                    .unwrap();
                    let zero_mass = expected
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| (j ^ INVERSION) & 2 == 0)
                        .fold(ExactScalar::zero(), |v, (_, mass)| a.add(&v, mass).unwrap());
                    assert_eq!(zero_mass, expected_zero);
                } else {
                    let uniform = uniform.unwrap();
                    assert_eq!(uniform.total_mass(), &ExactScalar::one());
                    assert_eq!(uniform.support().dimension(), 2);
                    let mut seen = std::collections::BTreeSet::new();
                    for seed in 0..4 {
                        let mut calls = 0;
                        let draw = uniform
                            .sample_bits(&mut || {
                                let bit = seed >> calls & 1 != 0;
                                calls += 1;
                                bit
                            })
                            .unwrap();
                        assert_eq!(calls, 2);
                        let index = draw.ones().fold(0, |v, j| v | (1 << j));
                        assert_eq!(&expected[index], uniform.mass_per_label());
                        seen.insert(index);
                    }
                    assert_eq!(seen.len(), 4);
                }
                let empty = network
                    .prepare_record_probabilities(&[], &options(4), &a)
                    .unwrap()
                    .evaluate(&a)
                    .unwrap();
                let scalar = empty.uniform_affine(&a).unwrap().unwrap();
                assert_eq!(scalar.total_mass(), &ExactScalar::one());
                assert_eq!(
                    scalar
                        .sample_bits(&mut || panic!("empty support consumed entropy"))
                        .unwrap(),
                    bits(0, 0)
                );
                assert_eq!(before, prepared.compilation().clone());
            }
        }
    }
}

#[test]
fn retained_label_limits_are_distinct_from_event_limits() {
    let a = ScalarArithmetic::default();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let mut limits = OriginalLimits::default();
        limits.completed.max_event_rows = 0;
        limits.completed.max_record_rows = 1;
        let ir = instrument(1, vec![], vec![measure(0)], limits, &a);
        let source = ir.compile_trace(compiler, &a).unwrap();
        let view = source
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        let mut opts = options(0);
        opts.limits.max_event_rows = 0;
        let keep = parity(1, false);
        let query = view
            .prepare_record_probabilities(std::slice::from_ref(&keep), &opts, &a)
            .unwrap();
        let probabilities = query.evaluate(&a).unwrap();
        let support = probabilities.uniform_affine(&a).unwrap().unwrap();
        assert_eq!(support.support().dimension(), 0);
        assert_eq!(support.mass_per_label(), &ExactScalar::one());
        assert_eq!(
            support
                .sample_bits(&mut || panic!("point support consumed entropy"))
                .unwrap(),
            bits(1, 0)
        );
        assert!(matches!(
            view.prepare_record_probabilities(&[keep.clone(), keep.clone()], &opts, &a),
            Err(OriginalError::ResourceLimit {
                option: "max_record_rows",
                limit: 1,
                required: 2
            })
        ));
        assert!(matches!(
            source.prepare_network_probability(std::slice::from_ref(&keep), &opts, &a),
            Err(OriginalError::ResourceLimit {
                option: "max_event_rows",
                limit: 0,
                required: 1
            })
        ));
        assert!(matches!(
            view.prepare_record_probabilities(
                &[OriginalRecordParity {
                    records: vec!["missing".into()],
                    inverted: false
                }],
                &opts,
                &a
            ),
            Err(OriginalError::Metadata { .. })
        ));
        let mut bad = opts;
        bad.limits.max_geometry_words = query.geometry_words() as usize - 1;
        assert!(matches!(
            view.prepare_record_probabilities(&[keep], &bad, &a),
            Err(OriginalError::ResourceLimit {
                option: "max_network_geometry_words",
                ..
            })
        ));
        assert_eq!(
            query
                .evaluate(&a)
                .unwrap()
                .uniform_affine(&a)
                .unwrap()
                .unwrap()
                .total_mass(),
            &ExactScalar::one()
        );
    }
}

#[test]
fn prepared_record_probabilities_freeze_the_assignment_and_survive_errors() {
    let a = ScalarArithmetic::default();
    let ir = instrument(
        1,
        vec![],
        vec![
            OriginalOperation {
                id: "before".into(),
                gate: G::Identity(vec![0]),
            },
            measure(0),
        ],
        OriginalLimits::default(),
        &a,
    );
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let source = ir.compile_trace(compiler, &a).unwrap();
        let plain = source
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        let flipped = source
            .with_assignment(
                &OriginalTraceAssignment {
                    faults: vec![OriginalFault {
                        after: "before".into(),
                        wire: 0,
                        pauli: P::X,
                    }],
                    report_flips: vec![],
                },
                &a,
            )
            .unwrap();
        let cancelled = source
            .with_assignment(
                &OriginalTraceAssignment {
                    faults: vec![OriginalFault {
                        after: "before".into(),
                        wire: 0,
                        pauli: P::X,
                    }],
                    report_flips: vec!["r0".into()],
                },
                &a,
            )
            .unwrap();
        let keep = [parity(1, false)];
        let query = plain
            .prepare_record_probabilities(&keep, &options(0), &a)
            .unwrap();
        for (view, expected) in [(&flipped, true), (&cancelled, false), (&plain, false)] {
            let answer = view
                .prepare_record_probabilities(&keep, &options(0), &a)
                .unwrap()
                .evaluate(&a)
                .unwrap();
            let support = answer.uniform_affine(&a).unwrap().unwrap();
            assert_eq!(support.support().dimension(), 0);
            assert_eq!(
                support
                    .sample_bits(&mut || panic!("deterministic view consumed entropy"))
                    .unwrap(),
                bits(1, usize::from(expected))
            );
            let scalar = source
                .prepare_network_probability(&keep, &options(0), &a)
                .unwrap();
            assert_eq!(
                scalar
                    .evaluate(view, &bits(1, usize::from(expected)), &a)
                    .unwrap()
                    .value,
                ExactScalar::one()
            );
            assert_eq!(
                scalar
                    .evaluate(view, &bits(1, usize::from(!expected)), &a)
                    .unwrap()
                    .value,
                ExactScalar::zero()
            );
            assert_eq!(
                query
                    .evaluate(&a)
                    .unwrap()
                    .uniform_affine(&a)
                    .unwrap()
                    .unwrap()
                    .support()
                    .offset(),
                &bits(1, 0)
            );
        }
        let mut impossible = options(0);
        impossible.max_record_occurrences = 0;
        assert!(matches!(
            plain.prepare_record_probabilities(&keep, &impossible, &a),
            Err(OriginalError::ResourceLimit {
                option: "max_record_occurrences",
                ..
            })
        ));
        let answer = query.evaluate(&a).unwrap();
        let support = answer.uniform_affine(&a).unwrap().unwrap();
        assert_eq!(support.total_mass(), &ExactScalar::one());
        assert_eq!(support.support().offset(), &bits(1, 0));
    }
}

#[test]
fn retained_probability_traces_quantum_outputs_and_discarded_environment() {
    let a = ScalarArithmetic::default();
    let ops = vec![
        gate("h0".into(), Gate::H, vec![0]),
        gate("t0".into(), Gate::T, vec![0]),
        gate("cx".into(), Gate::Cx, vec![0, 1]),
        gate("h2".into(), Gate::H, vec![2]),
        gate("t2".into(), Gate::T, vec![2]),
        OriginalOperation {
            id: "environment".into(),
            gate: G::Discard(2),
        },
        measure(0),
    ];
    let ir = instrument(
        3,
        vec![1],
        ops,
        OriginalLimits {
            expansion: ExpansionLimits {
                max_side_terms: 1,
                ..Default::default()
            },
            ..Default::default()
        },
        &a,
    );
    let half = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
        .unwrap();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let source = ir.compile_trace(compiler, &a).unwrap();
        assert_eq!(source.outputs(), &[1]);
        assert!(!source.environment().is_empty());
        let view = source
            .with_assignment(&OriginalTraceAssignment::default(), &a)
            .unwrap();
        let keep = [parity(1, false)];
        let query = view
            .prepare_record_probabilities(&keep, &options(0), &a)
            .unwrap();
        assert_eq!(
            (
                query.phase_resource().quotient_rank(),
                query.phase_resource().num_terms()
            ),
            (0, 1)
        );
        let probability = query.evaluate(&a).unwrap();
        let certificate = probability.uniform_affine(&a).unwrap().unwrap();
        assert_eq!(certificate.total_mass(), &ExactScalar::one());
        assert_eq!(certificate.mass_per_label(), &half);
        assert_eq!(certificate.support().dimension(), 1);
        for label in 0..2 {
            let mass = probability
                .terms()
                .iter()
                .fold(ExactScalar::zero(), |v, term| {
                    a.add(
                        &v,
                        &term
                            .prepare()
                            .unwrap()
                            .amplitude(&bits(1, label), &a)
                            .unwrap(),
                    )
                    .unwrap()
                });
            assert_eq!(mass, half);
        }
        assert_eq!(
            view.prepare_record_probabilities(&[], &options(0), &a)
                .unwrap()
                .evaluate(&a)
                .unwrap()
                .uniform_affine(&a)
                .unwrap()
                .unwrap()
                .total_mass(),
            &ExactScalar::one()
        );
    }
}
