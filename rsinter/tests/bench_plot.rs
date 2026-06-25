use rsinter::bench::plot::render_benchmark_plot;
use rsinter::bench::result::{BenchmarkResultRow, CaseSummary, MetricMap, PairMapExt, ParamMap};
use rsinter::bench::spec::{BenchmarkSpec, DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR};
use rsinter::failure::FailureKind;

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
            failure_kind: FailureKind::LogicalFailure,
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
            failure_kind: FailureKind::LogicalFailure,
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
        failure_kind: FailureKind::LogicalFailure,
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
        failure_kind: FailureKind::SolverFailure,
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
        failure_kind: FailureKind::Ok,
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
            failure_kind: FailureKind::LogicalFailure,
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
            failure_kind: FailureKind::LogicalFailure,
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

#[test]
fn render_benchmark_plot_rejects_specs_without_panels() {
    let spec = spec_with_panels(
        "Surface Decoder",
        "params.p",
        "log",
        r#"[plot.series]
group_by = ["runner"]
label_template = "{runner}"
"#,
        "",
    );
    let rows = vec![ok_row("rmatching", 3, 0.002, 0.001, 2.0, 2000.0, 12.0)];

    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("plot.svg");
    let err = render_benchmark_plot(&spec, &rows, &out).unwrap_err();

    assert!(err.contains("at least one panel"));
    assert!(!out.exists());
}

#[test]
fn render_benchmark_plot_writes_png_for_linear_numeric_panel_without_title() {
    let spec = spec_with_panels(
        "",
        "params.p",
        "linear",
        r#"[plot.series]
group_by = ["runner"]
label_template = "{language} {runner} d={params.distance} n={case_summary.num_dets} t={metrics.decode_us_per_shot}"
"#,
        r#"[[plot.panel]]
metric = "metrics.decode_us_per_shot"
scale = "linear"
label = "Decode Time Per Shot"
"#,
    );
    let rows = vec![
        ok_row("rmatching", 3, 0.002, 0.001, 2.0, 2000.0, 12.0),
        ok_row("rmatching", 3, 0.005, 0.01, 20.0, 2000.0, 15.0),
    ];

    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("plot.png");
    render_benchmark_plot(&spec, &rows, &out).unwrap();

    assert!(out.exists());
    assert!(std::fs::metadata(out).unwrap().len() > 0);
}

#[test]
fn render_error_rate_panel_supports_linear_and_log_scale_combinations() {
    for (x_scale, y_scale) in [("linear", "log"), ("log", "linear"), ("linear", "linear")] {
        let spec = spec_with_panels(
            "Surface Decoder",
            "params.p",
            x_scale,
            r#"[plot.series]
group_by = ["runner", "params.distance"]
label_template = "{runner} d={params.distance}"
"#,
            &format!(
                r#"[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "{y_scale}"
label = "Logical Error Rate"
"#
            ),
        );
        let rows = vec![
            ok_row("rmatching", 3, 0.002, 0.001, 2.0, 2000.0, 12.0),
            ok_row("rmatching", 3, 0.005, 0.01, 20.0, 2000.0, 15.0),
        ];

        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join(format!("plot-{x_scale}-{y_scale}.svg"));
        render_benchmark_plot(&spec, &rows, &out).unwrap();

        assert!(std::fs::read_to_string(out).unwrap().contains("<svg"));
    }
}

#[test]
fn render_numeric_panel_supports_mixed_linear_and_log_scale_combinations() {
    for (x_scale, y_scale) in [("linear", "log"), ("log", "linear")] {
        let spec = spec_with_panels(
            "Surface Decoder",
            "params.p",
            x_scale,
            r#"[plot.series]
group_by = ["runner", "params.distance"]
label_template = "{runner} d={params.distance}"
"#,
            &format!(
                r#"[[plot.panel]]
metric = "metrics.decode_us_per_shot"
scale = "{y_scale}"
label = "Decode Time Per Shot"
"#
            ),
        );
        let rows = vec![
            ok_row("rmatching", 3, 0.002, 0.001, 2.0, 2000.0, 12.0),
            ok_row("rmatching", 3, 0.005, 0.01, 20.0, 2000.0, 15.0),
        ];

        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join(format!("plot-{x_scale}-{y_scale}.svg"));
        render_benchmark_plot(&spec, &rows, &out).unwrap();

        assert!(std::fs::read_to_string(out).unwrap().contains("<svg"));
    }
}

