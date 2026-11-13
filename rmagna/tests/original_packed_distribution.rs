// SPDX-License-Identifier: MPL-2.0
// Complete PackedNoiseTests::test_marginals_and_joint_law_across_lanes_and_blocks,
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_packed_noise.py.
use num_bigint::BigInt;
use num_rational::BigRational;
use rmagna::noise::{NoiseModel, NoiseOptions, NoiseSampler, RandomSource, SplitMix64};
use rmagna::probability::Probability;
struct Counted {
    source: SplitMix64,
    words: usize,
}
impl RandomSource for Counted {
    fn next_u64(&mut self) -> u64 {
        self.words += 1;
        self.source.next_u64()
    }
}
fn six_sigma(observed: u64, shots: u64, denominator: u64, kind: &str) {
    // p=1/d. Squaring the original STRICT bound gives this integer test;
    // no floating-point square root or tolerance rounding is involved.
    let delta = i128::from(denominator) * i128::from(observed) - i128::from(shots);
    assert!(
        delta * delta < 36 * i128::from(shots) * i128::from(denominator - 1),
        "{kind}: observed={observed},shots={shots},p=1/{denominator}"
    );
}
#[test]
fn original_marginals_and_joint_law_cross_all_lane_and_block_boundaries() {
    const SITES: usize = 131;
    const SHOTS: u64 = 4096;
    let row = vec![
        "3/4".parse::<Probability>().unwrap(),
        "1/4".parse::<Probability>().unwrap(),
    ];
    let rows = vec![row.clone(); SITES];
    let model = NoiseModel::new(rows.clone(), &NoiseOptions::default()).unwrap();
    assert_eq!(model.num_sites(), SITES);
    let sampler = NoiseSampler::physical(model.clone(), NoiseOptions::default());
    let probabilities = (0..=SITES)
        .map(|faults| {
            BigRational::new(
                BigInt::from(3).pow((SITES - faults) as u32),
                BigInt::from(4).pow(SITES as u32),
            )
        })
        .collect::<Vec<_>>();
    let marginals = [0, 1, 63, 64, 127, 130];
    let pairs = [(0, 1), (0, 63), (63, 64), (64, 130)];
    let mut marginal_counts = [0_u64; 6];
    let mut joint_counts = [0_u64; 4];
    let mut seeds = SplitMix64::new(928);
    for _ in 0..SHOTS {
        let mut random = Counted {
            source: SplitMix64::new(seeds.next_u64()),
            words: 0,
        };
        let draw = sampler.draw(&mut random).unwrap();
        let mut selected = [false; SITES];
        assert!(
            draw.faults
                .windows(2)
                .all(|pair| pair[0].site < pair[1].site),
            "assignments must be ordered and unique"
        );
        for fault in &draw.faults {
            assert!(fault.site < SITES);
            assert_eq!(fault.alternative, 0);
            assert!(!selected[fault.site]);
            selected[fault.site] = true;
        }
        assert_eq!(draw.physical_probability, probabilities[draw.faults.len()]);
        assert_eq!(draw.proposal_probability, draw.physical_probability);
        assert_eq!(draw.likelihood_ratio, "1".parse::<BigRational>().unwrap());
        assert_eq!(draw.entropy_words, SITES);
        assert_eq!(random.words, SITES);
        for (count, &site) in marginal_counts.iter_mut().zip(&marginals) {
            *count += u64::from(selected[site]);
        }
        for (count, &(left, right)) in joint_counts.iter_mut().zip(&pairs) {
            *count += u64::from(selected[left] && selected[right]);
        }
    }
    for (&site, &count) in marginals.iter().zip(&marginal_counts) {
        six_sigma(count, SHOTS, 4, &format!("marginal boundary law at {site}"));
    }
    for (&pair, &count) in pairs.iter().zip(&joint_counts) {
        six_sigma(count, SHOTS, 16, &format!("joint boundary law at {pair:?}"));
    }
    for (site, original) in rows.iter().enumerate() {
        assert_eq!(model.probabilities(site).unwrap(), row);
        assert_eq!(original, &row);
    }
    assert_eq!(model.num_sites(), SITES);
}
