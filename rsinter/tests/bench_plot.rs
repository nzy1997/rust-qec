use rsinter::bench::plot::{logical_rate_fit_for_plot, render_benchmark_plot};
use rsinter::bench::result::{BenchmarkResultRow, CaseSummary, MetricMap, PairMapExt, ParamMap};
use rsinter::bench::spec::{BenchmarkSpec, LogicalRateUnit};
use rsinter::failure::FailureKind;
use rsinter::stats::{fit_binomial, shot_error_rate_to_piece_error_rate};

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
fn logical_rate_unit_transforms_best_and_interval_bounds() {
    let row = ok_row_with_metadata(
        "rmatching",
        3,
        Some(10),
        0.002,
        0.01,
        10.0,
        1000.0,
        12.0,
        Some(2),
        None,
    );
    let fit = fit_binomial(1000, 10, 9.0);
    let expected_low = fit.low.unwrap().max(1e-10);
    let expected_best = fit.best.unwrap().max(1e-10);
    let expected_high = fit.high.unwrap().max(1e-10);

    let per_shot = logical_rate_fit_for_plot(&row, LogicalRateUnit::PerShot).unwrap();
    assert_close(per_shot.low, expected_low);
    assert_close(per_shot.best.unwrap(), 0.01);
    assert_close(per_shot.best.unwrap(), expected_best);
    assert_close(per_shot.high, expected_high);

    let per_round = logical_rate_fit_for_plot(&row, LogicalRateUnit::PerRound).unwrap();
    assert_close(
        per_round.low,
        shot_error_rate_to_piece_error_rate(expected_low, 10.0).max(1e-10),
    );
    assert_close(
        per_round.best.unwrap(),
        shot_error_rate_to_piece_error_rate(0.01, 10.0).max(1e-10),
    );
    assert_close(
        per_round.high,
        shot_error_rate_to_piece_error_rate(expected_high, 10.0).max(1e-10),
    );

    let per_observable = logical_rate_fit_for_plot(&row, LogicalRateUnit::PerObservable).unwrap();
    assert_close(
        per_observable.low,
        shot_error_rate_to_piece_error_rate(expected_low, 2.0).max(1e-10),
    );
    assert_close(
        per_observable.best.unwrap(),
        shot_error_rate_to_piece_error_rate(0.01, 2.0).max(1e-10),
    );
    assert_close(
        per_observable.high,
        shot_error_rate_to_piece_error_rate(expected_high, 2.0).max(1e-10),
    );

    let fallback_obs_row = ok_row_with_metadata(
        "rmatching",
        3,
        Some(10),
        0.002,
        0.01,
        10.0,
        1000.0,
        12.0,
        None,
        Some(2),
    );
    let fallback_observable =
        logical_rate_fit_for_plot(&fallback_obs_row, LogicalRateUnit::PerObservable).unwrap();
    assert_close(
        fallback_observable.best.unwrap(),
        per_observable.best.unwrap(),
    );

    let per_round_per_observable =
        logical_rate_fit_for_plot(&row, LogicalRateUnit::PerRoundPerObservable).unwrap();
    assert_close(
        per_round_per_observable.low,
        shot_error_rate_to_piece_error_rate(expected_low, 20.0).max(1e-10),
    );
    assert_close(
        per_round_per_observable.best.unwrap(),
        shot_error_rate_to_piece_error_rate(0.01, 20.0).max(1e-10),
    );
    assert_close(
        per_round_per_observable.high,
        shot_error_rate_to_piece_error_rate(expected_high, 20.0).max(1e-10),
    );

    let zero_error_row = ok_row_with_metadata(
        "zero",
        3,
        Some(10),
        0.002,
        0.0,
        0.0,
        1000.0,
        12.0,
        Some(2),
        None,
    );
    let zero_fit = logical_rate_fit_for_plot(&zero_error_row, LogicalRateUnit::PerRound).unwrap();
    assert!(
        zero_fit.best.is_none(),
        "zero-error best estimate must stay absent after transform"
    );

    let dir = tempfile::tempdir().unwrap();
    for unit in [
        "per_shot",
        "per_round",
        "per_observable",
        "per_round_per_observable",
    ] {
        let spec = spec_with_logical_rate_unit(unit);
        let out = dir.path().join(format!("{unit}.svg"));
        render_benchmark_plot(&spec, std::slice::from_ref(&row), &out).unwrap();
        assert!(std::fs::read_to_string(out).unwrap().contains("<svg"));
    }

    let per_round_spec = spec_with_logical_rate_unit("per_round");
    let missing_rounds_row = ok_row_with_metadata(
        "missing_rounds",
        3,
        None,
        0.002,
        0.01,
        10.0,
        1000.0,
        12.0,
        Some(2),
        None,
    );
    let missing_rounds_err = render_benchmark_plot(
        &per_round_spec,
        &[missing_rounds_row],
        &dir.path().join("missing-rounds.svg"),
    )
    .unwrap_err();
    assert!(missing_rounds_err.contains("logical_rate_unit = \"per_round\""));
    assert!(missing_rounds_err.contains("params.rounds"));

    let zero_rounds_row = ok_row_with_metadata(
        "zero_rounds",
        3,
        Some(0),
        0.002,
        0.01,
        10.0,
        1000.0,
        12.0,
        Some(2),
        None,
    );
    let zero_rounds_err = render_benchmark_plot(
        &per_round_spec,
        &[zero_rounds_row],
        &dir.path().join("zero-rounds.svg"),
    )
    .unwrap_err();
    assert!(zero_rounds_err.contains("logical_rate_unit = \"per_round\""));
    assert!(zero_rounds_err.contains("positive numeric params.rounds"));

    let mut nonnumeric_rounds_row = row.clone();
    nonnumeric_rounds_row
        .params
        .insert("rounds".to_string(), serde_json::json!("ten"));
    let nonnumeric_rounds_err = render_benchmark_plot(
        &per_round_spec,
        &[nonnumeric_rounds_row],
        &dir.path().join("nonnumeric-rounds.svg"),
    )
    .unwrap_err();
    assert!(nonnumeric_rounds_err.contains("logical_rate_unit = \"per_round\""));
    assert!(nonnumeric_rounds_err.contains("positive numeric params.rounds"));

    let per_observable_spec = spec_with_logical_rate_unit("per_observable");
    let missing_observable_row = ok_row_with_metadata(
        "missing_observable",
        3,
        Some(10),
        0.002,
        0.01,
        10.0,
        1000.0,
        12.0,
        None,
        None,
    );
    let missing_observable_err = render_benchmark_plot(
        &per_observable_spec,
        &[missing_observable_row],
        &dir.path().join("missing-observable.svg"),
    )
    .unwrap_err();
    assert!(missing_observable_err.contains("logical_rate_unit = \"per_observable\""));
    assert!(missing_observable_err.contains("case_summary.logical_observable_count"));
    assert!(missing_observable_err.contains("case_summary.num_obs"));

    let zero_observable_row = ok_row_with_metadata(
        "zero_observable",
        3,
        Some(10),
        0.002,
        0.01,
        10.0,
        1000.0,
        12.0,
        Some(0),
        None,
    );
    let zero_observable_err = render_benchmark_plot(
        &per_observable_spec,
        &[zero_observable_row],
        &dir.path().join("zero-observable.svg"),
    )
    .unwrap_err();
    assert!(zero_observable_err.contains("logical_rate_unit = \"per_observable\""));
    assert!(zero_observable_err.contains("positive numeric case_summary.logical_observable_count"));
    assert!(zero_observable_err.contains("case_summary.num_obs"));

    let mut nonnumeric_observable_row = row.clone();
    nonnumeric_observable_row.case_summary.insert(
        "logical_observable_count".to_string(),
        serde_json::json!("two"),
    );
    let nonnumeric_observable_err = render_benchmark_plot(
        &per_observable_spec,
        &[nonnumeric_observable_row],
        &dir.path().join("nonnumeric-observable.svg"),
    )
    .unwrap_err();
    assert!(nonnumeric_observable_err.contains("logical_rate_unit = \"per_observable\""));
    assert!(nonnumeric_observable_err
        .contains("positive numeric case_summary.logical_observable_count"));
    assert!(nonnumeric_observable_err.contains("case_summary.num_obs"));
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

fn spec_with_logical_rate_unit(unit: &str) -> BenchmarkSpec {
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
title = "Surface Decoder"
logical_rate_unit = "{unit}"

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
    ))
    .unwrap()
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

fn ok_row_with_metadata(
    runner: &str,
    distance: u64,
    rounds: Option<u64>,
    p: f64,
    logical_error_rate: f64,
    logical_errors: f64,
    shots_used: f64,
    decode_us_per_shot: f64,
    logical_observable_count: Option<u64>,
    num_obs: Option<u64>,
) -> BenchmarkResultRow {
    let mut row = ok_row(
        runner,
        distance,
        p,
        logical_error_rate,
        logical_errors,
        shots_used,
        decode_us_per_shot,
    );
    if let Some(rounds) = rounds {
        row.params
            .insert("rounds".to_string(), serde_json::json!(rounds));
    }
    if let Some(count) = logical_observable_count {
        row.case_summary.insert(
            "logical_observable_count".to_string(),
            serde_json::json!(count),
        );
    }
    if let Some(count) = num_obs {
        row.case_summary
            .insert("num_obs".to_string(), serde_json::json!(count));
    }
    row
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-12,
        "actual {actual} did not match expected {expected}"
    );
}
