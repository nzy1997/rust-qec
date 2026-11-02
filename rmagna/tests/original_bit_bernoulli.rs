// SPDX-License-Identifier: MPL-2.0
// Original finite scenarios adapted from itpyi/magna, commit
// 56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_global_sampling.py: exact_comparisons_and_extreme_probabilities.
// License: fixtures/original_bit_bernoulli.LICENSE. Additional stopping oracles are independent.
use rmagna::noise::RandomSource;
use rmagna::records::{
    BinaryDraw, BitDraw, BitSamplingError, SamplingError, bernoulli_ratio, bernoulli_ratio_bits,
};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use sha2::{Digest, Sha256};
use std::{cmp::Ordering, collections::BTreeSet};

#[test]
fn original_2048_bit_calls_endpoints_and_one_bit_stopping() {
    let a = ScalarArithmetic::default();
    assert_eq!(
        a.scalar([3.into(), (-2).into(), 0.into(), 2.into()], 0)
            .unwrap()
            .real_sign()
            .unwrap(),
        Ordering::Greater
    );
    assert_eq!(
        a.scalar([1.into(), (-1).into(), 0.into(), 1.into()], 0)
            .unwrap()
            .real_sign()
            .unwrap(),
        Ordering::Less
    );
    let one = ExactScalar::one();
    let tiny = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -2048)
        .unwrap();
    let complement = a.subtract(&one, &tiny).unwrap();
    for (p, bit, answer) in [(&tiny, false, true), (&complement, true, false)] {
        let mut calls = 0;
        let draw = bernoulli_ratio_bits(
            p,
            &one,
            &mut || {
                calls += 1;
                bit
            },
            2048,
            &a,
        )
        .unwrap();
        assert_eq!(
            draw,
            BitDraw {
                selected: answer,
                entropy_bits: 2048
            }
        );
        assert_eq!(calls, 2048);
        let mut calls = 0;
        assert_eq!(
            bernoulli_ratio_bits(
                p,
                &one,
                &mut || {
                    calls += 1;
                    bit
                },
                2047,
                &a
            ),
            Err(BitSamplingError::EntropyLimit { limit: 2047 })
        );
        assert_eq!(calls, 2047);
    }
    for (p, answer) in [(&ExactScalar::zero(), false), (&one, true)] {
        assert_eq!(
            bernoulli_ratio_bits(p, &one, &mut || panic!("endpoint consumed entropy"), 0, &a)
                .unwrap(),
            BitDraw {
                selected: answer,
                entropy_bits: 0
            }
        );
    }
    let half = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -1)
        .unwrap();
    for bit in [false, true] {
        let mut calls = 0;
        let draw = bernoulli_ratio_bits(
            &half,
            &one,
            &mut || {
                calls += 1;
                bit
            },
            1,
            &a,
        )
        .unwrap();
        assert_eq!(
            draw,
            BitDraw {
                selected: !bit,
                entropy_bits: 1
            }
        );
        assert_eq!(calls, 1);
    }
}

#[test]
fn invalid_masses_and_resource_errors_keep_actual_entropy_consumption() {
    let a = ScalarArithmetic::default();
    let scalar = |x: i64| {
        a.scalar([x.into(), 0.into(), 0.into(), 0.into()], 0)
            .unwrap()
    };
    for (numerator, total) in [(0, 0), (1, -1), (-1, 1), (2, 1)] {
        assert_eq!(
            bernoulli_ratio_bits(
                &scalar(numerator),
                &scalar(total),
                &mut || panic!("invalid mass consumed entropy"),
                8,
                &a
            ),
            Err(BitSamplingError::Sampling(SamplingError::InvalidMass))
        );
    }
    let nonreal = a.omega(1, 0).unwrap();
    assert!(matches!(
        bernoulli_ratio_bits(
            &nonreal,
            &scalar(1),
            &mut || panic!("nonreal mass consumed entropy"),
            8,
            &a
        ),
        Err(BitSamplingError::Sampling(SamplingError::Scalar(_)))
    ));
    let tight = ScalarArithmetic::new(1).unwrap();
    assert!(matches!(
        bernoulli_ratio_bits(
            &scalar(0),
            &scalar(3),
            &mut || panic!("initial scalar failure consumed entropy"),
            8,
            &tight
        ),
        Err(BitSamplingError::Sampling(SamplingError::Scalar(_)))
    ));
    let refinement_budget = ScalarArithmetic::new(2).unwrap();
    let mut calls = 0;
    assert!(matches!(
        bernoulli_ratio_bits(
            &scalar(1),
            &scalar(3),
            &mut || {
                calls += 1;
                false
            },
            8,
            &refinement_budget
        ),
        Err(BitSamplingError::Sampling(SamplingError::Scalar(_)))
    ));
    assert_eq!(calls, 2);
}

