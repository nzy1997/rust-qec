use rmagna::clifford::CliffordGate as C;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::unitary::{UnitaryCircuit, UnitaryError, UnitaryGate as G};
mod common;
use common::{bits, dense_gate, next};

fn dense(
    gates: &[G],
    width: usize,
    input: usize,
    arithmetic: &ScalarArithmetic,
) -> Vec<ExactScalar> {
    let mut values = vec![ExactScalar::zero(); 1 << width];
    values[input] = ExactScalar::one();
    for &gate in gates {
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
