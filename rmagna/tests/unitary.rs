use rmagna::clifford::CliffordGate as C;
use rmagna::gf2::BitVector;
use rmagna::pauli::PauliWord;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::unitary::{PauliInsertion, UnitaryCircuit, UnitaryError, UnitaryGate as G};
mod common;
use common::{bits, dense_gate, next};

fn dense(
    gates: &[G],
    width: usize,
    input: usize,
    arithmetic: &ScalarArithmetic,
) -> Vec<ExactScalar> {
    dense_inserted(gates, width, input, &[], arithmetic)
}

fn dense_inserted(
    gates: &[G],
    width: usize,
    input: usize,
    insertions: &[PauliInsertion],
    arithmetic: &ScalarArithmetic,
) -> Vec<ExactScalar> {
    let mut values = vec![ExactScalar::zero(); 1 << width];
    values[input] = ExactScalar::one();
    for (position, gate) in gates.iter().copied().map(Some).chain([None]).enumerate() {
        for insertion in insertions.iter().filter(|p| p.before_gate == position) {
            let word = &insertion.word;
            let flip = word.x().ones().map(|j| 1 << j).sum::<usize>();
            let mut output = vec![ExactScalar::zero(); values.len()];
            for (x, value) in values.iter().enumerate() {
                let sign = word.z().dot(&bits(width, x)).unwrap();
                output[x ^ flip] = arithmetic
                    .multiply(
                        value,
                        &arithmetic
                            .omega(i64::from(word.phase()) + 4 * i64::from(sign), 0)
                            .unwrap(),
                    )
                    .unwrap();
            }
            values = output;
        }
        let Some(gate) = gate else {
            break;
        };
        if let G::Clifford(gate) = gate {
            dense_gate(gate, &mut values, arithmetic);
            continue;
        }
        // Direct diagonal matrices, independent of the compiler's parity-copy
        // identities and its Gaussian/quotient contraction.
        for (x, value) in values.iter_mut().enumerate() {
            let phase = match gate {
                G::T { qubit, turns } => i64::from(turns) * ((x >> qubit) & 1) as i64,
                G::Cs { a, b, turns } => 2 * i64::from(turns) * ((x >> a) & (x >> b) & 1) as i64,
                G::Ccz { a, b, c } => 4 * ((x >> a) & (x >> b) & (x >> c) & 1) as i64,
                G::Clifford(_) => unreachable!(),
            };
            *value = arithmetic
                .multiply(value, &arithmetic.omega(phase, 0).unwrap())
                .unwrap();
        }
    }
    values
}

fn reduced_density(
    state: &[ExactScalar],
    width: usize,
    outputs: &[usize],
    row: &BitVector,
    outcome: bool,
    a: &ScalarArithmetic,
) -> Vec<Vec<ExactScalar>> {
    let mut rho = vec![vec![ExactScalar::zero(); 1 << outputs.len()]; 1 << outputs.len()];
    let selected = |x: usize| {
        outputs
            .iter()
            .enumerate()
            .fold(0, |out, (j, &q)| out | (((x >> q) & 1) << j))
    };
    for (x, vx) in state.iter().enumerate() {
        for (y, vy) in state.iter().enumerate() {
            if row.dot(&bits(width, x)).unwrap() != outcome
                || row.dot(&bits(width, y)).unwrap() != outcome
            {
                continue;
            }
            if (0..width).any(|q| !outputs.contains(&q) && ((x >> q) & 1) != ((y >> q) & 1)) {
                continue;
            }
            let term = a.multiply(vx, &vy.conjugate()).unwrap();
            let entry = &mut rho[selected(x)][selected(y)];
            *entry = a.add(entry, &term).unwrap();
        }
    }
    rho
}
fn density_product(
    rho: &[Vec<ExactScalar>],
    tau: &[Vec<ExactScalar>],
    a: &ScalarArithmetic,
) -> ExactScalar {
    let mut total = ExactScalar::zero();
    for (x, row) in rho.iter().enumerate() {
        for (y, value) in row.iter().enumerate() {
            total = a
                .add(&total, &a.multiply(value, &tau[y][x]).unwrap())
                .unwrap();
        }
    }
    total
}

#[test]
fn prescribed_pauli_insertions_reuse_the_reference_boundary_and_match_dense_faults() {
    let arithmetic = ScalarArithmetic::default();
    let gates = [
        G::Clifford(C::H(0)),
        G::T { qubit: 0, turns: 1 },
        G::Clifford(C::Cx {
            control: 0,
            target: 1,
        }),
        G::Clifford(C::H(2)),
        G::Cs {
            a: 1,
            b: 2,
            turns: 3,
        },
        G::Ccz { a: 0, b: 1, c: 2 },
        G::Clifford(C::H(1)),
        G::T { qubit: 1, turns: 0 },
        G::Clifford(C::S { qubit: 2, turns: 3 }),
    ];
    let base = UnitaryCircuit::new(3, gates)
        .unwrap()
        .compile(&bits(3, 0), &arithmetic)
        .unwrap();
    let mut rng = 0x781234abcde;
    for trial in 0..64 {
        let insertions = (0..3)
            .map(|_| PauliInsertion {
                before_gate: next(&mut rng) % (gates.len() + 1),
                word: PauliWord::new(
                    bits(3, next(&mut rng)),
                    bits(3, next(&mut rng)),
                    (next(&mut rng) % 8) as u8,
                )
                .unwrap(),
            })
            .collect::<Vec<_>>();
        let actual = base.with_insertions(&insertions, &arithmetic).unwrap();
        assert_eq!(actual.phase_legs(), base.phase_legs());
        let expected = dense_inserted(&gates, 3, 0, &insertions, &arithmetic);
        for (x, value) in expected.iter().enumerate() {
            assert_eq!(
                &actual
                    .amplitude(&bits(3, x), 256, &arithmetic)
                    .unwrap()
                    .value,
                value,
                "trial={trial}, x={x}"
            );
        }
        let row = bits(3, next(&mut rng));
        for y in 0..2 {
            let mut mass = ExactScalar::zero();
            for (x, value) in expected.iter().enumerate() {
                if row.dot(&bits(3, x)).unwrap() == (y != 0) {
                    mass = arithmetic
                        .add(&mass, &arithmetic.norm_squared(value).unwrap())
                        .unwrap();
                }
            }
            assert_eq!(
                actual
                    .probability_z(std::slice::from_ref(&row), &bits(1, y), 256, &arithmetic)
                    .unwrap()
                    .value,
                mass
            );
        }
        let restored = actual.with_insertions(&[], &arithmetic).unwrap();
        assert_eq!(
            restored.overlap(&base, 256, &arithmetic).unwrap().value,
            ExactScalar::one()
        );
    }
    let x = PauliWord::new(bits(3, 1), bits(3, 0), 0).unwrap();
    let z = PauliWord::new(bits(3, 0), bits(3, 1), 0).unwrap();
    for insertions in [
        vec![
            PauliInsertion {
                before_gate: 1,
                word: x.clone(),
            },
            PauliInsertion {
                before_gate: 1,
                word: z.clone(),
            },
        ],
        vec![
            PauliInsertion {
                before_gate: 1,
                word: z,
            },
            PauliInsertion {
                before_gate: 1,
                word: x,
            },
        ],
    ] {
        let actual = base.with_insertions(&insertions, &arithmetic).unwrap();
        let expected = dense_inserted(&gates, 3, 0, &insertions, &arithmetic);
        for (x, value) in expected.iter().enumerate() {
            assert_eq!(
                &actual
                    .amplitude(&bits(3, x), 256, &arithmetic)
                    .unwrap()
                    .value,
                value
            );
        }
    }
    assert!(matches!(
        base.with_insertions(
            &[PauliInsertion {
                before_gate: gates.len() + 1,
                word: PauliWord::identity(3).unwrap()
            }],
            &arithmetic
        ),
        Err(UnitaryError::InvalidInsertion { .. })
    ));
    assert!(matches!(
        base.with_insertions(
            &[PauliInsertion {
                before_gate: 0,
                word: PauliWord::identity(2).unwrap()
            }],
            &arithmetic
        ),
        Err(UnitaryError::DimensionMismatch)
    ));
}

#[test]
fn noncommuting_unitary_networks_match_dense_amplitudes_and_events() {
    let arithmetic = ScalarArithmetic::default();
    let mut state = 0x8971bcdef;
    for width in 1..=4 {
        for trial in 0..12 {
            let mut gates = Vec::new();
            for layer in 0..4 {
                let q = next(&mut state) % width;
                gates.push(G::Clifford(C::H(q)));
                gates.push(G::T {
                    qubit: q,
                    turns: (next(&mut state) % 8) as u8,
                });
                if width > 1 {
                    let r = (q + 1 + next(&mut state) % (width - 1)) % width;
                    gates.push(if layer % 2 == 0 {
                        G::Cs {
                            a: q,
                            b: r,
                            turns: (next(&mut state) % 4) as u8,
                        }
                    } else {
                        G::Clifford(C::Cx {
                            control: q,
                            target: r,
                        })
                    });
                }
                if width > 2 && layer == 1 {
                    gates.push(G::Ccz { a: 0, b: 1, c: 2 });
                }
            }
            let input = next(&mut state) % (1 << width);
            let circuit = UnitaryCircuit::new(width, gates.clone()).unwrap();
            assert_eq!(circuit.width(), width);
            assert_eq!(circuit.gates(), gates);
            let compiled = circuit.compile(&bits(width, input), &arithmetic).unwrap();
            assert_eq!(compiled.width(), width);
            let expected = dense(&gates, width, input, &arithmetic);
            let mut distribution = vec![ExactScalar::zero(); 1 << width];
            for (x, value) in expected.iter().enumerate() {
                let actual = compiled
                    .amplitude(&bits(width, x), 256, &arithmetic)
                    .unwrap();
                assert_eq!(&actual.value, value, "width={width}, trial={trial}, x={x}");
                distribution[x] = arithmetic.norm_squared(value).unwrap();
            }
            let norm = compiled
                .probability_z(&[], &bits(0, 0), 256, &arithmetic)
                .unwrap();
            assert_eq!(norm.value, ExactScalar::one());
            let row = bits(width, next(&mut state));
            for outcome in 0..2 {
                let mut expected_mass = ExactScalar::zero();
                for (x, p) in distribution.iter().enumerate() {
                    if row.dot(&bits(width, x)).unwrap() == (outcome != 0) {
                        expected_mass = arithmetic.add(&expected_mass, p).unwrap();
                    }
                }
                assert_eq!(
                    compiled
                        .probability_z(
                            std::slice::from_ref(&row),
                            &bits(1, outcome),
                            256,
                            &arithmetic
                        )
                        .unwrap()
                        .value,
                    expected_mass
                );
            }
        }
    }
}

