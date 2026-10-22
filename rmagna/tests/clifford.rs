use rmagna::clifford::{CliffordCircuit, CliffordError, CliffordGate as Gate};
use rmagna::gf2::BitVector;
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;

mod common;
use common::{bits, dense_gate, next};

#[test]
fn random_clifford_circuits_match_literal_complex_matrices() {
    let arithmetic = ScalarArithmetic::default();
    let mut state = 0x72893abcdef;
    for width in 1..=6 {
        for trial in 0..24 {
            let mut gates = Vec::new();
            for _ in 0..48 {
                let q = next(&mut state) % width;
                let r = if width == 1 {
                    q
                } else {
                    (q + 1 + next(&mut state) % (width - 1)) % width
                };
                let selection = next(&mut state) % if width == 1 { 4 } else { 7 };
                gates.push(match selection {
                    0 => Gate::H(q),
                    1 => Gate::S {
                        qubit: q,
                        turns: (next(&mut state) % 4) as u8,
                    },
                    2 => Gate::X(q),
                    3 => Gate::Z(q),
                    4 => Gate::Cx {
                        control: q,
                        target: r,
                    },
                    5 => Gate::Cz { a: q, b: r },
                    _ => Gate::Swap { a: q, b: r },
                });
            }
            let initial = next(&mut state) % (1 << width);
            let coefficient = arithmetic.omega(trial, -2).unwrap();
            let input = AffineKet::new(
                bits(width, initial),
                vec![],
                QuadraticForm::new(0, vec![], vec![]).unwrap(),
                coefficient.clone(),
            )
            .unwrap();
            let circuit = CliffordCircuit::new(width, gates.clone()).unwrap();
            assert_eq!(circuit.width(), width);
            assert_eq!(circuit.gates(), gates);
            let actual = circuit.evolve(&input, &arithmetic).unwrap().unwrap();
            assert!(actual.columns().len() <= width);
            assert_eq!(
                actual.norm_squared(&arithmetic).unwrap(),
                arithmetic.norm_squared(&coefficient).unwrap()
            );
            let mut expected = vec![ExactScalar::zero(); 1 << width];
            expected[initial] = coefficient;
            for gate in gates {
                dense_gate(gate, &mut expected, &arithmetic);
            }
            let prepared = actual.prepare().unwrap();
            for (x, value) in expected.iter().enumerate() {
                assert_eq!(
                    &prepared.amplitude(&bits(width, x), &arithmetic).unwrap(),
                    value,
                    "width={width}, trial={trial}, x={x}"
                );
            }
            let recovered = circuit
                .adjoint()
                .evolve(&actual, &arithmetic)
                .unwrap()
                .unwrap()
                .prepare()
                .unwrap();
            for x in 0..1 << width {
                let expected = if x == initial {
                    input.coefficient().clone()
                } else {
                    ExactScalar::zero()
                };
                assert_eq!(
                    recovered.amplitude(&bits(width, x), &arithmetic).unwrap(),
                    expected
                );
            }
        }
    }
}

