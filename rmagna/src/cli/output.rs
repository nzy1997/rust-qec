use rmagna::file_api::FileRunReport;
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
    let costs = raw.execution_costs;
    let row = |c: rmagna::costs::OperationCost| json!({"attempted":c.attempted,"succeeded":c.succeeded,"failed":c.failed,"seconds":c.seconds});
    value["execution_costs"] = json!({"scope":"this_call_including_failed_attempts","timing":"inclusive_wall_seconds","compilation":row(costs.compilation),"query":row(costs.query),"decoder":row(costs.decoder),"support":row(costs.support)});
    value["execution_limits"] = raw.execution_limits.to_json();
    value["timings"] = json!({"preparation_seconds":raw.timings.preparation_seconds,"sampling_seconds":raw.timings.sampling_seconds,"total_seconds":raw.timings.total_seconds});
    value["numeric_export"] = json!({"status":"failed","exact_results_persisted":false});
    value
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
