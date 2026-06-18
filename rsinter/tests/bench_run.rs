use rsinter::bench::registry::build_default_rust_runner_registry;
use rsinter::bench::result::read_results_jsonl;
use rsinter::bench::run::run_rust_benchmark;
use rsinter::bench::spec::BenchmarkSpec;
use rsinter::failure::FailureKind;
use std::fs;
use std::path::Path;

fn issue91_surface_spec(extra_params: &str) -> String {
    format!(
        r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rbposd_lsd"
language = "rust"
impl_key = "rbposd"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
{extra_params}

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{{runner}}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#
    )
}

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

    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap();

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
fn rust_benchmark_run_supports_memory_z_input_type() {
    let spec_text = r#"
name = "surface_decoder_memory_z"
version = 1
mode = "independent"

[[runner]]
name = "rmatching_memory_z"
language = "rust"
impl_key = "rmatching"

[runner.params]
input_type = "memory-z"
distance = [3]
rounds = [9]
p = [0.008]
max_shots = 8
max_errors = 8
batch_size = 4

[plot]
title = "Surface Decoder Memory-Z"

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

    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap();
    let data = fs::read(
        artifact_root
            .join("rmatching_memory_z")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].params["input_type"], serde_json::json!("memory-z"));
    assert_eq!(rows[0].params["distance"], serde_json::json!(3));
    assert_eq!(rows[0].params["rounds"], serde_json::json!(9));
    assert_eq!(rows[0].params["p"], serde_json::json!(0.008));
    assert_eq!(rows[0].case_summary["num_dets"], serde_json::json!(72));
    assert_eq!(rows[0].case_summary["num_obs"], serde_json::json!(1));
    assert_eq!(rows[0].status, "ok");
    assert_eq!(rows[0].error, None);
    assert_ne!(rows[0].failure_kind, FailureKind::SolverFailure);
    assert_ne!(rows[0].failure_kind, FailureKind::SamplerError);
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

    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap();
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
    assert_eq!(
        rows[0].params["decoder_impl"],
        serde_json::json!("rmatching")
    );
    assert_eq!(rows[0].params["seed"], serde_json::json!(12_345));
}

#[test]
fn rbposd_benchmark_records_normalized_decoder_params() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rbposd_tuned"
language = "rust"
impl_key = "rbposd"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
bp_iters = 50
early_stop = false
osd_order = 10

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

    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap();
    let data = fs::read(
        artifact_root
            .join("rbposd_tuned")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].params["bp_iters"], serde_json::json!(50));
    assert_eq!(rows[0].params["early_stop"], serde_json::json!(false));
    assert_eq!(rows[0].params["osd_order"], serde_json::json!(10));
    assert_eq!(rows[0].params["bp_algorithm"], serde_json::json!("min_sum"));
    assert_eq!(
        rows[0].params["osd_method"],
        serde_json::json!("combination_sweep")
    );
}

#[test]
fn rbposd_benchmark_rejects_both_bp_iteration_aliases() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rbposd_bad"
language = "rust"
impl_key = "rbposd"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
bp_iters = 50
max_bp_iterations = 60

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

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();

    assert_eq!(
        err,
        "rbposd params must not set both bp_iters and max_bp_iterations"
    );
}

#[test]
fn rbposd_benchmark_rejects_unsupported_bp_algorithm() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rbposd_bad"
language = "rust"
impl_key = "rbposd"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
bp_algorithm = "sum_product"

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

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();

    assert_eq!(
        err,
        "rbposd bp_algorithm must be \"min_sum\", got \"sum_product\""
    );
    assert!(!dir.path().join("rbposd_bad").exists());
}

#[test]
fn rbposd_benchmark_rejects_unsupported_osd_method() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rbposd_bad"
language = "rust"
impl_key = "rbposd"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
osd_method = "unknown_method"

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

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();

    assert_eq!(
        err,
        "rbposd osd_method must be \"combination_sweep\", got \"unknown_method\""
    );
    assert!(!dir.path().join("rbposd_bad").exists());
}

