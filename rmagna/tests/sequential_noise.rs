use num_bigint::BigUint;
use num_rational::BigRational;
use num_traits::{One, Zero};
use rmagna::noise::{
    ChannelSelection, ConditionalLawError, ConditionalSiteLaw, NoiseError, NoiseSampling,
    RandRangeSource, RandomSource, SequentialDrawError, SequentialNoiseError,
    SequentialNoiseOptions, SequentialNoiseSampler, SequentialNoiseSite, SplitMix64,
    WordRangeError, WordRangeSource,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::VecDeque, convert::Infallible};

fn ratio(value: &str) -> BigRational {
    value.parse().unwrap()
}

fn site(p: &str, weights: Option<Vec<BigRational>>) -> SequentialNoiseSite {
    let channels = weights.as_ref().map_or(1, Vec::len);
    SequentialNoiseSite::new(
        channels,
        ratio(p),
        weights,
        &SequentialNoiseOptions::default(),
    )
    .unwrap()
}

fn configured(sites: Vec<SequentialNoiseSite>) -> SequentialNoiseSampler {
    SequentialNoiseSampler::configured(
        sites,
        None,
        &NoiseSampling::Physical,
        SequentialNoiseOptions::default(),
    )
    .unwrap()
}

struct Tape {
    entries: Vec<(BigUint, BigUint)>,
    position: usize,
}

impl RandRangeSource for Tape {
    type Error = Infallible;

    fn randrange(&mut self, bound: &BigUint) -> Result<BigUint, Infallible> {
        let (expected, value) = &self.entries[self.position];
        assert_eq!(bound, expected, "request {}", self.position);
        self.position += 1;
        Ok(value.clone())
    }
}

fn original_sites() -> Vec<SequentialNoiseSite> {
    vec![
        site("1/7", Some(vec![ratio("1/3"), ratio("2/3")])),
        site("1/10", None),
        site(&format!("1/{}", BigUint::one() << 140usize), None),
        site("0", None),
        site("1", None),
    ]
}

