// SPDX-License-Identifier: MPL-2.0
// Continuation scenarios from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f.
// Full source generator fixture includes tests/native/reference_reuse.cpp.
use rmagna::{
    clifford::CliffordGate as C,
    gf2::BitVector,
    pauli::PauliWord,
    scalar::{ExactScalar, ScalarArithmetic},
    stabilizer::AffineKet,
    unitary::{
        CompiledUnitary, CompilerStrategy, ContinuationOptions, OwnedStabilizerExpansion,
        PauliInsertion, UnitaryCircuit, UnitaryError, UnitaryGate as U,
    },
};
const COMPILERS: [CompilerStrategy; 2] = [
    CompilerStrategy::ParityCopies,
    CompilerStrategy::SharedCommutingLayer,
];
fn bits(width: usize, indices: impl IntoIterator<Item = usize>) -> BitVector {
    BitVector::from_indices(width, indices).unwrap()
}
fn dense(expansion: &OwnedStabilizerExpansion, a: &ScalarArithmetic) -> Vec<ExactScalar> {
    let mut values = vec![ExactScalar::zero(); 1 << expansion.width()];
    for ket in expansion.entries() {
        for code in 0..1usize << ket.columns().len() {
            let coordinates = bits(
                ket.columns().len(),
                (0..ket.columns().len()).filter(|&j| code & (1 << j) != 0),
            );
            let mut output = ket.offset().clone();
            for j in coordinates.ones() {
                output.xor_assign(&ket.columns()[j]).unwrap();
            }
            let index = output.ones().fold(0, |word, q| word | (1 << q));
            let phase = a
                .omega(
                    2 * i64::from(ket.phase().evaluate(&coordinates).unwrap()),
                    0,
                )
                .unwrap();
            values[index] = a
                .add(
                    &values[index],
                    &a.multiply(ket.coefficient(), &phase).unwrap(),
                )
                .unwrap();
        }
    }
    values
}
fn pauli(width: usize, q: usize, letter: usize) -> PauliWord {
    PauliWord::new(
        bits(width, (letter != 2).then_some(q)),
        bits(width, (letter != 1).then_some(q)),
        if letter == 3 { 2 } else { 0 },
    )
    .unwrap()
}
pub fn warm_templates() {
    let a = ScalarArithmetic::default();
    let half = a.omega(0, -2).unwrap();
    let mut cases = 0;
    for compiler in COMPILERS {
        let source =
            UnitaryCircuit::new(1, vec![U::Clifford(C::H(0)), U::T { qubit: 0, turns: 1 }])
                .unwrap()
                .compile_with_strategy(&bits(1, []), &a, compiler)
                .unwrap();
        let suffix =
            UnitaryCircuit::new(1, vec![U::T { qubit: 0, turns: 7 }, U::Clifford(C::H(0))])
                .unwrap();
        let continuation = source
            .prepare_continuation(&suffix, Default::default(), &a)
            .unwrap();
        let plan = continuation
            .prepare_probability_with_fibers(&[bits(1, [0])], 16, Default::default(), &a)
            .unwrap();
        let before = continuation.query_work_statistics();
        for _ in 0..8 {
            for position in [1, 2] {
                for letter in 1..=3 {
                    let assigned = source
                        .with_insertions(
                            &[PauliInsertion {
                                before_gate: position,
                                word: pauli(1, 0, letter),
                            }],
                            &a,
                        )
                        .unwrap();
                    let view = continuation.apply(&assigned, &a).unwrap();
                    let expected = if letter == 1 && position == 1 {
                        ExactScalar::one()
                    } else if letter == 2 || (letter == 3 && position == 1) {
                        ExactScalar::zero()
                    } else {
                        half.clone()
                    };
                    assert_eq!(
                        view.evaluate_probability(&plan, &bits(1, []), 16, &a)
                            .unwrap()
                            .value,
                        expected
                    );
                    cases += 1;
                }
            }
        }
        let after = continuation.query_work_statistics();
        assert_eq!(
            after.affine_closures_prepared,
            before.affine_closures_prepared
        );
        assert_eq!(after.phase_maps_prepared, before.phase_maps_prepared);
        assert_eq!(
            after.phase_resources_prepared,
            before.phase_resources_prepared
        );
        let stats = continuation.statistics();
        assert_eq!(
            (
                stats.suffix_compilations,
                stats.transport_tables_prepared,
                stats.frame_applications
            ),
            (1, 1, 48)
        );
        assert_eq!(stats.query_original_positions, 0);
        assert_eq!(stats.suffix_original_positions, 2);
        assert_eq!(stats.unique_program_owners, 3);
        assert!(stats.unique_reference_boundaries <= 3);
        let foreign =
            UnitaryCircuit::new(1, vec![U::Clifford(C::H(0)), U::T { qubit: 0, turns: 1 }])
                .unwrap()
                .compile_with_strategy(&bits(1, []), &a, compiler)
                .unwrap();
        assert!(matches!(
            continuation.apply(&foreign, &a),
            Err(UnitaryError::IncompatibleProgram)
        ));
        assert_eq!(continuation.statistics(), stats);
        assert!(matches!(
            source.prepare_continuation(
                &suffix,
                ContinuationOptions {
                    max_model_geometry_words: 0,
                    ..Default::default()
                },
                &a
            ),
            Err(UnitaryError::ContinuationResourceLimit {
                option: "max_model_geometry_words",
                ..
            })
        ));
        let input = AffineKet::new(
            bits(1, []),
            vec![],
            rmagna::quadratic::QuadraticForm::new(0, vec![], vec![]).unwrap(),
            a.omega(3, 0).unwrap(),
        )
        .unwrap();
        let phased =
            UnitaryCircuit::new(1, vec![U::Clifford(C::H(0)), U::T { qubit: 0, turns: 1 }])
                .unwrap()
                .compile_stabilizer_with_strategy(&input, &a, compiler)
                .unwrap();
        let continued = phased
            .prepare_continuation(&suffix, Default::default(), &a)
            .unwrap()
            .apply(&phased, &a)
            .unwrap();
        assert_eq!(
            dense(
                &continued
                    .stabilizer_expansion(Default::default(), &a)
                    .unwrap(),
                &a
            ),
            [a.omega(3, 0).unwrap(), ExactScalar::zero()]
        );
    }
    println!(
        "{cases} fixed prepared continuation masses,2compilers,no hot suffix/table/resource/map factories;exact single global coefficient and foreign/budget refusals"
    );
}