#[test]
fn rbposd_benchmark_rejects_unknown_decoder_param_without_results() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rbposd_bad"
language = "rust"
impl_key = "rbposd"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
bogus = 1

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

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();

    assert_eq!(err, "unknown rbposd runner param: bogus");
    assert!(!dir.path().join("rbposd_bad").exists());
}

#[test]
fn rbposd_runner_rejects_unknown_lsd_param_without_artifacts() {
    let spec_text = issue91_surface_spec("bogus_lsd = 1");
    let spec: BenchmarkSpec = toml::from_str(&spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();

    assert_eq!(err, "unknown rbposd runner param: bogus_lsd");
    assert!(!dir.path().join("rbposd_lsd").exists());
}

#[test]
fn rbposd_runner_rejects_mixed_osd_and_lsd_params_without_artifacts() {
    let spec_text = issue91_surface_spec(
        r#"
osd_order = 10
lsd_order = 1
"#,
    );
    let spec: BenchmarkSpec = toml::from_str(&spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();

    assert_eq!(err, "rbposd params must not mix OSD and LSD decoder params");
    assert!(!dir.path().join("rbposd_lsd").exists());
}

#[test]
fn rbposd_lsd_run_fails_without_silent_osd_fallback_or_artifacts() {
    let spec_text = issue91_surface_spec(
        r#"
lsd_method = "localized_statistics"
lsd_order = 1
"#,
    );
    let spec: BenchmarkSpec = toml::from_str(&spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();

    assert_eq!(
        err,
        "rbposd LSD DEM decoding is not implemented yet; see issue #92"
    );
    assert!(!dir.path().join("rbposd_lsd").exists());
}

#[test]
fn rust_benchmark_preflights_all_runner_params_before_writing_results() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching_ok"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 1
max_errors = 1
batch_size = 1

[[runner]]
name = "rbposd_bad"
language = "rust"
impl_key = "rbposd"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
bogus = 1

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

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();

    assert_eq!(err, "unknown rbposd runner param: bogus");
    assert!(!dir.path().join("rmatching_ok").exists());
    assert!(!dir.path().join("rbposd_bad").exists());
}

#[test]
fn rust_benchmark_preflights_decoder_param_values_before_writing_results() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching_ok"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 1
max_errors = 1
batch_size = 1

[[runner]]
name = "rilpqec_bad"
language = "rust"
impl_key = "rilpqec"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
mip_gap = 1.0

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

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();

    assert_eq!(err, "mip_gap must be in [0, 1)");
    assert!(!dir.path().join("rmatching_ok").exists());
    assert!(!dir.path().join("rilpqec_bad").exists());
}

#[test]
fn rilpqec_benchmark_records_normalized_decoder_params() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rilpqec_tuned"
language = "rust"
impl_key = "rilpqec"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
backend = "highs"
time_limit_s = 5.0
mip_gap = 0.01
threads = 1
verbose = true

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

    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap();
    let data = fs::read(
        artifact_root
            .join("rilpqec_tuned")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].params["backend"], serde_json::json!("highs"));
    assert_eq!(rows[0].params["time_limit_s"], serde_json::json!(5.0));
    assert_eq!(rows[0].params["mip_gap"], serde_json::json!(0.01));
    assert_eq!(rows[0].params["threads"], serde_json::json!(1));
    assert_eq!(rows[0].params["verbose"], serde_json::json!(true));
}

#[cfg(not(feature = "gurobi"))]
#[test]
fn rilpqec_gurobi_without_feature_records_unsupported_failure_kind() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rilpqec_gurobi"
language = "rust"
impl_key = "rilpqec"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 1
max_errors = 1
batch_size = 1
backend = "gurobi"

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

    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap();
    let data = fs::read(
        artifact_root
            .join("rilpqec_gurobi")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "error");
    assert_eq!(rows[0].failure_kind, FailureKind::Unsupported);
    assert!(
        rows[0]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("no ILP backend is available"),
        "row error was: {:?}",
        rows[0].error
    );
}

