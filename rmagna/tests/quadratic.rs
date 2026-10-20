use rmagna::gf2::BitVector;
use rmagna::quadratic::{QuadraticError, QuadraticForm};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};

fn bits(width: usize, mask: u64) -> BitVector {
    BitVector::from_indices(width, (0..width).filter(|&j| mask & (1 << j) != 0)).unwrap()
}

fn form(width: usize, constant: u8, mut linear: u64, mut edges: u64) -> QuadraticForm {
    let coefficients = (0..width)
        .map(|_| {
            let phase = (linear & 3) as u8;
            linear >>= 2;
            phase
        })
        .collect();
    let mut upper = (0..width)
        .map(|_| BitVector::zeros(width).unwrap())
        .collect::<Vec<_>>();
    for (i, row) in upper.iter_mut().enumerate() {
        for j in i + 1..width {
            row.set(j, edges & 1 != 0).unwrap();
            edges >>= 1;
        }
    }
    QuadraticForm::new(constant, coefficients, upper).unwrap()
}

fn direct_sum(form: &QuadraticForm, arithmetic: &ScalarArithmetic) -> ExactScalar {
    let mut total = ExactScalar::zero();
    for mask in 0..1 << form.num_variables() {
        let phase = form.evaluate(&bits(form.num_variables(), mask)).unwrap();
        total = arithmetic
            .add(&total, &arithmetic.omega(i64::from(phase) * 2, 0).unwrap())
            .unwrap();
    }
    total
}

