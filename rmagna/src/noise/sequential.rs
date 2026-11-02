//! Explicit occurrence/channel sampling with a caller-owned range stream.

use super::{
    ConditionalLawError, ConditionalSiteLaw, NoiseError, NoiseModel, NoiseOptions, NoiseSampler,
    NoiseSampling, RandomSource, ScaledProposalSummary, uniform_below,
};
use crate::instrument::FaultSelection;
use num_bigint::BigUint;
use num_rational::BigRational;
use num_traits::{One, Zero};
use std::{collections::BTreeSet, fmt};

/// Supplies an integer in `0..upper_exclusive`, including a request with bound
/// one. A sampler validates every response. Its call budget counts requests
/// even when a source implements a deterministic bound without consuming bits.
/// This protocol does not prescribe a particular pseudorandom generator.
pub trait RandRangeSource {
    type Error;
    fn randrange(&mut self, upper_exclusive: &BigUint) -> Result<BigUint, Self::Error>;
}

impl<F, E> RandRangeSource for F
where
    F: FnMut(&BigUint) -> Result<BigUint, E>,
{
    type Error = E;

    fn randrange(&mut self, upper_exclusive: &BigUint) -> Result<BigUint, E> {
        self(upper_exclusive)
    }
}

/// Bounded rejection sampling from the existing unbiased 64-bit word source.
/// Its word budget is cumulative for the lifetime of this adapter. Dropping
/// it releases the caller's stream; constructing another adapter starts a new
/// budget without rewinding that stream. Bound one consumes no random words.
/// This is independent of Python's `Random.randrange` implementation.
pub struct WordRangeSource<'a, R> {
    random: &'a mut R,
    limit: usize,
    used: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WordRangeError {
    EmptyRange,
    Entropy(NoiseError),
}

impl fmt::Display for WordRangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRange => f.write_str("range source requires a positive exclusive bound"),
            Self::Entropy(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for WordRangeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EmptyRange => None,
            Self::Entropy(error) => Some(error),
        }
    }
}

impl<'a, R: RandomSource> WordRangeSource<'a, R> {
    pub fn new(random: &'a mut R, max_entropy_words: usize) -> Self {
        Self {
            random,
            limit: max_entropy_words,
            used: 0,
        }
    }

    pub fn entropy_words(&self) -> usize {
        self.used
    }
}

impl<R: RandomSource> RandRangeSource for WordRangeSource<'_, R> {
    type Error = WordRangeError;

    fn randrange(&mut self, upper_exclusive: &BigUint) -> Result<BigUint, WordRangeError> {
        // A sampler only supplies positive bounds. Direct adapter callers also
        // get a typed error instead of subtracting one from a zero BigUint.
        if upper_exclusive.is_zero() {
            return Err(WordRangeError::EmptyRange);
        }
        uniform_below(upper_exclusive, self.random, &mut self.used, self.limit)
            .map_err(WordRangeError::Entropy)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SequentialNoiseOptions {
    pub max_denominator_bits: u64,
    pub max_sites: usize,
    /// Maximum number of conditional channel entries across all sites.
    pub max_channels: usize,
    /// Per-draw callback budget, including deterministic endpoint requests.
    pub max_range_calls: usize,
}

impl Default for SequentialNoiseOptions {
    fn default() -> Self {
        Self {
            max_denominator_bits: NoiseOptions::default().max_denominator_bits,
            max_sites: 1 << 20,
            max_channels: 1 << 20,
            max_range_calls: 1 << 20,
        }
    }
}

/// Equal weights can still use either protocol; the choice is explicit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelSelection {
    Uniform,
    SequentialWeights,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SequentialNoiseError {
    Law(ConditionalLawError),
    Model(NoiseError),
    SiteLimit { limit: usize, requested: usize },
    ChannelLimit { limit: usize, requested: usize },
    ChannelCountOverflow,
}

impl fmt::Display for SequentialNoiseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Law(error) => error.fmt(f),
            Self::Model(error) => error.fmt(f),
            Self::SiteLimit { limit, requested } => write!(
                f,
                "sequential noise declares {requested} sites, limit is {limit}"
            ),
            Self::ChannelLimit { limit, requested } => write!(
                f,
                "sequential noise declares {requested} channels, limit is {limit}"
            ),
            Self::ChannelCountOverflow => {
                f.write_str("sequential noise channel count overflows usize")
            }
        }
    }
}

