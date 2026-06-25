use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use plotters::coord::Shift;
use plotters::prelude::*;

use crate::bench::result::BenchmarkResultRow;
use crate::bench::spec::{BenchmarkSpec, PanelSpec};
use crate::stats::fit_binomial;

const MAX_LIKELIHOOD_FACTOR: f64 = 9.0;
const BENCH_PANEL_WIDTH: u32 = 800;
const BENCH_CANVAS_HEIGHT: u32 = 600;
const MIN_LOG_Y: f64 = 1e-10;

type NumericGroups = BTreeMap<String, Vec<(f64, f64)>>;
type ErrorRateGroups = BTreeMap<String, Vec<ErrorRatePoint>>;

#[derive(Clone, Copy)]
struct ErrorRatePoint {
    x: f64,
    low: f64,
    best: Option<f64>,
    high: f64,
}

#[derive(Clone, Copy)]
struct SeriesStyle {
    color: RGBAColor,
    pattern: LinePattern,
}

#[derive(Clone, Copy)]
enum LinePattern {
    Solid,
    Dashed { size: u32, spacing: u32 },
}

enum PreparedPanel {
    ErrorRate(ErrorRatePanelData),
    Numeric(NumericPanelData),
}

struct ErrorRatePanelData {
    x_label: String,
    y_label: String,
    x_scale: String,
    y_scale: String,
    x_range: (f64, f64),
    y_range: (f64, f64),
    groups: ErrorRateGroups,
}

struct NumericPanelData {
    x_label: String,
    y_label: String,
    x_scale: String,
    y_scale: String,
    x_range: (f64, f64),
    y_range: (f64, f64),
    groups: NumericGroups,
}

pub fn render_benchmark_plot(
    spec: &BenchmarkSpec,
    rows: &[BenchmarkResultRow],
    out: &Path,
) -> Result<(), String> {
    if spec.plot.panels.is_empty() {
        return Err("plot spec must contain at least one panel".into());
    }

    let ok_rows: Vec<&BenchmarkResultRow> = rows.iter().filter(|row| row.status == "ok").collect();
    if ok_rows.is_empty() {
        return Err("plot requires at least one ok row; no ok rows available".into());
    }

    let series_styles = build_series_styles(spec, &ok_rows);
    let panels = spec
        .plot
        .panels
        .iter()
        .map(|panel| prepare_panel(spec, panel, &ok_rows))
        .collect::<Result<Vec<_>, _>>()?;

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let width = BENCH_PANEL_WIDTH
        .checked_mul(panels.len() as u32)
        .ok_or_else(|| "plot width overflow".to_string())?;
    let ext = out
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("svg")
        .to_ascii_lowercase();

    if ext == "png" {
        let root = BitMapBackend::new(out, (width, BENCH_CANVAS_HEIGHT)).into_drawing_area();
        render_plot_on(root, spec, &panels, &series_styles)?;
    } else {
        let root = SVGBackend::new(out, (width, BENCH_CANVAS_HEIGHT)).into_drawing_area();
        render_plot_on(root, spec, &panels, &series_styles)?;
    }

    Ok(())
}

fn prepare_panel(
    spec: &BenchmarkSpec,
    panel: &PanelSpec,
    rows: &[&BenchmarkResultRow],
) -> Result<PreparedPanel, String> {
    let metric_key = panel
        .metric
        .strip_prefix("metrics.")
        .unwrap_or(panel.metric.as_str());
    if metric_key == "logical_error_rate" {
        return Ok(PreparedPanel::ErrorRate(prepare_error_rate_panel(
            spec, panel, rows,
        )?));
    }
    Ok(PreparedPanel::Numeric(prepare_numeric_panel(
        spec, panel, rows, metric_key,
    )?))
}