#[test]
fn target_overlap_closes_all_outputs_before_phase_reduction() {
    let arithmetic = ScalarArithmetic::default();
    let left = [
        G::Clifford(C::H(0)),
        G::T { qubit: 0, turns: 1 },
        G::Clifford(C::Cx {
            control: 0,
            target: 1,
        }),
        G::Clifford(C::H(0)),
        G::Cs {
            a: 0,
            b: 1,
            turns: 3,
        },
    ];
    let right = [
        G::Clifford(C::H(1)),
        G::T { qubit: 1, turns: 3 },
        G::Clifford(C::H(0)),
        G::Cs {
            a: 1,
            b: 0,
            turns: 1,
        },
        G::Clifford(C::S { qubit: 1, turns: 3 }),
    ];
    let a = UnitaryCircuit::new(2, left)
        .unwrap()
        .compile(&bits(2, 0), &arithmetic)
        .unwrap();
    let b = UnitaryCircuit::new(2, right)
        .unwrap()
        .compile(&bits(2, 1), &arithmetic)
        .unwrap();
    let av = dense(&left, 2, 0, &arithmetic);
    let bv = dense(&right, 2, 1, &arithmetic);
    let mut expected = ExactScalar::zero();
    for (a, b) in av.iter().zip(&bv) {
        expected = arithmetic
            .add(&expected, &arithmetic.multiply(a, &b.conjugate()).unwrap())
            .unwrap();
    }
    assert_eq!(
        a.overlap(&b, 256, &arithmetic).unwrap().value,
        expected.conjugate()
    );
    assert_eq!(b.overlap(&a, 256, &arithmetic).unwrap().value, expected);
    assert_eq!(
        a.overlap(&a, 256, &arithmetic).unwrap().value,
        ExactScalar::one()
    );
    let identity = UnitaryCircuit::new(2, [])
        .unwrap()
        .compile(&bits(2, 1), &arithmetic)
        .unwrap();
    assert_eq!(
        a.overlap(&identity, 256, &arithmetic).unwrap().value,
        av[1].conjugate()
    );
}

#[test]
fn eight_qubit_network_includes_a_genuine_residual_quotient() {
    let arithmetic = ScalarArithmetic::default();
    let gates = [
        G::Clifford(C::H(0)),
        G::Clifford(C::H(4)),
        G::Clifford(C::H(7)),
        G::T { qubit: 0, turns: 1 },
        G::T { qubit: 4, turns: 1 },
        G::T { qubit: 7, turns: 1 },
        G::Cs {
            a: 0,
            b: 4,
            turns: 3,
        },
        G::Ccz { a: 0, b: 4, c: 7 },
        G::Clifford(C::Cx {
            control: 4,
            target: 6,
        }),
        G::Clifford(C::H(4)),
        G::Clifford(C::H(6)),
        G::Clifford(C::H(7)),
        G::T { qubit: 7, turns: 5 },
        G::Clifford(C::H(0)),
    ];
    let compiled = UnitaryCircuit::new(8, gates)
        .unwrap()
        .compile(&bits(8, 0), &arithmetic)
        .unwrap();
    let expected = dense(&gates, 8, 0, &arithmetic);
    let mut max_rank = 0;
    for (x, value) in expected.iter().enumerate() {
        let result = compiled.amplitude(&bits(8, x), 256, &arithmetic).unwrap();
        max_rank = max_rank.max(result.quotient_rank);
        assert_eq!(&result.value, value);
    }
    assert!(max_rank > 1);
    let rows = [bits(8, 0x51), bits(8, 0xc0)];
    for y in 0..4 {
        let mut expected_mass = ExactScalar::zero();
        for (x, value) in expected.iter().enumerate() {
            if rows
                .iter()
                .enumerate()
                .all(|(j, row)| row.dot(&bits(8, x)).unwrap() == (y >> j & 1 != 0))
            {
                expected_mass = arithmetic
                    .add(&expected_mass, &arithmetic.norm_squared(value).unwrap())
                    .unwrap();
            }
        }
        assert_eq!(
            compiled
                .probability_z(&rows, &bits(2, y), 256, &arithmetic)
                .unwrap()
                .value,
            expected_mass
        );
    }
}

#[test]
fn one_qubit_interference_and_global_clifford_relations_are_exact() {
    let arithmetic = ScalarArithmetic::default();
    let circuit = UnitaryCircuit::new(
        1,
        [
            G::Clifford(C::H(0)),
            G::T { qubit: 0, turns: 1 },
            G::Clifford(C::H(0)),
        ],
    )
    .unwrap()
    .compile(&bits(1, 0), &arithmetic)
    .unwrap();
    let expected = arithmetic
        .multiply(
            &arithmetic
                .add(&ExactScalar::one(), &arithmetic.omega(1, 0).unwrap())
                .unwrap(),
            &arithmetic.omega(0, -2).unwrap(),
        )
        .unwrap();
    assert_eq!(
        circuit
            .amplitude(&bits(1, 0), 2, &arithmetic)
            .unwrap()
            .value,
        expected
    );
    assert_eq!(
        circuit
            .probability_z(&[bits(1, 1)], &bits(1, 0), 4, &arithmetic)
            .unwrap()
            .value,
        arithmetic.norm_squared(&expected).unwrap()
    );
    let clifford = UnitaryCircuit::new(
        1,
        [
            G::Clifford(C::H(0)),
            G::Clifford(C::S { qubit: 0, turns: 1 }),
        ]
        .repeat(3),
    )
    .unwrap()
    .compile(&bits(1, 0), &arithmetic)
    .unwrap();
    assert_eq!(clifford.phase_legs(), 0);
    let actual = clifford.amplitude(&bits(1, 0), 1, &arithmetic).unwrap();
    assert_eq!(actual.quotient_rank, 0);
    assert_eq!(actual.terms, 1);
    assert_eq!(actual.value, arithmetic.omega(1, 0).unwrap());
}

#[test]
fn full_ccz_truth_table_and_invalid_queries_have_explicit_outcomes() {
    let arithmetic = ScalarArithmetic::default();
    for input in 0..8 {
        let compiled = UnitaryCircuit::new(3, [G::Ccz { a: 0, b: 1, c: 2 }])
            .unwrap()
            .compile(&bits(3, input), &arithmetic)
            .unwrap();
        for output in 0..8 {
            let expected = if output != input {
                ExactScalar::zero()
            } else if input == 7 {
                ExactScalar::one().negated()
            } else {
                ExactScalar::one()
            };
            assert_eq!(
                compiled
                    .amplitude(&bits(3, output), 1, &arithmetic)
                    .unwrap()
                    .value,
                expected
            );
        }
        let impossible = compiled
            .probability_z(&[bits(3, 1), bits(3, 1)], &bits(2, 1), 1, &arithmetic)
            .unwrap();
        assert_eq!(impossible.value, ExactScalar::zero());
    }
    assert!(matches!(
        UnitaryCircuit::new(1, [G::T { qubit: 0, turns: 8 }]),
        Err(UnitaryError::InvalidTurns)
    ));
    assert!(matches!(
        UnitaryCircuit::new(
            2,
            [G::Cs {
                a: 0,
                b: 1,
                turns: 4
            }]
        ),
        Err(UnitaryError::InvalidTurns)
    ));
    assert!(matches!(
        UnitaryCircuit::new(
            2,
            [G::Cs {
                a: 0,
                b: 0,
                turns: 1
            }]
        ),
        Err(UnitaryError::RepeatedQubit)
    ));
    assert!(matches!(
        UnitaryCircuit::new(2, [G::Ccz { a: 0, b: 1, c: 0 }]),
        Err(UnitaryError::RepeatedQubit)
    ));
    assert!(UnitaryCircuit::new(1, [G::T { qubit: 1, turns: 1 }]).is_err());
    let compiled = UnitaryCircuit::new(1, [G::Clifford(C::H(0)), G::T { qubit: 0, turns: 1 }])
        .unwrap()
        .compile(&bits(1, 0), &arithmetic)
        .unwrap();
    assert!(matches!(
        compiled.amplitude(&bits(2, 0), 4, &arithmetic),
        Err(UnitaryError::DimensionMismatch)
    ));
    assert!(matches!(
        compiled.probability_z(&[bits(2, 0)], &bits(1, 0), 4, &arithmetic),
        Err(UnitaryError::DimensionMismatch)
    ));
    assert!(compiled.amplitude(&bits(1, 0), 0, &arithmetic).is_err());
    let vacuum = UnitaryCircuit::new(0, [])
        .unwrap()
        .compile(&bits(0, 0), &arithmetic)
        .unwrap();
    assert_eq!(
        vacuum.amplitude(&bits(0, 0), 1, &arithmetic).unwrap().value,
        ExactScalar::one()
    );
    assert_eq!(
        vacuum
            .probability_z(&[], &bits(0, 0), 1, &arithmetic)
            .unwrap()
            .value,
        ExactScalar::one()
    );
}

