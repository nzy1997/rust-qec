// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//! Complete original SingleTraceTests.test_entangled_trace_and_postselection_are_distinct
//! from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_single_trace.py,
//! plus independent analytical sectors and immutable public-view regressions.

use rmagna::circuit::{BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::clifford::{CliffordCircuit, CliffordGate};
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    CompileOptions, CompiledInstrument, FaultSelection, InstrumentError,
    InstrumentQueryLayout as Q, QueryPreparation, QueryPreparationOutcome as O, compile_affine,
    compile_affine_stabilizer, compile_unitary,
};
use rmagna::pauli::PauliWord;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::unitary::{CompilerStrategy, DensityBoundary, DensityLayout};

const ORIGINAL_INPUT_WIDTH: usize = 2;
const ORIGINAL_INITIAL_X_MASKS: [usize; 1] = [1];
const ORIGINAL_SOURCE: &str = "T 0\nCX 0 1\nMX 1";
const ORIGINAL_OUTPUTS: [usize; 1] = [0];
const ORIGINAL_PAULIS: [(usize, usize); 3] = [(1, 0), (1, 1), (0, 1)];
const ORIGINAL_OUTCOMES: [bool; 2] = [false, true];

fn bits(width: usize, mask: usize) -> BitVector {
    BitVector::from_indices(width, (0..width).filter(|j| mask >> j & 1 != 0)).unwrap()
}
fn record(index: usize) -> BitExpr {
    BitExpr::atom(BitAtom::Record(index))
}
fn word(x: usize, z: usize) -> PauliWord {
    // Native x=z=1 denotes Hermitian Y: i X Z, not the real XZ matrix.
    PauliWord::new(bits(1, x), bits(1, z), if x & z != 0 { 2 } else { 0 }).unwrap()
}
fn strategies() -> [CompilerStrategy; 2] {
    [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ]
}
fn policies() -> [QueryPreparation; 5] {
    [
        QueryPreparation::Cold,
        QueryPreparation::Trace,
        QueryPreparation::FixedFibers(Default::default()),
        QueryPreparation::FixedCatalogue(Default::default()),
        QueryPreparation::Native(Default::default()),
    ]
}
fn options(strategy: CompilerStrategy, mode: QueryPreparation) -> CompileOptions {
    CompileOptions {
        compiler_strategy: strategy,
        probability_preparation: mode,
        density_preparation: mode,
        moment_preparation: mode,
        ..Default::default()
    }
}
fn entangled(options: &CompileOptions, a: &ScalarArithmetic) -> CompiledInstrument {
    assert_eq!(ORIGINAL_INITIAL_X_MASKS, [1]);
    let zero = CliffordCircuit::computational(bits(ORIGINAL_INPUT_WIDTH, 0)).unwrap();
    let input = CliffordCircuit::new(ORIGINAL_INPUT_WIDTH, [CliffordGate::H(0)])
        .unwrap()
        .evolve(&zero, a)
        .unwrap()
        .unwrap();
    compile_affine_stabilizer(
        &parse_circuit(ORIGINAL_SOURCE, &ParseOptions::default()).unwrap(),
        &input,
        options,
        a,
    )
    .unwrap()
}

#[test]
fn native_entangled_trace_and_postselection_are_distinct() {
    let a = ScalarArithmetic::default();
    let half = a.omega(0, -2).unwrap();
    let quarter = a.omega(0, -4).unwrap();
    let signed_sector = a.omega(0, -3).unwrap(); // sqrt(2) / 4
    let target = compile_unitary(
        &parse_circuit("I 0", &ParseOptions::default()).unwrap(),
        &CompileOptions::default(),
        &a,
    )
    .unwrap();
    for strategy in strategies() {
        for mode in policies() {
            let original = entangled(&options(strategy, mode), &a);
            assert_eq!(
                original.acceptance(&[], 256, &a).unwrap().value,
                ExactScalar::one()
            );
            let views = ORIGINAL_OUTCOMES.map(|outcome| {
                original
                    .with_reported_conditions(&[(record(0), outcome)])
                    .unwrap()
            });
            for (outcome, view) in views.iter().enumerate() {
                assert_eq!(view.acceptance(&[], 256, &a).unwrap().value, half);
                assert_eq!(
                    view.target_density_weight(
                        &ORIGINAL_OUTPUTS,
                        &target,
                        DensityBoundary {
                            outputs: &[0],
                            event_rows: &[],
                            outcomes: &bits(0, 0)
                        },
                        &[],
                        256,
                        &a,
                    )
                    .unwrap()
                    .value,
                    quarter,
                );
                for reported in 0..2 {
                    assert_eq!(
                        view.reported_probability(&[record(0)], &bits(1, reported), &[], 256, &a)
                            .unwrap()
                            .value,
                        half,
                    );
                    assert_eq!(
                        view.accepted_probability(&[record(0)], &bits(1, reported), &[], 256, &a)
                            .unwrap()
                            .value,
                        if reported == outcome {
                            half.clone()
                        } else {
                            ExactScalar::zero()
                        },
                    );
                }
            }
            for (x, z) in ORIGINAL_PAULIS {
                let pauli = word(x, z);
                assert_eq!(
                    original
                        .pauli_moment(&ORIGINAL_OUTPUTS, &pauli, &[], 256, &a)
                        .unwrap()
                        .value,
                    ExactScalar::zero(),
                );
                let values = views.each_ref().map(|view| {
                    view.pauli_moment(&ORIGINAL_OUTPUTS, &pauli, &[], 256, &a)
                        .unwrap()
                        .value
                });
                assert_eq!(a.add(&values[0], &values[1]).unwrap(), ExactScalar::zero());
                let expected = if x != 0 {
                    signed_sector.clone()
                } else {
                    ExactScalar::zero()
                };
                assert_eq!(values, [expected.clone(), expected.negated()]);
                if x != 0 {
                    assert_ne!(values[0], ExactScalar::zero());
                    assert_ne!(values[1], ExactScalar::zero());
                }
            }
            assert_eq!(
                original.acceptance(&[], 256, &a).unwrap().value,
                ExactScalar::one()
            );
        }
    }
}

