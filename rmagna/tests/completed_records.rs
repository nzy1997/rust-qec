//! Complete original NativeRecordTests contracts and independent exact vectors.
use rmagna::gf2::BitVector;
use rmagna::noise::{RandomSource, SplitMix64};
use rmagna::pauli::PauliWord;
use rmagna::quadratic::QuadraticForm;
use rmagna::records::{
    CompletedRecordError as Error, CompletedRecordLimits as Limits, CompletedRecordSpec as Spec,
    CompletedSamplingOptions as Sampling, SamplingError,
};
use rmagna::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use rmagna::stabilizer::AffineKet;

fn bits(width: usize, mask: usize) -> BitVector {
    BitVector::from_indices(
        width,
        (0..width.min(usize::BITS as usize)).filter(|j| mask >> j & 1 != 0),
    )
    .unwrap()
}
fn q(linear: &[u8], upper: &[usize]) -> QuadraticForm {
    QuadraticForm::new(
        0,
        linear.to_vec(),
        upper.iter().map(|&v| bits(linear.len(), v)).collect(),
    )
    .unwrap()
}
fn scalar(a: &ScalarArithmetic, c: [i32; 4], power: i64) -> ExactScalar {
    a.scalar(c.map(Into::into), power).unwrap()
}
fn scaled(ket: &AffineKet, factor: &ExactScalar, a: &ScalarArithmetic) -> AffineKet {
    AffineKet::new(
        ket.offset().clone(),
        ket.columns().to_vec(),
        ket.phase().clone(),
        a.multiply(ket.coefficient(), factor).unwrap(),
    )
    .unwrap()
}
fn native_dense_terms(a: &ScalarArithmetic) -> Vec<AffineKet> {
    // Original Gaussian coefficients omega/sqrt(2), -i/sqrt(2), combined
    // with the original ket global phases omega^3 and omega^6 respectively.
    vec![
        AffineKet::new(
            bits(3, 0),
            vec![bits(3, 1), bits(3, 2), bits(3, 4)],
            q(&[1, 2, 0], &[2, 4, 0]),
            a.omega(4, -4).unwrap(),
        )
        .unwrap(),
        AffineKet::new(
            bits(3, 1),
            vec![bits(3, 3), bits(3, 4)],
            q(&[3, 1], &[2, 0]),
            a.omega(4, -3).unwrap(),
        )
        .unwrap(),
    ]
}
fn literal_parts(a: &ScalarArithmetic) -> (Vec<ExactScalar>, Vec<ExactScalar>) {
    // Literal eight-component vectors, independent of all affine/quadratic
    // evaluation, GF(2) elimination and completed-pair kernels.
    let left = [
        [-1, 0, 0, 0],
        [0, 0, -1, 0],
        [1, 0, 0, 0],
        [0, 0, -1, 0],
        [-1, 0, 0, 0],
        [0, 0, -1, 0],
        [-1, 0, 0, 0],
        [0, 0, 1, 0],
    ];
    let right = [
        [0, 0, 0, 0],
        [0, -1, 0, 1],
        [0, 1, 0, 1],
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        [0, -1, 0, -1],
        [0, 1, 0, -1],
        [0, 0, 0, 0],
    ];
    (
        left.map(|c| scalar(a, c, -2)).to_vec(),
        right.map(|c| scalar(a, c, -2)).to_vec(),
    )
}
fn literal_moments(
    psi: &[ExactScalar],
    labels: &[usize],
    event: Option<(usize, bool)>,
    x: usize,
    z: usize,
    phase: u8,
    a: &ScalarArithmetic,
) -> Vec<ExactScalar> {
    let mut values = vec![ExactScalar::zero(); 1 << labels.len()];
    for word in 0..8 {
        if event.is_some_and(|(mask, bit)| ((word & mask).count_ones() % 2 != 0) != bit) {
            continue;
        }
        let label = labels.iter().enumerate().fold(0, |v, (j, &mask)| {
            v | (((word & mask).count_ones() as usize & 1) << j)
        });
        let sign = if (z & (word ^ x)).count_ones().is_multiple_of(2) {
            0
        } else {
            4
        };
        let factor = a.omega(i64::from(phase) + sign, 0).unwrap();
        let value = a
            .multiply(
                &psi[word].conjugate(),
                &a.multiply(&factor, &psi[word ^ x]).unwrap(),
            )
            .unwrap();
        values[label] = a.add(&values[label], &value).unwrap();
    }
    values
}
fn t_terms(a: &ScalarArithmetic) -> Vec<AffineKet> {
    vec![
        AffineKet::new(
            bits(1, 0),
            vec![bits(1, 1)],
            q(&[0], &[0]),
            a.omega(0, -2).unwrap(),
        )
        .unwrap(),
        AffineKet::new(
            bits(1, 0),
            vec![bits(1, 1)],
            q(&[2], &[0]),
            a.omega(1, -2).unwrap(),
        )
        .unwrap(),
    ]
}
fn t_values(a: &ScalarArithmetic) -> [ExactScalar; 2] {
    [scalar(a, [2, 1, 0, -1], -2), scalar(a, [2, -1, 0, 1], -2)]
}

