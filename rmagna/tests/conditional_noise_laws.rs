// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//! Complete original WeightedNoiseTests.test_invalid_weights from
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_weighted_noise.py,
//! plus exact owned-law/override, importance and resource regressions.
//! Original X/Z actions belong to the caller's catalog; this API retains their
//! channel indices/probabilities without a native object or RNG trajectory ABI.

use num_rational::BigRational;
use rmagna::instrument::FaultSelection;
use rmagna::noise::{
    ConditionalLawError, ConditionalSiteLaw, NoiseError, NoiseModel, NoiseOptions, NoiseSampler,
    NoiseSampling, SplitMix64,
};
use rmagna::probability::ProbabilityError;
use std::collections::BTreeSet;

const ORIGINAL_CHANNELS: usize = 2;
const ORIGINAL_OCCURRENCE: i64 = 1;
const ORIGINAL_INVALID_WEIGHTS: [&[i64]; 3] = [&[1], &[1, 1], &[-1, 2]];

fn r(numerator: i64, denominator: i64) -> BigRational {
    BigRational::new(numerator.into(), denominator.into())
}

fn law(occurrence: BigRational, weights: &[BigRational]) -> ConditionalSiteLaw {
    ConditionalSiteLaw::new(weights.len(), occurrence, weights.to_vec()).unwrap()
}

fn rows(model: &NoiseModel) -> Vec<Vec<BigRational>> {
    (0..model.num_sites())
        .map(|site| {
            model
                .probabilities(site)
                .unwrap()
                .iter()
                .map(|p| p.value().clone())
                .collect()
        })
        .collect()
}

#[test]
fn native_invalid_weights() {
    let expected = [
        ConditionalLawError::WeightCount {
            declared: 2,
            provided: 1,
        },
        ConditionalLawError::WeightSum { sum: r(2, 1) },
        ConditionalLawError::Weight {
            channel: 0,
            source: ProbabilityError::OutOfRange,
        },
    ];
    let diagnostics = [
        "provide exactly one weight per channel",
        "sum exactly to one",
        "channel 0",
    ];
    for ((weights, expected), diagnostic) in ORIGINAL_INVALID_WEIGHTS
        .iter()
        .zip(expected)
        .zip(diagnostics)
    {
        let error = ConditionalSiteLaw::new(
            ORIGINAL_CHANNELS,
            r(ORIGINAL_OCCURRENCE, 1),
            weights.iter().map(|&value| r(value, 1)).collect(),
        )
        .unwrap_err();
        assert_eq!(error, expected);
        assert!(error.to_string().contains(diagnostic));
    }
}

#[test]
fn conditional_laws_validate_exact_inputs_and_own_source_values() {
    assert_eq!(
        ConditionalSiteLaw::new(0, r(0, 1), vec![]),
        Err(ConditionalLawError::NoChannels)
    );
    for (declared, provided) in [(2, 0), (2, 3), (usize::MAX, 2)] {
        assert_eq!(
            ConditionalSiteLaw::new(declared, r(1, 1), vec![r(0, 1); provided]),
            Err(ConditionalLawError::WeightCount { declared, provided }),
        );
    }
    assert_eq!(
        ConditionalSiteLaw::new(2, r(0, 1), vec![r(1, 4), r(1, 4)]),
        Err(ConditionalLawError::WeightSum { sum: r(1, 2) }),
    );
    assert_eq!(
        ConditionalSiteLaw::new(2, r(0, 1), vec![r(-1, 1), r(2, 1)]),
        Err(ConditionalLawError::Weight {
            channel: 0,
            source: ProbabilityError::OutOfRange
        }),
    );
    assert_eq!(
        ConditionalSiteLaw::new(1, BigRational::new_raw(0.into(), 0.into()), vec![r(1, 1)]),
        Err(ConditionalLawError::Occurrence(
            ProbabilityError::ZeroDenominator
        )),
    );
    assert_eq!(
        ConditionalSiteLaw::new(
            2,
            r(0, 1),
            vec![BigRational::new_raw(1.into(), 0.into()), r(1, 1)]
        ),
        Err(ConditionalLawError::Weight {
            channel: 0,
            source: ProbabilityError::ZeroDenominator
        }),
    );
    for probability in [r(-1, 7), r(8, 7)] {
        assert_eq!(
            ConditionalSiteLaw::new(1, probability, vec![r(1, 1)]),
            Err(ConditionalLawError::Occurrence(
                ProbabilityError::OutOfRange
            )),
        );
    }
    let mut source = vec![r(1, 4), r(3, 4)];
    let owned = ConditionalSiteLaw::new(2, r(0, 1), source.clone()).unwrap();
    source[0] = r(-1, 1);
    source.clear();
    drop(source);
    assert_eq!(owned.num_channels(), 2);
    assert_eq!(owned.occurrence_probability().value(), &r(0, 1));
    assert_eq!(
        owned
            .conditional_weights()
            .iter()
            .map(|p| p.value().clone())
            .collect::<Vec<_>>(),
        vec![r(1, 4), r(3, 4)]
    );
    let zero_weight = law(r(0, 1), &[r(0, 1), r(1, 1)]);
    assert_eq!(zero_weight.conditional_weights()[0].value(), &r(0, 1));
    assert_eq!(zero_weight.conditional_weights()[1].value(), &r(1, 1));
}

