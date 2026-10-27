//! Numerical observations of a continuous prefix, with separate validity states.
use super::{BatchExportError, Encoder, validate_execution, validate_records, validate_summaries};
use crate::numeric::{NumericError, NumericOptions};
use crate::task::{RunPhase, TaskStreamReport, TaskStreamStatus};
use serde_json::{Value, json};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamExportError {
    InvalidState(String),
    Numeric { field: String, source: NumericError },
}
impl fmt::Display for StreamExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidState(message) => {
                write!(f, "cannot export inconsistent stream report: {message}")
            }
            Self::Numeric { field, source } => write!(f, "cannot export stream {field}: {source}"),
        }
    }
}
impl std::error::Error for StreamExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Numeric { source, .. } => Some(source),
            Self::InvalidState(_) => None,
        }
    }
}
impl From<BatchExportError> for StreamExportError {
    fn from(value: BatchExportError) -> Self {
        match value {
            BatchExportError::InvalidState(message) => Self::InvalidState(message),
            BatchExportError::Numeric { field, source } => Self::Numeric { field, source },
        }
    }
}
fn invalid(message: &str) -> StreamExportError {
    StreamExportError::InvalidState(message.to_owned())
}
fn validate(report: &TaskStreamReport) -> Result<(), StreamExportError> {
    if report.requested == 0
        || report
            .completed
            .checked_add(report.failed)
            .and_then(|n| n.checked_add(report.not_attempted))
            != Some(report.requested)
    {
        return Err(invalid(
            "requested must be positive and equal completed + failed + not_attempted",
        ));
    }
    let consistent = match report.status {
        TaskStreamStatus::Running => {
            report.completed < report.requested && report.failed == 0 && report.failure.is_none()
        }
        TaskStreamStatus::DrawsComplete => {
            report.completed == report.requested && report.failed == 0 && report.failure.is_none()
        }
        TaskStreamStatus::Failed => report.failed == 1 && report.failure.is_some(),
    };
    if !consistent {
        return Err(invalid(
            "draw status, cursors and terminal failure disagree",
        ));
    }
    if let Some(failure) = &report.failure
        && (!matches!(failure.phase, RunPhase::Sampling | RunPhase::Accumulation)
            || failure.shot != Some(report.completed)
            || !matches!(failure.kind, "TaskError" | "Unwind"))
    {
        return Err(invalid(
            "terminal stream failure must describe the first uncompleted sampling/accumulation attempt",
        ));
    }
    if report
        .failure
        .as_ref()
        .is_some_and(|failure| failure.message.is_empty())
    {
        return Err(invalid("terminal failure.message must be nonempty"));
    }
    if report.completed == 0 {
        if report.completed_network_compilations != 0 || report.completed_history_groups != 0 {
            return Err(invalid(
                "completed resources require at least one completed draw",
            ));
        }
        if !matches!(report.statistics, Ok(None)) || report.efficiency.is_err() {
            return Err(invalid(
                "an empty prefix requires absent statistics and a valid empty efficiency summary",
            ));
        }
        if report.failed == 0
            && (report.random_state != report.seed
                || report.timings.sampling_seconds != 0.0
                || [
                    report.execution_costs.compilation,
                    report.execution_costs.query,
                    report.execution_costs.decoder,
                    report.execution_costs.support,
                ]
                .iter()
                .any(|c| c.attempted != 0 || c.seconds != 0.0))
        {
            return Err(invalid(
                "an unattempted stream cannot have consumed RNG or sampling work",
            ));
        }
    } else if matches!(report.statistics, Ok(None)) {
        return Err(invalid(
            "a nonempty prefix requires statistics or an explicit diagnostic failure",
        ));
    }
    if report.timings.preparation_seconds.is_some() {
        return Err(invalid(
            "stream preparation is separate from cumulative sampling timings",
        ));
    }
    validate_summaries(
        report.completed,
        report.statistics.as_ref().ok().and_then(Option::as_ref),
        report.efficiency.as_ref().ok(),
    )?;
    validate_records(report.completed, report.sampled_records.as_deref())?;
    validate_execution(
        report.execution_limits,
        report.execution_costs,
        &report.timings,
    )?;
    for (operation, cost) in [
        ("compilation", report.execution_costs.compilation),
        ("query", report.execution_costs.query),
        ("decoder", report.execution_costs.decoder),
        ("support", report.execution_costs.support),
    ] {
        if cost.attempted == 0 && cost.seconds != 0.0 {
            return Err(StreamExportError::InvalidState(format!(
                "execution_costs.{operation}.seconds must be zero without attempts",
            )));
        }
    }
    if report.completed_network_compilations as u128
        > u128::from(report.execution_costs.compilation.succeeded)
    {
        return Err(invalid(
            "completed network compilations exceed successful measured compilation work",
        ));
    }
    if report.completed_history_groups as u128
        > u128::from(report.execution_costs.query.succeeded)
            + u128::from(report.execution_costs.support.succeeded)
    {
        return Err(invalid(
            "completed history groups exceed successful measured query and support work",
        ));
    }
    Ok(())
}