#[test]
fn original_dense_events_and_hermitian_moments() {
    let a = ScalarArithmetic::default();
    let terms = native_dense_terms(&a);
    let (left, right) = literal_parts(&a);
    let psi = left
        .iter()
        .zip(&right)
        .map(|(l, r)| a.add(l, r).unwrap())
        .collect::<Vec<_>>();
    for (word, expected) in psi.iter().enumerate() {
        let actual = terms.iter().fold(ExactScalar::zero(), |v, k| {
            a.add(
                &v,
                &k.prepare().unwrap().amplitude(&bits(3, word), &a).unwrap(),
            )
            .unwrap()
        });
        assert_eq!(&actual, expected);
    }
    for event in [false, true] {
        let spec = Spec::new(
            3,
            &[bits(3, 1), bits(3, 2)],
            &[bits(3, 3)],
            &bits(1, usize::from(event)),
            Limits::default(),
        )
        .unwrap();
        for (x, z) in [(0_usize, 0_usize), (0, 4), (4, 0), (4, 4)] {
            let phase = 2 * (x & z).count_ones() as u8;
            let pauli = PauliWord::new(bits(3, x), bits(3, z), phase).unwrap();
            let values = spec.moment(&terms, &pauli, &a).unwrap();
            let expected = literal_moments(&psi, &[1, 2], Some((3, event)), x, z, phase, &a);
            for (word, v) in expected.iter().enumerate() {
                assert_eq!(values.value(&bits(2, word), &a).unwrap(), *v);
            }
            let total = expected
                .iter()
                .fold(ExactScalar::zero(), |v, x| a.add(&v, x).unwrap());
            assert_eq!(values.total(&a).unwrap(), total);
            assert_eq!(
                values.mass(&[bits(2, 1)], &bits(1, 1), &a).unwrap(),
                a.add(&expected[1], &expected[3]).unwrap()
            );
            assert_eq!(values.statistics().source_entries, 2);
            assert_eq!(values.statistics().pair_work, 4);
            assert!(values.statistics().retained_terms <= 4);
            if x == 0 && z == 0 {
                let probability = spec.probability(&terms, &a).unwrap();
                assert_eq!(probability.total(&a).unwrap(), total);
                for (word, v) in expected.iter().enumerate() {
                    assert_eq!(probability.value(&bits(2, word), &a).unwrap(), *v);
                }
            }
        }
    }
}

