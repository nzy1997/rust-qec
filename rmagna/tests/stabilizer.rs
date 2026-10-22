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

#[test]
fn packed_parity_pullbacks_match_wide_hidden_path_enumeration() {
    use std::collections::HashMap;
    let arithmetic = ScalarArithmetic::default();
    let mut next = rng();
    for paths in [7, 65] {
        let width = 129;
        let offset =
            BitVector::from_indices(width, (0..width).filter(|_| next() & 1 != 0)).unwrap();
        let mut columns = (0..6)
            .map(|_| {
                BitVector::from_indices(width, (0..width).filter(|_| next() & 1 != 0)).unwrap()
            })
            .collect::<Vec<_>>();
        let mut dependent = columns[0].clone();
        dependent.xor_assign(&columns[2]).unwrap();
        columns.push(dependent);
        columns.resize(paths, BitVector::zeros(width).unwrap());
        let mut linear = vec![0; paths];
        for value in &mut linear[..7] {
            *value = (next() % 4) as u8;
        }
        let upper = (0..paths)
            .map(|i| {
                BitVector::from_indices(paths, (i + 1..7).filter(|_| next() & 1 != 0)).unwrap()
            })
            .collect::<Vec<_>>();
        let phase = QuadraticForm::new(3, linear, upper).unwrap();
        let coefficient = arithmetic.omega(3, -5).unwrap();
        let ket = AffineKet::new(
            offset.clone(),
            columns.clone(),
            phase.clone(),
            coefficient.clone(),
        )
        .unwrap();
        let mut amplitudes: HashMap<BitVector, ExactScalar> = HashMap::new();
        for assignment in 0_u64..128 {
            let mut output = offset.clone();
            let mut power = 3_u64;
            for (j, column) in columns.iter().enumerate().take(7) {
                if assignment >> j & 1 != 0 {
                    output.xor_assign(column).unwrap();
                    power += u64::from(phase.linear()[j]);
                    for k in j + 1..7 {
                        if assignment >> k & 1 != 0 && phase.upper()[j].get(k).unwrap() {
                            power += 2;
                        }
                    }
                }
            }
            let term = arithmetic
                .multiply(
                    &coefficient,
                    &arithmetic
                        .omega((2 * (power % 4)) as i64, 2 * (paths as i64 - 7))
                        .unwrap(),
                )
                .unwrap();
            let entry = amplitudes.entry(output).or_insert_with(ExactScalar::zero);
            *entry = arithmetic.add(entry, &term).unwrap();
        }
        let witness = amplitudes.iter().find(|(_, a)| !a.is_zero()).unwrap().0;
        for dense in [false, true] {
            let pool = if dense {
                (0..4)
                    .map(|_| {
                        BitVector::from_indices(width, (0..width).filter(|_| next() & 1 != 0))
                            .unwrap()
                    })
                    .collect::<Vec<_>>()
            } else {
                [(0, 64), (63, 128), (1, 127), (62, 65)]
                    .into_iter()
                    .map(|(a, b)| BitVector::from_indices(width, [a, b]).unwrap())
                    .collect()
            };
            let parity = |row: &BitVector, output: &BitVector| {
                row.ones().fold(false, |p, j| p ^ output.get(j).unwrap())
            };
            for count in [63, 64, 65, 129] {
                let rows = (0..count).map(|j| pool[j % 4].clone()).collect::<Vec<_>>();
                let outcomes = BitVector::from_indices(
                    count,
                    rows.iter()
                        .enumerate()
                        .filter(|(_, row)| parity(row, witness))
                        .map(|(j, _)| j),
                )
                .unwrap();
                let projected = ket
                    .project_z(&rows, &outcomes, &arithmetic)
                    .unwrap()
                    .unwrap();
                let prepared = projected.prepare().unwrap();
                let mut expected_norm = ExactScalar::zero();
                for (output, amplitude) in &amplitudes {
                    let accepted = rows
                        .iter()
                        .enumerate()
                        .all(|(j, row)| parity(row, output) == outcomes.get(j).unwrap());
                    let expected = if accepted {
                        amplitude.clone()
                    } else {
                        ExactScalar::zero()
                    };
                    assert_eq!(
                        prepared.amplitude(output, &arithmetic).unwrap(),
                        expected,
                        "paths={paths}, rows={count}, dense={dense}"
                    );
                    expected_norm = arithmetic
                        .add(&expected_norm, &arithmetic.norm_squared(&expected).unwrap())
                        .unwrap();
                }
                assert_eq!(projected.norm_squared(&arithmetic).unwrap(), expected_norm);
                let mut impossible = outcomes.clone();
                impossible.set(4, !impossible.get(0).unwrap()).unwrap();
                assert!(
                    ket.project_z(&rows, &impossible, &arithmetic)
                        .unwrap()
                        .is_none()
                );
            }
        }
    }
}