#[test]
fn boundary_conjugation_and_linear_sums_match_literal_amplitudes() {
    let arithmetic = ScalarArithmetic::default();
    for mask in 0..8 {
        let input = CliffordCircuit::computational(bits(3, mask)).unwrap();
        let ket = CliffordCircuit::new(
            3,
            [
                Gate::H(0),
                Gate::H(1),
                Gate::H(2),
                Gate::S { qubit: 0, turns: 1 },
                Gate::Cz { a: 0, b: 2 },
            ],
        )
        .unwrap()
        .evolve(&input, &arithmetic)
        .unwrap()
        .unwrap();
        let original = ket.prepare().unwrap();
        let conjugate = ket.conjugate().prepare().unwrap();
        for x in 0..8 {
            assert_eq!(
                conjugate.amplitude(&bits(3, x), &arithmetic).unwrap(),
                original
                    .amplitude(&bits(3, x), &arithmetic)
                    .unwrap()
                    .conjugate()
            );
        }
        for keep in [vec![], vec![2], vec![2, 0], vec![1, 1, 0], vec![2, 1, 0]] {
            let mut expected = vec![ExactScalar::zero(); 1 << keep.len()];
            for x in 0..8 {
                let y = keep
                    .iter()
                    .enumerate()
                    .map(|(j, &i)| ((x >> i) & 1) << j)
                    .sum::<usize>();
                expected[y] = arithmetic
                    .add(
                        &expected[y],
                        &original.amplitude(&bits(3, x), &arithmetic).unwrap(),
                    )
                    .unwrap();
            }
            let actual = ket
                .sum_boundary(&keep, &arithmetic)
                .unwrap()
                .map(|k| k.prepare().unwrap());
            for (y, value) in expected.iter().enumerate() {
                let found = actual
                    .as_ref()
                    .map(|p| p.amplitude(&bits(keep.len(), y), &arithmetic).unwrap())
                    .unwrap_or_else(ExactScalar::zero);
                assert_eq!(&found, value);
            }
        }
        assert!(ket.sum_boundary(&[3], &arithmetic).is_err());
    }
}

#[test]
fn clifford_group_relations_retain_global_eighth_root_phase() {
    let arithmetic = ScalarArithmetic::default();
    let gates = [Gate::H(0), Gate::S { qubit: 0, turns: 1 }].repeat(3);
    for x in 0..2 {
        let initial = CliffordCircuit::computational(bits(1, x)).unwrap();
        let result = CliffordCircuit::new(1, gates.clone())
            .unwrap()
            .evolve(&initial, &arithmetic)
            .unwrap()
            .unwrap();
        let prepared = result.prepare().unwrap();
        assert_eq!(
            prepared.amplitude(&bits(1, x), &arithmetic).unwrap(),
            arithmetic.omega(1, 0).unwrap()
        );
        assert_eq!(
            prepared.amplitude(&bits(1, x ^ 1), &arithmetic).unwrap(),
            ExactScalar::zero()
        );
    }
    let gates = [
        Gate::H(0),
        Gate::H(0),
        Gate::S { qubit: 0, turns: 1 },
        Gate::S { qubit: 0, turns: 3 },
    ];
    let initial = CliffordCircuit::computational(bits(1, 1)).unwrap();
    assert_eq!(
        CliffordCircuit::new(1, gates)
            .unwrap()
            .evolve(&initial, &arithmetic)
            .unwrap()
            .unwrap()
            .prepare()
            .unwrap()
            .amplitude(&bits(1, 1), &arithmetic)
            .unwrap(),
        ExactScalar::one()
    );
}

#[test]
fn bell_projection_and_wide_boundaries_preserve_unnormalized_mass() {
    let arithmetic = ScalarArithmetic::default();
    let width = 4097;
    let initial = CliffordCircuit::computational(BitVector::zeros(width).unwrap()).unwrap();
    let result = CliffordCircuit::new(
        width,
        [
            Gate::H(64),
            Gate::Cx {
                control: 64,
                target: 4096,
            },
            Gate::S {
                qubit: 4096,
                turns: 3,
            },
        ],
    )
    .unwrap()
    .evolve(&initial, &arithmetic)
    .unwrap()
    .unwrap();
    let prepared = result.prepare().unwrap();
    assert_eq!(
        prepared
            .amplitude(&BitVector::zeros(width).unwrap(), &arithmetic)
            .unwrap(),
        arithmetic.omega(0, -1).unwrap()
    );
    assert_eq!(
        prepared
            .amplitude(
                &BitVector::from_indices(width, [64, 4096]).unwrap(),
                &arithmetic
            )
            .unwrap(),
        arithmetic.omega(6, -1).unwrap()
    );
    assert_eq!(
        result.norm_squared(&arithmetic).unwrap(),
        ExactScalar::one()
    );
    let branch = result
        .project_z(
            &[BitVector::from_indices(width, [4096]).unwrap()],
            &bits(1, 1),
            &arithmetic,
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        branch.norm_squared(&arithmetic).unwrap(),
        arithmetic.omega(0, -2).unwrap()
    );
}