fn prepare_error_rate_panel(
    spec: &BenchmarkSpec,
    panel: &PanelSpec,
    rows: &[&BenchmarkResultRow],
) -> Result<ErrorRatePanelData, String> {
    let mut groups: ErrorRateGroups = BTreeMap::new();
    let mut x_values = Vec::new();
    let mut y_values = Vec::new();

    for row in rows {
        let x = resolve_required_numeric_field(row, &spec.plot.x.field)?;
        validate_plot_value(&spec.plot.x.field, x, &spec.plot.x.scale, row)?;

        let shots = required_count_metric(row, "shots_used")?;
        if shots == 0 {
            return Err(format!(
                "shots_used must be positive for {}",
                row_context(row)
            ));
        }
        let errors = required_count_metric(row, "logical_errors")?;
        if errors > shots {
            return Err(format!(
                "logical_errors must be <= shots_used for {}",
                row_context(row)
            ));
        }

        let fit = fit_binomial(shots, errors, MAX_LIKELIHOOD_FACTOR);
        let low = fit.low.unwrap_or(0.0).max(MIN_LOG_Y);
        let best = if errors == 0 {
            None
        } else {
            Some(fit.best.unwrap_or(0.0).max(MIN_LOG_Y))
        };
        let high = fit.high.unwrap_or(0.0).max(MIN_LOG_Y);
        let label = render_series_label(row, spec);

        groups
            .entry(label)
            .or_default()
            .push(ErrorRatePoint { x, low, best, high });
        x_values.push(x);
        y_values.extend([low, high]);
        if let Some(best) = best {
            y_values.push(best);
        }
    }

    for points in groups.values_mut() {
        points.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
    }

    if groups.is_empty() {
        return Err(format!("no plottable points for panel {}", panel.label));
    }

    let x_range = padded_range(&x_values, &spec.plot.x.scale, &spec.plot.x.field)?;
    let y_range = padded_range(&y_values, &panel.scale, &panel.metric)?;
    Ok(ErrorRatePanelData {
        x_label: spec.plot.x.label.clone(),
        y_label: panel.label.clone(),
        x_scale: spec.plot.x.scale.clone(),
        y_scale: panel.scale.clone(),
        x_range,
        y_range,
        groups,
    })
}

fn prepare_numeric_panel(
    spec: &BenchmarkSpec,
    panel: &PanelSpec,
    rows: &[&BenchmarkResultRow],
    metric_key: &str,
) -> Result<NumericPanelData, String> {
    let mut groups: NumericGroups = BTreeMap::new();
    let mut x_values = Vec::new();
    let mut y_values = Vec::new();

    for row in rows {
        let x = resolve_required_numeric_field(row, &spec.plot.x.field)?;
        validate_plot_value(&spec.plot.x.field, x, &spec.plot.x.scale, row)?;

        let y = required_metric(row, metric_key)?;
        validate_plot_value(&format!("metrics.{metric_key}"), y, &panel.scale, row)?;

        let label = render_series_label(row, spec);
        groups.entry(label).or_default().push((x, y));
        x_values.push(x);
        y_values.push(y);
    }

    for points in groups.values_mut() {
        points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }

    if groups.is_empty() {
        return Err(format!("no plottable points for panel {}", panel.label));
    }

    let x_range = padded_range(&x_values, &spec.plot.x.scale, &spec.plot.x.field)?;
    let y_range = padded_range(&y_values, &panel.scale, &panel.metric)?;
    Ok(NumericPanelData {
        x_label: spec.plot.x.label.clone(),
        y_label: panel.label.clone(),
        x_scale: spec.plot.x.scale.clone(),
        y_scale: panel.scale.clone(),
        x_range,
        y_range,
        groups,
    })
}

fn render_plot_on<DB: DrawingBackend>(
    root: DrawingArea<DB, Shift>,
    spec: &BenchmarkSpec,
    panels: &[PreparedPanel],
    series_styles: &BTreeMap<String, SeriesStyle>,
) -> Result<(), String>
where
    DB::ErrorType: 'static,
{
    root.fill(&WHITE).map_err(|e| e.to_string())?;

    let content = if spec.plot.title.trim().is_empty() {
        root.clone()
    } else {
        root.titled(spec.plot.title.as_str(), ("sans-serif", 28))
            .map_err(|e| e.to_string())?
    };
    let areas = content.split_evenly((1, panels.len()));

    for (area, panel) in areas.into_iter().zip(panels.iter()) {
        match panel {
            PreparedPanel::ErrorRate(data) => {
                render_error_rate_panel_on(area, data, series_styles)?
            }
            PreparedPanel::Numeric(data) => render_numeric_panel_on(area, data, series_styles)?,
        }
    }

    root.present().map_err(|e| e.to_string())
}