pub fn mixed_magic_compilers() {
    let a = ScalarArithmetic::default();
    let mut cases = 0;
    for source_compiler in COMPILERS {
        for suffix_compiler in COMPILERS {
            for kind in 0..4 {
                let mut prefix = vec![
                    U::Clifford(C::H(0)),
                    U::Clifford(C::H(1)),
                    U::Clifford(C::H(2)),
                ];
                prefix.extend(match kind {
                    0 => vec![U::T { qubit: 0, turns: 1 }],
                    1 => vec![U::Cs {
                        a: 0,
                        b: 1,
                        turns: 3,
                    }],
                    2 => vec![U::Ccz { a: 0, b: 1, c: 2 }],
                    _ => vec![U::T { qubit: 0, turns: 1 }, U::T { qubit: 0, turns: 7 }],
                });
                let source = UnitaryCircuit::new(3, prefix.clone())
                    .unwrap()
                    .compile_with_strategy(&bits(3, []), &a, source_compiler)
                    .unwrap();
                let magic = match kind {
                    0 => U::Ccz { a: 0, b: 1, c: 3 },
                    1 => U::Cs {
                        a: 1,
                        b: 3,
                        turns: 1,
                    },
                    _ => U::T { qubit: 3, turns: 3 },
                };
                let tail = vec![
                    U::Clifford(C::H(3)),
                    U::Clifford(C::Cx {
                        control: 0,
                        target: 3,
                    }),
                    U::Clifford(C::H(1)),
                    magic,
                    U::Clifford(C::H(0)),
                ];
                let suffix = UnitaryCircuit::new(4, tail.clone()).unwrap();
                let continuation = source
                    .prepare_continuation(
                        &suffix,
                        ContinuationOptions {
                            compiler: Some(suffix_compiler),
                            ..Default::default()
                        },
                        &a,
                    )
                    .unwrap();
                let probability = continuation
                    .prepare_probability_with_fibers(&[bits(4, [0])], 4096, Default::default(), &a)
                    .unwrap();
                let query_before = continuation.query_work_statistics();
                prefix.extend(tail);
                let fresh = UnitaryCircuit::new(4, prefix)
                    .unwrap()
                    .compile_with_strategy(&bits(4, []), &a, source_compiler)
                    .unwrap();
                for index in 0..12 {
                    let position = if index < 9 { 3 } else { 4 };
                    let q = index % 3;
                    let letter = 1 + index / 3 % 3;
                    let word = pauli(3, q, letter);
                    let phase = (word.phase() + [0, 2, 7][index % 3]) & 7;
                    let input = PauliWord::new(word.x().clone(), word.z().clone(), phase).unwrap();
                    let assigned = source
                        .with_insertions(
                            &[PauliInsertion {
                                before_gate: position,
                                word: input,
                            }],
                            &a,
                        )
                        .unwrap();
                    let continued = continuation.apply(&assigned, &a).unwrap();
                    let lifted = pauli(4, q, letter);
                    let expected = fresh
                        .with_insertions(
                            &[PauliInsertion {
                                before_gate: position,
                                word: PauliWord::new(lifted.x().clone(), lifted.z().clone(), phase)
                                    .unwrap(),
                            }],
                            &a,
                        )
                        .unwrap();
                    let expected_state = state(&expected, &a);
                    assert_eq!(
                        dense(
                            &continued
                                .stabilizer_expansion(Default::default(), &a)
                                .unwrap(),
                            &a
                        ),
                        expected_state,
                        "source={source_compiler:?} suffix={suffix_compiler:?} kind={kind} index={index}"
                    );
                    for bit in 0..2 {
                        let mass = expected_state
                            .iter()
                            .enumerate()
                            .filter(|(index, _)| index & 1 == bit)
                            .try_fold(ExactScalar::zero(), |sum, (_, value)| {
                                a.add(&sum, &a.multiply(&value.conjugate(), value).unwrap())
                            })
                            .unwrap();
                        assert_eq!(
                            continued
                                .evaluate_probability(
                                    &probability,
                                    &bits(1, (bit != 0).then_some(0)),
                                    4096,
                                    &a
                                )
                                .unwrap()
                                .value,
                            mass,
                            "prepared mixed-compiler mass source={source_compiler:?} suffix={suffix_compiler:?} kind={kind} index={index} bit={bit}"
                        );
                    }
                    cases += 1;
                }
                let stats = continuation.statistics();
                assert_eq!(stats.frame_applications, 12);
                assert_eq!(stats.suffix_compilations, 1);
                assert_eq!(stats.transport_tables_prepared, 1);
                assert_eq!(stats.query_original_positions, 0);
                let query_after = continuation.query_work_statistics();
                assert_eq!(
                    query_after.affine_closures_prepared,
                    query_before.affine_closures_prepared
                );
                assert_eq!(
                    query_after.affine_geometry_preparations,
                    query_before.affine_geometry_preparations
                );
                assert_eq!(
                    query_after.phase_maps_prepared,
                    query_before.phase_maps_prepared
                );
                assert_eq!(
                    query_after.phase_resources_prepared,
                    query_before.phase_resources_prepared
                );
                assert_eq!(
                    query_after.fixed_evaluations - query_before.fixed_evaluations,
                    24
                );
                for (options, option) in [
                    (
                        ContinuationOptions {
                            max_coherent_width: 0,
                            ..Default::default()
                        },
                        "max_coherent_width",
                    ),
                    (
                        ContinuationOptions {
                            max_suffix_gates: 0,
                            ..Default::default()
                        },
                        "max_suffix_gates",
                    ),
                ] {
                    assert!(
                        matches!(source.prepare_continuation(&suffix, options, &a), Err(UnitaryError::ContinuationResourceLimit { option: actual, .. }) if actual == option)
                    );
                }
                for insertion_options in [
                    rmagna::unitary::InsertionPreparationOptions {
                        max_catalog_entries: 0,
                        ..Default::default()
                    },
                    rmagna::unitary::InsertionPreparationOptions {
                        max_table_bytes: 0,
                        ..Default::default()
                    },
                ] {
                    assert!(matches!(
                        source.prepare_continuation(
                            &suffix,
                            ContinuationOptions {
                                insertions: insertion_options,
                                ..Default::default()
                            },
                            &a
                        ),
                        Err(UnitaryError::InsertionResourceLimit { .. })
                    ));
                }
                assert_eq!(continuation.statistics(), stats);
            }
        }
    }
    assert_eq!(cases, 192);
    println!(
        "192 mixed-compiler continuation states and384prepared masses: T/CS/CCZ/cancelled T axes,three source wires plus fresh physical wire,all Pauli letters and nontrivial global phases; exact fresh-full-compile states,zero hot scalar factories and resource refusals"
    );
}

