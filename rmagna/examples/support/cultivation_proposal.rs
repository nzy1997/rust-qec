//! Exact uniform-rate occurrence laws and the full-support detector-closing mixture.
use super::cultivation_flow::DetectorClosure;
use num_rational::BigRational;
use num_traits::{One, ToPrimitive, Zero};
use rmagna::gf2::BitVector;
use rmagna::instrument::FaultSelection;
use rmagna::noise::{OriginalNoiseCategory, RandomSource};
use rmagna::probability::Probability;
use serde::{Deserialize, Serialize};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// The paper inputs have equal positive conditional channel weights (1, 1/3,
/// or 1/15). Counts, rather than large rational weights, suffice per draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WeightKey {
    pub(crate) occurrences: usize,
    pub(crate) tail_candidates: usize,
    pub(crate) passing_candidates: usize,
    pub(crate) passes: bool,
}

pub(crate) struct Draw {
    pub(crate) faults: Vec<FaultSelection>,
    pub(crate) weight: WeightKey,
    pub(crate) entropy_words: usize,
}

struct Entropy<'a, R> {
    random: &'a mut R,
    used: usize,
    limit: usize,
}
impl<R: RandomSource> Entropy<'_, R> {
    fn below(&mut self, bound: u64) -> Result<u64> {
        if bound == 0 {
            return Err("cultivation range must have a positive bound".into());
        }
        if bound == 1 {
            return Ok(0);
        }
        let bits = 64 - (bound - 1).leading_zeros();
        let mask = u64::MAX >> (64 - bits);
        loop {
            if self.used == self.limit {
                return Err(
                    format!("cultivation draw exceeded max_entropy_words={}", self.limit).into(),
                );
            }
            self.used += 1;
            let value = self.random.next_u64() & mask;
            if value < bound {
                return Ok(value);
            }
        }
    }
}

pub(crate) struct Sampler {
    sites: Vec<usize>,
    channels: Vec<usize>,
    pub(crate) physical_rate: BigRational,
    pub(crate) proposal_rate: BigRational,
    occurrence_numerator: u64,
    occurrence_denominator: u64,
    closing_sites: usize,
    pub(crate) closure: Option<DetectorClosure>,
}

impl Sampler {
    pub(crate) fn new(
        categories: &[OriginalNoiseCategory],
        physical_rate: BigRational,
        closing: Option<(DetectorClosure, usize)>,
    ) -> Result<Self> {
        if categories.is_empty() || categories.len() > 65536 {
            return Err("cultivation requires 1..65536 original noise categories".into());
        }
        let physical_rate = Probability::from_rational(physical_rate)?.value().clone();
        let mut sites = Vec::with_capacity(categories.len());
        let mut channels = Vec::with_capacity(categories.len());
        for (index, category) in categories.iter().enumerate() {
            let count = category.channels.len();
            if category.guard.is_some() || count == 0 || count > 15 {
                return Err(format!(
                    "cultivation original site {index} requires 1..15 unguarded channels"
                )
                .into());
            }
            let first = category.channels[0].probability.value();
            if first.is_zero()
                || category
                    .channels
                    .iter()
                    .any(|channel| channel.probability.value() != first)
            {
                return Err(format!("cultivation original site {index} requires equal positive conditional channel weights").into());
            }
            if sites
                .last()
                .is_some_and(|previous| *previous >= category.internal_site)
            {
                return Err("cultivation original sites are not in unique source order".into());
            }
            sites.push(category.internal_site);
            channels.push(count);
        }
        let (closure, closing_sites, proposal_rate) = if let Some((closure, count)) = closing {
            if !matches!(count, 1 | 2) || categories.len() < 4 {
                return Err(
                    "cultivation closing requires 1/2 tail sites and at least four original sites"
                        .into(),
                );
            }
            if closure.signatures.len() != channels.len()
                || closure
                    .signatures
                    .iter()
                    .zip(&channels)
                    .any(|(row, &count)| {
                        row.len() != count
                            || row
                                .iter()
                                .any(|bits| bits.width() != closure.offset.width())
                    })
            {
                return Err(
                    "cultivation detector signature dimensions differ from original channels"
                        .into(),
                );
            }
            let rate = &physical_rate + BigRational::new(3.into(), categories.len().into());
            if rate >= BigRational::one() {
                return Err("cultivation proposal q=p+3/N must be strictly below one".into());
            }
            (Some(closure), count, rate)
        } else {
            (None, 0, physical_rate.clone())
        };
        let occurrence_numerator = proposal_rate
            .numer()
            .to_u64()
            .ok_or("cultivation fixed-grid occurrence numerator exceeds u64")?;
        let occurrence_denominator = proposal_rate
            .denom()
            .to_u64()
            .ok_or("cultivation fixed-grid occurrence denominator exceeds u64")?;
        Ok(Self {
            sites,
            channels,
            physical_rate,
            proposal_rate,
            occurrence_numerator,
            occurrence_denominator,
            closing_sites,
            closure,
        })
    }

    pub(crate) fn num_sites(&self) -> usize {
        self.sites.len()
    }

    pub(crate) fn draw(&self, random: &mut impl RandomSource, max_words: usize) -> Result<Draw> {
        let mut entropy = Entropy {
            random,
            used: 0,
            limit: max_words,
        };
        let (faults, weight) = self.draw_ranges(|bound| entropy.below(bound))?;
        Ok(Draw {
            faults,
            weight,
            entropy_words: entropy.used,
        })
    }