#[test]
fn render_benchmark_plot_rejects_invalid_error_rate_counts() {
    let spec = spec_with_panels(
        "Surface Decoder",
        "params.p",
        "log",
        r#"[plot.series]
group_by = ["runner"]
label_template = "{runner}"
"#,
        r#"[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#,
    );

    let dir = tempfile::tempdir().unwrap();
    let zero_shots_out = dir.path().join("zero-shots.svg");
    let zero_shots_err = render_benchmark_plot(
        &spec,
        &[ok_row("rmatching", 3, 0.002, 0.001, 0.0, 0.0, 12.0)],
        &zero_shots_out,
    )
    .unwrap_err();
    assert!(zero_shots_err.contains("shots_used must be positive"));

    let too_many_errors_out = dir.path().join("too-many-errors.svg");
    let too_many_errors_err = render_benchmark_plot(
        &spec,
        &[ok_row("rmatching", 3, 0.002, 0.001, 11.0, 10.0, 12.0)],
        &too_many_errors_out,
    )
    .unwrap_err();
    assert!(too_many_errors_err.contains("logical_errors must be <= shots_used"));
}

#[test]
fn render_benchmark_plot_rejects_missing_and_nonfinite_numeric_fields() {
    let missing_x_spec = spec_with_panels(
        "Surface Decoder",
        "case_summary.missing",
        "linear",
        r#"[plot.series]
group_by = ["runner"]
label_template = "{runner}"
"#,
        r#"[[plot.panel]]
metric = "metrics.decode_us_per_shot"
scale = "linear"
label = "Decode Time Per Shot"
"#,
    );
    let missing_metric_spec = spec_with_panels(
        "Surface Decoder",
        "params.p",
        "linear",
        r#"[plot.series]
group_by = ["runner"]
label_template = "{runner}"
"#,
        r#"[[plot.panel]]
metric = "metrics.missing"
scale = "linear"
label = "Missing Metric"
"#,
    );
    let nonfinite_metric_spec = spec_with_panels(
        "Surface Decoder",
        "params.p",
        "linear",
        r#"[plot.series]
group_by = ["runner"]
label_template = "{runner}"
"#,
        r#"[[plot.panel]]
metric = "metrics.decode_us_per_shot"
scale = "linear"
label = "Decode Time Per Shot"
"#,
    );

    let dir = tempfile::tempdir().unwrap();
    let rows = vec![ok_row("rmatching", 3, 0.002, 0.001, 2.0, 2000.0, 12.0)];
    let missing_x_err =
        render_benchmark_plot(&missing_x_spec, &rows, &dir.path().join("missing-x.svg"))
            .unwrap_err();
    assert!(missing_x_err.contains("missing required plot field case_summary.missing"));

    let missing_metric_err = render_benchmark_plot(
        &missing_metric_spec,
        &rows,
        &dir.path().join("missing-metric.svg"),
    )
    .unwrap_err();
    assert!(missing_metric_err.contains("missing required metric missing"));

    let nonfinite_rows = vec![ok_row(
        "rmatching",
        3,
        0.002,
        0.001,
        2.0,
        2000.0,
        f64::INFINITY,
    )];
    let nonfinite_metric_err = render_benchmark_plot(
        &nonfinite_metric_spec,
        &nonfinite_rows,
        &dir.path().join("nonfinite.svg"),
    )
    .unwrap_err();
    assert!(nonfinite_metric_err.contains("must be finite"));
}

