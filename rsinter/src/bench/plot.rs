#[cfg(feature = "plotting")]
pub use enabled::{
    LogLogFitForPlot, LogicalRateFitForPlot, log_log_fit_for_plot, logical_rate_fit_for_plot,
    render_benchmark_plot,
};

#[cfg(not(feature = "plotting"))]
use std::path::Path;

#[cfg(not(feature = "plotting"))]
use crate::bench::result::BenchmarkResultRow;
#[cfg(not(feature = "plotting"))]
use crate::bench::spec::{BenchmarkSpec, LogicalRateUnit};

#[cfg(not(feature = "plotting"))]
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalRateFitForPlot {
    pub low: f64,
    pub best: Option<f64>,
    pub high: f64,
}

#[cfg(not(feature = "plotting"))]
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

#[cfg(not(feature = "plotting"))]
pub fn render_benchmark_plot(
    _spec: &BenchmarkSpec,
    _rows: &[BenchmarkResultRow],
    _out: &Path,
) -> Result<(), String> {
    Err("requires Cargo feature 'plotting'".into())
}

#[cfg(not(feature = "plotting"))]
#[doc(hidden)]
pub fn logical_rate_fit_for_plot(
    _row: &BenchmarkResultRow,
    _unit: LogicalRateUnit,
) -> Result<LogicalRateFitForPlot, String> {
    Err("requires Cargo feature 'plotting'".into())
}

#[cfg(not(feature = "plotting"))]
#[doc(hidden)]
pub fn log_log_fit_for_plot(_points: &[(f64, Option<f64>)]) -> Option<LogLogFitForPlot> {
    None
}

#[cfg(feature = "plotting")]
mod enabled {
use std::collections::BTreeMap;
use std::path::Path;

use plotters::coord::Shift;
use plotters::element::DashedPathElement;
use plotters::prelude::*;
use plotters::style::text_anchor::{HPos, Pos, VPos};

use crate::bench::result::BenchmarkResultRow;
use crate::bench::spec::{
    BenchmarkSpec, LogicalRateUnit, PanelSpec, PlotFitKind,
    DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR,
};
use crate::stats::{fit_binomial, shot_error_rate_to_piece_error_rate};

const BENCH_PANEL_WIDTH: u32 = 800;
const BENCH_CANVAS_HEIGHT: u32 = 600;
const MIN_LOG_Y: f64 = 1e-10;

type SeriesKey = Vec<String>;
type NumericGroups = BTreeMap<SeriesKey, SeriesData<(f64, f64)>>;
type ErrorRateGroups = BTreeMap<SeriesKey, SeriesData<ErrorRatePoint>>;

struct SeriesData<T> {
    label: String,
    points: Vec<T>,
    fit: Option<LogLogFitForPlot>,
}

#[derive(Clone, Copy)]
struct ErrorRatePoint {
    x: f64,
    low: f64,
    best: Option<f64>,
    high: f64,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalRateFitForPlot {
    pub low: f64,
    pub best: Option<f64>,
    pub high: f64,
}

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
    x_ticks: Vec<f64>,
    groups: ErrorRateGroups,
}

struct NumericPanelData {
    x_label: String,
    y_label: String,
    x_scale: String,
    y_scale: String,
    x_range: (f64, f64),
    y_range: (f64, f64),
    x_ticks: Vec<f64>,
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

    let series_styles = build_series_styles(spec, &ok_rows)?;
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

#[doc(hidden)]
pub fn logical_rate_fit_for_plot(
    row: &BenchmarkResultRow,
    unit: LogicalRateUnit,
) -> Result<LogicalRateFitForPlot, String> {
    logical_rate_fit_for_plot_with_factor(row, unit, DEFAULT_CONFIDENCE_INTERVAL_LIKELIHOOD_FACTOR)
}

fn logical_rate_fit_for_plot_with_factor(
    row: &BenchmarkResultRow,
    unit: LogicalRateUnit,
    confidence_interval_likelihood_factor: f64,
) -> Result<LogicalRateFitForPlot, String> {
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

    let pieces = logical_rate_pieces(row, unit)?;
    let fit = fit_binomial(shots, errors, confidence_interval_likelihood_factor);
    let low = transform_logical_rate(fit.low.unwrap_or(0.0), pieces).max(MIN_LOG_Y);
    let best = if errors == 0 {
        None
    } else {
        Some(transform_logical_rate(fit.best.unwrap_or(0.0), pieces).max(MIN_LOG_Y))
    };
    let high = transform_logical_rate(fit.high.unwrap_or(0.0), pieces).max(MIN_LOG_Y);

    Ok(LogicalRateFitForPlot { low, best, high })
}

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
    let sum_xx = valid_points
        .iter()
        .map(|(x, _)| x.ln() * x.ln())
        .sum::<f64>();
    let sum_xy = valid_points
        .iter()
        .map(|(x, y)| x.ln() * y.ln())
        .sum::<f64>();
    let denominator = n * sum_xx - sum_x * sum_x;
    if !denominator.is_finite() || denominator == 0.0 {
        return None;
    }

