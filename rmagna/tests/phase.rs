use rmagna::BigInt;
use rmagna::gf2::{BitVector, LinearFactorization};
use rmagna::phase::{ClosedPhaseSum, ParityPhase, PhaseError, PreparedPhaseSum};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use serde_json::Value;

fn bits(width: usize, value: u64) -> BitVector {
    BitVector::from_indices(width, (0..width.min(64)).filter(|&j| value >> j & 1 != 0)).unwrap()
}

fn zero_boundary(width: usize) -> QuadraticForm {
    QuadraticForm::new(
        0,
        vec![0; width],
        vec![BitVector::zeros(width).unwrap(); width],
    )
    .unwrap()
}

fn third_difference(phase: &ParityPhase, u: u64, v: u64, w: u64, start: u64) -> u8 {
    let mut result = 0_i32;
    for subset in 0_u32..8 {
        let x = start
            ^ if subset & 1 != 0 { u } else { 0 }
            ^ if subset & 2 != 0 { v } else { 0 }
            ^ if subset & 4 != 0 { w } else { 0 };
        let value = i32::from(phase.evaluate(&bits(phase.num_variables(), x)).unwrap());
        result += if subset.count_ones() % 2 == 1 {
            value
        } else {
            -value
        };
    }
    result.rem_euclid(8) as u8
}