#[test]
fn original_randrange_stream_and_exact_weights() {
    let bytes = include_bytes!("fixtures/original_noise_stream.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "4c016355476665b7c92d8313a792da7d899ca3aac26ad4ef7a5526f222017d67"
    );
    let fixture: Value = serde_json::from_slice(bytes).unwrap();
    let manifest: Value = serde_json::from_str(include_str!(
        "../../benchmarks/magna_reproduction/reference.json"
    ))
    .unwrap();
    assert_eq!(fixture["schema"], "rmagna.original-noise-stream.v1");
    assert_eq!(
        fixture["source"]["upstream_commit"],
        manifest["upstream"]["commit"]
    );
    assert_eq!(
        fixture["source"]["reference_manifest_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../benchmarks/magna_reproduction/reference.json"
            ))
        )
    );
    assert_eq!(
        fixture["source"]["test_source_sha256"],
        manifest["upstream"]["files"]["tests/core/test_weighted_noise.py"]
    );
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(
                fixture["source"]["test_source"]
                    .as_str()
                    .unwrap()
                    .as_bytes()
            )
        ),
        fixture["source"]["test_source_sha256"]
    );
    assert_eq!(
        fixture["source"]["test_id"],
        "test_weighted_noise.NativeNoiseStreamTests.test_original_randrange_stream_and_exact_weights"
    );
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 4);
    let mut draws = 0;
    let mut requests = 0;
    for case in cases {
        assert_eq!(case["seed"], 814);
        let sampling = case["proposal_scale"]
            .as_str()
            .map_or(NoiseSampling::Physical, |s| NoiseSampling::Importance {
                proposal_scale: ratio(s),
            });
        let sampler = SequentialNoiseSampler::configured(
            original_sites(),
            case["physical_rate"].as_str().map(ratio),
            &sampling,
            SequentialNoiseOptions::default(),
        )
        .unwrap();
        let mut tape = Tape {
            entries: case["range_tape"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    (
                        row[0].as_str().unwrap().parse().unwrap(),
                        row[1].as_str().unwrap().parse().unwrap(),
                    )
                })
                .collect(),
            position: 0,
        };
        let rows = case["draws"].as_array().unwrap();
        assert_eq!(rows.len(), 120);
        for row in rows {
            assert_eq!(tape.position, row["range_start"].as_u64().unwrap() as usize);
            let draw = sampler.draw(&mut tape).unwrap();
            assert_eq!(tape.position, row["range_end"].as_u64().unwrap() as usize);
            assert_eq!(
                draw.range_calls,
                (row["range_end"].as_u64().unwrap() - row["range_start"].as_u64().unwrap())
                    as usize
            );
            let expected = row["channels"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().map(|x| x as usize))
                .collect::<Vec<_>>();
            assert_eq!(draw.channels, expected);
            assert_eq!(
                draw.physical_probability,
                ratio(row["physical_probability"].as_str().unwrap())
            );
            assert_eq!(
                draw.proposal_probability,
                ratio(row["proposal_probability"].as_str().unwrap())
            );
            assert_eq!(
                draw.likelihood_ratio,
                ratio(row["weight"].as_str().unwrap())
            );
            let mut faults = Vec::new();
            let mut flips = Vec::new();
            // This catalog belongs to the test caller, including I's empty
            // action. Source labels are fixture data, not Rust engine IDs.
            for (index, channel) in draw.channels.iter().enumerate() {
                if let Some(channel) = channel {
                    match (index, *channel) {
                        (0, 0) => {}
                        (0, 1) => faults.push(json!({"after":"report","qubit":4,"pauli":"Y"})),
                        (1, 0) => flips.push(json!("m0")),
                        (2, 0) => faults.push(json!({"after":"huge","qubit":9,"pauli":"X"})),
                        (3, 0) => faults.push(json!({"after":"zero","qubit":1,"pauli":"Z"})),
                        (4, 0) => faults.push(json!({"after":"one","qubit":2,"pauli":"X"})),
                        _ => panic!("unexpected caller-catalog channel"),
                    }
                }
            }
            assert_eq!(json!(faults), row["faults"]);
            assert_eq!(json!(flips), row["flips"]);
            // Generation executed the original raw Python-state equality after
            // every draw. Rust reproduces that caller-owned source protocol by
            // exact request/result replay and the checked tape position above.
            assert_eq!(row["post_state_sha256"].as_str().unwrap().len(), 64);
            draws += 1;
        }
        assert_eq!(tape.position, tape.entries.len());
        requests += tape.position;
    }
    assert_eq!(draws, 480);
    assert_eq!(requests, 3076);
}

#[test]
fn equal_channel_weights_retain_explicit_uniform_or_sequential_calls() {
    let options = SequentialNoiseOptions::default();
    let uniform = SequentialNoiseSite::new(2, ratio("1"), None, &options).unwrap();
    let weighted = site("1", Some(vec![ratio("1/2"), ratio("1/2")]));
    assert_eq!(uniform.channel_selection(), ChannelSelection::Uniform);
    assert_eq!(
        weighted.channel_selection(),
        ChannelSelection::SequentialWeights
    );
    let mut uniform_source = Tape {
        entries: vec![(1u8.into(), 0u8.into()), (2u8.into(), 1u8.into())],
        position: 0,
    };
    let mut weighted_source = Tape {
        entries: vec![
            (1u8.into(), 0u8.into()),
            (2u8.into(), 1u8.into()),
            (1u8.into(), 0u8.into()),
        ],
        position: 0,
    };
    let left = configured(vec![uniform]).draw(&mut uniform_source).unwrap();
    let right = configured(vec![weighted])
        .draw(&mut weighted_source)
        .unwrap();
    assert_eq!(left.channels, vec![Some(1)]);
    assert_eq!(right.channels, left.channels);
    assert_eq!(left.physical_probability, ratio("1/2"));
    assert_eq!(right.physical_probability, left.physical_probability);
    assert_eq!(left.range_calls, 2);
    assert_eq!(right.range_calls, 3);
}

