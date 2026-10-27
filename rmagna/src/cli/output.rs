use rmagna::costs::ExecutionCosts;
use rmagna::file_api::{FileEnumerationReport, FileRunReport};
use rmagna::task::{BatchExportError, RunStatus};
use serde_json::{Value, json};
use std::io::{self, Write};

pub fn failure(phase: &str, message: impl ToString) -> Value {
    json!({"schema_version":2,"software_version":concat!("rmagna-",env!("CARGO_PKG_VERSION")),"status":"failed","failures":[{"phase":phase,"type":"CliError","message":message.to_string()}]})
}
/// Export failure is a CLI failure even if execution completed. Preserve actual
/// counts and metadata; never substitute zero estimates or discard a prefix.
pub fn export_failure(report: &FileRunReport, error: &BatchExportError) -> Value {
    let raw = report.raw_result();
    let mut value = report.metadata().clone();
    value["status"] = json!("failed");
    value["engine_status"] = json!(match raw.status {
        RunStatus::Complete => "complete",
        RunStatus::Failed => "failed",
    });
    for (key, count) in [
        ("requested", raw.requested),
        ("completed", raw.completed),
        ("failed", raw.failed),
        ("not_attempted", raw.not_attempted),
        ("seed", raw.seed),
    ] {
        value[key] = json!(count);
    }
    for key in [
        "estimates",
        "estimates_scientific",
        "standard_errors",
        "standard_errors_scientific",
        "partial_statistics",
        "efficiency",
        "weights",
    ] {
        value[key] = Value::Null;
    }
    value["sampled_records"] = json!(raw.sampled_records);
    let mut failures: Vec<_> = raw.failures.iter().map(|f| json!({"phase":format!("{:?}", f.phase).to_lowercase(),"shot":f.shot,"type":f.kind,"message":f.message})).collect();
    failures.push(
        json!({"phase":"export","shot":null,"type":"BatchExportError","message":error.to_string()}),
    );
    value["failures"] = json!(failures);
    value["execution_costs"] = costs(raw.execution_costs);
    value["execution_limits"] = raw.execution_limits.to_json();
    value["timings"] = json!({"preparation_seconds":raw.timings.preparation_seconds,"sampling_seconds":raw.timings.sampling_seconds,"total_seconds":raw.timings.total_seconds});
    if let Some(seconds) = value["progress"]["seconds"].as_f64() {
        value["timings"]["progress_seconds"] = json!(seconds);
    }
    value["numeric_export"] = json!({"status":"failed","exact_results_persisted":false});
    value
}
pub(super) fn costs(costs: ExecutionCosts) -> Value {
    let row = |c: rmagna::costs::OperationCost| json!({"attempted":c.attempted,"succeeded":c.succeeded,"failed":c.failed,"seconds":c.seconds});
    json!({"scope":"this_call_including_failed_attempts","timing":"inclusive_wall_seconds","compilation":row(costs.compilation),"query":row(costs.query),"decoder":row(costs.decoder),"support":row(costs.support)})
}
pub fn enumeration_export_failure(
    report: &FileEnumerationReport,
    error: &BatchExportError,
) -> Value {
    let mut value = report.metadata().clone();
    value["status"] = json!("failed");
    value["engine_status"] = json!(if report.is_complete() {
        "complete"
    } else {
        "failed"
    });
    value["result"] = Value::Null;
    value["failures"] =
        json!([{"phase":"export","type":"BatchExportError","message":error.to_string()}]);
    value["metadata_failure"] = report.metadata_failure().map_or(
        Value::Null,
        |e| json!({"field":e.field,"message":e.message}),
    );
    if let Ok(raw) = report.raw_result() {
        value["traversal_counts"] = json!({"patterns":raw.patterns,"histories":raw.histories,"marginal_queries":raw.marginal_queries,"replay_attempts":raw.replay_attempts,"network_compilations":raw.network_compilations});
    }
    value["execution_costs"] = costs(report.execution_costs());
    value["timings"] =
        json!({"total_seconds":report.total_seconds(),"components_are_additive":false});
    value["numeric_export"] = json!({"status":"failed","exact_results_persisted":false});
    value
}
fn real(value: &Value) -> String {
    if let Some(n) = value["value"].as_f64() {
        return n.to_string();
    }
    match (
        value["scientific"]["mantissa"].as_f64(),
        value["scientific"]["exponent2"].as_i64(),
    ) {
        (Some(m), Some(e)) => format!("{m} * 2^{e}"),
        _ => "n/a".to_owned(),
    }
}
fn rational(value: &Value) -> String {
    match (value["numerator"].as_str(), value["denominator"].as_str()) {
        (Some(n), Some("1")) => n.to_owned(),
        (Some(n), Some(d)) => format!("{n}/{d}"),
        _ => "n/a".to_owned(),
    }
}
fn interval(value: &Value) -> String {
    if value.is_null() {
        "n/a".to_owned()
    } else {
        format!("[{}, {}]", real(&value["lower"]), real(&value["upper"]))
    }
}
fn enumeration_summary(stdout: &mut impl Write, value: &Value) -> io::Result<()> {
    let r = &value["result"];
    if r.is_null() {
        return Ok(());
    }
    writeln!(
        stdout,
        "Enumerated patterns: {}; positive histories: {}",
        r["patterns"], r["histories"]
    )?;
    writeln!(
        stdout,
        "Covered physical probability: {}",
        rational(&r["covered_probability"])
    )?;
    writeln!(
        stdout,
        "Omitted physical probability: {}",
        rational(&r["omitted_probability"])
    )?;
    writeln!(
        stdout,
        "Covered acceptance mass: {}",
        real(&r["masses"]["accepted"])
    )?;
    writeln!(
        stdout,
        "Covered joint error mass: {}",
        real(&r["masses"]["error"])
    )?;
    writeln!(
        stdout,
        "Full acceptance interval: {}",
        interval(&r["full_acceptance"])
    )?;
    writeln!(
        stdout,
        "Full joint error interval: {}",
        interval(&r["full_error"])
    )?;
    writeln!(
        stdout,
        "Full conditional error interval: {}",
        interval(&r["conditional_error"])
    )
}
fn estimate(value: &Value, key: &str) -> String {
    if let Some(number) = value["estimates"][key].as_f64() {
        return number.to_string();
    }
    let scaled = &value["estimates_scientific"][key];
    match (scaled["mantissa"].as_f64(), scaled["exponent2"].as_i64()) {
        (Some(m), Some(e)) => format!("{m} * 2^{e}"),
        _ => "n/a".to_owned(),
    }
}
fn number_or_na(value: &Value) -> String {
    value
        .as_f64()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "n/a".into())
}
fn preparation_summary(stdout: &mut impl Write, value: &Value) -> io::Result<()> {
    let counts = &value["sampling_preparation"]["counts"];
    if !counts.is_null() {
        writeln!(
            stdout,
            "Prepared families: {} created, {} reused, {} cold; {}/{} requests attempted",
            counts["created"],
            counts["reused"],
            counts["cold"],
            counts["attempted"],
            counts["planned"]
        )?;
    }
    writeln!(
        stdout,
        "Preparation seconds: {}",
        number_or_na(&value["timings"]["total_seconds"])
    )?;
    for key in ["failure", "metadata_failure"] {
        if let Some(message) = value[key]["message"].as_str() {
            writeln!(
                stdout,
                "{}: {message}",
                value[key]["field"].as_str().unwrap_or("preparation")
            )?;
        }
    }
    Ok(())
}
fn comparison_summary(stdout: &mut impl Write, value: &Value) -> io::Result<()> {
    writeln!(
        stdout,
        "Pilot draws requested per sampler: {}",
        value["pilot_shots_per_channel"]
    )?;
    if let Some(sampler) = value["recommended_sampler"].as_str() {
        writeln!(
            stdout,
            "Gamma: {}; recommended sampler: {sampler}",
            value["gamma"]
        )?;
    } else {
        writeln!(
            stdout,
            "No recommendation: {}",
            value["reason"].as_str().unwrap_or("unavailable")
        )?;
    }
    for sampler in ["affine_support", "exact_marginal"] {
        let pilot = &value["pilots"][sampler];
        let receipt = &pilot["sampling_receipt"];
        let draws = if receipt.is_object() {
            format!(
                "requested {}, completed {}, failed {}, not attempted {}",
                receipt["requested"],
                receipt["completed"],
                receipt["failed"],
                receipt["not_attempted"]
            )
        } else {
            "sampling not started".to_owned()
        };
        writeln!(
            stdout,
            "{sampler}: {}; draws: {draws}; preparation seconds: {}; sampling seconds: {}",
            pilot["status"].as_str().unwrap_or("unavailable"),
            number_or_na(&pilot["preparation"]["timings"]["total_seconds"]),
            number_or_na(&pilot["sampling_receipt"]["sampling_seconds"])
        )?;
        for key in ["failure", "metadata_failure"] {
            if let Some(message) = pilot["preparation"][key]["message"].as_str() {
                writeln!(stdout, "  preparation: {message}")?;
            }
        }
        for key in ["failure", "sampling_export_failure"] {
            if let Some(message) = pilot[key].as_str() {
                writeln!(stdout, "  {message}")?;
            }
        }
        if let Some(failures) = pilot["sampling"]["failures"].as_array() {
            for failure in failures {
                if let Some(message) = failure["message"].as_str() {
                    writeln!(stdout, "  sampling: {message}")?;
                }
            }
        }
    }
    writeln!(
        stdout,
        "Pilot point estimate only; automatic switching disabled; no production run."
    )
}
pub fn print(value: &Value, json_output: bool) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    if json_output {
        serde_json::to_writer(&mut stdout, value)?;
        writeln!(stdout)
    } else {
        writeln!(
            stdout,
            "rmagna: {}",
            value["status"].as_str().unwrap_or("unknown")
        )?;
        if let Some(qubits) = value["circuit"]["qubits"].as_u64() {
            writeln!(
                stdout,
                "Circuit: {qubits} qubits, {} records, {} instructions",
                value["circuit"]["records"], value["circuit"]["instructions"]
            )?;
        }
        if let Some(count) = value["completed"].as_u64() {
            writeln!(stdout, "Completed draws: {count}/{}", value["requested"])?;
            let statistics = if value["status"] == "failed" {
                &value["partial_statistics"]
            } else {
                value
            };
            let prefix = if value["status"] == "failed" {
                "Partial "
            } else {
                ""
            };
            for (label, key) in [
                ("acceptance", "acceptance"),
                ("joint error mass", "error"),
                ("conditional error", "conditional_error"),
            ] {
                writeln!(stdout, "{prefix}{label}: {}", estimate(statistics, key))?;
            }
        }
        if value["kind"] == "preparation" {
            preparation_summary(&mut stdout, value)?;
        }
        if value["kind"] == "sampler_comparison" {
            comparison_summary(&mut stdout, value)?;
        }
        if value["kind"] == "enumeration" {
            enumeration_summary(&mut stdout, value)?;
            for key in ["failure", "metadata_failure"] {
                if let Some(message) = value[key]["message"].as_str() {
                    writeln!(
                        stdout,
                        "{}: {}: {message}",
                        value["failure_phase"].as_str().unwrap_or("metadata"),
                        value[key]["field"].as_str().unwrap_or("execution")
                    )?;
                }
            }
        }
        if let Some(failures) = value["failures"].as_array() {
            for failure in failures {
                writeln!(
                    stdout,
                    "{}: {}",
                    failure["phase"].as_str().unwrap_or("error"),
                    failure["message"].as_str().unwrap_or("unknown failure")
                )?;
            }
        }
        if let Some(route) = value["route"].as_str() {
            writeln!(stdout, "Route: {route}")?;
        }
        Ok(())
    }
}

/// Keep diagnostics on stderr and leave stdout as the final report.
pub fn progress(stderr: &mut impl Write, event: &Value) -> io::Result<()> {
    let efficiency = &event["efficiency"];
    let kappa = efficiency["kappa"]
        .as_f64()
        .map(|v| v.to_string())
        .unwrap_or_else(|| {
            let reason = event["diagnostic_failure"]["message"]
                .as_str()
                .or_else(|| efficiency["status"].as_str())
                .unwrap_or("unavailable");
            format!("unavailable ({reason})")
        });
    writeln!(
        stderr,
        "[{}/{}] history_sampler={} kappa={kappa}",
        event["completed"],
        event["requested"],
        event["history_sampler"].as_str().unwrap_or("unavailable")
    )?;
    stderr.flush()
}