#[test]
fn prepared_z_projections_match_literal_paths_under_compatible_phase_updates() {
    use rmagna::stabilizer::PreparedZProjection;
    let arithmetic = ScalarArithmetic::default();
    let mut next = rng();
    let rows = [bits(3, 3), bits(3, 6), bits(3, 5)];
    for _ in 0..24 {
        let reference = random_ket(3, 6, &mut next, &arithmetic);
        let prepared = PreparedZProjection::new(&reference, &rows).unwrap();
        for update in 0..4 {
            let phase = QuadraticForm::new(
                (next() & 3) as u8,
                reference
                    .phase()
                    .linear()
                    .iter()
                    .map(|&c| (c + 2 * (next() & 1) as u8) & 3)
                    .collect(),
                reference.phase().upper().to_vec(),
            )
            .unwrap();
            let coefficient = if update == 3 {
                ExactScalar::zero()
            } else {
                arithmetic
                    .omega((next() & 7) as i64, (next() & 7) as i64 - 5)
                    .unwrap()
            };
            let input = AffineKet::new(
                bits(3, next()),
                reference.columns().to_vec(),
                phase,
                coefficient,
            )
            .unwrap();
            let amplitudes = literal(&input, &arithmetic);
            for outcomes in 0..8 {
                let expected =
                    amplitudes
                        .iter()
                        .enumerate()
                        .map(|(x, a)| {
                            if [3_usize, 6, 5].iter().enumerate().all(|(j, &r)| {
                                (x & r).count_ones() as usize & 1 == outcomes >> j & 1
                            }) {
                                a.clone()
                            } else {
                                ExactScalar::zero()
                            }
                        })
                        .collect::<Vec<_>>();
                let projected = prepared
                    .project(&input, &bits(3, outcomes as u64))
                    .unwrap()
                    .and_then(|k| k.contract(&arithmetic).unwrap());
                assert_state(projected.as_ref(), 3, &expected, &arithmetic);
            }
        }
        let mut odd = reference.phase().linear().to_vec();
        odd[0] ^= 1;
        let odd = AffineKet::new(
            reference.offset().clone(),
            reference.columns().to_vec(),
            QuadraticForm::new(0, odd, reference.phase().upper().to_vec()).unwrap(),
            ExactScalar::one(),
        )
        .unwrap();
        assert!(matches!(
            prepared.project(&odd, &bits(3, 0)),
            Err(StabilizerError::IncompatibleProjection)
        ));
        let mut upper = reference.phase().upper().to_vec();
        let bit = upper[0].get(1).unwrap();
        upper[0].set(1, !bit).unwrap();
        let cross = AffineKet::new(
            reference.offset().clone(),
            reference.columns().to_vec(),
            QuadraticForm::new(0, reference.phase().linear().to_vec(), upper).unwrap(),
            ExactScalar::one(),
        )
        .unwrap();
        assert!(matches!(
            prepared.project(&cross, &bits(3, 0)),
            Err(StabilizerError::IncompatibleProjection)
        ));
        let mut columns = reference.columns().to_vec();
        let bit = columns[0].get(0).unwrap();
        columns[0].set(0, !bit).unwrap();
        let changed = AffineKet::new(
            reference.offset().clone(),
            columns,
            reference.phase().clone(),
            ExactScalar::one(),
        )
        .unwrap();
        assert!(matches!(
            prepared.project(&changed, &bits(3, 0)),
            Err(StabilizerError::IncompatibleProjection)
        ));
        assert!(matches!(
            prepared.project(&reference, &bits(2, 0)),
            Err(StabilizerError::DimensionMismatch)
        ));
    }
}

