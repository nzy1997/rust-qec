//! Exact finite-data comparison of history-sampler variance coefficients.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EfficiencyStatus {
    Available,
    NoErrorMetric,
    InsufficientSamples,
    ZeroAcceptance,
    ZeroResidualVariance,
}
impl EfficiencyStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::NoErrorMetric => "no_error_metric",
            Self::InsufficientSamples => "insufficient_samples",
            Self::ZeroAcceptance => "zero_acceptance",
            Self::ZeroResidualVariance => "zero_residual_variance",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EfficiencySummary {
    pub status: EfficiencyStatus,
    pub completed: u64,
    pub zero_weights: u64,
    pub kappa: Option<ExactField>,
    pub variance_coefficient: Option<ExactField>,
    pub exact_marginal_variance_coefficient: Option<ExactField>,
    pub effective_sample_size: ExactField,
    /// Completed draws divided by weight ESS; absent when every weight is zero.
    pub rho: Option<ExactField>,
    pub mean_weight: Option<ExactField>,
}
/// Fixed number of accumulators; integer sizes obey the arithmetic budget.
#[derive(Clone)]
pub struct HistoryEfficiency {
    statistics: ExactStatistics,
    /// Sum of (L/W) times [A^2, A*E, E^2], for positive W.
    reference: [ExactField; 3],
    zero_weights: u64,
    missing_error: bool,
}
impl HistoryEfficiency {
    pub fn new(arithmetic: FieldArithmetic) -> Result<Self, StatisticsError> {
        Ok(Self {
            statistics: ExactStatistics::new(
                2,
                StatisticsOptions {
                    max_dimensions: 2,
                    arithmetic,
                },
            )?,
            reference: std::array::from_fn(|_| ExactField::zero()),
            zero_weights: 0,
            missing_error: false,
        })
    }
    pub fn count(&self) -> u64 {
        self.statistics.count()
    }
    /// A and E are already weighted contributions; W is the complete
    /// noise/history weight and L the noise-only likelihood. No weight is
    /// applied to A or E again. Missing error metrics remain unavailable.
    pub fn push(
        &mut self,
        accepted: &ExactField,
        error: Option<&ExactField>,
        weight: &ExactField,
        noise_likelihood: &ExactField,
    ) -> Result<(), StatisticsError> {
        let f = &self.statistics.arithmetic;
        let zero = ExactField::zero();
        let e = error.unwrap_or(&zero);
        for x in [accepted, e, weight, noise_likelihood] {
            f.add(x, &zero)?.real_sign()?;
        }
        if weight.real_sign()? == Ordering::Less {
            return Err(StatisticsError::NegativeWeight);
        }
        if noise_likelihood.real_sign()? == Ordering::Less {
            return Err(StatisticsError::NegativeNoiseLikelihood);
        }
        if accepted.real_sign()? == Ordering::Less
            || e.real_sign()? == Ordering::Less
            || f.subtract(accepted, e)?.real_sign()? == Ordering::Less
        {
            return Err(StatisticsError::InvalidProbabilityContribution);
        }
        if !accepted.is_zero() && (weight.is_zero() || noise_likelihood.is_zero()) {
            return Err(StatisticsError::ZeroLikelihoodContribution);
        }
        let zeros = self
            .zero_weights
            .checked_add(u64::from(weight.is_zero()))
            .ok_or(StatisticsError::CountOverflow)?;
        let mut reference = self.reference.clone();
        if error.is_some() && !accepted.is_zero() {
            let factor = f.divide(noise_likelihood, weight)?;
            for (sum, (x, y)) in
                reference
                    .iter_mut()
                    .zip([(accepted, accepted), (accepted, e), (e, e)])
            {
                *sum = f.add(sum, &f.multiply(&factor, &f.multiply(x, y)?)?)?;
            }
        }
        self.statistics
            .push(&[accepted.clone(), e.clone()], weight)?;
        self.reference = reference;
        self.zero_weights = zeros;
        self.missing_error |= error.is_none();
        Ok(())
    }
    /// Merge disjoint partitions under the receiver's arithmetic budget.
    /// All failed updates and merges leave the receiver unchanged.
    pub fn merge(&mut self, other: &Self) -> Result<(), StatisticsError> {
        let f = &self.statistics.arithmetic;
        let zeros = self
            .zero_weights
            .checked_add(other.zero_weights)
            .ok_or(StatisticsError::CountOverflow)?;
        let mut reference = self.reference.clone();
        for (x, y) in reference.iter_mut().zip(&other.reference) {
            *x = f.add(x, y)?;
        }
        self.statistics.merge(&other.statistics)?;
        self.reference = reference;
        self.zero_weights = zeros;
        self.missing_error |= other.missing_error;
        Ok(())
    }
    fn residual(
        &self,
        products: &[ExactField],
        center: &ExactField,
    ) -> Result<ExactField, FieldError> {
        let f = &self.statistics.arithmetic;
        let twice_center = f.add(center, center)?;
        f.add(
            &f.subtract(&products[2], &f.multiply(&twice_center, &products[1])?)?,
            &f.multiply(&f.multiply(center, center)?, &products[0])?,
        )
    }
    pub fn summarize(&self) -> Result<EfficiencySummary, StatisticsError> {
        let s = &self.statistics;
        let f = &s.arithmetic;
        let n = s.count_value()?;
        let ess = if s.weight_squared.is_zero() {
            ExactField::zero()
        } else {
            f.divide(&f.multiply(&s.weight, &s.weight)?, &s.weight_squared)?
        };
        let rho = if ess.is_zero() {
            None
        } else {
            Some(f.divide(&n, &ess)?)
        };
        let mean_weight = if s.count == 0 {
            None
        } else {
            Some(f.divide(&s.weight, &n)?)
        };
        let mut out = EfficiencySummary {
            status: EfficiencyStatus::Available,
            completed: s.count,
            zero_weights: self.zero_weights,
            kappa: None,
            variance_coefficient: None,
            exact_marginal_variance_coefficient: None,
            effective_sample_size: ess,
            rho,
            mean_weight,
        };
        if self.missing_error {
            out.status = EfficiencyStatus::NoErrorMetric;
        } else if s.count < 2 {
            out.status = EfficiencyStatus::InsufficientSamples;
        } else if s.sums[0].is_zero() {
            out.status = EfficiencyStatus::ZeroAcceptance;
        } else {
            let ratio = f.divide(&s.sums[1], &s.sums[0])?;
            let reference = self.residual(&self.reference, &ratio)?;
            if reference.is_zero() {
                out.status = EfficiencyStatus::ZeroResidualVariance;
            } else {
                let actual = self.residual(&s.products, &ratio)?;
                let divisor = f.multiply(&s.sums[0], &s.sums[0])?;
                out.kappa = Some(f.divide(&actual, &reference)?);
                out.variance_coefficient = Some(f.multiply(&n, &f.divide(&actual, &divisor)?)?);
                out.exact_marginal_variance_coefficient =
                    Some(f.multiply(&n, &f.divide(&reference, &divisor)?)?);
            }
        }
        Ok(out)
    }
}