#[test]
fn render_benchmark_plot_rejects_unsupported_axis_scales() {
    let unsupported_error_rate_spec = spec_with_panels(
        "Surface Decoder",
        "params.p",
        "sqrt",
        r#"[plot.series]
group_by = ["runner"]
label_template = "{runner}"
"#,
        r#"[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#,
    );
    let unsupported_numeric_spec = spec_with_panels(
        "Surface Decoder",
        "params.p",
        "linear",
        r#"[plot.series]
group_by = ["runner"]
label_template = "{runner}"
"#,
        r#"[[plot.panel]]
metric = "metrics.decode_us_per_shot"
scale = "sqrt"
label = "Decode Time Per Shot"
"#,
    );
    let rows = vec![ok_row("rmatching", 3, 0.002, 0.001, 2.0, 2000.0, 12.0)];
    let dir = tempfile::tempdir().unwrap();

    let error_rate_err = render_benchmark_plot(
        &unsupported_error_rate_spec,
        &rows,
        &dir.path().join("bad-error-rate.svg"),
    )
    .unwrap_err();
    assert!(error_rate_err.contains("unsupported axis scales"));

    let numeric_err = render_benchmark_plot(
        &unsupported_numeric_spec,
        &rows,
        &dir.path().join("bad-numeric.svg"),
    )
    .unwrap_err();
    assert!(numeric_err.contains("unsupported axis scales"));
}

#[test]
fn render_benchmark_plot_rejects_nonfinite_and_negative_count_metrics() {
    let spec = spec_with_panels(
        "Surface Decoder",
        "params.p",
        "log",
        r#"[plot.series]
group_by = ["runner"]
label_template = "{runner}"
"#,
        r#"[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#,
    );
    let dir = tempfile::tempdir().unwrap();

    let nonfinite_shots_err = render_benchmark_plot(
        &spec,
        &[ok_row(
            "rmatching",
            3,
            0.002,
            0.001,
            2.0,
            f64::INFINITY,
            12.0,
        )],
        &dir.path().join("nonfinite-shots.svg"),
    )
    .unwrap_err();
    assert!(nonfinite_shots_err.contains("shots_used must be finite"));

    let negative_errors_err = render_benchmark_plot(
        &spec,
        &[ok_row("rmatching", 3, 0.002, 0.001, -1.0, 2000.0, 12.0)],
        &dir.path().join("negative-errors.svg"),
    )
    .unwrap_err();
    assert!(negative_errors_err.contains("logical_errors must be non-negative"));
}

#[test]
fn zero_error_logical_rate_uses_interval_without_fake_best_point() {
    let spec = spec_with_panels(
        "Surface Decoder",
        "params.p",
        "log",
        r#"[plot.series]
group_by = ["runner"]
label_template = "{runner}"
"#,
        r#"[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#,
    );

    let zero_error_row = ok_row("zero", 3, 0.002, 0.0, 0.0, 2000.0, 12.0);
    let nonzero_error_row = ok_row("nonzero", 3, 0.004, 0.001, 2.0, 2000.0, 12.0);
    let dir = tempfile::tempdir().unwrap();

    let combined_out = dir.path().join("zero-and-nonzero.svg");
    render_benchmark_plot(
        &spec,
        &[zero_error_row.clone(), nonzero_error_row],
        &combined_out,
    )
    .unwrap();
    let combined_svg = std::fs::read_to_string(combined_out).unwrap();
    let combined_marker_count = combined_svg.matches("<circle").count();
    assert_eq!(
        combined_marker_count, 1,
        "expected only the nonzero row to draw a best marker; svg was:\n{combined_svg}"
    );

    let zero_only_out = dir.path().join("zero-only.svg");
    render_benchmark_plot(&spec, &[zero_error_row], &zero_only_out).unwrap();
    let zero_only_svg = std::fs::read_to_string(zero_only_out).unwrap();
    assert!(zero_only_svg.contains("<svg"));
    let zero_only_interval_polylines = zero_only_svg
        .lines()
        .filter(|line| line.contains("<polyline"))
        .filter(|line| line.contains("fill=\"none\""))
        .filter(|line| line.contains("stroke-width=\"1\""))
        .filter(|line| line.contains("stroke=\"#") && !line.contains("stroke=\"#000000\""))
        .count();
    assert!(
        zero_only_interval_polylines >= 3,
        "zero-error interval should render error-bar primitives; svg was:\n{zero_only_svg}"
    );
    assert_eq!(
        zero_only_svg.matches("<circle").count(),
        0,
        "zero-error interval-only row must not draw a best marker; svg was:\n{zero_only_svg}"
    );
}