impl std::error::Error for SequentialNoiseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Law(error) => Some(error),
            Self::Model(error) => Some(error),
            _ => None,
        }
    }
}

/// An owned conditional law and its explicit channel-selection protocol.
/// Actions and any source labels remain in the caller's channel catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SequentialNoiseSite {
    law: ConditionalSiteLaw,
    selection: ChannelSelection,
}

impl SequentialNoiseSite {
    /// `None` requests uniform channel selection; `Some` retains sequential
    /// conditional selection, including when all supplied weights are equal.
    /// Limits are checked before allocating uniform weights.
    pub fn new(
        declared_channels: usize,
        occurrence: BigRational,
        weights: Option<Vec<BigRational>>,
        options: &SequentialNoiseOptions,
    ) -> Result<Self, SequentialNoiseError> {
        check_channels(declared_channels, options.max_channels)?;
        let (weights, selection) = match weights {
            Some(weights) => (weights, ChannelSelection::SequentialWeights),
            None if declared_channels == 0 => {
                return Err(SequentialNoiseError::Law(ConditionalLawError::NoChannels));
            }
            None => {
                let required = BigUint::from(declared_channels).bits();
                if required > options.max_denominator_bits {
                    return Err(SequentialNoiseError::Model(NoiseError::DenominatorLimit {
                        limit: options.max_denominator_bits,
                        required,
                    }));
                }
                let weight = BigRational::new(1.into(), declared_channels.into());
                (vec![weight; declared_channels], ChannelSelection::Uniform)
            }
        };
        let law = ConditionalSiteLaw::new(declared_channels, occurrence, weights)
            .map_err(SequentialNoiseError::Law)?;
        Ok(Self { law, selection })
    }

    pub fn from_weighted_law(
        law: ConditionalSiteLaw,
        options: &SequentialNoiseOptions,
    ) -> Result<Self, SequentialNoiseError> {
        check_channels(law.num_channels(), options.max_channels)?;
        Ok(Self {
            law,
            selection: ChannelSelection::SequentialWeights,
        })
    }

    pub fn law(&self) -> &ConditionalSiteLaw {
        &self.law
    }

    pub fn channel_selection(&self) -> ChannelSelection {
        self.selection
    }
}

fn check_channels(requested: usize, limit: usize) -> Result<(), SequentialNoiseError> {
    if requested > limit {
        return Err(SequentialNoiseError::ChannelLimit { limit, requested });
    }
    Ok(())
}

fn denominator(value: &BigRational, limit: u64) -> Result<BigUint, SequentialNoiseError> {
    let denominator = value
        .denom()
        .to_biguint()
        .expect("validated positive denominator");
    if denominator.bits() > limit {
        return Err(SequentialNoiseError::Model(NoiseError::DenominatorLimit {
            limit,
            required: denominator.bits(),
        }));
    }
    Ok(denominator)
}

#[derive(Clone, Debug)]
struct ChannelRequest {
    numerator: BigUint,
    denominator: BigUint,
}