#[test]
fn every_quadratic_form_up_to_three_variables_matches_explicit_gauss_sum() {
    let arithmetic = ScalarArithmetic::default();
    for width in 0_usize..=3 {
        let edge_count = width * width.saturating_sub(1) / 2;
        for constant in 0..4 {
            for linear in 0..1 << (2 * width) {
                for edges in 0..1 << edge_count {
                    let q = form(width, constant, linear, edges);
                    assert_eq!(
                        q.gauss_sum(&arithmetic).unwrap(),
                        direct_sum(&q, &arithmetic),
                        "{q:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn dense_higher_dimensional_phases_match_literal_sums() {
    let arithmetic = ScalarArithmetic::default();
    let mut state = 0xa168235bd39_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for width in 4..=9 {
        for _ in 0..32 {
            let q = form(width, (next() % 4) as u8, next(), next());
            assert_eq!(
                q.gauss_sum(&arithmetic).unwrap(),
                direct_sum(&q, &arithmetic),
                "{q:?}"
            );
        }
    }
}

#[test]
fn affine_restriction_preserves_phase_for_injective_and_dependent_maps() {
    let mut state = 0xd234abc543_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for width in 0..=6 {
        for coordinates in 0..=5 {
            for _ in 0..8 {
                let q = form(width, (next() % 4) as u8, next(), next());
                let shift = bits(width, next());
                let basis = (0..coordinates)
                    .map(|_| bits(width, next()))
                    .collect::<Vec<_>>();
                let restricted = q.restrict(&shift, &basis).unwrap();
                assert_eq!(restricted.num_variables(), coordinates);
                for mask in 0_u64..1 << coordinates {
                    let mut x = shift.clone();
                    for (j, vector) in basis.iter().enumerate() {
                        if mask & (1 << j) != 0 {
                            x.xor_assign(vector).unwrap();
                        }
                    }
                    assert_eq!(
                        restricted.evaluate(&bits(coordinates, mask)).unwrap(),
                        q.evaluate(&x).unwrap()
                    );
                }
            }
        }
    }
}

#[test]
fn many_independent_hyperbolic_pairs_and_odd_phases_keep_global_scale() {
    let arithmetic = ScalarArithmetic::default();
    for width in [64, 65, 129, 130] {
        let mut upper = (0..width)
            .map(|_| BitVector::zeros(width).unwrap())
            .collect::<Vec<_>>();
        for i in (0..width - 1).step_by(2) {
            upper[i].set(i + 1, true).unwrap();
        }
        let mut linear = vec![0; width];
        if width % 2 == 1 {
            linear[width - 1] = 1;
        }
        let q = QuadraticForm::new(3, linear, upper).unwrap();
        let expected_phase = if width % 2 == 1 { 7 } else { 6 };
        assert_eq!(
            q.gauss_sum(&arithmetic).unwrap(),
            arithmetic.omega(expected_phase, width as i64).unwrap()
        );
    }
    let q = form(1, 0, 2, 0);
    assert_eq!(q.gauss_sum(&arithmetic).unwrap(), ExactScalar::zero());
}

#[test]
fn malformed_phases_triangles_and_dimensions_are_rejected() {
    assert_eq!(
        QuadraticForm::new(4, vec![], vec![]),
        Err(QuadraticError::InvalidPhase)
    );
    assert_eq!(
        QuadraticForm::new(0, vec![4], vec![bits(1, 0)]),
        Err(QuadraticError::InvalidPhase)
    );
    assert_eq!(
        QuadraticForm::new(0, vec![0], vec![bits(1, 1)]),
        Err(QuadraticError::InvalidUpperTriangle)
    );
    assert_eq!(
        QuadraticForm::new(0, vec![0], vec![]),
        Err(QuadraticError::DimensionMismatch)
    );
    let q = form(3, 0, 0, 0);
    assert_eq!(
        q.evaluate(&bits(2, 0)),
        Err(QuadraticError::DimensionMismatch)
    );
    assert_eq!(
        q.restrict(&bits(3, 0), &[bits(2, 0)]),
        Err(QuadraticError::DimensionMismatch)
    );
    assert_eq!(
        q.restrict(&bits(2, 0), &[]),
        Err(QuadraticError::DimensionMismatch)
    );
}

#[test]
fn packed_affine_restrictions_match_literal_wide_phase_polynomials() {
    // Evaluate the polynomial with integer sums, independently of packed polar
    // images, affine restriction and QuadraticForm::evaluate.
    let mut rng = 0x57a129fedbc_u64;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    for width in [0, 1, 63, 64, 65, 128, 129] {
        let constant = (next() & 3) as u8;
        let linear = (0..width).map(|_| (next() & 3) as u8).collect::<Vec<_>>();
        let edges = (0..width)
            .flat_map(|i| (i + 1..width).map(move |j| (i, j)))
            .filter(|_| next() & 3 == 0)
            .collect::<Vec<_>>();
        let upper = (0..width)
            .map(|i| {
                BitVector::from_indices(
                    width,
                    edges.iter().filter_map(|&(a, b)| (a == i).then_some(b)),
                )
                .unwrap()
            })
            .collect();
        let q = QuadraticForm::new(constant, linear.clone(), upper).unwrap();
        for mode in 0..3 {
            let shift =
                BitVector::from_indices(width, (0..width).filter(|_| next() & 1 != 0)).unwrap();
            let mut basis = (0..width)
                .map(|j| {
                    BitVector::from_indices(
                        width,
                        (0..width).filter(|&i| match mode {
                            0 => i == j,
                            1 => i == j || i == (j + 64) % width,
                            _ => next() & 1 != 0,
                        }),
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>();
            basis.push(BitVector::zeros(width).unwrap());
            if width > 0 {
                basis.push(basis[width / 2].clone());
            }
            let restricted = q.restrict(&shift, &basis).unwrap();
            for trial in 0..40 {
                let y = BitVector::from_indices(
                    basis.len(),
                    (0..basis.len()).filter(|_| trial == 0 || next() & 1 != 0),
                )
                .unwrap();
                let mut x = (0..width)
                    .map(|i| shift.get(i).unwrap())
                    .collect::<Vec<_>>();
                for j in y.ones() {
                    for (i, bit) in x.iter_mut().enumerate() {
                        *bit ^= basis[j].get(i).unwrap();
                    }
                }
                let expected = (usize::from(constant)
                    + linear
                        .iter()
                        .enumerate()
                        .map(|(i, &c)| usize::from(c) * usize::from(x[i]))
                        .sum::<usize>()
                    + 2 * edges.iter().filter(|&&(i, j)| x[i] && x[j]).count())
                    % 4;
                assert_eq!(
                    restricted.evaluate(&y).unwrap(),
                    expected as u8,
                    "width={width}, mode={mode}, trial={trial}"
                );
            }
        }
    }
}

#[test]
fn prepared_gauss_sums_match_every_small_form_and_compatible_update() {
    use rmagna::quadratic::PreparedQuadraticSum;
    let a = ScalarArithmetic::default();
    for width in 0_usize..=3 {
        let edges = width * width.saturating_sub(1) / 2;
        for constant in 0..4 {
            for linear in 0..1 << (2 * width) {
                for edge in 0..1 << edges {
                    let reference = form(width, constant, linear, edge);
                    let prepared = PreparedQuadraticSum::new(&reference).unwrap();
                    for even in 0..1 << width {
                        let mask = (0..width)
                            .map(|j| ((even >> j) & 1) << (2 * j + 1))
                            .sum::<u64>();
                        for updated_constant in 0..4 {
                            let updated = form(width, updated_constant, linear ^ mask, edge);
                            assert_eq!(
                                prepared.evaluate(&updated, &a).unwrap(),
                                direct_sum(&updated, &a),
                                "reference={reference:?}, updated={updated:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn prepared_gauss_sums_preserve_wide_phases_and_zero_reference_updates() {
    use rmagna::quadratic::PreparedQuadraticSum;
    let a = ScalarArithmetic::default();
    let zero_reference = form(3, 3, 2, 4);
    assert_eq!(direct_sum(&zero_reference, &a), ExactScalar::zero());
    let prepared = PreparedQuadraticSum::new(&zero_reference).unwrap();
    // The first variable is isolated and kills the reference. The remaining
    // pair must still be recorded before a compatible update removes that zero.
    let nonzero = form(3, 1, 0, 4);
    assert_eq!(
        prepared.evaluate(&nonzero, &a).unwrap(),
        a.omega(2, 4).unwrap()
    );
    assert_eq!(
        prepared.evaluate(&zero_reference, &a).unwrap(),
        ExactScalar::zero()
    );
    let mut rng = 0x18234facbed_u64;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    for width in [65, 129] {
        let mut upper = vec![BitVector::zeros(width).unwrap(); width];
        for j in (0..width - 1).step_by(2) {
            upper[j].set(j + 1, true).unwrap();
        }
        let mut linear = vec![0; width];
        linear[width - 1] = 1;
        let reference = QuadraticForm::new(3, linear, upper.clone()).unwrap();
        let prepared = PreparedQuadraticSum::new(&reference).unwrap();
        for _ in 0..64 {
            let constant = (next() & 3) as u8;
            let mut linear = (0..width)
                .map(|_| 2 * (next() & 1) as u8)
                .collect::<Vec<_>>();
            linear[width - 1] += 1;
            let mut phase = 2 * i64::from(constant) + if linear[width - 1] == 1 { 1 } else { -1 };
            for j in (0..width - 1).step_by(2) {
                phase += 4 * i64::from(linear[j] == 2 && linear[j + 1] == 2);
            }
            let updated = QuadraticForm::new(constant, linear, upper.clone()).unwrap();
            assert_eq!(
                prepared.evaluate(&updated, &a).unwrap(),
                a.omega(phase, width as i64).unwrap()
            );
        }
    }
    let reference = form(3, 0, 0, 0);
    let prepared = PreparedQuadraticSum::new(&reference).unwrap();
    for incompatible in [form(2, 0, 0, 0), form(3, 0, 1, 0), form(3, 0, 0, 1)] {
        assert_eq!(
            prepared.evaluate(&incompatible, &a),
            Err(QuadraticError::IncompatiblePreparation)
        );
    }
    let tight = ScalarArithmetic::new(1).unwrap();
    // Canonical Gaussian values require at most one coefficient bit here.
    assert_eq!(
        prepared.evaluate(&reference, &tight).unwrap(),
        a.omega(0, 6).unwrap()
    );
}