#[test]
fn prepared_z_projections_preserve_wide_complex_amplitudes_and_empty_boundaries() {
    use rmagna::stabilizer::PreparedZProjection;
    let arithmetic = ScalarArithmetic::default();
    let width = 129;
    let positions = (0..64).chain([128]).collect::<Vec<_>>();
    let columns = positions
        .iter()
        .map(|&j| BitVector::from_indices(width, [j]).unwrap())
        .collect::<Vec<_>>();
    let phase =
        QuadraticForm::new(0, vec![0; 65], vec![BitVector::zeros(65).unwrap(); 65]).unwrap();
    let reference = AffineKet::new(
        BitVector::zeros(width).unwrap(),
        columns.clone(),
        phase,
        arithmetic.omega(0, -65).unwrap(),
    )
    .unwrap();
    let rows = vec![BitVector::from_indices(width, [0]).unwrap(); 65];
    let prepared = PreparedZProjection::new(&reference, &rows).unwrap();
    let input = AffineKet::new(
        BitVector::from_indices(width, [0, 128]).unwrap(),
        columns,
        QuadraticForm::new(
            3,
            (0..65).map(|j| 2 * (j % 2) as u8).collect(),
            vec![BitVector::zeros(65).unwrap(); 65],
        )
        .unwrap(),
        arithmetic.omega(3, -65).unwrap(),
    )
    .unwrap();
    let outcomes = BitVector::from_indices(65, 0..65).unwrap();
    let projected = prepared.project(&input, &outcomes).unwrap().unwrap();
    assert_eq!(
        projected.norm_squared(&arithmetic).unwrap(),
        arithmetic.omega(0, -2).unwrap()
    );
    let amplitudes = projected.prepare().unwrap();
    for assignment in 0..128_u64 {
        let mut output = input.offset().clone();
        let mut power = 3;
        for (j, &coordinate) in positions.iter().enumerate().skip(1) {
            if assignment >> (j % 7) & 1 != 0 {
                output
                    .set(coordinate, !output.get(coordinate).unwrap())
                    .unwrap();
                power += input.phase().linear()[j] as i64;
            }
        }
        let expected = arithmetic
            .multiply(
                input.coefficient(),
                &arithmetic.omega(2 * power, 0).unwrap(),
            )
            .unwrap();
        assert_eq!(
            amplitudes.amplitude(&output, &arithmetic).unwrap(),
            expected
        );
        output.set(0, false).unwrap();
        assert_eq!(
            amplitudes.amplitude(&output, &arithmetic).unwrap(),
            ExactScalar::zero()
        );
    }
    let mut impossible = outcomes.clone();
    impossible.set(64, false).unwrap();
    assert!(prepared.project(&input, &impossible).unwrap().is_none());
    assert!(PreparedZProjection::new(&reference, &[bits(3, 1)]).is_err());
    let empty = AffineKet::new(
        bits(0, 0),
        vec![],
        QuadraticForm::new(2, vec![], vec![]).unwrap(),
        ExactScalar::one(),
    )
    .unwrap();
    let empty_plan = PreparedZProjection::new(&empty, &[]).unwrap();
    let result = empty_plan.project(&empty, &bits(0, 0)).unwrap().unwrap();
    assert_eq!(
        result
            .prepare()
            .unwrap()
            .amplitude(&bits(0, 0), &arithmetic)
            .unwrap(),
        ExactScalar::one().negated()
    );
}