    let slope = (n * sum_xy - sum_x * sum_y) / denominator;
    let intercept = (sum_y - slope * sum_x) / n;

    let x_min = valid_points
        .iter()
        .map(|(x, _)| *x)
        .fold(f64::INFINITY, f64::min);
    let x_max = valid_points
        .iter()
        .map(|(x, _)| *x)
        .fold(f64::NEG_INFINITY, f64::max);

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

fn logical_rate_pieces(
    row: &BenchmarkResultRow,
    unit: LogicalRateUnit,
) -> Result<Option<f64>, String> {
    match unit {
        LogicalRateUnit::PerShot => Ok(None),
        LogicalRateUnit::PerRound => Ok(Some(required_positive_metadata(
            row,
            unit,
            "params.rounds",
        )?)),
        LogicalRateUnit::PerObservable => Ok(Some(required_observable_count(row, unit)?)),
        LogicalRateUnit::PerRoundPerObservable => {
            let rounds = required_positive_metadata(row, unit, "params.rounds")?;
            let observables = required_observable_count(row, unit)?;
            Ok(Some(rounds * observables))
        }
    }
}

fn required_observable_count(
    row: &BenchmarkResultRow,
    unit: LogicalRateUnit,
) -> Result<f64, String> {
    resolve_numeric_field(row, "case_summary.logical_observable_count")
        .or_else(|| resolve_numeric_field(row, "case_summary.num_obs"))
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| {
            format!(
                "logical_rate_unit = \"{}\" requires positive numeric case_summary.logical_observable_count or case_summary.num_obs for {}",
                unit.as_str(),
                row_context(row)
            )
        })
}

fn required_positive_metadata(
    row: &BenchmarkResultRow,
    unit: LogicalRateUnit,
    field: &str,
) -> Result<f64, String> {
    resolve_numeric_field(row, field)
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| {
            format!(
                "logical_rate_unit = \"{}\" requires positive numeric {field} for {}",
                unit.as_str(),
                row_context(row)
            )
        })
}

fn transform_logical_rate(shot_rate: f64, pieces: Option<f64>) -> f64 {
    match pieces {
        Some(pieces) => shot_error_rate_to_piece_error_rate(shot_rate, pieces),
        None => shot_rate,
    }
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

        let fit = logical_rate_fit_for_plot_with_factor(
            row,
            spec.plot.logical_rate_unit,
            spec.plot.confidence_interval_likelihood_factor,
        )?;
        let key = series_key(row, spec)?;
        let label = render_series_label(row, spec);

        groups
            .entry(key)
            .or_insert_with(|| SeriesData {
                label,
                points: Vec::new(),
                fit: None,
            })
            .points
            .push(ErrorRatePoint {
                x,
                low: fit.low,
                best: fit.best,
                high: fit.high,
            });
        x_values.push(x);
        y_values.extend([fit.low, fit.high]);
        if let Some(best) = fit.best {
            y_values.push(best);
        }
    }

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
        x_ticks: sorted_unique_f64(&x_values),
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

        let key = series_key(row, spec)?;
        let label = render_series_label(row, spec);
        groups
            .entry(key)
            .or_insert_with(|| SeriesData {
                label,
                points: Vec::new(),
                fit: None,
            })
            .points
            .push((x, y));
        x_values.push(x);
        y_values.push(y);
    }

    for series in groups.values_mut() {
        series
            .points
            .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
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
        x_ticks: sorted_unique_f64(&x_values),
        groups,
    })
}

fn render_plot_on<DB: DrawingBackend>(
    root: DrawingArea<DB, Shift>,
    spec: &BenchmarkSpec,
    panels: &[PreparedPanel],
    series_styles: &BTreeMap<SeriesKey, SeriesStyle>,
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

    for (index, (area, panel)) in areas.into_iter().zip(panels.iter()).enumerate() {
        let show_legend = index == 0;
        match panel {
            PreparedPanel::ErrorRate(data) => {
                render_error_rate_panel_on(area, data, series_styles, show_legend)?
            }
            PreparedPanel::Numeric(data) => {
                render_numeric_panel_on(area, data, series_styles, show_legend)?
            }
        }
    }

    root.present().map_err(|e| e.to_string())
}