fn render_error_rate_panel_on<DB: DrawingBackend>(
    area: DrawingArea<DB, Shift>,
    data: &ErrorRatePanelData,
    series_styles: &BTreeMap<String, SeriesStyle>,
) -> Result<(), String>
where
    DB::ErrorType: 'static,
{
    match (data.x_scale.as_str(), data.y_scale.as_str()) {
        ("log", "log") => {
            let mut chart = ChartBuilder::on(&area)
                .margin(30)
                .x_label_area_size(50)
                .y_label_area_size(70)
                .build_cartesian_2d(
                    (data.x_range.0..data.x_range.1).log_scale(),
                    (data.y_range.0..data.y_range.1).log_scale(),
                )
                .map_err(|e| e.to_string())?;
            chart
                .configure_mesh()
                .x_desc(data.x_label.as_str())
                .y_desc(data.y_label.as_str())
                .draw()
                .map_err(|e| e.to_string())?;
            draw_error_rate_series(&mut chart, &data.groups, series_styles)?;
        }
        ("linear", "log") => {
            let mut chart = ChartBuilder::on(&area)
                .margin(30)
                .x_label_area_size(50)
                .y_label_area_size(70)
                .build_cartesian_2d(
                    data.x_range.0..data.x_range.1,
                    (data.y_range.0..data.y_range.1).log_scale(),
                )
                .map_err(|e| e.to_string())?;
            chart
                .configure_mesh()
                .x_desc(data.x_label.as_str())
                .y_desc(data.y_label.as_str())
                .draw()
                .map_err(|e| e.to_string())?;
            draw_error_rate_series(&mut chart, &data.groups, series_styles)?;
        }
        ("log", "linear") => {
            let mut chart = ChartBuilder::on(&area)
                .margin(30)
                .x_label_area_size(50)
                .y_label_area_size(70)
                .build_cartesian_2d(
                    (data.x_range.0..data.x_range.1).log_scale(),
                    data.y_range.0..data.y_range.1,
                )
                .map_err(|e| e.to_string())?;
            chart
                .configure_mesh()
                .x_desc(data.x_label.as_str())
                .y_desc(data.y_label.as_str())
                .draw()
                .map_err(|e| e.to_string())?;
            draw_error_rate_series(&mut chart, &data.groups, series_styles)?;
        }
        ("linear", "linear") => {
            let mut chart = ChartBuilder::on(&area)
                .margin(30)
                .x_label_area_size(50)
                .y_label_area_size(70)
                .build_cartesian_2d(
                    data.x_range.0..data.x_range.1,
                    data.y_range.0..data.y_range.1,
                )
                .map_err(|e| e.to_string())?;
            chart
                .configure_mesh()
                .x_desc(data.x_label.as_str())
                .y_desc(data.y_label.as_str())
                .draw()
                .map_err(|e| e.to_string())?;
            draw_error_rate_series(&mut chart, &data.groups, series_styles)?;
        }
        _ => {
            return Err(format!(
                "unsupported axis scales: x={}, y={}",
                data.x_scale, data.y_scale
            ));
        }
    }

    Ok(())
}

fn render_numeric_panel_on<DB: DrawingBackend>(
    area: DrawingArea<DB, Shift>,
    data: &NumericPanelData,
    series_styles: &BTreeMap<String, SeriesStyle>,
) -> Result<(), String>
where
    DB::ErrorType: 'static,
{
    match (data.x_scale.as_str(), data.y_scale.as_str()) {
        ("log", "log") => {
            let mut chart = ChartBuilder::on(&area)
                .margin(30)
                .x_label_area_size(50)
                .y_label_area_size(70)
                .build_cartesian_2d(
                    (data.x_range.0..data.x_range.1).log_scale(),
                    (data.y_range.0..data.y_range.1).log_scale(),
                )
                .map_err(|e| e.to_string())?;
            chart
                .configure_mesh()
                .x_desc(data.x_label.as_str())
                .y_desc(data.y_label.as_str())
                .draw()
                .map_err(|e| e.to_string())?;
            draw_numeric_series(&mut chart, &data.groups, series_styles)?;
        }
        ("linear", "log") => {
            let mut chart = ChartBuilder::on(&area)
                .margin(30)
                .x_label_area_size(50)
                .y_label_area_size(70)
                .build_cartesian_2d(
                    data.x_range.0..data.x_range.1,
                    (data.y_range.0..data.y_range.1).log_scale(),
                )
                .map_err(|e| e.to_string())?;
            chart
                .configure_mesh()
                .x_desc(data.x_label.as_str())
                .y_desc(data.y_label.as_str())
                .draw()
                .map_err(|e| e.to_string())?;
            draw_numeric_series(&mut chart, &data.groups, series_styles)?;
        }
        ("log", "linear") => {
            let mut chart = ChartBuilder::on(&area)
                .margin(30)
                .x_label_area_size(50)
                .y_label_area_size(70)
                .build_cartesian_2d(
                    (data.x_range.0..data.x_range.1).log_scale(),
                    data.y_range.0..data.y_range.1,
                )
                .map_err(|e| e.to_string())?;
            chart
                .configure_mesh()
                .x_desc(data.x_label.as_str())
                .y_desc(data.y_label.as_str())
                .draw()
                .map_err(|e| e.to_string())?;
            draw_numeric_series(&mut chart, &data.groups, series_styles)?;
        }
        ("linear", "linear") => {
            let mut chart = ChartBuilder::on(&area)
                .margin(30)
                .x_label_area_size(50)
                .y_label_area_size(70)
                .build_cartesian_2d(
                    data.x_range.0..data.x_range.1,
                    data.y_range.0..data.y_range.1,
                )
                .map_err(|e| e.to_string())?;
            chart
                .configure_mesh()
                .x_desc(data.x_label.as_str())
                .y_desc(data.y_label.as_str())
                .draw()
                .map_err(|e| e.to_string())?;
            draw_numeric_series(&mut chart, &data.groups, series_styles)?;
        }
        _ => {
            return Err(format!(
                "unsupported axis scales: x={}, y={}",
                data.x_scale, data.y_scale
            ));
        }
    }

    Ok(())
}

