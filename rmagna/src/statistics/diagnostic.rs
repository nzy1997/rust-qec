//! Explicit floating diagnostic domain over unchanged exact history moments.
//!
//! This models a declared binary64 scientific-mantissa interface. It does not
//! reproduce arbitrary host long-double accumulation or residual cancellation.
use super::*;
use crate::numeric::{NumericError, NumericOptions, ScientificReal, approximate_real};

/// Per-draw E/A range after separately rounding E and A to scientific binary64
/// mantissas, dividing those mantissas in binary64, and normalizing with frexp.
/// The lower bound is inclusive; valid contributions also have exponent <= 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScientificDiagnosticDomain {
    min_ratio_frexp_exponent: i64,
}
impl ScientificDiagnosticDomain {
    /// Declare the range explicitly. -510 matches the native binary64
    /// long-double floor (-1021 / 2, truncated toward zero); other hosts can
    /// declare a different floor. No host ABI is detected or implied.
    pub fn new(min_ratio_frexp_exponent: i64) -> Result<Self, DiagnosticDomainError> {
        if min_ratio_frexp_exponent > 1 {
            return Err(DiagnosticDomainError::InvalidDomain);
        }
        Ok(Self {
            min_ratio_frexp_exponent,
        })
    }
    pub fn min_ratio_frexp_exponent(self) -> i64 {
        self.min_ratio_frexp_exponent
    }
    pub fn ratio_semantics(self) -> &'static str {
        "binary64_scientific_mantissa_ratio"
    }
}

