// SPDX-License-Identifier: MPL-2.0
// Complete sampling scenarios from itpyi/magna 56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_file_native.py (SHA256 b078965d22ae894eb7b1bb9d16000d49ae43f7ccd5727074d05802d7b572e4a1).
// Seeded Boolean entropy is caller-owned; native MT stream identity is not asserted.
use super::OriginalRecordProbabilities;
use crate::gf2::BitVector;
use crate::instrument::{
    OriginalRecordDraw, OriginalRecordDrawOptions, OriginalRecordSamplingLimits,
    PreparedOriginalRecordSampling,
};
use crate::noise::{RandomSource, SplitMix64};
use crate::quadratic::QuadraticForm;
use crate::scalar::{ExactScalar, ScalarArithmetic};
use crate::stabilizer::AffineKet;

fn scalar(a: &ScalarArithmetic, coefficients: [i32; 4], power: i64) -> ExactScalar {
    a.scalar(coefficients.map(Into::into), power).unwrap()
}

fn line(width: usize, phase: u8, coefficient: ExactScalar) -> AffineKet {
    AffineKet::new(
        BitVector::zeros(width).unwrap(),
        vec![BitVector::from_indices(width, 0..width).unwrap()],
        QuadraticForm::new(0, vec![phase], vec![BitVector::zeros(1).unwrap()]).unwrap(),
        coefficient,
    )
    .unwrap()
}

fn point_one(coefficient: ExactScalar) -> AffineKet {
    AffineKet::new(
        BitVector::from_indices(1, [0]).unwrap(),
        vec![],
        QuadraticForm::new(0, vec![], vec![]).unwrap(),
        coefficient,
    )
    .unwrap()
}

fn draw(
    plan: &PreparedOriginalRecordSampling,
    seed: u64,
    a: &ScalarArithmetic,
) -> OriginalRecordDraw {
    let mut random = SplitMix64::new(seed);
    let mut calls = 0;
    let result = plan
        .sample_bits(
            OriginalRecordDrawOptions::default(),
            &mut || {
                calls += 1;
                random.next_u64() & 1 != 0
            },
            a,
        )
        .unwrap();
    assert_eq!(result.entropy_bits, calls);
    if calls == 0 {
        assert_eq!(random.state(), seed);
    }
    result
}

#[test]
fn affine_mixture_big_boundary_and_reproducibility() {
    let a = ScalarArithmetic::default();
    let half = scalar(&a, [1, 0, 0, 0], -1);
    let tensor = OriginalRecordProbabilities {
        terms: vec![line(130, 0, half.clone())],
    };
    let plan = tensor
        .prepare_affine_sampling(OriginalRecordSamplingLimits::default(), &a)
        .unwrap();
    assert_eq!(plan.total_mass(), &ExactScalar::one());
    assert_eq!(plan.support_bound().width(), 130);
    assert_eq!(plan.statistics().support_dimension, 1);
    let zero = BitVector::zeros(130).unwrap();
    let all = BitVector::from_indices(130, 0..130).unwrap();
    let draws = (0..256)
        .map(|seed| draw(&plan, seed, &a))
        .collect::<Vec<_>>();
    for result in &draws {
        assert!(result.label == zero || result.label == all);
        assert_eq!(result.label_mass, half);
        assert_eq!(result.total_mass, ExactScalar::one());
        assert_eq!(result.support_dimension, 1);
        assert_eq!(result.conditional_queries, 1);
        assert_eq!(result.entropy_bits, 1);
    }
    assert!(draws.iter().any(|result| result.label == zero));
    assert!(draws.iter().any(|result| result.label == all));
    let frequency = draws.iter().filter(|result| result.label == all).count() as f64 / 256.0;
    assert!((frequency - 0.5).abs() < 0.12);
    assert_eq!(
        draws,
        (0..256)
            .map(|seed| draw(&plan, seed, &a))
            .collect::<Vec<_>>()
    );
}

#[test]
fn exact_irrational_signed_global_marginal() {
    let a = ScalarArithmetic::default();
    let tensor = OriginalRecordProbabilities {
        terms: vec![
            line(1, 0, scalar(&a, [1, 0, 0, 0], -1)),
            line(1, 2, scalar(&a, [0, 1, 0, -1], -2)),
        ],
    };
    let plan = tensor
        .prepare_affine_sampling(OriginalRecordSamplingLimits::default(), &a)
        .unwrap();
    let masses = [scalar(&a, [2, 1, 0, -1], -2), scalar(&a, [2, -1, 0, 1], -2)];
    assert_eq!(a.add(&masses[0], &masses[1]).unwrap(), ExactScalar::one());
    assert_eq!(plan.total_mass(), &ExactScalar::one());
    assert_eq!(plan.statistics().support_dimension, 1);
    let mut ones = 0;
    for seed in 0..1024 {
        let result = draw(&plan, seed, &a);
        assert_eq!(result.label.width(), 1);
        let selected = result.label.get(0).unwrap();
        ones += usize::from(selected);
        assert_eq!(result.label_mass, masses[usize::from(selected)]);
        assert_eq!(result.total_mass, ExactScalar::one());
        assert_eq!(result.support_dimension, 1);
        assert_eq!(result.conditional_queries, 1);
        assert!(result.entropy_bits > 0);
    }
    assert!((ones as f64 / 1024.0 - (0.5 - 2_f64.sqrt() / 4.0)).abs() < 0.035);
}

#[test]
fn complex_terms_cancel_before_sampling() {
    let a = ScalarArithmetic::default();
    let coefficients = [scalar(&a, [1, 0, 1, 0], 0), scalar(&a, [0, 0, -1, 0], 0)];
    assert_eq!(
        a.add(&coefficients[0], &coefficients[1]).unwrap(),
        ExactScalar::one()
    );
    let squared = coefficients
        .iter()
        .map(|c| a.multiply(c, &c.conjugate()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        a.add(&squared[0], &squared[1]).unwrap(),
        scalar(&a, [3, 0, 0, 0], 0)
    );
    let tensor = OriginalRecordProbabilities {
        terms: coefficients.into_iter().map(point_one).collect(),
    };
    let plan = tensor
        .prepare_affine_sampling(OriginalRecordSamplingLimits::default(), &a)
        .unwrap();
    assert_eq!(plan.total_mass(), &ExactScalar::one());
    assert_eq!(plan.statistics().support_dimension, 0);
    for seed in 0..10 {
        let result = draw(&plan, seed, &a);
        assert_eq!(result.label, BitVector::from_indices(1, [0]).unwrap());
        assert_eq!(result.label_mass, ExactScalar::one());
        assert_eq!(result.total_mass, ExactScalar::one());
        assert_eq!(result.support_dimension, 0);
        assert_eq!(result.conditional_queries, 0);
        assert_eq!(result.entropy_bits, 0);
    }
}
