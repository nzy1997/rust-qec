use rmagna::gf2::BitVector;
use rmagna::phase::{CanonicalPhaseCore, ParityPhase, PhaseCoreLimits, PhaseError};
fn bits(n: usize, word: usize) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|j| word >> j & 1 != 0)).unwrap()
}
fn check(phase: &ParityPhase) {
    let n = phase.num_variables();
    let count = 1 << n;
    let core = CanonicalPhaseCore::new(phase, &PhaseCoreLimits::default()).unwrap();
    let table = (0..count)
        .map(|x| phase.evaluate(&bits(n, x)).unwrap())
        .collect::<Vec<_>>();
    let (global, q) = core.clifford_correction();
    assert_eq!(global, table[0]);
    assert_eq!(q.num_variables(), n);
    assert_eq!(core.dimension(), n);
    // Independent finite differences of complete values, followed by a direct
    // monomial sum. This does not use the constructor's parity-axis expansion
    // or its subset evaluation transform.
    let mut coefficients = vec![0_u8; count];
    for (mask, coefficient) in coefficients.iter_mut().enumerate().skip(1) {
        let mut sub = mask;
        let mut value = 0_i64;
        loop {
            let sign = if (mask.count_ones() - sub.count_ones()) % 2 == 0 {
                1
            } else {
                -1
            };
            value += sign * i64::from(table[sub]);
            if sub == 0 {
                break;
            }
            sub = (sub - 1) & mask;
        }
        let value = value.rem_euclid(8) as u8;
        *coefficient = match mask.count_ones() {
            1 => value & 1,
            2 => {
                assert_eq!(value & 1, 0);
                value & 3
            }
            3 => {
                assert_eq!(value & 3, 0);
                value
            }
            _ => {
                assert_eq!(value, 0);
                0
            }
        };
    }
    for (word, value) in table.iter().enumerate() {
        let expected = coefficients
            .iter()
            .enumerate()
            .filter(|(mask, _)| mask & word == *mask)
            .map(|(_, c)| usize::from(*c))
            .sum::<usize>() as u8
            & 7;
        assert_eq!(core.phase_table()[word], expected);
        assert_eq!(
            (core.phase_table()[word] + global + 2 * q.evaluate(&bits(n, word)).unwrap()) & 7,
            *value,
            "dimension={n} word={word}"
        );
    }
}
#[test]
fn every_small_signature_with_signs_clifford_gauges_and_globals_matches_finite_differences() {
    for n in 0..=3 {
        let axes = (1 << n) - 1;
        for signature in 0..1_usize << axes {
            for global in 0..8 {
                let terms = (1..=axes)
                    .map(|axis| {
                        let odd = (signature >> (axis - 1) & 1) as u8;
                        let even = 2 * ((axis + signature + usize::from(global)) % 4) as u8;
                        (bits(n, axis), odd + even)
                    })
                    .collect::<Vec<_>>();
                let phase = ParityPhase::new(n, global, terms.clone()).unwrap();
                check(&phase);
                let updated = ParityPhase::new(
                    n,
                    (global + 5) & 7,
                    terms.into_iter().map(|(axis, c)| (axis, (c + 2) & 7)),
                )
                .unwrap();
                check(&updated);
                assert_eq!(
                    CanonicalPhaseCore::new(&phase, &PhaseCoreLimits::default())
                        .unwrap()
                        .phase_table(),
                    CanonicalPhaseCore::new(&updated, &PhaseCoreLimits::default())
                        .unwrap()
                        .phase_table()
                );
            }
        }
    }
}
#[test]
fn shifted_and_mixed_coordinates_reconstruct_all_phases_without_claiming_basis_invariance() {
    let mut state = 731_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for n in [4, 5] {
        for _ in 0..120 {
            let terms = (1..1 << n)
                .map(|axis| (bits(n, axis), (next() & 7) as u8))
                .collect::<Vec<_>>();
            let phase = ParityPhase::new(n, (next() & 7) as u8, terms).unwrap();
            check(&phase);
            let identity = (0..n).map(|j| bits(n, 1 << j)).collect::<Vec<_>>();
            let shifted = phase
                .restrict(&bits(n, next() as usize & ((1 << n) - 1)), &identity)
                .unwrap();
            check(&shifted);
            assert_eq!(
                CanonicalPhaseCore::new(&phase, &PhaseCoreLimits::default())
                    .unwrap()
                    .phase_table(),
                CanonicalPhaseCore::new(&shifted, &PhaseCoreLimits::default())
                    .unwrap()
                    .phase_table()
            );
            let mut mixed = identity;
            let last = mixed[n - 1].clone();
            mixed[0].xor_assign(&last).unwrap();
            mixed.rotate_left(1);
            let changed = phase
                .restrict(&bits(n, next() as usize & ((1 << n) - 1)), &mixed)
                .unwrap();
            check(&changed);
        }
    }
}
#[test]
fn parity_ccz_zero_dimension_and_resource_boundaries_are_explicit() {
    let phase = ParityPhase::new(2, 0, [(bits(2, 3), 1)]).unwrap();
    let limits = PhaseCoreLimits {
        max_dimension: 2,
        max_table_values: 4,
        max_work: 15,
    };
    let core = CanonicalPhaseCore::new(&phase, &limits).unwrap();
    assert_eq!(core.phase_table(), [0, 1, 1, 4]);
    assert_eq!(core.work_units(), 15);
    let (global, q) = core.clifford_correction();
    assert_eq!(global, 0);
    assert_eq!(q.evaluate(&bits(2, 3)).unwrap(), 2);
    for (option, limits) in [
        (
            "phase_core.max_dimension",
            PhaseCoreLimits {
                max_dimension: 1,
                ..limits
            },
        ),
        (
            "phase_core.max_table_values",
            PhaseCoreLimits {
                max_table_values: 3,
                ..limits
            },
        ),
        (
            "phase_core.max_work",
            PhaseCoreLimits {
                max_work: 14,
                ..limits
            },
        ),
    ] {
        assert!(
            matches!(CanonicalPhaseCore::new(&phase,&limits),Err(PhaseError::CoreLimit{option:name,..})if name==option)
        );
    }
    let ccz = ParityPhase::new(
        3,
        0,
        (1..8).map(|axis| {
            (
                bits(3, axis),
                if axis.count_ones() % 2 == 1 { 1 } else { 7 },
            )
        }),
    )
    .unwrap();
    let core = CanonicalPhaseCore::new(&ccz, &PhaseCoreLimits::default()).unwrap();
    assert_eq!(core.phase_table(), [0, 0, 0, 0, 0, 0, 0, 4]);
    let (global, q) = core.clifford_correction();
    assert_eq!(global, 0);
    for word in 0..8 {
        assert_eq!(q.evaluate(&bits(3, word)).unwrap(), 0);
    }
    let constant = ParityPhase::new(0, 7, []).unwrap();
    let core = CanonicalPhaseCore::new(
        &constant,
        &PhaseCoreLimits {
            max_dimension: 0,
            max_table_values: 1,
            max_work: 1,
        },
    )
    .unwrap();
    assert_eq!(core.phase_table(), [0]);
    assert_eq!(core.clifford_correction().0, 7);
    assert_eq!(core.work_units(), 1);
    assert!(
        CanonicalPhaseCore::new(
            &constant,
            &PhaseCoreLimits {
                max_work: 0,
                ..Default::default()
            }
        )
        .is_err()
    );
    for n in [65, 129] {
        let wide =
            ParityPhase::new(n, 0, [(BitVector::from_indices(n, [n - 1]).unwrap(), 1)]).unwrap();
        let error = CanonicalPhaseCore::new(
            &wide,
            &PhaseCoreLimits {
                max_dimension: n,
                max_table_values: usize::MAX,
                max_work: u128::MAX,
            },
        )
        .unwrap_err();
        assert!(
            matches!(error,PhaseError::CoreLimit{option:"phase_core.max_table_values",required,..}if required>usize::MAX as u128)
        );
    }
}