#[test]
fn partial_targets_trace_entangled_environments_and_apply_events_on_both_copies() {
    let arithmetic = ScalarArithmetic::default();
    let gates = [
        G::Clifford(C::H(0)),
        G::Clifford(C::H(2)),
        G::Clifford(C::Cx {
            control: 0,
            target: 1,
        }),
        G::T { qubit: 0, turns: 1 },
        G::Cs {
            a: 1,
            b: 2,
            turns: 1,
        },
        G::Clifford(C::Cx {
            control: 2,
            target: 3,
        }),
        G::Clifford(C::H(1)),
    ];
    let protocol = UnitaryCircuit::new(4, gates)
        .unwrap()
        .compile(&bits(4, 0), &arithmetic)
        .unwrap();
    let values = dense(&gates, 4, 0, &arithmetic);
    for outputs in [vec![], vec![1], vec![3, 1], vec![3, 2, 1, 0]] {
        let target_gates = (0..outputs.len())
            .flat_map(|q| {
                [
                    G::Clifford(C::H(q)),
                    G::T {
                        qubit: q,
                        turns: (2 * q + 1) as u8,
                    },
                ]
            })
            .collect::<Vec<_>>();
        let target = UnitaryCircuit::new(outputs.len(), target_gates.clone())
            .unwrap()
            .compile(&bits(outputs.len(), 0), &arithmetic)
            .unwrap();
        let target_values = dense(&target_gates, outputs.len(), 0, &arithmetic);
        let environment = (0..4).filter(|q| !outputs.contains(q)).collect::<Vec<_>>();
        for row in [bits(4, 0), bits(4, 1), bits(4, 6), bits(4, 15)] {
            for outcome in 0..2 {
                let mut expected = ExactScalar::zero();
                for env in 0..1 << environment.len() {
                    let base = environment
                        .iter()
                        .enumerate()
                        .map(|(i, &q)| ((env >> i) & 1) << q)
                        .sum::<usize>();
                    let mut amplitude = ExactScalar::zero();
                    for (out, target_value) in target_values.iter().enumerate() {
                        let x = base
                            | outputs
                                .iter()
                                .enumerate()
                                .map(|(i, &q)| ((out >> i) & 1) << q)
                                .sum::<usize>();
                        if row.dot(&bits(4, x)).unwrap() != (outcome != 0) {
                            continue;
                        }
                        amplitude = arithmetic
                            .add(
                                &amplitude,
                                &arithmetic
                                    .multiply(&values[x], &target_value.conjugate())
                                    .unwrap(),
                            )
                            .unwrap();
                    }
                    expected = arithmetic
                        .add(&expected, &arithmetic.norm_squared(&amplitude).unwrap())
                        .unwrap();
                }
                let actual = protocol
                    .target_weight(
                        &outputs,
                        &target,
                        std::slice::from_ref(&row),
                        &bits(1, outcome),
                        256,
                        &arithmetic,
                    )
                    .unwrap();
                assert_eq!(
                    actual.value, expected,
                    "outputs={outputs:?}, row={row:?}, outcome={outcome}"
                );
            }
        }
    }
    let target = UnitaryCircuit::new(2, [])
        .unwrap()
        .compile(&bits(2, 0), &arithmetic)
        .unwrap();
    assert!(matches!(
        protocol.target_weight(&[0, 0], &target, &[], &bits(0, 0), 256, &arithmetic),
        Err(UnitaryError::RepeatedOutput)
    ));
    assert!(
        protocol
            .target_weight(&[0, 4], &target, &[], &bits(0, 0), 256, &arithmetic)
            .is_err()
    );
}

#[test]
fn reduced_projected_density_overlaps_match_literal_density_matrices() {
    use rmagna::unitary::DensityBoundary;
    let a = ScalarArithmetic::default();
    let pg = [
        G::Clifford(C::H(0)),
        G::Clifford(C::H(2)),
        G::T { qubit: 0, turns: 1 },
        G::Clifford(C::Cx {
            control: 0,
            target: 1,
        }),
        G::Cs {
            a: 1,
            b: 2,
            turns: 1,
        },
        G::Clifford(C::H(1)),
        G::Clifford(C::Cx {
            control: 2,
            target: 3,
        }),
        G::T { qubit: 3, turns: 3 },
    ];
    let tg = [
        G::Clifford(C::H(1)),
        G::Clifford(C::H(2)),
        G::T { qubit: 1, turns: 1 },
        G::Clifford(C::Cx {
            control: 1,
            target: 0,
        }),
        G::Cs {
            a: 0,
            b: 2,
            turns: 3,
        },
        G::Clifford(C::H(0)),
    ];
    let p = UnitaryCircuit::new(4, pg)
        .unwrap()
        .compile(&bits(4, 0), &a)
        .unwrap();
    let t = UnitaryCircuit::new(3, tg)
        .unwrap()
        .compile(&bits(3, 0), &a)
        .unwrap();
    let pv = dense(&pg, 4, 0, &a);
    let tv = dense(&tg, 3, 0, &a);
    // Form the full density entries and trace equal environment indices. This
    // oracle uses no boundary ket, phase quotient or Gaussian elimination.
    let pr = bits(4, 9);
    let tr = bits(3, 3);
    for (po, to) in [
        (vec![0], vec![2]),
        (vec![3, 1], vec![0, 2]),
        (vec![], vec![]),
        (vec![2, 0, 1], vec![1, 0, 2]),
    ] {
        let left = rmagna::unitary::DensityLayout {
            outputs: &po,
            event_rows: std::slice::from_ref(&pr),
        };
        let right = rmagna::unitary::DensityLayout {
            outputs: &to,
            event_rows: std::slice::from_ref(&tr),
        };
        let prepared = p.prepare_density_overlap(left, &t, right, &a).unwrap();
        let fixed = p
            .prepare_density_overlap_with_fibers(
                left,
                &t,
                right,
                4096,
                rmagna::fibers::FiberOptions::default(),
                &a,
            )
            .unwrap();
        for flags in 0..4 {
            let pbit = flags & 1 != 0;
            let tbit = flags & 2 != 0;
            let rho = reduced_density(&pv, 4, &po, &pr, pbit, &a);
            let tau = reduced_density(&tv, 3, &to, &tr, tbit, &a);
            let expected = density_product(&rho, &tau, &a);
            let actual = p
                .density_overlap(
                    DensityBoundary {
                        outputs: &po,
                        event_rows: std::slice::from_ref(&pr),
                        outcomes: &bits(1, pbit as usize),
                    },
                    &t,
                    DensityBoundary {
                        outputs: &to,
                        event_rows: std::slice::from_ref(&tr),
                        outcomes: &bits(1, tbit as usize),
                    },
                    256,
                    &a,
                )
                .unwrap();
            assert_eq!(
                actual.value, expected,
                "outputs {po:?}/{to:?} flags {flags}"
            );
            for plan in [&prepared, &fixed] {
                assert_eq!(
                    plan.evaluate(
                        &p,
                        &t,
                        &bits(1, pbit as usize),
                        &bits(1, tbit as usize),
                        4096,
                        &a
                    )
                    .unwrap()
                    .value,
                    expected
                );
            }
        }
    }
    let empty = bits(0, 0);
    let plain = |outputs| DensityBoundary {
        outputs,
        event_rows: &[],
        outcomes: &empty,
    };
    assert!(matches!(
        p.density_overlap(plain(&[0]), &t, plain(&[0, 1]), 256, &a),
        Err(UnitaryError::DimensionMismatch)
    ));
    assert!(matches!(
        p.density_overlap(plain(&[0, 0]), &t, plain(&[0, 1]), 256, &a),
        Err(UnitaryError::RepeatedOutput)
    ));
    assert!(
        p.density_overlap(plain(&[0]), &t, plain(&[3]), 256, &a)
            .is_err()
    );
}