fn render_error_rate_panel_on<DB: DrawingBackend>(
    area: DrawingArea<DB, Shift>,
    data: &ErrorRatePanelData,
    series_styles: &BTreeMap<SeriesKey, SeriesStyle>,
    show_legend: bool,
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
                .x_labels(8)
                .x_label_formatter(&|value| format_axis_tick(*value))
                .x_desc(data.x_label.as_str())
                .y_desc(data.y_label.as_str())
                .draw()
                .map_err(|e| e.to_string())?;
            draw_manual_x_tick_labels(&mut chart, &data.x_ticks, data.y_range, &data.y_scale)?;
            draw_error_rate_series(&mut chart, &data.groups, series_styles, show_legend)?;
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
                .x_labels(8)
                .x_label_formatter(&|value| format_axis_tick(*value))
                .x_desc(data.x_label.as_str())
                .y_desc(data.y_label.as_str())
                .draw()
                .map_err(|e| e.to_string())?;
            draw_manual_x_tick_labels(&mut chart, &data.x_ticks, data.y_range, &data.y_scale)?;
            draw_error_rate_series(&mut chart, &data.groups, series_styles, show_legend)?;
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
                .x_labels(8)
                .x_label_formatter(&|value| format_axis_tick(*value))
                .x_desc(data.x_label.as_str())
                .y_desc(data.y_label.as_str())
                .draw()
                .map_err(|e| e.to_string())?;
            draw_manual_x_tick_labels(&mut chart, &data.x_ticks, data.y_range, &data.y_scale)?;
            draw_error_rate_series(&mut chart, &data.groups, series_styles, show_legend)?;
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
                .x_labels(8)
                .x_label_formatter(&|value| format_axis_tick(*value))
                .x_desc(data.x_label.as_str())
                .y_desc(data.y_label.as_str())
                .draw()
                .map_err(|e| e.to_string())?;
            draw_manual_x_tick_labels(&mut chart, &data.x_ticks, data.y_range, &data.y_scale)?;
            draw_error_rate_series(&mut chart, &data.groups, series_styles, show_legend)?;
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
    series_styles: &BTreeMap<SeriesKey, SeriesStyle>,
    show_legend: bool,
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
                .x_labels(8)
                .x_label_formatter(&|value| format_axis_tick(*value))
                .x_desc(data.x_label.as_str())
                .y_desc(data.y_label.as_str())
                .draw()
                .map_err(|e| e.to_string())?;
            draw_manual_x_tick_labels(&mut chart, &data.x_ticks, data.y_range, &data.y_scale)?;
            draw_numeric_series(&mut chart, &data.groups, series_styles, show_legend)?;
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
                .x_labels(8)
                .x_label_formatter(&|value| format_axis_tick(*value))
                .x_desc(data.x_label.as_str())
                .y_desc(data.y_label.as_str())
                .draw()
                .map_err(|e| e.to_string())?;
            draw_manual_x_tick_labels(&mut chart, &data.x_ticks, data.y_range, &data.y_scale)?;
            draw_numeric_series(&mut chart, &data.groups, series_styles, show_legend)?;
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
                .x_labels(8)
                .x_label_formatter(&|value| format_axis_tick(*value))
                .x_desc(data.x_label.as_str())
                .y_desc(data.y_label.as_str())
                .draw()
                .map_err(|e| e.to_string())?;
            draw_manual_x_tick_labels(&mut chart, &data.x_ticks, data.y_range, &data.y_scale)?;
            draw_numeric_series(&mut chart, &data.groups, series_styles, show_legend)?;
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
                .x_labels(8)
                .x_label_formatter(&|value| format_axis_tick(*value))
                .x_desc(data.x_label.as_str())
                .y_desc(data.y_label.as_str())
                .draw()
                .map_err(|e| e.to_string())?;
            draw_manual_x_tick_labels(&mut chart, &data.x_ticks, data.y_range, &data.y_scale)?;
            draw_numeric_series(&mut chart, &data.groups, series_styles, show_legend)?;
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
    series_styles: &BTreeMap<SeriesKey, SeriesStyle>,
    show_legend: bool,
) -> Result<(), String>
where
    DB: DrawingBackend + 'a,
    DB::ErrorType: 'static,
    XR: Ranged<ValueType = f64>,
    YR: Ranged<ValueType = f64>,
{
    for (index, (key, series)) in groups.iter().enumerate() {
        let style = series_styles
            .get(key)
            .copied()
            .unwrap_or_else(|| default_series_style(index));
        let points = &series.points;
        let label = &series.label;

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

        if let Some(fit) = series.fit {
            draw_fit_line_series(chart, label, fit, style)?;
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

    draw_series_legend(chart, show_legend)
}

fn draw_numeric_series<'a, DB, XR, YR>(
    chart: &mut ChartContext<'a, DB, Cartesian2d<XR, YR>>,
    groups: &NumericGroups,
    series_styles: &BTreeMap<SeriesKey, SeriesStyle>,
    show_legend: bool,
) -> Result<(), String>
where
    DB: DrawingBackend + 'a,
    DB::ErrorType: 'static,
    XR: Ranged<ValueType = f64>,
    YR: Ranged<ValueType = f64>,
{
    for (index, (key, series)) in groups.iter().enumerate() {
        let style = series_styles
            .get(key)
            .copied()
            .unwrap_or_else(|| default_series_style(index));
        let points = &series.points;
        let label = &series.label;

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

    draw_series_legend(chart, show_legend)
}

fn draw_manual_x_tick_labels<'a, DB, XR, YR>(
    chart: &mut ChartContext<'a, DB, Cartesian2d<XR, YR>>,
    x_ticks: &[f64],
    y_range: (f64, f64),
    y_scale: &str,
) -> Result<(), String>
where
    DB: DrawingBackend + 'a,
    DB::ErrorType: 'static,
    XR: Ranged<ValueType = f64>,
    YR: Ranged<ValueType = f64>,
{
    let y = manual_x_tick_label_y(y_range, y_scale);
    chart
        .draw_series(x_ticks.iter().copied().map(|x| {
            let style = TextStyle::from(("sans-serif", 12).into_font())
                .pos(Pos::new(HPos::Center, VPos::Top));
            EmptyElement::at((x, y)) + Text::new(format_axis_tick(x), (0, 16), style)
        }))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn manual_x_tick_label_y(y_range: (f64, f64), y_scale: &str) -> f64 {
    if y_scale == "log" {
        y_range.0 * (y_range.1 / y_range.0).powf(0.02)
    } else {
        y_range.0 + (y_range.1 - y_range.0) * 0.02
    }
}

fn draw_series_legend<'a, DB, XR, YR>(
    chart: &mut ChartContext<'a, DB, Cartesian2d<XR, YR>>,
    show_legend: bool,
) -> Result<(), String>
where
    DB: DrawingBackend + 'a,
    DB::ErrorType: 'static,
    XR: Ranged<ValueType = f64>,
    YR: Ranged<ValueType = f64>,
{
    if !show_legend {
        return Ok(());
    }
    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::LowerRight)
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
                    DashedPathElement::new(
                        [(x, y), (x + 28, y)],
                        size,
                        spacing,
                        ShapeStyle::from(&legend_color).stroke_width(2),
                    )
                });
        }
    }
    Ok(())
}

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
            DashedPathElement::new(
                [(x, y), (x + 28, y)],
                4,
                4,
                ShapeStyle::from(&legend_color).stroke_width(2),
            )
        });
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