#[test]
fn invalid_gates_dimensions_and_zero_input_fail_without_approximation() {
    assert!(matches!(
        CliffordCircuit::new(1, [Gate::H(1)]),
        Err(CliffordError::QubitOutOfBounds { qubit: 1, width: 1 })
    ));
    assert!(matches!(
        CliffordCircuit::new(2, [Gate::S { qubit: 0, turns: 4 }]),
        Err(CliffordError::InvalidTurns)
    ));
    for gate in [
        Gate::Cx {
            control: 0,
            target: 0,
        },
        Gate::Cz { a: 0, b: 0 },
        Gate::Swap { a: 0, b: 0 },
    ] {
        assert!(matches!(
            CliffordCircuit::new(1, [gate]),
            Err(CliffordError::RepeatedQubit { qubit: 0 })
        ));
    }
    let arithmetic = ScalarArithmetic::default();
    let input = CliffordCircuit::computational(bits(1, 0)).unwrap();
    assert!(matches!(
        CliffordCircuit::new(2, [])
            .unwrap()
            .evolve(&input, &arithmetic),
        Err(CliffordError::DimensionMismatch)
    ));
    let zero = AffineKet::new(
        bits(1, 0),
        vec![bits(1, 0)],
        QuadraticForm::new(0, vec![2], vec![bits(1, 0)]).unwrap(),
        ExactScalar::one(),
    )
    .unwrap();
    assert!(
        CliffordCircuit::new(1, [Gate::H(0)])
            .unwrap()
            .evolve(&zero, &arithmetic)
            .unwrap()
            .is_none()
    );
    let vacuum = CliffordCircuit::computational(bits(0, 0)).unwrap();
    assert_eq!(
        CliffordCircuit::new(0, [])
            .unwrap()
            .evolve(&vacuum, &arithmetic)
            .unwrap()
            .unwrap()
            .coefficient(),
        &ExactScalar::one()
    );
}

#[test]
fn hadamards_on_fixed_wide_coordinates_preserve_fourier_signs_and_global_scale() {
    let a = ScalarArithmetic::default();
    let width = 129;
    let initial = AffineKet::new(
        BitVector::from_indices(width, [63, 128]).unwrap(),
        vec![BitVector::from_indices(width, [1, 65]).unwrap()],
        QuadraticForm::new(1, vec![3], vec![BitVector::zeros(1).unwrap()]).unwrap(),
        a.scalar([2.into(), (-3).into(), 5.into(), 7.into()], -5)
            .unwrap(),
    )
    .unwrap();
    let observed = [0, 63, 64, 128];
    let circuit = CliffordCircuit::new(width, observed.into_iter().map(Gate::H)).unwrap();
    let evolved = circuit.evolve(&initial, &a).unwrap().unwrap();
    assert_eq!(evolved.columns().len(), 5);
    let prepared = evolved.prepare().unwrap();
    for support in [false, true] {
        for assignment in 0..16usize {
            let mut value = BitVector::zeros(width).unwrap();
            value.set(1, support).unwrap();
            value.set(65, support).unwrap();
            for (i, &q) in observed.iter().enumerate() {
                value.set(q, assignment >> i & 1 != 0).unwrap();
            }
            let sign = (assignment >> 1 & 1) ^ (assignment >> 3 & 1);
            let factor = a
                .omega(2 + 6 * i64::from(support) + 4 * sign as i64, -4)
                .unwrap();
            let expected = a.multiply(initial.coefficient(), &factor).unwrap();
            assert_eq!(prepared.amplitude(&value, &a).unwrap(), expected);
        }
    }
    let recovered = circuit.adjoint().evolve(&evolved, &a).unwrap().unwrap();
    for support in [false, true] {
        let mut value = initial.offset().clone();
        if support {
            value.xor_assign(&initial.columns()[0]).unwrap();
        }
        assert_eq!(
            recovered.prepare().unwrap().amplitude(&value, &a).unwrap(),
            initial.prepare().unwrap().amplitude(&value, &a).unwrap()
        );
    }
    assert_eq!(recovered.columns().len(), 1);
}