#[test]
fn effective_rates_retain_zero_and_one_occurrence_conditional_laws() {
    let options = NoiseOptions::default();
    let laws = vec![
        law(r(0, 1), &[r(0, 1), r(1, 1)]),
        law(r(1, 1), &[r(1, 4), r(3, 4)]),
    ];
    let original = laws.clone();
    let unmodified = NoiseModel::from_conditional_laws(&laws, None, &options).unwrap();
    assert_eq!(
        rows(&unmodified),
        vec![
            vec![r(1, 1), r(0, 1), r(0, 1)],
            vec![r(0, 1), r(1, 4), r(3, 4)]
        ]
    );
    let overridden = NoiseModel::from_conditional_laws(&laws, Some(r(1, 3)), &options).unwrap();
    assert_eq!(
        rows(&overridden),
        vec![
            vec![r(2, 3), r(0, 1), r(1, 3)],
            vec![r(2, 3), r(1, 12), r(1, 4)]
        ]
    );
    assert_eq!(
        overridden
            .probability(&[FaultSelection {
                site: 0,
                alternative: 0
            }])
            .unwrap(),
        r(0, 1)
    );
    assert_eq!(
        overridden
            .probability(&[FaultSelection {
                site: 0,
                alternative: 1
            }])
            .unwrap(),
        r(2, 9)
    );
    assert!(matches!(
        overridden.probability(&[FaultSelection {
            site: 0,
            alternative: 2
        }]),
        Err(NoiseError::InvalidFault {
            site: 0,
            alternative: 2
        })
    ));
    for (rate, expected) in [
        (
            r(0, 1),
            vec![
                vec![r(1, 1), r(0, 1), r(0, 1)],
                vec![r(1, 1), r(0, 1), r(0, 1)],
            ],
        ),
        (
            r(1, 7),
            vec![
                vec![r(6, 7), r(0, 1), r(1, 7)],
                vec![r(6, 7), r(1, 28), r(3, 28)],
            ],
        ),
        (
            r(1, 1),
            vec![
                vec![r(0, 1), r(0, 1), r(1, 1)],
                vec![r(0, 1), r(1, 4), r(3, 4)],
            ],
        ),
    ] {
        let model = NoiseModel::from_conditional_laws(&laws, Some(rate), &options).unwrap();
        assert_eq!(model.num_sites(), 2);
        assert_eq!(rows(&model), expected);
        assert_eq!(model.probabilities(2), None);
        assert_eq!(laws, original);
    }
    drop(laws);
    assert_eq!(
        rows(&overridden),
        vec![
            vec![r(2, 3), r(0, 1), r(1, 3)],
            vec![r(2, 3), r(1, 12), r(1, 4)]
        ]
    );
}

#[test]
fn configured_laws_have_exact_likelihoods_and_deterministic_endpoints() {
    let options = NoiseOptions::default();
    let laws = vec![
        law(r(0, 1), &[r(0, 1), r(1, 1)]),
        law(r(1, 1), &[r(1, 1), r(0, 1)]),
    ];
    let original = laws.clone();
    let model = NoiseModel::from_conditional_laws(&laws, Some(r(1, 3)), &options).unwrap();
    let sampler = NoiseSampler::configured(
        model.clone(),
        &NoiseSampling::Importance {
            proposal_scale: r(2, 1),
        },
        &BTreeSet::new(),
        options,
    )
    .unwrap();
    let physical = [r(4, 9), r(2, 9), r(2, 9), r(1, 9)];
    let proposal = [r(1, 9), r(2, 9), r(2, 9), r(4, 9)];
    let likelihood = [r(4, 1), r(1, 1), r(1, 1), r(1, 4)];
    let mut random = SplitMix64::new(26);
    let mut seen = BTreeSet::new();
    for _ in 0..128 {
        let draw = sampler.draw(&mut random).unwrap();
        let choice = match draw.faults.as_slice() {
            [] => 0,
            [
                FaultSelection {
                    site: 0,
                    alternative: 1,
                },
            ] => 1,
            [
                FaultSelection {
                    site: 1,
                    alternative: 0,
                },
            ] => 2,
            [
                FaultSelection {
                    site: 0,
                    alternative: 1,
                },
                FaultSelection {
                    site: 1,
                    alternative: 0,
                },
            ] => 3,
            other => panic!("unexpected zero-weight channel draw: {other:?}"),
        };
        seen.insert(choice);
        assert_eq!(draw.physical_probability, physical[choice]);
        assert_eq!(draw.proposal_probability, proposal[choice]);
        assert_eq!(draw.likelihood_ratio, likelihood[choice]);
    }
    assert_eq!(seen, BTreeSet::from([0, 1, 2, 3]));
    assert!(matches!(
        NoiseSampler::configured(
            model,
            &NoiseSampling::Importance {
                proposal_scale: r(100, 1)
            },
            &BTreeSet::new(),
            options
        ),
        Err(NoiseError::ScaledProbability { site: 0 })
    ));
    for rate in [r(0, 1), r(1, 1)] {
        let forced = rate == r(1, 1);
        let endpoint_options = NoiseOptions {
            max_entropy_words: 0,
            ..options
        };
        let model =
            NoiseModel::from_conditional_laws(&laws, Some(rate), &endpoint_options).unwrap();
        let sampler = NoiseSampler::configured(
            model,
            &NoiseSampling::Importance {
                proposal_scale: r(100, 1),
            },
            &BTreeSet::new(),
            endpoint_options,
        )
        .unwrap();
        assert_eq!(
            sampler
                .scaled_proposal_summary()
                .unwrap()
                .endpoint_categories,
            2
        );
        assert_eq!(
            sampler.scaled_proposal_summary().unwrap().scaled_categories,
            0
        );
        let mut random = SplitMix64::new(17);
        let draw = sampler.draw(&mut random).unwrap();
        assert_eq!(
            draw.faults,
            if forced {
                vec![
                    FaultSelection {
                        site: 0,
                        alternative: 1,
                    },
                    FaultSelection {
                        site: 1,
                        alternative: 0,
                    },
                ]
            } else {
                vec![]
            }
        );
        assert_eq!(draw.physical_probability, r(1, 1));
        assert_eq!(draw.proposal_probability, r(1, 1));
        assert_eq!(draw.likelihood_ratio, r(1, 1));
        assert_eq!(draw.entropy_words, 0);
        assert_eq!(random.state(), 17);
    }
    assert_eq!(laws, original);
}