#[test]
fn plot_confidence_interval_factor_is_read_from_toml() {
    let default_spec: BenchmarkSpec =
        toml::from_str(&confidence_interval_factor_spec_text("")).unwrap();
    default_spec.validate().unwrap();
    assert_eq!(
        default_spec.plot.confidence_interval_likelihood_factor,
        DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR
    );

    let wide_spec: BenchmarkSpec = toml::from_str(&confidence_interval_factor_spec_text(
        "confidence_interval_likelihood_factor = 25.0",
    ))
    .unwrap();
    wide_spec.validate().unwrap();
    assert_eq!(wide_spec.plot.confidence_interval_likelihood_factor, 25.0);

    let rows = confidence_interval_probe_rows();
    let default_svg = render_plot_svg(&default_spec, &rows, "default-factor.svg");
    let wide_svg = render_plot_svg(&wide_spec, &rows, "wide-factor.svg");
    let default_interval_height = target_interval_pixel_height(&default_svg);
    let wide_interval_height = target_interval_pixel_height(&wide_svg);
    assert!(
        wide_interval_height > default_interval_height,
        "factor 25.0 should produce a wider target interval than the default; \
         default={default_interval_height}, wide={wide_interval_height}\n\
         default svg:\n{default_svg}\nwide svg:\n{wide_svg}"
    );

    for invalid in ["0.0", "nan", "-2.0"] {
        let spec: BenchmarkSpec = toml::from_str(&confidence_interval_factor_spec_text(&format!(
            "confidence_interval_likelihood_factor = {invalid}"
        )))
        .unwrap();
        let err = spec
            .validate()
            .expect_err("invalid confidence interval factor should fail validation");
        assert!(
            err.contains("confidence_interval_likelihood_factor"),
            "error should name the invalid field, got: {err}"
        );
        assert!(
            err.contains("finite") && err.contains(">= 1.0"),
            "error should explain the finite >= 1.0 constraint, got: {err}"
        );
    }
}

#[test]
fn render_benchmark_plot_handles_single_linear_point_and_dashed_distance_series() {
    let single_point_spec = spec_with_panels(
        "Surface Decoder",
        "params.p",
        "linear",
        r#"[plot.series]
group_by = ["runner"]
label_template = "{runner}"
"#,
        r#"[[plot.panel]]
metric = "metrics.decode_us_per_shot"
scale = "linear"
label = "Decode Time Per Shot"
"#,
    );
    let dashed_series_spec = spec_with_panels(
        "Surface Decoder",
        "params.p",
        "linear",
        r#"[plot.series]
group_by = ["runner", "params.distance"]
label_template = "{runner} d={params.distance}"
"#,
        r#"[[plot.panel]]
metric = "metrics.decode_us_per_shot"
scale = "linear"
label = "Decode Time Per Shot"
"#,
    );
    let dir = tempfile::tempdir().unwrap();

    let single_out = dir.path().join("single.svg");
    render_benchmark_plot(
        &single_point_spec,
        &[ok_row("rmatching", 3, 0.002, 0.001, 2.0, 2000.0, 12.0)],
        &single_out,
    )
    .unwrap();
    assert!(
        std::fs::read_to_string(single_out)
            .unwrap()
            .contains("<svg")
    );

    let dashed_rows = vec![
        ok_row("rmatching", 3, 0.002, 0.001, 2.0, 2000.0, 12.0),
        ok_row("rmatching", 3, 0.005, 0.01, 20.0, 2000.0, 15.0),
        ok_row("rmatching", 5, 0.002, 0.0005, 1.0, 2000.0, 20.0),
        ok_row("rmatching", 5, 0.005, 0.008, 16.0, 2000.0, 24.0),
    ];
    let dashed_out = dir.path().join("dashed.svg");
    render_benchmark_plot(&dashed_series_spec, &dashed_rows, &dashed_out).unwrap();
    assert!(
        std::fs::read_to_string(dashed_out)
            .unwrap()
            .contains("rmatching d=5")
    );
}

fn spec_with_panels(
    title: &str,
    x_field: &str,
    x_scale: &str,
    series: &str,
    panels: &str,
) -> BenchmarkSpec {
    toml::from_str(&format!(
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
title = "{title}"

[plot.x]
field = "{x_field}"
scale = "{x_scale}"
label = "Physical Error Rate"

{series}

{panels}
"#
    ))
    .unwrap()
}