fn draw_error_rate_series<'a, DB, XR, YR>(
    chart: &mut ChartContext<'a, DB, Cartesian2d<XR, YR>>,
    groups: &ErrorRateGroups,
    series_styles: &BTreeMap<String, SeriesStyle>,
) -> Result<(), String>
where
    DB: DrawingBackend + 'a,
    DB::ErrorType: 'static,
    XR: Ranged<ValueType = f64>,
    YR: Ranged<ValueType = f64>,
{
    for (index, (label, points)) in groups.iter().enumerate() {
        let style = series_styles
            .get(label)
            .copied()
            .unwrap_or_else(|| default_series_style(index));

        if points.len() > 1 {
            chart
                .draw_series(std::iter::once(Polygon::new(
                    confidence_band_polygon(points),
                    ShapeStyle::from(&style.color.mix(0.18)).filled(),
                )))
                .map_err(|e| e.to_string())?;
        }

        let best_points: Vec<(f64, f64)> = points
            .iter()
            .filter_map(|point| point.best.map(|best| (point.x, best)))
            .collect();
        if !best_points.is_empty() {
            draw_line_series(chart, label, &best_points, style)?;

            chart
                .draw_series(
                    best_points.iter().copied().map(|point| {
                        Circle::new(point, 4, ShapeStyle::from(&style.color).filled())
                    }),
                )
                .map_err(|e| e.to_string())?;
        }

        if points.len() == 1 {
            const CAP_FACTOR: f64 = 1.015;
            let point = points[0];
            let x = point.x;
            let low = point.low;
            let high = point.high;
            let x_lo = x / CAP_FACTOR;
            let x_hi = x * CAP_FACTOR;
            chart
                .draw_series([
                    PathElement::new(
                        vec![(x, low), (x, high)],
                        ShapeStyle::from(&style.color).stroke_width(1),
                    ),
                    PathElement::new(
                        vec![(x_lo, low), (x_hi, low)],
                        ShapeStyle::from(&style.color).stroke_width(1),
                    ),
                    PathElement::new(
                        vec![(x_lo, high), (x_hi, high)],
                        ShapeStyle::from(&style.color).stroke_width(1),
                    ),
                ])
                .map_err(|e| e.to_string())?;
        }
    }

    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::UpperLeft)
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .draw()
        .map_err(|e| e.to_string())
}

fn draw_numeric_series<'a, DB, XR, YR>(
    chart: &mut ChartContext<'a, DB, Cartesian2d<XR, YR>>,
    groups: &NumericGroups,
    series_styles: &BTreeMap<String, SeriesStyle>,
) -> Result<(), String>
where
    DB: DrawingBackend + 'a,
    DB::ErrorType: 'static,
    XR: Ranged<ValueType = f64>,
    YR: Ranged<ValueType = f64>,
{
    for (index, (label, points)) in groups.iter().enumerate() {
        let style = series_styles
            .get(label)
            .copied()
            .unwrap_or_else(|| default_series_style(index));

        draw_line_series(chart, label, points, style)?;
        chart
            .draw_series(
                points
                    .iter()
                    .copied()
                    .map(|point| Circle::new(point, 4, ShapeStyle::from(&style.color).filled())),
            )
            .map_err(|e| e.to_string())?;
    }

    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::UpperLeft)
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .draw()
        .map_err(|e| e.to_string())
}

