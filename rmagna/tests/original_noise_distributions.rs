// SPDX-License-Identifier: MPL-2.0
// Complete finite contract from pinned Magna commit
// 56847478c840395735146ef1e1ccdc3d8aedc10f, tests/core/test_noise_numeric_fastpaths.py,
// NoiseNumericFastpathTests.test_depolarizing_and_nonuniform_channels_match_declared_law.
use num_rational::BigRational;
use num_traits::One;
use rmagna::instrument::FaultSelection;
use rmagna::noise::{
    NoiseDraw, NoiseError, NoiseModel, NoiseOptions, NoiseSampler, RandomSource, SplitMix64,
};
use rmagna::probability::Probability;
use std::collections::BTreeMap;

const SHOTS: usize = 32_768;
const SECOND_WEIGHTS: [usize; 6] = [4, 0, 1, 2, 0, 3];

fn probability(text: &str) -> Probability {
    text.parse().unwrap()
}

fn original_model() -> NoiseModel {
    let mut depolarizing = vec![probability("1/2")];
    depolarizing.extend(vec![probability("1/30"); 15]);
    NoiseModel::new(
        vec![
            depolarizing,
            ["2/5", "0", "1/10", "1/5", "0", "3/10"]
                .map(probability)
                .to_vec(),
            ["0", "1", "0"].map(probability).to_vec(),
            vec![probability("1")],
        ],
        &NoiseOptions::default(),
    )
    .unwrap()
}

struct Words {
    values: [u64; 2],
    used: usize,
}
impl Words {
    fn new(first: u64, second: u64) -> Self {
        Self {
            values: [first, second],
            used: 0,
        }
    }
}
impl RandomSource for Words {
    fn next_u64(&mut self) -> u64 {
        let word = self.values[self.used];
        self.used += 1;
        word
    }
}

struct Counted {
    inner: SplitMix64,
    used: usize,
}
impl RandomSource for Counted {
    fn next_u64(&mut self) -> u64 {
        self.used += 1;
        self.inner.next_u64()
    }
}

fn checked_choices(draw: &NoiseDraw) -> [usize; 2] {
    assert!(
        draw.faults
            .windows(2)
            .all(|pair| pair[0].site < pair[1].site),
        "faults must be ordered and cannot repeat a site"
    );
    let mut choices = [0; 4];
    for fault in &draw.faults {
        assert!(fault.site < 4, "unexpected fault: {fault:?}");
        choices[fault.site] = fault.alternative.checked_add(1).unwrap();
    }
    assert!(choices[0] < 16);
    assert!(matches!(choices[1], 0 | 2 | 3 | 5));
    assert_eq!(choices[2], 1, "the unit-probability fault is compulsory");
    assert_eq!(choices[3], 0, "the empty category stays identity");
    let first_weight = if choices[0] == 0 { 15 } else { 1 };
    let mass = BigRational::new(
        (first_weight * SECOND_WEIGHTS[choices[1]]).into(),
        300.into(),
    );
    assert_eq!(draw.physical_probability, mass);
    assert_eq!(draw.proposal_probability, mass);
    assert_eq!(draw.likelihood_ratio, BigRational::one());
    [choices[0], choices[1]]
}

fn seeded_draw(sampler: &NoiseSampler, seed: u64) -> NoiseDraw {
    let mut source = Counted {
        inner: SplitMix64::new(seed),
        used: 0,
    };
    let draw = sampler.draw(&mut source).unwrap();
    assert_eq!(draw.entropy_words, source.used);
    assert!(source.used >= 2);
    draw
}

fn within_original_tolerance(count: usize, probability: f64) {
    if probability == 0.0 {
        assert_eq!(count, 0);
    } else {
        let n = SHOTS as f64;
        assert!(
            (count as f64 - n * probability).abs()
                < 6.0 * (n * probability * (1.0 - probability)).sqrt() + 2.0,
            "count={count}, shots={SHOTS}, probability={probability}"
        );
    }
}

