# Benchmark Plot Fit Overlays Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add optional `log_log` fit overlays for logical-error-rate benchmark plot series.

**Architecture:** Add a default-disabled `plot.fit` spec section, compute per-series log-log least-squares fits after existing grouping and sorting, then render successful fits as separate dashed overlay lines. Tests exercise parsing, overlay output, zero-error exclusion, and skip behavior through the existing integration test harness.

**Tech Stack:** Rust 2024, serde TOML parsing, plotters SVG backend, rsinter integration tests.

## Global Constraints

- TOML shape: `[plot.fit] enabled = true` and `kind = "log_log"`.
- Omitted `plot.fit` must preserve existing disabled behavior.
- Only `log_log` is in scope.
- `log_log` means unweighted least squares for `ln(y)` versus `ln(x)`.
- Fit each plotted series independently after `plot.series.group_by` is applied.
- Only finite nonzero best points participate in fits.
- Zero-error interval-only rows must not participate in fits as fake floor points.
- Skip fits with fewer than two valid finite nonzero best points; do not panic.
- Draw fit overlays over each series' valid x range with a visually distinct line style and legend entry.
- Do not add a new warning subsystem.
- Required focused verification command: `cargo test -p rsinter --test bench_plot plot_fit_ignores_zero_error_floor_points`.
- Broader requested verification command: `cargo test`.

---

## File Structure

- Modify `rsinter/src/bench/spec.rs`: add `PlotFitSpec`, `PlotFitKind`, defaults, and a `fit` field on `PlotSpec`.
- Modify `rsinter/src/bench/plot.rs`: store optional fit data on internal series data, compute log-log fits from optional best points, include fit endpoints in y ranges, and draw dashed fit overlays.
- Modify `rsinter/tests/bench_spec.rs`: add parsing/default/invalid-kind coverage.
- Modify `rsinter/tests/bench_plot.rs`: add `plot_fit_ignores_zero_error_floor_points` integration coverage.

### Task 1: Fit Spec Parsing

**Files:**
- Modify: `rsinter/src/bench/spec.rs`
- Modify: `rsinter/tests/bench_spec.rs`

**Interfaces:**
- Consumes: existing `PlotSpec` TOML parsing.
- Produces: `PlotSpec.fit: PlotFitSpec`, `PlotFitSpec { enabled: bool, kind: PlotFitKind }`, and `PlotFitKind::LogLog`.

- [ ] **Step 1: Write spec tests first**

Add `PlotFitKind` to the import list in `rsinter/tests/bench_spec.rs`:

```rust
use rsinter::bench::spec::{
    AxisSpec, BenchmarkMode, BenchmarkSpec, LogicalRateUnit, PanelSpec, PlotFitKind, PlotSpec,
    SeriesSpec,
};
```

Add these tests after `benchmark_spec_allows_omitted_plot_series_group_by`:

```rust
#[test]
fn benchmark_spec_defaults_plot_fit_to_disabled_log_log() {
    let text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
p = [0.002]
rounds = [3]
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
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;

    let spec: BenchmarkSpec = toml::from_str(text).unwrap();
    assert!(!spec.plot.fit.enabled);
    assert_eq!(spec.plot.fit.kind, PlotFitKind::LogLog);
}

#[test]
fn benchmark_spec_parses_enabled_log_log_plot_fit() {
    let text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
p = [0.002]
rounds = [3]
max_shots = 2000
max_errors = 20
batch_size = 256

[plot]
title = "Surface Decoder"

[plot.fit]
enabled = true
kind = "log_log"

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

    let spec: BenchmarkSpec = toml::from_str(text).unwrap();
    assert!(spec.plot.fit.enabled);
    assert_eq!(spec.plot.fit.kind, PlotFitKind::LogLog);
}

#[test]
fn benchmark_spec_rejects_unsupported_plot_fit_kind() {
    let text = r#"
name = "surface_decoder"
version = 1
mode = "independent"

[[runner]]
name = "rmatching"
language = "rust"
impl_key = "rmatching"

[runner.params]
distance = [3]
p = [0.002]
rounds = [3]
max_shots = 2000
max_errors = 20
batch_size = 256

[plot]
title = "Surface Decoder"

[plot.fit]
enabled = true
kind = "linear"

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

    let err = toml::from_str::<BenchmarkSpec>(text).unwrap_err();
    assert!(err.to_string().contains("linear"));
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p rsinter --test bench_spec benchmark_spec_defaults_plot_fit_to_disabled_log_log
```