#[test]
fn every_random_clifford_prefix_matches_literal_amplitudes() {
    let a = ScalarArithmetic::default();
    let mut rng = 0x97121facbd;
    for width in [3, 4] {
        for trial in 0..8 {
            let input = next(&mut rng) % (1 << width);
            let mut ket = CliffordCircuit::computational(bits(width, input)).unwrap();
            let mut expected = vec![ExactScalar::zero(); 1 << width];
            expected[input] = ExactScalar::one();
            for step in 0..32 {
                let q = next(&mut rng) % width;
                let r = (q + 1 + next(&mut rng) % (width - 1)) % width;
                let gate = match next(&mut rng) % 7 {
                    0 | 1 => Gate::H(q),
                    2 => Gate::S {
                        qubit: q,
                        turns: (next(&mut rng) & 3) as u8,
                    },
                    3 => Gate::Cx {
                        control: q,
                        target: r,
                    },
                    4 => Gate::Cz { a: q, b: r },
                    5 => Gate::X(q),
                    _ => Gate::Swap { a: q, b: r },
                };
                ket = CliffordCircuit::new(width, [gate])
                    .unwrap()
                    .evolve(&ket, &a)
                    .unwrap()
                    .unwrap();
                dense_gate(gate, &mut expected, &a);
                let prepared = ket.prepare().unwrap();
                for (output, value) in expected.iter().enumerate() {
                    assert_eq!(
                        &prepared.amplitude(&bits(width, output), &a).unwrap(),
                        value,
                        "width={width}, trial={trial}, step={step}, output={output}"
                    );
                }
            }
        }
    }
}

#[test]
fn wide_hadamards_contract_a_dense_hidden_direction_and_keep_complex_fourier_values() {
    let a = ScalarArithmetic::default();
    let width = 129;
    let q = 64;
    let offset = BitVector::from_indices(width, [1, 64, 65, 128]).unwrap();
    // Replacing column 64 of the identity by all ones makes the unique
    // preimage of e64 dense across three packed words.
    let columns = (0..width)
        .map(|j| BitVector::from_indices(width, (0..width).filter(|&i| i == j || j == q)).unwrap())
        .collect();
    let linear = (0..width).map(|j| ((3 * j + 1) & 3) as u8).collect();
    let upper = (0..width)
        .map(|i| {
            BitVector::from_indices(width, (i + 1..width).filter(|&j| (i * 17 + j * 3) % 13 < 3))
                .unwrap()
        })
        .collect();
    let phase = QuadraticForm::new(3, linear, upper).unwrap();
    let coefficient = a.omega(3, -(width as i64)).unwrap();
    let input =
        AffineKet::new(offset.clone(), columns, phase.clone(), coefficient.clone()).unwrap();
    let circuit = CliffordCircuit::new(width, [Gate::H(q)]).unwrap();
    let actual = circuit.evolve(&input, &a).unwrap().unwrap();
    assert!(actual.columns().len() <= width);
    let prepared = actual.prepare().unwrap();
    let mut rng = 0x61359fdab;
    for trial in 0..40 {
        let output = BitVector::from_indices(
            width,
            (0..width).filter(|_| trial == 0 || next(&mut rng) & 1 != 0),
        )
        .unwrap();
        let mut expected = ExactScalar::zero();
        for bit in [false, true] {
            let x64 = bit ^ offset.get(q).unwrap();
            let x = BitVector::from_indices(
                width,
                (0..width).filter(|&j| {
                    if j == q {
                        x64
                    } else {
                        output.get(j).unwrap() ^ offset.get(j).unwrap() ^ x64
                    }
                }),
            )
            .unwrap();
            let exponent = 2 * i64::from(phase.evaluate(&x).unwrap())
                + 4 * i64::from(bit && output.get(q).unwrap());
            expected = a
                .add(
                    &expected,
                    &a.multiply(&coefficient, &a.omega(exponent, -1).unwrap())
                        .unwrap(),
                )
                .unwrap();
        }
        assert_eq!(
            prepared.amplitude(&output, &a).unwrap(),
            expected,
            "trial={trial}"
        );
    }
    let recovered = circuit
        .evolve(&actual, &a)
        .unwrap()
        .unwrap()
        .prepare()
        .unwrap();
    let original = input.prepare().unwrap();
    for indices in [vec![], vec![0, 64, 128], (0..width).collect()] {
        let value = BitVector::from_indices(width, indices).unwrap();
        assert_eq!(
            recovered.amplitude(&value, &a).unwrap(),
            original.amplitude(&value, &a).unwrap()
        );
    }
    // Removing one GHZ row loses no rank: the transformed state has two paths.
    let ghz = AffineKet::new(
        BitVector::zeros(width).unwrap(),
        vec![BitVector::from_indices(width, 0..width).unwrap()],
        QuadraticForm::new(1, vec![1], vec![bits(1, 0)]).unwrap(),
        a.omega(5, -1).unwrap(),
    )
    .unwrap();
    let state = circuit.evolve(&ghz, &a).unwrap().unwrap();
    assert_eq!(state.columns().len(), 2);
    let prepared = state.prepare().unwrap();
    for b in [false, true] {
        for y in [false, true] {
            let output =
                BitVector::from_indices(width, (0..width).filter(|&j| if j == q { y } else { b }))
                    .unwrap();
            assert_eq!(
                prepared.amplitude(&output, &a).unwrap(),
                a.omega(7 + 2 * i64::from(b) + 4 * i64::from(b && y), -2)
                    .unwrap()
            );
        }
    }
}

