use rsinter::bench::plot::render_benchmark_plot;
use rsinter::bench::result::{BenchmarkResultRow, CaseSummary, MetricMap, PairMapExt, ParamMap};
use rsinter::bench::spec::BenchmarkSpec;

#[test]
fn render_benchmark_plot_writes_svg_for_ok_rows() {
    let spec: BenchmarkSpec = toml::from_str(
        r#"
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
max_shots = 2000
max_errors = 20
batch_size = 256

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner", "params.distance"]
label_template = "{runner} d={params.distance}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#,
    )
    .unwrap();

    let rows = vec![
        BenchmarkResultRow {
            benchmark: "surface_decoder".into(),
            runner: "rmatching".into(),
            language: "rust".into(),
            status: "ok".into(),
            params: ParamMap::from_pairs([
                ("distance", serde_json::json!(3)),
                ("p", serde_json::json!(0.002)),
            ]),
            case_summary: CaseSummary::new(),
            metrics: MetricMap::from_pairs([
                ("logical_error_rate", 0.001),
                ("shots_used", 2000.0),
                ("logical_errors", 2.0),
            ]),
            artifacts: std::collections::BTreeMap::new(),
            error: None,
        },
        BenchmarkResultRow {
            benchmark: "surface_decoder".into(),
            runner: "rmatching".into(),
            language: "rust".into(),
            status: "ok".into(),
            params: ParamMap::from_pairs([
                ("distance", serde_json::json!(3)),
                ("p", serde_json::json!(0.005)),
            ]),
            case_summary: CaseSummary::new(),
            metrics: MetricMap::from_pairs([
                ("logical_error_rate", 0.01),
                ("shots_used", 2000.0),
                ("logical_errors", 20.0),
            ]),
            artifacts: std::collections::BTreeMap::new(),
            error: None,
        },
    ];

    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("plot.svg");
    render_benchmark_plot(&spec, &rows, &out).unwrap();
    assert!(std::fs::read_to_string(out).unwrap().contains("<svg"));
}

#[test]
fn render_benchmark_plot_writes_single_svg_for_multiple_panels() {
    let spec: BenchmarkSpec = toml::from_str(
        r#"
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
max_shots = 2000
max_errors = 20
batch_size = 256

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner", "params.distance"]
label_template = "{runner} d={params.distance}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"

[[plot.panel]]
metric = "metrics.decode_us_per_shot"
scale = "log"
label = "Decode Time Per Shot"
"#,
    )
    .unwrap();

    let rows = vec![BenchmarkResultRow {
        benchmark: "surface_decoder".into(),
        runner: "rmatching".into(),
        language: "rust".into(),
        status: "ok".into(),
        params: ParamMap::from_pairs([
            ("distance", serde_json::json!(3)),
            ("p", serde_json::json!(0.002)),
        ]),
        case_summary: CaseSummary::new(),
        metrics: MetricMap::from_pairs([
            ("logical_error_rate", 0.001),
            ("decode_us_per_shot", 12.0),
            ("shots_used", 2000.0),
            ("logical_errors", 2.0),
        ]),
        artifacts: std::collections::BTreeMap::new(),
        error: None,
    }];

    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("plot.svg");
    render_benchmark_plot(&spec, &rows, &out).unwrap();
    let svg = std::fs::read_to_string(out).unwrap();
    assert!(svg.contains("<svg"));
}

#[test]
fn render_benchmark_plot_rejects_rows_without_any_ok_status() {
    let spec: BenchmarkSpec = toml::from_str(
        r#"
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
max_shots = 2000
max_errors = 20
batch_size = 256

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner", "params.distance"]
label_template = "{runner} d={params.distance}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#,
    )
    .unwrap();

    let rows = vec![BenchmarkResultRow {
        benchmark: "surface_decoder".into(),
        runner: "rmatching".into(),
        language: "rust".into(),
        status: "error".into(),
        params: ParamMap::from_pairs([
            ("distance", serde_json::json!(3)),
            ("p", serde_json::json!(0.002)),
        ]),
        case_summary: CaseSummary::new(),
        metrics: MetricMap::from_pairs([("logical_error_rate", 0.0)]),
        artifacts: std::collections::BTreeMap::new(),
        error: Some("decode failed".into()),
    }];

    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("plot.svg");
    let err = render_benchmark_plot(&spec, &rows, &out).unwrap_err();
    assert!(err.contains("no ok rows"));
    assert!(!out.exists());
}

#[test]
fn render_benchmark_plot_rejects_missing_error_rate_inputs() {
    let spec: BenchmarkSpec = toml::from_str(
        r#"
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
max_shots = 2000
max_errors = 20
batch_size = 256

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner", "params.distance"]
label_template = "{runner} d={params.distance}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#,
    )
    .unwrap();

    let rows = vec![BenchmarkResultRow {
        benchmark: "surface_decoder".into(),
        runner: "rmatching".into(),
        language: "rust".into(),
        status: "ok".into(),
        params: ParamMap::from_pairs([
            ("distance", serde_json::json!(3)),
            ("p", serde_json::json!(0.002)),
        ]),
        case_summary: CaseSummary::new(),
        metrics: MetricMap::from_pairs([("logical_error_rate", 0.001)]),
        artifacts: std::collections::BTreeMap::new(),
        error: None,
    }];

    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("plot.svg");
    let err = render_benchmark_plot(&spec, &rows, &out).unwrap_err();
    assert!(err.contains("shots_used"));
}

#[test]
fn render_benchmark_plot_rejects_zero_value_on_log_numeric_panel() {
    let spec: BenchmarkSpec = toml::from_str(
        r#"
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
max_shots = 2000
max_errors = 20
batch_size = 256

[plot]
title = "Surface Decoder"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner", "params.distance"]
label_template = "{runner} d={params.distance}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"

[[plot.panel]]
metric = "metrics.decode_us_per_shot"
scale = "log"
label = "Decode Time Per Shot"
"#,
    )
    .unwrap();

    let rows = vec![
        BenchmarkResultRow {
            benchmark: "surface_decoder".into(),
            runner: "rmatching".into(),
            language: "rust".into(),
            status: "ok".into(),
            params: ParamMap::from_pairs([
                ("distance", serde_json::json!(3)),
                ("p", serde_json::json!(0.002)),
            ]),
            case_summary: CaseSummary::new(),
            metrics: MetricMap::from_pairs([
                ("logical_error_rate", 0.001),
                ("decode_us_per_shot", 0.0),
                ("shots_used", 2000.0),
                ("logical_errors", 2.0),
            ]),
            artifacts: std::collections::BTreeMap::new(),
            error: None,
        },
        BenchmarkResultRow {
            benchmark: "surface_decoder".into(),
            runner: "rmatching".into(),
            language: "rust".into(),
            status: "ok".into(),
            params: ParamMap::from_pairs([
                ("distance", serde_json::json!(3)),
                ("p", serde_json::json!(0.005)),
            ]),
            case_summary: CaseSummary::new(),
            metrics: MetricMap::from_pairs([
                ("logical_error_rate", 0.01),
                ("decode_us_per_shot", 15.0),
                ("shots_used", 2000.0),
                ("logical_errors", 20.0),
            ]),
            artifacts: std::collections::BTreeMap::new(),
            error: None,
        },
    ];

    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("plot.svg");
    let err = render_benchmark_plot(&spec, &rows, &out).unwrap_err();
    assert!(err.contains("positive"));
}