/// Export an immutable continuous prefix. Draw completion and diagnostic validity
/// remain separate. Estimates are provisional until every draw and both derived
/// diagnostics complete successfully. Errors leave the caller's typed report intact.
/// This is a numerical payload, not a durable checkpoint or versioned file report.
pub fn export_stream_metrics(
    report: &TaskStreamReport,
    options: NumericOptions,
) -> Result<Value, StreamExportError> {
    validate(report)?;
    let statistics = report.statistics.as_ref().ok().and_then(Option::as_ref);
    let efficiency = report.efficiency.as_ref().ok();
    let diagnostics_valid = report.statistics.is_ok() && report.efficiency.is_ok();
    let final_estimates = report.status == TaskStreamStatus::DrawsComplete && diagnostics_valid;
    let mut encoder = Encoder::new(options);
    let mut output = serde_json::Map::new();
    if let Some(s) = statistics {
        output.extend(
            encoder
                .statistics(s, "")?
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
    output.insert(
        "efficiency".to_owned(),
        efficiency
            .map(|s| encoder.efficiency(s))
            .transpose()?
            .unwrap_or(Value::Null),
    );
    let weights = if let Some(s) = efficiency {
        encoder.weights(
            s.completed,
            s.mean_weight.as_ref(),
            &s.effective_sample_size,
        )?
    } else if let Some(s) = statistics {
        encoder.weights(s.completed, Some(&s.mean_weight), &s.effective_sample_size)?
    } else {
        Value::Null
    };
    output.insert("weights".to_owned(), weights);
    let status = if report.status == TaskStreamStatus::Failed {
        "failed"
    } else if !diagnostics_valid {
        "diagnostic_failed"
    } else if final_estimates {
        "complete"
    } else {
        "running"
    };
    let estimation = if report.statistics.is_err() {
        "unavailable"
    } else if statistics.is_none() {
        "empty"
    } else if final_estimates {
        "final"
    } else {
        "provisional"
    };
    output.insert("kind".to_owned(), json!("stream_metrics"));
    output.insert("status".to_owned(), json!(status));
    output.insert(
        "draw_status".to_owned(),
        json!(match report.status {
            TaskStreamStatus::Running => "running",
            TaskStreamStatus::DrawsComplete => "draws_complete",
            TaskStreamStatus::Failed => "failed",
        }),
    );
    output.insert("estimation_status".to_owned(), json!(estimation));
    output.insert(
        "diagnostics".to_owned(),
        json!({
            "statistics": match &report.statistics {
                Ok(Some(_)) => json!({"status":"available","error":null}),
                Ok(None) => json!({"status":"empty","error":null}),
                Err(e) => json!({"status":"failed","error":{"field":e.field,"message":e.message}}),
            },
            "efficiency": match &report.efficiency {
                Ok(_) => json!({"status":"available","error":null}),
                Err(e) => json!({"status":"failed","error":{"message":e.to_string()}}),
            },
        }),
    );
    for (name, count) in [
        ("requested", report.requested),
        ("completed", report.completed),
        ("failed", report.failed),
        ("not_attempted", report.not_attempted),
        ("seed", report.seed),
        ("random_state", report.random_state),
    ] {
        output.insert(name.to_owned(), json!(count));
    }
    output.insert("sampled_records".to_owned(), json!(report.sampled_records));
    output.insert("failure".to_owned(), json!(report.failure.as_ref().map(|e| json!({
        "phase": match e.phase { RunPhase::Sampling=>"sampling", RunPhase::Accumulation=>"accumulation", _=>unreachable!("validated stream failure") },
        "shot":e.shot,"type":e.kind,"message":e.message,
    }))));
    output.insert("timings".to_owned(), json!({"preparation_seconds":null,"sampling_seconds":report.timings.sampling_seconds,"total_seconds":report.timings.total_seconds}));
    output.insert("completed_resources".to_owned(), json!({"network_compilations":report.completed_network_compilations,"history_groups":report.completed_history_groups}));
    output.insert(
        "execution_limits".to_owned(),
        report.execution_limits.to_json(),
    );
    let c = report.execution_costs;
    let cost = |c: crate::costs::OperationCost| json!({"attempted":c.attempted,"succeeded":c.succeeded,"failed":c.failed,"seconds":c.seconds});
    output.insert("execution_costs".to_owned(), json!({"scope":"cumulative_stream_including_failed_attempts","timing":"inclusive_wall_seconds","compilation":cost(c.compilation),"query":cost(c.query),"decoder":cost(c.decoder),"support":cost(c.support)}));
    output.insert(
        "normalization".to_owned(),
        json!("mean joint contributions; conditional error is ratio of aggregate means"),
    );
    output.insert("uncertainty".to_owned(), json!("IID draw standard errors; conditional ratio uses paired delta method; null for fewer than two draws"));
    output.insert("numerics".to_owned(), encoder.numerics());
    Ok(Value::Object(output))
}