#[test]
fn rilpqec_benchmark_rejects_invalid_mip_gap() {
    let spec_text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rilpqec_bad"
language = "rust"
impl_key = "rilpqec"

[runner.params]
distance = [3]
rounds = [3]
p = [0.002]
max_shots = 0
max_errors = 5
batch_size = 4
mip_gap = 1.0

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

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();

    assert_eq!(err, "mip_gap must be in [0, 1)");
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

    let err = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();
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

    let artifact_root = run_rust_benchmark(
        &good_spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap();
    let results_path = artifact_root
        .join("mwpm_alias")
        .join("test-run")
        .join("results.jsonl");
    assert!(results_path.exists());

    let err = run_rust_benchmark(
        &bad_spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_err();
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
    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap();
    let artifact_dir = artifact_root.join("rmatching").join("test-run");

    assert!(artifact_dir.join("run_manifest.json").exists());
    assert!(artifact_dir.join("results.jsonl").exists());
    assert!(!staging_dir.exists());
}

#[test]
fn rust_benchmark_run_supports_css_input_type() {
    let spec_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bench/minimal_css_decoder.toml");
    let text = fs::read_to_string(&spec_path).unwrap();
    let spec: BenchmarkSpec = toml::from_str(&text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        spec_path.parent().unwrap(),
    )
    .unwrap();
    let data = fs::read(
        artifact_root
            .join("rmatching")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].params["input_type"], serde_json::json!("css"));
    assert_eq!(rows[0].params["code_id"], serde_json::json!("steane"));
    assert_eq!(rows[0].params["basis"], serde_json::json!("x"));
    assert_eq!(
        rows[0].params["observables"],
        serde_json::json!("../css/steane_logicals_x.json")
    );
    assert_eq!(
        rows[0].params["logical_observable_source"],
        serde_json::json!("explicit")
    );
    assert_eq!(
        rows[0].params["logical_observable_basis"],
        serde_json::json!("x")
    );
    assert_eq!(
        rows[0].params["logical_failure_aggregation"],
        serde_json::json!("any_logical")
    );
    assert_eq!(rows[0].case_summary["num_obs"], serde_json::json!(1));
    assert_eq!(
        rows[0].case_summary["logical_observable_count"],
        serde_json::json!(1)
    );
}

#[test]
fn rust_benchmark_run_reports_css_file_path_context() {
    let spec_text = r#"
name = "css_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
input_type = "css"
code_id = "steane"
hx = "missing_hx.json"
hz = "../css/steane_hz.json"
basis = "x"
rounds = [1]
p = [0.0]
schedule = "greedy"
observables = "../css/steane_logicals_x.json"
max_shots = 8
max_errors = 4
batch_size = 4

[plot]
title = "CSS Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner", "params.code_id"]
label_template = "{runner} {params.code_id}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;

    let spec: BenchmarkSpec = toml::from_str(spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();
    let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bench");

    let err = run_rust_benchmark(&spec, "rust", dir.path(), &registry, &spec_dir).unwrap_err();

    assert!(err.contains("hx"), "error was: {err}");
    assert!(err.contains("missing_hx.json"), "error was: {err}");
}

#[test]
fn rust_benchmark_run_supports_bb72_css_explicit_observables() {
    let spec_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bench/minimal_bb72_css_decoder.toml");
    let text = fs::read_to_string(&spec_path).unwrap();
    let spec: BenchmarkSpec = toml::from_str(&text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        spec_path.parent().unwrap(),
    )
    .unwrap();
    let data = fs::read(
        artifact_root
            .join("rmatching")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].params["input_type"], serde_json::json!("css"));
    assert_eq!(rows[0].params["code_id"], serde_json::json!("bb72"));
    assert_eq!(
        rows[0].params["logical_observable_source"],
        serde_json::json!("explicit")
    );
    assert_eq!(
        rows[0].params["logical_observable_basis"],
        serde_json::json!("x")
    );
    assert_eq!(
        rows[0].params["logical_failure_aggregation"],
        serde_json::json!("any_logical")
    );
    assert_eq!(rows[0].case_summary["num_obs"], serde_json::json!(12));
    assert_eq!(
        rows[0].case_summary["logical_observable_count"],
        serde_json::json!(12)
    );
    assert_eq!(rows[0].status, "ok");
    assert_eq!(rows[0].error, None);
}