#[test]
fn original_sampling_completed_marginal_2048_draws() {
    let a = ScalarArithmetic::default();
    let values = t_values(&a);
    let marginal = Spec::new(1, &[bits(1, 1)], &[], &bits(0, 0), Limits::default())
        .unwrap()
        .probability(&t_terms(&a), &a)
        .unwrap();
    assert_eq!(marginal.total(&a).unwrap(), ExactScalar::one());
    assert_eq!(marginal.value(&bits(1, 0), &a).unwrap(), values[0]);
    assert_eq!(marginal.value(&bits(1, 1), &a).unwrap(), values[1]);
    let mut rng = SplitMix64::new(739);
    let mut count = 0;
    for _ in 0..2048 {
        let draw = marginal.sample(&mut rng, &Sampling::default(), &a).unwrap();
        let word = usize::from(draw.values.get(0).unwrap());
        count += word;
        assert_eq!(draw.mass, values[word]);
        assert_eq!(draw.condition_mass, ExactScalar::one());
        assert_eq!(draw.mass_queries, 2);
        assert_eq!(draw.gaussian_sums, 8);
        assert!(draw.entropy_words > 0);
    }
    let expected = (2.0 - 2.0_f64.sqrt()) / 4.0;
    assert!(
        (count as f64 / 2048.0 - expected).abs() < 0.04,
        "count={count}"
    );
}

#[test]
fn original_reject_noncommuting_pauli() {
    let a = ScalarArithmetic::default();
    let plus = AffineKet::new(
        bits(1, 0),
        vec![bits(1, 1)],
        q(&[0], &[0]),
        a.omega(0, -1).unwrap(),
    )
    .unwrap();
    let plan = Spec::new(1, &[bits(1, 1)], &[], &bits(0, 0), Limits::default()).unwrap();
    let error = plan
        .moment(
            &[plus],
            &PauliWord::new(bits(1, 1), bits(1, 0), 0).unwrap(),
            &a,
        )
        .unwrap_err();
    assert_eq!(
        error,
        Error::NonCommutingPauli {
            kind: "record",
            index: 0
        }
    );
}

#[test]
fn dependent_labels_all_common_phases_and_relative_complex_weights_match_literal_vectors() {
    let a = ScalarArithmetic::default();
    let source = native_dense_terms(&a);
    let (left, right) = literal_parts(&a);
    let labels = [3, 5, 6, 3, 0];
    let rows = labels.map(|mask| bits(3, mask));
    let pauli = PauliWord::new(bits(3, 7), bits(3, 4), 2).unwrap();
    for global in 0..8 {
        for relative in [0, 1, 3] {
            let common = a.omega(global, 0).unwrap();
            let rel = a.omega(relative, 0).unwrap();
            let terms = [
                scaled(&source[0], &common, &a),
                scaled(&source[1], &a.multiply(&common, &rel).unwrap(), &a),
            ];
            let psi = left
                .iter()
                .zip(&right)
                .map(|(l, r)| {
                    a.multiply(&common, &a.add(l, &a.multiply(&rel, r).unwrap()).unwrap())
                        .unwrap()
                })
                .collect::<Vec<_>>();
            let spec = Spec::new(3, &rows, &[], &bits(0, 0), Limits::default()).unwrap();
            let probability = spec.probability(&terms, &a).unwrap();
            let moment = spec.moment(&terms, &pauli, &a).unwrap();
            let probabilities = literal_moments(&psi, &labels, None, 0, 0, 0, &a);
            let moments = literal_moments(&psi, &labels, None, 7, 4, 2, &a);
            for word in 0..32 {
                assert_eq!(
                    probability.value(&bits(5, word), &a).unwrap(),
                    probabilities[word]
                );
                assert_eq!(moment.value(&bits(5, word), &a).unwrap(), moments[word]);
            }
            assert_eq!(probability.total(&a).unwrap(), ExactScalar::one());
            assert_eq!(
                probability
                    .mass(&[bits(5, 1), bits(5, 1)], &bits(2, 1), &a)
                    .unwrap(),
                ExactScalar::zero()
            );
            assert_eq!(
                probability.mass(&[bits(5, 0)], &bits(1, 1), &a).unwrap(),
                ExactScalar::zero()
            );
        }
    }
}