#[test]
fn appended_conditions_own_inputs_preserve_acceptance_and_validate_records() {
    let a = ScalarArithmetic::default();
    let half = a.omega(0, -2).unwrap();
    let source = parse_circuit(
        "H 0\nCX 0 1\nM 0 1\nDETECTOR rec[-2]\nPOSTSELECT det[0]",
        &ParseOptions::default(),
    )
    .unwrap();
    for strategy in strategies() {
        for mode in policies() {
            let original = compile_affine(&source, &options(strategy, mode), &a).unwrap();
            let mut inputs = vec![(record(1), false)];
            let view = original.with_reported_conditions(&inputs).unwrap();
            inputs[0] = (record(999), true);
            inputs.clear();
            drop(inputs);
            let mut dependent = record(0);
            dependent.xor_assign(&record(1));
            let dependent = view
                .with_reported_conditions(&[(dependent, false), (record(1), false)])
                .unwrap();
            let opposite = view.with_reported_conditions(&[(record(1), true)]).unwrap();
            let conflicts_with_original = original
                .with_reported_conditions(&[(record(0), true)])
                .unwrap();
            let constant = BitExpr {
                constant: true,
                ..Default::default()
            };
            let impossible_constant = original
                .with_reported_conditions(&[(constant.clone(), false)])
                .unwrap();
            let tautology = view.with_reported_conditions(&[(constant, true)]).unwrap();
            for p in [&original, &view, &dependent, &tautology] {
                assert_eq!(p.acceptance(&[], 256, &a).unwrap().value, half);
            }
            for p in [&opposite, &conflicts_with_original, &impossible_constant] {
                assert_eq!(
                    p.acceptance(&[], 256, &a).unwrap().value,
                    ExactScalar::zero()
                );
                assert_eq!(
                    p.pauli_moment(&[0], &word(0, 1), &[], 256, &a)
                        .unwrap()
                        .value,
                    ExactScalar::zero()
                );
                assert_eq!(
                    p.reported_probability(&[], &bits(0, 0), &[], 256, &a)
                        .unwrap()
                        .value,
                    ExactScalar::one()
                );
            }
            assert!(matches!(
                view.with_reported_conditions(&[(record(2), false)]),
                Err(InstrumentError::RecordOutOfBounds {
                    record: 2,
                    count: 2
                })
            ));
            assert!(matches!(
                view.with_reported_conditions(&[(
                    BitExpr::atom(BitAtom::Decision { call: 0, bit: 0 }),
                    false
                )]),
                Err(InstrumentError::NonAffineQuery)
            ));
            assert_eq!(
                original
                    .with_reported_conditions(&[])
                    .unwrap()
                    .acceptance(&[], 256, &a)
                    .unwrap()
                    .value,
                half
            );
            assert_eq!(original.acceptance(&[], 256, &a).unwrap().value, half);
            assert_eq!(view.acceptance(&[], 256, &a).unwrap().value, half);
        }
    }
}

