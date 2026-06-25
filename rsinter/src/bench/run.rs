use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use crate::bench::merge::merge_result_rows;
use crate::bench::registry::{
    expand_runner_points_for_runner, BenchCasePoint, BenchRunContext, RustBenchRunner,
    RustRunnerRegistry,
};
use crate::bench::result::{
    read_results_jsonl, write_results_jsonl, BenchmarkResultRow, RunManifest,
};
use crate::bench::spec::{BenchmarkSpec, RunnerSpec};

struct PlannedRustRun<'a> {
    runner: &'a RunnerSpec,
    runner_impl: &'a dyn RustBenchRunner,
    points: Vec<BenchCasePoint>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BenchRunOptions {
    pub resume: bool,
}

pub fn run_rust_benchmark(
    spec: &BenchmarkSpec,
    language: &str,
    out_root: &Path,
    registry: &RustRunnerRegistry,
    spec_dir: &Path,
) -> Result<PathBuf, String> {
    run_rust_benchmark_with_options(
        spec,
        language,
        out_root,
        registry,
        spec_dir,
        BenchRunOptions::default(),
    )
}

pub fn run_rust_benchmark_with_options(
    spec: &BenchmarkSpec,
    language: &str,
    out_root: &Path,
    registry: &RustRunnerRegistry,
    spec_dir: &Path,
    options: BenchRunOptions,
) -> Result<PathBuf, String> {
    spec.validate()?;
    fs::create_dir_all(out_root).map_err(|e| e.to_string())?;
    if !options.resume {
        clear_rust_run_artifacts(spec, language, out_root)?;
    }
    let planned_runs = plan_rust_runs(spec, language, registry)?;
    let resume_rows = if options.resume {
        load_resume_rows(&planned_runs, out_root)?
    } else {
        BTreeMap::new()
    };

    for PlannedRustRun {
        runner,
        runner_impl,
        points,
    } in planned_runs
    {
        let artifact_dir = out_root.join(&runner.name).join("test-run");
        let staging_dir = out_root.join(&runner.name).join("test-run.tmp");
        if staging_dir.exists() {
            fs::remove_dir_all(&staging_dir).map_err(|e| e.to_string())?;
        }

        let ctx = BenchRunContext {
            benchmark_name: spec.name.clone(),
            runner_name: runner.name.clone(),
            language: language.to_string(),
            seed: 12_345,
            spec_dir: spec_dir.to_path_buf(),
        };
        let existing_rows = resume_rows.get(&runner.name).cloned().unwrap_or_default();
        let completed = completed_identities(&existing_rows)?;
        let mut fresh_rows = Vec::new();
        for point in &points {
            let row = runner_impl.run_point(point, &ctx)?;
            let identity = row.identity()?;
            if !completed.contains(&identity) {
                fresh_rows.push(row);
            }
        }
        let rows = if options.resume {
            merge_result_rows(vec![existing_rows, fresh_rows])?
        } else {
            fresh_rows
        };

        fs::create_dir_all(&staging_dir).map_err(|e| e.to_string())?;
        let manifest = RunManifest::new(
            spec.name.clone(),
            spec.version,
            runner.name.clone(),
            language.to_string(),
            artifact_dir.display().to_string(),
        );
        serde_json::to_writer_pretty(
            File::create(staging_dir.join("run_manifest.json")).map_err(|e| e.to_string())?,
            &manifest,
        )
        .map_err(|e| e.to_string())?;
        let mut file =
            File::create(staging_dir.join("results.jsonl")).map_err(|e| e.to_string())?;
        write_results_jsonl(&rows, &mut file)?;
        if artifact_dir.exists() {
            fs::remove_dir_all(&artifact_dir).map_err(|e| e.to_string())?;
        }
        fs::rename(&staging_dir, &artifact_dir).map_err(|e| e.to_string())?;
    }

    Ok(out_root.to_path_buf())
}

fn clear_rust_run_artifacts(
    spec: &BenchmarkSpec,
    language: &str,
    out_root: &Path,
) -> Result<(), String> {
    for runner in spec
        .runners
        .iter()
        .filter(|runner| runner.language == language)
    {
        let artifact_dir = out_root.join(&runner.name).join("test-run");
        let staging_dir = out_root.join(&runner.name).join("test-run.tmp");
        if artifact_dir.exists() {
            fs::remove_dir_all(&artifact_dir).map_err(|e| e.to_string())?;
        }
        if staging_dir.exists() {
            fs::remove_dir_all(&staging_dir).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn plan_rust_runs<'a>(
    spec: &'a BenchmarkSpec,
    language: &str,
    registry: &'a RustRunnerRegistry,
) -> Result<Vec<PlannedRustRun<'a>>, String> {
    let mut planned_runs = Vec::new();
    for runner in spec
        .runners
        .iter()
        .filter(|runner| runner.language == language)
    {
        let runner_impl = registry
            .get(&runner.impl_key)
            .ok_or_else(|| format!("unknown rust runner: {}", runner.impl_key))?;
        let points = expand_runner_points_for_runner(runner_impl.name(), &runner.params)?;
        for point in &points {
            runner_impl.preflight_point(point)?;
        }
        planned_runs.push(PlannedRustRun {
            runner,
            runner_impl: runner_impl.as_ref(),
            points,
        });
    }
    Ok(planned_runs)
}

fn load_resume_rows(
    planned_runs: &[PlannedRustRun<'_>],
    out_root: &Path,
) -> Result<BTreeMap<String, Vec<BenchmarkResultRow>>, String> {
    let mut rows_by_runner = BTreeMap::new();
    for planned in planned_runs {
        let path = out_root
            .join(&planned.runner.name)
            .join("test-run")
            .join("results.jsonl");
        if !path.exists() {
            continue;
        }
        let data = fs::read(&path).map_err(|error| {
            format!("failed to read resume results {}: {error}", path.display())
        })?;
        let rows = read_results_jsonl(&data[..]).map_err(|error| {
            format!("failed to read resume results {}: {error}", path.display())
        })?;
        rows_by_runner.insert(planned.runner.name.clone(), rows);
    }
    Ok(rows_by_runner)
}

fn completed_identities(rows: &[BenchmarkResultRow]) -> Result<BTreeSet<String>, String> {
    let mut completed = BTreeSet::new();
    for row in rows {
        if row.status == "ok" {
            completed.insert(row.identity()?);
        }
    }
    Ok(completed)
}