#[test]
fn predict_zero_benchmark_runs_bb72_css_negative_control() {
    let spec_text = r#"
name = "bb72_predict_zero"
version = 1
mode = "independent"

[[runner]]
name = "predict-zero-v1"
language = "rust"
impl_key = "predict-zero"

[runner.params]
input_type = "css"
code_id = "bivariate-bicycle-code-m6-n6"
hx = "../css/bb72_hx.json"
hz = "../css/bb72_hz.json"
observables = "../css/bb72_logicals_x.json"
basis = "x"
schedule = "greedy"
rounds = [3]
p = [0.001]
seed = 12345
max_shots = 64
max_errors = 64
batch_size = 64

[plot]
title = "BB72 Predict Zero"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner", "params.code_id"]
label_template = "{runner} {params.code_id}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "linear"
label = "Logical Error Rate"
"#;

    let spec: BenchmarkSpec = toml::from_str(spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();
    let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bench");

    let artifact_root =
        run_rust_benchmark(&spec, "rust", dir.path(), &registry, &spec_dir).unwrap();
    let data = fs::read(
        artifact_root
            .join("predict-zero-v1")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].runner, "predict-zero-v1");
    assert_eq!(
        rows[0].params["decoder_impl"],
        serde_json::json!("predict-zero")
    );
    assert_eq!(rows[0].params["seed"], serde_json::json!(12_345));
    assert_eq!(rows[0].params["input_type"], serde_json::json!("css"));
    assert_eq!(
        rows[0].params["code_id"],
        serde_json::json!("bivariate-bicycle-code-m6-n6")
    );
    assert_eq!(rows[0].case_summary["num_obs"], serde_json::json!(12));
    assert_eq!(rows[0].status, "ok");
    assert_eq!(rows[0].error, None);

    let logical_error_rate = rows[0].metrics["logical_error_rate"];
    assert!(
        (0.35..=0.65).contains(&logical_error_rate),
        "predict-zero control LER was {logical_error_rate}"
    );
}