Expected: compile failure because `PlotSpec.fit` and `PlotFitKind` do not exist yet.

- [ ] **Step 3: Add the fit spec types**

In `rsinter/src/bench/spec.rs`, add `fit` to `PlotSpec`:

```rust
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PlotSpec {
    pub title: String,
    #[serde(default)]
    pub logical_rate_unit: LogicalRateUnit,
    #[serde(default)]
    pub fit: PlotFitSpec,
    pub x: AxisSpec,
    pub series: SeriesSpec,
    #[serde(default, rename = "panel")]
    pub panels: Vec<PanelSpec>,
}
```

Add these types after `LogicalRateUnit`:

```rust
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PlotFitSpec {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub kind: PlotFitKind,
}

impl Default for PlotFitSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: PlotFitKind::LogLog,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlotFitKind {
    LogLog,
}

impl Default for PlotFitKind {
    fn default() -> Self {
        Self::LogLog
    }
}
```

Update the literal `PlotSpec` in `benchmark_spec_rejects_missing_runners` to include:

```rust
fit: Default::default(),
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
cargo test -p rsinter --test bench_spec benchmark_spec_defaults_plot_fit_to_disabled_log_log
cargo test -p rsinter --test bench_spec benchmark_spec_parses_enabled_log_log_plot_fit
cargo test -p rsinter --test bench_spec benchmark_spec_rejects_unsupported_plot_fit_kind
```

