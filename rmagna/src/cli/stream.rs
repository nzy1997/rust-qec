//! Continuous CLI sampling with independently committed exact checkpoints.
mod persistence;
use super::{Args, load_task, output, transport};
use rmagna::{
    file_api::{FileTask, FileTaskStream},
    statistics::CheckpointOptions,
    task::{
        StreamRecordRetention, TaskStreamOptions, TaskStreamReport, TaskStreamStatus,
        export_stream_metrics,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::PathBuf};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    schema_version: u32,
    kind: String,
    executable_sha256: String,
    configuration_sha256: String,
    controller_declarations_sha256: Option<String>,
    terminal_failure: Option<String>,
    checkpoint: String,
}
struct Context {
    executable: String,
    configuration: String,
    declarations: Option<String>,
    saved: Option<Saved>,
    previous: Option<PathBuf>,
    latest: Option<PathBuf>,
    // Initial and terminal receipts can add two entries beyond u64::MAX draws.
    index: u128,
    terminal_failure: Option<String>,
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn executable() -> std::io::Result<String> {
    let mut file = File::open(std::env::current_exe()?)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn options(args: &Args) -> TaskStreamOptions {
    TaskStreamOptions {
        run: args.run,
        records: if args.stream.retain {
            StreamRecordRetention::Retain
        } else {
            StreamRecordRetention::Discard
        },
    }
}
fn implementation(hash: &str) -> String {
    format!("rmagna-cli-stream-v1:executable-file-sha256:{hash}")
}
fn preflight(args: &Args) -> Result<Context, String> {
    let executable = executable().map_err(|e| format!("executable file identity: {e}"))?;
    // Debug of resolved typed settings is intentionally scoped to this exact
    // executable. Operational output/checkpoint intervals are not run settings.
    let configuration = digest(
        format!(
            "{:?}",
            (
                args.file,
                &args.load,
                args.run,
                &args.sampling,
                args.stream.retain,
                &args.python,
                &args.module_root,
                transport(args),
            )
        )
        .as_bytes(),
    );
    let declarations = args
        .controllers
        .as_ref()
        .map(|p| persistence::read(p, args.load.task.max_json_bytes).map(|s| digest(s.as_bytes())))
        .transpose()
        .map_err(|e| format!("controller declarations: {e}"))?;
    let saved = args
        .stream
        .resume
        .as_ref()
        .map(|p| -> Result<Saved, String> {
            let text = persistence::read(p, args.stream.max_bytes)
                .map_err(|e| format!("resume {}: {e}", p.display()))?;
            if !text.trim_start().starts_with('{') {
                return Err("CLI checkpoint must be an object".into());
            }
            let saved: Saved =
                serde_json::from_str(&text).map_err(|e| format!("CLI checkpoint: {e}"))?;
            if saved.schema_version != 1 || saved.kind != "rmagna_cli_stream" {
                return Err("unsupported CLI checkpoint schema/kind".into());
            }
            if saved.executable_sha256 != executable {
                return Err("checkpoint executable file SHA-256 changed".into());
            }
            if saved.configuration_sha256 != configuration {
                return Err("checkpoint original run configuration changed".into());
            }
            if saved.controller_declarations_sha256 != declarations {
                return Err("checkpoint controller declarations changed".into());
            }
            let raw: Value = serde_json::from_str(&saved.checkpoint)
                .map_err(|e| format!("exact checkpoint: {e}"))?;
            let requested = args.run.shots.to_string();
            let seed = args.run.seed.to_string();
            let record_policy = if args.stream.retain { "retain" } else { "discard" };
            if raw["schema_version"] != 1
                || raw["kind"] != "task_stream"
                || raw["attestation"] != "caller_declared_unverified"
                || raw["record_policy"] != record_policy
                || raw["requested"].as_str() != Some(requested.as_str())
                || raw["original_seed"].as_str() != Some(seed.as_str())
                || raw["identity"]["implementation"] != implementation(&executable)
            {
                return Err(
                    "exact checkpoint schema, attestation, record policy, original count/seed or implementation mismatch"
                        .into(),
                );
            }
            Ok(saved)
        })
        .transpose()?;
    let directory = args
        .stream
        .directory
        .as_ref()
        .expect("required stream directory");
    persistence::reserve(directory)
        .map_err(|e| format!("checkpoint directory {}: {e}", directory.display()))?;
    let terminal_failure = saved.as_ref().and_then(|s| s.terminal_failure.clone());
    Ok(Context {
        executable,
        configuration,
        declarations,
        saved,
        previous: args.stream.resume.clone(),
        latest: None,
        index: 0,
        terminal_failure,
    })
}

pub fn add_failure(value: &mut Value, phase: &str, error: impl ToString) {
    value["status"] = json!("failed");
    if !value["failures"].is_array() {
        value["failures"] = json!([]);
    }
    value["failures"]
        .as_array_mut()
        .unwrap()
        .push(json!({"phase":phase,"type":"CliError","message":error.to_string()}));
    value[format!("{phase}_outcome")] = json!({"status":"failed","message":error.to_string()});
}

fn checkpoint(
    args: &Args,
    context: &mut Context,
    stream: &FileTaskStream<'_>,
    value: &mut Value,
) -> bool {
    let encoded = stream
        .checkpoint_json(CheckpointOptions {
            max_json_bytes: args.stream.max_bytes,
        })
        .map_err(|e| e.to_string())
        .and_then(|checkpoint| {
            serde_json::to_string(&Saved {
                schema_version: 1,
                kind: "rmagna_cli_stream".into(),
                executable_sha256: context.executable.clone(),
                configuration_sha256: context.configuration.clone(),
                controller_declarations_sha256: context.declarations.clone(),
                checkpoint,
                terminal_failure: context.terminal_failure.clone(),
            })
            .map_err(|e| e.to_string())
        });
    let text = match encoded {
        Ok(text) => text,
        Err(e) => {
            add_failure(value, "persistence", e);
            return false;
        }
    };
    match persistence::publish(
        args.stream.directory.as_ref().unwrap(),
        context.index,
        &text,
        args.stream.max_bytes,
    ) {
        Ok(path) => {
            context.latest = Some(path);
            context.index += 1;
            value["persistence_outcome"] = json!({"status":"committed"});
            true
        }
        Err(e) => {
            add_failure(value, "persistence", e);
            // The cap applies to this fallback too. If publication failed for an
            // IO reason, retain the latest exact state in the final report.
            if text.len() <= args.stream.max_bytes {
                value["unpersisted_checkpoint"] = json!(text);
            }
            false
        }
    }
}
fn identity_matches(task: &FileTask, args: &Args, context: &Context) -> Result<(), String> {
    if let Some(saved) = &context.saved {
        let identity = task
            .stream_identity(options(args), &implementation(&context.executable))
            .map_err(|e| e.to_string())?;
        let mut expected = identity.content.to_json();
        for key in ["algorithm", "encoding", "controller_attestation"] {
            expected.as_object_mut().unwrap().remove(key);
        }
        expected["implementation"] = json!(identity.implementation);
        let raw: Value = serde_json::from_str(&saved.checkpoint).map_err(|e| e.to_string())?;
        if raw["identity"] != expected {
            return Err("checkpoint loaded content or controller manifest changed".into());
        }
    }
    Ok(())
}
fn run(args: &Args, context: &mut Context, value: &mut Value) -> Result<(), String> {
    let (task, worker) = load_task(args).map_err(|e| e.to_string())?;
    if let Some(worker) = &worker
        && worker.manifest()["controllers_sha256"].as_str() != context.declarations.as_deref()
    {
        return Err("controller declarations changed between preflight and worker loading".into());
    }
    identity_matches(&task, args, context)?;
    *value = task
        .inspect_stream(options(args), &implementation(&context.executable))
        .map_err(|e| e.to_string())?;
    value["status"] = json!("running");
    value["failures"] = json!([]);
    value["controller_process"] = worker.as_ref().map_or(
        Value::Null,
        |w| json!({"manifest":w.manifest(),"transport":transport(args)}),
    );
    let started = std::time::Instant::now();
    let compiled = task
        .compile_with_limits(args.preparation.execution_limits)
        .map_err(|e| e.to_string())?;
    value["preparation"] = json!({"seconds":started.elapsed().as_secs_f64(),"limits":args.preparation.execution_limits.to_json(),"scope":"this_process_separate_from_sampling"});
    let mut stream = match &context.saved {
        Some(saved) => compiled.restore_stream_json(
            &saved.checkpoint,
            options(args),
            &implementation(&context.executable),
            CheckpointOptions {
                max_json_bytes: args.stream.max_bytes,
            },
        ),
        None => compiled.start_stream(options(args), &implementation(&context.executable)),
    }
    .map_err(|e| e.to_string())?;
    let (raw, persistence_ok) = advance(args, context, &mut stream, value);
    report(args, &raw, value);
    finish(args, context, &stream, &raw, persistence_ok, value);
    Ok(())
}
fn advance(
    args: &Args,
    context: &mut Context,
    stream: &mut FileTaskStream<'_>,
    value: &mut Value,
) -> (TaskStreamReport, bool) {
    let mut raw = stream.snapshot();
    // Commit the restored/initial state before attempting any new draw.
    let mut persistence_ok = checkpoint(args, context, stream, value);
    let mut remaining = args.stream.stop_after.unwrap_or(u64::MAX);
    while persistence_ok
        && context.terminal_failure.is_none()
        && raw.statistics.is_ok()
        && raw.efficiency.is_ok()
        && remaining > 0
        && stream.status() == TaskStreamStatus::Running
    {
        let count = remaining
            .min(args.stream.every.get())
            .min(raw.not_attempted);
        // The library records an unwind sentinel before consuming any entropy.
        let advance =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| stream.advance(count)));
        raw = match advance {
            Ok(Ok(report)) => report,
            Ok(Err(error)) => {
                add_failure(value, "draw", error);
                stream.snapshot()
            }
            Err(_) => {
                add_failure(value, "draw", "draw unwound; saved stream is terminal");
                stream.snapshot()
            }
        };
        remaining -= count;
        persistence_ok = checkpoint(args, context, stream, value);
        if raw.statistics.is_err() || raw.efficiency.is_err() {
            break;
        }
    }
    (raw, persistence_ok)
}
fn report(args: &Args, raw: &TaskStreamReport, value: &mut Value) {
    let diagnostics_ok = raw.statistics.is_ok() && raw.efficiency.is_ok();
    value["draw_outcome"] = json!({"status":match raw.status {TaskStreamStatus::Running=>"paused",TaskStreamStatus::DrawsComplete=>"complete",TaskStreamStatus::Failed=>"failed"}});
    value["diagnostic_outcome"] = json!({"status":if diagnostics_ok {"available"}else{"failed"},"statistics":raw.statistics.as_ref().err().map(ToString::to_string),"efficiency":raw.efficiency.as_ref().err().map(ToString::to_string)});
    // Preserve counters even if numerical conversion fails. No synthetic zeros.
    for (k, n) in [
        ("requested", raw.requested),
        ("completed", raw.completed),
        ("failed", raw.failed),
        ("not_attempted", raw.not_attempted),
        ("seed", raw.seed),
        ("random_state", raw.random_state),
    ] {
        value[k] = json!(n);
    }
    value["sampled_records"] = json!(raw.sampled_records);
    value["execution_costs"] = output::costs(raw.execution_costs);
    value["execution_costs"]["scope"] = json!("cumulative_stream_including_failed_attempts");
    value["timings"] = json!({"sampling_seconds":raw.timings.sampling_seconds,"total_seconds":raw.timings.total_seconds});
    match export_stream_metrics(raw, args.file.numeric) {
        Ok(metrics) => {
            value["metrics"] = metrics;
            value["export_outcome"] = json!({"status":"complete"});
        }
        Err(e) => add_failure(value, "export", e),
    }
    if raw.status == TaskStreamStatus::Failed {
        add_failure(
            value,
            "draw",
            raw.failure
                .as_ref()
                .map(|e| e.message.as_str())
                .unwrap_or("terminal draw failure"),
        );
    }
    if !diagnostics_ok {
        add_failure(value, "diagnostic", "derived stream summaries failed");
    }
}
fn finish(
    args: &Args,
    context: &mut Context,
    stream: &FileTaskStream<'_>,
    raw: &TaskStreamReport,
    mut persistence_ok: bool,
    value: &mut Value,
) {
    if let Some(reason) = &context.terminal_failure {
        add_failure(value, "terminal", reason);
    }
    if value["failures"].as_array().is_some_and(|f| !f.is_empty()) && persistence_ok {
        context.terminal_failure =
            Some("a previous stream invocation failed; no additional draws permitted".into());
        persistence_ok = checkpoint(args, context, stream, value);
    }
    if value["failures"].as_array().is_some_and(|f| f.is_empty()) && persistence_ok {
        value["status"] = json!(if raw.status == TaskStreamStatus::DrawsComplete {
            "complete"
        } else {
            "paused"
        });
    }
}
pub fn execute(args: &Args) -> Value {
    let mut context = match preflight(args) {
        Ok(context) => context,
        Err(e) => {
            let mut value = output::failure("stream_preflight", e);
            value["kind"] = json!("stream");
            for phase in ["draw", "diagnostic", "persistence", "export"] {
                value[format!("{phase}_outcome")] = json!({"status":"not_started"});
            }
            return value;
        }
    };
    let mut value = json!({"schema_version":2,"software_version":concat!("rmagna-",env!("CARGO_PKG_VERSION")),"kind":"stream","status":"running","failures":[]});
    if let Err(e) = run(args, &mut context, &mut value) {
        add_failure(&mut value, "input_or_preparation", e);
    }
    for phase in ["draw", "diagnostic", "persistence", "export"] {
        let key = format!("{phase}_outcome");
        if value.get(&key).is_none() {
            value[&key] = json!({"status":"not_started"});
        }
    }
    if let Some(saved) = &context.saved {
        let raw: Value =
            serde_json::from_str(&saved.checkpoint).expect("preflight decoded exact JSON");
        value["saved_prefix"] = json!({"requested":raw["requested"],"completed":raw["committed"],"failed":raw["failed"],"not_attempted":raw["not_attempted"],"random_state":raw["random_state"],"scope":"original saved state; restoration and current counts are reported separately"});
    }
    value["checkpoint"] = json!({"latest_committed":context.latest.as_ref().or(context.previous.as_ref()),"previous":context.previous,"new_directory":args.stream.directory});
    value["executable_file"] = json!({"sha256":context.executable,"scope":"current executable file bytes; not mapped-code or external-callback authentication"});
    value
}
