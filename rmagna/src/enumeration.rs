//! Bounded physical noise sectors, independent of quantum response evaluation.
use crate::instrument::FaultSelection;
use crate::noise::{NoiseError, NoiseModel};
use crate::probability::{Probability, ProbabilityError};
use num_rational::BigRational;
use num_traits::{One, Zero};
use std::fmt;

#[derive(Clone, Copy, Debug)]
pub struct EnumerationOptions {
    pub max_weight: usize,
    pub max_patterns: usize,
    pub max_fault_entries: usize,
    pub max_sites: usize,
    pub max_site_weight_pairs: usize,
    pub max_probability_bits: u64,
}
impl Default for EnumerationOptions {
    fn default() -> Self {
        Self {
            max_weight: 1,
            max_patterns: 100_000,
            max_fault_entries: 1_000_000,
            max_sites: 100_000,
            max_site_weight_pairs: 10_000_000,
            max_probability_bits: 1 << 20,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnumerationError {
    InvalidPatternLimit,
    ResourceLimit {
        option: &'static str,
        limit: u128,
        required: u128,
    },
    Noise(NoiseError),
    Probability(ProbabilityError),
}
impl fmt::Display for EnumerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPatternLimit => write!(f, "max_patterns must be positive"),
            Self::ResourceLimit {
                option,
                limit,
                required,
            } => write!(
                f,
                "noise enumeration needs {option} >= {required}; configured limit is {limit}"
            ),
            Self::Noise(error) => error.fmt(f),
            Self::Probability(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for EnumerationError {}
impl From<NoiseError> for EnumerationError {
    fn from(error: NoiseError) -> Self {
        Self::Noise(error)
    }
}
impl From<ProbabilityError> for EnumerationError {
    fn from(error: ProbabilityError) -> Self {
        Self::Probability(error)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoisePattern {
    pub faults: Vec<FaultSelection>,
    pub probability: Probability,
}
impl NoisePattern {
    pub fn weight(&self) -> usize {
        self.faults.len()
    }
}
#[derive(Clone, Debug)]
pub struct NoiseEnumeration {
    patterns: Vec<NoisePattern>,
    by_weight: Vec<Probability>,
    covered: Probability,
    omitted: Probability,
}
impl NoiseEnumeration {
    pub fn patterns(&self) -> &[NoisePattern] {
        &self.patterns
    }
    /// Entries cover 0..=min(max_weight, number of categories), including empty sectors.
    pub fn by_weight(&self) -> &[Probability] {
        &self.by_weight
    }
    pub fn covered_probability(&self) -> &Probability {
        &self.covered
    }
    pub fn omitted_probability(&self) -> &Probability {
        &self.omitted
    }
}
fn limit(option: &'static str, configured: u128, required: u128) -> Result<(), EnumerationError> {
    if required > configured {
        return Err(EnumerationError::ResourceLimit {
            option,
            limit: configured,
            required,
        });
    }
    Ok(())
}
fn check_probability(
    value: &BigRational,
    options: &EnumerationOptions,
) -> Result<(), EnumerationError> {
    limit(
        "max_probability_bits",
        options.max_probability_bits.into(),
        value.numer().bits().max(value.denom().bits()).into(),
    )
}
/// Next lexicographic k-combination, including the unique empty combination.
fn advance_combination(indices: &mut [usize], size: usize) -> bool {
    for j in (0..indices.len()).rev() {
        if indices[j] < size - (indices.len() - j) {
            indices[j] += 1;
            for k in j + 1..indices.len() {
                indices[k] = indices[k - 1] + 1;
            }
            return true;
        }
    }
    false
}

/// Enumerate only positive physical assignments. Pattern/fault-entry counts and
/// counting work are bounded before generation. Weight counts selected noise
/// categories, independently of Pauli support or later guard activation.
pub fn enumerate_noise(
    model: &NoiseModel,
    options: &EnumerationOptions,
) -> Result<NoiseEnumeration, EnumerationError> {
    if options.max_patterns == 0 {
        return Err(EnumerationError::InvalidPatternLimit);
    }
    let n = model.num_sites();
    let k = options.max_weight.min(n);
    limit("max_sites", options.max_sites as u128, n as u128)?;
    limit(
        "max_site_weight_pairs",
        options.max_site_weight_pairs as u128,
        (n as u128) * (k as u128 + 1),
    )?;
    // Counts saturate one past the caller's cap; no assignments are allocated
    // until every supported weight and the total retained entries fit.
    let bound = options.max_patterns as u128 + 1;
    let mut counts = vec![0_u128; k + 1];
    counts[0] = 1;
    let mut alternatives = Vec::with_capacity(n);
    let mut mandatory = Vec::new();
    let mut optional = Vec::new();
    for site in 0..n {
        let row = model
            .probabilities(site)
            .expect("site belongs to immutable model");
        let choices = row
            .iter()
            .enumerate()
            .skip(1)
            .filter_map(|(j, p)| (!p.value().is_zero()).then_some(j - 1))
            .collect::<Vec<_>>();
        let has_identity = !row[0].value().is_zero();
        for weight in (0..=k).rev() {
            let previous = if weight > 0 { counts[weight - 1] } else { 0 };
            let absent = if has_identity { counts[weight] } else { 0 };
            counts[weight] = absent
                .saturating_add(previous.saturating_mul(choices.len() as u128))
                .min(bound);
        }
        if !has_identity {
            mandatory.push(site);
        } else if !choices.is_empty() {
            optional.push(site);
        }
        alternatives.push(choices);
    }
    let count = counts
        .iter()
        .fold(0_u128, |sum, &c| sum.saturating_add(c).min(bound));
    limit("max_patterns", options.max_patterns as u128, count)?;
    let entries = counts.iter().enumerate().fold(0_u128, |sum, (w, &c)| {
        sum.saturating_add(c.saturating_mul(w as u128))
    });
    limit(
        "max_fault_entries",
        options.max_fault_entries as u128,
        entries,
    )?;
    let mut patterns = Vec::with_capacity(count as usize);
    let mut masses = vec![BigRational::zero(); k + 1];
    if mandatory.len() <= k {
        for extra in 0..=(k - mandatory.len()).min(optional.len()) {
            let mut indices = (0..extra).collect::<Vec<_>>();
            loop {
                let mut sites = mandatory
                    .iter()
                    .copied()
                    .chain(indices.iter().map(|&j| optional[j]))
                    .collect::<Vec<_>>();
                sites.sort_unstable();
                let mut choices = vec![0; sites.len()];
                loop {
                    let faults = sites
                        .iter()
                        .zip(&choices)
                        .map(|(&site, &j)| FaultSelection {
                            site,
                            alternative: alternatives[site][j],
                        })
                        .collect::<Vec<_>>();
                    let probability = model.probability(&faults)?;
                    check_probability(&probability, options)?;
                    masses[sites.len()] += &probability;
                    check_probability(&masses[sites.len()], options)?;
                    patterns.push(NoisePattern {
                        faults,
                        probability: Probability::from_rational(probability)?,
                    });
                    let mut changed = false;
                    for j in (0..choices.len()).rev() {
                        choices[j] += 1;
                        if choices[j] < alternatives[sites[j]].len() {
                            changed = true;
                            break;
                        }
                        choices[j] = 0;
                    }
                    if !changed {
                        break;
                    }
                }
                if !advance_combination(&mut indices, optional.len()) {
                    break;
                }
            }
        }
    }
    let covered = masses.iter().sum::<BigRational>();
    check_probability(&covered, options)?;
    let omitted = BigRational::one() - &covered;
    check_probability(&omitted, options)?;
    Ok(NoiseEnumeration {
        patterns,
        by_weight: masses
            .into_iter()
            .map(Probability::from_rational)
            .collect::<Result<_, _>>()?,
        covered: Probability::from_rational(covered)?,
        omitted: Probability::from_rational(omitted)?,
    })
}