#[derive(Clone, Debug)]
/// Owned two-stage noise sampling with an injectable exact range protocol.
///
/// ```
/// use num_rational::BigRational;
/// use rmagna::noise::{NoiseSampling, SequentialNoiseOptions, SequentialNoiseSampler,
///     SequentialNoiseSite, SplitMix64, WordRangeSource};
/// let options = SequentialNoiseOptions::default();
/// let site = SequentialNoiseSite::new(1, BigRational::from_integer(1.into()), None, &options)?;
/// let sampler = SequentialNoiseSampler::configured(vec![site], None,
///     &NoiseSampling::Physical, options)?;
/// let mut random = SplitMix64::new(17);
/// let mut source = WordRangeSource::new(&mut random, 0);
/// let draw = sampler.draw(&mut source)?;
/// assert_eq!(draw.channels, vec![Some(0)]);
/// assert_eq!(draw.range_calls, 2);
/// assert_eq!(source.entropy_words(), 0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct SequentialNoiseSampler {
    sites: Vec<SequentialNoiseSite>,
    categorical: NoiseSampler,
    conditional_requests: Vec<Vec<ChannelRequest>>,
    options: SequentialNoiseOptions,
}

/// Every position corresponds to the supplied site order. `Some(channel)`
/// records occurrence even if that channel's caller-owned action is empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SequentialNoiseDraw {
    pub channels: Vec<Option<usize>>,
    pub physical_probability: BigRational,
    pub proposal_probability: BigRational,
    pub likelihood_ratio: BigRational,
    pub range_calls: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SequentialDrawError<E> {
    Source(E),
    OutOfRange {
        upper_exclusive: BigUint,
        value: BigUint,
    },
    RangeCallLimit {
        limit: usize,
    },
    Model(NoiseError),
}

impl<E: fmt::Display> fmt::Display for SequentialDrawError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(f, "sequential range source failed: {error}"),
            Self::OutOfRange {
                upper_exclusive,
                value,
            } => write!(
                f,
                "range source returned {value}, expected an integer in 0..{upper_exclusive}"
            ),
            Self::RangeCallLimit { limit } => {
                write!(
                    f,
                    "sequential noise exhausted its {limit} range-call budget"
                )
            }
            Self::Model(error) => error.fmt(f),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SequentialDrawError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Model(error) => Some(error),
            _ => None,
        }
    }
}

impl SequentialNoiseSampler {
    /// Reuse canonical effective-rate, common-scale support and exact mass
    /// validation. Uniform and weighted channel protocols remain distinct.
    /// Denominator budgets also validate retained conditional weights at
    /// inactive occurrence endpoints, before any random-source request.
    pub fn configured(
        sites: Vec<SequentialNoiseSite>,
        effective_physical_rate: Option<BigRational>,
        sampling: &NoiseSampling,
        options: SequentialNoiseOptions,
    ) -> Result<Self, SequentialNoiseError> {
        if sites.len() > options.max_sites {
            return Err(SequentialNoiseError::SiteLimit {
                limit: options.max_sites,
                requested: sites.len(),
            });
        }
        let mut total = 0usize;
        for site in &sites {
            total = total
                .checked_add(site.law.num_channels())
                .ok_or(SequentialNoiseError::ChannelCountOverflow)?;
            check_channels(total, options.max_channels)?;
        }
        let laws = sites
            .iter()
            .map(|site| site.law.clone())
            .collect::<Vec<_>>();
        // Canonical construction supplies probability and support validation.
        // The caller's range source owns its independent entropy budget.
        let noise_options = NoiseOptions {
            max_denominator_bits: options.max_denominator_bits,
            max_entropy_words: 0,
        };
        let physical =
            NoiseModel::from_conditional_laws(&laws, effective_physical_rate, &noise_options)
                .map_err(SequentialNoiseError::Law)?;
        let categorical =
            NoiseSampler::configured(physical, sampling, &BTreeSet::new(), noise_options)
                .map_err(SequentialNoiseError::Model)?;
        let mut conditional_requests = Vec::with_capacity(sites.len());
        for site in &sites {
            let mut remaining = BigRational::one();
            let mut requests = Vec::with_capacity(site.law.num_channels());
            for weight in site.law.conditional_weights() {
                // Even unreachable tails retain validated denominators, but
                // never divide by a zero remaining mass.
                denominator(weight.value(), options.max_denominator_bits)?;
                let conditional = if remaining.is_zero() {
                    BigRational::zero()
                } else {
                    weight.value() / &remaining
                };
                requests.push(ChannelRequest {
                    numerator: conditional
                        .numer()
                        .to_biguint()
                        .expect("nonnegative weight"),
                    denominator: denominator(&conditional, options.max_denominator_bits)?,
                });
                remaining -= weight.value();
            }
            conditional_requests.push(requests);
        }
        Ok(Self {
            sites,
            categorical,
            conditional_requests,
            options,
        })
    }

