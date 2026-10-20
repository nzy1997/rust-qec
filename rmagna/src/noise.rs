//! Exact independent categorical noise and reproducible prescribed assignments.
use crate::instrument::{CompiledInstrument, FaultSelection};
use crate::probability::Probability;
use num_bigint::BigUint;
use num_rational::BigRational;
use num_traits::{One, Zero};
use std::{collections::BTreeMap, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoiseError {
    InvalidCategory { site: usize },
    InvalidFault { site: usize, alternative: usize },
    RepeatedFault { site: usize },
    ProposalShape,
    MissingSupport { site: usize, choice: usize },
    DenominatorLimit { limit: u64, required: u64 },
    EntropyLimit { limit: usize },
}
impl fmt::Display for NoiseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCategory { site } => write!(
                f,
                "noise category {site} must contain identity and sum exactly to one"
            ),
            Self::InvalidFault { site, alternative } => {
                write!(f, "noise site {site} has no alternative {alternative}")
            }
            Self::RepeatedFault { site } => {
                write!(f, "noise site {site} was selected more than once")
            }
            Self::ProposalShape => write!(
                f,
                "proposal must have the same sites and alternatives as physical noise"
            ),
            Self::MissingSupport { site, choice } => write!(
                f,
                "proposal assigns zero probability to physical site {site} choice {choice}"
            ),
            Self::DenominatorLimit { limit, required } => write!(
                f,
                "categorical denominator requires {required} bits, limit is {limit}"
            ),
            Self::EntropyLimit { limit } => {
                write!(f, "noise draw exhausted its {limit} random-word budget")
            }
        }
    }
}
impl std::error::Error for NoiseError {}

#[derive(Clone, Copy, Debug)]
pub struct NoiseOptions {
    pub max_denominator_bits: u64,
    pub max_entropy_words: usize,
}
impl Default for NoiseOptions {
    fn default() -> Self {
        Self {
            max_denominator_bits: 1 << 20,
            max_entropy_words: 1 << 20,
        }
    }
}

/// Supplies unbiased independent 64-bit words. Implementations may provide a
/// deterministic pseudorandom stream; its statistical quality is their contract.
pub trait RandomSource {
    fn next_u64(&mut self) -> u64;
}
/// Stable SplitMix64 stream with an explicit resumable state. This generator is
/// for reproducible Monte Carlo, and has no cryptographic security contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}
impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    pub fn state(&self) -> u64 {
        self.state
    }
}
impl RandomSource for SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
}

#[derive(Clone, Debug)]
struct Category {
    probabilities: Vec<Probability>,
    cumulative: Vec<BigUint>,
    denominator: BigUint,
}
#[derive(Clone, Debug)]
pub struct NoiseModel {
    categories: Vec<Category>,
}
fn gcd(mut a: BigUint, mut b: BigUint) -> BigUint {
    while !b.is_zero() {
        let remainder = &a % &b;
        a = b;
        b = remainder;
    }
    a
}
impl NoiseModel {
    /// Each category starts with identity, followed by the site's alternatives.
    /// Empty site lists are valid. Every category must sum exactly to one.
    pub fn new(
        categories: Vec<Vec<Probability>>,
        options: &NoiseOptions,
    ) -> Result<Self, NoiseError> {
        let mut compiled = Vec::with_capacity(categories.len());
        for (site, probabilities) in categories.into_iter().enumerate() {
            if probabilities.is_empty()
                || probabilities.iter().map(|p| p.value()).sum::<BigRational>()
                    != BigRational::one()
            {
                return Err(NoiseError::InvalidCategory { site });
            }
            let mut denominator = BigUint::one();
            for p in &probabilities {
                let next = p
                    .value()
                    .denom()
                    .to_biguint()
                    .expect("probability denominator is positive");
                denominator = (&denominator / gcd(denominator.clone(), next.clone())) * next;
                if denominator.bits() > options.max_denominator_bits {
                    return Err(NoiseError::DenominatorLimit {
                        limit: options.max_denominator_bits,
                        required: denominator.bits(),
                    });
                }
            }
            let mut cumulative = Vec::with_capacity(probabilities.len());
            let mut running = BigUint::zero();
            for p in &probabilities {
                running += p
                    .value()
                    .numer()
                    .to_biguint()
                    .expect("nonnegative probability")
                    * (&denominator / p.value().denom().to_biguint().unwrap());
                cumulative.push(running.clone());
            }
            compiled.push(Category {
                probabilities,
                cumulative,
                denominator,
            });
        }
        Ok(Self {
            categories: compiled,
        })
    }
    pub fn physical(
        instrument: &CompiledInstrument,
        options: &NoiseOptions,
    ) -> Result<Self, NoiseError> {
        Self::new(
            instrument
                .noise_sites()
                .iter()
                .map(|site| {
                    std::iter::once(site.identity_probability().clone())
                        .chain(
                            site.alternatives()
                                .iter()
                                .map(|alternative| alternative.probability().clone()),
                        )
                        .collect()
                })
                .collect(),
            options,
        )
    }
    pub fn num_sites(&self) -> usize {
        self.categories.len()
    }
    pub fn probabilities(&self, site: usize) -> Option<&[Probability]> {
        self.categories
            .get(site)
            .map(|category| category.probabilities.as_slice())
    }
    fn denominator_bits(&self) -> u64 {
        self.categories
            .iter()
            .map(|c| c.denominator.bits())
            .max()
            .unwrap_or(0)
    }
    fn choices(&self, faults: &[FaultSelection]) -> Result<BTreeMap<usize, usize>, NoiseError> {
        let mut choices = BTreeMap::new();
        for fault in faults {
            let choice = fault
                .alternative
                .checked_add(1)
                .filter(|&choice| {
                    self.categories
                        .get(fault.site)
                        .is_some_and(|c| choice < c.probabilities.len())
                })
                .ok_or(NoiseError::InvalidFault {
                    site: fault.site,
                    alternative: fault.alternative,
                })?;
            if choices.insert(fault.site, choice).is_some() {
                return Err(NoiseError::RepeatedFault { site: fault.site });
            }
        }
        Ok(choices)
    }
    pub fn probability(&self, faults: &[FaultSelection]) -> Result<BigRational, NoiseError> {
        let choices = self.choices(faults)?;
        Ok(self
            .categories
            .iter()
            .enumerate()
            .map(|(site, category)| {
                category.probabilities[choices.get(&site).copied().unwrap_or(0)].value()
            })
            .product())
    }
}