#[test]
fn original_depolarizing_and_nonuniform_channels_match_declared_law() {
    let model = original_model();
    let before = (0..4)
        .map(|site| model.probabilities(site).unwrap().to_vec())
        .collect::<Vec<_>>();
    let sampler = NoiseSampler::physical(model.clone(), NoiseOptions::default());
    let mut counts = [[0_usize; 6]; 16];
    let mut distinct_masses = BTreeMap::new();
    for a in 0..30 {
        for b in 0..10 {
            let mut source = Words::new(a, b);
            let draw = sampler.draw(&mut source).unwrap();
            let choices = checked_choices(&draw);
            // Literal intervals of the original laws, not a second CDF builder.
            let first = if a < 15 { 0 } else { a as usize - 14 };
            let second = match b {
                0..=3 => 0,
                4 => 2,
                5..=6 => 3,
                7..=9 => 5,
                _ => unreachable!(),
            };
            assert_eq!(choices, [first, second]);
            assert_eq!(source.used, 2);
            assert_eq!(draw.entropy_words, 2);
            counts[first][second] += 1;
            distinct_masses
                .entry(choices)
                .or_insert(draw.physical_probability);
        }
    }
    for (first, row) in counts.iter().enumerate() {
        let first_weight = if first == 0 { 15 } else { 1 };
        for (second, &count) in row.iter().enumerate() {
            assert_eq!(count, first_weight * SECOND_WEIGHTS[second]);
            if count != 0 {
                assert_eq!(
                    distinct_masses[&[first, second]],
                    BigRational::new(count.into(), 300.into())
                );
            }
        }
    }
    assert_eq!(counts.iter().flatten().sum::<usize>(), 300);
    assert_eq!(distinct_masses.len(), 64);
    assert_eq!(
        distinct_masses.values().cloned().sum::<BigRational>(),
        BigRational::one()
    );

    let strict = NoiseSampler::physical(
        model.clone(),
        NoiseOptions {
            max_entropy_words: 1,
            ..Default::default()
        },
    );
    let mut refused = Words::new(29, 9);
    assert_eq!(
        strict.draw(&mut refused),
        Err(NoiseError::EntropyLimit { limit: 1 })
    );
    assert_eq!(refused.used, 1);
    let retry = NoiseSampler::physical(
        model.clone(),
        NoiseOptions {
            max_entropy_words: 2,
            ..Default::default()
        },
    );
    let mut recovered = Words::new(29, 9);
    let draw = retry.draw(&mut recovered).unwrap();
    assert_eq!(checked_choices(&draw), [15, 5]);
    assert_eq!(
        draw.faults,
        [
            FaultSelection {
                site: 0,
                alternative: 14
            },
            FaultSelection {
                site: 1,
                alternative: 4
            },
            FaultSelection {
                site: 2,
                alternative: 0
            },
        ]
    );
    assert_eq!(
        draw.physical_probability,
        BigRational::new(1.into(), 100.into())
    );
    assert_eq!((recovered.used, draw.entropy_words), (2, 2));

    // Reproducible Rust entropy is independent of the original native MT ABI.
    // Each supplied seed starts its own stream, preserving the original
    // per-seed batching contract without adding a production batch API.
    let mut seed_source = SplitMix64::new(13);
    let seeds = (0..SHOTS)
        .map(|_| seed_source.next_u64())
        .collect::<Vec<_>>();
    let draws = seeds
        .iter()
        .map(|&seed| seeded_draw(&sampler, seed))
        .collect::<Vec<_>>();
    let (mut first_counts, mut second_counts) = ([0_usize; 16], [0_usize; 6]);
    let mut joint_faults = 0;
    for draw in &draws {
        let [a, b] = checked_choices(draw);
        first_counts[a] += 1;
        second_counts[b] += 1;
        joint_faults += usize::from(a != 0 && b != 0);
    }
    for (choice, &count) in first_counts.iter().enumerate() {
        within_original_tolerance(count, if choice == 0 { 0.5 } else { 1.0 / 30.0 });
    }
    for (&count, &weight) in second_counts.iter().zip(&SECOND_WEIGHTS) {
        within_original_tolerance(count, weight as f64 / 10.0);
    }
    within_original_tolerance(joint_faults, 0.3);
    let partitioned = seeds[..17]
        .iter()
        .map(|&seed| seeded_draw(&sampler, seed))
        .chain(
            seeds[17..41]
                .iter()
                .map(|&seed| seeded_draw(&sampler, seed)),
        )
        .collect::<Vec<_>>();
    assert_eq!(&draws[..41], partitioned.as_slice());
    for (site, probabilities) in before.iter().enumerate() {
        assert_eq!(model.probabilities(site).unwrap(), probabilities.as_slice());
    }
}