#[test]
fn every_signature_up_to_three_coordinates_has_the_exact_radical() {
    for width in 0..=3 {
        let size = 1 << width;
        for axes in 0..1 << (size - 1) {
            let phase = ParityPhase::new(
                width,
                5,
                (1..size)
                    .filter(|a| axes >> (a - 1) & 1 != 0)
                    .map(|a| (bits(width, a), 1)),
            )
            .unwrap();
            let quotient = phase.quotient().unwrap();
            assert_eq!(quotient.num_variables(), width);
            assert_eq!(quotient.basis().len(), width);
            assert_eq!(
                LinearFactorization::new(width, quotient.basis())
                    .unwrap()
                    .rank(),
                width
            );
            let mut radical_size = 0;
            for u in 0..size {
                let mut is_radical = true;
                for v in 0..size {
                    for w in 0..size {
                        let sigma = phase
                            .signature(&bits(width, u), &bits(width, v), &bits(width, w))
                            .unwrap();
                        assert_eq!(
                            third_difference(&phase, u, v, w, (u + v) % size),
                            4 * u8::from(sigma)
                        );
                        is_radical &= !sigma;
                    }
                }
                radical_size += usize::from(is_radical);
            }
            assert_eq!(radical_size, 1 << quotient.radical().len());
            for u in quotient.radical() {
                for v in 0..width {
                    for w in 0..width {
                        assert!(
                            !phase
                                .signature(u, &bits(width, 1 << v), &bits(width, 1 << w))
                                .unwrap()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn t_cs_ccz_include_repeated_indices_and_preserve_their_truth_tables() {
    let t = ParityPhase::new(1, 0, [(bits(1, 1), 1)]).unwrap();
    assert_eq!(t.quotient().unwrap().rank(), 1);
    assert!(t.signature(&bits(1, 1), &bits(1, 1), &bits(1, 1)).unwrap());
    let cs = ParityPhase::new(2, 0, [(bits(2, 1), 1), (bits(2, 2), 1), (bits(2, 3), 7)]).unwrap();
    assert_eq!(cs.quotient().unwrap().rank(), 2);
    assert!(cs.signature(&bits(2, 1), &bits(2, 1), &bits(2, 2)).unwrap());
    assert!(!cs.signature(&bits(2, 1), &bits(2, 1), &bits(2, 1)).unwrap());
    let ccz = ParityPhase::new(
        3,
        0,
        (1_u64..8).map(|a| (bits(3, a), if a.count_ones() % 2 == 1 { 1 } else { 7 })),
    )
    .unwrap();
    assert_eq!(ccz.quotient().unwrap().rank(), 3);
    assert!(
        ccz.signature(&bits(3, 1), &bits(3, 2), &bits(3, 4))
            .unwrap()
    );
    assert!(
        !ccz.signature(&bits(3, 1), &bits(3, 1), &bits(3, 2))
            .unwrap()
    );
    for x in 0..8 {
        assert_eq!(t.evaluate(&bits(1, x & 1)).unwrap(), (x & 1) as u8);
        assert_eq!(
            cs.evaluate(&bits(2, x & 3)).unwrap(),
            if x & 3 == 3 { 2 } else { 0 }
        );
        assert_eq!(
            ccz.evaluate(&bits(3, x)).unwrap(),
            if x == 7 { 4 } else { 0 }
        );
    }
}

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

#[test]
fn dependent_affine_maps_preserve_phase_and_affine_shifts_preserve_signature() {
    let mut state = 0x3278abcdef_u64;
    for width in 0..=7 {
        for _ in 0..20 {
            let phase = ParityPhase::new(
                width,
                (next(&mut state) % 8) as u8,
                (0..20).map(|_| (bits(width, next(&mut state)), (next(&mut state) % 8) as u8)),
            )
            .unwrap();
            let offset = bits(width, next(&mut state));
            let basis = (0..5)
                .map(|_| bits(width, next(&mut state)))
                .collect::<Vec<_>>();
            let restricted = phase.restrict(&offset, &basis).unwrap();
            let unshifted = phase.restrict(&bits(width, 0), &basis).unwrap();
            assert_eq!(
                restricted
                    .subtract(&unshifted)
                    .unwrap()
                    .quotient()
                    .unwrap()
                    .rank(),
                0
            );
            for x in 0..32 {
                let mut image = offset.clone();
                for j in bits(5, x).ones() {
                    image.xor_assign(&basis[j]).unwrap();
                }
                assert_eq!(
                    restricted.evaluate(&bits(5, x)).unwrap(),
                    phase.evaluate(&image).unwrap()
                );
            }
        }
    }
}

#[test]
fn clifford_extraction_handles_odd_parity_cancellation_and_global_phase() {
    let vanished = ParityPhase::new(4, 7, (1..16).map(|a| (bits(4, a), 1))).unwrap();
    assert!(vanished.terms().iter().all(|(_, c)| c & 1 != 0));
    let (global, quadratic) = vanished.clifford().unwrap();
    assert_eq!(global, 7);
    for x in 0..16 {
        assert_eq!(quadratic.evaluate(&bits(4, x)).unwrap(), 0);
    }
    let mut state = 0xa1378bdeff;
    for width in 0..=8 {
        for _ in 0..30 {
            let phase = ParityPhase::new(
                width,
                (next(&mut state) % 8) as u8,
                (0..20).map(|_| {
                    (
                        bits(width, next(&mut state)),
                        2 * (next(&mut state) % 4) as u8,
                    )
                }),
            )
            .unwrap();
            let (global, q) = phase.clifford().unwrap();
            for x in 0..1 << width {
                assert_eq!(
                    phase.evaluate(&bits(width, x)).unwrap(),
                    (global + 2 * q.evaluate(&bits(width, x)).unwrap()) & 7
                );
            }
        }
    }
}

fn direct_sum(
    phase: &ParityPhase,
    boundary: &QuadraticForm,
    arithmetic: &ScalarArithmetic,
) -> ExactScalar {
    let mut sum = ExactScalar::zero();
    for x in 0..1 << phase.num_variables() {
        let v = bits(phase.num_variables(), x);
        let p =
            i64::from(phase.evaluate(&v).unwrap()) + 2 * i64::from(boundary.evaluate(&v).unwrap());
        sum = arithmetic
            .add(&sum, &arithmetic.omega(p, 0).unwrap())
            .unwrap();
    }
    sum
}

#[test]
fn prepared_quotients_match_literal_sums_across_clifford_and_pauli_updates() {
    let arithmetic = ScalarArithmetic::default();
    let mut state = 0x78129abcde;
    for width in 0..=8 {
        for _ in 0..24 {
            // Dependent axes ensure both a genuine quotient and a nonempty
            // radical occur; additional even terms change Clifford corrections.
            let a = bits(width, next(&mut state));
            let b = bits(width, next(&mut state));
            let mut ab = a.clone();
            ab.xor_assign(&b).unwrap();
            let mut terms = vec![(a, 1), (b, 3), (ab, 7)];
            terms.extend((0..8).map(|_| {
                (
                    bits(width, next(&mut state)),
                    2 * (next(&mut state) % 4) as u8,
                )
            }));
            let reference = ParityPhase::new(width, (next(&mut state) % 8) as u8, terms).unwrap();
            let prepared = PreparedPhaseSum::new(&reference, 256, &arithmetic).unwrap();
            assert_eq!(prepared.num_terms(), 1 << prepared.quotient().rank());
            let identities = (0..width).map(|j| bits(width, 1 << j)).collect::<Vec<_>>();
            let mut upper = vec![bits(width, 0); width];
            for (i, row) in upper.iter_mut().enumerate() {
                for j in i + 1..width {
                    row.set(j, next(&mut state) & 1 != 0).unwrap();
                }
            }
            let boundary = QuadraticForm::new(
                (next(&mut state) % 4) as u8,
                (0..width).map(|_| (next(&mut state) % 4) as u8).collect(),
                upper,
            )
            .unwrap();
            let coefficient = arithmetic.omega(3, -3).unwrap();
            for _ in 0..5 {
                let shifted = reference
                    .restrict(&bits(width, next(&mut state)), &identities)
                    .unwrap();
                let extra = ParityPhase::new(
                    width,
                    (next(&mut state) % 8) as u8,
                    (0..4).map(|_| {
                        (
                            bits(width, next(&mut state)),
                            2 * (next(&mut state) % 4) as u8,
                        )
                    }),
                )
                .unwrap();
                let updated = shifted.subtract(&extra).unwrap();
                assert_eq!(
                    prepared
                        .evaluate(&updated, &boundary, &coefficient, &arithmetic)
                        .unwrap(),
                    arithmetic
                        .multiply(&coefficient, &direct_sum(&updated, &boundary, &arithmetic))
                        .unwrap()
                );
            }
        }
    }
}

#[test]
fn wide_low_rank_resource_keeps_gaussian_scale_without_physical_enumeration() {
    let arithmetic = ScalarArithmetic::default();
    for width in [65, 129] {
        let phase = ParityPhase::new(
            width,
            3,
            [(
                BitVector::from_indices(width, [0, 64, width - 1]).unwrap(),
                1,
            )],
        )
        .unwrap();
        let prepared = PreparedPhaseSum::new(&phase, 2, &arithmetic).unwrap();
        assert_eq!(prepared.quotient().rank(), 1);
        assert_eq!(prepared.quotient().radical().len(), width - 1);
        let expected = arithmetic
            .multiply(
                &arithmetic
                    .add(&ExactScalar::one(), &arithmetic.omega(1, 0).unwrap())
                    .unwrap(),
                &arithmetic.omega(3, 2 * (width as i64 - 1)).unwrap(),
            )
            .unwrap();
        assert_eq!(
            prepared
                .evaluate(
                    &phase,
                    &zero_boundary(width),
                    &ExactScalar::one(),
                    &arithmetic
                )
                .unwrap(),
            expected
        );
    }
}

#[test]
fn invalid_dimensions_signature_changes_and_resource_limits_fail_explicitly() {
    let arithmetic = ScalarArithmetic::default();
    assert_eq!(ParityPhase::new(1, 8, []), Err(PhaseError::InvalidPhase));
    assert_eq!(
        ParityPhase::new(1, 0, [(bits(1, 1), 8)]),
        Err(PhaseError::InvalidPhase)
    );
    assert_eq!(
        ParityPhase::new(1, 0, [(bits(2, 1), 1)]),
        Err(PhaseError::DimensionMismatch)
    );
    let t = ParityPhase::new(1, 0, [(bits(1, 1), 1)]).unwrap();
    assert_eq!(t.clifford(), Err(PhaseError::NotClifford));
    assert!(matches!(
        PreparedPhaseSum::new(&t, 1, &arithmetic),
        Err(PhaseError::TermLimit { rank: 1, limit: 1 })
    ));
    let prepared = PreparedPhaseSum::new(&t, 2, &arithmetic).unwrap();
    let identity = ParityPhase::new(1, 0, []).unwrap();
    assert_eq!(
        prepared.evaluate(
            &identity,
            &zero_boundary(1),
            &ExactScalar::one(),
            &arithmetic
        ),
        Err(PhaseError::NotClifford)
    );
    assert_eq!(
        prepared.evaluate(&t, &zero_boundary(2), &ExactScalar::one(), &arithmetic),
        Err(PhaseError::DimensionMismatch)
    );
    assert_eq!(t.evaluate(&bits(2, 0)), Err(PhaseError::DimensionMismatch));
    assert_eq!(
        t.restrict(&bits(1, 0), &[bits(2, 0)]),
        Err(PhaseError::DimensionMismatch)
    );
    assert_eq!(
        t.subtract(&ParityPhase::new(2, 0, []).unwrap()),
        Err(PhaseError::DimensionMismatch)
    );
    assert_eq!(
        t.signature(&bits(1, 0), &bits(2, 0), &bits(1, 0)),
        Err(PhaseError::DimensionMismatch)
    );
    let high_rank = ParityPhase::new(
        usize::BITS as usize,
        0,
        (0..usize::BITS as usize).map(|j| {
            (
                BitVector::from_indices(usize::BITS as usize, [j]).unwrap(),
                1,
            )
        }),
    )
    .unwrap();
    assert!(matches!(
        PreparedPhaseSum::new(&high_rank, usize::MAX, &arithmetic),
        Err(PhaseError::TermLimit { .. })
    ));
    let merged =
        ParityPhase::new(1, 3, [(bits(1, 1), 1), (bits(1, 1), 7), (bits(1, 0), 5)]).unwrap();
    assert!(merged.terms().is_empty());
    assert_eq!(merged.constant(), 3);
}

#[test]
fn prescribed_native_closed_networks_match_full_complex_scalars() {
    let arithmetic = ScalarArithmetic::default();
    let document: Value = serde_json::from_str(include_str!("data/phase-reference.json")).unwrap();
    assert_eq!(document["schema"], "rmagna.phase-reference.v1");
    assert_eq!(
        document["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    let vectors = document["vectors"].as_array().unwrap();
    assert_eq!(vectors.len(), 28);
    for vector in vectors {
        let width = vector["width"].as_u64().unwrap() as usize;
        let parse_bits = |value: &Value| {
            let integer: BigInt = value.as_str().unwrap().parse().unwrap();
            let (_, bytes) = integer.to_bytes_le();
            BitVector::from_indices(
                width,
                bytes.iter().enumerate().flat_map(|(i, byte)| {
                    (0..8)
                        .filter(move |&j| byte >> j & 1 != 0)
                        .map(move |j| 8 * i + j)
                }),
            )
            .unwrap()
        };
        let terms = vector["terms"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| (parse_bits(&t[0]), t[1].as_u64().unwrap() as u8));
        let phase =
            ParityPhase::new(width, vector["constant"].as_u64().unwrap() as u8, terms).unwrap();
        let boundary = QuadraticForm::new(
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
                .map(parse_bits)
                .collect(),
        )
        .unwrap();
        let prepared = PreparedPhaseSum::new(&phase, 8, &arithmetic).unwrap();
        assert_eq!(
            prepared.quotient().rank() as u64,
            vector["quotient_rank"].as_u64().unwrap()
        );
        let expected = arithmetic
            .scalar(
                std::array::from_fn(|i| {
                    vector["result"]["coefficients"][i]
                        .as_str()
                        .unwrap()
                        .parse()
                        .unwrap()
                }),
                vector["result"]["power"].as_i64().unwrap(),
            )
            .unwrap();
        let coefficient = arithmetic
            .omega(0, vector["sqrt2_power"].as_i64().unwrap())
            .unwrap();
        assert_eq!(
            prepared
                .evaluate(&phase, &boundary, &coefficient, &arithmetic)
                .unwrap(),
            expected,
            "{vector}"
        );
        let fibers = rmagna::fibers::PreparedCliffordFibers::new(
            &phase,
            8,
            rmagna::fibers::FiberOptions::default(),
        )
        .unwrap();
        assert_eq!(
            fibers
                .evaluate(&phase, &boundary, &coefficient, &arithmetic)
                .unwrap(),
            expected,
            "native fiber {vector}"
        );
        let adaptive = ClosedPhaseSum::new_adaptive(
            &phase,
            &boundary,
            &coefficient,
            8,
            &arithmetic,
            rmagna::fibers::FiberOptions::default(),
        )
        .unwrap();
        assert_eq!(
            adaptive.evaluate(&arithmetic).unwrap(),
            expected,
            "native adaptive {vector}"
        );
        let absorbed =
            ClosedPhaseSum::new(&phase, &boundary, &coefficient, 8, &arithmetic).unwrap();
        assert!(absorbed.quotient_rank() <= prepared.quotient().rank());
        assert_eq!(
            absorbed.quotient_rank() as u64,
            vector["absorbed_rank"].as_u64().unwrap()
        );
        assert_eq!(absorbed.evaluate(&arithmetic).unwrap(), expected);
    }
}

#[test]
fn radical_constraints_reduce_the_resource_before_the_term_limit() {
    let arithmetic = ScalarArithmetic::default();
    // Summing the third bit enforces x0=x1. Two T phases become S on
    // that constrained coordinate, which can then be summed by Gauss elimination.
    let phase = ParityPhase::new(3, 5, [(bits(3, 1), 1), (bits(3, 2), 1)]).unwrap();
    let boundary =
        QuadraticForm::new(3, vec![0, 0, 0], vec![bits(3, 4), bits(3, 4), bits(3, 0)]).unwrap();
    assert_eq!(phase.quotient().unwrap().rank(), 2);
    assert!(matches!(
        PreparedPhaseSum::new(&phase, 1, &arithmetic),
        Err(PhaseError::TermLimit { rank: 2, limit: 1 })
    ));
    let reduced =
        ClosedPhaseSum::new(&phase, &boundary, &ExactScalar::one(), 1, &arithmetic).unwrap();
    assert_eq!(reduced.quotient_rank(), 0);
    assert_eq!(reduced.num_terms(), 1);
    assert_eq!(reduced.reductions().len(), 2);
    assert_eq!(reduced.reductions()[0].input_variables, 3);
    assert_eq!(reduced.reductions()[0].signature_rank, 2);
    assert_eq!(reduced.reductions()[0].output_variables, 1);
    assert_eq!(
        reduced.evaluate(&arithmetic).unwrap(),
        direct_sum(&phase, &boundary, &arithmetic)
    );
}

#[test]
fn radical_absorption_matches_literal_sums_and_explicit_zero_cases() {
    let arithmetic = ScalarArithmetic::default();
    let mut state = 0x7123abcde;
    for width in 0..=8 {
        for _ in 0..48 {
            let a = bits(width, next(&mut state));
            let b = bits(width, next(&mut state));
            let mut ab = a.clone();
            ab.xor_assign(&b).unwrap();
            let phase = ParityPhase::new(
                width,
                (next(&mut state) % 8) as u8,
                [(a, 1), (b, 3), (ab, 7)],
            )
            .unwrap();
            let mut upper = vec![bits(width, 0); width];
            for (i, row) in upper.iter_mut().enumerate() {
                for j in i + 1..width {
                    row.set(j, next(&mut state) & 1 != 0).unwrap();
                }
            }
            let boundary = QuadraticForm::new(
                (next(&mut state) % 4) as u8,
                (0..width).map(|_| (next(&mut state) % 4) as u8).collect(),
                upper,
            )
            .unwrap();
            let coefficient = arithmetic.omega(3, -3).unwrap();
            let result =
                ClosedPhaseSum::new(&phase, &boundary, &coefficient, 256, &arithmetic).unwrap();
            assert_eq!(
                result.evaluate(&arithmetic).unwrap(),
                arithmetic
                    .multiply(&coefficient, &direct_sum(&phase, &boundary, &arithmetic))
                    .unwrap()
            );
            assert!(
                result
                    .reductions()
                    .iter()
                    .all(|r| r.output_variables < r.input_variables)
            );
        }
    }
    let phase = ParityPhase::new(2, 0, [(bits(2, 1), 1)]).unwrap();
    let boundary = QuadraticForm::new(0, vec![0, 2], vec![bits(2, 0); 2]).unwrap();
    let zero = ClosedPhaseSum::new(&phase, &boundary, &ExactScalar::one(), 0, &arithmetic).unwrap();
    assert_eq!(zero.num_terms(), 0);
    assert_eq!(zero.evaluate(&arithmetic).unwrap(), ExactScalar::zero());
    assert!(matches!(
        ClosedPhaseSum::new(
            &phase,
            &zero_boundary(1),
            &ExactScalar::one(),
            4,
            &arithmetic
        ),
        Err(PhaseError::DimensionMismatch)
    ));
}

#[test]
fn packed_phase_pullbacks_match_direct_wide_boolean_substitution() {
    let mut state = 0x6adf02b189e34_u64;
    let mut random = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let width = 129;
    let mut basis = (0..64)
        .map(|_| BitVector::from_indices(width, (0..width).filter(|_| random() & 1 != 0)).unwrap())
        .collect::<Vec<_>>();
    let mut dependent = basis[0].clone();
    dependent.xor_assign(&basis[63]).unwrap();
    basis.push(dependent);
    for dense in [false, true] {
        for count in [63, 64, 65, 129] {
            let mut terms = (0..count)
                .map(|j| {
                    let indices = (0..width).filter(|&k| if dense { k != j } else { k == j });
                    (
                        BitVector::from_indices(width, indices).unwrap(),
                        (j % 7 + 1) as u8,
                    )
                })
                .collect::<Vec<_>>();
            // Duplicate axes cancel without changing the threshold case.
            terms.push((terms[0].0.clone(), 3));
            terms.push((terms[0].0.clone(), 5));
            terms.push((BitVector::zeros(width).unwrap(), 7));
            let phase = ParityPhase::new(width, 5, terms.clone()).unwrap();
            assert_eq!(phase.terms().len(), count);
            for offset_case in 0..2 {
                let shift =
                    BitVector::from_indices(width, (0..width).filter(|_| random() & 1 != 0))
                        .unwrap();
                for active_basis in [&basis[..], &[][..]] {
                    let restricted = phase.restrict(&shift, active_basis).unwrap();
                    assert_eq!(restricted.num_variables(), active_basis.len());
                    for _ in 0..64 {
                        let assignment = BitVector::from_indices(
                            active_basis.len(),
                            (0..active_basis.len()).filter(|_| random() & 1 != 0),
                        )
                        .unwrap();
                        let mut substituted = (0..width)
                            .map(|k| shift.get(k).unwrap())
                            .collect::<Vec<_>>();
                        for direction in assignment.ones() {
                            for k in active_basis[direction].ones() {
                                substituted[k] ^= true;
                            }
                        }
                        let mut expected = 5_u64;
                        for (axis, coefficient) in &terms {
                            if axis.ones().fold(false, |p, k| p ^ substituted[k]) {
                                expected += u64::from(*coefficient);
                            }
                        }
                        assert_eq!(
                            restricted.evaluate(&assignment).unwrap(),
                            (expected % 8) as u8,
                            "dense={dense}, count={count}, offset={offset_case}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn wide_clifford_extraction_preserves_odd_identities_and_packed_carries() {
    let width = 129;
    let mut state = 0x794b103a62_u64;
    let mut random = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for dense in [false, true] {
        let basis = (0..12)
            .map(|j| {
                BitVector::from_indices(
                    width,
                    (0..width).filter(|&k| k == j || (dense && k >= 12 && random() & 1 != 0)),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let axis = |indices: &[usize]| {
            let mut result = BitVector::zeros(width).unwrap();
            for &j in indices {
                result.xor_assign(&basis[j]).unwrap();
            }
            result
        };
        let mut terms = Vec::new();
        for group in 0..6 {
            let weight = (2 * group + 1) as u8 & 7;
            // Inclusion-exclusion of all nonempty parities of four bits is
            // 8*a*b*c*d, hence zero modulo eight despite odd coefficients.
            for mask in 1..16_usize {
                let indices = (0..4)
                    .filter(|&j| mask >> j & 1 != 0)
                    .map(|j| (2 * group + j) % 12)
                    .collect::<Vec<_>>();
                let coefficient = if mask.count_ones() & 1 != 0 {
                    weight
                } else {
                    (8 - weight) & 7
                };
                terms.push((axis(&indices), coefficient));
            }
        }
        for j in 0..12 {
            terms.push((axis(&[j, (j + 3) % 12]), [2, 4, 6][j % 3]));
        }
        let phase = ParityPhase::new(width, 7, terms).unwrap();
        assert!(phase.terms().iter().any(|(_, c)| c & 1 != 0));
        let identity = (0..width)
            .map(|j| BitVector::from_indices(width, [j]).unwrap())
            .collect::<Vec<_>>();
        let shifted = phase
            .restrict(
                &BitVector::from_indices(width, (0..width).filter(|_| random() & 1 != 0)).unwrap(),
                &identity,
            )
            .unwrap();
        for phase in [&phase, &shifted] {
            let (global, quadratic) = phase.clifford().unwrap();
            for _ in 0..256 {
                let value =
                    BitVector::from_indices(width, (0..width).filter(|_| random() & 1 != 0))
                        .unwrap();
                assert_eq!(
                    phase.evaluate(&value).unwrap(),
                    (global + 2 * quadratic.evaluate(&value).unwrap()) & 7,
                    "dense={dense}"
                );
            }
        }
        let t = ParityPhase::new(width, 0, [(axis(&[0]), 1)]).unwrap();
        let cs = ParityPhase::new(
            width,
            0,
            [(axis(&[0]), 1), (axis(&[1]), 1), (axis(&[0, 1]), 7)],
        )
        .unwrap();
        let ccz = ParityPhase::new(
            width,
            0,
            (1..8_usize).map(|mask| {
                let indices = (0..3).filter(|&j| mask >> j & 1 != 0).collect::<Vec<_>>();
                (
                    axis(&indices),
                    if mask.count_ones() & 1 != 0 { 1 } else { 7 },
                )
            }),
        )
        .unwrap();
        for rejected in [&t, &cs, &ccz] {
            assert!(matches!(rejected.clifford(), Err(PhaseError::NotClifford)));
        }
        let u = BitVector::from_indices(width, [0]).unwrap();
        let v = BitVector::from_indices(width, [1]).unwrap();
        let w = BitVector::from_indices(width, [2]).unwrap();
        assert!(!ccz.signature(&u, &u, &v).unwrap());
        assert!(ccz.signature(&u, &v, &w).unwrap());
    }
}

#[test]
fn compressed_signature_quotients_match_full_wide_coordinate_equations() {
    let width = 129;
    let pivots = [0, 1, 64, 65, 128];
    let mut rng = 0x3a791bcdef_u64;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    for scenario in 0..4 {
        let dimension = if scenario == 0 { 4 } else { 5 };
        let embedding = (0..dimension)
            .map(|j| {
                BitVector::from_indices(
                    width,
                    (0..width).filter(|i| {
                        if let Some(k) = pivots[..dimension].iter().position(|p| p == i) {
                            k == j
                        } else {
                            next() & 1 != 0
                        }
                    }),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let terms = (1_usize..1 << dimension)
            .map(|mask| {
                let mut axis = BitVector::zeros(width).unwrap();
                for (j, row) in embedding.iter().enumerate() {
                    if mask & (1 << j) != 0 {
                        axis.xor_assign(row).unwrap();
                    }
                }
                let coefficient = match scenario {
                    0 => 1, // All 15 nonzero four-dimensional axes cancel cubically.
                    1 => (next() & 7) as u8,
                    2 => 2 * ((next() & 3) as u8),
                    _ => {
                        if mask.count_ones() < 3 {
                            1
                        } else {
                            3
                        }
                    }
                };
                (axis, coefficient)
            })
            .collect::<Vec<_>>();
        let phase = ParityPhase::new(width, 3, terms).unwrap();
        let mut equations = Vec::new();
        // Literal cubic coefficients over every ambient coordinate pair. This
        // oracle never selects a spanning coordinate subset or an early stop.
        for i in 0..width {
            for j in i..width {
                equations.push(
                    BitVector::from_indices(
                        width,
                        (0..width).filter(|&k| {
                            phase.terms().iter().fold(false, |value, (axis, c)| {
                                value
                                    ^ (c & 1 != 0
                                        && axis.get(i).unwrap()
                                        && axis.get(j).unwrap()
                                        && axis.get(k).unwrap())
                            })
                        }),
                    )
                    .unwrap(),
                );
            }
        }
        let full = LinearFactorization::new(width, &equations).unwrap();
        let actual = phase.quotient().unwrap();
        assert_eq!(actual.rank(), full.rank(), "scenario={scenario}");
        let mut expected = full
            .pivot_columns()
            .iter()
            .map(|&j| BitVector::from_indices(width, [j]).unwrap())
            .collect::<Vec<_>>();
        expected.extend(full.nullspace().unwrap());
        assert_eq!(actual.basis(), expected, "scenario={scenario}");
        if scenario == 0 || scenario == 2 {
            assert_eq!(actual.rank(), 0);
        }
        for direction in actual.radical() {
            for row in &equations {
                assert!(!row.dot(direction).unwrap());
            }
        }
    }
}

#[test]
fn normalized_phase_differences_preserve_every_axis_and_independent_truth_values() {
    let mut seed = 20261021_u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for width in [0, 1, 3, 7, 65, 129] {
        for _ in 0..24 {
            let mut source = Vec::new();
            let mut changed = Vec::new();
            for _ in 0..40 {
                let axis =
                    BitVector::from_indices(width, (0..width).filter(|_| next() & 3 == 0)).unwrap();
                let c = (next() & 7) as u8;
                source.push((axis.clone(), c));
                changed.push((axis.clone(), if next() & 1 == 0 { c } else { (8 - c) & 7 }));
                source.push((axis.clone(), 2));
                source.push((axis.clone(), 6));
                if next() & 1 == 0 {
                    changed.push((
                        BitVector::from_indices(width, (0..width).filter(|_| next() & 3 == 0))
                            .unwrap(),
                        (next() & 7) as u8,
                    ));
                }
            }
            let left = ParityPhase::new(width, (next() & 7) as u8, source).unwrap();
            let right = ParityPhase::new(width, (next() & 7) as u8, changed).unwrap();
            let difference = left.subtract(&right).unwrap();
            let previous = ParityPhase::new(
                width,
                (left.constant() + 8 - right.constant()) & 7,
                left.terms()
                    .iter()
                    .cloned()
                    .chain(right.terms().iter().map(|(a, c)| (a.clone(), (8 - c) & 7))),
            )
            .unwrap();
            assert_eq!(difference, previous);
            assert!(
                difference
                    .terms()
                    .windows(2)
                    .all(|terms| terms[0].0.words() < terms[1].0.words())
            );
            assert!(
                difference
                    .terms()
                    .iter()
                    .all(|(axis, c)| !axis.is_zero() && (1..=7).contains(c))
            );
            for _ in 0..64 {
                let x =
                    BitVector::from_indices(width, (0..width).filter(|_| next() & 1 != 0)).unwrap();
                assert_eq!(
                    difference.evaluate(&x).unwrap(),
                    (left.evaluate(&x).unwrap() + 8 - right.evaluate(&x).unwrap()) & 7
                );
            }
            let cancelled = left.subtract(&left).unwrap();
            assert_eq!(cancelled.constant(), 0);
            assert!(cancelled.terms().is_empty());
        }
    }
}

#[test]
fn phase_difference_empty_inputs_cross_word_order_and_constants_are_exact() {
    let axis = |j| BitVector::from_indices(129, [j]).unwrap();
    let left = ParityPhase::new(129, 7, [(axis(0), 1), (axis(63), 4), (axis(64), 7)]).unwrap();
    let right = ParityPhase::new(129, 2, [(axis(64), 2), (axis(128), 3), (axis(0), 1)]).unwrap();
    let difference = left.subtract(&right).unwrap();
    assert_eq!(difference.constant(), 5);
    assert_eq!(
        difference.terms(),
        &[(axis(128), 5), (axis(64), 5), (axis(63), 4)]
    );
    for word in 0..16 {
        let value = BitVector::from_indices(
            129,
            [0, 63, 64, 128]
                .into_iter()
                .enumerate()
                .filter_map(|(j, k)| (word >> j & 1 != 0).then_some(k)),
        )
        .unwrap();
        assert_eq!(
            difference.evaluate(&value).unwrap(),
            (5 + 4 * ((word >> 1) & 1) + 5 * ((word >> 2) & 1) + 5 * ((word >> 3) & 1)) as u8 & 7
        );
    }
    let empty = ParityPhase::new(129, 0, []).unwrap();
    assert_eq!(left.subtract(&empty).unwrap(), left);
    let negative = empty.subtract(&right).unwrap();
    assert_eq!(negative.constant(), 6);
    assert_eq!(
        negative.terms(),
        &[(axis(128), 5), (axis(64), 6), (axis(0), 7)]
    );
    assert_eq!(empty.subtract(&empty).unwrap(), empty);
    let scalar = ParityPhase::new(0, 7, []).unwrap();
    assert_eq!(
        scalar
            .subtract(&ParityPhase::new(0, 2, []).unwrap())
            .unwrap()
            .constant(),
        5
    );
    assert!(matches!(
        left.subtract(&scalar),
        Err(PhaseError::DimensionMismatch)
    ));
}
