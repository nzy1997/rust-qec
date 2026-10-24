use rmagna::fibers::{FiberOptions, PreparedCliffordFibers};
use rmagna::gf2::BitVector;
use rmagna::phase::{ParityPhase, PhaseError};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
fn bits(n: usize, x: usize) -> BitVector {
    BitVector::from_indices(
        n,
        (0..n.min(usize::BITS as usize)).filter(|&i| x >> i & 1 != 0),
    )
    .unwrap()
}
fn zero(n: usize) -> QuadraticForm {
    QuadraticForm::new(0, vec![0; n], vec![bits(n, 0); n]).unwrap()
}
fn cubic(n: usize, a: usize, b: usize, c: usize) -> Vec<(BitVector, u8)> {
    vec![
        (bits(n, 1 << a), 1),
        (bits(n, 1 << b), 1),
        (bits(n, 1 << c), 1),
        (bits(n, (1 << a) | (1 << b)), 7),
        (bits(n, (1 << a) | (1 << c)), 7),
        (bits(n, (1 << b) | (1 << c)), 7),
        (bits(n, (1 << a) | (1 << b) | (1 << c)), 1),
    ]
}
fn literal(n: usize, phase: impl Fn(usize) -> u8, a: &ScalarArithmetic) -> ExactScalar {
    let mut lattice: [i64; 4] = [0; 4];
    for x in 0..(1 << n) {
        let e = phase(x) & 7;
        lattice[usize::from(e & 3)] += if e & 4 == 0 { 1 } else { -1 };
    }
    a.scalar(lattice.map(Into::into), 0).unwrap()
}
#[test]
fn labelled_fiber_expansions_match_literal_complex_path_sums_with_hidden_paths() {
    use rmagna::stabilizer::AffineKet;
    let a = ScalarArithmetic::default();
    let reference =
        ParityPhase::new(4, 3, [(bits(4, 1), 1), (bits(4, 3), 3), (bits(4, 14), 7)]).unwrap();
    let plan = PreparedCliffordFibers::new(&reference, 16, FiberOptions::default()).unwrap();
    for width in [0, 3, 65, 129] {
        let column =
            |j: usize| BitVector::from_indices(width, (0..width).filter(|&i| i % 7 == j)).unwrap();
        // Two coincident columns and one hidden column force Gaussian path
        // sums; the oracle below enumerates the original 16 paths directly.
        let columns = vec![
            column(0),
            column(1),
            column(0),
            BitVector::zeros(width).unwrap(),
        ];
        let offset = column(2);
        let mut upper = vec![bits(4, 0); 4];
        upper[0].set(2, true).unwrap();
        upper[1].set(3, true).unwrap();
        let phase = QuadraticForm::new(1, vec![1, 2, 3, 0], upper).unwrap();
        let boundary = AffineKet::new(
            offset.clone(),
            columns.clone(),
            phase,
            a.omega(3, -7).unwrap(),
        )
        .unwrap();
        for changed in [false, true] {
            let updated = ParityPhase::new(
                4,
                if changed { 1 } else { 3 },
                [(bits(4, 1), 1), (bits(4, 3), 3), (bits(4, 14), 7)]
                    .into_iter()
                    .chain(changed.then_some((bits(4, 5), 2)))
                    .chain(changed.then_some((bits(4, 9), 4))),
            )
            .unwrap();
            let terms = plan.expand(&updated, &boundary, &a).unwrap();
            let prepared = terms
                .iter()
                .map(|term| term.prepare().unwrap())
                .collect::<Vec<_>>();
            let mut oracle = std::collections::HashMap::<BitVector, ExactScalar>::new();
            for word in 0usize..16 {
                let b = |j: usize| (word >> j) & 1usize;
                let mut label = offset.clone();
                for (j, col) in columns.iter().enumerate() {
                    if b(j) != 0 {
                        label.xor_assign(col).unwrap();
                    }
                }
                let p = if changed { 1 } else { 3 }
                    + b(0)
                    + 3 * (b(0) ^ b(1))
                    + 7 * (b(1) ^ b(2) ^ b(3))
                    + if changed {
                        2 * (b(0) ^ b(2)) + 4 * (b(0) ^ b(3))
                    } else {
                        0
                    };
                let q = 1 + b(0) + 2 * b(1) + 3 * b(2) + 2 * b(0) * b(2) + 2 * b(1) * b(3);
                let amplitude = a.omega((3 + p + 2 * q) as i64, -7).unwrap();
                let old = oracle
                    .get(&label)
                    .cloned()
                    .unwrap_or_else(ExactScalar::zero);
                oracle.insert(label, a.add(&old, &amplitude).unwrap());
            }
            for (label, expected) in oracle {
                let actual = prepared.iter().fold(ExactScalar::zero(), |total, k| {
                    a.add(&total, &k.amplitude(&label, &a).unwrap()).unwrap()
                });
                assert_eq!(actual, expected, "width={width},changed={changed}");
            }
        }
    }
    let invalid =
        AffineKet::new(bits(1, 0), vec![bits(1, 1)], zero(1), ExactScalar::zero()).unwrap();
    assert!(matches!(
        plan.expand(&reference, &invalid, &a),
        Err(PhaseError::DimensionMismatch)
    ));
    let incompatible =
        ParityPhase::new(4, 3, [(bits(4, 1), 2), (bits(4, 3), 3), (bits(4, 14), 7)]).unwrap();
    let boundary = AffineKet::new(
        bits(1, 0),
        vec![bits(1, 0); 4],
        zero(4),
        ExactScalar::zero(),
    )
    .unwrap();
    assert!(matches!(
        plan.expand(&incompatible, &boundary, &a),
        Err(PhaseError::NotClifford)
    ));
}
#[test]
fn t_pairs_and_ccz_use_two_certified_affine_clifford_terms() {
    let a = ScalarArithmetic::default();
    let pair = ParityPhase::new(2, 0, [(bits(2, 1), 1), (bits(2, 2), 1)]).unwrap();
    let plan = PreparedCliffordFibers::new(&pair, 2, FiberOptions::default()).unwrap();
    assert_eq!(plan.quotient_rank(), 2);
    assert_eq!(plan.num_terms(), 2);
    assert_eq!(plan.enumerated_coordinates(), 1);
    assert_eq!(
        plan.evaluate(&pair, &zero(2), &ExactScalar::one(), &a)
            .unwrap(),
        literal(2, |x| x.count_ones() as u8, &a)
    );
    let ccz = ParityPhase::new(3, 0, cubic(3, 0, 1, 2)).unwrap();
    let plan = PreparedCliffordFibers::new(&ccz, 2, FiberOptions::default()).unwrap();
    assert_eq!(plan.quotient_rank(), 3);
    assert_eq!(plan.num_terms(), 2);
    assert_eq!(
        plan.evaluate(&ccz, &zero(3), &ExactScalar::one(), &a)
            .unwrap(),
        a.scalar([6.into(), 0.into(), 0.into(), 0.into()], 0)
            .unwrap()
    );
    // Every affine shift retains this partition.
    for shift in 0..8 {
        let shifted = ccz
            .restrict(
                &bits(3, shift),
                &(0..3).map(|i| bits(3, 1 << i)).collect::<Vec<_>>(),
            )
            .unwrap();
        assert_eq!(
            plan.evaluate(&shifted, &zero(3), &ExactScalar::one(), &a)
                .unwrap(),
            literal(3, |x| if (x ^ shift) == 7 { 4 } else { 0 }, &a)
        );
    }
}
#[test]
fn every_three_variable_signature_and_clifford_update_matches_raw_monomial_sums() {
    let a = ScalarArithmetic::default();
    for n in 0..=3 {
        let pairs = (0..n)
            .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
            .collect::<Vec<_>>();
        let count = n + pairs.len() + usize::from(n == 3);
        for mask in 0..(1 << count) {
            let mut terms = Vec::new();
            for i in 0..n {
                if mask >> i & 1 != 0 {
                    terms.push((bits(n, 1 << i), 1));
                }
            }
            for (j, &(u, v)) in pairs.iter().enumerate() {
                if mask >> (n + j) & 1 != 0 {
                    terms.extend([
                        (bits(n, 1 << u), 1),
                        (bits(n, 1 << v), 1),
                        (bits(n, (1 << u) | (1 << v)), 7),
                    ]);
                }
            }
            if n == 3 && mask >> (n + pairs.len()) & 1 != 0 {
                terms.extend(cubic(n, 0, 1, 2));
            }
            let reference = ParityPhase::new(n, 3, terms.clone()).unwrap();
            let plan =
                PreparedCliffordFibers::new(&reference, 1 << n, FiberOptions::default()).unwrap();
            assert_eq!(
                plan.clifford_directions().len() + plan.enumerated_coordinates(),
                n
            );
            for update in 0..3 {
                let mut changed = terms.clone();
                for i in 0..n {
                    changed.push((bits(n, 1 << i), ((i + update) % 4 * 2) as u8));
                }
                let updated = ParityPhase::new(n, ((3 + update) & 7) as u8, changed).unwrap();
                let boundary = QuadraticForm::new(
                    2,
                    vec![1; n],
                    (0..n)
                        .map(|i| {
                            BitVector::from_indices(
                                n,
                                (i + 1..n).filter(|j| (i + j + update) % 2 == 0),
                            )
                            .unwrap()
                        })
                        .collect(),
                )
                .unwrap();
                let expected = literal(
                    n,
                    |x| {
                        let mut value = 3 + update + 4;
                        for i in 0..n {
                            let b = x >> i & 1;
                            value += b * (mask >> i & 1) + b * ((i + update) % 4 * 2) + 2 * b;
                        }
                        for (j, &(u, v)) in pairs.iter().enumerate() {
                            value += 2 * ((x >> u & 1) * (x >> v & 1)) * (mask >> (n + j) & 1);
                            if (u + v + update) % 2 == 0 {
                                value += 4 * ((x >> u & 1) * (x >> v & 1));
                            }
                        }
                        if n == 3 {
                            value += 4 * usize::from(x == 7) * (mask >> (n + pairs.len()) & 1);
                        }
                        (value & 7) as u8
                    },
                    &a,
                );
                assert_eq!(
                    plan.evaluate(&updated, &boundary, &ExactScalar::one(), &a)
                        .unwrap(),
                    expected,
                    "n={n} mask={mask} update={update}"
                );
            }
        }
    }
}
#[test]
fn disconnected_controlled_clifford_blocks_need_four_terms_instead_of_sixty_four() {
    let a = ScalarArithmetic::default();
    let phase =
        ParityPhase::new(6, 1, cubic(6, 0, 1, 2).into_iter().chain(cubic(6, 3, 4, 5))).unwrap();
    let plan = PreparedCliffordFibers::new(&phase, 4, FiberOptions::default()).unwrap();
    assert_eq!(plan.quotient_rank(), 6);
    assert_eq!(plan.num_terms(), 4);
    assert_eq!(
        plan.evaluate(&phase, &zero(6), &ExactScalar::one(), &a)
            .unwrap(),
        literal(
            6,
            |x| 1 + 4 * u8::from(x & 7 == 7) + 4 * u8::from(x >> 3 == 7),
            &a
        )
    );
}
#[test]
fn incompatible_updates_budgets_and_disabled_search_are_explicit() {
    let a = ScalarArithmetic::default();
    let phase = ParityPhase::new(2, 0, [(bits(2, 1), 1), (bits(2, 2), 1)]).unwrap();
    assert!(matches!(
        PreparedCliffordFibers::new(&phase, 1, FiberOptions::default()),
        Err(PhaseError::FiberTermLimit { coordinates: 1, .. })
    ));
    assert!(
        PreparedCliffordFibers::new(
            &phase,
            2,
            FiberOptions {
                attempts: 0,
                ..Default::default()
            }
        )
        .is_err()
    );
    let fallback = PreparedCliffordFibers::new(
        &phase,
        4,
        FiberOptions {
            max_search_rank: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(fallback.num_terms(), 4);
    let plan = PreparedCliffordFibers::new(&phase, 2, FiberOptions::default()).unwrap();
    let different = ParityPhase::new(2, 0, [(bits(2, 1), 1)]).unwrap();
    assert!(matches!(
        plan.evaluate(&different, &zero(2), &ExactScalar::one(), &a),
        Err(PhaseError::NotClifford)
    ));
    assert!(matches!(
        plan.evaluate(&phase, &zero(1), &ExactScalar::one(), &a),
        Err(PhaseError::DimensionMismatch)
    ));
    let empty = ParityPhase::new(0, 7, []).unwrap();
    let plan = PreparedCliffordFibers::new(&empty, 1, FiberOptions::default()).unwrap();
    assert_eq!(plan.num_terms(), 1);
    assert_eq!(
        plan.evaluate(&empty, &zero(0), &a.omega(2, -7).unwrap(), &a)
            .unwrap(),
        a.omega(9, -7).unwrap()
    );
}
#[test]
fn wide_clifford_radicals_and_complex_boundary_coefficients_preserve_phase() {
    let a = ScalarArithmetic::default();
    let n = 129;
    let phase = ParityPhase::new(
        n,
        1,
        [
            (BitVector::from_indices(n, [0, 64]).unwrap(), 1),
            (BitVector::from_indices(n, [128]).unwrap(), 1),
        ],
    )
    .unwrap();
    let plan = PreparedCliffordFibers::new(&phase, 2, FiberOptions::default()).unwrap();
    assert_eq!(plan.quotient_rank(), 2);
    assert_eq!(plan.num_terms(), 2);
    assert_eq!(plan.clifford_directions().len(), 128);
    let coefficient = a
        .scalar([2.into(), (-3).into(), 5.into(), 7.into()], -101)
        .unwrap();
    let small = literal(
        3,
        |x| 1 + u8::from((x & 1 != 0) ^ (x & 2 != 0)) + u8::from(x & 4 != 0),
        &a,
    );
    let expected = a
        .multiply(
            &a.multiply(&small, &a.omega(0, 252).unwrap()).unwrap(),
            &coefficient,
        )
        .unwrap();
    assert_eq!(
        plan.evaluate(&phase, &zero(n), &coefficient, &a).unwrap(),
        expected
    );
}

#[test]
fn dense_higher_dimensional_fibers_and_covariant_sign_updates_match_integer_lattice_oracles() {
    let a = ScalarArithmetic::default();
    let mut state = 0x53bca012u64;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        state
    };
    for n in 4..=8 {
        for case in 0..12 {
            let masks = (0..(n * 3 + 1))
                .map(|_| {
                    (
                        ((next() >> 20) as usize) & ((1 << n) - 1),
                        (next() >> 32) as u8 & 7,
                    )
                })
                .collect::<Vec<_>>();
            let make = |changed: bool| {
                ParityPhase::new(
                    n,
                    if changed { 5 } else { 2 },
                    masks
                        .iter()
                        .map(|&(mask, c)| (bits(n, mask), if changed { (8 - c) & 7 } else { c })),
                )
                .unwrap()
            };
            let reference = make(false);
            let updated = make(true);
            let plan =
                PreparedCliffordFibers::new(&reference, 1 << n, FiberOptions::default()).unwrap();
            let linear = (0..n).map(|_| (next() >> 32) as u8 & 3).collect::<Vec<_>>();
            let crosses = (0..n)
                .map(|i| {
                    (i + 1..n)
                        .map(|j| (j, next() >> 63 != 0))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let boundary = QuadraticForm::new(
                3,
                linear.clone(),
                crosses
                    .iter()
                    .map(|row| {
                        BitVector::from_indices(n, row.iter().filter_map(|&(j, v)| v.then_some(j)))
                            .unwrap()
                    })
                    .collect(),
            )
            .unwrap();
            let raw = literal(
                n,
                |x| {
                    let mut phase = 5 + 6;
                    for &(mask, c) in &masks {
                        if (x & mask).count_ones() & 1 != 0 {
                            phase += usize::from((8 - c) & 7);
                        }
                    }
                    for i in 0..n {
                        if x >> i & 1 != 0 {
                            phase += 2 * usize::from(linear[i]);
                            for &(j, v) in &crosses[i] {
                                if v && x >> j & 1 != 0 {
                                    phase += 4;
                                }
                            }
                        }
                    }
                    (phase & 7) as u8
                },
                &a,
            );
            let coefficient = a.omega(3, -5).unwrap();
            let expected = a.multiply(&raw, &coefficient).unwrap();
            assert_eq!(
                plan.evaluate(&updated, &boundary, &coefficient, &a)
                    .unwrap(),
                expected,
                "width={n} case={case}"
            );
            assert!(
                PreparedCliffordFibers::new(
                    &reference,
                    plan.num_terms() - 1,
                    FiberOptions::default()
                )
                .is_err()
            );
        }
    }
}
#[test]
fn adaptive_closed_sums_apply_the_smaller_term_budget_after_radical_absorption() {
    use rmagna::phase::ClosedPhaseSum;
    let a = ScalarArithmetic::default();
    let pair = ParityPhase::new(2, 0, [(bits(2, 1), 1), (bits(2, 2), 1)]).unwrap();
    assert!(ClosedPhaseSum::new(&pair, &zero(2), &ExactScalar::one(), 2, &a).is_err());
    let adaptive = ClosedPhaseSum::new_adaptive(
        &pair,
        &zero(2),
        &ExactScalar::one(),
        2,
        &a,
        FiberOptions::default(),
    )
    .unwrap();
    assert_eq!(adaptive.quotient_rank(), 2);
    assert_eq!(adaptive.num_terms(), 2);
    assert_eq!(
        adaptive.evaluate(&a).unwrap(),
        literal(2, |x| x.count_ones() as u8, &a)
    );
    let empty = ClosedPhaseSum::new_adaptive(
        &pair,
        &zero(2),
        &ExactScalar::zero(),
        0,
        &a,
        FiberOptions::default(),
    )
    .unwrap();
    assert_eq!(empty.num_terms(), 0);
    assert_eq!(empty.evaluate(&a).unwrap(), ExactScalar::zero());
}

#[test]
fn four_variable_parity_controlled_cubic_phase_has_two_even_odd_fibers() {
    // phi = x0+x3+4*x1*x2*(x0 XOR x3). Even parity leaves 2*x0;
    // odd parity leaves 1+4*x1*x2. Both are Clifford on their affine fiber.
    let phase = ParityPhase::new(
        4,
        0,
        [(bits(4, 1), 1), (bits(4, 8), 1)]
            .into_iter()
            .chain(cubic(4, 0, 1, 2))
            .chain(cubic(4, 3, 1, 2)),
    )
    .unwrap();
    let plan = PreparedCliffordFibers::new(&phase, 2, FiberOptions::default()).unwrap();
    assert_eq!(plan.quotient_rank(), 4);
    assert_eq!(plan.num_terms(), 2);
    let a = ScalarArithmetic::default();
    for offset in 0..16 {
        let shifted = phase
            .restrict(
                &bits(4, offset),
                &(0..4).map(|i| bits(4, 1 << i)).collect::<Vec<_>>(),
            )
            .unwrap();
        let expected = literal(
            4,
            |y| {
                let x = y ^ offset;
                ((x & 1)
                    + (x >> 3 & 1)
                    + 4 * (x >> 1 & 1) * (x >> 2 & 1) * ((x & 1) ^ (x >> 3 & 1)))
                    as u8
            },
            &a,
        );
        assert_eq!(
            plan.evaluate(&shifted, &zero(4), &ExactScalar::one(), &a)
                .unwrap(),
            expected
        );
    }
    let wide = ParityPhase::new(
        65,
        0,
        (0..65).map(|i| (BitVector::from_indices(65, [i]).unwrap(), 1)),
    )
    .unwrap();
    assert!(matches!(
        PreparedCliffordFibers::new(&wide, 4, FiberOptions::default()),
        Err(PhaseError::FiberTermLimit {
            coordinates: 65,
            ..
        })
    ));
}
