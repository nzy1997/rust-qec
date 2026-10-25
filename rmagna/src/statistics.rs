//! Exact streaming statistics for already weighted real contribution vectors.
//!
//! Uncertainty estimates assume independent draws from the same intended sampling
//! law with finite second moments. Dependence and replication are not checked,
//! and no autocorrelation correction is applied.
use crate::field::{ExactField, FieldArithmetic, FieldError};
use num_rational::BigRational;
use std::{cmp::Ordering, fmt};
mod checkpoint;
mod efficiency;
pub use checkpoint::{CheckpointError, CheckpointOptions};
pub use efficiency::{EfficiencyStatus, EfficiencySummary, HistoryEfficiency};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatisticsError {
    DimensionLimit { dimensions: usize, limit: usize },
    DimensionMismatch { expected: usize, actual: usize },
    InvalidIndex { index: usize, dimensions: usize },
    SizeOverflow,
    CountOverflow,
    NegativeWeight,
    NegativeNoiseLikelihood,
    InvalidProbabilityContribution,
    ZeroLikelihoodContribution,
    Field(FieldError),
}
impl fmt::Display for StatisticsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionLimit { dimensions, limit } => write!(
                f,
                "statistics need {dimensions} quantities; limit is {limit}"
            ),
            Self::DimensionMismatch { expected, actual } => write!(
                f,
                "statistics expect {expected} quantities; received {actual}"
            ),
            Self::InvalidIndex { index, dimensions } => write!(
                f,
                "quantity {index} is outside {dimensions} statistics quantities"
            ),
            Self::SizeOverflow => write!(f, "statistics accumulator dimensions overflow"),
            Self::CountOverflow => write!(f, "statistics draw count exceeds u64"),
            Self::NegativeWeight => write!(f, "proposal diagnostic weight must be nonnegative"),
            Self::NegativeNoiseLikelihood => write!(f, "noise-only likelihood must be nonnegative"),
            Self::InvalidProbabilityContribution => write!(
                f,
                "efficiency contributions require 0 <= error <= acceptance"
            ),
            Self::ZeroLikelihoodContribution => write!(
                f,
                "positive acceptance requires positive complete and noise-only likelihoods"
            ),
            Self::Field(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for StatisticsError {}
impl From<FieldError> for StatisticsError {
    fn from(error: FieldError) -> Self {
        Self::Field(error)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StatisticsOptions {
    pub max_dimensions: usize,
    pub arithmetic: FieldArithmetic,
}
impl Default for StatisticsOptions {
    fn default() -> Self {
        Self {
            max_dimensions: 64,
            arithmetic: FieldArithmetic::default(),
        }
    }
}

/// Number of accumulators is independent of draws. Exact integer sizes remain
/// subject to the configured field-arithmetic budget.
#[derive(Clone)]
pub struct ExactStatistics {
    dimensions: usize,
    count: u64,
    sums: Vec<ExactField>,
    products: Vec<ExactField>,
    weight: ExactField,
    weight_squared: ExactField,
    arithmetic: FieldArithmetic,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatisticsSummary {
    pub count: u64,
    pub means: Vec<ExactField>,
    /// Unbiased sample covariance divided by the draw count. Absent for n=1.
    /// Its uncertainty interpretation requires the module's independent-draw
    /// assumptions; correlated or replicated rows receive no correction.
    pub covariance_of_mean: Option<Vec<Vec<ExactField>>>,
    pub mean_weight: ExactField,
    /// Weight-only diagnostic; zero when every weight is zero.
    pub effective_sample_size: ExactField,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairedRatio {
    pub ratio: ExactField,
    /// Paired delta-method variance, not an exact finite-sample variance or a
    /// confidence interval. Absent for fewer than two draws.
    /// Requires the module's sampling assumptions; no autocorrelation correction.
    pub variance: Option<ExactField>,
}
fn product_count(dimensions: usize) -> Result<usize, StatisticsError> {
    dimensions
        .checked_add(1)
        .and_then(|n| dimensions.checked_mul(n))
        .map(|n| n / 2)
        .ok_or(StatisticsError::SizeOverflow)
}
fn position(i: usize, j: usize) -> usize {
    let (i, j) = if i <= j { (i, j) } else { (j, i) };
    j * (j + 1) / 2 + i
}
impl ExactStatistics {
    pub fn new(dimensions: usize, options: StatisticsOptions) -> Result<Self, StatisticsError> {
        if dimensions > options.max_dimensions {
            return Err(StatisticsError::DimensionLimit {
                dimensions,
                limit: options.max_dimensions,
            });
        }
        let size = product_count(dimensions)?;
        Ok(Self {
            dimensions,
            count: 0,
            sums: vec![ExactField::zero(); dimensions],
            products: vec![ExactField::zero(); size],
            weight: ExactField::zero(),
            weight_squared: ExactField::zero(),
            arithmetic: options.arithmetic,
        })
    }
    pub fn count(&self) -> u64 {
        self.count
    }
    pub fn num_quantities(&self) -> usize {
        self.dimensions
    }
    fn shape(&self, actual: usize) -> Result<(), StatisticsError> {
        if actual != self.dimensions {
            return Err(StatisticsError::DimensionMismatch {
                expected: self.dimensions,
                actual,
            });
        }
        Ok(())
    }
    /// `values` are already weighted joint contributions. `weight` is the
    /// combined noise/history likelihood for ESS only; it is not applied again.
    /// A zero-weight draw still increments the count. Failed updates are atomic.
    pub fn push(
        &mut self,
        values: &[ExactField],
        weight: &ExactField,
    ) -> Result<(), StatisticsError> {
        self.shape(values.len())?;
        let count = self
            .count
            .checked_add(1)
            .ok_or(StatisticsError::CountOverflow)?;
        let a = &self.arithmetic;
        for value in values.iter().chain(std::iter::once(weight)) {
            a.add(value, &ExactField::zero())?.real_sign()?;
        }
        if weight.real_sign()? == Ordering::Less {
            return Err(StatisticsError::NegativeWeight);
        }
        let sums = self
            .sums
            .iter()
            .zip(values)
            .map(|(sum, value)| a.add(sum, value))
            .collect::<Result<Vec<_>, _>>()?;
        let mut products = Vec::with_capacity(self.products.len());
        for (j, right) in values.iter().enumerate() {
            for (i, left) in values[..=j].iter().enumerate() {
                products.push(a.add(&self.products[position(i, j)], &a.multiply(left, right)?)?);
            }
        }
        let next_weight = a.add(&self.weight, weight)?;
        let weight_squared = a.add(&self.weight_squared, &a.multiply(weight, weight)?)?;
        self.count = count;
        self.sums = sums;
        self.products = products;
        self.weight = next_weight;
        self.weight_squared = weight_squared;
        Ok(())
    }
    /// Merge nonoverlapping partitions of the same intended sampling law. Neither
    /// overlap nor draw independence is checked. The receiver's arithmetic budget
    /// applies; incompatible shapes and failed arithmetic leave it unchanged.
    pub fn merge(&mut self, other: &Self) -> Result<(), StatisticsError> {
        self.shape(other.dimensions)?;
        let count = self
            .count
            .checked_add(other.count)
            .ok_or(StatisticsError::CountOverflow)?;
        let a = &self.arithmetic;
        let sums = self
            .sums
            .iter()
            .zip(&other.sums)
            .map(|(x, y)| a.add(x, y))
            .collect::<Result<Vec<_>, _>>()?;
        let products = self
            .products
            .iter()
            .zip(&other.products)
            .map(|(x, y)| a.add(x, y))
            .collect::<Result<Vec<_>, _>>()?;
        let weight = a.add(&self.weight, &other.weight)?;
        let weight_squared = a.add(&self.weight_squared, &other.weight_squared)?;
        self.count = count;
        self.sums = sums;
        self.products = products;
        self.weight = weight;
        self.weight_squared = weight_squared;
        Ok(())
    }
    fn count_value(&self) -> Result<ExactField, FieldError> {
        self.arithmetic
            .from_rational(BigRational::from_integer(self.count.into()))
    }
    fn covariance(&self, i: usize, j: usize, n: &ExactField) -> Result<ExactField, FieldError> {
        let a = &self.arithmetic;
        let centered = a.subtract(
            &self.products[position(i, j)],
            &a.divide(&a.multiply(&self.sums[i], &self.sums[j])?, n)?,
        )?;
        let divisor = a.multiply(n, &a.subtract(n, &ExactField::one())?)?;
        a.divide(&centered, &divisor)
    }
    /// No draws produce no summary. Standard errors require a separate rounded
    /// square root of the exact nonnegative diagonal covariance entries.
    pub fn summarize(&self) -> Result<Option<StatisticsSummary>, StatisticsError> {
        if self.count == 0 {
            return Ok(None);
        }
        let a = &self.arithmetic;
        let n = self.count_value()?;
        let means = self
            .sums
            .iter()
            .map(|sum| a.divide(sum, &n))
            .collect::<Result<Vec<_>, _>>()?;
        let covariance_of_mean = if self.count > 1 {
            Some(
                (0..self.dimensions)
                    .map(|i| {
                        (0..self.dimensions)
                            .map(|j| self.covariance(i, j, &n))
                            .collect()
                    })
                    .collect::<Result<Vec<Vec<_>>, FieldError>>()?,
            )
        } else {
            None
        };
        let effective_sample_size = if self.weight_squared.is_zero() {
            ExactField::zero()
        } else {
            a.divide(
                &a.multiply(&self.weight, &self.weight)?,
                &self.weight_squared,
            )?
        };
        Ok(Some(StatisticsSummary {
            count: self.count,
            means,
            covariance_of_mean,
            mean_weight: a.divide(&self.weight, &n)?,
            effective_sample_size,
        }))
    }
    /// Ratio of aggregate means, with paired covariance. A zero aggregate
    /// denominator (or no draws) produces None, never an invented zero ratio.
    pub fn paired_ratio(
        &self,
        numerator: usize,
        denominator: usize,
    ) -> Result<Option<PairedRatio>, StatisticsError> {
        for index in [numerator, denominator] {
            if index >= self.dimensions {
                return Err(StatisticsError::InvalidIndex {
                    index,
                    dimensions: self.dimensions,
                });
            }
        }
        if self.count == 0 || self.sums[denominator].is_zero() {
            return Ok(None);
        }
        let a = &self.arithmetic;
        let ratio = a.divide(&self.sums[numerator], &self.sums[denominator])?;
        let variance = if self.count > 1 {
            let n = self.count_value()?;
            let mean = a.divide(&self.sums[denominator], &n)?;
            let xx = self.covariance(denominator, denominator, &n)?;
            let xy = self.covariance(numerator, denominator, &n)?;
            let yy = self.covariance(numerator, numerator, &n)?;
            let cross = a.multiply(&ratio, &xy)?;
            let residual = a.subtract(
                &a.add(&yy, &a.multiply(&a.multiply(&ratio, &ratio)?, &xx)?)?,
                &a.add(&cross, &cross)?,
            )?;
            Some(a.divide(&residual, &a.multiply(&mean, &mean)?)?)
        } else {
            None
        };
        Ok(Some(PairedRatio { ratio, variance }))
    }
}