#[test]
fn pauli_moments_match_literal_matrices_under_projected_events() {
    use rmagna::pauli::PauliWord;
    let a = ScalarArithmetic::default();
    let gates = [
        G::Clifford(C::H(0)),
        G::Clifford(C::H(1)),
        G::T { qubit: 0, turns: 1 },
        G::Cs {
            a: 0,
            b: 1,
            turns: 1,
        },
        G::Clifford(C::Cx {
            control: 1,
            target: 2,
        }),
        G::Clifford(C::H(2)),
        G::T { qubit: 2, turns: 3 },
        G::Ccz { a: 0, b: 1, c: 2 },
        G::Clifford(C::H(1)),
    ];
    let compiled = UnitaryCircuit::new(3, gates)
        .unwrap()
        .compile(&bits(3, 0), &a)
        .unwrap();
    let values = dense(&gates, 3, 0, &a);
    for xmask in 0_usize..8 {
        for zmask in 0_usize..8 {
            for extra in 0..2 {
                let phase = ((2 * (xmask & zmask).count_ones()) as u8 + extra) & 7;
                let word = PauliWord::new(bits(3, xmask), bits(3, zmask), phase).unwrap();
                for mask in [0_usize, 1, 3] {
                    for outcome in 0..2 {
                        let mut expected = ExactScalar::zero();
                        for input in 0_usize..8 {
                            let output = input ^ xmask;
                            if (input & mask).count_ones() % 2 != outcome
                                || (output & mask).count_ones() % 2 != outcome
                            {
                                continue;
                            }
                            let phase =
                                i64::from(phase) + 4 * i64::from((input & zmask).count_ones() % 2);
                            let term = a
                                .multiply(
                                    &a.multiply(&values[output].conjugate(), &values[input])
                                        .unwrap(),
                                    &a.omega(phase, 0).unwrap(),
                                )
                                .unwrap();
                            expected = a.add(&expected, &term).unwrap();
                        }
                        let actual = compiled
                            .pauli_moment(
                                &word,
                                &[bits(3, mask)],
                                &bits(1, outcome as usize),
                                256,
                                &a,
                            )
                            .unwrap();
                        assert_eq!(
                            actual.value, expected,
                            "X{xmask} Z{zmask} phase{phase} mask{mask} outcome{outcome}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn closed_unitary_queries_use_certified_clifford_fibers_before_the_term_budget() {
    let a = ScalarArithmetic::default();
    let gates = (0..4)
        .map(|qubit| G::Clifford(C::H(qubit)))
        .chain((0..4).map(|qubit| G::T { qubit, turns: 1 }))
        .chain((0..4).map(|qubit| G::Clifford(C::H(qubit))))
        .collect::<Vec<_>>();
    let circuit = UnitaryCircuit::new(4, gates.clone())
        .unwrap()
        .compile(&bits(4, 0), &a)
        .unwrap();
    let expected = dense(&gates, 4, 0, &a);
    for (x, value) in expected.iter().enumerate() {
        let query = circuit.amplitude(&bits(4, x), 4, &a).unwrap();
        assert_eq!(&query.value, value);
        assert_eq!(query.quotient_rank, 4);
        assert_eq!(query.terms, 4);
    }
    assert!(circuit.amplitude(&bits(4, 0), 3, &a).is_err());
}

#[test]
fn final_scalar_closure_cancels_hidden_paths_before_resource_budgeting() {
    let arithmetic = ScalarArithmetic::default();
    let mut gates = (0..3).map(|q| G::Clifford(C::H(q))).collect::<Vec<_>>();
    gates.extend([
        G::Ccz { a: 0, b: 1, c: 2 },
        G::Ccz { a: 0, b: 1, c: 2 },
        G::T { qubit: 1, turns: 1 },
        G::T { qubit: 1, turns: 7 },
    ]);
    gates.extend((0..3).map(|q| G::Clifford(C::H(q))));
    gates.push(G::Clifford(C::X(2)));
    gates.extend(
        [
            G::Clifford(C::H(1)),
            G::Clifford(C::S { qubit: 1, turns: 1 }),
        ]
        .repeat(3),
    );
    let circuit = UnitaryCircuit::new(3, gates.clone())
        .unwrap()
        .compile(&bits(3, 0), &arithmetic)
        .unwrap();
    let literal = dense(&gates, 3, 0, &arithmetic);
    assert_eq!(literal[4], arithmetic.omega(1, 0).unwrap());
    assert!(
        literal
            .iter()
            .enumerate()
            .all(|(x, a)| x == 4 || a.is_zero())
    );
    for (x, expected) in literal.iter().enumerate() {
        let actual = circuit.amplitude(&bits(3, x), 1, &arithmetic).unwrap();
        assert_eq!(&actual.value, expected);
        assert!(actual.terms <= 1);
    }
    let target = UnitaryCircuit::new(3, [])
        .unwrap()
        .compile(&bits(3, 4), &arithmetic)
        .unwrap();
    assert_eq!(
        circuit.overlap(&target, 1, &arithmetic).unwrap().value,
        arithmetic.omega(7, 0).unwrap()
    );
    let rows = vec![bits(3, 4); 65];
    let one = BitVector::from_indices(65, 0..65).unwrap();
    assert_eq!(
        circuit
            .probability_z(&rows, &one, 1, &arithmetic)
            .unwrap()
            .value,
        ExactScalar::one()
    );
    assert_eq!(
        circuit
            .target_weight(&[0, 1, 2], &target, &rows, &one, 1, &arithmetic)
            .unwrap()
            .value,
        ExactScalar::one()
    );
    let z = PauliWord::new(bits(3, 0), bits(3, 4), 0).unwrap();
    assert_eq!(
        circuit
            .pauli_moment(&z, &rows, &one, 1, &arithmetic)
            .unwrap()
            .value,
        ExactScalar::one().negated()
    );
    let mut impossible = one.clone();
    impossible.set(64, false).unwrap();
    let zero = circuit
        .probability_z(&rows, &impossible, 0, &arithmetic)
        .unwrap();
    assert_eq!(zero.value, ExactScalar::zero());
    assert_eq!(zero.terms, 0);
}

#[test]
fn prepared_probability_plans_match_literal_faulted_noncommuting_circuits() {
    let arithmetic = ScalarArithmetic::default();
    let gates = [
        G::Clifford(C::H(0)),
        G::Clifford(C::H(2)),
        G::T { qubit: 0, turns: 1 },
        G::Clifford(C::H(0)),
        G::Cs {
            a: 0,
            b: 2,
            turns: 3,
        },
        G::Ccz { a: 0, b: 1, c: 2 },
        G::Clifford(C::Cx {
            control: 2,
            target: 1,
        }),
        G::Clifford(C::H(1)),
        G::T { qubit: 2, turns: 5 },
    ];
    let base = UnitaryCircuit::new(3, gates)
        .unwrap()
        .compile(&bits(3, 1), &arithmetic)
        .unwrap();
    let rows = [bits(3, 3), bits(3, 6), bits(3, 5)];
    let seed_view = base
        .with_insertions(
            &[PauliInsertion {
                before_gate: 0,
                word: PauliWord::new(bits(3, 1), bits(3, 4), 3).unwrap(),
            }],
            &arithmetic,
        )
        .unwrap();
    let prepared = seed_view.prepare_probability(&rows, &arithmetic).unwrap();
    let norm = base.prepare_probability(&[], &arithmetic).unwrap();
    let mut rng = 0x85e1bad35;
    for _ in 0..32 {
        let faults = (0..4)
            .map(|_| PauliInsertion {
                before_gate: next(&mut rng) % (gates.len() + 1),
                word: PauliWord::new(
                    bits(3, next(&mut rng)),
                    bits(3, next(&mut rng)),
                    (next(&mut rng) & 7) as u8,
                )
                .unwrap(),
            })
            .collect::<Vec<_>>();
        let state = seed_view.with_insertions(&faults, &arithmetic).unwrap();
        let expected = dense_inserted(&gates, 3, 1, &faults, &arithmetic);
        assert_eq!(
            norm.evaluate(&state, &bits(0, 0), 256, &arithmetic)
                .unwrap()
                .value,
            ExactScalar::one()
        );
        for outcome in 0..8 {
            let mut mass = ExactScalar::zero();
            for (x, value) in expected.iter().enumerate() {
                if [3_usize, 6, 5]
                    .iter()
                    .enumerate()
                    .all(|(j, &r)| ((x & r).count_ones() as usize & 1) == ((outcome >> j) & 1))
                {
                    mass = arithmetic
                        .add(&mass, &arithmetic.norm_squared(value).unwrap())
                        .unwrap();
                }
            }
            let actual = prepared
                .evaluate(&state, &bits(3, outcome), 256, &arithmetic)
                .unwrap();
            assert_eq!(actual.value, mass);
            let ordinary = state
                .probability_z(&rows, &bits(3, outcome), 256, &arithmetic)
                .unwrap();
            assert_eq!(actual.quotient_rank, ordinary.quotient_rank);
            assert_eq!(actual.terms, ordinary.terms);
            if actual.terms > 0 {
                assert!(
                    prepared
                        .evaluate(&state, &bits(3, outcome), actual.terms - 1, &arithmetic)
                        .is_err()
                );
            }
        }
    }
    let unrelated = UnitaryCircuit::new(3, gates)
        .unwrap()
        .compile(&bits(3, 1), &arithmetic)
        .unwrap();
    assert!(matches!(
        prepared.evaluate(&unrelated, &bits(3, 0), 256, &arithmetic),
        Err(UnitaryError::IncompatibleProgram)
    ));
    assert!(matches!(
        prepared.evaluate(&base, &bits(2, 0), 256, &arithmetic),
        Err(UnitaryError::DimensionMismatch)
    ));
    assert!(matches!(
        base.prepare_probability(&[bits(4, 0)], &arithmetic),
        Err(UnitaryError::DimensionMismatch)
    ));
    assert_eq!(
        prepared
            .evaluate(&base, &bits(3, 1), 0, &arithmetic)
            .unwrap()
            .value,
        ExactScalar::zero()
    );
}

#[test]
fn shared_commuting_layers_preserve_internal_fault_signs_and_prepared_queries() {
    use rmagna::unitary::CompilerStrategy;
    let a = ScalarArithmetic::default();
    let gates = [
        G::Clifford(C::H(0)),
        G::Clifford(C::H(1)),
        G::Clifford(C::H(2)),
        G::T { qubit: 0, turns: 1 },
        G::Clifford(C::S { qubit: 1, turns: 1 }),
        G::Clifford(C::H(1)),
        G::T { qubit: 1, turns: 3 },
        G::Clifford(C::X(0)),
        G::T { qubit: 0, turns: 5 },
        G::Cs {
            a: 0,
            b: 1,
            turns: 1,
        },
        G::Ccz { a: 0, b: 1, c: 2 },
        G::T { qubit: 2, turns: 7 },
        G::Clifford(C::H(0)),
        G::T { qubit: 0, turns: 1 },
        G::Clifford(C::H(2)),
        G::T { qubit: 2, turns: 3 },
    ];
    let circuit = UnitaryCircuit::new(3, gates).unwrap();
    let base = circuit
        .compile_with_strategy(&bits(3, 1), &a, CompilerStrategy::SharedCommutingLayer)
        .unwrap();
    let old = circuit.compile(&bits(3, 1), &a).unwrap();
    let diagnostics = base.compilation();
    assert_eq!(diagnostics.strategy, CompilerStrategy::SharedCommutingLayer);
    assert_eq!(diagnostics.reference_layer, Some(0));
    assert_eq!(diagnostics.shared_phase_coordinates, 3);
    assert!(base.phase_legs() < old.phase_legs());
    assert_eq!(diagnostics.coherent_width, 3 + base.phase_legs());
    let rows = [bits(3, 3)];
    let prepared = base.prepare_probability(&rows, &a).unwrap();
    let frozen = base
        .prepare_probability_with_fibers(&rows, 4096, rmagna::fibers::FiberOptions::default(), &a)
        .unwrap();
    assert!(prepared.fixed_phase_resource().is_none());
    let resource = frozen.fixed_phase_resource().unwrap();
    assert!(resource.num_terms() > 0);
    for position in 0..=gates.len() {
        for mask in 0..64 {
            let fault = PauliInsertion {
                before_gate: position,
                word: PauliWord::new(
                    bits(3, mask & 7),
                    bits(3, mask >> 3),
                    ((mask + position) & 7) as u8,
                )
                .unwrap(),
            };
            let state = base
                .with_insertions(std::slice::from_ref(&fault), &a)
                .unwrap();
            let expected = dense_inserted(&gates, 3, 1, std::slice::from_ref(&fault), &a);
            for (x, amplitude) in expected.iter().enumerate() {
                assert_eq!(
                    &state.amplitude(&bits(3, x), 256, &a).unwrap().value,
                    amplitude,
                    "position={position}, mask={mask}, output={x}"
                );
            }
            let mut event = ExactScalar::zero();
            for (x, amplitude) in expected.iter().enumerate() {
                if (x & 3).count_ones() & 1 == 0 {
                    event = a.add(&event, &a.norm_squared(amplitude).unwrap()).unwrap();
                }
            }
            assert_eq!(
                prepared
                    .evaluate(&state, &bits(1, 0), 256, &a)
                    .unwrap()
                    .value,
                event
            );
            assert_eq!(
                frozen
                    .evaluate(&state, &bits(1, 0), 4096, &a)
                    .unwrap()
                    .value,
                event
            );
            if mask == 17 {
                let restored = state.with_insertions(&[], &a).unwrap();
                assert_eq!(
                    restored.overlap(&base, 256, &a).unwrap().value,
                    ExactScalar::one()
                );
            }
        }
    }
    let mut rng = 0x518ebfe;
    for _ in 0..48 {
        let faults = (0..4)
            .map(|_| PauliInsertion {
                before_gate: next(&mut rng) % (gates.len() + 1),
                word: PauliWord::new(
                    bits(3, next(&mut rng)),
                    bits(3, next(&mut rng)),
                    (next(&mut rng) & 7) as u8,
                )
                .unwrap(),
            })
            .collect::<Vec<_>>();
        let state = base.with_insertions(&faults, &a).unwrap();
        let expected = dense_inserted(&gates, 3, 1, &faults, &a);
        for (x, expected) in expected.iter().enumerate() {
            assert_eq!(
                &state.amplitude(&bits(3, x), 256, &a).unwrap().value,
                expected
            );
        }
        let alternate = state.prepare_probability(&rows, &a).unwrap();
        assert_eq!(
            alternate
                .evaluate(&base, &bits(1, 0), 256, &a)
                .unwrap()
                .value,
            prepared
                .evaluate(&base, &bits(1, 0), 256, &a)
                .unwrap()
                .value
        );
    }
}

#[test]
fn shared_layer_ties_and_noncommuting_random_circuits_match_literal_matrices() {
    use rmagna::unitary::CompilerStrategy;
    let a = ScalarArithmetic::default();
    let tied = UnitaryCircuit::new(
        1,
        [
            G::T { qubit: 0, turns: 1 },
            G::Clifford(C::H(0)),
            G::T { qubit: 0, turns: 1 },
            G::Clifford(C::H(0)),
            G::T { qubit: 0, turns: 1 },
        ],
    )
    .unwrap()
    .compile_with_strategy(&bits(1, 0), &a, CompilerStrategy::SharedCommutingLayer)
    .unwrap();
    assert_eq!(tied.compilation().layer_slots, [1, 1, 1]);
    assert_eq!(tied.compilation().reference_layer, Some(0));
    let mut rng = 0x72ab184ed;
    for width in 1..=4 {
        for _ in 0..24 {
            let mut gates = Vec::new();
            for layer in 0..4 {
                let q = next(&mut rng) % width;
                gates.push(G::Clifford(C::H(q)));
                gates.push(G::Clifford(C::S {
                    qubit: q,
                    turns: (next(&mut rng) & 3) as u8,
                }));
                gates.push(G::T {
                    qubit: q,
                    turns: (next(&mut rng) & 7) as u8,
                });
                if width > 1 {
                    let r = (q + 1 + next(&mut rng) % (width - 1)) % width;
                    gates.push(G::Cs {
                        a: q,
                        b: r,
                        turns: (next(&mut rng) & 3) as u8,
                    });
                    gates.push(G::Clifford(match layer % 3 {
                        0 => C::Cx {
                            control: q,
                            target: r,
                        },
                        1 => C::Cz { a: q, b: r },
                        _ => C::Swap { a: q, b: r },
                    }));
                }
                if width > 2 && layer == 1 {
                    gates.push(G::Ccz { a: 0, b: 1, c: 2 });
                }
            }
            let input = next(&mut rng) % (1 << width);
            let circuit = UnitaryCircuit::new(width, gates.clone()).unwrap();
            let compiled = circuit
                .compile_with_strategy(
                    &bits(width, input),
                    &a,
                    CompilerStrategy::SharedCommutingLayer,
                )
                .unwrap();
            let expected = dense(&gates, width, input, &a);
            for (x, value) in expected.iter().enumerate() {
                assert_eq!(
                    &compiled.amplitude(&bits(width, x), 4096, &a).unwrap().value,
                    value
                );
            }
            assert_eq!(
                compiled
                    .probability_z(&[], &bits(0, 0), 4096, &a)
                    .unwrap()
                    .value,
                ExactScalar::one()
            );
        }
    }
    let split_gates = [
        G::T { qubit: 2, turns: 1 },
        G::Clifford(C::H(2)),
        G::Ccz { a: 0, b: 1, c: 2 },
    ];
    let split = UnitaryCircuit::new(3, split_gates).unwrap();
    for input in 0..8 {
        let compiled = split
            .compile_with_strategy(&bits(3, input), &a, CompilerStrategy::SharedCommutingLayer)
            .unwrap();
        assert_eq!(compiled.compilation().layer_slots, [3, 5]);
        assert_eq!(compiled.compilation().reference_layer, Some(1));
        for position in 0..=split_gates.len() {
            let fault = PauliInsertion {
                before_gate: position,
                word: PauliWord::new(bits(3, 7), bits(3, 3), 2).unwrap(),
            };
            let state = compiled
                .with_insertions(std::slice::from_ref(&fault), &a)
                .unwrap();
            let expected = dense_inserted(&split_gates, 3, input, std::slice::from_ref(&fault), &a);
            for (x, expected) in expected.iter().enumerate() {
                assert_eq!(
                    &state.amplitude(&bits(3, x), 256, &a).unwrap().value,
                    expected
                );
            }
        }
    }
    let empty = UnitaryCircuit::new(0, [])
        .unwrap()
        .compile_with_strategy(&bits(0, 0), &a, CompilerStrategy::SharedCommutingLayer)
        .unwrap();
    assert_eq!(empty.compilation().reference_layer, None);
    assert_eq!(
        empty.compilation().strategy,
        CompilerStrategy::SharedCommutingLayer
    );
    assert_eq!(
        empty.amplitude(&bits(0, 0), 1, &a).unwrap().value,
        ExactScalar::one()
    );
}

#[test]
fn fixed_input_observation_relations_keep_fault_dependent_constants() {
    use rmagna::unitary::CompilerStrategy;
    let a = ScalarArithmetic::default();
    let gates = [G::Ccz { a: 0, b: 1, c: 2 }];
    let circuit = UnitaryCircuit::new(3, gates).unwrap();
    for input in 0..8 {
        let base = circuit
            .compile_with_strategy(&bits(3, input), &a, CompilerStrategy::SharedCommutingLayer)
            .unwrap();
        let d = base.compilation();
        assert_eq!(d.shared_observations_before_reduction, 3);
        assert_eq!(d.shared_phase_coordinates, 0);
        assert_eq!(d.coherent_width, 3);
        assert_eq!(d.reference_coherent_width, 6);
        assert_eq!(base.phase_legs(), 0);
        let rows = [bits(3, 3), bits(3, 5)];
        let prepared = base.prepare_probability(&rows, &a).unwrap();
        for position in 0..=1 {
            for mask in 0..64 {
                let fault = PauliInsertion {
                    before_gate: position,
                    word: PauliWord::new(
                        bits(3, mask & 7),
                        bits(3, mask >> 3),
                        ((input + mask + position) & 7) as u8,
                    )
                    .unwrap(),
                };
                let state = base
                    .with_insertions(std::slice::from_ref(&fault), &a)
                    .unwrap();
                let expected = dense_inserted(&gates, 3, input, std::slice::from_ref(&fault), &a);
                for (x, value) in expected.iter().enumerate() {
                    assert_eq!(
                        &state.amplitude(&bits(3, x), 1, &a).unwrap().value,
                        value,
                        "input={input}, position={position}, mask={mask}, output={x}"
                    );
                }
                let output = input ^ (mask & 7);
                let outcome = usize::from((output & 3).count_ones() & 1 != 0)
                    | (usize::from((output & 5).count_ones() & 1 != 0) << 1);
                for observed in 0..4 {
                    assert_eq!(
                        prepared
                            .evaluate(&state, &bits(2, observed), 1, &a)
                            .unwrap()
                            .value,
                        if observed == outcome {
                            ExactScalar::one()
                        } else {
                            ExactScalar::zero()
                        }
                    );
                }
                // Preparation from a perturbed view still describes the original program.
                if mask == 17 {
                    let from_fault = state.prepare_probability(&rows, &a).unwrap();
                    assert_eq!(
                        from_fault
                            .evaluate(
                                &base,
                                &bits(
                                    2,
                                    usize::from((input & 3).count_ones() & 1 != 0)
                                        | (usize::from((input & 5).count_ones() & 1 != 0) << 1)
                                ),
                                1,
                                &a
                            )
                            .unwrap()
                            .value,
                        ExactScalar::one()
                    );
                    for (x, value) in dense(&gates, 3, input, &a).iter().enumerate() {
                        assert_eq!(
                            &state
                                .with_insertions(&[], &a)
                                .unwrap()
                                .amplitude(&bits(3, x), 1, &a)
                                .unwrap()
                                .value,
                            value
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn fixed_input_observation_relations_cross_packed_words_without_losing_paths() {
    use rmagna::unitary::CompilerStrategy;
    let a = ScalarArithmetic::default();
    let width = 129;
    let mut gates = vec![G::Clifford(C::H(0))];
    gates.extend((1..width).map(|target| G::Clifford(C::Cx { control: 0, target })));
    gates.extend((0..width).map(|qubit| G::T { qubit, turns: 1 }));
    let base = UnitaryCircuit::new(width, gates)
        .unwrap()
        .compile_with_strategy(
            &BitVector::zeros(width).unwrap(),
            &a,
            CompilerStrategy::SharedCommutingLayer,
        )
        .unwrap();
    let d = base.compilation();
    assert_eq!(d.shared_observations_before_reduction, width);
    assert_eq!(d.shared_phase_coordinates, 1);
    assert_eq!(d.coherent_width, width + 1);
    assert_eq!(d.reference_coherent_width, 2 * width);
    assert_eq!(d.path_variables, 1);
    let rows = [BitVector::from_indices(width, [0, 64, 128]).unwrap()];
    let prepared = base.prepare_probability(&rows, &a).unwrap();
    for position in [width, width + 65, 2 * width] {
        let fault = PauliInsertion {
            before_gate: position,
            word: PauliWord::new(
                BitVector::from_indices(width, [64]).unwrap(),
                BitVector::from_indices(width, [128]).unwrap(),
                3,
            )
            .unwrap(),
        };
        let state = base.with_insertions(&[fault], &a).unwrap();
        for all in [false, true] {
            let mut output = BitVector::from_indices(width, (0..width).filter(|_| all)).unwrap();
            output.set(64, !all).unwrap();
            // Z128 contributes its pre-flip bit. T64 sees the flipped bit only
            // when the fault precedes that rotation; all other T gates see `all`.
            let t64_after_fault = position <= width + 64;
            let ones = 128 * i64::from(all) + i64::from(all ^ t64_after_fault);
            let expected = a.omega(3 + 4 * i64::from(all) + ones, -1).unwrap();
            assert_eq!(state.amplitude(&output, 2, &a).unwrap().value, expected);
        }
        let impossible = BitVector::from_indices(width, [63, 64]).unwrap();
        assert_eq!(
            state.amplitude(&impossible, 2, &a).unwrap().value,
            ExactScalar::zero()
        );
        for outcome in 0..2 {
            assert_eq!(
                prepared
                    .evaluate(&state, &bits(1, outcome), 4, &a)
                    .unwrap()
                    .value,
                a.omega(0, -2).unwrap()
            );
        }
    }
}

#[test]
fn prepared_probability_traces_accept_fault_flipped_zero_reference_events() {
    use rmagna::unitary::CompilerStrategy;
    let a = ScalarArithmetic::default();
    let gates = [
        G::Clifford(C::X(0)),
        G::Clifford(C::H(1)),
        G::T { qubit: 0, turns: 3 },
        G::T { qubit: 1, turns: 1 },
        G::Clifford(C::Cx {
            control: 1,
            target: 2,
        }),
    ];
    let rows = [bits(3, 1), bits(3, 6)];
    let fault = PauliInsertion {
        before_gate: 0,
        word: PauliWord::new(bits(3, 1), bits(3, 2), 3).unwrap(),
    };
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let base = UnitaryCircuit::new(3, gates)
            .unwrap()
            .compile_with_strategy(&bits(3, 0), &a, strategy)
            .unwrap();
        let perturbed = base
            .with_insertions(std::slice::from_ref(&fault), &a)
            .unwrap();
        let reference_plan = base.prepare_probability(&rows, &a).unwrap();
        let perturbed_plan = perturbed.prepare_probability(&rows, &a).unwrap();
        let fixed = base
            .prepare_probability_with_fibers(
                &rows,
                4096,
                rmagna::fibers::FiberOptions::default(),
                &a,
            )
            .unwrap();
        let fixed_from_fault = perturbed
            .prepare_probability_with_fibers(
                &rows,
                4096,
                rmagna::fibers::FiberOptions::default(),
                &a,
            )
            .unwrap();
        assert_eq!(
            fixed.fixed_phase_resource().unwrap().num_terms(),
            fixed_from_fault.fixed_phase_resource().unwrap().num_terms()
        );
        assert!(
            base.prepare_probability_with_fibers(
                &rows,
                0,
                rmagna::fibers::FiberOptions::default(),
                &a
            )
            .is_err()
        );
        assert!(
            fixed
                .evaluate(
                    &base,
                    &bits(2, 1),
                    fixed.fixed_phase_resource().unwrap().num_terms() - 1,
                    &a
                )
                .is_err()
        );
        let unrelated = UnitaryCircuit::new(3, gates)
            .unwrap()
            .compile_with_strategy(&bits(3, 0), &a, strategy)
            .unwrap();
        assert!(matches!(
            fixed.evaluate(&unrelated, &bits(2, 1), 4096, &a),
            Err(UnitaryError::IncompatibleProgram)
        ));
        for (state, assignments) in [(&base, vec![]), (&perturbed, vec![fault.clone()])] {
            let values = dense_inserted(&gates, 3, 0, &assignments, &a);
            for outcome in 0..4 {
                let mut expected = ExactScalar::zero();
                for (x, amplitude) in values.iter().enumerate() {
                    let actual = (x & 1) | ((((x >> 1) ^ (x >> 2)) & 1) << 1);
                    if actual == outcome {
                        expected = a
                            .add(&expected, &a.norm_squared(amplitude).unwrap())
                            .unwrap();
                    }
                }
                for plan in [&reference_plan, &perturbed_plan, &fixed, &fixed_from_fault] {
                    let actual = plan.evaluate(state, &bits(2, outcome), 4, &a).unwrap();
                    assert_eq!(
                        actual.value, expected,
                        "strategy={strategy:?}, outcome={outcome}"
                    );
                    let cold = state
                        .probability_z(&rows, &bits(2, outcome), 4, &a)
                        .unwrap();
                    if plan.fixed_phase_resource().is_none() {
                        assert_eq!(actual.quotient_rank, cold.quotient_rank);
                        assert_eq!(actual.terms, cold.terms);
                    } else if actual.terms > 0 {
                        assert_eq!(
                            actual.terms,
                            plan.fixed_phase_resource().unwrap().num_terms()
                        );
                    }
                    if expected.is_zero() {
                        assert_eq!(
                            plan.evaluate(state, &bits(2, outcome), 0, &a)
                                .unwrap()
                                .value,
                            ExactScalar::zero()
                        );
                    }
                }
            }
        }
        assert_eq!(
            reference_plan
                .evaluate(&base, &bits(2, 0), 4, &a)
                .unwrap()
                .value,
            ExactScalar::zero()
        );
        assert_eq!(
            reference_plan
                .evaluate(&perturbed, &bits(2, 0), 4, &a)
                .unwrap()
                .value,
            ExactScalar::one()
        );
    }
}

#[test]
fn prepared_density_families_match_faulted_literal_matrices_and_zero_events() {
    use rmagna::unitary::{CompilerStrategy as S, DensityLayout};
    let a = ScalarArithmetic::default();
    let pg = [
        G::Clifford(C::H(0)),
        G::Clifford(C::H(2)),
        G::T { qubit: 0, turns: 1 },
        G::Clifford(C::Cx {
            control: 0,
            target: 1,
        }),
        G::Cs {
            a: 1,
            b: 2,
            turns: 1,
        },
        G::Clifford(C::H(1)),
        G::Ccz { a: 0, b: 2, c: 3 },
        G::T { qubit: 3, turns: 3 },
        G::Clifford(C::X(3)),
    ];
    let tg = [
        G::Clifford(C::H(1)),
        G::Clifford(C::H(2)),
        G::T { qubit: 1, turns: 1 },
        G::Clifford(C::Cx {
            control: 1,
            target: 0,
        }),
        G::Cs {
            a: 0,
            b: 2,
            turns: 3,
        },
        G::Clifford(C::H(0)),
    ];
    let pr = bits(4, 8);
    let tr = bits(3, 5);
    let mut rng = 0x71823dabcf;
    for left_strategy in [S::ParityCopies, S::SharedCommutingLayer] {
        for right_strategy in [S::ParityCopies, S::SharedCommutingLayer] {
            let p = UnitaryCircuit::new(4, pg)
                .unwrap()
                .compile_with_strategy(&bits(4, 0), &a, left_strategy)
                .unwrap();
            let t = UnitaryCircuit::new(3, tg)
                .unwrap()
                .compile_with_strategy(&bits(3, 0), &a, right_strategy)
                .unwrap();
            for (po, to) in [(vec![], vec![]), (vec![3, 1], vec![0, 2])] {
                let left = DensityLayout {
                    outputs: &po,
                    event_rows: std::slice::from_ref(&pr),
                };
                let right = DensityLayout {
                    outputs: &to,
                    event_rows: std::slice::from_ref(&tr),
                };
                let prepared = p.prepare_density_overlap(left, &t, right, &a).unwrap();
                let fixed = p
                    .prepare_density_overlap_with_fibers(
                        left,
                        &t,
                        right,
                        4096,
                        rmagna::fibers::FiberOptions::default(),
                        &a,
                    )
                    .unwrap();
                for trial in 0..12 {
                    let pf = PauliInsertion {
                        before_gate: trial % (pg.len() + 1),
                        word: PauliWord::new(
                            bits(4, next(&mut rng)),
                            bits(4, next(&mut rng)),
                            (next(&mut rng) & 7) as u8,
                        )
                        .unwrap(),
                    };
                    let tf = PauliInsertion {
                        before_gate: trial % (tg.len() + 1),
                        word: PauliWord::new(
                            bits(3, next(&mut rng)),
                            bits(3, next(&mut rng)),
                            (next(&mut rng) & 7) as u8,
                        )
                        .unwrap(),
                    };
                    let ps = p.with_insertions(std::slice::from_ref(&pf), &a).unwrap();
                    let ts = t.with_insertions(std::slice::from_ref(&tf), &a).unwrap();
                    let pv = dense_inserted(&pg, 4, 0, &[pf], &a);
                    let tv = dense_inserted(&tg, 3, 0, &[tf], &a);
                    let from_fault = ps
                        .prepare_density_overlap_with_fibers(
                            left,
                            &ts,
                            right,
                            4096,
                            rmagna::fibers::FiberOptions::default(),
                            &a,
                        )
                        .unwrap();
                    assert_eq!(
                        from_fault.fixed_phase_resource().unwrap().num_terms(),
                        fixed.fixed_phase_resource().unwrap().num_terms()
                    );
                    for flags in 0..4 {
                        let lb = flags & 1 != 0;
                        let rb = flags & 2 != 0;
                        let expected = density_product(
                            &reduced_density(&pv, 4, &po, &pr, lb, &a),
                            &reduced_density(&tv, 3, &to, &tr, rb, &a),
                            &a,
                        );
                        for plan in [&prepared, &fixed, &from_fault] {
                            assert_eq!(
                                plan.evaluate(
                                    &ps,
                                    &ts,
                                    &bits(1, lb as usize),
                                    &bits(1, rb as usize),
                                    4096,
                                    &a
                                )
                                .unwrap()
                                .value,
                                expected,
                                "left={left_strategy:?}, right={right_strategy:?}, trial={trial}, flags={flags}"
                            );
                        }
                    }
                }
                // Reference bit 3 is deterministically one. The zero event is
                // impossible on the reference but possible after a final X3.
                assert_eq!(
                    fixed
                        .evaluate(&p, &t, &bits(1, 0), &bits(1, 0), 0, &a)
                        .unwrap()
                        .value,
                    ExactScalar::zero()
                );
                let flipped = p
                    .with_insertions(
                        &[PauliInsertion {
                            before_gate: pg.len(),
                            word: PauliWord::new(bits(4, 8), bits(4, 0), 0).unwrap(),
                        }],
                        &a,
                    )
                    .unwrap();
                if po.is_empty() {
                    let value = fixed
                        .evaluate(&flipped, &t, &bits(1, 0), &bits(1, 0), 4096, &a)
                        .unwrap()
                        .value;
                    assert!(!value.is_zero());
                    assert!(
                        fixed
                            .evaluate(
                                &flipped,
                                &t,
                                &bits(1, 0),
                                &bits(1, 0),
                                fixed.fixed_phase_resource().unwrap().num_terms() - 1,
                                &a
                            )
                            .is_err()
                    );
                }
                let unrelated = UnitaryCircuit::new(4, pg)
                    .unwrap()
                    .compile_with_strategy(&bits(4, 0), &a, left_strategy)
                    .unwrap();
                assert!(matches!(
                    fixed.evaluate(&unrelated, &t, &bits(1, 1), &bits(1, 0), 4096, &a),
                    Err(UnitaryError::IncompatibleProgram)
                ));
                let unrelated = UnitaryCircuit::new(3, tg)
                    .unwrap()
                    .compile_with_strategy(&bits(3, 0), &a, right_strategy)
                    .unwrap();
                assert!(matches!(
                    fixed.evaluate(&p, &unrelated, &bits(1, 1), &bits(1, 0), 4096, &a),
                    Err(UnitaryError::IncompatibleProgram)
                ));
                assert!(matches!(
                    fixed.evaluate(&p, &t, &bits(0, 0), &bits(1, 0), 4096, &a),
                    Err(UnitaryError::DimensionMismatch)
                ));
                assert!(
                    p.prepare_density_overlap_with_fibers(
                        left,
                        &t,
                        right,
                        0,
                        rmagna::fibers::FiberOptions::default(),
                        &a
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn prepared_density_layouts_reject_invalid_shapes_and_preserve_the_vacuum() {
    use rmagna::unitary::DensityLayout as L;
    let a = ScalarArithmetic::default();
    let p = UnitaryCircuit::new(3, [])
        .unwrap()
        .compile(&bits(3, 0), &a)
        .unwrap();
    let empty = L {
        outputs: &[],
        event_rows: &[],
    };
    assert!(matches!(
        p.prepare_density_overlap(
            L {
                outputs: &[0, 0],
                event_rows: &[]
            },
            &p,
            L {
                outputs: &[1, 2],
                event_rows: &[]
            },
            &a
        ),
        Err(UnitaryError::RepeatedOutput)
    ));
    assert!(matches!(
        p.prepare_density_overlap(
            L {
                outputs: &[0],
                event_rows: &[]
            },
            &p,
            empty,
            &a
        ),
        Err(UnitaryError::DimensionMismatch)
    ));
    assert!(
        p.prepare_density_overlap(
            L {
                outputs: &[3],
                event_rows: &[]
            },
            &p,
            L {
                outputs: &[0],
                event_rows: &[]
            },
            &a
        )
        .is_err()
    );
    assert!(matches!(
        p.prepare_density_overlap(
            L {
                outputs: &[],
                event_rows: &[bits(2, 0)]
            },
            &p,
            empty,
            &a
        ),
        Err(UnitaryError::DimensionMismatch)
    ));
    let vacuum = UnitaryCircuit::new(0, [])
        .unwrap()
        .compile(&bits(0, 0), &a)
        .unwrap();
    let plan = vacuum
        .prepare_density_overlap_with_fibers(
            empty,
            &vacuum,
            empty,
            1,
            rmagna::fibers::FiberOptions::default(),
            &a,
        )
        .unwrap();
    assert_eq!(
        plan.evaluate(&vacuum, &vacuum, &bits(0, 0), &bits(0, 0), 1, &a)
            .unwrap()
            .value,
        ExactScalar::one()
    );
}

#[test]
fn prepared_insertion_suffixes_match_every_three_qubit_pauli_and_dense_amplitudes() {
    use rmagna::unitary::{CompilerStrategy, InsertionPreparationOptions};
    let a = ScalarArithmetic::default();
    let gates = [
        C::H(0),
        C::S { qubit: 1, turns: 0 },
        C::S { qubit: 0, turns: 1 },
        C::Cx {
            control: 0,
            target: 2,
        },
        C::S { qubit: 2, turns: 2 },
        C::Cz { a: 1, b: 2 },
        C::S { qubit: 1, turns: 3 },
        C::X(2),
        C::Z(0),
        C::Swap { a: 0, b: 1 },
        C::H(2),
    ]
    .map(G::Clifford);
    let catalog = (0..=gates.len())
        .flat_map(|position| {
            (0..8).flat_map(move |x| {
                (0..8).map(move |z| PauliInsertion {
                    before_gate: position,
                    word: PauliWord::new(
                        bits(3, x),
                        bits(3, z),
                        ((position + x + 3 * z) % 8) as u8,
                    )
                    .unwrap(),
                })
            })
        })
        .collect::<Vec<_>>();
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let base = UnitaryCircuit::new(3, gates)
            .unwrap()
            .compile_with_strategy(&bits(3, 5), &a, strategy)
            .unwrap();
        let prepared = base
            .prepare_insertions(&catalog, InsertionPreparationOptions::default())
            .unwrap();
        assert_eq!(prepared.len(), catalog.len());
        assert!(!prepared.is_empty());
        for (index, insertion) in catalog.iter().enumerate() {
            let updated = base
                .with_prepared_insertions(&prepared, &[index], &a)
                .unwrap();
            let expected = dense_inserted(&gates, 3, 5, std::slice::from_ref(insertion), &a);
            for (output, value) in expected.iter().enumerate() {
                assert_eq!(
                    &updated.amplitude(&bits(3, output), 1, &a).unwrap().value,
                    value,
                    "{strategy:?} / {index} / {output}"
                );
            }
        }
    }
}

#[test]
fn prepared_mixed_phase_assignments_preserve_order_repetition_and_original_program() {
    use rmagna::unitary::{CompilerStrategy, InsertionPreparationOptions};
    let a = ScalarArithmetic::default();
    let gates = [
        G::Clifford(C::H(0)),
        G::T { qubit: 0, turns: 1 },
        G::Clifford(C::H(1)),
        G::Cs {
            a: 0,
            b: 1,
            turns: 3,
        },
        G::Clifford(C::Cx {
            control: 1,
            target: 2,
        }),
        G::Ccz { a: 0, b: 1, c: 2 },
        G::Clifford(C::H(0)),
        G::T { qubit: 1, turns: 7 },
    ];
    // Entries 1 and 2 share a position and anticommute; supplied order matters.
    let catalog = [
        (8, 3, 5, 7),
        (3, 1, 0, 1),
        (3, 0, 1, 3),
        (0, 6, 3, 2),
        (5, 7, 5, 6),
        (1, 2, 4, 5),
        (4, 3, 3, 0),
        (2, 0, 6, 4),
    ]
    .map(|(before_gate, x, z, phase)| PauliInsertion {
        before_gate,
        word: PauliWord::new(bits(3, x), bits(3, z), phase).unwrap(),
    });
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let base = UnitaryCircuit::new(3, gates)
            .unwrap()
            .compile_with_strategy(&bits(3, 0), &a, strategy)
            .unwrap();
        let changed = base.with_insertions(&catalog[0..2], &a).unwrap();
        let prepared = changed
            .prepare_insertions(&catalog, InsertionPreparationOptions::default())
            .unwrap();
        let assignments: &[&[usize]] = &[
            &[],
            &[0],
            &[1, 2],
            &[2, 1],
            &[1, 1],
            &[4, 3, 4],
            &[7, 2, 7, 0],
        ];
        for indices in assignments {
            let insertions = indices
                .iter()
                .map(|&j| catalog[j].clone())
                .collect::<Vec<_>>();
            let expected = dense_inserted(&gates, 3, 0, &insertions, &a);
            let direct = base.with_insertions(&insertions, &a).unwrap();
            let updated = changed
                .with_prepared_insertions(&prepared, indices, &a)
                .unwrap();
            for (output, value) in expected.iter().enumerate() {
                assert_eq!(
                    &updated.amplitude(&bits(3, output), 256, &a).unwrap().value,
                    value
                );
                assert_eq!(
                    &direct.amplitude(&bits(3, output), 256, &a).unwrap().value,
                    value
                );
            }
        }
        // Reversed equal-position order differs by the exact scalar -1.
        let xz = base
            .with_prepared_insertions(&prepared, &[1, 2], &a)
            .unwrap();
        let zx = base
            .with_prepared_insertions(&prepared, &[2, 1], &a)
            .unwrap();
        for output in 0..8 {
            assert_eq!(
                xz.amplitude(&bits(3, output), 256, &a).unwrap().value,
                zx.amplitude(&bits(3, output), 256, &a)
                    .unwrap()
                    .value
                    .negated()
            );
        }
        let other = UnitaryCircuit::new(3, gates)
            .unwrap()
            .compile_with_strategy(&bits(3, 0), &a, strategy)
            .unwrap();
        assert!(matches!(
            other.with_prepared_insertions(&prepared, &[], &a),
            Err(UnitaryError::IncompatibleProgram)
        ));
        assert!(matches!(
            base.with_prepared_insertions(&prepared, &[8], &a),
            Err(UnitaryError::InvalidPreparedInsertion { index: 8, count: 8 })
        ));
    }
}

#[test]
fn insertion_preparation_budgets_wide_masks_empty_catalogs_and_scalar_vacuum_are_exact() {
    use rmagna::unitary::InsertionPreparationOptions as Options;
    let a = ScalarArithmetic::default();
    let gates = [
        G::Clifford(C::H(0)),
        G::Clifford(C::Cx {
            control: 0,
            target: 64,
        }),
        G::Clifford(C::S {
            qubit: 128,
            turns: 3,
        }),
        G::Clifford(C::Swap { a: 1, b: 128 }),
    ];
    let base = UnitaryCircuit::new(129, gates)
        .unwrap()
        .compile(&BitVector::zeros(129).unwrap(), &a)
        .unwrap();
    let catalog = [
        PauliInsertion {
            before_gate: 0,
            word: PauliWord::new(
                BitVector::from_indices(129, [0, 64, 128]).unwrap(),
                BitVector::from_indices(129, [1, 63, 128]).unwrap(),
                7,
            )
            .unwrap(),
        },
        PauliInsertion {
            before_gate: 4,
            word: PauliWord::identity(129).unwrap(),
        },
    ];
    let prepared = base
        .prepare_insertions(&catalog, Options::default())
        .unwrap();
    let bytes = usize::try_from(prepared.estimated_peak_bytes()).unwrap();
    assert!(
        base.prepare_insertions(
            &catalog,
            Options {
                max_catalog_entries: 2,
                max_table_bytes: bytes
            }
        )
        .is_ok()
    );
    assert!(
        matches!(base.prepare_insertions(&catalog,Options{max_table_bytes:bytes-1,..Default::default()}),Err(UnitaryError::InsertionResourceLimit{option:"max_table_bytes",limit,required}) if limit==bytes-1 && required==bytes as u128)
    );
    assert!(matches!(
        base.prepare_insertions(
            &catalog,
            Options {
                max_catalog_entries: 1,
                ..Default::default()
            }
        ),
        Err(UnitaryError::InsertionResourceLimit {
            option: "max_catalog_entries",
            limit: 1,
            required: 2
        })
    ));
    for chosen in [&[0][..], &[1, 0, 0][..]] {
        let direct = base
            .with_insertions(
                &chosen
                    .iter()
                    .map(|&j| catalog[j].clone())
                    .collect::<Vec<_>>(),
                &a,
            )
            .unwrap();
        let fast = base
            .with_prepared_insertions(&prepared, chosen, &a)
            .unwrap();
        assert_eq!(
            direct.overlap(&fast, 1, &a).unwrap().value,
            ExactScalar::one()
        );
    }
    let empty = base
        .prepare_insertions(
            &[],
            Options {
                max_catalog_entries: 0,
                max_table_bytes: 0,
            },
        )
        .unwrap();
    assert_eq!(empty.estimated_peak_bytes(), 0);
    assert!(empty.is_empty());
    let restored = base
        .with_insertions(&catalog[..1], &a)
        .unwrap()
        .with_prepared_insertions(&empty, &[], &a)
        .unwrap();
    assert_eq!(
        restored.overlap(&base, 1, &a).unwrap().value,
        ExactScalar::one()
    );
    let bad = PauliInsertion {
        before_gate: 5,
        word: PauliWord::identity(129).unwrap(),
    };
    assert!(matches!(
        base.prepare_insertions(&[bad], Options::default()),
        Err(UnitaryError::InvalidInsertion {
            position: 5,
            gate_count: 4
        })
    ));
    let bad = PauliInsertion {
        before_gate: 0,
        word: PauliWord::identity(128).unwrap(),
    };
    assert!(matches!(
        base.prepare_insertions(&[bad], Options::default()),
        Err(UnitaryError::DimensionMismatch)
    ));
    let vacuum = UnitaryCircuit::new(0, [])
        .unwrap()
        .compile(&bits(0, 0), &a)
        .unwrap();
    let scalar = [PauliInsertion {
        before_gate: 0,
        word: PauliWord::new(bits(0, 0), bits(0, 0), 3).unwrap(),
    }];
    let plan = vacuum
        .prepare_insertions(&scalar, Options::default())
        .unwrap();
    assert_eq!(
        vacuum
            .with_prepared_insertions(&plan, &[0, 0, 0], &a)
            .unwrap()
            .amplitude(&bits(0, 0), 1, &a)
            .unwrap()
            .value,
        a.omega(1, 0).unwrap()
    );
}

#[test]
fn prepared_shared_signs_cover_late_snapshot_cuts_and_equal_lowered_boundaries() {
    use rmagna::unitary::{CompilerStrategy, InsertionPreparationOptions};
    let a = ScalarArithmetic::default();
    let gates = [
        G::T { qubit: 2, turns: 1 },
        G::Clifford(C::H(2)),
        G::Ccz { a: 0, b: 1, c: 2 },
        G::T { qubit: 0, turns: 1 },
        G::T { qubit: 1, turns: 3 },
        G::T { qubit: 2, turns: 7 },
        G::Clifford(C::S { qubit: 0, turns: 3 }),
        G::Clifford(C::Cz { a: 0, b: 1 }),
        G::T { qubit: 0, turns: 5 },
        G::Clifford(C::Swap { a: 0, b: 1 }),
        G::Clifford(C::Cx {
            control: 0,
            target: 1,
        }),
        G::T { qubit: 1, turns: 1 },
        G::Clifford(C::H(0)),
        G::Cs {
            a: 0,
            b: 1,
            turns: 3,
        },
    ];
    let base = UnitaryCircuit::new(3, gates)
        .unwrap()
        .compile_with_strategy(&bits(3, 3), &a, CompilerStrategy::SharedCommutingLayer)
        .unwrap();
    assert!(base.compilation().reference_layer.unwrap() > 0);
    let mut catalog = (0..=gates.len())
        .flat_map(|position| {
            [1, 2, 3, 5, 7].into_iter().flat_map(move |x| {
                [0, 1, 4, 7].into_iter().map(move |z| PauliInsertion {
                    before_gate: position,
                    word: PauliWord::new(bits(3, x), bits(3, z), ((position + x + z) % 8) as u8)
                        .unwrap(),
                })
            })
        })
        .collect::<Vec<_>>();
    let mut shuffle = 0x72fa891;
    for j in (1..catalog.len()).rev() {
        let k = next(&mut shuffle) % (j + 1);
        catalog.swap(j, k);
    }
    let plan = base
        .prepare_insertions(&catalog, InsertionPreparationOptions::default())
        .unwrap();
    let bytes = usize::try_from(plan.estimated_peak_bytes()).unwrap();
    assert!(
        base.prepare_insertions(
            &catalog,
            InsertionPreparationOptions {
                max_table_bytes: bytes,
                ..Default::default()
            }
        )
        .is_ok()
    );
    assert!(
        matches!(base.prepare_insertions(&catalog,InsertionPreparationOptions{max_table_bytes:bytes-1,..Default::default()}),Err(UnitaryError::InsertionResourceLimit{required,..}) if required==bytes as u128)
    );
    let row = [bits(3, 1)];
    let frozen = base
        .prepare_probability_with_fibers(&row, 1024, rmagna::fibers::FiberOptions::default(), &a)
        .unwrap();
    let mut cases = (0..catalog.len()).map(|j| vec![j]).collect::<Vec<_>>();
    let mut seed = 0x8155c13;
    cases.extend((0..48).map(|_| {
        (0..4)
            .map(|_| next(&mut seed) % catalog.len())
            .collect::<Vec<_>>()
    }));
    cases.extend([vec![1, 1], vec![25, 4, 25, 4], vec![48, 46, 3, 48]]);
    for selected in cases {
        let insertions = selected
            .iter()
            .map(|&j| catalog[j].clone())
            .collect::<Vec<_>>();
        let expected = dense_inserted(&gates, 3, 3, &insertions, &a);
        let changed = base.with_prepared_insertions(&plan, &selected, &a).unwrap();
        let direct = base.with_insertions(&insertions, &a).unwrap();
        for (output, value) in expected.iter().enumerate() {
            assert_eq!(
                &changed.amplitude(&bits(3, output), 1024, &a).unwrap().value,
                value
            );
            assert_eq!(
                &direct.amplitude(&bits(3, output), 1024, &a).unwrap().value,
                value
            );
        }
        let event = expected
            .iter()
            .enumerate()
            .filter(|(j, _)| j & 1 == 1)
            .try_fold(ExactScalar::zero(), |s, (_, v)| {
                a.add(&s, &a.norm_squared(v).unwrap())
            })
            .unwrap();
        assert_eq!(
            frozen
                .evaluate(&changed, &bits(1, 1), 1024, &a)
                .unwrap()
                .value,
            event
        );
    }
}
