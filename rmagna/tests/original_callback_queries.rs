// SPDX-License-Identifier: MPL-2.0
// Complete finite callback scenario from itpyi/magna
// 56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_kernel_contract.py::KernelContractTests::test_legacy_flags_cannot_select_an_old_kernel_and_general_callbacks_work.
// Literal two-dimensional matrices supply all expected amplitudes. Rust owns
// the collected result; private Python reuse hints and caches have no Rust ABI.
use rmagna::clifford::{CliffordCircuit, CliffordGate};
use rmagna::gf2::BitVector;
use rmagna::instrument::*;
use rmagna::phase::NativePhaseOptions;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::unitary::CompilerStrategy;
use std::sync::{Arc, Mutex};
fn bits(n: usize, value: usize) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|j| value >> j & 1 != 0)).unwrap()
}
fn instrument(
    calls: Arc<Mutex<Vec<bool>>>,
    limits: OriginalLimits,
    a: &ScalarArithmetic,
) -> Result<OriginalInstrument, OriginalError> {
    let initial = CliffordCircuit::new(1, [CliffordGate::H(0)])
        .unwrap()
        .evolve(&CliffordCircuit::computational(bits(1, 0)).unwrap(), a)
        .unwrap()
        .unwrap();
    let measurement = |name: &str, pauli| OriginalOperation {
        id: name.into(),
        gate: OriginalGate::Measure {
            factors: vec![(0, pauli)],
            inverted: false,
            record: name.into(),
        },
    };
    OriginalInstrument::new(
        vec![0],
        vec![0],
        initial,
        vec![
            measurement("a", OriginalPauli::Z),
            OriginalOperation {
                id: "c".into(),
                gate: OriginalGate::Control,
            },
            measurement("b", OriginalPauli::X),
        ],
        vec![OriginalControl {
            operation: "c".into(),
            parents: vec!["a".into()],
            version: "literal-v1".into(),
            callback: Arc::new(move |reports| {
                let value = reports.get("a")?;
                assert_eq!(reports.entries(), &[("a".into(), value)]);
                calls.lock().unwrap().push(value);
                Ok(vec![
                    OriginalGate::H(0),
                    OriginalGate::Phase {
                        wires: vec![0],
                        turns: 1 + i64::from(value),
                    },
                ])
            }),
        }],
        limits,
        a,
    )
}
fn assignment(a: bool, b: bool) -> OriginalAssignment {
    OriginalAssignment {
        true_values: vec![("a".into(), a), ("b".into(), b)],
        report_flips: vec!["a".into()],
        faults: vec![OriginalFault {
            after: "c".into(),
            wire: 0,
            pauli: OriginalPauli::Y,
        }],
    }
}
fn literal(a: bool, b: bool, arithmetic: &ScalarArithmetic) -> [ExactScalar; 2] {
    // P_X(b) Y T^(1+reported_a) H P_Z(a)|+>, reported_a=1-a.
    // Both coefficients are literal matrix products, including absolute phase.
    let first = arithmetic.omega(3 * i64::from(a), -4).unwrap();
    let i = arithmetic.omega(2, -4).unwrap();
    let signed = |x: &ExactScalar| if b { x.negated() } else { x.clone() };
    [
        arithmetic.add(&first, &signed(&i)).unwrap(),
        arithmetic.add(&signed(&first), &i).unwrap(),
    ]
}
fn probability(v: &[ExactScalar; 2], a: &ScalarArithmetic) -> ExactScalar {
    a.add(
        &a.norm_squared(&v[0]).unwrap(),
        &a.norm_squared(&v[1]).unwrap(),
    )
    .unwrap()
}
#[test]
fn original_general_quantum_callbacks_use_owned_single_sided_probabilities() {
    let a = ScalarArithmetic::default();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for native in [false, true] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let mut limits = OriginalLimits::default();
            limits.expansion.max_path_bits = 0;
            limits.expansion.max_side_terms = 0;
            limits.expansion.max_pairs = 0;
            limits.completed.max_pairs = 0;
            let ir = instrument(calls.clone(), limits, &a).unwrap();
            let mut retained = Vec::new();
            let mut total = ExactScalar::zero();
            let options = OriginalNetworkOptions {
                resource: if native {
                    OriginalNetworkResource::Native(NativePhaseOptions::default())
                } else {
                    OriginalNetworkResource::default()
                },
                ..Default::default()
            };
            for first in [false, true] {
                for second in [false, true] {
                    let branch = ir
                        .compile(&assignment(first, second), compiler, &a)
                        .unwrap();
                    let expected = literal(first, second, &a);
                    let p = probability(&expected, &a);
                    let origins = branch.origins().to_vec();
                    let decisions = branch.decisions().to_vec();
                    let records = branch.records().to_vec();
                    let diagnostics = branch.compilation().clone();
                    assert!(
                        branch.expansion(&a).is_err(),
                        "side-only budget must refuse expansion"
                    );
                    let query = branch.prepare_scalar_probability(&options, &a).unwrap();
                    assert_eq!(
                        query.physical_outcomes(),
                        &bits(2, usize::from(first) + 2 * usize::from(second))
                    );
                    assert_eq!(query.records(), branch.records());
                    assert_eq!(query.compilation(), &diagnostics);
                    assert!(query.resource().num_terms() > 0);
                    for _ in 0..3 {
                        assert_eq!(query.evaluate(&a).unwrap().value, p);
                    }
                    assert_eq!(branch.origins(), origins);
                    assert_eq!(branch.decisions(), decisions);
                    assert_eq!(branch.records(), records);
                    assert_eq!(branch.compilation(), &diagnostics);
                    assert_eq!(branch.decisions()[0].inputs.get("a"), Ok(!first));
                    assert_eq!(branch.decisions()[0].emitted_operations, 2);
                    assert_eq!(calls.lock().unwrap().len(), retained.len() + 1);
                    total = a.add(&total, &p).unwrap();
                    drop(branch);
                    retained.push((query, p));
                }
            }
            assert_eq!(total, ExactScalar::one());
            assert_eq!(*calls.lock().unwrap(), [true, true, false, false]);
            // Separate explicit compilation recollects each callback once. Its
            // ordinary projected amplitudes preserve the same global/Y phases.
            let ordinary = instrument(calls.clone(), OriginalLimits::default(), &a).unwrap();
            for first in [false, true] {
                for second in [false, true] {
                    let branch = ordinary
                        .compile(&assignment(first, second), compiler, &a)
                        .unwrap();
                    assert_eq!(branch.width(), 1);
                    for (label, want) in literal(first, second, &a).iter().enumerate() {
                        assert_eq!(branch.amplitude(&bits(1, label), 64, &a).unwrap(), *want);
                    }
                }
            }
            assert_eq!(
                *calls.lock().unwrap(),
                [true, true, false, false, true, true, false, false]
            );
            drop(ir);
            drop(ordinary);
            for (query, expected) in retained {
                assert_eq!(query.evaluate(&a).unwrap().value, expected);
            }
            assert_eq!(calls.lock().unwrap().len(), 8);
        }
    }
}
#[test]
fn original_owned_probability_admits_actual_geometry_and_keeps_earlier_queries_usable() {
    let a = ScalarArithmetic::default();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let ir = instrument(calls.clone(), OriginalLimits::default(), &a).unwrap();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let branch = ir.compile(&assignment(true, false), compiler, &a).unwrap();
        let expected = probability(&literal(true, false, &a), &a);
        let original = branch
            .prepare_scalar_probability(&Default::default(), &a)
            .unwrap();
        for (label, options) in [
            (
                "max_event_rows",
                OriginalNetworkOptions {
                    limits: OriginalNetworkLimits {
                        max_event_rows: 0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ),
            (
                "max_record_occurrences",
                OriginalNetworkOptions {
                    max_record_occurrences: 0,
                    ..Default::default()
                },
            ),
            (
                "max_network_joint_paths",
                OriginalNetworkOptions {
                    limits: OriginalNetworkLimits {
                        max_joint_paths: 0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ),
            (
                "max_network_geometry_words",
                OriginalNetworkOptions {
                    limits: OriginalNetworkLimits {
                        max_geometry_words: 0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ),
        ] {
            assert!(
                matches!(branch.prepare_scalar_probability(&options,&a),Err(OriginalError::ResourceLimit{option,limit:0,required}) if option==label && required>0)
            );
            assert_eq!(original.evaluate(&a).unwrap().value, expected);
        }
        let restricted = OriginalNetworkOptions {
            resource: OriginalNetworkResource::Native(NativePhaseOptions {
                selection: rmagna::catalogue::PhaseSelectionOptions {
                    max_quotient_rank: 0,
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };
        let failure = match branch.prepare_scalar_probability(&restricted, &a) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("Native resource refusal bypassed"),
        };
        assert!(failure.contains("max_quotient_rank"));
        assert!(
            branch
                .prepare_scalar_probability(
                    &OriginalNetworkOptions {
                        max_terms: 0,
                        ..Default::default()
                    },
                    &a
                )
                .is_err()
        );
        assert_eq!(original.evaluate(&a).unwrap().value, expected);
        assert_eq!(
            branch
                .prepare_scalar_probability(&Default::default(), &a)
                .unwrap()
                .evaluate(&a)
                .unwrap()
                .value,
            expected
        );
    }
    assert_eq!(calls.lock().unwrap().len(), 2);
    // Constructor/lowering limits are enforced before an owned query exists.
    // They retain their existing meanings; zero initial geometry is not a
    // purported side-only budget in the current Rust architecture.
    let mut invalid = OriginalLimits::default();
    invalid.expansion.max_geometry_words = 0;
    assert!(instrument(calls.clone(), invalid, &a).is_err());
    let narrow = OriginalLimits {
        max_physical_slots: 1,
        ..Default::default()
    };
    let source = instrument(calls.clone(), narrow, &a).unwrap();
    assert!(
        source
            .compile(&assignment(true, false), CompilerStrategy::ParityCopies, &a)
            .is_err()
    );
    let paths = OriginalLimits {
        max_path_bits: 1,
        ..Default::default()
    };
    let source = instrument(calls.clone(), paths, &a).unwrap();
    assert!(
        source
            .compile(&assignment(true, false), CompilerStrategy::ParityCopies, &a)
            .is_err()
    );
}