Expected: all three tests pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add rsinter/src/bench/spec.rs rsinter/tests/bench_spec.rs
git commit -m "feat: parse benchmark plot fit config"
```

### Task 2: Log-Log Fit Computation And Rendering

**Files:**
- Modify: `rsinter/src/bench/plot.rs`
- Modify: `rsinter/tests/bench_plot.rs`

**Interfaces:**
- Consumes: `BenchmarkSpec.plot.fit`, existing `ErrorRatePoint { x, low, best, high }`, grouped `SeriesData<ErrorRatePoint>`, and `render_benchmark_plot`.
- Produces: `LogLogFitForPlot`, `log_log_fit_for_plot(points: &[(f64, Option<f64>)]) -> Option<LogLogFitForPlot>`, optional internal fit overlays, and SVG fit legend entries.

- [ ] **Step 1: Write the failing plot regression**

Update the import in `rsinter/tests/bench_plot.rs`:

```rust
use rsinter::bench::plot::{log_log_fit_for_plot, logical_rate_fit_for_plot, render_benchmark_plot};
```

Add this test before `plot_series_group_by_is_independent_from_label`:

```rust
#[test]
fn plot_fit_ignores_zero_error_floor_points() {
    let spec = spec_with_panels(
        "Surface Decoder",
        "params.p",
        "log",
        r#"[plot.fit]
enabled = true
kind = "log_log"

[plot.series]
group_by = ["runner"]
label_template = "{runner}"
"#,
        r#"[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#,
    );
    let nonzero_rows = vec![
        ok_row("rmatching", 3, 0.001, 0.001, 1.0, 1000.0, 12.0),
        ok_row("rmatching", 3, 0.002, 0.004, 4.0, 1000.0, 12.0),
        ok_row("rmatching", 3, 0.004, 0.016, 16.0, 1000.0, 12.0),
    ];

    let dir = tempfile::tempdir().unwrap();
    let fit_out = dir.path().join("fit.svg");
    render_benchmark_plot(&spec, &nonzero_rows, &fit_out).unwrap();
    let fit_svg = std::fs::read_to_string(fit_out).unwrap();
    assert!(
        fit_svg.contains("rmatching fit"),
        "three finite nonzero points should draw a fit overlay legend entry; svg was:\n{fit_svg}"
    );

    let nonzero_points = fit_points_from_rows(&nonzero_rows);
    let nonzero_fit = log_log_fit_for_plot(&nonzero_points).unwrap();
    assert_close(nonzero_fit.slope, 2.0);

    let zero_row = ok_row("rmatching", 3, 0.0025, 0.0, 0.0, 1000.0, 12.0);
    let mut rows_with_zero = nonzero_rows.clone();
    rows_with_zero.push(zero_row.clone());
    let rows_with_zero_points = fit_points_from_rows(&rows_with_zero);
    let fit_with_zero = log_log_fit_for_plot(&rows_with_zero_points).unwrap();
    assert_close(fit_with_zero.slope, nonzero_fit.slope);

    let mut fake_floor_points = nonzero_points.clone();
    fake_floor_points.push((0.0025, Some(1e-10)));
    let fake_floor_fit = log_log_fit_for_plot(&fake_floor_points).unwrap();
    assert!(
        (fake_floor_fit.slope - nonzero_fit.slope).abs() > 0.1,
        "a fake floor point should materially change the slope"
    );

    let skip_out = dir.path().join("skip.svg");
    render_benchmark_plot(&spec, &[nonzero_rows[0].clone(), zero_row], &skip_out).unwrap();
    let skip_svg = std::fs::read_to_string(skip_out).unwrap();
    assert!(
        !skip_svg.contains("rmatching fit"),
        "fewer than two finite nonzero best points should skip the fit; svg was:\n{skip_svg}"
    );
}
```

Add this helper near the other helpers:

```rust
fn fit_points_from_rows(rows: &[BenchmarkResultRow]) -> Vec<(f64, Option<f64>)> {
    rows.iter()
        .map(|row| {
            let x = row.params.get("p").and_then(|value| value.as_f64()).unwrap();
            let fit = logical_rate_fit_for_plot(row, LogicalRateUnit::PerShot).unwrap();
            (x, fit.best)
        })
        .collect()
}
```

- [ ] **Step 2: Run test to verify RED**

Run:

```bash
cargo test -p rsinter --test bench_plot plot_fit_ignores_zero_error_floor_points
```

Expected: compile failure because `log_log_fit_for_plot` does not exist yet, and/or TOML parsing fails before Task 1 is complete.

- [ ] **Step 3: Add fit storage and public hidden fit result**

In `rsinter/src/bench/plot.rs`, update imports:

```rust
use crate::bench::spec::{BenchmarkSpec, LogicalRateUnit, PanelSpec, PlotFitKind};
```

Change `SeriesData<T>` to:

```rust
struct SeriesData<T> {
    label: String,
    points: Vec<T>,
    fit: Option<LogLogFitForPlot>,
}
```

Add this public hidden result type near `LogicalRateFitForPlot`:

```rust
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogLogFitForPlot {
    pub slope: f64,
    pub intercept: f64,
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}
```

Every `SeriesData` construction must set `fit: None`.

- [ ] **Step 4: Implement log-log least squares**

Add this helper after `logical_rate_fit_for_plot`:

```rust
#[doc(hidden)]
pub fn log_log_fit_for_plot(points: &[(f64, Option<f64>)]) -> Option<LogLogFitForPlot> {
    let valid_points: Vec<(f64, f64)> = points
        .iter()
        .filter_map(|(x, y)| {
            let y = (*y)?;
            if x.is_finite() && y.is_finite() && *x > 0.0 && y > 0.0 {
                Some((*x, y))
            } else {
                None
            }
        })
        .collect();
    if valid_points.len() < 2 {
        return None;
    }

    let n = valid_points.len() as f64;
    let sum_x = valid_points.iter().map(|(x, _)| x.ln()).sum::<f64>();
    let sum_y = valid_points.iter().map(|(_, y)| y.ln()).sum::<f64>();
    let sum_xx = valid_points.iter().map(|(x, _)| x.ln() * x.ln()).sum::<f64>();
    let sum_xy = valid_points.iter().map(|(x, y)| x.ln() * y.ln()).sum::<f64>();
    let denominator = n * sum_xx - sum_x * sum_x;
    if !denominator.is_finite() || denominator == 0.0 {
        return None;
    }

    let slope = (n * sum_xy - sum_x * sum_y) / denominator;
    let intercept = (sum_y - slope * sum_x) / n;
    if !slope.is_finite() || !intercept.is_finite() {
        return None;
    }

    let x_min = valid_points.iter().map(|(x, _)| *x).fold(f64::INFINITY, f64::min);
    let x_max = valid_points
        .iter()
        .map(|(x, _)| *x)
        .fold(f64::NEG_INFINITY, f64::max);
    if !x_min.is_finite() || !x_max.is_finite() || x_min <= 0.0 || x_max <= 0.0 || x_min == x_max {
        return None;
    }
    let y_min = (intercept + slope * x_min.ln()).exp();
    let y_max = (intercept + slope * x_max.ln()).exp();
    if !y_min.is_finite() || !y_max.is_finite() || y_min <= 0.0 || y_max <= 0.0 {
        return None;
    }

    Some(LogLogFitForPlot {
        slope,
        intercept,
        x_min,
        x_max,
        y_min,
        y_max,
    })
}
```

- [ ] **Step 5: Compute fits after grouping**

In `prepare_error_rate_panel`, after sorting each series, add fit computation:

```rust
let fit_enabled = spec.plot.fit.enabled
    && spec.plot.fit.kind == PlotFitKind::LogLog
    && spec.plot.x.scale == "log"
    && panel.scale == "log";
