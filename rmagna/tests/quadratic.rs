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
