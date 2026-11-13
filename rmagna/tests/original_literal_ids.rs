// SPDX-License-Identifier: MPL-2.0
// Complete KernelContractTests.test_literal_internal_ids_do_not_collide_with_original_ids,
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_kernel_contract.py.
use rmagna::clifford::CliffordCircuit;
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    OriginalAssignment, OriginalBranchCertificateOptions, OriginalControl, OriginalFault,
    OriginalGate as G, OriginalInstrument, OriginalLimits, OriginalNetworkResource,
    OriginalOperation, OriginalOrigin, OriginalPauli,
};
use rmagna::phase::NativePhaseOptions;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::CompilerStrategy;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
fn bits(word: usize) -> BitVector {
    BitVector::from_indices(1, (word != 0).then_some(0)).unwrap()
}
fn op(id: &str, gate: G) -> OriginalOperation {
    OriginalOperation {
        id: id.into(),
        gate,
    }
}
fn sum(states: &[AffineKet], word: usize, a: &ScalarArithmetic) -> ExactScalar {
    states.iter().fold(ExactScalar::zero(), |v, k| {
        a.add(&v, &k.prepare().unwrap().amplitude(&bits(word), a).unwrap())
            .unwrap()
    })
}
#[test]
fn original_literal_ids_preserve_distinct_emission_fault_and_source_origins() {
    let a = ScalarArithmetic::default();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for fault_case in [false, true] {
            let calls = Arc::new(AtomicUsize::new(0));
            let counter = calls.clone();
            let (operations, controls, assignment, origins) = if fault_case {
                (
                    vec![
                        op("g", G::H(0)),
                        op(
                            "g/fault/0",
                            G::Phase {
                                wires: vec![0],
                                turns: 1,
                            },
                        ),
                    ],
                    vec![],
                    OriginalAssignment {
                        faults: vec![OriginalFault {
                            after: "g".into(),
                            wire: 0,
                            pauli: OriginalPauli::Y,
                        }],
                        ..Default::default()
                    },
                    vec![
                        OriginalOrigin::Operation("g".into()),
                        OriginalOrigin::Fault {
                            after: "g".into(),
                            ordinal: 0,
                        },
                        OriginalOrigin::Operation("g/fault/0".into()),
                    ],
                )
            } else {
                (
                    vec![
                        op("c", G::Control),
                        op(
                            "c/0",
                            G::Phase {
                                wires: vec![0],
                                turns: 1,
                            },
                        ),
                    ],
                    vec![OriginalControl {
                        operation: "c".into(),
                        parents: vec![],
                        version: "literal-id-v1".into(),
                        callback: Arc::new(move |inputs| {
                            assert!(inputs.entries().is_empty());
                            counter.fetch_add(1, Ordering::SeqCst);
                            Ok(vec![G::H(0)])
                        }),
                    }],
                    OriginalAssignment::default(),
                    vec![
                        OriginalOrigin::Operation("c".into()),
                        OriginalOrigin::ControlEmission {
                            control: "c".into(),
                            ordinal: 0,
                        },
                        OriginalOrigin::Operation("c/0".into()),
                    ],
                )
            };
            let ir = OriginalInstrument::new(
                vec![0],
                vec![0],
                CliffordCircuit::computational(bits(0)).unwrap(),
                operations,
                controls,
                OriginalLimits::default(),
                &a,
            )
            .unwrap();
            let branch = ir.compile(&assignment, compiler, &a).unwrap();
            drop(ir);
            assert_eq!(branch.origins(), origins);
            assert_eq!(branch.outputs(), &[0]);
            assert!(branch.environment().is_empty());
            assert!(branch.records().is_empty());
            assert_eq!(branch.width(), 1);
            assert_eq!(branch.decisions().len(), usize::from(!fault_case));
            if !fault_case {
                let d = &branch.decisions()[0];
                assert_eq!(d.operation, "c");
                assert_eq!(d.version, "literal-id-v1");
                assert!(d.inputs.entries().is_empty());
                assert_eq!(d.emitted_operations, 1);
            }
            let expected = if fault_case {
                [a.omega(-2, -1).unwrap(), a.omega(3, -1).unwrap()]
            } else {
                [a.omega(0, -1).unwrap(), a.omega(1, -1).unwrap()]
            };
            let diagnostics = branch.compilation().clone();
            // Three explicit immutable preparations replace the upstream private
            // cache flags; their layouts and performance are not ABI claims.
            for (resource, catalogue) in [
                (OriginalNetworkResource::default(), None),
                (
                    OriginalNetworkResource::Native(NativePhaseOptions::default()),
                    None,
                ),
                (
                    OriginalNetworkResource::default(),
                    Some(rmagna::catalogue::builtin_phase_catalogue().unwrap()),
                ),
            ] {
                let options = OriginalBranchCertificateOptions {
                    resource,
                    catalogue,
                    ..Default::default()
                };
                for _ in 0..3 {
                    let ordinary = branch.expansion(&a).unwrap();
                    let cert = branch.compile_after_absorption(&options, &a).unwrap();
                    for (word, value) in expected.iter().enumerate() {
                        assert_eq!(sum(&ordinary, word, &a), *value);
                        assert_eq!(branch.amplitude(&bits(word), 1024, &a).unwrap(), *value);
                        assert_eq!(sum(cert.states(), word, &a), *value);
                    }
                    assert_eq!(branch.probability(&a).unwrap(), ExactScalar::one());
                    assert_eq!(cert.probability(), &ExactScalar::one());
                    assert_eq!(branch.origins(), origins);
                    assert_eq!(branch.compilation(), &diagnostics);
                    assert_eq!(calls.load(Ordering::SeqCst), usize::from(!fault_case));
                }
            }
        }
    }
}