for series in groups.values_mut() {
    series
        .points
        .sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
    if fit_enabled {
        let fit_points: Vec<(f64, Option<f64>)> = series
            .points
            .iter()
            .map(|point| (point.x, point.best))
            .collect();
        series.fit = log_log_fit_for_plot(&fit_points);
        if let Some(fit) = series.fit {
            y_values.extend([fit.y_min, fit.y_max]);
        }
    }
}
```

- [ ] **Step 6: Draw dashed fit overlays**

Add this helper after `draw_line_series`:

```rust
fn draw_fit_line_series<'a, DB, XR, YR>(
    chart: &mut ChartContext<'a, DB, Cartesian2d<XR, YR>>,
    label: &str,
    fit: LogLogFitForPlot,
    style: SeriesStyle,
) -> Result<(), String>
where
    DB: DrawingBackend + 'a,
    DB::ErrorType: 'static,
    XR: Ranged<ValueType = f64>,
    YR: Ranged<ValueType = f64>,
{
    let line_color = style.color.mix(0.55);
    let line_style = ShapeStyle::from(&line_color).stroke_width(2);
    let legend_color = line_color;
    chart
        .draw_series(DashedLineSeries::new(
            [(fit.x_min, fit.y_min), (fit.x_max, fit.y_max)],
            4,
            4,
            line_style,
        ))
        .map_err(|e| e.to_string())?
        .label(format!("{label} fit"))
        .legend(move |(x, y)| {
            PathElement::new(
                vec![(x, y), (x + 20, y)],
                ShapeStyle::from(&legend_color).stroke_width(2),
            )
        });
    Ok(())
}
```

In `draw_error_rate_series`, after drawing measured best markers, add:

```rust
if let Some(fit) = series.fit {
    draw_fit_line_series(chart, label, fit, style)?;
}
```

- [ ] **Step 7: Run focused tests to verify GREEN**

Run:

```bash
cargo test -p rsinter --test bench_plot plot_fit_ignores_zero_error_floor_points
cargo test -p rsinter --test bench_plot zero_error_logical_rate_uses_interval_without_fake_best_point
cargo test -p rsinter --test bench_plot plot_series_group_by_is_independent_from_label
```

Expected: all focused tests pass.

- [ ] **Step 8: Commit**

Run:

```bash
git add rsinter/src/bench/plot.rs rsinter/tests/bench_plot.rs
git commit -m "feat: add benchmark plot fit overlays"
```