#[test]
fn all_small_dyadic_decisions_match_integer_intervals() {
    let a = ScalarArithmetic::default();
    let mut cases = 0;
    for depth in 1..=6 {
        let denominator = 1u64 << depth;
        for threshold in 0..=denominator {
            let p = a
                .scalar(
                    [threshold.into(), 0.into(), 0.into(), 0.into()],
                    -i64::from(depth),
                )
                .unwrap();
            for word in 0..denominator {
                let mut prefix = 0;
                let mut consumed = 0;
                let expected = loop {
                    let scaled = threshold << consumed;
                    if prefix * denominator >= scaled {
                        break false;
                    }
                    if (prefix + 1) * denominator <= scaled {
                        break true;
                    }
                    prefix = (prefix << 1) | ((word >> (depth - 1 - consumed)) & 1);
                    consumed += 1;
                };
                let mut calls = 0;
                let actual = bernoulli_ratio_bits(
                    &p,
                    &ExactScalar::one(),
                    &mut || {
                        let bit = word >> (depth - 1 - calls) & 1 != 0;
                        calls += 1;
                        bit
                    },
                    depth as usize,
                    &a,
                )
                .unwrap();
                assert_eq!(actual.selected, expected);
                assert_eq!(actual.entropy_bits, consumed as usize);
                assert_eq!(calls, consumed);
                cases += 1;
            }
        }
    }
    println!("{cases} literal dyadic stopping cases passed");
}

struct Words {
    word: u64,
    calls: usize,
}
impl RandomSource for Words {
    fn next_u64(&mut self) -> u64 {
        self.calls += 1;
        self.word
    }
}

#[test]
fn pre_refactor_word_results_and_consumption_remain_unchanged() {
    // Captured from the old word API before this refactor. Metadata identifies
    // that capture; the semantic rows are checked independently on every host.
    let frozen = include_bytes!("fixtures/original_word_bernoulli.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(frozen)),
        "decf9cc6b922d0e4b1be4efeda3409a1749c059d59009dbfda1b9cddee1f63bb"
    );
    let fixture: serde_json::Value = serde_json::from_slice(frozen).unwrap();
    assert_eq!(
        fixture["schema"],
        "rmagna.word-bernoulli-before-bit-refactor.v1"
    );
    let a = ScalarArithmetic::default();
    let one = ExactScalar::one();
    let tiny = a
        .scalar([1.into(), 0.into(), 0.into(), 0.into()], -2048)
        .unwrap();
    let complement = a.subtract(&one, &tiny).unwrap();
    let root = a
        .scalar([0.into(), 1.into(), 0.into(), (-1).into()], -1)
        .unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 80);
    let mut seen = BTreeSet::new();
    for case in cases {
        let label = case["probability"].as_str().unwrap();
        let probability = match label {
            "zero" => ExactScalar::zero(),
            "one" => one.clone(),
            "tiny_2_pow_neg2048" => tiny.clone(),
            "one_minus_tiny" => complement.clone(),
            "sqrt2_over2" => root.clone(),
            other => panic!("unknown frozen probability {other}"),
        };
        let word = case["word"].as_u64().unwrap();
        let cap = usize::try_from(case["cap"].as_u64().unwrap()).unwrap();
        assert!([0, 1, u64::MAX / 2, u64::MAX].contains(&word));
        assert!([0, 1, 2, 32].contains(&cap));
        assert!(seen.insert((label, word, cap)));
        let mut source = Words { word, calls: 0 };
        let result = bernoulli_ratio(&probability, &one, &mut source, cap, &a);
        match case["status"].as_str().unwrap() {
            "draw" => assert_eq!(
                result,
                Ok(BinaryDraw {
                    selected: case["selected"].as_bool().unwrap(),
                    entropy_words: usize::try_from(case["entropy_words"].as_u64().unwrap())
                        .unwrap(),
                })
            ),
            "entropy_limit" => {
                assert!(case["selected"].is_null());
                assert!(case["entropy_words"].is_null());
                assert_eq!(result, Err(SamplingError::EntropyLimit { limit: cap }));
            }
            other => panic!("unknown frozen status {other}"),
        }
        assert_eq!(
            source.calls,
            usize::try_from(case["rng_calls"].as_u64().unwrap()).unwrap()
        );
    }
    assert_eq!(seen.len(), 80);
}