fn draw_line_series<'a, DB, XR, YR>(
    chart: &mut ChartContext<'a, DB, Cartesian2d<XR, YR>>,
    label: &str,
    points: &[(f64, f64)],
    style: SeriesStyle,
) -> Result<(), String>
where
    DB: DrawingBackend + 'a,
    DB::ErrorType: 'static,
    XR: Ranged<ValueType = f64>,
    YR: Ranged<ValueType = f64>,
{
    let line_style = ShapeStyle::from(&style.color).stroke_width(2);
    let legend_color = style.color;
    match style.pattern {
        LinePattern::Solid => {
            chart
                .draw_series(LineSeries::new(points.iter().copied(), line_style))
                .map_err(|e| e.to_string())?
                .label(label.to_string())
                .legend(move |(x, y)| {
                    PathElement::new(
                        vec![(x, y), (x + 20, y)],
                        ShapeStyle::from(&legend_color).stroke_width(2),
                    )
                });
        }
        LinePattern::Dashed { size, spacing } => {
            chart
                .draw_series(DashedLineSeries::new(
                    points.iter().copied(),
                    size,
                    spacing,
                    line_style,
                ))
                .map_err(|e| e.to_string())?
                .label(label.to_string())
                .legend(move |(x, y)| {
                    PathElement::new(
                        vec![(x, y), (x + 20, y)],
                        ShapeStyle::from(&legend_color).stroke_width(2),
                    )
                });
        }
    }
    Ok(())
}

fn padded_range(values: &[f64], scale: &str, field_name: &str) -> Result<(f64, f64), String> {
    if values.is_empty() {
        return Err(format!(
            "cannot compute axis range for empty values in {field_name}"
        ));
    }
    if let Some(value) = values.iter().copied().find(|value| !value.is_finite()) {
        return Err(format!("{field_name} must be finite, got {value}"));
    }
    if scale == "log" {
        if let Some(value) = values.iter().copied().find(|value| *value <= 0.0) {
            return Err(format!(
                "log scale requires positive values for {field_name}, got {value}"
            ));
        }
    }

    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if scale == "log" {
        return Ok((min / 1.1, max * 1.1));
    }

    if min == max {
        let pad = if min == 0.0 { 1.0 } else { min.abs() * 0.1 };
        return Ok((min - pad, max + pad));
    }
    let span = max - min;
    Ok((min - span * 0.1, max + span * 0.1))
}

fn validate_plot_value(
    field_name: &str,
    value: f64,
    scale: &str,
    row: &BenchmarkResultRow,
) -> Result<(), String> {
    if !value.is_finite() {
        return Err(format!(
            "{field_name} must be finite for {}, got {value}",
            row_context(row)
        ));
    }
    if scale == "log" && value <= 0.0 {
        return Err(format!(
            "{field_name} must be positive for log scale in {}, got {value}",
            row_context(row)
        ));
    }
    Ok(())
}

fn resolve_required_numeric_field(row: &BenchmarkResultRow, field: &str) -> Result<f64, String> {
    resolve_numeric_field(row, field).ok_or_else(|| {
        format!(
            "missing required plot field {field} for {}",
            row_context(row)
        )
    })
}

fn required_metric(row: &BenchmarkResultRow, key: &str) -> Result<f64, String> {
    row.metrics
        .get(key)
        .copied()
        .ok_or_else(|| format!("missing required metric {key} for {}", row_context(row)))
}

fn required_count_metric(row: &BenchmarkResultRow, key: &str) -> Result<u64, String> {
    let value = required_metric(row, key)?;
    if !value.is_finite() {
        return Err(format!("{key} must be finite for {}", row_context(row)));
    }
    if value < 0.0 {
        return Err(format!(
            "{key} must be non-negative for {}",
            row_context(row)
        ));
    }
    Ok(value.round() as u64)
}

fn resolve_numeric_field(row: &BenchmarkResultRow, field: &str) -> Option<f64> {
    let (scope, key) = field.split_once('.')?;
    match scope {
        "params" => row
            .params
            .get(key)
            .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|n| n as f64))),
        "metrics" => row.metrics.get(key).copied(),
        "case_summary" => row
            .case_summary
            .get(key)
            .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|n| n as f64))),
        _ => None,
    }
}

