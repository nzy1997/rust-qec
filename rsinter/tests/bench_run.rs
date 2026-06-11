use rsinter::bench::registry::build_default_rust_runner_registry;
use rsinter::bench::result::read_results_jsonl;
use rsinter::bench::run::run_rust_benchmark;
use rsinter::bench::spec::BenchmarkSpec;
use std::fs;

#[test]
fn rust_benchmark_run_writes_manifest_and_results_jsonl() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 20
max_errors = 5
batch_size = 4

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;

    let spec: BenchmarkSpec = toml::from_str(spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let artifact_root = run_rust_benchmark(&spec, "rust", dir.path(), &registry).unwrap();

    let artifact_dir = artifact_root.join("rmatching").join("test-run");
    assert!(artifact_dir.join("run_manifest.json").exists());
    assert!(artifact_dir.join("results.jsonl").exists());

    let data = std::fs::read(artifact_dir.join("results.jsonl")).unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].runner, "rmatching");
    assert_eq!(rows[0].language, "rust");
}

#[test]
fn rust_benchmark_results_use_runner_name_not_impl_key() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "mwpm_alias"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 12
max_errors = 5
batch_size = 4

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;

    let spec: BenchmarkSpec = toml::from_str(spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let artifact_root = run_rust_benchmark(&spec, "rust", dir.path(), &registry).unwrap();
    let data = fs::read(
        artifact_root
            .join("mwpm_alias")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].runner, "mwpm_alias");
}

#[test]
fn rust_benchmark_run_rejects_invalid_distance_before_codegen_panic() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [1]
rounds = [3]
p = [0.002]
max_shots = 20
max_errors = 5
batch_size = 4

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;

    let spec: BenchmarkSpec = toml::from_str(spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let err = run_rust_benchmark(&spec, "rust", dir.path(), &registry).unwrap_err();
    assert!(err.contains("distance"));
}

#[test]
fn rust_benchmark_run_does_not_leave_stale_results_when_retry_fails() {
    let good_spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "mwpm_alias"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 12
max_errors = 5
batch_size = 4

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;
    let bad_spec_text = good_spec_text.replace("distance = [3]", "distance = [1]");

    let good_spec: BenchmarkSpec = toml::from_str(good_spec_text).unwrap();
    let bad_spec: BenchmarkSpec = toml::from_str(&bad_spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let artifact_root = run_rust_benchmark(&good_spec, "rust", dir.path(), &registry).unwrap();
    let results_path = artifact_root
        .join("mwpm_alias")
        .join("test-run")
        .join("results.jsonl");
    assert!(results_path.exists());

    let err = run_rust_benchmark(&bad_spec, "rust", dir.path(), &registry).unwrap_err();
    assert!(err.contains("distance"));
    assert!(!results_path.exists());
}

#[test]
fn rust_benchmark_run_clears_stale_staging_dir_before_writing() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 8
max_errors = 2
batch_size = 4

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;

    let spec: BenchmarkSpec = toml::from_str(spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let staging_dir = dir.path().join("rmatching").join("test-run.tmp");
    fs::create_dir_all(&staging_dir).unwrap();
    fs::write(staging_dir.join("stale.txt"), "stale").unwrap();

    let registry = build_default_rust_runner_registry();
    let artifact_root = run_rust_benchmark(&spec, "rust", dir.path(), &registry).unwrap();
    let artifact_dir = artifact_root.join("rmatching").join("test-run");

    assert!(artifact_dir.join("run_manifest.json").exists());
    assert!(artifact_dir.join("results.jsonl").exists());
    assert!(!staging_dir.exists());
}
