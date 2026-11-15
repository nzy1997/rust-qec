//! Owned named actions over the existing exact categorical sampler.
use super::*;
use crate::circuit::Basis;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SparseNoiseAction {
    Pauli {
        operation: String,
        factors: Vec<(usize, Basis)>,
    },
    ReportFlip {
        record: String,
    },
}
#[derive(Clone, Copy, Debug)]
pub struct SparseNoiseOptions {
    pub noise: NoiseOptions,
    pub max_sites: usize,
    pub max_actions: usize,
    pub max_factors: usize,
    /// Retained operation/record UTF-8 payload bytes, excluding allocator headers.
    pub max_name_bytes: usize,
}
impl Default for SparseNoiseOptions {
    fn default() -> Self {
        Self {
            noise: NoiseOptions::default(),
            max_sites: 65536,
            max_actions: 1 << 20,
            max_factors: 4 << 20,
            max_name_bytes: 64 << 20,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SparseNoiseError {
    Noise(NoiseError),
    Shape {
        site: Option<usize>,
    },
    Limit {
        option: &'static str,
        required: u128,
        limit: usize,
    },
    DuplicateSite(usize),
    GuardedSite(usize),
    ImpossibleProposal,
}
impl fmt::Display for SparseNoiseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Noise(e) => e.fmt(f),
            Self::Shape { site } => {
                write!(f, "noise action shape does not match category {site:?}")
            }
            Self::Limit {
                option,
                required,
                limit,
            } => write!(f, "{option}={limit} is insufficient; need {required}"),
            Self::DuplicateSite(site) => write!(f, "duplicate original noise site {site}"),
            Self::GuardedSite(site) => write!(
                f,
                "original noise site {site} requires causal guard resolution; use the task runtime"
            ),
            Self::ImpossibleProposal => {
                write!(f, "selected choices have zero proposal probability")
            }
        }
    }
}
impl std::error::Error for SparseNoiseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Noise(e) => Some(e),
            _ => None,
        }
    }
}
impl From<NoiseError> for SparseNoiseError {
    fn from(e: NoiseError) -> Self {
        Self::Noise(e)
    }
}
fn cap(option: &'static str, required: u128, limit: usize) -> Result<(), SparseNoiseError> {
    if required > limit as u128 {
        Err(SparseNoiseError::Limit {
            option,
            required,
            limit,
        })
    } else {
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SparseNoiseDraw {
    pub gates: BTreeMap<String, Vec<(usize, Basis)>>,
    /// Ordered XOR list, with the same repeated-name meaning as other inputs.
    pub report_flips: Vec<String>,
    pub noise: NoiseDraw,
}
/// The categorical model and action table are owned independently of the source
/// circuit and caller buffers. Action lookup never changes the sampled stream.
pub struct SparseNoiseSampler {
    sampler: NoiseSampler,
    actions: Arc<Vec<Vec<SparseNoiseAction>>>,
}
impl SparseNoiseSampler {
    /// One action per nonidentity alternative. Shape/payload admission precedes
    /// cloning the borrowed nested action buffers and constructing a proposal.
    pub fn new(
        physical: NoiseModel,
        actions: &[Vec<SparseNoiseAction>],
        sampling: &NoiseSampling,
        options: SparseNoiseOptions,
    ) -> Result<Self, SparseNoiseError> {
        cap(
            "sparse_noise.max_sites",
            physical.num_sites() as u128,
            options.max_sites,
        )?;
        if actions.len() != physical.num_sites() {
            return Err(SparseNoiseError::Shape { site: None });
        }
        let mut count = 0u128;
        let mut factors = 0u128;
        let mut bytes = 0u128;
        for (site, channels) in actions.iter().enumerate() {
            if channels.len().checked_add(1) != Some(physical.probabilities(site).unwrap().len()) {
                return Err(SparseNoiseError::Shape { site: Some(site) });
            }
            count += channels.len() as u128;
            for action in channels {
                match action {
                    SparseNoiseAction::Pauli {
                        operation,
                        factors: parts,
                    } => {
                        bytes += operation.len() as u128;
                        factors += parts.len() as u128;
                    }
                    SparseNoiseAction::ReportFlip { record } => bytes += record.len() as u128,
                }
            }
        }
        cap("sparse_noise.max_actions", count, options.max_actions)?;
        cap("sparse_noise.max_factors", factors, options.max_factors)?;
        cap("sparse_noise.max_name_bytes", bytes, options.max_name_bytes)?;
        let sampler =
            NoiseSampler::configured(physical, sampling, &BTreeSet::new(), options.noise)?;
        Ok(Self {
            sampler,
            actions: Arc::new(actions.to_vec()),
        })
    }
    /// Construct named file actions from captured original categories. Missing
    /// internal sites remain identity-only. Guards need the causal task runtime.
    pub fn from_categories(
        categories: &[OriginalNoiseCategory],
        sampling: &NoiseSampling,
        options: SparseNoiseOptions,
    ) -> Result<Self, SparseNoiseError> {
        let sites = categories
            .iter()
            .map(|c| c.internal_site as u128 + 1)
            .max()
            .unwrap_or(0);
        cap("sparse_noise.max_sites", sites, options.max_sites)?;
        let mut seen = BTreeSet::new();
        let mut count = 0u128;
        let mut factors = 0u128;
        let mut bytes = 0u128;
        let digits = |v: usize| if v == 0 { 1 } else { v.ilog10() as u128 + 1 };
        for category in categories {
            if !seen.insert(category.internal_site) {
                return Err(SparseNoiseError::DuplicateSite(category.internal_site));
            }
            if category.guard.is_some() {
                return Err(SparseNoiseError::GuardedSite(category.internal_site));
            }
            count += category.channels.len() as u128;
            for channel in &category.channels {
                match &channel.action {
                    OriginalNoiseAction::Pauli { factors: parts } => {
                        factors += parts.len() as u128;
                        bytes += 2 + digits(channel.source.operation);
                    }
                    OriginalNoiseAction::ReportFlip { record } => bytes += 1 + digits(*record),
                }
            }
        }
        cap("sparse_noise.max_actions", count, options.max_actions)?;
        cap("sparse_noise.max_factors", factors, options.max_factors)?;
        cap("sparse_noise.max_name_bytes", bytes, options.max_name_bytes)?;
        let one = Probability::from_rational(BigRational::one()).expect("unit probability");
        let mut probabilities = vec![vec![one]; sites as usize];
        let mut actions = vec![Vec::new(); sites as usize];
        for category in categories {
            probabilities[category.internal_site] =
                std::iter::once(category.identity_probability.clone())
                    .chain(category.channels.iter().map(|c| c.probability.clone()))
                    .collect();
            actions[category.internal_site] = category
                .channels
                .iter()
                .map(|c| match &c.action {
                    OriginalNoiseAction::Pauli { factors } => SparseNoiseAction::Pauli {
                        operation: format!("op{}", c.source.operation),
                        factors: factors.clone(),
                    },
                    OriginalNoiseAction::ReportFlip { record } => SparseNoiseAction::ReportFlip {
                        record: format!("m{record}"),
                    },
                })
                .collect();
        }
        Self::new(
            NoiseModel::new(probabilities, &options.noise)?,
            &actions,
            sampling,
            options,
        )
    }
    pub fn actions(&self) -> &[Vec<SparseNoiseAction>] {
        &self.actions
    }
    pub fn draw(
        &self,
        random: &mut impl RandomSource,
    ) -> Result<SparseNoiseDraw, SparseNoiseError> {
        Ok(self.assemble(self.sampler.draw(random)?))
    }
    /// Exact deterministic replay of categorical selections, consuming no RNG.
    /// Category indices are Rust indices; adapters must map foreign action order.
    pub fn prescribed(
        &self,
        faults: &[crate::instrument::FaultSelection],
    ) -> Result<SparseNoiseDraw, SparseNoiseError> {
        let physical_probability = self.sampler.physical.probability(faults)?;
        let proposal_probability = self.sampler.proposal.probability(faults)?;
        if proposal_probability.is_zero() {
            return Err(SparseNoiseError::ImpossibleProposal);
        }
        let likelihood_ratio = &physical_probability / &proposal_probability;
        Ok(self.assemble(NoiseDraw {
            faults: faults.to_vec(),
            physical_probability,
            proposal_probability,
            likelihood_ratio,
            entropy_words: 0,
        }))
    }
    fn assemble(&self, noise: NoiseDraw) -> SparseNoiseDraw {
        let mut gates = BTreeMap::<String, Vec<(usize, Basis)>>::new();
        let mut report_flips = Vec::new();
        for fault in &noise.faults {
            match &self.actions[fault.site][fault.alternative] {
                SparseNoiseAction::Pauli { operation, factors } => gates
                    .entry(operation.clone())
                    .or_default()
                    .extend_from_slice(factors),
                SparseNoiseAction::ReportFlip { record } => report_flips.push(record.clone()),
            }
        }
        SparseNoiseDraw {
            gates,
            report_flips,
            noise,
        }
    }
}