#[test]
fn hadamards_canonicalize_dependent_nonzero_input_before_using_support_rank() {
    let a = ScalarArithmetic::default();
    let input = AffineKet::new(
        bits(3, 2),
        vec![bits(3, 3), bits(3, 3), bits(3, 0)],
        QuadraticForm::new(3, vec![1, 2, 1], vec![bits(3, 6), bits(3, 4), bits(3, 0)]).unwrap(),
        a.omega(5, -3).unwrap(),
    )
    .unwrap();
    let mut expected = vec![ExactScalar::zero(); 8];
    for mask in 0..8 {
        let label = 2 ^ if (mask & 1 != 0) ^ (mask & 2 != 0) {
            3
        } else {
            0
        };
        let term = a
            .multiply(
                input.coefficient(),
                &a.omega(
                    2 * i64::from(input.phase().evaluate(&bits(3, mask)).unwrap()),
                    0,
                )
                .unwrap(),
            )
            .unwrap();
        expected[label] = a.add(&expected[label], &term).unwrap();
    }
    let gates = [
        Gate::H(0),
        Gate::H(1),
        Gate::S { qubit: 2, turns: 1 },
        Gate::H(2),
        Gate::H(0),
    ];
    for gate in gates {
        dense_gate(gate, &mut expected, &a);
    }
    let state = CliffordCircuit::new(3, gates)
        .unwrap()
        .evolve(&input, &a)
        .unwrap()
        .unwrap()
        .prepare()
        .unwrap();
    for (output, value) in expected.iter().enumerate() {
        assert_eq!(&state.amplitude(&bits(3, output), &a).unwrap(), value);
    }
}