#[test]
fn conditional_models_enforce_denominator_budgets_and_recover_after_errors() {
    let laws = vec![law(r(0, 1), &[r(1, 3), r(2, 3)])];
    let original = laws.clone();
    let low = NoiseOptions {
        max_denominator_bits: 4,
        ..NoiseOptions::default()
    };
    assert!(matches!(
        NoiseModel::from_conditional_laws(&laws, Some(r(1, 7)), &low),
        Err(ConditionalLawError::Model(NoiseError::DenominatorLimit {
            limit: 4,
            required: 5
        }))
    ));
    for (rate, expected) in [
        (
            BigRational::new_raw(1.into(), 0.into()),
            ProbabilityError::ZeroDenominator,
        ),
        (r(-1, 7), ProbabilityError::OutOfRange),
        (r(8, 7), ProbabilityError::OutOfRange),
    ] {
        for source in [&laws[..], &[][..]] {
            let error =
                NoiseModel::from_conditional_laws(source, Some(rate.clone()), &low).unwrap_err();
            assert_eq!(error, ConditionalLawError::EffectiveRate(expected.clone()));
            assert!(error.to_string().contains("shared effective physical rate"));
        }
    }
    let options = NoiseOptions {
        max_denominator_bits: 5,
        ..NoiseOptions::default()
    };
    let model = NoiseModel::from_conditional_laws(&laws, Some(r(1, 7)), &options).unwrap();
    assert_eq!(rows(&model), vec![vec![r(6, 7), r(1, 21), r(2, 21)]]);
    let sampler = NoiseSampler::configured(
        model,
        &NoiseSampling::Importance {
            proposal_scale: r(2, 1),
        },
        &BTreeSet::new(),
        options,
    )
    .unwrap();
    let mut random = SplitMix64::new(91);
    let mut seen = BTreeSet::new();
    for _ in 0..128 {
        let draw = sampler.draw(&mut random).unwrap();
        if draw.faults.is_empty() {
            assert_eq!(draw.physical_probability, r(6, 7));
            assert_eq!(draw.proposal_probability, r(5, 7));
            assert_eq!(draw.likelihood_ratio, r(6, 5));
            seen.insert(0);
        } else {
            assert_eq!(draw.faults.len(), 1);
            assert_eq!(draw.faults[0].site, 0);
            match draw.faults[0].alternative {
                0 => {
                    assert_eq!(draw.physical_probability, r(1, 21));
                    assert_eq!(draw.proposal_probability, r(2, 21));
                    seen.insert(1);
                }
                1 => {
                    assert_eq!(draw.physical_probability, r(2, 21));
                    assert_eq!(draw.proposal_probability, r(4, 21));
                    seen.insert(2);
                }
                other => panic!("unexpected channel {other}"),
            }
            assert_eq!(draw.likelihood_ratio, r(1, 2));
        }
    }
    assert_eq!(seen, BTreeSet::from([0, 1, 2]));
    assert_eq!(laws, original);
    let empty = NoiseModel::from_conditional_laws(&[], Some(r(1, 7)), &options).unwrap();
    assert_eq!(empty.num_sites(), 0);
    assert_eq!(empty.probability(&[]).unwrap(), r(1, 1));
}