struct NoEntropy;
impl RandomSource for NoEntropy {
    fn next_u64(&mut self) -> u64 {
        panic!("deterministic or refused draws must not read entropy")
    }
}

#[test]
fn subnormalization_cancellation_vacuum_and_signed_moments_are_not_renormalized() {
    let a = ScalarArithmetic::default();
    let terms = t_terms(&a)
        .iter()
        .map(|t| scaled(t, &a.omega(0, -2).unwrap(), &a))
        .collect::<Vec<_>>();
    let expected = t_values(&a).map(|v| a.multiply(&v, &a.omega(0, -4).unwrap()).unwrap());
    let marginal = Spec::new(1, &[bits(1, 1)], &[], &bits(0, 0), Limits::default())
        .unwrap()
        .probability(&terms, &a)
        .unwrap();
    assert_eq!(marginal.total(&a).unwrap(), a.omega(0, -4).unwrap());
    let mut captured = bits(1, 0);
    let no_records = Spec::new(1, &[], &[bits(1, 1)], &captured, Limits::default()).unwrap();
    captured.set(0, true).unwrap();
    let no_records = no_records.probability(&terms, &a).unwrap();
    assert_eq!(no_records.total(&a).unwrap(), expected[0]);
    let unreported = no_records
        .sample(
            &mut NoEntropy,
            &Sampling {
                max_entropy_words: 0,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
    assert_eq!(unreported.values.width(), 0);
    assert_eq!(unreported.mass, expected[0]);
    assert_eq!(unreported.condition_mass, expected[0]);
    assert_eq!(unreported.entropy_words, 0);
    for (bit, expected_mass) in expected.iter().enumerate() {
        assert_eq!(&marginal.value(&bits(1, bit), &a).unwrap(), expected_mass);
        let spec = Spec::new(
            1,
            &[bits(1, 1)],
            &[bits(1, 1)],
            &bits(1, bit),
            Limits::default(),
        )
        .unwrap();
        let conditioned = spec.probability(&terms, &a).unwrap();
        let draw = conditioned
            .sample(
                &mut NoEntropy,
                &Sampling {
                    max_entropy_words: 0,
                    ..Default::default()
                },
                &a,
            )
            .unwrap();
        assert_eq!(draw.values, bits(1, bit));
        assert_eq!(draw.mass, *expected_mass);
        assert_eq!(draw.condition_mass, *expected_mass);
        assert_eq!(draw.entropy_words, 0);
        let negative = spec
            .moment(
                &terms,
                &PauliWord::new(bits(1, 0), bits(1, 0), 4).unwrap(),
                &a,
            )
            .unwrap();
        assert_eq!(negative.total(&a).unwrap(), expected_mass.negated());
        assert_eq!(
            negative.value(&bits(1, bit), &a).unwrap(),
            expected_mass.negated()
        );
    }
    let vacuum = AffineKet::new(bits(0, 0), vec![], q(&[], &[]), a.omega(3, 0).unwrap()).unwrap();
    let plan = Spec::new(0, &[], &[], &bits(0, 0), Limits::default()).unwrap();
    let empty = plan.probability(std::slice::from_ref(&vacuum), &a).unwrap();
    assert_eq!(empty.total(&a).unwrap(), ExactScalar::one());
    let draw = empty
        .sample(
            &mut NoEntropy,
            &Sampling {
                max_entropy_words: 0,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
    assert_eq!(draw.values.width(), 0);
    assert_eq!(draw.mass, ExactScalar::one());
    assert_eq!(draw.mass_queries, 1);
    let hidden_zero = AffineKet::new(
        bits(0, 0),
        vec![bits(0, 0)],
        q(&[2], &[0]),
        ExactScalar::one(),
    )
    .unwrap();
    for zero in [
        vec![],
        vec![vacuum.clone(), scaled(&vacuum, &a.integer(-1).unwrap(), &a)],
        vec![scaled(&vacuum, &ExactScalar::zero(), &a)],
        vec![hidden_zero],
    ] {
        let marginal = plan.probability(&zero, &a).unwrap();
        assert_eq!(marginal.total(&a).unwrap(), ExactScalar::zero());
        assert!(matches!(
            marginal.sample(&mut NoEntropy, &Sampling::default(), &a),
            Err(Error::Sampling(SamplingError::ImpossibleCondition))
        ));
    }
    let inconsistent = Spec::new(
        1,
        &[bits(1, 1)],
        &[bits(1, 1), bits(1, 1)],
        &bits(2, 1),
        Limits::default(),
    )
    .unwrap()
    .probability(&terms, &a)
    .unwrap();
    assert_eq!(inconsistent.total(&a).unwrap(), ExactScalar::zero());
}

#[test]
fn wide_fixed_support_fits_path_geometry_below_the_old_square_mask_payload() {
    let a = ScalarArithmetic::default();
    let width: usize = 16385;
    let cap: usize = 1 << 22;
    assert!((width as u128) * (2 * width).div_ceil(64) as u128 > cap as u128);
    let col1 = BitVector::from_indices(width, [0, width - 1]).unwrap();
    let col2 = BitVector::from_indices(width, [64, 128]).unwrap();
    let terms = [
        AffineKet::new(
            BitVector::zeros(width).unwrap(),
            vec![col1.clone(), col2.clone()],
            q(&[1, 3], &[2, 0]),
            a.omega(3, -3).unwrap(),
        )
        .unwrap(),
        AffineKet::new(
            BitVector::from_indices(width, [width - 1]).unwrap(),
            vec![col1, col2],
            q(&[2, 1], &[0, 0]),
            a.omega(5, -3).unwrap(),
        )
        .unwrap(),
    ];
    let row = BitVector::from_indices(width, [width - 1]).unwrap();
    let event = BitVector::from_indices(width, [0]).unwrap();
    let spec = Spec::new(
        width,
        std::slice::from_ref(&row),
        &[event],
        &bits(1, 0),
        Limits {
            max_geometry_words: cap,
            ..Default::default()
        },
    )
    .unwrap();
    let probability = spec.probability(&terms, &a).unwrap();
    let z = PauliWord::new(BitVector::zeros(width).unwrap(), row, 0).unwrap();
    let signed = spec.moment(&terms, &z, &a).unwrap();
    for bit in 0..2 {
        assert_eq!(
            probability.value(&bits(1, bit), &a).unwrap(),
            a.omega(0, -4).unwrap()
        );
        assert_eq!(
            signed.value(&bits(1, bit), &a).unwrap(),
            a.omega(if bit == 0 { 0 } else { 4 }, -4).unwrap()
        );
    }
    assert_eq!(probability.total(&a).unwrap(), a.omega(0, -2).unwrap());
    assert_eq!(signed.total(&a).unwrap(), ExactScalar::zero());
    assert_eq!(probability.statistics().pair_work, 4);
    assert_eq!(probability.statistics().retained_terms, 2);
    assert!(probability.statistics().estimated_geometry_words <= cap as u128);
}

#[test]
fn finite_dimensions_entries_pairs_paths_geometry_work_and_queries_fail_without_altering_inputs() {
    let a = ScalarArithmetic::default();
    let terms = t_terms(&a);
    let rows = [bits(1, 1)];
    assert_eq!(
        Spec::new(usize::MAX, &[], &[], &bits(0, 0), Limits::default()).unwrap_err(),
        Error::DimensionOverflow
    );
    for (badrows, events, outcomes) in [
        (vec![bits(2, 1)], vec![], bits(0, 0)),
        (vec![bits(1, 1)], vec![bits(2, 1)], bits(1, 0)),
        (vec![], vec![bits(1, 1)], bits(0, 0)),
    ] {
        assert_eq!(
            Spec::new(1, &badrows, &events, &outcomes, Limits::default()).unwrap_err(),
            Error::DimensionMismatch
        );
    }
    assert!(matches!(
        Spec::new(
            1,
            &[bits(1, 1), bits(2, 1)],
            &[],
            &bits(0, 0),
            Limits::default()
        ),
        Err(Error::DimensionMismatch)
    ));
    for (option, limits) in [
        (
            "max_record_rows",
            Limits {
                max_record_rows: 0,
                ..Default::default()
            },
        ),
        (
            "max_geometry_words",
            Limits {
                max_geometry_words: 0,
                ..Default::default()
            },
        ),
    ] {
        assert!(
            matches!(Spec::new(1,&rows,&[],&bits(0,0),limits),Err(Error::ResourceLimit{option:got,..})if got==option)
        );
    }
    assert!(matches!(
        Spec::new(
            1,
            &[],
            &rows,
            &bits(1, 0),
            Limits {
                max_event_rows: 0,
                ..Default::default()
            }
        ),
        Err(Error::ResourceLimit {
            option: "max_event_rows",
            ..
        })
    ));
    for (option, limits) in [
        (
            "max_source_entries",
            Limits {
                max_source_entries: 1,
                ..Default::default()
            },
        ),
        (
            "max_pairs",
            Limits {
                max_pairs: 3,
                ..Default::default()
            },
        ),
        (
            "max_pair_paths",
            Limits {
                max_pair_paths: 1,
                ..Default::default()
            },
        ),
        (
            "max_geometry_words",
            Limits {
                max_geometry_words: 140,
                ..Default::default()
            },
        ),
        (
            "max_construction_work",
            Limits {
                max_construction_work: 0,
                ..Default::default()
            },
        ),
    ] {
        let spec = Spec::new(1, &rows, &[], &bits(0, 0), limits).unwrap();
        assert!(
            matches!(spec.probability(&terms,&a),Err(Error::ResourceLimit{option:got,..})if got==option)
        );
    }
    let spec = Spec::new(1, &rows, &[], &bits(0, 0), Limits::default()).unwrap();
    let wrong = AffineKet::new(bits(2, 0), vec![], q(&[], &[]), ExactScalar::one()).unwrap();
    assert!(matches!(
        spec.probability(&[terms[0].clone(), wrong], &a),
        Err(Error::DimensionMismatch)
    ));
    assert!(matches!(
        spec.moment(&terms, &PauliWord::identity(2).unwrap(), &a),
        Err(Error::DimensionMismatch)
    ));
    assert!(matches!(
        spec.moment(
            &terms,
            &PauliWord::new(bits(1, 0), bits(1, 0), 1).unwrap(),
            &a
        ),
        Err(Error::NonHermitianPauli)
    ));
    let event_only = Spec::new(1, &[], &rows, &bits(1, 0), Limits::default()).unwrap();
    assert!(matches!(
        event_only.moment(
            &terms,
            &PauliWord::new(bits(1, 1), bits(1, 0), 0).unwrap(),
            &a
        ),
        Err(Error::NonCommutingPauli {
            kind: "event",
            index: 0
        })
    ));
    let probability = spec.probability(&terms, &a).unwrap();
    assert!(matches!(
        probability.value(&bits(2, 0), &a),
        Err(Error::DimensionMismatch)
    ));
    assert!(matches!(
        probability.mass(&[bits(2, 1)], &bits(1, 0), &a),
        Err(Error::DimensionMismatch)
    ));
    assert!(matches!(
        probability.mass(&[bits(1, 1), bits(2, 1)], &bits(2, 0), &a),
        Err(Error::DimensionMismatch)
    ));
    assert!(matches!(
        probability.mass(&[bits(1, 1)], &bits(0, 0), &a),
        Err(Error::DimensionMismatch)
    ));
    assert!(matches!(
        probability.value(&bits(1, 0), &ScalarArithmetic::new(1).unwrap()),
        Err(Error::Scalar(ScalarError::CoefficientLimit { .. }))
    ));
    let only_total = Spec::new(
        1,
        &rows,
        &[],
        &bits(0, 0),
        Limits {
            max_query_rows: 0,
            ..Default::default()
        },
    )
    .unwrap()
    .probability(&terms, &a)
    .unwrap();
    assert_eq!(only_total.total(&a).unwrap(), ExactScalar::one());
    assert!(matches!(
        only_total.value(&bits(1, 0), &a),
        Err(Error::ResourceLimit {
            option: "max_query_rows",
            ..
        })
    ));
    assert!(matches!(
        only_total.sample(&mut NoEntropy, &Sampling::default(), &a),
        Err(Error::ResourceLimit {
            option: "max_query_rows",
            ..
        })
    ));
    assert_eq!(
        spec.probability(&terms, &a).unwrap().total(&a).unwrap(),
        ExactScalar::one()
    );
}

#[test]
fn sampling_admission_entropy_failures_continuation_and_owned_results_are_explicit() {
    let a = ScalarArithmetic::default();
    let probability = {
        let mut rows = vec![bits(1, 1)];
        let spec = Spec::new(1, &rows, &[], &bits(0, 0), Limits::default()).unwrap();
        rows[0] = bits(1, 0);
        let terms = t_terms(&a);
        spec.probability(&terms, &a).unwrap()
    };
    let values = t_values(&a);
    for (bit, v) in values.iter().enumerate() {
        assert_eq!(probability.value(&bits(1, bit), &a).unwrap(), *v);
    }
    assert!(matches!(
        probability.sample(
            &mut NoEntropy,
            &Sampling {
                max_mass_queries: 1,
                ..Default::default()
            },
            &a
        ),
        Err(Error::ResourceLimit {
            option: "max_mass_queries",
            ..
        })
    ));
    assert!(matches!(
        probability.sample(
            &mut NoEntropy,
            &Sampling {
                max_entropy_words: 0,
                ..Default::default()
            },
            &a
        ),
        Err(Error::Sampling(SamplingError::EntropyLimit { limit: 0 }))
    ));
    struct Boundary {
        reads: usize,
    }
    impl RandomSource for Boundary {
        fn next_u64(&mut self) -> u64 {
            self.reads += 1;
            if self.reads == 1 {
                0xDA827999FCEF3242
            } else {
                0
            }
        }
    }
    let mut boundary = Boundary { reads: 0 };
    assert!(matches!(
        probability.sample(
            &mut boundary,
            &Sampling {
                max_entropy_words: 1,
                ..Default::default()
            },
            &a
        ),
        Err(Error::Sampling(SamplingError::EntropyLimit { limit: 1 }))
    ));
    assert_eq!(boundary.reads, 1);
    let draw = probability
        .sample(&mut boundary, &Sampling::default(), &a)
        .unwrap();
    assert_eq!(boundary.reads, 2);
    assert_eq!(draw.values, bits(1, 0));
    assert_eq!(draw.mass, values[0]);
    let mut uninterrupted = SplitMix64::new(739);
    let mut resumed = SplitMix64::new(739);
    let all = (0..64)
        .map(|_| {
            probability
                .sample(&mut uninterrupted, &Sampling::default(), &a)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let mut chunks = (0..17)
        .map(|_| {
            probability
                .sample(&mut resumed, &Sampling::default(), &a)
                .unwrap()
        })
        .collect::<Vec<_>>();
    resumed = SplitMix64::new(resumed.state());
    chunks.extend((17..64).map(|_| {
        probability
            .sample(&mut resumed, &Sampling::default(), &a)
            .unwrap()
    }));
    assert_eq!(all, chunks);
    assert_eq!(uninterrupted.state(), resumed.state());
    assert_eq!(probability.total(&a).unwrap(), ExactScalar::one());
}
