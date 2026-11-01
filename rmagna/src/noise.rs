//! Exact independent categorical noise and reproducible prescribed assignments.
mod conditional;
mod original;
pub use conditional::{ConditionalLawError, ConditionalSiteLaw};
pub use original::{
    OriginalNoiseAction, OriginalNoiseAnchor, OriginalNoiseCategory, OriginalNoiseChannel,
    OriginalNoiseError, OriginalNoiseOptions, original_noise_categories,
};
pub(crate) use original::{correlated_channel, pauli_channels};

use crate::instrument::{CompiledInstrument, FaultSelection};
use crate::probability::Probability;
use num_bigint::BigUint;
use num_rational::BigRational;
use num_traits::{One, Zero};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoiseError {
    InvalidCategory { site: usize },
    InvalidFault { site: usize, alternative: usize },
    RepeatedFault { site: usize },
    ProposalShape,
    InvalidProposalScale,
    InvalidExcludedSite { site: usize },
    ScaledProbability { site: usize },
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
            Self::InvalidProposalScale => {
                write!(f, "proposal scale must be a finite positive rational")
            }
            Self::InvalidExcludedSite { site } => write!(
                f,
                "excluded noise site {site} is outside the physical model"
            ),
            Self::ScaledProbability { site } => write!(
                f,
                "proposal scale makes noise site {site} total error probability reach or exceed one"
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum NoiseSampling {
    #[default]
    Physical,
    Importance {
        proposal_scale: BigRational,
    },
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
    identity_positive: bool,
    replacement_weights: Vec<BigRational>,
}
#[derive(Clone, Debug)]
pub struct NoiseModel {
    categories: Vec<Category>,
    positive_identity_product: BigRational,
    forced_sites: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScaledProposalSummary {
    pub proposal_scale: BigRational,
    pub categories: usize,
    pub scaled_categories: usize,
    pub unchanged_categories: usize,
    pub excluded_categories: usize,
    /// Total error probability zero or one, regardless of exclusion. A total
    /// of one can still have several stochastic nonidentity alternatives.
    pub endpoint_categories: usize,
}
#[derive(Clone, Debug)]
pub struct ScaledNoiseProposal {
    model: NoiseModel,
    summary: ScaledProposalSummary,
}
impl ScaledNoiseProposal {
    pub fn model(&self) -> &NoiseModel {
        &self.model
    }
    pub fn into_model(self) -> NoiseModel {
        self.model
    }
    pub fn summary(&self) -> &ScaledProposalSummary {
        &self.summary
    }
}

/// Balanced exact products avoid repeatedly reducing a growing left prefix.
pub(crate) fn probability_product<'a>(
    values: impl IntoIterator<Item = &'a BigRational>,
) -> BigRational {
    let mut level = values
        .into_iter()
        .filter(|p| !p.is_one())
        .cloned()
        .collect::<Vec<_>>();
    if level.iter().any(Zero::is_zero) {
        return BigRational::zero();
    }
    while level.len() > 1 {
        level = level
            .chunks(2)
            .map(|pair| {
                if pair.len() == 2 {
                    &pair[0] * &pair[1]
                } else {
                    pair[0].clone()
                }
            })
            .collect();
    }
    level.pop().unwrap_or_else(BigRational::one)
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
            let identity = probabilities[0].value();
            let identity_positive = !identity.is_zero();
            let replacement_weights = probabilities
                .iter()
                .map(|p| {
                    if identity_positive {
                        p.value() / identity
                    } else {
                        p.value().clone()
                    }
                })
                .collect();
            compiled.push(Category {
                probabilities,
                cumulative,
                denominator,
                identity_positive,
                replacement_weights,
            });
        }
        let forced_sites = compiled.iter().filter(|c| !c.identity_positive).count();
        let positive_identity_product = probability_product(
            compiled
                .iter()
                .filter(|c| c.identity_positive)
                .map(|c| c.probabilities[0].value()),
        );
        Ok(Self {
            categories: compiled,
            positive_identity_product,
            forced_sites,
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
    /// Scale exclusive nonidentity masses of eligible categories. The caller
    /// supplies excluded sites (for example causal guards); total-error zero
    /// and one endpoints always remain unchanged. Every scaled total must be
    /// strictly below one to preserve support for physical identity choices.
    pub fn scaled_proposal(
        &self,
        scale: &BigRational,
        excluded: &BTreeSet<usize>,
        options: &NoiseOptions,
    ) -> Result<ScaledNoiseProposal, NoiseError> {
        if scale.denom().is_zero() {
            return Err(NoiseError::InvalidProposalScale);
        }
        let scale = BigRational::new(scale.numer().clone(), scale.denom().clone());
        if scale <= BigRational::zero() {
            return Err(NoiseError::InvalidProposalScale);
        }
        self.validate_exclusions(excluded)?;
        let mut categories = Vec::with_capacity(self.num_sites());
        let mut scaled_categories = 0;
        let mut endpoint_categories = 0;
        for (site, category) in self.categories.iter().enumerate() {
            let identity = category.probabilities[0].value();
            let endpoint = identity.is_zero() || identity.is_one();
            endpoint_categories += usize::from(endpoint);
            if endpoint || excluded.contains(&site) || scale.is_one() {
                categories.push(category.probabilities.clone());
                continue;
            }
            let error = BigRational::one() - identity;
            let proposed = error * &scale;
            if proposed >= BigRational::one() {
                return Err(NoiseError::ScaledProbability { site });
            }
            let mut probabilities = Vec::with_capacity(category.probabilities.len());
            probabilities.push(
                Probability::from_rational(BigRational::one() - proposed)
                    .map_err(|_| NoiseError::ScaledProbability { site })?,
            );
            for p in &category.probabilities[1..] {
                probabilities.push(
                    p.scaled(&scale)
                        .map_err(|_| NoiseError::ScaledProbability { site })?,
                );
            }
            categories.push(probabilities);
            scaled_categories += 1;
        }
        Ok(ScaledNoiseProposal {
            model: Self::new(categories, options)?,
            summary: ScaledProposalSummary {
                proposal_scale: scale,
                categories: self.num_sites(),
                scaled_categories,
                unchanged_categories: self.num_sites() - scaled_categories,
                excluded_categories: excluded.len(),
                endpoint_categories,
            },
        })
    }
    fn validate_exclusions(&self, excluded: &BTreeSet<usize>) -> Result<(), NoiseError> {
        for &site in excluded {
            if site >= self.num_sites() {
                return Err(NoiseError::InvalidExcludedSite { site });
            }
        }
        Ok(())
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
        // Validate every supplied fault before returning a zero mass.
        if choices
            .keys()
            .filter(|&&site| !self.categories[site].identity_positive)
            .count()
            != self.forced_sites
        {
            return Ok(BigRational::zero());
        }
        if choices.is_empty() {
            return Ok(self.positive_identity_product.clone());
        }
        let corrections = probability_product(
            choices
                .iter()
                .map(|(&site, &choice)| &self.categories[site].replacement_weights[choice]),
        );
        Ok(&self.positive_identity_product * corrections)
    }
}

#[derive(Clone, Debug)]
pub struct NoiseSampler {
    physical: NoiseModel,
    proposal: NoiseModel,
    options: NoiseOptions,
    denominator_bits: u64,
    same_distribution: bool,
    scaled_summary: Option<ScaledProposalSummary>,
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
    /// Prepare physical or exact common-scale importance noise. Excluded sites
    /// must be valid in both modes; the causal runtime supplies guarded sites.
    pub fn configured(
        model: NoiseModel,
        sampling: &NoiseSampling,
        excluded: &BTreeSet<usize>,
        options: NoiseOptions,
    ) -> Result<Self, NoiseError> {
        model.validate_exclusions(excluded)?;
        match sampling {
            NoiseSampling::Physical => Ok(Self::physical(model, options)),
            NoiseSampling::Importance { proposal_scale } => {
                let proposal = model.scaled_proposal(proposal_scale, excluded, &options)?;
                let summary = proposal.summary().clone();
                let mut sampler = Self::importance(model, proposal.into_model(), options)?;
                sampler.scaled_summary = Some(summary);
                Ok(sampler)
            }
        }
    }
    /// Present only for configured common-scale importance sampling, including
    /// a unit scale. Arbitrary proposals and physical sampling return None.
    pub fn scaled_proposal_summary(&self) -> Option<&ScaledProposalSummary> {
        self.scaled_summary.as_ref()
    }
    pub fn physical(model: NoiseModel, options: NoiseOptions) -> Self {
        let denominator_bits = model.denominator_bits();
        Self {
            proposal: model.clone(),
            physical: model,
            options,
            denominator_bits,
            same_distribution: true,
            scaled_summary: None,
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
        let same_distribution = physical
            .categories
            .iter()
            .zip(&proposal.categories)
            .all(|(p, q)| p.probabilities == q.probabilities);
        Ok(Self {
            physical,
            proposal,
            options,
            denominator_bits,
            same_distribution,
            scaled_summary: None,
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
            if choice != 0 {
                faults.push(FaultSelection {
                    site,
                    alternative: choice - 1,
                });
            }
        }
        let p = self.physical.probability(&faults)?;
        let q = if self.same_distribution {
            p.clone()
        } else {
            self.proposal.probability(&faults)?
        };
        let likelihood_ratio = if self.same_distribution {
            BigRational::one()
        } else {
            &p / &q
        };
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