fn sorted_unique_f64(values: &[f64]) -> Vec<f64> {
    let mut values = values.to_vec();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    values.dedup_by(|left, right| (*left - *right).abs() <= 1e-12);
    values
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
) -> Result<BTreeMap<SeriesKey, SeriesStyle>, String> {
    if rows.iter().any(|row| row.params.contains_key("code_id")) {
        let mut series_keys = Vec::new();
        for row in rows.iter() {
            let key = series_key(row, spec)?;
            if !series_keys.contains(&key) {
                series_keys.push(key);
            }
        }
        series_keys.sort();
        let series_index: BTreeMap<SeriesKey, usize> = series_keys
            .into_iter()
            .enumerate()
            .map(|(index, key)| (key, index))
            .collect();
        let mut styles = BTreeMap::new();
        for row in rows.iter() {
            let key = series_key(row, spec)?;
            let color_index = series_index.get(&key).copied().unwrap_or(0);
            styles.entry(key).or_insert_with(|| SeriesStyle {
                color: legacy_matplotlib_color(color_index),
                pattern: line_pattern_for_runner(&row.runner),
            });
        }
        return Ok(styles);
    }

    let mut family_values: Vec<String> = rows
        .iter()
        .map(|row| decoder_family_for_style(&row.runner).to_string())
        .collect();
    family_values.sort();
    family_values.dedup();

    let family_index: BTreeMap<String, usize> = family_values
        .into_iter()
        .enumerate()
        .map(|(index, family)| (family, index))
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
    let distance_count = distance_index.len().max(1);

    let mut styles = BTreeMap::new();
    for row in rows.iter() {
        let key = series_key(row, spec)?;
        styles.entry(key).or_insert_with(|| {
            let family = decoder_family_for_style(&row.runner);
            let family_offset = family_index.get(family).copied().unwrap_or(0);
            let distance_offset = row
                .params
                .get("distance")
                .map(value_to_string)
                .and_then(|value| distance_index.get(&value).copied())
                .unwrap_or(0);
            let color_index = family_offset * distance_count + distance_offset;
            SeriesStyle {
                color: legacy_matplotlib_color(color_index),
                pattern: line_pattern_for_runner(&row.runner),
            }
        });
    }
    Ok(styles)
}

