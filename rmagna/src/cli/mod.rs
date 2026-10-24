mod args;
mod output;

use args::{Args, Mode};
use rmagna::file_api::{FileSamplerComparisonOptions, FileTask};
use rmagna::runtime::ControllerRegistry;
use rmagna::runtime::process::ControllerProcess;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap, error::Error, fs::OpenOptions, io::Write, path::Path, process::Command,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn execute(args: &Args) -> Result<Value> {
    let mut registry = ControllerRegistry::default();
    let mut identities = BTreeMap::new();
    // Keep the process alive through execution and serialize its actual manifest
    // into every binding identity, including the configured transport limits.
    let worker = if let Some(config) = &args.controllers {
        let root = args
            .module_root
            .as_deref()
            .unwrap_or_else(|| config.parent().unwrap_or(Path::new(".")));
        let mut command = Command::new(&args.python);
        command
            .args([
                "-I",
                "-B",
                "-u",
                "-c",
                include_str!("../../python/decoder_worker.py"),
                "--controllers",
            ])
            .arg(config)
            .arg("--module-root")
            .arg(root);
        let worker = ControllerProcess::spawn(command, args.process)?;
        worker.register_all(&mut registry)?;
        for name in worker.versions().keys() {
            identities.insert(name.clone(), json!({"binding":worker.manifest()["bindings"][name],"manifest":worker.manifest(),"transport":transport(args)}));
        }
        Some(worker)
    } else {
        None
    };
    let task = FileTask::load(
        &args.circuit,
        args.task.as_deref(),
        &registry,
        args.sampling.clone(),
        args.file,
        &args.load,
        identities,
    )?;
    let mut value = match args.mode {
        Mode::Validate => {
            let mut value = task.inspect(&args.run)?;
            value["kind"] = json!("validation");
            value["status"] = json!("valid");
            value
        }
        Mode::Inspect => {
            let mut value = task
                .compile_with_limits(args.run.execution_limits)?
                .inspect(&args.run)?;
            value["status"] = json!("complete");
            value
        }
        Mode::Run => {
            let report = task.run(&args.run)?;
            match report.to_json() {
                Ok(value) => value,
                Err(error) => output::export_failure(&report, &error),
            }
        }
        Mode::Prepare => task.prepare_sampling(&args.preparation)?.report().to_json(),
        Mode::Compare => task
            .compare_samplers(&FileSamplerComparisonOptions {
                pilot: args.run,
                preparation: args.preparation,
            })?
            .to_json(),
        Mode::Enumerate => {
            let report = task.enumerate(&args.enumeration)?;
            match report.to_json() {
                Ok(value) => value,
                Err(error) => output::enumeration_export_failure(&report, &error),
            }
        }
    };
    value["controller_process"] = worker.as_ref().map_or(
        Value::Null,
        |worker| json!({"manifest":worker.manifest(),"transport":transport(args)}),
    );
    Ok(value)
}
fn transport(args: &Args) -> Value {
    let p = args.process;
    json!({"timeout":{"seconds":p.timeout.as_secs(),"nanoseconds":p.timeout.subsec_nanos()},"max_frame_bytes":p.max_frame_bytes,"max_bits":p.max_bits,"max_bindings":p.max_bindings})
}
pub fn main() -> u8 {
    let raw = match std::env::args_os()
        .skip(1)
        .map(|s| s.into_string().map_err(|_| "arguments must be UTF-8"))
        .collect::<std::result::Result<Vec<_>, _>>()
    {
        Ok(raw) => raw,
        Err(error) => {
            eprintln!("rmagna: {error}");
            return 2;
        }
    };
    let help = (raw.len() == 1 && matches!(raw[0].as_str(), "--help" | "-h"))
        || (raw.len() == 2
            && matches!(
                raw[0].as_str(),
                "run" | "inspect" | "validate" | "enumerate" | "prepare" | "compare-samplers"
            )
            && matches!(raw[1].as_str(), "--help" | "-h"));
    if help || raw == ["--version"] {
        let text = if help {
            args::HELP
        } else {
            concat!(
                "rmagna ",
                env!("CARGO_PKG_VERSION"),
                " (independent Rust implementation)\n"
            )
        };
        return if std::io::stdout().lock().write_all(text.as_bytes()).is_ok() {
            0
        } else {
            1
        };
    }
    let wants_json = raw.iter().any(|s| s == "--json");
    let args = match Args::parse(raw) {
        Ok(args) => args,
        Err(error) => {
            let _ = output::print(&output::failure("arguments", error), wants_json);
            return 2;
        }
    };
    // Reserve the destination before importing decoders or running expensive work.
    let mut destination = match args
        .output
        .as_ref()
        .map(|p| OpenOptions::new().write(true).create_new(true).open(p))
        .transpose()
    {
        Ok(file) => file,
        Err(error) => {
            let _ = output::print(
                &output::failure(
                    "output",
                    format!(
                        "cannot create {}: {error}",
                        args.output.as_ref().expect("output path").display()
                    ),
                ),
                args.json,
            );
            return 1;
        }
    };
    let value = execute(&args)
        .unwrap_or_else(|error| output::failure("input_or_preparation", error.to_string()));
    if let Some(file) = &mut destination {
        let written = serde_json::to_writer_pretty(&mut *file, &value)
            .map_err(std::io::Error::other)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.flush());
        if let Err(error) = written {
            let _ = output::print(
                &output::failure("output", format!("cannot write report: {error}")),
                args.json,
            );
            return 1;
        }
    }
    if output::print(&value, args.json).is_err() {
        return 1;
    }
    u8::from(value["status"] == "failed")
}
