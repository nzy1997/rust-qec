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