fn legacy_matplotlib_color(index: usize) -> RGBAColor {
    const COLORS: [RGBColor; 10] = [
        RGBColor(0x1f, 0x77, 0xb4),
        RGBColor(0xff, 0x7f, 0x0e),
        RGBColor(0x2c, 0xa0, 0x2c),
        RGBColor(0xd6, 0x27, 0x28),
        RGBColor(0x94, 0x67, 0xbd),
        RGBColor(0x8c, 0x56, 0x4b),
        RGBColor(0xe3, 0x77, 0xc2),
        RGBColor(0x7f, 0x7f, 0x7f),
        RGBColor(0xbc, 0xbd, 0x22),
        RGBColor(0x17, 0xbe, 0xcf),
    ];
    COLORS[index % COLORS.len()].to_rgba()
}

fn decoder_family_for_style(runner: &str) -> &str {
    match runner {
        "pymatching" | "rmatching" => "mwpm",
        "ilpqec" | "rilpqec" => "ilp",
        "ldpc" | "rbposd" => "bp",
        other => other,
    }
}

fn line_pattern_for_runner(runner: &str) -> LinePattern {
    match runner {
        "rmatching" | "rilpqec" | "rbposd" => LinePattern::Dashed {
            size: 12,
            spacing: 8,
        },
        _ => LinePattern::Solid,
    }
}

fn default_series_style(index: usize) -> SeriesStyle {
    SeriesStyle {
        color: Palette99::pick(index).mix(0.9),
        pattern: LinePattern::Solid,
    }
}

fn format_axis_tick(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    if value == 0.0 {
        return "0".into();
    }
    let abs = value.abs();
    if (0.001..1.0).contains(&abs) {
        trim_decimal(format!("{value:.4}"))
    } else if (1.0..10_000.0).contains(&abs) {
        trim_decimal(format!("{value:.3}"))
    } else {
        format!("{value:.1e}")
    }
}

fn trim_decimal(mut value: String) -> String {
    if value.contains('.') {
        while value.ends_with('0') {
            value.pop();
        }
        if value.ends_with('.') {
            value.pop();
        }
    }
    value
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

fn series_key(row: &BenchmarkResultRow, spec: &BenchmarkSpec) -> Result<SeriesKey, String> {
    if spec.plot.series.group_by.is_empty() {
        return Ok(vec![format!("label={}", render_series_label(row, spec))]);
    }

    spec.plot
        .series
        .group_by
        .iter()
        .map(|field| {
            resolve_series_group_field(row, field)
                .map(|value| format!("{field}={value}"))
                .ok_or_else(|| {
                    format!(
                        "missing required series group field {field} for {}",
                        row_context(row)
                    )
                })
        })
        .collect()
}

fn resolve_series_group_field(row: &BenchmarkResultRow, field: &str) -> Option<String> {
    if field == "runner" {
        return Some(row.runner.clone());
    }
    if field == "language" {
        return Some(row.language.clone());
    }

    let (scope, key) = field.split_once('.')?;
    match scope {
        "params" => row.params.get(key).map(value_to_string),
        "metrics" => row.metrics.get(key).copied().map(metric_to_string),
        "case_summary" => row.case_summary.get(key).map(value_to_string),
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_and_axis_tick_helpers_cover_fallback_edges() {
        let style = default_series_style(3);
        match style.pattern {
            LinePattern::Solid => {}
            LinePattern::Dashed { .. } => panic!("default series style should be solid"),
        }

        assert_eq!(format_axis_tick(f64::INFINITY), "inf");
        assert_eq!(format_axis_tick(0.0), "0");
        assert_eq!(format_axis_tick(0.0012), "0.0012");
        assert_eq!(format_axis_tick(1.0), "1");
        assert_eq!(format_axis_tick(10_000.0), "1.0e4");
    }
}
}