fn confidence_interval_factor_spec_text(plot_extra: &str) -> String {
    format!(
        r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "zero-anchor"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
p = [0.001]
rounds = [3]
max_shots = 2000
max_errors = 20
batch_size = 256

[[runner]]
name = "target"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
p = [0.01]
rounds = [3]
max_shots = 1000
max_errors = 10
batch_size = 256

[[runner]]
name = "one-anchor"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
p = [0.1]
rounds = [3]
max_shots = 2000
max_errors = 2000
batch_size = 256

[plot]
title = "Surface Decoder"
{plot_extra}

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

fn confidence_interval_probe_rows() -> Vec<BenchmarkResultRow> {
    vec![
        ok_row("zero-anchor", 3, 0.001, 0.0, 0.0, 2000.0, 12.0),
        ok_row("target", 3, 0.01, 0.01, 10.0, 1000.0, 12.0),
        ok_row("one-anchor", 3, 0.1, 1.0, 2000.0, 2000.0, 12.0),
    ]
}

fn render_plot_svg(spec: &BenchmarkSpec, rows: &[BenchmarkResultRow], file_name: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join(file_name);
    render_benchmark_plot(spec, rows, &out).unwrap();
    std::fs::read_to_string(out).unwrap()
}

#[derive(Debug)]
struct VerticalIntervalSegment {
    x: f64,
    height: f64,
}

fn target_interval_pixel_height(svg: &str) -> f64 {
    let mut segments: Vec<_> = svg
        .lines()
        .filter(|line| line.contains("<polyline"))
        .filter(|line| line.contains("fill=\"none\""))
        .filter(|line| line.contains("stroke-width=\"1\""))
        .filter(|line| line.contains("stroke=\"#") && !line.contains("stroke=\"#000000\""))
        .filter_map(parse_vertical_interval_segment)
        .collect();
    segments.sort_by(|left, right| {
        left.x
            .partial_cmp(&right.x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    assert!(
        segments.len() >= 3,
        "expected at least three single-point interval segments, got {segments:?}\nsvg:\n{svg}"
    );
    segments[segments.len() / 2].height
}

fn parse_vertical_interval_segment(line: &str) -> Option<VerticalIntervalSegment> {
    let points = svg_attribute(line, "points")?;
    let mut points = points.split_whitespace().filter_map(parse_svg_point);
    let first = points.next()?;
    let second = points.next()?;
    if points.next().is_some() {
        return None;
    }
    if (first.0 - second.0).abs() > 1e-6 {
        return None;
    }
    let height = (first.1 - second.1).abs();
    if height <= 0.0 {
        return None;
    }
    Some(VerticalIntervalSegment { x: first.0, height })
}

fn svg_attribute<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}=\"");
    let start = line.find(&prefix)? + prefix.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

fn parse_svg_point(point: &str) -> Option<(f64, f64)> {
    let (x, y) = point.split_once(',')?;
    Some((x.parse().ok()?, y.parse().ok()?))
}

fn ok_row(
    runner: &str,
    distance: u64,
    p: f64,
    logical_error_rate: f64,
    logical_errors: f64,
    shots_used: f64,
    decode_us_per_shot: f64,
) -> BenchmarkResultRow {
    BenchmarkResultRow {
        benchmark: "surface_decoder".into(),
        runner: runner.into(),
        language: "rust".into(),
        status: "ok".into(),
        failure_kind: if logical_errors > 0.0 {
            FailureKind::LogicalFailure
        } else {
            FailureKind::Ok
        },
        params: ParamMap::from_pairs([
            ("distance", serde_json::json!(distance)),
            ("p", serde_json::json!(p)),
        ]),
        case_summary: CaseSummary::from_pairs([("num_dets", serde_json::json!(24))]),
        metrics: MetricMap::from_pairs([
            ("logical_error_rate", logical_error_rate),
            ("decode_us_per_shot", decode_us_per_shot),
            ("shots_used", shots_used),
            ("logical_errors", logical_errors),
        ]),
        artifacts: std::collections::BTreeMap::new(),
        error: None,
    }
}
