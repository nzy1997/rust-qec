//! Prepared seeded exact categorical draws with bounded packed occurrence groups.
use super::prepared_weights::{
    NoiseWeightError, NoiseWeightLimits, NoiseWeightStatistics, PreparedNoiseWeights,
    RawNoiseFactor,
};
use super::{
    NoiseError, NoiseModel, NoiseOptions, NoiseSampler, NoiseSampling, RandomSource,
    SparseNoiseAction, SparseNoiseError, SparseNoiseOptions, SparseNoiseSampler, SplitMix64,
};
use crate::instrument::FaultSelection;
use crate::probability::{OccurrenceThreshold, Probability};
use num_rational::BigRational;
use num_traits::{One, Zero};
use std::collections::BTreeMap;
mod admission;
#[derive(Clone, Copy, Debug)]
pub struct PreparedNoiseOptions {
    pub sparse: SparseNoiseOptions,
    pub weights: NoiseWeightLimits,
    pub max_batch: usize,
    /// Conservative compiled numeric-model payload; excludes named actions,
    /// separately bounded raw count tables, allocator capacity and scratch.
    pub max_model_payload_bytes: usize,
    pub max_batch_cells: usize,
    /// Conservative owned output fields/magnitude payload; excludes allocator
    /// headers, spare capacity and construction/evaluation scratch, not RSS.
    pub max_batch_payload_bytes: usize,
}
impl Default for PreparedNoiseOptions {
    fn default() -> Self {
        Self {
            sparse: Default::default(),
            weights: Default::default(),
            max_batch: 65536,
            max_model_payload_bytes: 128 << 20,
            max_batch_cells: 1 << 20,
            max_batch_payload_bytes: 64 << 20,
        }
    }
}
#[derive(Debug)]
pub enum PreparedNoiseError {
    Noise(NoiseError),
    Sparse(SparseNoiseError),
    Weight(NoiseWeightError),
    Shape {
        field: &'static str,
        site: Option<usize>,
        expected: usize,
        actual: usize,
    },
    Category(usize),
    BatchLimit {
        required: usize,
        limit: usize,
    },
    RequestLimit {
        option: &'static str,
        required: u128,
        limit: usize,
    },
}
impl From<NoiseError> for PreparedNoiseError {
    fn from(e: NoiseError) -> Self {
        Self::Noise(e)
    }
}
impl From<SparseNoiseError> for PreparedNoiseError {
    fn from(e: SparseNoiseError) -> Self {
        Self::Sparse(e)
    }
}
impl From<NoiseWeightError> for PreparedNoiseError {
    fn from(e: NoiseWeightError) -> Self {
        Self::Weight(e)
    }
}
struct Group {
    sites: Vec<usize>,
    threshold: OccurrenceThreshold,
    cutoff: u64,
    exact_tail: Option<NoiseSampler>,
}
enum Conditional {
    Never,
    Certain(usize),
    Model(usize),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreparedNoiseStatistics {
    pub sites: usize,
    pub conservative_model_payload_bytes: usize,
    pub occurrence_groups: usize,
    pub packed_groups: usize,
    pub exact_tail_groups: usize,
    pub endpoint_groups: usize,
    pub conditional_models: usize,
    pub weights: NoiseWeightStatistics,
}
#[derive(Clone, Debug, PartialEq, Eq)]
/// An owned draw. `raw_weight` is the caller-declared factor product, not an
/// automatically established physical/proposal likelihood ratio.
pub struct PreparedNoiseDraw {
    pub choices: Vec<Option<usize>>,
    pub actions: Vec<SparseNoiseAction>,
    pub raw_weight: RawNoiseFactor,
    pub physical_probability: BigRational,
    pub entropy_words: usize,
    /// Actual uniform bit planes requested by the occurrence driver.
    pub occurrence_planes: usize,
    /// Actual exact scalar residual evaluations, only for U == floor(p*2^64).
    pub exact_tail_evaluations: usize,
}
/// Immutable, unguarded seeded categorical schedule. Equal occurrence totals
/// share bit-plane groups. A scalar residual corrects the finite threshold
/// exactly at its equality boundary; conditional alternatives also use exact
/// scalar samplers. Tiny positive probabilities therefore remain possible.
/// Batches and their seed partitions agree; packed and scalar algorithms need
/// not assign the same outcomes to the same seed. Guards require causal runtime.
///
/// ```
/// use rmagna::noise::{PreparedNoiseSchedule,PreparedNoiseOptions,SparseNoiseAction};
/// # fn main()->Result<(),Box<dyn std::error::Error>> {
/// let laws=vec![vec!["1/4".parse()?];3];
/// let actions=(0..3).map(|j|vec![SparseNoiseAction::ReportFlip{record:format!("m{j}")}]).collect::<Vec<_>>();
/// let plan=PreparedNoiseSchedule::unweighted(&laws,&actions,PreparedNoiseOptions::default())?;
/// let draws=plan.draw_seeded_many(&[11,22])?;
/// assert_eq!(draws.len(),2);
/// assert_eq!(plan.statistics().packed_groups,1);
/// # Ok(()) }
/// ```
pub struct PreparedNoiseSchedule {
    draw_payload_bound: u128,
    owner: SparseNoiseSampler,
    groups: Vec<Group>,
    conditional: Vec<Conditional>,
    models: Vec<NoiseSampler>,
    weights: PreparedNoiseWeights,
    options: PreparedNoiseOptions,
    statistics: PreparedNoiseStatistics,
}
fn sampler(
    row: Vec<Probability>,
    options: NoiseOptions,
) -> Result<NoiseSampler, PreparedNoiseError> {
    Ok(NoiseSampler::physical(
        NoiseModel::new(vec![row], &options)?,
        options,
    ))
}
struct Budget<'a, R> {
    inner: &'a mut R,
    used: usize,
    limit: usize,
    exceeded: bool,
}
impl<R: RandomSource> RandomSource for Budget<'_, R> {
    fn next_u64(&mut self) -> u64 {
        if self.used >= self.limit {
            self.exceeded = true;
            0
        } else {
            self.used += 1;
            self.inner.next_u64()
        }
    }
}
impl<R: RandomSource> Budget<'_, R> {
    fn check(&self) -> Result<(), PreparedNoiseError> {
        if self.exceeded {
            Err(NoiseError::EntropyLimit { limit: self.limit }.into())
        } else {
            Ok(())
        }
    }
}
// The existing finite primitive can stop after K's last nonzero bit. In the
// non-dyadic case, retain equal-prefix lanes and finish only their comparison
// against K. No scalar tail runs for a lane already below or above the cutoff.
fn draw_occurrences<R: RandomSource>(
    group: &Group,
    live: u64,
    rng: &mut Budget<'_, R>,
    planes: &mut usize,
    tails: &mut usize,
) -> Result<u64, PreparedNoiseError> {
    if group.exact_tail.is_none() {
        let selected = group.threshold.draw_bits_with(live, &mut || {
            *planes += 1;
            rng.next_u64()
        });
        rng.check()?;
        return Ok(selected);
    }
    let mut equal = live;
    let mut position = 0usize;
    let mut selected = {
        let mut next = || {
            let bits = rng.next_u64();
            *planes += 1;
            let one = (group.cutoff >> (63 - position)) & 1 != 0;
            equal &= if one { bits } else { !bits };
            position += 1;
            bits
        };
        group.threshold.draw_bits_with(live, &mut next)
    };
    rng.check()?;
    while position < 64 && equal != 0 {
        let bits = rng.next_u64();
        *planes += 1;
        rng.check()?;
        let one = (group.cutoff >> (63 - position)) & 1 != 0;
        equal &= if one { bits } else { !bits };
        position += 1;
    }
    let sampler = group.exact_tail.as_ref().expect("residual route");
    while equal != 0 {
        let lane = equal.trailing_zeros();
        equal &= equal - 1;
        *tails += 1;
        let draw = sampler.draw(rng)?;
        rng.check()?;
        if !draw.faults.is_empty() {
            selected |= 1u64 << lane;
        }
    }
    Ok(selected)
}
fn prepare_occurrences(
    totals: &[Probability],
    options: NoiseOptions,
) -> Result<(Vec<Group>, PreparedNoiseStatistics), PreparedNoiseError> {
    let mut groups: Vec<Group> = vec![];
    let mut keys = BTreeMap::<BigRational, usize>::new();
    let mut statistics = PreparedNoiseStatistics::default();
    for (site, total) in totals.iter().enumerate() {
        let key = total.value().clone();
        if let Some(&group) = keys.get(&key) {
            groups[group].sites.push(site);
            continue;
        }
        // p = K/2^64 + (R/D)/2^64. The finite kernel handles U<K;
        // only U==K needs a new exact Bernoulli(R/D), so arbitrary rational
        // laws remain exact without scalar occurrence work at every site.
        let remainder = (key.numer() << 64usize) % key.denom();
        let endpoint = key.is_zero() || key.is_one();
        let threshold = total.threshold_64();
        let cutoff = match threshold {
            OccurrenceThreshold::Below(k) => k,
            _ => 0,
        };
        let exact_tail = if remainder.is_zero() {
            None
        } else {
            let residual = Probability::from_ratio(remainder, key.denom().clone())
                .expect("proper exact residual");
            Some(sampler(vec![residual.complement(), residual], options)?)
        };
        statistics.endpoint_groups += usize::from(endpoint);
        statistics.packed_groups += usize::from(!endpoint);
        statistics.exact_tail_groups += usize::from(exact_tail.is_some());
        keys.insert(key, groups.len());
        groups.push(Group {
            sites: vec![site],
            threshold,
            cutoff,
            exact_tail,
        });
    }
    Ok((groups, statistics))
}
fn prepare_conditionals(
    channels: &[Vec<Probability>],
    totals: &[Probability],
    options: NoiseOptions,
) -> Result<(Vec<Conditional>, Vec<NoiseSampler>), PreparedNoiseError> {
    let mut conditional = vec![];
    let mut models = vec![];
    let mut model_keys = BTreeMap::<Vec<BigRational>, usize>::new();
    for (law, total) in channels.iter().zip(totals) {
        let nonzero = law
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.value().is_zero())
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        if nonzero.is_empty() {
            conditional.push(Conditional::Never);
            continue;
        }
        if nonzero.len() == 1 {
            conditional.push(Conditional::Certain(nonzero[0]));
            continue;
        }
        let key = law
            .iter()
            .map(|p| p.value() / total.value())
            .collect::<Vec<_>>();
        let model = match model_keys.get(&key) {
            Some(&i) => i,
            None => {
                let mut row = vec![Probability::zero()];
                row.extend(key.iter().cloned().map(|p| {
                    Probability::from_rational(p).expect("normalized nonnegative category")
                }));
                let index = models.len();
                models.push(sampler(row, options)?);
                model_keys.insert(key, index);
                index
            }
        };
        conditional.push(Conditional::Model(model));
    }
    Ok((conditional, models))
}
impl PreparedNoiseSchedule {
    /// Own nonidentity category probabilities, one action per alternative,
    /// and (identity, nonidentity) raw factor pairs. Limits precede output/RNG.
    pub fn new(
        channels: &[Vec<Probability>],
        actions: &[Vec<SparseNoiseAction>],
        factors: &[(RawNoiseFactor, RawNoiseFactor)],
        options: PreparedNoiseOptions,
    ) -> Result<Self, PreparedNoiseError> {
        let admission::Admitted {
            totals,
            rows,
            model_payload_bound,
            draw_payload_bound,
        } = admission::admit(channels, actions, factors, options)?;
        let weights = PreparedNoiseWeights::new(factors, options.weights)?;
        let owner = SparseNoiseSampler::new(
            NoiseModel::new(rows, &options.sparse.noise)?,
            actions,
            &NoiseSampling::Physical,
            options.sparse,
        )?;
        let (groups, mut statistics) = prepare_occurrences(&totals, options.sparse.noise)?;
        let (conditional, models) = prepare_conditionals(channels, &totals, options.sparse.noise)?;
        statistics.sites = channels.len();
        statistics.conservative_model_payload_bytes = model_payload_bound as usize;
        statistics.weights = weights.statistics();
        statistics.conditional_models = models.len();
        statistics.occurrence_groups = groups.len();
        Ok(Self {
            draw_payload_bound,
            owner,
            groups,
            conditional,
            models,
            weights,
            options,
            statistics,
        })
    }
    /// Prepare unit factors after admitting site counts, preserving the exact
    /// same occurrence/alternative stream as a weighted plan with these laws.
    pub fn unweighted(
        channels: &[Vec<Probability>],
        actions: &[Vec<SparseNoiseAction>],
        options: PreparedNoiseOptions,
    ) -> Result<Self, PreparedNoiseError> {
        if channels.len() > options.weights.max_sites {
            return Err(NoiseWeightError::Sites {
                required: channels.len(),
                limit: options.weights.max_sites,
            }
            .into());
        }
        if channels.len() > options.sparse.max_sites {
            return Err(SparseNoiseError::Limit {
                option: "sparse_noise.max_sites",
                required: channels.len() as u128,
                limit: options.sparse.max_sites,
            }
            .into());
        }
        let unit = vec![(RawNoiseFactor::one(), RawNoiseFactor::one()); channels.len()];
        Self::new(channels, actions, &unit, options)
    }
    pub fn statistics(&self) -> PreparedNoiseStatistics {
        self.statistics
    }
    pub fn draw_seeded_many(
        &self,
        seeds: &[u64],
    ) -> Result<Vec<PreparedNoiseDraw>, PreparedNoiseError> {
        if seeds.len() > self.options.max_batch {
            return Err(PreparedNoiseError::BatchLimit {
                required: seeds.len(),
                limit: self.options.max_batch,
            });
        }
        self.check_output(seeds.len())?;
        seeds
            .iter()
            .map(|&seed| self.draw(&mut SplitMix64::new(seed)))
            .collect()
    }
    pub fn draw_scalar_seeded(&self, seed: u64) -> Result<PreparedNoiseDraw, PreparedNoiseError> {
        self.check_output(1)?;
        let draw = self.owner.draw(&mut SplitMix64::new(seed))?;
        self.assemble(draw.noise.faults, draw.noise.entropy_words, 0, 0)
    }
    pub fn draw(
        &self,
        random: &mut impl RandomSource,
    ) -> Result<PreparedNoiseDraw, PreparedNoiseError> {
        self.check_output(1)?;
        let mut rng = Budget {
            inner: random,
            used: 0,
            limit: self.options.sparse.noise.max_entropy_words,
            exceeded: false,
        };
        let mut selected = vec![false; self.statistics.sites];
        let mut planes = 0usize;
        let mut tails = 0usize;
        for group in &self.groups {
            for block in group.sites.chunks(64) {
                let live = if block.len() == 64 {
                    u64::MAX
                } else {
                    (1u64 << block.len()) - 1
                };
                let bits = draw_occurrences(group, live, &mut rng, &mut planes, &mut tails)?;
                for (j, &site) in block.iter().enumerate() {
                    selected[site] = bits >> j & 1 != 0;
                }
            }
        }
        let mut faults = vec![];
        for (site, &yes) in selected.iter().enumerate() {
            if !yes {
                continue;
            }
            let alternative = match self.conditional[site] {
                Conditional::Never => unreachable!("zero total cannot occur"),
                Conditional::Certain(i) => i,
                Conditional::Model(i) => {
                    let conditional = self.models[i].draw(&mut rng)?;
                    rng.check()?;
                    conditional.faults[0].alternative
                }
            };
            faults.push(FaultSelection { site, alternative });
        }
        self.assemble(faults, rng.used, planes, tails)
    }
    pub fn draw_payload_bound(&self) -> u128 {
        self.draw_payload_bound
    }
    fn check_output(&self, count: usize) -> Result<(), PreparedNoiseError> {
        for (option, required, limit) in [
            (
                "max_batch_cells",
                count as u128 * self.statistics.sites as u128,
                self.options.max_batch_cells,
            ),
            (
                "max_batch_payload_bytes",
                count as u128 * self.draw_payload_bound,
                self.options.max_batch_payload_bytes,
            ),
        ] {
            if required > limit as u128 {
                return Err(PreparedNoiseError::RequestLimit {
                    option,
                    required,
                    limit,
                });
            }
        }
        Ok(())
    }
    fn assemble(
        &self,
        faults: Vec<FaultSelection>,
        entropy_words: usize,
        occurrence_planes: usize,
        exact_tail_evaluations: usize,
    ) -> Result<PreparedNoiseDraw, PreparedNoiseError> {
        let physical = self.owner.prescribed(&faults)?.noise.physical_probability;
        let mut choices = vec![None; self.statistics.sites];
        let mut selected = vec![false; self.statistics.sites];
        let mut actions = vec![];
        for f in faults {
            choices[f.site] = Some(f.alternative);
            selected[f.site] = true;
            actions.push(self.owner.actions()[f.site][f.alternative].clone());
        }
        Ok(PreparedNoiseDraw {
            choices,
            actions,
            raw_weight: self.weights.evaluate(&selected)?,
            physical_probability: physical,
            entropy_words,
            occurrence_planes,
            exact_tail_evaluations,
        })
    }
}

impl std::fmt::Display for PreparedNoiseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Noise(e) => e.fmt(f),
            Self::Sparse(e) => e.fmt(f),
            Self::Weight(e) => e.fmt(f),
            Self::Shape {
                field,
                site,
                expected,
                actual,
            } => write!(
                f,
                "prepared noise {field} shape at site {site:?}: expected {expected}, got {actual}"
            ),
            Self::Category(site) => write!(
                f,
                "prepared noise category {site}: nonidentity probabilities sum above one"
            ),
            Self::BatchLimit { required, limit } => write!(
                f,
                "prepared_noise.max_batch={limit} is insufficient; need {required}"
            ),
            Self::RequestLimit {
                option,
                required,
                limit,
            } => write!(
                f,
                "prepared_noise.{option}={limit} is insufficient; conservative requirement {required}"
            ),
        }
    }
}
impl std::error::Error for PreparedNoiseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Noise(e) => Some(e),
            Self::Sparse(e) => Some(e),
            Self::Weight(e) => Some(e),
            _ => None,
        }
    }
}

impl std::fmt::Debug for PreparedNoiseSchedule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedNoiseSchedule")
            .field("statistics", &self.statistics)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}