    /// Integer-range protocol used by independent CDF transcript checks.
    pub(crate) fn draw_ranges(
        &self,
        mut range: impl FnMut(u64) -> Result<u64>,
    ) -> Result<(Vec<FaultSelection>, WeightKey)> {
        let mut active = Vec::new();
        for site in 0..self.sites.len() {
            let value = choose(&mut range, self.occurrence_denominator)?;
            // The closing proposal follows the [1-q,q] CDF of B02. Physical
            // draws use the ordinary lower interval. Both laws remain exact.
            let occurs = if self.closure.is_some() {
                value >= self.occurrence_denominator - self.occurrence_numerator
            } else {
                value < self.occurrence_numerator
            };
            if occurs {
                active.push(site);
            }
        }
        let mut weight = WeightKey {
            occurrences: active.len(),
            tail_candidates: 0,
            passing_candidates: 0,
            passes: false,
        };
        let mut faults = Vec::with_capacity(active.len());
        let prefix = active.len().saturating_sub(self.closing_sites);
        let mut parity = self.closure.as_ref().map(|closure| closure.offset.clone());
        for &site in &active[..prefix] {
            let alternative = choose(&mut range, self.channels[site] as u64)? as usize;
            faults.push(FaultSelection {
                site: self.sites[site],
                alternative,
            });
            if let (Some(closure), Some(parity)) = (&self.closure, &mut parity) {
                parity.xor_assign(&closure.signatures[site][alternative])?;
            }
        }
        if let Some(closure) = &self.closure {
            let tail = &active[prefix..];
            let counts = tail
                .iter()
                .map(|&site| self.channels[site])
                .collect::<Vec<_>>();
            let candidates = counts.iter().product::<usize>();
            let parity = parity.as_ref().expect("closing parity is present");
            let mut passing = Vec::new();
            for candidate in 0..candidates {
                let alternatives = decode(candidate, &counts);
                let mut value = BitVector::zeros(closure.offset.width())?;
                for (&site, &alternative) in tail.iter().zip(&alternatives) {
                    value.xor_assign(&closure.signatures[site][alternative])?;
                }
                if &value == parity {
                    passing.push(candidate);
                }
            }
            let conditioned = !passing.is_empty() && choose(&mut range, 20)? != 0;
            let selected = if conditioned {
                passing[choose(&mut range, passing.len() as u64)? as usize]
            } else {
                choose(&mut range, candidates as u64)? as usize
            };
            weight.tail_candidates = candidates;
            weight.passing_candidates = passing.len();
            weight.passes = passing.binary_search(&selected).is_ok();
            for (&site, alternative) in tail.iter().zip(decode(selected, &counts)) {
                faults.push(FaultSelection {
                    site: self.sites[site],
                    alternative,
                });
            }
        }
        Ok((faults, weight))
    }

    /// Exact validation/export operation; never required in the per-draw loop.
    #[cfg(test)]
    pub(crate) fn likelihood(&self, key: WeightKey) -> Result<BigRational> {
        if key.occurrences > self.sites.len() {
            return Err("cultivation occurrence count exceeds original sites".into());
        }
        if self.closure.is_none() {
            if key.tail_candidates != 0 || key.passing_candidates != 0 || key.passes {
                return Err("physical cultivation draw contains a closing weight".into());
            }
            return Ok(BigRational::one());
        }
        if key.tail_candidates == 0
            || key.tail_candidates > 225
            || key.passing_candidates > key.tail_candidates
            || (key.passing_candidates == 0 && key.passes)
            || (key.passing_candidates == key.tail_candidates && !key.passes)
            || (key.occurrences == 0
                && (key.tail_candidates != 1
                    || key.passing_candidates
                        != usize::from(self.closure.as_ref().unwrap().offset.is_zero())))
        {
            return Err("invalid cultivation closing normalizer".into());
        }
        let ratio = (&self.physical_rate / &self.proposal_rate).pow(key.occurrences as i32)
            * ((BigRational::one() - &self.physical_rate)
                / (BigRational::one() - &self.proposal_rate))
                .pow((self.sites.len() - key.occurrences) as i32);
        let mixture = if key.passing_candidates == 0 {
            BigRational::one()
        } else if key.passes {
            BigRational::new(
                (key.passing_candidates + 19 * key.tail_candidates).into(),
                (20 * key.passing_candidates).into(),
            )
        } else {
            BigRational::new(1.into(), 20.into())
        };
        Ok(ratio / mixture)
    }
}

fn choose(range: &mut impl FnMut(u64) -> Result<u64>, bound: u64) -> Result<u64> {
    if bound == 0 {
        return Err("cultivation range must have a positive bound".into());
    }
    if bound == 1 {
        return Ok(0);
    }
    let value = range(bound)?;
    if value >= bound {
        return Err("cultivation range source returned a value outside its bound".into());
    }
    Ok(value)
}

fn decode(mut value: usize, counts: &[usize]) -> Vec<usize> {
    let mut result = vec![0; counts.len()];
    for (index, &count) in counts.iter().enumerate().rev() {
        result[index] = value % count;
        value /= count;
    }
    result
}