#[test]
fn independent_support_contraction_still_enforces_scalar_budgets() {
    let wide = ScalarArithmetic::default();
    let tight = ScalarArithmetic::new(1).unwrap();
    let input = AffineKet::new(
        bits(3, 0),
        vec![bits(3, 1), bits(3, 2)],
        QuadraticForm::new(3, vec![1, 3], vec![bits(2, 2), bits(2, 0)]).unwrap(),
        wide.integer(3).unwrap(),
    )
    .unwrap();
    assert!(input.contract(&tight).is_err());
    for gates in [vec![], vec![Gate::H(0)], vec![Gate::H(2)]] {
        assert!(
            CliffordCircuit::new(3, gates)
                .unwrap()
                .evolve(&input, &tight)
                .is_err()
        );
    }
    assert_eq!(input.contract(&wide).unwrap().unwrap(), input);
}

#[test]
fn owned_wide_clifford_updates_match_reverse_labels_and_diagonal_phases() {
    let a = ScalarArithmetic::default();
    let width = 129;
    let mut gates = (1..width)
        .map(|target| Gate::Cx { control: 0, target })
        .collect::<Vec<_>>();
    gates.extend(
        (0..width)
            .filter(|&control| control != 64)
            .map(|control| Gate::Cx {
                control,
                target: 64,
            }),
    );
    for q in 0..width {
        gates.push(Gate::S {
            qubit: q,
            turns: (q & 3) as u8,
        });
        gates.push(Gate::Cz {
            a: q,
            b: (q + 65) % width,
        });
        if q % 3 == 0 {
            gates.push(Gate::Z(q));
        }
        if q % 5 == 0 {
            gates.push(Gate::X(q));
        }
        if q % 7 == 0 {
            gates.push(Gate::Swap {
                a: q,
                b: (q + 64) % width,
            });
        }
    }
    let offset = BitVector::from_indices(width, [1, 64, 128]).unwrap();
    let phase = QuadraticForm::new(
        3,
        (0..width).map(|i| (i & 3) as u8).collect(),
        (0..width)
            .map(|i| {
                BitVector::from_indices(width, (i + 1..width).filter(|&j| (i + j) % 17 == 0))
                    .unwrap()
            })
            .collect(),
    )
    .unwrap();
    let input = AffineKet::new(
        offset.clone(),
        (0..width)
            .map(|i| BitVector::from_indices(width, [i]).unwrap())
            .collect(),
        phase,
        a.omega(5, -(width as i64)).unwrap(),
    )
    .unwrap();
    let snapshot = input.clone();
    let actual = CliffordCircuit::new(width, gates.clone())
        .unwrap()
        .evolve(&input, &a)
        .unwrap()
        .unwrap();
    assert_eq!(input, snapshot);
    let prepared = actual.prepare().unwrap();
    let mut rng = 0x1773ab293;
    for trial in 0..64 {
        let output = BitVector::from_indices(
            width,
            (0..width).filter(|_| trial == 0 || next(&mut rng) & 1 != 0),
        )
        .unwrap();
        let mut label = (0..width)
            .map(|j| output.get(j).unwrap())
            .collect::<Vec<_>>();
        let mut exponent = 0_i64;
        for gate in gates.iter().rev() {
            match *gate {
                Gate::X(q) => label[q] = !label[q],
                Gate::Z(q) => exponent += 4 * i64::from(label[q]),
                Gate::S { qubit, turns } => {
                    exponent += 2 * i64::from(turns) * i64::from(label[qubit])
                }
                Gate::Cz { a, b } => exponent += 4 * i64::from(label[a] && label[b]),
                Gate::Cx { control, target } => label[target] ^= label[control],
                Gate::Swap { a, b } => label.swap(a, b),
                Gate::H(_) => unreachable!(),
            }
        }
        let x = BitVector::from_indices(
            width,
            (0..width).filter(|&j| label[j] ^ offset.get(j).unwrap()),
        )
        .unwrap();
        exponent += 2 * i64::from(input.phase().evaluate(&x).unwrap());
        let expected = a
            .multiply(input.coefficient(), &a.omega(exponent, 0).unwrap())
            .unwrap();
        assert_eq!(
            prepared.amplitude(&output, &a).unwrap(),
            expected,
            "trial={trial}"
        );
    }
}