#[test]
fn irrational_and_nonunit_draws_preserve_exact_stops_and_caller_continuations() {
    let a = ScalarArithmetic::default();
    let one = ExactScalar::one();
    let three = a.integer(3).unwrap();
    let root = a
        .scalar([0.into(), 1.into(), 0.into(), (-1).into()], -1)
        .unwrap();
    let scaled = a.multiply(&three, &root).unwrap();
    let complement = a.subtract(&one, &root).unwrap();
    for (numerator, total, bit, answer, count) in [
        (&root, &one, false, true, 1),
        (&root, &one, true, false, 2),
        (&scaled, &three, false, true, 1),
        (&scaled, &three, true, false, 2),
        (&complement, &one, false, true, 2),
        (&complement, &one, true, false, 1),
    ] {
        let mut calls = 0;
        let draw = bernoulli_ratio_bits(
            numerator,
            total,
            &mut || {
                calls += 1;
                bit
            },
            8,
            &a,
        )
        .unwrap();
        assert_eq!(
            draw,
            BitDraw {
                selected: answer,
                entropy_bits: count
            }
        );
        assert_eq!(calls, count);
    }
    let eight = a.integer(8).unwrap();
    let mut source = [false, true, false, true].into_iter();
    let mut calls = 0;
    let (first, second) = {
        let mut random = || {
            calls += 1;
            source.next().expect("exact caller bit stream")
        };
        (
            bernoulli_ratio_bits(&three, &eight, &mut random, 4, &a).unwrap(),
            bernoulli_ratio_bits(&three, &eight, &mut random, 4, &a).unwrap(),
        )
    };
    assert_eq!(
        first,
        BitDraw {
            selected: true,
            entropy_bits: 3
        }
    );
    assert_eq!(
        second,
        BitDraw {
            selected: false,
            entropy_bits: 1
        }
    );
    assert_eq!(calls, 4);
    assert!(source.next().is_none());
    assert_eq!(
        BitSamplingError::EntropyLimit { limit: 7 }.to_string(),
        "binary sampling exhausted its 7 random-bit budget"
    );
}

#[test]
fn word_and_bit_admission_keep_exact_errors_and_actual_prefixes() {
    use rmagna::scalar::ScalarError;
    let a = ScalarArithmetic::default();
    let one = ExactScalar::one();
    let three = a.integer(3).unwrap();
    let tight = ScalarArithmetic::new(1).unwrap();
    let expected = SamplingError::Scalar(ScalarError::CoefficientLimit {
        required: 2,
        limit: 1,
    });
    let mut words = Words { word: 0, calls: 0 };
    assert_eq!(
        bernoulli_ratio(&ExactScalar::zero(), &three, &mut words, 0, &tight),
        Err(expected.clone())
    );
    assert_eq!(words.calls, 0);
    assert_eq!(
        bernoulli_ratio_bits(
            &ExactScalar::zero(),
            &three,
            &mut || panic!("admission failure consumed a bit"),
            0,
            &tight
        ),
        Err(BitSamplingError::Sampling(expected))
    );
    let mut words = Words { word: 0, calls: 0 };
    assert_eq!(
        bernoulli_ratio(
            &one,
            &three,
            &mut words,
            8,
            &ScalarArithmetic::new(64).unwrap()
        ),
        Err(SamplingError::Scalar(ScalarError::CoefficientLimit {
            required: 65,
            limit: 64
        }))
    );
    assert_eq!(words.calls, 1);
    let mut calls = 0;
    assert_eq!(
        bernoulli_ratio_bits(
            &one,
            &three,
            &mut || {
                calls += 1;
                false
            },
            8,
            &ScalarArithmetic::new(2).unwrap()
        ),
        Err(BitSamplingError::Sampling(SamplingError::Scalar(
            ScalarError::CoefficientLimit {
                required: 3,
                limit: 2
            }
        )))
    );
    assert_eq!(calls, 2);
}

#[test]
fn borrowed_dynamic_callbacks_and_zero_bit_limits_are_explicit() {
    let a = ScalarArithmetic::default();
    let one = ExactScalar::one();
    let half = a.omega(0, -2).unwrap();
    assert_eq!(
        bernoulli_ratio_bits(
            &half,
            &one,
            &mut || panic!("zero budget read entropy"),
            0,
            &a
        ),
        Err(BitSamplingError::EntropyLimit { limit: 0 })
    );
    let mut calls = 0;
    {
        let mut callback = || {
            calls += 1;
            true
        };
        let dynamic: &mut dyn FnMut() -> bool = &mut callback;
        assert_eq!(
            bernoulli_ratio_bits(&half, &one, dynamic, 1, &a).unwrap(),
            BitDraw {
                selected: false,
                entropy_bits: 1
            }
        );
    }
    assert_eq!(calls, 1);
    assert_eq!(
        bernoulli_ratio_bits(
            &one,
            &one,
            &mut || panic!("endpoint read entropy"),
            usize::MAX,
            &a
        )
        .unwrap(),
        BitDraw {
            selected: true,
            entropy_bits: 0
        }
    );
}