#[derive(Clone, Debug)]
pub struct NoiseSampler {
    physical: NoiseModel,
    proposal: NoiseModel,
    options: NoiseOptions,
    denominator_bits: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoiseDraw {
    pub faults: Vec<FaultSelection>,
    pub physical_probability: BigRational,
    pub proposal_probability: BigRational,
    pub likelihood_ratio: BigRational,
    pub entropy_words: usize,
}
impl NoiseSampler {
    pub fn physical(model: NoiseModel, options: NoiseOptions) -> Self {
        let denominator_bits = model.denominator_bits();
        Self {
            proposal: model.clone(),
            physical: model,
            options,
            denominator_bits,
        }
    }
    pub fn importance(
        physical: NoiseModel,
        proposal: NoiseModel,
        options: NoiseOptions,
    ) -> Result<Self, NoiseError> {
        if physical.categories.len() != proposal.categories.len() {
            return Err(NoiseError::ProposalShape);
        }
        for (site, (p, q)) in physical
            .categories
            .iter()
            .zip(&proposal.categories)
            .enumerate()
        {
            if p.probabilities.len() != q.probabilities.len() {
                return Err(NoiseError::ProposalShape);
            }
            for (choice, (p, q)) in p.probabilities.iter().zip(&q.probabilities).enumerate() {
                if !p.value().is_zero() && q.value().is_zero() {
                    return Err(NoiseError::MissingSupport { site, choice });
                }
            }
        }
        let denominator_bits = physical.denominator_bits().max(proposal.denominator_bits());
        if denominator_bits > options.max_denominator_bits {
            return Err(NoiseError::DenominatorLimit {
                limit: options.max_denominator_bits,
                required: denominator_bits,
            });
        }
        Ok(Self {
            physical,
            proposal,
            options,
            denominator_bits,
        })
    }
    pub fn draw(&self, random: &mut impl RandomSource) -> Result<NoiseDraw, NoiseError> {
        if self.denominator_bits > self.options.max_denominator_bits {
            return Err(NoiseError::DenominatorLimit {
                limit: self.options.max_denominator_bits,
                required: self.denominator_bits,
            });
        }
        let mut entropy_words = 0;
        let mut faults = Vec::new();
        let mut p = BigRational::one();
        let mut q = BigRational::one();
        for (site, category) in self.proposal.categories.iter().enumerate() {
            let value = uniform_below(
                &category.denominator,
                random,
                &mut entropy_words,
                self.options.max_entropy_words,
            )?;
            let choice = category
                .cumulative
                .partition_point(|boundary| boundary <= &value);
            p *= self.physical.categories[site].probabilities[choice].value();
            q *= category.probabilities[choice].value();
            if choice != 0 {
                faults.push(FaultSelection {
                    site,
                    alternative: choice - 1,
                });
            }
        }
        let likelihood_ratio = &p / &q;
        Ok(NoiseDraw {
            faults,
            physical_probability: p,
            proposal_probability: q,
            likelihood_ratio,
            entropy_words,
        })
    }
}
fn uniform_below(
    bound: &BigUint,
    random: &mut impl RandomSource,
    used: &mut usize,
    limit: usize,
) -> Result<BigUint, NoiseError> {
    let bits = (bound - BigUint::one()).bits();
    if bits == 0 {
        return Ok(BigUint::zero());
    }
    let words = bits.div_ceil(64) as usize;
    loop {
        if words > limit.saturating_sub(*used) {
            return Err(NoiseError::EntropyLimit { limit });
        }
        let mut limbs = Vec::with_capacity(words * 2);
        for j in 0..words {
            let mut value = random.next_u64();
            *used += 1;
            if j + 1 == words && !bits.is_multiple_of(64) {
                value &= (1_u64 << (bits % 64)) - 1;
            }
            limbs.extend([value as u32, (value >> 32) as u32]);
        }
        let value = BigUint::new(limbs);
        if &value < bound {
            return Ok(value);
        }
    }
}