#[test]
fn weighted_zero_entries_skip_calls_and_unit_tail_keeps_a_request() {
    let sampler = configured(vec![site(
        "1",
        Some(vec![ratio("0"), ratio("2/3"), ratio("1/3")]),
    )]);
    let mut tape = Tape {
        entries: vec![
            (1u8.into(), 0u8.into()),
            (3u8.into(), 2u8.into()),
            (1u8.into(), 0u8.into()),
        ],
        position: 0,
    };
    let draw = sampler.draw(&mut tape).unwrap();
    assert_eq!(draw.channels, vec![Some(2)]);
    assert_eq!(draw.physical_probability, ratio("1/3"));
    assert_eq!(draw.range_calls, 3);
    let mut zeros = |_: &BigUint| Ok::<_, Infallible>(BigUint::zero());
    let draw = configured(vec![site("1", Some(vec![ratio("1"), ratio("0")]))])
        .draw(&mut zeros)
        .unwrap();
    assert_eq!(draw.channels, vec![Some(0)]);
    assert_eq!(draw.range_calls, 2);
}

#[test]
fn empty_action_occurrence_and_nonoccurrence_have_separate_masses() {
    let sampler = configured(vec![site("1/7", Some(vec![ratio("1/3"), ratio("2/3")]))]);
    let mut occurring = Tape {
        entries: vec![(7u8.into(), 0u8.into()), (3u8.into(), 0u8.into())],
        position: 0,
    };
    let mut absent = Tape {
        entries: vec![(7u8.into(), 1u8.into())],
        position: 0,
    };
    let left = sampler.draw(&mut occurring).unwrap();
    let right = sampler.draw(&mut absent).unwrap();
    assert_eq!(left.channels, vec![Some(0)]);
    assert_eq!(right.channels, vec![None]);
    assert_eq!(left.physical_probability, ratio("1/21"));
    assert_eq!(right.physical_probability, ratio("6/7"));
}

#[test]
fn effective_rate_activates_endpoint_laws_and_reuses_scaled_support_checks() {
    let laws = [ratio("0"), ratio("1")]
        .into_iter()
        .map(|p| ConditionalSiteLaw::new(2, p, vec![ratio("1/3"), ratio("2/3")]).unwrap())
        .collect::<Vec<_>>();
    let options = SequentialNoiseOptions::default();
    let sites = laws
        .iter()
        .cloned()
        .map(|law| SequentialNoiseSite::from_weighted_law(law, &options).unwrap())
        .collect();
    let sampler = SequentialNoiseSampler::configured(
        sites,
        Some(ratio("1/7")),
        &NoiseSampling::Importance {
            proposal_scale: ratio("2"),
        },
        options,
    )
    .unwrap();
    let mut source = Tape {
        entries: vec![
            (7u8.into(), 0u8.into()),
            (3u8.into(), 2u8.into()),
            (1u8.into(), 0u8.into()),
            (7u8.into(), 0u8.into()),
            (3u8.into(), 2u8.into()),
            (1u8.into(), 0u8.into()),
        ],
        position: 0,
    };
    let draw = sampler.draw(&mut source).unwrap();
    assert_eq!(draw.channels, vec![Some(1), Some(1)]);
    assert_eq!(draw.physical_probability, ratio("4/441"));
    assert_eq!(draw.proposal_probability, ratio("16/441"));
    assert_eq!(draw.likelihood_ratio, ratio("1/4"));
    assert_eq!(
        sampler.scaled_proposal_summary().unwrap().scaled_categories,
        2
    );
    assert_eq!(laws[0].occurrence_probability().value(), &ratio("0"));
    assert_eq!(laws[1].occurrence_probability().value(), &ratio("1"));
    for scale in ["0", "-1", "3", "4"] {
        assert!(
            SequentialNoiseSampler::configured(
                vec![site("1/3", None)],
                None,
                &NoiseSampling::Importance {
                    proposal_scale: ratio(scale)
                },
                options
            )
            .is_err()
        );
    }
    assert!(matches!(
        SequentialNoiseSampler::configured(
            vec![site("1/3", None)],
            Some(ratio("1/2")),
            &NoiseSampling::Importance {
                proposal_scale: ratio("2")
            },
            options
        ),
        Err(SequentialNoiseError::Model(NoiseError::ScaledProbability {
            site: 0
        }))
    ));
}

