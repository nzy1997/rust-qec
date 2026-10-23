//! Numerical result payloads; execution identities and file schemas are separate.
use super::{RunPhase, RunStatus, TaskRunResult, TaskRunStatistics};
use crate::field::ExactField;
use crate::numeric::{Binary64, NumericError, NumericOptions, approximate_real, approximate_sqrt};
use crate::statistics::{EfficiencyStatus, EfficiencySummary};
use serde_json::{Value, json};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BatchExportError {
    InvalidState(String),
    Numeric { field: String, source: NumericError },
}
impl fmt::Display for BatchExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidState(message) => {
                write!(f, "cannot export inconsistent batch result: {message}")
            }
            Self::Numeric { field, source } => write!(f, "cannot export {field}: {source}"),
        }
    }
}
impl std::error::Error for BatchExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Numeric { source, .. } => Some(source),
            Self::InvalidState(_) => None,
        }
    }
}
struct Encoder {
    options: NumericOptions,
    underflow: Vec<String>,
    overflow: Vec<String>,
}
impl Encoder {
    fn number(
        &mut self,
        path: &str,
        value: Option<&ExactField>,
        root: bool,
    ) -> Result<(Value, Value), BatchExportError> {
        let Some(value) = value else {
            return Ok((Value::Null, Value::Null));
        };
        let result = if root {
            approximate_sqrt(value, self.options)
        } else {
            approximate_real(value, self.options)
        }
        .map_err(|source| BatchExportError::Numeric {
            field: path.to_owned(),
            source,
        })?;
        let ordinary = match result.binary64 {
            Binary64::Finite(value) => json!(value),
            Binary64::Underflow { .. } => {
                self.underflow.push(path.to_owned());
                Value::Null
            }
            Binary64::Overflow { .. } => {
                self.overflow.push(path.to_owned());
                Value::Null
            }
        };
        // Match the native scientific convention [0.5,1), preserving the
        // separately rounded direct binary64 rather than recomputing it.
        let m = result.scientific.mantissa;
        let scientific = if m == 0.0 {
            json!({"mantissa":0.0,"exponent2":0})
        } else {
            json!({"mantissa":m/2.0,"exponent2":result.scientific.exponent+1})
        };
        Ok((ordinary, scientific))
    }
    fn statistics(
        &mut self,
        s: &TaskRunStatistics,
        prefix: &str,
    ) -> Result<Value, BatchExportError> {
        let mut estimates = serde_json::Map::new();
        let mut scientific = serde_json::Map::new();
        let mut errors = serde_json::Map::new();
        let mut error_scientific = serde_json::Map::new();
        for (key, value, variance) in [
            (
                "acceptance",
                Some(&s.accepted),
                s.variances.accepted.as_ref(),
            ),
            ("error", s.error.as_ref(), s.variances.error.as_ref()),
            (
                "conditional_error",
                s.conditional_error.as_ref(),
                s.variances.conditional_error.as_ref(),
            ),
        ] {
            let (ordinary, scaled) =
                self.number(&format!("{prefix}estimates.{key}"), value, false)?;
            estimates.insert(key.to_owned(), ordinary);
            scientific.insert(key.to_owned(), scaled);
            let (ordinary, scaled) =
                self.number(&format!("{prefix}standard_errors.{key}"), variance, true)?;
            errors.insert(key.to_owned(), ordinary);
            error_scientific.insert(key.to_owned(), scaled);
        }
        let (mut ordinary, mut scaled, mut se, mut se_scaled) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for (index, (value, variance)) in s.moments.iter().zip(&s.variances.moments).enumerate() {
            let (x, y) = self.number(
                &format!("{prefix}estimates.moments[{index}]"),
                Some(value),
                false,
            )?;
            ordinary.push(x);
            scaled.push(y);
            let (x, y) = self.number(
                &format!("{prefix}standard_errors.moments[{index}]"),
                variance.as_ref(),
                true,
            )?;
            se.push(x);
            se_scaled.push(y);
        }
        estimates.insert("moments".to_owned(), json!(ordinary));
        scientific.insert("moments".to_owned(), json!(scaled));
        errors.insert("moments".to_owned(), json!(se));
        error_scientific.insert("moments".to_owned(), json!(se_scaled));
        Ok(
            json!({"estimates":estimates,"estimates_scientific":scientific,"standard_errors":errors,"standard_errors_scientific":error_scientific}),
        )
    }
    fn efficiency(&mut self, s: &EfficiencySummary) -> Result<Value, BatchExportError> {
        let mut result = serde_json::Map::new();
        result.insert("status".to_owned(), json!(s.status.as_str()));
        result.insert("completed".to_owned(), json!(s.completed));
        result.insert("zero_weights".to_owned(), json!(s.zero_weights));
        for (key, value) in [
            ("kappa", s.kappa.as_ref()),
            ("variance_coefficient", s.variance_coefficient.as_ref()),
            (
                "exact_marginal_variance_coefficient",
                s.exact_marginal_variance_coefficient.as_ref(),
            ),
            ("ess", Some(&s.effective_sample_size)),
            ("rho", s.rho.as_ref()),
            ("mean_weight", s.mean_weight.as_ref()),
        ] {
            let (ordinary, scientific) = self.number(&format!("efficiency.{key}"), value, false)?;
            if key == "kappa"
                && value.is_some()
                && ordinary.is_null()
                && s.status == EfficiencyStatus::Available
            {
                result.insert("status".to_owned(), json!("diagnostic_range_exceeded"));
            }
            result.insert(key.to_owned(), ordinary);
            result.insert(format!("{key}_scientific"), scientific);
        }
        result.insert("comparison".to_owned(),json!("same noise proposal and terminal estimator; exact_marginal draws histories from their true conditionals"));
        Ok(Value::Object(result))
    }
}
fn invalid(message: impl Into<String>) -> BatchExportError {
    BatchExportError::InvalidState(message.into())
}
fn validate(result: &TaskRunResult) -> Result<(), BatchExportError> {
    if result.requested == 0
        || result
            .completed
            .checked_add(result.failed)
            .and_then(|n| n.checked_add(result.not_attempted))
            != Some(result.requested)
    {
        return Err(invalid(
            "requested must be positive and equal completed + failed + not_attempted",
        ));
    }
    if result.completed == 0
        && (result.completed_network_compilations != 0 || result.completed_history_groups != 0)
    {
        return Err(invalid(
            "completed resources require at least one completed draw",
        ));
    }
    if result.failed > 1 {
        return Err(invalid("a stopped batch can have at most one failed draw"));
    }
    let failed_draws = result
        .failures
        .iter()
        .filter(|f| matches!(f.phase, RunPhase::Sampling | RunPhase::Accumulation))
        .collect::<Vec<_>>();
    if failed_draws.len() as u64 != result.failed
        || failed_draws
            .iter()
            .any(|f| f.shot != Some(result.completed))
    {
        return Err(invalid(
            "failed draw entries must match the first uncompleted shot",
        ));
    }
    if result.failures.iter().any(|f| {
        matches!(f.phase, RunPhase::Preparation | RunPhase::Finalization) && f.shot.is_some()
    }) {
        return Err(invalid(
            "preparation/finalization failures cannot name a failed draw",
        ));
    }
    if result
        .failures
        .iter()
        .any(|f| f.phase == RunPhase::Preparation)
        && (result.completed != 0
            || result.failed != 0
            || result.not_attempted != result.requested
            || result.failures.len() != 1)
    {
        return Err(invalid(
            "preparation failure requires zero attempted draws and one failure entry",
        ));
    }
    if result.status == RunStatus::Complete {
        if result.failed != 0
            || result.not_attempted != 0
            || !result.failures.is_empty()
            || result.statistics.is_none()
            || result.partial_statistics.is_some()
            || result.efficiency.is_none()
        {
            return Err(invalid(
                "complete results require complete statistics and no failures or provisional statistics",
            ));
        }
    } else if result.statistics.is_some() || result.failures.is_empty() {
        return Err(invalid(
            "failed results require failures and absent main statistics",
        ));
    }
    let preparation_failed = result
        .failures
        .iter()
        .any(|f| f.phase == RunPhase::Preparation);
    let finalization_failures = result
        .failures
        .iter()
        .filter(|f| f.phase == RunPhase::Finalization)
        .count();
    let summary = result
        .statistics
        .as_ref()
        .or(result.partial_statistics.as_ref());
    if preparation_failed {
        if result.efficiency.is_some() {
            return Err(invalid(
                "preparation failure cannot have accumulated efficiency",
            ));
        }
    } else {
        if result.failed == 0 && result.not_attempted != 0 {
            return Err(invalid(
                "only a preparation or draw failure can leave unattempted draws",
            ));
        }
        let missing_summaries = usize::from(result.completed > 0 && summary.is_none())
            + usize::from(result.efficiency.is_none());
        if finalization_failures != missing_summaries {
            return Err(invalid(
                "each unavailable accumulated summary requires its own finalization failure",
            ));
        }
    }
    for s in [&result.statistics, &result.partial_statistics]
        .into_iter()
        .flatten()
    {
        if s.completed != result.completed || s.completed == 0 {
            return Err(invalid(
                "statistics must describe exactly the positive completed count",
            ));
        }
        if s.moments.len() != s.variances.moments.len() {
            return Err(invalid("moment estimate and variance counts differ"));
        }
        let enough_samples = s.completed >= 2;
        let conditional_defined = s.error.is_some() && !s.accepted.is_zero();
        if s.conditional_error.is_some() != conditional_defined
            || s.variances.accepted.is_some() != enough_samples
            || s.variances.error.is_some() != (enough_samples && s.error.is_some())
            || s.variances.conditional_error.is_some() != (enough_samples && conditional_defined)
            || s.variances
                .moments
                .iter()
                .any(|v| v.is_some() != enough_samples)
        {
            return Err(invalid(
                "metric and variance availability must match definitions and completed count",
            ));
        }
    }
    if let Some(e) = &result.efficiency {
        if e.completed != result.completed || e.zero_weights > e.completed {
            return Err(invalid("efficiency counters disagree with completed draws"));
        }
        // Complete weights are nonnegative. Their sum and sum of squares
        // vanish exactly when every completed weight is zero (including n=0).
        let all_weights_zero = e.zero_weights == e.completed;
        if all_weights_zero != e.effective_sample_size.is_zero()
            || all_weights_zero != e.mean_weight.as_ref().is_none_or(ExactField::is_zero)
            || (all_weights_zero
                && matches!(
                    e.status,
                    EfficiencyStatus::Available | EfficiencyStatus::ZeroResidualVariance
                ))
        {
            return Err(invalid(
                "zero-weight count contradicts mean, ESS or efficiency availability",
            ));
        }
        if all_weights_zero
            && summary.is_some_and(|s| {
                !s.accepted.is_zero()
                    || s.error.as_ref().is_some_and(|v| !v.is_zero())
                    || s.moments.iter().any(|v| !v.is_zero())
                    || s.variances.accepted.as_ref().is_some_and(|v| !v.is_zero())
                    || s.variances.error.as_ref().is_some_and(|v| !v.is_zero())
                    || s.variances.moments.iter().flatten().any(|v| !v.is_zero())
            })
        {
            return Err(invalid(
                "all-zero weights require zero accumulated contributions",
            ));
        }
        let available = e.status == EfficiencyStatus::Available;
        if e.kappa.is_some() != available
            || e.variance_coefficient.is_some() != available
            || e.exact_marginal_variance_coefficient.is_some() != available
            || e.mean_weight.is_some() != (e.completed > 0)
            || e.rho.is_some() == e.effective_sample_size.is_zero()
        {
            return Err(invalid(
                "efficiency status and optional diagnostics disagree",
            ));
        }
        if (e.completed < 2
            && !matches!(
                e.status,
                EfficiencyStatus::NoErrorMetric | EfficiencyStatus::InsufficientSamples
            ))
            || (e.completed >= 2 && e.status == EfficiencyStatus::InsufficientSamples)
        {
            return Err(invalid("efficiency status disagrees with completed count"));
        }
        if let Some(s) = summary {
            if e.mean_weight.as_ref() != Some(&s.mean_weight)
                || e.effective_sample_size != s.effective_sample_size
            {
                return Err(invalid(
                    "statistics and efficiency weight summaries disagree",
                ));
            }
            let status_matches = if s.error.is_none() {
                e.status == EfficiencyStatus::NoErrorMetric
            } else if s.completed < 2 {
                e.status == EfficiencyStatus::InsufficientSamples
            } else if s.accepted.is_zero() {
                e.status == EfficiencyStatus::ZeroAcceptance
            } else {
                matches!(
                    e.status,
                    EfficiencyStatus::Available | EfficiencyStatus::ZeroResidualVariance
                )
            };
            if !status_matches {
                return Err(invalid(
                    "efficiency status disagrees with defined metrics and acceptance",
                ));
            }
        }
    }
    if result
        .sampled_records
        .as_ref()
        .is_some_and(|rows| rows.len() as u128 != result.completed as u128)
    {
        return Err(invalid(
            "retained record count disagrees with completed draws",
        ));
    }
    if let Some(rows) = &result.sampled_records
        && let Some(first) = rows.first()
        && rows.iter().any(|row| row.len() != first.len())
    {
        return Err(invalid("retained record rows must have a common width"));
    }
    if !result.timings.sampling_seconds.is_finite()
        || result.timings.sampling_seconds < 0.0
        || !result.timings.total_seconds.is_finite()
        || result.timings.total_seconds < 0.0
        || result
            .timings
            .preparation_seconds
            .is_some_and(|s| !s.is_finite() || s < 0.0)
    {
        return Err(invalid("timings must be finite and nonnegative"));
    }
    if result.timings.sampling_seconds > result.timings.total_seconds
        || result
            .timings
            .preparation_seconds
            .is_some_and(|s| s > result.timings.total_seconds)
    {
        return Err(invalid(
            "total timing cannot be smaller than a measured component",
        ));
    }
    Ok(())
}