fn state(compiled: &CompiledUnitary, a: &ScalarArithmetic) -> Vec<ExactScalar> {
    dense(
        &compiled
            .stabilizer_expansion(Default::default(), a)
            .unwrap(),
        a,
    )
}
struct Fixture {
    width: usize,
    gates: Vec<U>,
    ends: Vec<usize>,
    reset: bool,
}
fn fixture(variant: usize) -> Fixture {
    let mut gates = vec![];
    let mut ends = vec![];
    let mut add = |new: Vec<U>| {
        gates.extend(new);
        ends.push(gates.len());
    };
    add(vec![U::Clifford(C::H(0))]);
    add(vec![U::Clifford(C::H(1))]);
    add(vec![U::T {
        qubit: 0,
        turns: if variant & 1 != 0 { 7 } else { 1 },
    }]);
    add(vec![U::Clifford(C::Cx {
        control: 0,
        target: 1,
    })]);
    add(vec![
        U::Clifford(C::Cx {
            control: 1,
            target: 2,
        }),
        U::T { qubit: 2, turns: 3 },
        U::Clifford(C::Cx {
            control: 1,
            target: 2,
        }),
    ]);
    if variant & 2 != 0 {
        add(vec![U::Clifford(C::H(0))]);
        add(vec![U::T { qubit: 0, turns: 5 }]);
    }
    if variant & 4 != 0 {
        // Coherent MZ record3; fresh + lifetime4 replaces original wire2.
        add(vec![U::Clifford(C::Cx {
            control: 1,
            target: 3,
        })]);
        add(vec![U::Clifford(C::H(4))]);
    }
    // Realize source output order [2,0,1] as a fixed coordinate permutation.
    // Original fault locations remain before these endpoint-only swaps.
    if variant & 4 != 0 {
        gates.extend([
            U::Clifford(C::Swap { a: 0, b: 4 }),
            U::Clifford(C::Swap { a: 1, b: 4 }),
            U::Clifford(C::Swap { a: 2, b: 4 }),
        ]);
    } else {
        gates.extend([
            U::Clifford(C::Swap { a: 0, b: 2 }),
            U::Clifford(C::Swap { a: 1, b: 2 }),
        ]);
    }
    Fixture {
        width: if variant & 4 != 0 { 5 } else { 3 },
        gates,
        ends,
        reset: variant & 4 != 0,
    }
}
fn suffix(f: &Fixture) -> UnitaryCircuit {
    // Source slots0/1/2 now denote logical wires2/0/1, respectively.
    let new = f.width;
    let mut gates = vec![
        U::Clifford(C::H(new)),
        U::Clifford(C::Cx {
            control: 1,
            target: new,
        }),
        U::T {
            qubit: new,
            turns: 7,
        },
        U::Clifford(C::H(0)),
        U::Clifford(C::Cz { a: 0, b: new }),
        U::Clifford(C::Cx {
            control: 1,
            target: 0,
        }),
        U::T { qubit: 0, turns: 3 },
        U::Clifford(C::Cx {
            control: 1,
            target: 0,
        }),
    ];
    // Final output order [3,1,0,2], followed by record and environment slots.
    gates.extend([
        U::Clifford(C::Swap { a: 0, b: new }),
        U::Clifford(C::Swap { a: 1, b: 2 }),
    ]);
    if f.reset {
        gates.extend([
            U::Clifford(C::Swap { a: 3, b: 5 }),
            U::Clifford(C::Swap { a: 4, b: 5 }),
        ]);
    }
    UnitaryCircuit::new(f.width + 1, gates).unwrap()
}
pub fn all_source_generators() {
    let a = ScalarArithmetic::default();
    let mut total = 0;
    let mut frame_pairs = 0;
    let mut record_cases = 0;
    for compiler in COMPILERS {
        let mut generators = 0;
        for variant in 0..8 {
            let f = fixture(variant);
            let source = UnitaryCircuit::new(f.width, f.gates.clone())
                .unwrap()
                .compile_with_strategy(&bits(f.width, []), &a, compiler)
                .unwrap();
            let tail = suffix(&f);
            let continuation = source
                .prepare_continuation(&tail, Default::default(), &a)
                .unwrap();
            let mut all = f.gates.clone();
            all.extend_from_slice(tail.gates());
            let fresh = UnitaryCircuit::new(f.width + 1, all)
                .unwrap()
                .compile_with_strategy(&bits(f.width + 1, []), &a, compiler)
                .unwrap();
            let initial = continuation.statistics();
            assert_eq!(initial.source_original_positions, f.gates.len());
            assert_eq!(initial.suffix_original_positions, tail.gates().len());
            assert_eq!(initial.query_original_positions, 0);
            for (location, &end) in f.ends.iter().enumerate() {
                for wire in 0..3 {
                    let q = if f.reset && wire == 2 && location == f.ends.len() - 1 {
                        4
                    } else {
                        wire
                    };
                    for letter in 1..=3 {
                        let assigned = source
                            .with_insertions(
                                &[PauliInsertion {
                                    before_gate: end,
                                    word: pauli(f.width, q, letter),
                                }],
                                &a,
                            )
                            .unwrap();
                        let full = fresh
                            .with_insertions(
                                &[PauliInsertion {
                                    before_gate: end,
                                    word: pauli(f.width + 1, q, letter),
                                }],
                                &a,
                            )
                            .unwrap();
                        let found = continuation.apply(&assigned, &a).unwrap();
                        if compiler == CompilerStrategy::ParityCopies {
                            assert_eq!(
                                found.transported_frame(),
                                full.assignment_frame().unwrap(),
                                "variant{variant}/location{location}/wire{wire}/pauli{letter}"
                            );
                            frame_pairs += 1;
                        }
                        let found = dense(
                            &found.stabilizer_expansion(Default::default(), &a).unwrap(),
                            &a,
                        );
                        assert_eq!(
                            found,
                            state(&full, &a),
                            "{compiler:?}/variant{variant}/location{location}/wire{wire}/pauli{letter}"
                        );
                        if f.reset {
                            let flipped = wire == 1 && location >= f.ends.len() - 2 && letter != 2;
                            let mut one = ExactScalar::zero();
                            for (word, value) in found.iter().enumerate() {
                                if !value.is_zero() {
                                    assert_eq!(
                                        ((word >> 1) ^ (word >> 4)) & 1,
                                        usize::from(flipped)
                                    );
                                }
                                if word & (1 << 4) != 0 {
                                    one = a.add(&one, &a.norm_squared(value).unwrap()).unwrap();
                                }
                            }
                            assert_eq!(one, a.omega(0, -2).unwrap());
                            record_cases += 1;
                        }
                        generators += 1;
                    }
                }
            }
            let final_stats = continuation.statistics();
            assert_eq!(final_stats.frame_applications, (f.ends.len() * 9) as u64);
            assert_eq!(
                (
                    final_stats.suffix_compilations,
                    final_stats.transport_tables_prepared
                ),
                (1, 1)
            );
            assert_eq!(final_stats.unique_program_owners, 3);
            assert_eq!(final_stats.source_phase_axes, initial.source_phase_axes);
        }
        assert_eq!(generators, 504);
        total += generators;
    }
    println!(
        "{total} complete original generators across2compilers,{frame_pairs} exact raw phase/coordinate pairs,{record_cases} MZ temporal/correlation checks; exact full purified states and output permutations match fresh compilation without retaining duplicate prefix positions"
    );
}
