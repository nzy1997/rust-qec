//! Owned conditional channel laws, independent of occurrence probabilities.

use super::{NoiseError, NoiseModel, NoiseOptions};
use crate::probability::{Probability, ProbabilityError};
use num_rational::BigRational;
use num_traits::One;
use std::fmt;

/// Invalid conditional input or a rejected canonical categorical model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConditionalLawError {
    NoChannels,
    WeightCount {
        declared: usize,
        provided: usize,
    },
    Occurrence(ProbabilityError),
    Weight {
        channel: usize,
        source: ProbabilityError,
    },
    WeightSum {
        sum: BigRational,
    },
    EffectiveRate(ProbabilityError),
    Model(NoiseError),
}

impl fmt::Display for ConditionalLawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoChannels => {
                f.write_str("conditional noise law must declare at least one channel")
            }
            Self::WeightCount { declared, provided } => write!(
                f,
                "conditional noise law declares {declared} channels but supplies {provided} weights; provide exactly one weight per channel"
            ),
            Self::Occurrence(error) => write!(
                f,
                "invalid conditional noise occurrence probability: {error}"
            ),
            Self::Weight { channel, source } => write!(
                f,
                "invalid conditional weight at channel {channel}: {source}"
            ),
            Self::WeightSum { sum } => write!(
                f,
                "conditional noise weights must sum exactly to one; got {sum}"
            ),
            Self::EffectiveRate(error) => {
                write!(f, "invalid shared effective physical rate: {error}")
            }
            Self::Model(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ConditionalLawError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Occurrence(error) | Self::EffectiveRate(error) => Some(error),
            Self::Weight { source, .. } => Some(source),
            Self::Model(error) => Some(error),
            Self::NoChannels | Self::WeightCount { .. } | Self::WeightSum { .. } => None,
        }
    }
}

/// An owned occurrence probability and explicit unit-sum conditional weights.
///
/// Weights are retained even when the occurrence probability is zero or one.
/// Channel indices follow the supplied vector; their actions belong to the
/// caller's catalog. Retain this law to construct models at other physical
/// rates: an absolute zero-mass category alone cannot recover these weights.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConditionalSiteLaw {
    occurrence: Probability,
    weights: Vec<Probability>,
}

impl ConditionalSiteLaw {
    /// Validate the declared channel count and every exact weight, including
    /// at deterministic occurrence endpoints. Weights are never normalized.
    pub fn new(
        declared_channels: usize,
        occurrence: BigRational,
        weights: Vec<BigRational>,
    ) -> Result<Self, ConditionalLawError> {
        if declared_channels == 0 {
            return Err(ConditionalLawError::NoChannels);
        }
        if weights.len() != declared_channels {
            return Err(ConditionalLawError::WeightCount {
                declared: declared_channels,
                provided: weights.len(),
            });
        }
        let occurrence =
            Probability::from_rational(occurrence).map_err(ConditionalLawError::Occurrence)?;
        let weights = weights
            .into_iter()
            .enumerate()
            .map(|(channel, value)| {
                Probability::from_rational(value)
                    .map_err(|source| ConditionalLawError::Weight { channel, source })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sum = weights.iter().map(Probability::value).sum::<BigRational>();
        if sum != BigRational::one() {
            return Err(ConditionalLawError::WeightSum { sum });
        }
        Ok(Self {
            occurrence,
            weights,
        })
    }

    pub fn num_channels(&self) -> usize {
        self.weights.len()
    }

    pub fn occurrence_probability(&self) -> &Probability {
        &self.occurrence
    }

    pub fn conditional_weights(&self) -> &[Probability] {
        &self.weights
    }
}

impl NoiseModel {
    /// Construct owned absolute categories, optionally replacing every law's
    /// occurrence probability with one shared exact physical rate.
    ///
    /// Category entries are identity followed by the original channel indices,
    /// including zero weights. Denominator limits apply to the resulting
    /// canonical categories through [`NoiseModel::new`]. The laws are unchanged
    /// and can be reused; configured importance sampling acts on the new model.
    /// An invalid supplied rate is rejected even for an empty law list.
    ///
    /// ```
    /// use num_rational::BigRational;
    /// use rmagna::noise::{ConditionalSiteLaw, NoiseModel, NoiseOptions};
    /// let zero = BigRational::from_integer(0.into());
    /// let one = BigRational::from_integer(1.into());
    /// let law = ConditionalSiteLaw::new(2, zero.clone(), vec![zero, one])?;
    /// let rate = BigRational::new(1.into(), 3.into());
    /// let model = NoiseModel::from_conditional_laws(
    ///     &[law], Some(rate.clone()), &NoiseOptions::default(),
    /// )?;
    /// assert_eq!(model.probabilities(0).unwrap()[2].value(), &rate);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_conditional_laws(
        laws: &[ConditionalSiteLaw],
        effective_physical_rate: Option<BigRational>,
        options: &NoiseOptions,
    ) -> Result<Self, ConditionalLawError> {
        let shared = effective_physical_rate
            .map(Probability::from_rational)
            .transpose()
            .map_err(ConditionalLawError::EffectiveRate)?;
        let categories = laws
            .iter()
            .map(|law| {
                let occurrence = shared.as_ref().unwrap_or(&law.occurrence);
                std::iter::once(occurrence.complement())
                    .chain(law.weights.iter().map(|weight| {
                        Probability::from_rational(occurrence.value() * weight.value())
                            .expect("a product of validated probabilities lies in [0,1]")
                    }))
                    .collect()
            })
            .collect();
        Self::new(categories, options).map_err(ConditionalLawError::Model)
    }
}