fn build_series_styles(
    spec: &BenchmarkSpec,
    rows: &[&BenchmarkResultRow],
) -> BTreeMap<String, SeriesStyle> {
    let mut runner_order: Vec<String> = spec
        .runners
        .iter()
        .map(|runner| runner.name.clone())
        .collect();
    let existing_runners: BTreeSet<String> = runner_order.iter().cloned().collect();
    for runner in rows.iter().map(|row| row.runner.clone()) {
        if !existing_runners.contains(&runner) && !runner_order.contains(&runner) {
            runner_order.push(runner);
        }
    }

    let runner_index: BTreeMap<String, usize> = runner_order
        .into_iter()
        .enumerate()
        .map(|(index, runner)| (runner, index))
        .collect();

    let mut distance_values: Vec<String> = rows
        .iter()
        .filter_map(|row| row.params.get("distance").map(value_to_string))
        .collect();
    distance_values.sort_by(|a, b| compare_numeric_strings(a, b));
    distance_values.dedup();
    let distance_index: BTreeMap<String, usize> = distance_values
        .into_iter()
        .enumerate()
        .map(|(index, value)| (value, index))
        .collect();

    let mut styles = BTreeMap::new();
    for (index, row) in rows.iter().enumerate() {
        let label = render_series_label(row, spec);
        styles.entry(label).or_insert_with(|| {
            let color_index = runner_index.get(&row.runner).copied().unwrap_or(index);
            let pattern_index = row
                .params
                .get("distance")
                .map(value_to_string)
                .and_then(|value| distance_index.get(&value).copied())
                .unwrap_or(0);
            SeriesStyle {
                color: Palette99::pick(color_index).mix(0.9),
                pattern: line_pattern_for_index(pattern_index),
            }
        });
    }
    styles
}

fn line_pattern_for_index(index: usize) -> LinePattern {
    match index % 5 {
        0 => LinePattern::Solid,
        1 => LinePattern::Dashed {
            size: 12,
            spacing: 8,
        },
        2 => LinePattern::Dashed {
            size: 6,
            spacing: 6,
        },
        3 => LinePattern::Dashed {
            size: 2,
            spacing: 6,
        },
        _ => LinePattern::Dashed {
            size: 16,
            spacing: 6,
        },
    }
}

fn default_series_style(index: usize) -> SeriesStyle {
    SeriesStyle {
        color: Palette99::pick(index).mix(0.9),
        pattern: LinePattern::Solid,
    }
}

fn confidence_band_polygon(points: &[ErrorRatePoint]) -> Vec<(f64, f64)> {
    let mut polygon = Vec::with_capacity(points.len() * 2);
    polygon.extend(points.iter().map(|point| (point.x, point.high)));
    polygon.extend(points.iter().rev().map(|point| (point.x, point.low)));
    polygon
}

fn render_series_label(row: &BenchmarkResultRow, spec: &BenchmarkSpec) -> String {
    let mut label = spec.plot.series.label_template.clone();
    label = label.replace("{runner}", &row.runner);
    label = label.replace("{language}", &row.language);
    label = replace_value_placeholders(
        label,
        "params",
        row.params.iter().map(|(k, v)| (k.as_str(), v)),
    );
    label = replace_metric_placeholders(label, row.metrics.iter().map(|(k, v)| (k.as_str(), *v)));
    label = replace_value_placeholders(
        label,
        "case_summary",
        row.case_summary.iter().map(|(k, v)| (k.as_str(), v)),
    );
    label
}

fn replace_value_placeholders<'a>(
    mut label: String,
    scope: &str,
    values: impl Iterator<Item = (&'a str, &'a serde_json::Value)>,
) -> String {
    for (key, value) in values {
        label = label.replace(&format!("{{{scope}.{key}}}"), &value_to_string(value));
    }
    label
}

fn replace_metric_placeholders<'a>(
    mut label: String,
    values: impl Iterator<Item = (&'a str, f64)>,
) -> String {
    for (key, value) in values {
        label = label.replace(&format!("{{metrics.{key}}}"), &metric_to_string(value));
    }
    label
}

fn compare_numeric_strings(left: &str, right: &str) -> std::cmp::Ordering {
    match (left.parse::<f64>(), right.parse::<f64>()) {
        (Ok(a), Ok(b)) => a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => left.cmp(right),
    }
}

fn metric_to_string(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

fn row_context(row: &BenchmarkResultRow) -> String {
    let params = row
        .params
        .iter()
        .map(|(key, value)| format!("{key}={}", value_to_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!("runner={} [{}]", row.runner, params)
}

fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}