    pub fn sites(&self) -> &[SequentialNoiseSite] {
        &self.sites
    }

    pub fn scaled_proposal_summary(&self) -> Option<&ScaledProposalSummary> {
        self.categorical.scaled_proposal_summary()
    }

    pub fn draw<R: RandRangeSource + ?Sized>(
        &self,
        source: &mut R,
    ) -> Result<SequentialNoiseDraw, SequentialDrawError<R::Error>> {
        let mut range_calls = 0;
        let mut channels = Vec::with_capacity(self.sites.len());
        let mut faults = Vec::new();
        for (index, site) in self.sites.iter().enumerate() {
            let q = self.categorical.proposal.categories[index].probabilities[0].complement();
            let bound = q
                .value()
                .denom()
                .to_biguint()
                .expect("positive denominator");
            let value = self.request(source, &bound, &mut range_calls)?;
            if value
                >= q.value()
                    .numer()
                    .to_biguint()
                    .expect("nonnegative probability")
            {
                channels.push(None);
                continue;
            }
            let channel = match site.selection {
                ChannelSelection::Uniform => {
                    let bound = BigUint::from(site.law.num_channels());
                    let value = self.request(source, &bound, &mut range_calls)?;
                    // A value below a usize-derived bound fits usize.
                    use num_traits::ToPrimitive;
                    value
                        .to_usize()
                        .expect("validated channel index fits usize")
                }
                ChannelSelection::SequentialWeights => {
                    let mut selected = None;
                    for (channel, chance) in self.conditional_requests[index].iter().enumerate() {
                        if chance.numerator.is_zero() {
                            continue;
                        }
                        if self.request(source, &chance.denominator, &mut range_calls)?
                            < chance.numerator
                        {
                            selected = Some(channel);
                            break;
                        }
                    }
                    // Unit-sum weights always reach a conditional probability
                    // one, whose only valid source response is zero.
                    selected.expect("validated unit-sum channel law selects a channel")
                }
            };
            channels.push(Some(channel));
            faults.push(FaultSelection {
                site: index,
                alternative: channel,
            });
        }
        let physical_probability = self
            .categorical
            .physical
            .probability(&faults)
            .map_err(SequentialDrawError::Model)?;
        let proposal_probability = self
            .categorical
            .proposal
            .probability(&faults)
            .map_err(SequentialDrawError::Model)?;
        let likelihood_ratio = &physical_probability / &proposal_probability;
        Ok(SequentialNoiseDraw {
            channels,
            physical_probability,
            proposal_probability,
            likelihood_ratio,
            range_calls,
        })
    }

    fn request<R: RandRangeSource + ?Sized>(
        &self,
        source: &mut R,
        bound: &BigUint,
        calls: &mut usize,
    ) -> Result<BigUint, SequentialDrawError<R::Error>> {
        if *calls >= self.options.max_range_calls {
            return Err(SequentialDrawError::RangeCallLimit {
                limit: self.options.max_range_calls,
            });
        }
        *calls += 1;
        let value = source
            .randrange(bound)
            .map_err(SequentialDrawError::Source)?;
        if &value >= bound {
            return Err(SequentialDrawError::OutOfRange {
                upper_exclusive: bound.clone(),
                value,
            });
        }
        Ok(value)
    }
}