#[test]
#[ignore = "manual BB72 BP+OSD reference run; intentionally heavier than CI"]
fn manual_bb72_css_bposd_reference_fixture_records_paper_params() {
    let spec_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bench/bb72_css_bposd_reference.toml");
    let text = fs::read_to_string(&spec_path).unwrap();
    let spec: BenchmarkSpec = toml::from_str(&text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        spec_path.parent().unwrap(),
    )
    .unwrap();
    let data = fs::read(
        artifact_root
            .join("rbposd-osd10-reference")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();
    assert_eq!(rows.len(), 2);

    let mut seen_p003 = false;
    let mut seen_p01 = false;
    for row in rows {
        assert_eq!(row.params["decoder_impl"], serde_json::json!("rbposd"));
        assert_eq!(row.params["seed"], serde_json::json!(12_345));
        assert_eq!(row.params["bp_algorithm"], serde_json::json!("min_sum"));
        assert_eq!(row.params["bp_iters"], serde_json::json!(10_000));
        assert_eq!(
            row.params["osd_method"],
            serde_json::json!("combination_sweep")
        );
        assert_eq!(row.params["osd_order"], serde_json::json!(10));
        assert_eq!(
            row.params["logical_observable_source"],
            serde_json::json!("explicit")
        );
        assert_eq!(row.case_summary["num_obs"], serde_json::json!(12));
        assert_eq!(row.status, "ok");
        assert_eq!(row.error, None);

        let p = row.params["p"].as_f64().unwrap();
        if (p - 0.003).abs() < f64::EPSILON {
            seen_p003 = true;
        }
        if (p - 0.01).abs() < f64::EPSILON {
            seen_p01 = true;
        }
    }

    assert!(seen_p003);
    assert!(seen_p01);
}

#[test]
fn rust_benchmark_run_supports_bb72_css_bposd_fixture() {
    let spec_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bench/bb72_css_bposd_decoder.toml");
    let text = fs::read_to_string(&spec_path).unwrap();
    let spec: BenchmarkSpec = toml::from_str(&text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        spec_path.parent().unwrap(),
    )
    .unwrap();

    let rbposd_data = fs::read(
        artifact_root
            .join("rbposd-osd10-v1")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rbposd_rows = read_results_jsonl(&rbposd_data[..]).unwrap();
    assert_eq!(rbposd_rows.len(), 1);
    let rbposd_row = &rbposd_rows[0];
    assert_eq!(rbposd_row.params["input_type"], serde_json::json!("css"));
    assert_eq!(
        rbposd_row.params["code_id"],
        serde_json::json!("bivariate-bicycle-code-m6-n6")
    );
    assert_eq!(
        rbposd_row.params["logical_observable_source"],
        serde_json::json!("explicit")
    );
    assert_eq!(
        rbposd_row.params["decoder_impl"],
        serde_json::json!("rbposd")
    );
    assert_eq!(rbposd_row.params["seed"], serde_json::json!(12_345));
    assert_eq!(
        rbposd_row.params["bp_algorithm"],
        serde_json::json!("min_sum")
    );
    assert_eq!(rbposd_row.params["bp_iters"], serde_json::json!(50));
    assert_eq!(
        rbposd_row.params["osd_method"],
        serde_json::json!("combination_sweep")
    );
    assert_eq!(rbposd_row.params["osd_order"], serde_json::json!(10));
    assert_eq!(rbposd_row.case_summary["num_obs"], serde_json::json!(12));
    assert_eq!(rbposd_row.status, "ok");
    assert_eq!(rbposd_row.error, None);

    let predict_zero_data = fs::read(
        artifact_root
            .join("predict-zero-v1")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let predict_zero_rows = read_results_jsonl(&predict_zero_data[..]).unwrap();
    assert_eq!(predict_zero_rows.len(), 1);
    let predict_zero_row = &predict_zero_rows[0];
    assert_eq!(
        predict_zero_row.params["decoder_impl"],
        serde_json::json!("predict-zero")
    );
    assert_eq!(predict_zero_row.params["seed"], serde_json::json!(12_345));
    assert_eq!(
        predict_zero_row.case_summary["num_obs"],
        serde_json::json!(12)
    );
    assert_eq!(predict_zero_row.status, "ok");
    assert_eq!(predict_zero_row.error, None);
    let logical_error_rate = predict_zero_row.metrics["logical_error_rate"];
    assert!(
        (0.70..=0.80).contains(&logical_error_rate),
        "predict-zero fixture LER was {logical_error_rate}"
    );
}

#[test]
fn rust_benchmark_run_rejects_invalid_css_observables_before_results() {
    let spec_dir = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    let steane_h =
        r#"{"format":"sparse_rows","num_cols":7,"rows":[[0,3,5,6],[1,3,4,6],[2,4,5,6]]}"#;
    fs::write(spec_dir.path().join("hx.json"), steane_h).unwrap();
    fs::write(spec_dir.path().join("hz.json"), steane_h).unwrap();
    fs::write(
        spec_dir.path().join("bad_obs.json"),
        r#"{"format":"sparse_rows","num_cols":7,"rows":[[0]]}"#,
    )
    .unwrap();
    let spec_text = r#"
name = "bad_css_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
input_type = "css"
code_id = "steane"
hx = "hx.json"
hz = "hz.json"
basis = "x"
rounds = [1]
p = [0.0]
schedule = "greedy"
observables = "bad_obs.json"
max_shots = 4
max_errors = 4
batch_size = 4

[plot]
title = "Bad CSS Decoder"

[plot.x]
field = "params.p"
scale = "linear"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "linear"
label = "Logical Error Rate"
"#;
    let spec: BenchmarkSpec = toml::from_str(spec_text).unwrap();
    let registry = build_default_rust_runner_registry();

    let err =
        run_rust_benchmark(&spec, "rust", out_dir.path(), &registry, spec_dir.path()).unwrap_err();

    assert!(
        err.contains("observable 0 is not an X logical"),
        "error was: {err}"
    );
    assert!(
        !out_dir.path().join("rmatching").join("test-run").exists(),
        "invalid observable run must not produce a completed result directory"
    );
}