#[test]
fn deterministic_range_requests_are_counted_even_with_zero_word_entropy() {
    for p in ["0", "1"] {
        let sampler = SequentialNoiseSampler::configured(
            vec![site("0", None), site("1", None)],
            Some(ratio(p)),
            &NoiseSampling::Importance {
                proposal_scale: ratio("100"),
            },
            SequentialNoiseOptions::default(),
        )
        .unwrap();
        let mut random = SplitMix64::new(17);
        let mut adapter = WordRangeSource::new(&mut random, 0);
        let draw = sampler.draw(&mut adapter).unwrap();
        assert_eq!(adapter.entropy_words(), 0);
        assert_eq!(draw.range_calls, if p == "0" { 2 } else { 4 });
        assert_eq!(
            draw.channels,
            if p == "0" {
                vec![None, None]
            } else {
                vec![Some(0), Some(0)]
            }
        );
        assert_eq!(draw.likelihood_ratio, ratio("1"));
        assert_eq!(random.state(), 17);
    }
}

#[test]
fn invalid_source_responses_and_callback_failures_leave_the_sampler_reusable() {
    let sampler = configured(vec![site("1/7", None)]);
    let mut invalid = |bound: &BigUint| Ok::<_, Infallible>(bound.clone());
    assert!(
        matches!(sampler.draw(&mut invalid), Err(SequentialDrawError::OutOfRange { upper_exclusive, value }) if upper_exclusive==BigUint::from(7u8)&&value==upper_exclusive)
    );
    let mut failed = |_: &BigUint| Err::<BigUint, _>("source unavailable");
    assert_eq!(
        sampler.draw(&mut failed),
        Err(SequentialDrawError::Source("source unavailable"))
    );
    let mut zeros = |_: &BigUint| Ok::<_, Infallible>(BigUint::zero());
    let source: &mut dyn RandRangeSource<Error = Infallible> = &mut zeros;
    let draw = sampler.draw(source).unwrap();
    assert_eq!(draw.channels, vec![Some(0)]);
    assert_eq!(draw.physical_probability, ratio("1/7"));
}

#[test]
fn source_call_caps_fail_before_the_next_callback_and_empty_draws_need_none() {
    for limit in [0, 1] {
        let options = SequentialNoiseOptions {
            max_range_calls: limit,
            ..SequentialNoiseOptions::default()
        };
        let sampler = SequentialNoiseSampler::configured(
            vec![site("1", None)],
            None,
            &NoiseSampling::Physical,
            options,
        )
        .unwrap();
        let mut actual = 0;
        let mut source = |_: &BigUint| {
            actual += 1;
            Ok::<_, Infallible>(BigUint::zero())
        };
        assert_eq!(
            sampler.draw(&mut source),
            Err(SequentialDrawError::RangeCallLimit { limit })
        );
        assert_eq!(actual, limit);
    }
    let options = SequentialNoiseOptions {
        max_sites: 0,
        max_channels: 0,
        max_range_calls: 0,
        ..SequentialNoiseOptions::default()
    };
    let empty = SequentialNoiseSampler::configured(vec![], None, &NoiseSampling::Physical, options)
        .unwrap();
    let mut source =
        |_: &BigUint| -> Result<BigUint, Infallible> { panic!("empty draw called the source") };
    let draw = empty.draw(&mut source).unwrap();
    assert_eq!(draw.range_calls, 0);
    assert_eq!(draw.physical_probability, ratio("1"));
    assert_eq!(draw.proposal_probability, ratio("1"));
}

