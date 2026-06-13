use std::fs::{self, File};
use std::path::{Path, PathBuf};

use crate::bench::registry::{
    expand_runner_points_for_runner, BenchRunContext, RustRunnerRegistry,
};
use crate::bench::result::{write_results_jsonl, RunManifest};
use crate::bench::spec::BenchmarkSpec;

pub fn run_rust_benchmark(
    spec: &BenchmarkSpec,
    language: &str,
    out_root: &Path,
    registry: &RustRunnerRegistry,
    spec_dir: &Path,
) -> Result<PathBuf, String> {
    spec.validate()?;
    fs::create_dir_all(out_root).map_err(|e| e.to_string())?;

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

        let runner_impl = registry
            .get(&runner.impl_key)
            .ok_or_else(|| format!("unknown rust runner: {}", runner.impl_key))?;
        let points = expand_runner_points_for_runner(runner_impl.name(), &runner.params)?;

        let ctx = BenchRunContext {
            benchmark_name: spec.name.clone(),
            runner_name: runner.name.clone(),
            language: language.to_string(),
            seed: 12_345,
            spec_dir: spec_dir.to_path_buf(),
        };
        let mut rows = Vec::new();
        for point in &points {
            rows.push(runner_impl.run_point(point, &ctx)?);
        }

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
        fs::rename(&staging_dir, &artifact_dir).map_err(|e| e.to_string())?;
    }

    Ok(out_root.to_path_buf())
}