/// Export the numerical payload shared by complete and failed batch results.
/// This is not a full versioned file result: execution metadata, fingerprints
/// and detailed query instrumentation must be supplied by their owning layer.
pub fn export_batch_metrics(
    result: &TaskRunResult,
    options: NumericOptions,
) -> Result<Value, BatchExportError> {
    validate(result)?;
    let mut encoder = Encoder {
        options,
        underflow: Vec::new(),
        overflow: Vec::new(),
    };
    let mut output = serde_json::Map::new();
    if let Some(statistics) = &result.statistics {
        output.extend(
            encoder
                .statistics(statistics, "")?
                .as_object()
                .expect("statistics object")
                .clone(),
        );
    } else {
        for name in [
            "estimates",
            "estimates_scientific",
            "standard_errors",
            "standard_errors_scientific",
        ] {
            output.insert(name.to_owned(), Value::Null);
        }
    }
    if result.status == RunStatus::Failed {
        let partial = result
            .partial_statistics
            .as_ref()
            .map(|s| encoder.statistics(s, "partial_statistics."))
            .transpose()?
            .unwrap_or(Value::Null);
        output.insert("partial_statistics".to_owned(), partial);
    }
    let efficiency = result
        .efficiency
        .as_ref()
        .map(|s| encoder.efficiency(s))
        .transpose()?
        .unwrap_or(Value::Null);
    output.insert("efficiency".to_owned(), efficiency);
    let weights = if let Some(s) = &result.efficiency {
        let (mean, mean_scientific) =
            encoder.number("weights.mean", s.mean_weight.as_ref(), false)?;
        let (ess, ess_scientific) =
            encoder.number("weights.ess", Some(&s.effective_sample_size), false)?;
        json!({"completed":s.completed,"mean":mean,"mean_scientific":mean_scientific,"ess":ess,"ess_scientific":ess_scientific,"convention":"complete noise likelihood times history mass / history proposal; no history factor for exact_marginal","ess_scope":"complete importance weights; not a guarantee of error-rate precision"})
    } else {
        Value::Null
    };
    output.insert("weights".to_owned(), weights);
    output.insert(
        "status".to_owned(),
        json!(match result.status {
            RunStatus::Complete => "complete",
            RunStatus::Failed => "failed",
        }),
    );
    for (name, count) in [
        ("requested", result.requested),
        ("completed", result.completed),
        ("failed", result.failed),
        ("not_attempted", result.not_attempted),
        ("seed", result.seed),
    ] {
        output.insert(name.to_owned(), json!(count));
    }
    output.insert("sampled_records".to_owned(), json!(result.sampled_records));
    output.insert("failures".to_owned(),json!(result.failures.iter().map(|f|json!({"phase":match f.phase {RunPhase::Preparation=>"preparation",RunPhase::Sampling=>"sampling",RunPhase::Accumulation=>"accumulation",RunPhase::Finalization=>"finalization"},"shot":f.shot,"type":f.kind,"message":f.message})).collect::<Vec<_>>()));
    output.insert("timings".to_owned(),json!({"preparation_seconds":result.timings.preparation_seconds,"sampling_seconds":result.timings.sampling_seconds,"total_seconds":result.timings.total_seconds}));
    output.insert("completed_resources".to_owned(),json!({"network_compilations":result.completed_network_compilations,"history_groups":result.completed_history_groups}));
    output.insert(
        "normalization".to_owned(),
        json!("mean joint contributions; conditional error is ratio of aggregate means"),
    );
    output.insert("uncertainty".to_owned(),json!("IID draw standard errors; conditional ratio uses paired delta method; null for fewer than two draws"));
    output.insert("numerics".to_owned(),json!({"representation":"mantissa * 2**exponent2","underflow_fields":encoder.underflow,"overflow_fields":encoder.overflow}));
    Ok(Value::Object(output))
}