#[test]
fn views_share_prepared_families_limits_and_outcome_independent_geometry() {
    let a = ScalarArithmetic::default();
    let target = compile_unitary(
        &parse_circuit("I 0", &ParseOptions::default()).unwrap(),
        &CompileOptions::default(),
        &a,
    )
    .unwrap();
    let x = word(1, 0);
    for strategy in strategies() {
        for mode in policies() {
            let mut config = options(strategy, mode);
            config.max_prepared_moments = 1;
            let original = entangled(&config, &a);
            let zero = original
                .with_reported_conditions(&[(record(0), false)])
                .unwrap();
            let one = original
                .with_reported_conditions(&[(record(0), true)])
                .unwrap();
            let cold = matches!(mode, QueryPreparation::Cold);
            for query in [
                Q::Acceptance,
                Q::PauliMoment {
                    outputs: &[0],
                    word: &x,
                },
                Q::TargetDensity {
                    outputs: &[0],
                    target: &target,
                    target_layout: DensityLayout {
                        outputs: &[0],
                        event_rows: &[],
                    },
                },
            ] {
                assert_eq!(
                    zero.prepare_query(query, 256, &a).unwrap(),
                    if cold { O::Cold } else { O::Created }
                );
                assert_eq!(
                    one.prepare_query(query, 256, &a).unwrap(),
                    if cold { O::Cold } else { O::Reused }
                );
            }
            for p in [&original, &zero, &one] {
                assert_eq!(
                    p.prepared_moment_statistics().unwrap().cached_queries,
                    usize::from(!cold)
                );
                assert_eq!(
                    p.prepared_density_statistics().unwrap().cached_queries,
                    usize::from(!cold)
                );
                assert_eq!(
                    p.prepared_probability_statistics().unwrap().cached_queries,
                    usize::from(!cold)
                );
            }
            if !cold {
                assert!(matches!(
                    original.prepare_query(
                        Q::PauliMoment {
                            outputs: &[0],
                            word: &x
                        },
                        256,
                        &a
                    ),
                    Err(InstrumentError::ResourceLimit {
                        option: "max_prepared_moments",
                        limit: 1,
                        required: 2
                    })
                ));
            }
            assert_eq!(
                zero.pauli_moment(&[0], &x, &[], 256, &a).unwrap().value,
                a.omega(0, -3).unwrap()
            );
            assert_eq!(
                one.pauli_moment(&[0], &x, &[], 256, &a).unwrap().value,
                a.omega(0, -3).unwrap().negated()
            );
            assert_eq!(
                original.acceptance(&[], 256, &a).unwrap().value,
                ExactScalar::one()
            );
        }
    }
}

#[test]
fn views_keep_prescribed_fault_report_flip_and_source_postselection_semantics() {
    let a = ScalarArithmetic::default();
    let source = "H 0\nCX 0 1\nMX(1/7) 1\nZ_ERROR(1/5) 0";
    let selected = format!("{source}\nDETECTOR rec[-1]\nPOSTSELECT det[0]");
    for strategy in strategies() {
        for mode in policies() {
            let config = options(strategy, mode);
            let original = compile_affine(
                &parse_circuit(source, &ParseOptions::default()).unwrap(),
                &config,
                &a,
            )
            .unwrap();
            let source_selected = compile_affine(
                &parse_circuit(&selected, &ParseOptions::default()).unwrap(),
                &config,
                &a,
            )
            .unwrap();
            let zero = original
                .with_reported_conditions(&[(record(0), false)])
                .unwrap();
            let one = original
                .with_reported_conditions(&[(record(0), true)])
                .unwrap();
            let site = |denominator: i64| {
                original
                    .noise_sites()
                    .iter()
                    .position(|s| {
                        s.alternatives().iter().any(|p| {
                            p.probability().value()
                                == &num_rational::BigRational::new(1.into(), denominator.into())
                        })
                    })
                    .unwrap()
            };
            let report = site(7);
            let physical = site(5);
            assert_ne!(report, physical);
            for mask in 0..4 {
                let faults = [(report, 1), (physical, 2)]
                    .into_iter()
                    .filter_map(|(site, flag)| {
                        (mask & flag != 0).then_some(FaultSelection {
                            site,
                            alternative: 0,
                        })
                    })
                    .collect::<Vec<_>>();
                let expected_fault_mass = num_rational::BigRational::new(
                    (if mask & 1 != 0 { 1 } else { 6 } * if mask & 2 != 0 { 1 } else { 4 }).into(),
                    35.into(),
                );
                for (outcome, p) in [&zero, &one].into_iter().enumerate() {
                    assert_eq!(p.fault_probability(&faults).unwrap(), expected_fault_mass);
                    assert_eq!(
                        p.acceptance(&faults, 256, &a).unwrap().value,
                        a.omega(0, -2).unwrap()
                    );
                    let negative = (outcome != 0) ^ (mask & 1 != 0) ^ (mask & 2 != 0);
                    let expected = a.omega(if negative { 4 } else { 0 }, -2).unwrap();
                    assert_eq!(
                        p.pauli_moment(&[0], &word(1, 0), &faults, 256, &a)
                            .unwrap()
                            .value,
                        expected
                    );
                    for reported in 0..2 {
                        assert_eq!(
                            p.reported_probability(
                                &[record(0)],
                                &bits(1, reported),
                                &faults,
                                256,
                                &a
                            )
                            .unwrap()
                            .value,
                            a.omega(0, -2).unwrap()
                        );
                    }
                }
                assert_eq!(
                    original.fault_probability(&faults).unwrap(),
                    expected_fault_mass
                );
                assert_eq!(
                    original.acceptance(&faults, 256, &a).unwrap().value,
                    ExactScalar::one()
                );
                assert_eq!(
                    zero.acceptance(&faults, 256, &a).unwrap().value,
                    source_selected.acceptance(&faults, 256, &a).unwrap().value
                );
                assert_eq!(
                    zero.pauli_moment(&[0], &word(1, 0), &faults, 256, &a)
                        .unwrap()
                        .value,
                    source_selected
                        .pauli_moment(&[0], &word(1, 0), &faults, 256, &a)
                        .unwrap()
                        .value
                );
            }
        }
    }
}
