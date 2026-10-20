use rmagna::BigInt;
use rmagna::gf2::{BitVector, LinearFactorization};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::{AffineKet, StabilizerError};

#[test]
fn prescribed_native_contractions_match_at_wide_boundaries() {
    let arithmetic = ScalarArithmetic::default();
    let data: serde_json::Value =
        serde_json::from_str(include_str!("data/stabilizer-reference.json")).unwrap();
    assert_eq!(data["schema"], "rmagna.stabilizer-reference.v1");
    assert_eq!(
        data["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    let vectors = data["vectors"].as_array().unwrap();
    assert_eq!(vectors.len(), 24);
    let parse_bits = |width: usize, value: &serde_json::Value| {
        let big = value.as_str().unwrap().parse::<BigInt>().unwrap();
        let (sign, mut words) = big.to_u64_digits();
        assert_ne!(sign, num_bigint::Sign::Minus);
        assert!(words.len() <= width.div_ceil(64));
        words.resize(width.div_ceil(64), 0);
        BitVector::from_words(width, words).unwrap()
    };
    let check = |actual: &ExactScalar, expected: &serde_json::Value| {
        assert_eq!(actual.power(), expected["power"].as_i64().unwrap());
        for (i, c) in actual.coefficients().iter().enumerate() {
            assert_eq!(c.to_string(), expected["coefficients"][i].as_str().unwrap());
        }
    };
    for vector in vectors {
        let width = vector["width"].as_u64().unwrap() as usize;
        let columns = vector["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| parse_bits(width, v))
            .collect::<Vec<_>>();
        let paths = columns.len();
        let phase = QuadraticForm::new(
            0,
            vector["linear"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect(),
            vector["upper"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| parse_bits(paths, v))
                .collect(),
        )
        .unwrap();
        let coefficient = arithmetic
            .omega(
                vector["phase"].as_i64().unwrap(),
                vector["sqrt2_power"].as_i64().unwrap(),
            )
            .unwrap();
        let ket = AffineKet::new(
            parse_bits(width, &vector["base"]),
            columns,
            phase,
            coefficient,
        )
        .unwrap();
        check(&ket.norm_squared(&arithmetic).unwrap(), &vector["norm"]);
        let expected = vector["amplitudes"].as_array().unwrap();
        let contracted = ket.contract(&arithmetic).unwrap();
        if expected.is_empty() {
            assert!(contracted.is_none());
            continue;
        }
        let contracted = contracted.unwrap();
        assert_eq!(
            LinearFactorization::new(width, contracted.columns())
                .unwrap()
                .rank(),
            contracted.columns().len()
        );
        assert_eq!(expected.len(), 1 << contracted.columns().len());
        let prepared = contracted.prepare().unwrap();
        for row in expected {
            check(
                &prepared
                    .amplitude(&parse_bits(width, &row["output"]), &arithmetic)
                    .unwrap(),
                &row["value"],
            );
        }
    }
}

fn bits(width: usize, mask: u64) -> BitVector {
    BitVector::from_indices(width, (0..width).filter(|&i| mask & (1 << i) != 0)).unwrap()
}

fn literal(ket: &AffineKet, arithmetic: &ScalarArithmetic) -> Vec<ExactScalar> {
    let mut amplitudes = vec![ExactScalar::zero(); 1 << ket.width()];
    let mask = |v: &BitVector| v.words().first().copied().unwrap_or(0);
    for path in 0_u64..1 << ket.columns().len() {
        let mut output = mask(ket.offset());
        let mut phase = u64::from(ket.phase().constant());
        for (j, column) in ket.columns().iter().enumerate() {
            if path & (1 << j) != 0 {
                output ^= mask(column);
                phase += u64::from(ket.phase().linear()[j])
                    + 2 * u64::from((mask(&ket.phase().upper()[j]) & path).count_ones());
            }
        }
        let amplitude = arithmetic
            .multiply(
                ket.coefficient(),
                &arithmetic.omega((2 * (phase % 4)) as i64, 0).unwrap(),
            )
            .unwrap();
        amplitudes[output as usize] = arithmetic
            .add(&amplitudes[output as usize], &amplitude)
            .unwrap();
    }
    amplitudes
}

fn assert_state(
    ket: Option<&AffineKet>,
    width: usize,
    expected: &[ExactScalar],
    arithmetic: &ScalarArithmetic,
) {
    match ket {
        None => assert!(expected.iter().all(ExactScalar::is_zero)),
        Some(ket) => {
            assert_eq!(ket.width(), width);
            let prepared = ket.prepare().unwrap();
            for (i, value) in expected.iter().enumerate() {
                assert_eq!(
                    &prepared
                        .amplitude(&bits(width, i as u64), arithmetic)
                        .unwrap(),
                    value
                );
            }
            assert_eq!(&literal(ket, arithmetic), expected);
            assert_eq!(
                LinearFactorization::new(width, ket.columns())
                    .unwrap()
                    .rank(),
                ket.columns().len()
            );
        }
    }
}

fn random_ket(
    width: usize,
    paths: usize,
    next: &mut impl FnMut() -> u64,
    arithmetic: &ScalarArithmetic,
) -> AffineKet {
    let offset = bits(width, next());
    let columns = (0..paths).map(|_| bits(width, next())).collect();
    let linear = (0..paths).map(|_| (next() % 4) as u8).collect();
    let upper = (0..paths)
        .map(|i| {
            BitVector::from_indices(paths, (i + 1..paths).filter(|_| next() & 1 != 0)).unwrap()
        })
        .collect();
    let phase = QuadraticForm::new((next() % 4) as u8, linear, upper).unwrap();
    let scale = arithmetic
        .omega((next() % 8) as i64, (next() % 9) as i64 - 6)
        .unwrap();
    AffineKet::new(offset, columns, phase, scale).unwrap()
}

fn rng() -> impl FnMut() -> u64 {
    let mut state = 0x98acb735671_u64;
    move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    }
}

#[test]
fn dependent_and_hidden_path_contractions_match_literal_complex_amplitudes() {
    let arithmetic = ScalarArithmetic::default();
    let mut next = rng();
    for width in 0..=5 {
        for paths in 0..=8 {
            for _ in 0..4 {
                let ket = random_ket(width, paths, &mut next, &arithmetic);
                let expected = literal(&ket, &arithmetic);
                let prepared = ket.prepare().unwrap();
                for (i, amplitude) in expected.iter().enumerate() {
                    assert_eq!(
                        prepared
                            .amplitude(&bits(width, i as u64), &arithmetic)
                            .unwrap(),
                        *amplitude
                    );
                }
                let contracted = ket.contract(&arithmetic).unwrap();
                assert_state(contracted.as_ref(), width, &expected, &arithmetic);
                if let Some(contracted) = contracted {
                    let twice = contracted.contract(&arithmetic).unwrap();
                    assert_state(twice.as_ref(), width, &expected, &arithmetic);
                }
            }
        }
    }
}

#[test]
fn overlaps_and_norms_match_summed_dense_products_with_global_phase() {
    let arithmetic = ScalarArithmetic::default();
    let mut next = rng();
    for _ in 0..128 {
        let a = random_ket(3, (next() % 9) as usize, &mut next, &arithmetic);
        let b = random_ket(3, (next() % 9) as usize, &mut next, &arithmetic);
        let da = literal(&a, &arithmetic);
        let db = literal(&b, &arithmetic);
        let mut expected = ExactScalar::zero();
        let mut norm = ExactScalar::zero();
        for (a, b) in da.iter().zip(&db) {
            expected = arithmetic
                .add(&expected, &arithmetic.multiply(&a.conjugate(), b).unwrap())
                .unwrap();
            norm = arithmetic
                .add(&norm, &arithmetic.norm_squared(a).unwrap())
                .unwrap();
        }
        assert_eq!(a.overlap(&b, &arithmetic).unwrap(), expected);
        assert_eq!(b.overlap(&a, &arithmetic).unwrap(), expected.conjugate());
        assert_eq!(a.norm_squared(&arithmetic).unwrap(), norm);
    }
}

#[test]
fn parity_projection_is_exact_and_preserves_unnormalized_mass() {
    let arithmetic = ScalarArithmetic::default();
    let mut next = rng();
    let rows = vec![bits(3, 3), bits(3, 6), bits(3, 5)];
    for _ in 0..32 {
        let ket = random_ket(3, 6, &mut next, &arithmetic);
        let original = literal(&ket, &arithmetic);
        for outcomes in 0_u64..8 {
            let expected = original
                .iter()
                .enumerate()
                .map(|(i, value)| {
                    let accepted = [3_u64, 6, 5].iter().enumerate().all(|(j, &row)| {
                        ((row & i as u64).count_ones() & 1) as u64 == (outcomes >> j) & 1
                    });
                    if accepted {
                        value.clone()
                    } else {
                        ExactScalar::zero()
                    }
                })
                .collect::<Vec<_>>();
            let projected = ket
                .project_z(&rows, &bits(3, outcomes), &arithmetic)
                .unwrap();
            assert_state(projected.as_ref(), 3, &expected, &arithmetic);
        }
    }
    let bell = AffineKet::new(
        bits(2, 0),
        vec![bits(2, 3)],
        QuadraticForm::new(0, vec![0], vec![bits(1, 0)]).unwrap(),
        arithmetic.omega(0, -1).unwrap(),
    )
    .unwrap();
    let projected = bell
        .project_z(&[bits(2, 1)], &bits(1, 0), &arithmetic)
        .unwrap()
        .unwrap();
    assert_eq!(
        projected.norm_squared(&arithmetic).unwrap(),
        arithmetic.omega(0, -2).unwrap()
    );
    assert!(
        bell.project_z(&[bits(2, 1), bits(2, 1)], &bits(2, 1), &arithmetic)
            .unwrap()
            .is_none()
    );
}

#[test]
fn pauli_order_and_tensor_boundary_order_keep_complex_phase() {
    let arithmetic = ScalarArithmetic::default();
    let mut next = rng();
    for _ in 0..16 {
        let a = random_ket(2, 5, &mut next, &arithmetic);
        let original = literal(&a, &arithmetic);
        for x in 0_u64..4 {
            for z in 0_u64..4 {
                let modified = a.pauli(&bits(2, x), &bits(2, z)).unwrap();
                let actual = literal(&modified, &arithmetic);
                for i in 0..4 {
                    let expected = if (i as u64 & z).count_ones() & 1 == 0 {
                        original[i].clone()
                    } else {
                        original[i].negated()
                    };
                    assert_eq!(actual[i ^ x as usize], expected);
                }
            }
        }
        let b = random_ket(3, 4, &mut next, &arithmetic);
        let db = literal(&b, &arithmetic);
        let actual = literal(&a.tensor(&b, &arithmetic).unwrap(), &arithmetic);
        for (i, left) in original.iter().enumerate() {
            for (j, right) in db.iter().enumerate() {
                assert_eq!(
                    actual[i | (j << 2)],
                    arithmetic.multiply(left, right).unwrap()
                );
            }
        }
    }
}

#[test]
fn wide_boundary_and_zero_kets_do_not_inherit_a_fixed_qubit_cap() {
    let arithmetic = ScalarArithmetic::default();
    let width = 4097;
    let offset = BitVector::from_indices(width, [63, 127]).unwrap();
    let a = BitVector::from_indices(width, [0, 64, 128]).unwrap();
    let b = BitVector::from_indices(width, [4096]).unwrap();
    let phase = QuadraticForm::new(3, vec![1, 2], vec![bits(2, 2), bits(2, 0)]).unwrap();
    let ket = AffineKet::new(
        offset.clone(),
        vec![a.clone(), b.clone()],
        phase,
        arithmetic.omega(5, -2).unwrap(),
    )
    .unwrap();
    let prepared = ket.prepare().unwrap();
    for mask in 0..4 {
        let mut output = offset.clone();
        if mask & 1 != 0 {
            output.xor_assign(&a).unwrap();
        }
        if mask & 2 != 0 {
            output.xor_assign(&b).unwrap();
        }
        let phase = 3 + (mask & 1) + 2 * ((mask >> 1) & 1) + 2 * (mask & 1) * ((mask >> 1) & 1);
        assert_eq!(
            prepared.amplitude(&output, &arithmetic).unwrap(),
            arithmetic.omega(5 + 2 * phase, -2).unwrap()
        );
    }
    assert_eq!(
        prepared
            .amplitude(&BitVector::zeros(width).unwrap(), &arithmetic)
            .unwrap(),
        ExactScalar::zero()
    );
    let zero = AffineKet::new(
        bits(1, 0),
        vec![bits(1, 0)],
        QuadraticForm::new(0, vec![2], vec![bits(1, 0)]).unwrap(),
        ExactScalar::one(),
    )
    .unwrap();
    assert!(zero.contract(&arithmetic).unwrap().is_none());
    assert_eq!(zero.norm_squared(&arithmetic).unwrap(), ExactScalar::zero());
}

#[test]
fn malformed_boundary_and_phase_dimensions_are_rejected() {
    let arithmetic = ScalarArithmetic::default();
    let phase = QuadraticForm::new(0, vec![], vec![]).unwrap();
    assert!(matches!(
        AffineKet::new(
            bits(1, 0),
            vec![bits(1, 0)],
            phase.clone(),
            ExactScalar::one()
        ),
        Err(StabilizerError::DimensionMismatch)
    ));
    let ket = AffineKet::new(bits(1, 0), vec![], phase, ExactScalar::one()).unwrap();
    assert!(ket.pauli(&bits(2, 0), &bits(1, 0)).is_err());
    assert!(
        ket.prepare()
            .unwrap()
            .amplitude(&bits(2, 0), &arithmetic)
            .is_err()
    );
    assert!(
        ket.project_z(&[bits(2, 0)], &bits(1, 0), &arithmetic)
            .is_err()
    );
    assert!(
        ket.project_z(&[bits(1, 0)], &bits(2, 0), &arithmetic)
            .is_err()
    );
}
