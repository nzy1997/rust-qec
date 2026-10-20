use rmagna::clifford::{CliffordCircuit, CliffordGate as C};
use rmagna::pauli::{PauliError, PauliWord};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
mod common;
use common::{bits, dense_gate, next};

fn dense_pauli(
    word: &PauliWord,
    values: &[ExactScalar],
    arithmetic: &ScalarArithmetic,
) -> Vec<ExactScalar> {
    let mut result = vec![ExactScalar::zero(); values.len()];
    let flip = word.x().ones().map(|j| 1 << j).sum::<usize>();
    for (x, value) in values.iter().enumerate() {
        let sign = word.z().dot(&bits(word.width(), x)).unwrap();
        result[x ^ flip] = arithmetic
            .multiply(
                value,
                &arithmetic
                    .omega(i64::from(word.phase()) + 4 * i64::from(sign), 0)
                    .unwrap(),
            )
            .unwrap();
    }
    result
}

#[test]
fn clifford_conjugation_matches_literal_complex_matrices_for_every_three_qubit_pauli() {
    let arithmetic = ScalarArithmetic::default();
    let mut gates = Vec::new();
    for q in 0..3 {
        gates.extend([C::H(q), C::X(q), C::Z(q)]);
        gates.extend((0..4).map(|turns| C::S { qubit: q, turns }));
    }
    for a in 0..3 {
        for b in 0..3 {
            if a != b {
                gates.extend([
                    C::Cx {
                        control: a,
                        target: b,
                    },
                    C::Cz { a, b },
                    C::Swap { a, b },
                ]);
            }
        }
    }
    let mut rng = 0x81317abcdef;
    // Also exercise the shared deterministic oracle seed for phase choices.
    for x in 0..8 {
        for z in 0..8 {
            let word = PauliWord::new(bits(3, x), bits(3, z), (next(&mut rng) % 8) as u8).unwrap();
            for &gate in &gates {
                let transported = word.conjugated(gate).unwrap();
                for basis in 0..8 {
                    let mut input = vec![ExactScalar::zero(); 8];
                    input[basis] = ExactScalar::one();
                    let mut left = dense_pauli(&word, &input, &arithmetic);
                    dense_gate(gate, &mut left, &arithmetic);
                    let mut right = input;
                    dense_gate(gate, &mut right, &arithmetic);
                    assert_eq!(
                        left,
                        dense_pauli(&transported, &right, &arithmetic),
                        "{word:?} / {gate:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn ordered_pauli_products_and_hermitian_y_keep_global_phase() {
    let arithmetic = ScalarArithmetic::default();
    for ax in 0..4 {
        for az in 0..4 {
            for bx in 0..4 {
                for bz in 0..4 {
                    let a = PauliWord::new(bits(2, ax), bits(2, az), 3).unwrap();
                    let b = PauliWord::new(bits(2, bx), bits(2, bz), 6).unwrap();
                    let product = a.multiply(&b).unwrap();
                    for basis in 0..4 {
                        let mut input = vec![ExactScalar::zero(); 4];
                        input[basis] = ExactScalar::one();
                        assert_eq!(
                            dense_pauli(&product, &input, &arithmetic),
                            dense_pauli(&a, &dense_pauli(&b, &input, &arithmetic), &arithmetic)
                        );
                    }
                }
            }
        }
    }
    let y = PauliWord::new(bits(1, 1), bits(1, 1), 2).unwrap();
    assert_eq!(y.multiply(&y).unwrap(), PauliWord::identity(1).unwrap());
    let input = CliffordCircuit::computational(bits(1, 0)).unwrap();
    let output = y.apply(&input, &arithmetic).unwrap().prepare().unwrap();
    assert_eq!(
        output.amplitude(&bits(1, 1), &arithmetic).unwrap(),
        arithmetic.omega(2, 0).unwrap()
    );
    assert_eq!(y.conjugated(C::H(0)).unwrap().phase(), 6);
}

#[test]
fn malformed_masks_phases_and_gate_endpoints_fail() {
    assert_eq!(
        PauliWord::new(bits(1, 0), bits(2, 0), 0),
        Err(PauliError::DimensionMismatch)
    );
    assert_eq!(
        PauliWord::new(bits(1, 0), bits(1, 0), 8),
        Err(PauliError::InvalidPhase)
    );
    let one = PauliWord::identity(1).unwrap();
    assert_eq!(
        one.multiply(&PauliWord::identity(2).unwrap()),
        Err(PauliError::DimensionMismatch)
    );
    assert!(one.conjugated(C::H(1)).is_err());
    assert!(
        one.apply(
            &CliffordCircuit::computational(bits(2, 0)).unwrap(),
            &ScalarArithmetic::default()
        )
        .is_err()
    );
}