#[test]
fn literal_four_coordinate_core_and_clifford_rest_survive_signs_and_affine_shifts() {
    // Literal values were derived from complete-value finite differences before
    // compiling the normalizer. This fixture mixes odd and even coefficients.
    let terms = [(3, 7), (5, 3), (12, 6), (15, 1), (1, 2), (2, 4), (8, 6)];
    let phase = ParityPhase::new(4, 5, terms.map(|(mask, c)| (bits(4, mask), c))).unwrap();
    let limits = PhaseCoreLimits {
        max_dimension: 4,
        max_table_values: 16,
        max_work: 106,
    };
    let expected = [0, 1, 0, 1, 0, 1, 2, 7, 1, 4, 3, 2, 3, 2, 3, 2];
    let values = [5, 2, 1, 6, 7, 4, 1, 2, 2, 5, 4, 3, 6, 5, 2, 1];
    let core = CanonicalPhaseCore::new(&phase, &limits).unwrap();
    assert_eq!(core.phase_table(), expected);
    assert_eq!(core.work_units(), 106);
    let (global, correction) = core.clifford_correction();
    assert_eq!(global, 5);
    assert_eq!(correction.linear(), &[2, 2, 1, 2]);
    assert_eq!(
        correction.upper(),
        &[bits(4, 8), bits(4, 12), bits(4, 0), bits(4, 0)]
    );
    for (word, &value) in values.iter().enumerate() {
        assert_eq!(phase.evaluate(&bits(4, word)).unwrap(), value);
        assert_eq!(
            (expected[word] + global + 2 * correction.evaluate(&bits(4, word)).unwrap()) & 7,
            value
        );
    }
    assert!(matches!(
        CanonicalPhaseCore::new(
            &phase,
            &PhaseCoreLimits {
                max_work: 105,
                ..limits
            }
        ),
        Err(PhaseError::CoreLimit {
            option: "phase_core.max_work",
            limit: 105,
            required: 106
        })
    ));
    let identity = [bits(4, 1), bits(4, 2), bits(4, 4), bits(4, 8)];
    let shifted = phase.restrict(&bits(4, 10), &identity).unwrap();
    assert_eq!(
        CanonicalPhaseCore::new(&shifted, &limits)
            .unwrap()
            .phase_table(),
        expected
    );
    let reversed =
        ParityPhase::new(4, 3, terms.map(|(mask, c)| (bits(4, mask), (8 - c) & 7))).unwrap();
    assert_eq!(
        CanonicalPhaseCore::new(&reversed, &limits)
            .unwrap()
            .phase_table(),
        expected
    );
    let columns = [bits(4, 3), bits(4, 2), bits(4, 12), bits(4, 8)];
    let mixed = phase.restrict(&bits(4, 10), &columns).unwrap();
    let mixed_core = CanonicalPhaseCore::new(&mixed, &limits).unwrap();
    assert_eq!(
        mixed_core.phase_table(),
        &[0, 1, 0, 1, 1, 4, 1, 4, 1, 2, 3, 4, 2, 5, 4, 7]
    );
    assert_ne!(mixed_core.phase_table(), core.phase_table());
    for word in 0..16 {
        let original_word = [3, 2, 12, 8].iter().enumerate().fold(10, |x, (j, column)| {
            if word >> j & 1 != 0 { x ^ column } else { x }
        });
        assert_eq!(
            mixed.evaluate(&bits(4, word)).unwrap(),
            values[original_word]
        );
    }
}