#[test]
fn site_channel_and_denominator_caps_validate_owned_and_effective_inputs() {
    let options = SequentialNoiseOptions {
        max_sites: 1,
        max_channels: 1,
        ..SequentialNoiseOptions::default()
    };
    assert!(matches!(
        SequentialNoiseSite::new(2, ratio("1"), None, &options),
        Err(SequentialNoiseError::ChannelLimit {
            limit: 1,
            requested: 2
        })
    ));
    assert!(matches!(
        SequentialNoiseSampler::configured(
            vec![site("0", None), site("1", None)],
            None,
            &NoiseSampling::Physical,
            options
        ),
        Err(SequentialNoiseError::SiteLimit {
            limit: 1,
            requested: 2
        })
    ));
    let options = SequentialNoiseOptions {
        max_sites: 2,
        ..options
    };
    assert!(matches!(
        SequentialNoiseSampler::configured(
            vec![site("0", None), site("1", None)],
            None,
            &NoiseSampling::Physical,
            options
        ),
        Err(SequentialNoiseError::ChannelLimit {
            limit: 1,
            requested: 2
        })
    ));
    let options = SequentialNoiseOptions {
        max_denominator_bits: 140,
        ..SequentialNoiseOptions::default()
    };
    assert!(
        SequentialNoiseSampler::configured(
            vec![site(&format!("1/{}", BigUint::one() << 140usize), None)],
            None,
            &NoiseSampling::Physical,
            options
        )
        .is_err()
    );
    let tiny = BigRational::new(1.into(), (BigUint::one() << 140usize).into());
    let latent = SequentialNoiseSite::new(
        2,
        ratio("0"),
        Some(vec![tiny.clone(), ratio("1") - tiny]),
        &SequentialNoiseOptions::default(),
    )
    .unwrap();
    assert!(
        SequentialNoiseSampler::configured(vec![latent], None, &NoiseSampling::Physical, options)
            .is_err()
    );
    assert!(matches!(
        SequentialNoiseSampler::configured(
            vec![],
            Some(ratio("2")),
            &NoiseSampling::Physical,
            options
        ),
        Err(SequentialNoiseError::Law(
            ConditionalLawError::EffectiveRate(_)
        ))
    ));
}

struct Words(VecDeque<u64>);

impl RandomSource for Words {
    fn next_u64(&mut self) -> u64 {
        self.0.pop_front().expect("word tape exhausted")
    }
}

#[test]
fn word_adapter_values_rejections_masks_and_arbitrary_precision_are_independent() {
    let mut words = Words(VecDeque::from([3, 2, 9, 0, 11, 3, 0xf000_0000_0000_1005]));
    let mut adapter = WordRangeSource::new(&mut words, 7);
    assert_eq!(adapter.randrange(&3u8.into()).unwrap(), 2u8.into());
    assert_eq!(adapter.entropy_words(), 2);
    assert_eq!(
        adapter
            .randrange(&((BigUint::one() << 64usize) + BigUint::one()))
            .unwrap(),
        9u8.into()
    );
    assert_eq!(adapter.entropy_words(), 4);
    assert_eq!(
        adapter.randrange(&(BigUint::one() << 140usize)).unwrap(),
        (BigUint::from(5u8) << 128usize) + (BigUint::from(3u8) << 64usize) + BigUint::from(11u8)
    );
    assert_eq!(adapter.entropy_words(), 7);
    assert_eq!(adapter.randrange(&BigUint::one()).unwrap(), BigUint::zero());
    assert_eq!(adapter.entropy_words(), 7);
    assert_eq!(
        adapter.randrange(&BigUint::zero()),
        Err(WordRangeError::EmptyRange)
    );
}

#[test]
fn word_adapter_entropy_failure_preserves_consumed_state_and_can_recover() {
    let mut words = Words(VecDeque::from([3, 1]));
    {
        let mut adapter = WordRangeSource::new(&mut words, 1);
        assert_eq!(
            adapter.randrange(&3u8.into()),
            Err(WordRangeError::Entropy(NoiseError::EntropyLimit {
                limit: 1
            }))
        );
        assert_eq!(adapter.entropy_words(), 1);
    }
    let mut adapter = WordRangeSource::new(&mut words, 1);
    assert_eq!(adapter.randrange(&3u8.into()).unwrap(), 1u8.into());
    assert_eq!(adapter.entropy_words(), 1);
    let mut words = Words(VecDeque::from([0, 0]));
    let mut adapter = WordRangeSource::new(&mut words, 1);
    assert!(adapter.randrange(&(BigUint::one() << 100usize)).is_err());
    assert_eq!(adapter.entropy_words(), 0);
    assert_eq!(words.0.len(), 2);
}