/// Fixed-size evidence from completed draws, including draws with absent error
/// metrics. Zero E or zero A is not a checked nonzero ratio.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScientificDomainEvidence {
    pub draws: u64,
    pub checked_ratios: u64,
    pub out_of_domain: u64,
    pub missing_errors: u64,
    pub minimum_ratio_frexp_exponent: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiagnosticDomainError {
    InvalidDomain,
    IncompatibleDomain,
    MissingDrawProvenance,
    ExponentOverflow,
    Statistics(StatisticsError),
    Numeric(NumericError),
}
impl fmt::Display for DiagnosticDomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDomain => f.write_str("ratio exponent floor must be at most 1"),
            Self::IncompatibleDomain => f.write_str("scientific diagnostic policies differ"),
            Self::MissingDrawProvenance => f.write_str(
                "nonempty exact totals have no per-draw diagnostic provenance; replay the draws",
            ),
            Self::ExponentOverflow => f.write_str("scientific ratio exponent exceeds i64"),
            Self::Statistics(e) => e.fmt(f),
            Self::Numeric(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for DiagnosticDomainError {}
impl From<StatisticsError> for DiagnosticDomainError {
    fn from(e: StatisticsError) -> Self {
        Self::Statistics(e)
    }
}
impl From<NumericError> for DiagnosticDomainError {
    fn from(e: NumericError) -> Self {
        Self::Numeric(e)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScientificEfficiencyStatus {
    Available,
    NoErrorMetric,
    DiagnosticRangeExceeded,
    InsufficientSamples,
    ZeroAcceptance,
    ZeroResidualVariance,
}
impl ScientificEfficiencyStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::NoErrorMetric => "no_error_metric",
            Self::DiagnosticRangeExceeded => "diagnostic_range_exceeded",
            Self::InsufficientSamples => "insufficient_samples",
            Self::ZeroAcceptance => "zero_acceptance",
            Self::ZeroResidualVariance => "zero_residual_variance",
        }
    }
}
impl From<EfficiencyStatus> for ScientificEfficiencyStatus {
    fn from(s: EfficiencyStatus) -> Self {
        match s {
            EfficiencyStatus::Available => Self::Available,
            EfficiencyStatus::NoErrorMetric => Self::NoErrorMetric,
            EfficiencyStatus::InsufficientSamples => Self::InsufficientSamples,
            EfficiencyStatus::ZeroAcceptance => Self::ZeroAcceptance,
            EfficiencyStatus::ZeroResidualVariance => Self::ZeroResidualVariance,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScientificEfficiencySummary {
    pub status: ScientificEfficiencyStatus,
    /// Exact residuals and actual weight diagnostics, even when the declared
    /// scientific diagnostic is unavailable. Contributions are never clamped.
    pub exact: EfficiencySummary,
    /// Export of the exact kappa, present on final ordinary-range failure but
    /// absent on per-draw range failure or missing error metrics.
    pub kappa_scientific: Option<ScientificReal>,
    pub domain: ScientificDiagnosticDomain,
    pub evidence: ScientificDomainEvidence,
}

/// Bounded wrapper with atomic push/merge, using the receiver's field and
/// numeric work budgets. Only empty exact accumulators can acquire a policy
/// without replay; exact moments cannot recover per-draw range information.
#[derive(Clone)]
pub struct ScientificEfficiency {
    pub(super) exact: HistoryEfficiency,
    pub(super) domain: ScientificDiagnosticDomain,
    pub(super) numeric: NumericOptions,
    pub(super) evidence: ScientificDomainEvidence,
}
impl ScientificEfficiency {
    pub fn new(
        arithmetic: FieldArithmetic,
        domain: ScientificDiagnosticDomain,
        numeric: NumericOptions,
    ) -> Result<Self, DiagnosticDomainError> {
        Self::from_empty_exact(HistoryEfficiency::new(arithmetic)?, domain, numeric)
    }
    pub fn from_empty_exact(
        exact: HistoryEfficiency,
        domain: ScientificDiagnosticDomain,
        numeric: NumericOptions,
    ) -> Result<Self, DiagnosticDomainError> {
        if exact.count() != 0 {
            return Err(DiagnosticDomainError::MissingDrawProvenance);
        }
        Ok(Self {
            exact,
            domain,
            numeric,
            evidence: ScientificDomainEvidence::default(),
        })
    }
    pub fn exact(&self) -> &HistoryEfficiency {
        &self.exact
    }
    pub fn domain(&self) -> ScientificDiagnosticDomain {
        self.domain
    }
    pub fn evidence(&self) -> ScientificDomainEvidence {
        self.evidence
    }

    /// A and E are already weighted. Scientific exports are separate, so this
    /// deliberately does not round the exact quotient E/A.
    pub fn push(
        &mut self,
        accepted: &ExactField,
        error: Option<&ExactField>,
        weight: &ExactField,
        noise_likelihood: &ExactField,
    ) -> Result<(), DiagnosticDomainError> {
        let mut exact = self.exact.clone();
        exact.push(accepted, error, weight, noise_likelihood)?;
        let mut evidence = self.evidence;
        evidence.draws = exact.count();
        if let Some(error) = error {
            if !accepted.is_zero() && !error.is_zero() {
                let a = approximate_real(accepted, self.numeric)?.scientific;
                let e = approximate_real(error, self.numeric)?.scientific;
                let quotient = e.mantissa / a.mantissa;
                let exponent =
                    i128::from(e.exponent) - i128::from(a.exponent) + i128::from(quotient >= 1.0);
                let exponent =
                    i64::try_from(exponent).map_err(|_| DiagnosticDomainError::ExponentOverflow)?;
                evidence.checked_ratios = add_count(evidence.checked_ratios, 1)?;
                evidence.out_of_domain = add_count(
                    evidence.out_of_domain,
                    u64::from(exponent < self.domain.min_ratio_frexp_exponent),
                )?;
                evidence.minimum_ratio_frexp_exponent = Some(
                    evidence
                        .minimum_ratio_frexp_exponent
                        .map_or(exponent, |old| old.min(exponent)),
                );
            }
        } else {
            evidence.missing_errors = add_count(evidence.missing_errors, 1)?;
        }
        self.exact = exact;
        self.evidence = evidence;
        Ok(())
    }

    /// Merge disjoint streams with identical declared policies. Numeric work
    /// limits are caller resources, not semantic policies or persisted grants.
    pub fn merge(&mut self, other: &Self) -> Result<(), DiagnosticDomainError> {
        if self.domain != other.domain {
            return Err(DiagnosticDomainError::IncompatibleDomain);
        }
        let mut exact = self.exact.clone();
        exact.merge(&other.exact)?;
        let a = self.evidence;
        let b = other.evidence;
        let evidence = ScientificDomainEvidence {
            draws: add_count(a.draws, b.draws)?,
            checked_ratios: add_count(a.checked_ratios, b.checked_ratios)?,
            out_of_domain: add_count(a.out_of_domain, b.out_of_domain)?,
            missing_errors: add_count(a.missing_errors, b.missing_errors)?,
            minimum_ratio_frexp_exponent: a
                .minimum_ratio_frexp_exponent
                .into_iter()
                .chain(b.minimum_ratio_frexp_exponent)
                .min(),
        };
        self.exact = exact;
        self.evidence = evidence;
        Ok(())
    }

    pub fn summarize(&self) -> Result<ScientificEfficiencySummary, DiagnosticDomainError> {
        let exact = self.exact.summarize()?;
        let mut status = if self.evidence.missing_errors != 0 {
            ScientificEfficiencyStatus::NoErrorMetric
        } else if self.evidence.out_of_domain != 0 {
            ScientificEfficiencyStatus::DiagnosticRangeExceeded
        } else {
            exact.status.into()
        };
        let mut kappa_scientific = None;
        if status == ScientificEfficiencyStatus::Available {
            let value = approximate_real(
                exact.kappa.as_ref().expect("available exact kappa"),
                self.numeric,
            )?
            .scientific;
            // Check the already-rounded scientific pair, rather than the
            // independently rounded exact-to-binary64 export. At half the
            // least subnormal, ties-to-even yields zero.
            if !ordinary_pair_in_range(value) {
                status = ScientificEfficiencyStatus::DiagnosticRangeExceeded;
            }
            kappa_scientific = Some(value);
        }
        Ok(ScientificEfficiencySummary {
            status,
            exact,
            kappa_scientific,
            domain: self.domain,
            evidence: self.evidence,
        })
    }
}
fn add_count(a: u64, b: u64) -> Result<u64, DiagnosticDomainError> {
    a.checked_add(b)
        .ok_or(StatisticsError::CountOverflow.into())
}
fn ordinary_pair_in_range(value: ScientificReal) -> bool {
    value.mantissa == 0.0
        || (value.exponent <= 1023
            && (value.exponent > -1075 || (value.exponent == -1075 && value.mantissa.abs() > 1.0)))
}