#[test]
fn prepared_affine_contractions_preserve_literal_updated_path_amplitudes() {
    use rmagna::stabilizer::PreparedAffineContraction;
    let a = ScalarArithmetic::default();
    let mut next = rng();
    for width in 0..=5 {
        for paths in 0..=8 {
            for _ in 0..3 {
                let reference = random_ket(width, paths, &mut next, &a);
                let plan = PreparedAffineContraction::new(&reference).unwrap();
                for update in 0..8 {
                    let phase = QuadraticForm::new(
                        (next() & 3) as u8,
                        reference
                            .phase()
                            .linear()
                            .iter()
                            .map(|&c| (c + 2 * (next() & 1) as u8) & 3)
                            .collect(),
                        reference.phase().upper().to_vec(),
                    )
                    .unwrap();
                    let coefficient = if update == 0 {
                        ExactScalar::zero()
                    } else {
                        a.omega((next() & 7) as i64, -3).unwrap()
                    };
                    let input = AffineKet::new(
                        bits(width, next()),
                        reference.columns().to_vec(),
                        phase,
                        coefficient,
                    )
                    .unwrap();
                    let expected = literal(&input, &a);
                    let actual = plan.contract(&input, &a).unwrap();
                    assert_state(actual.as_ref(), width, &expected, &a);
                    if let Some(ket) = actual {
                        let rows = (0..width)
                            .map(|i| {
                                BitVector::from_indices(
                                    ket.columns().len(),
                                    ket.columns()
                                        .iter()
                                        .enumerate()
                                        .filter_map(|(j, c)| c.get(i).unwrap().then_some(j)),
                                )
                                .unwrap()
                            })
                            .collect::<Vec<_>>();
                        assert_eq!(
                            rmagna::gf2::LinearFactorization::new(ket.columns().len(), &rows)
                                .unwrap()
                                .pivot_columns()
                                .len(),
                            ket.columns().len()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn prepared_affine_contractions_handle_zero_references_and_wide_hidden_paths() {
    use rmagna::stabilizer::{PreparedAffineContraction, StabilizerError};
    let a = ScalarArithmetic::default();
    let width = 129;
    let reference = AffineKet::new(
        BitVector::zeros(width).unwrap(),
        vec![BitVector::zeros(width).unwrap(); 2],
        QuadraticForm::new(3, vec![2, 0], vec![bits(2, 0); 2]).unwrap(),
        ExactScalar::one(),
    )
    .unwrap();
    let plan = PreparedAffineContraction::new(&reference).unwrap();
    assert!(plan.contract(&reference, &a).unwrap().is_none());
    let offset = BitVector::from_indices(width, [64, 128]).unwrap();
    let input = AffineKet::new(
        offset.clone(),
        reference.columns().to_vec(),
        QuadraticForm::new(1, vec![0, 0], vec![bits(2, 0); 2]).unwrap(),
        a.omega(3, -2).unwrap(),
    )
    .unwrap();
    let ket = plan.contract(&input, &a).unwrap().unwrap();
    assert!(ket.columns().is_empty());
    assert_eq!(ket.offset(), &offset);
    assert_eq!(
        ket.prepare().unwrap().amplitude(&offset, &a).unwrap(),
        a.omega(5, 2).unwrap()
    );
    assert_eq!(
        ket.prepare()
            .unwrap()
            .amplitude(&BitVector::zeros(width).unwrap(), &a)
            .unwrap(),
        ExactScalar::zero()
    );
    let zero_coefficient = AffineKet::new(
        reference.offset().clone(),
        reference.columns().to_vec(),
        reference.phase().clone(),
        ExactScalar::zero(),
    )
    .unwrap();
    let zero_plan = PreparedAffineContraction::new(&zero_coefficient).unwrap();
    assert_eq!(zero_plan.contract(&input, &a).unwrap().unwrap(), ket);
    for (columns, phase) in [
        (
            vec![
                BitVector::from_indices(width, [0]).unwrap(),
                BitVector::zeros(width).unwrap(),
            ],
            input.phase().clone(),
        ),
        (
            input.columns().to_vec(),
            QuadraticForm::new(1, vec![1, 0], vec![bits(2, 0); 2]).unwrap(),
        ),
        (
            input.columns().to_vec(),
            QuadraticForm::new(1, vec![0, 0], vec![bits(2, 2), bits(2, 0)]).unwrap(),
        ),
    ] {
        let incompatible =
            AffineKet::new(offset.clone(), columns, phase, ExactScalar::zero()).unwrap();
        assert!(matches!(
            plan.contract(&incompatible, &a),
            Err(StabilizerError::IncompatibleContraction)
        ));
    }
    let paths = 129;
    let mut upper = vec![BitVector::zeros(paths).unwrap(); paths];
    for j in (0..paths - 1).step_by(2) {
        upper[j].set(j + 1, true).unwrap();
    }
    let mut linear = vec![0; paths];
    linear[paths - 1] = 1;
    let reference = AffineKet::new(
        offset.clone(),
        vec![BitVector::zeros(width).unwrap(); paths],
        QuadraticForm::new(2, linear, upper.clone()).unwrap(),
        a.omega(5, -4).unwrap(),
    )
    .unwrap();
    let plan = PreparedAffineContraction::new(&reference).unwrap();
    for shift in 0..4 {
        let mut linear = vec![0; paths];
        for (j, coefficient) in linear.iter_mut().enumerate().take(paths - 1) {
            *coefficient = 2 * u8::from((j + shift) % 3 == 0);
        }
        linear[paths - 1] = if shift & 1 == 0 { 1 } else { 3 };
        let mut phase = 5 + 2 * shift as i64 + if shift & 1 == 0 { 1 } else { -1 };
        for j in (0..paths - 1).step_by(2) {
            phase += 4 * i64::from(linear[j] == 2 && linear[j + 1] == 2);
        }
        let input = AffineKet::new(
            offset.clone(),
            reference.columns().to_vec(),
            QuadraticForm::new(shift as u8, linear, upper.clone()).unwrap(),
            reference.coefficient().clone(),
        )
        .unwrap();
        let actual = plan.contract(&input, &a).unwrap().unwrap();
        assert_eq!(
            actual.prepare().unwrap().amplitude(&offset, &a).unwrap(),
            a.omega(phase, paths as i64 - 4).unwrap()
        );
    }
    let narrow = ScalarArithmetic::new(1).unwrap();
    let input = AffineKet::new(
        bits(3, 0),
        vec![bits(3, 1)],
        QuadraticForm::new(0, vec![1], vec![bits(1, 0)]).unwrap(),
        a.integer(3).unwrap(),
    )
    .unwrap();
    assert!(
        PreparedAffineContraction::new(&input)
            .unwrap()
            .contract(&input, &narrow)
            .is_err()
    );
}
