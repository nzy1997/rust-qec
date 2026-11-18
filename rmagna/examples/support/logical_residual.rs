//! Exact full-support sampling of each physical nine-wire encoded input.
use rmagna::noise::RandomSource;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::error::Error;

pub(crate) const WIDTH: usize = 9;
pub(crate) const PATCHES: usize = 15;
const TOTAL_MASS: u128 = 300_u128.pow(WIDTH as u32);
const FIXTURE: &[u8] =
    include_bytes!("../../../benchmarks/magna_reproduction/paper/logical-residuals.json");

#[derive(Deserialize)]
struct Classifier {
    #[serde(rename = "counts_IXZY")]
    counts: [[usize; WIDTH + 1]; 4],
    x_residual: Vec<u8>,
    z_residual: Vec<u8>,
}

struct Bucket {
    class: u8,
    weight: usize,
    words: Vec<u32>,
}

pub(crate) struct ResidualProposal {
    identity: Vec<Bucket>,
    nonidentity: Vec<Bucket>,
    pub(crate) identity_mass: u128,
    pub(crate) fixture_sha256: String,
}

pub(crate) struct PatchDraw {
    pub(crate) word: u32,
    pub(crate) weight: usize,
    pub(crate) residual: u8,
}

fn atom_numerator(weight: usize) -> u128 {
    // p0/3 = 1/300 and 1-p0 = 297/300.
    297_u128.pow((WIDTH - weight) as u32)
}

impl ResidualProposal {
    pub(crate) fn new() -> Result<Self, Box<dyn Error>> {
        let classifier: Classifier = serde_json::from_slice(FIXTURE)?;
        let rows = 1 << WIDTH;
        if classifier.x_residual.len() != rows
            || classifier.z_residual.len() != rows
            || classifier
                .x_residual
                .iter()
                .chain(&classifier.z_residual)
                .any(|&value| value > 1)
        {
            return Err("invalid logical residual classifier".into());
        }
        let mut words: [[Vec<u32>; WIDTH + 1]; 4] =
            std::array::from_fn(|_| std::array::from_fn(|_| Vec::new()));
        for (x, &x_residual) in classifier.x_residual.iter().enumerate() {
            for (z, &z_residual) in classifier.z_residual.iter().enumerate() {
                let class = usize::from(x_residual + 2 * z_residual);
                let weight = (x | z).count_ones() as usize;
                let mut word = 0;
                for wire in 0..WIDTH {
                    let label = ((x >> wire) & 1) | (((z >> wire) & 1) << 1);
                    word |= (label as u32) << (2 * wire);
                }
                words[class][weight].push(word);
            }
        }
        let mut identity = Vec::new();
        let mut nonidentity = Vec::new();
        for (class, groups) in words.into_iter().enumerate() {
            for (weight, words) in groups.into_iter().enumerate() {
                if words.len() != classifier.counts[class][weight] {
                    return Err("logical residual classifier counts changed".into());
                }
                if words.is_empty() {
                    continue;
                }
                let bucket = Bucket {
                    class: class as u8,
                    weight,
                    words,
                };
                if class == 0 {
                    identity.push(bucket);
                } else {
                    nonidentity.push(bucket);
                }
            }
        }
        let mass = |buckets: &[Bucket]| -> u128 {
            buckets
                .iter()
                .map(|bucket| (bucket.words.len() as u128) * atom_numerator(bucket.weight))
                .sum()
        };
        let identity_mass = mass(&identity);
        if identity_mass == 0
            || identity_mass >= TOTAL_MASS
            || identity_mass + mass(&nonidentity) != TOTAL_MASS
        {
            return Err("logical residual proposal does not cover physical patterns".into());
        }
        Ok(Self {
            identity,
            nonidentity,
            identity_mass,
            fixture_sha256: format!("{:x}", Sha256::digest(FIXTURE)),
        })
    }

    pub(crate) fn draw(&self, random: &mut impl RandomSource) -> PatchDraw {
        // Draw the 1/5 nonidentity mixture, then use exactly the physical law
        // conditional on that partition. This is equivalent to first splitting
        // nonidentity mass into X, Z and Y in proportion to their physical mass.
        let (buckets, mass) = if below(random, 5) == 0 {
            (&self.nonidentity, TOTAL_MASS - self.identity_mass)
        } else {
            (&self.identity, self.identity_mass)
        };
        let mut ticket = below(random, mass);
        for bucket in buckets {
            let atom = atom_numerator(bucket.weight);
            let width = (bucket.words.len() as u128) * atom;
            if ticket < width {
                return PatchDraw {
                    word: bucket.words[(ticket / atom) as usize],
                    weight: bucket.weight,
                    residual: bucket.class,
                };
            }
            ticket -= width;
        }
        unreachable!("prepared bucket widths sum to the sampled partition mass")
    }
}

fn below(random: &mut impl RandomSource, bound: u128) -> u128 {
    let bits = 128 - (bound - 1).leading_zeros();
    if bits == 0 {
        return 0;
    }
    let mask = u128::MAX >> (128 - bits);
    loop {
        let low = u128::from(random.next_u64());
        let draw = if bits > 64 {
            low | (u128::from(random.next_u64()) << 64)
        } else {
            low
        };
        if draw & mask < bound {
            return draw & mask;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmagna::noise::SplitMix64;
    use std::collections::BTreeSet;

    #[test]
    fn proposal_partitions_all_physical_patterns_and_preserves_their_weights() {
        let sampler = ResidualProposal::new().unwrap();
        let raw: Classifier = serde_json::from_slice(FIXTURE).unwrap();
        let expected = raw.counts[0]
            .iter()
            .enumerate()
            .map(|(weight, &count)| (count as u128) * atom_numerator(weight))
            .sum::<u128>();
        assert_eq!(sampler.identity_mass, expected);
        let mut seen = BTreeSet::new();
        for bucket in sampler.identity.iter().chain(&sampler.nonidentity) {
            for &word in &bucket.words {
                let (mut x, mut z, mut occupied) = (0, 0, 0);
                for wire in 0..WIDTH {
                    let label = (word >> (wire * 2)) & 3;
                    x |= ((label & 1) as usize) << wire;
                    z |= ((label >> 1) as usize) << wire;
                    occupied += usize::from(label != 0);
                }
                assert_eq!(occupied, bucket.weight);
                assert_eq!(raw.x_residual[x] + 2 * raw.z_residual[z], bucket.class);
                assert!(seen.insert(word));
            }
        }
        assert_eq!(seen.len(), 4_usize.pow(WIDTH as u32));
        let mut independent = SplitMix64::new(2026100631);
        assert!(
            (0..256)
                .map(|_| sampler.draw(&mut independent))
                .any(|draw| draw.residual > 0)
        );
    }
}
