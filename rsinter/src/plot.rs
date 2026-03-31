use std::collections::BTreeMap;
use std::path::Path;

use plotters::prelude::*;

use crate::stats::{fit_binomial, shot_error_rate_to_piece_error_rate};
use crate::task_stats::TaskStats;

const MAX_LIKELIHOOD_FACTOR: f64 = 9.0;
const CANVAS_WIDTH: u32 = 800;
const CANVAS_HEIGHT: u32 = 600;

pub fn plot_error_rate(
    stats: &[TaskStats],
    x_func: impl Fn(&TaskStats) -> f64,
    group_func: impl Fn(&TaskStats) -> String,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    plot_error_rate_transformed(
        stats,
        x_func,
        group_func,
        "Logical Error Rate",
        |rate, _stat| rate,
        output,
    )
}

pub fn plot_error_rate_per_piece(
    stats: &[TaskStats],
    x_func: impl Fn(&TaskStats) -> f64,
    group_func: impl Fn(&TaskStats) -> String,
    pieces_func: impl Fn(&TaskStats) -> f64,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    plot_error_rate_transformed(
        stats,
        x_func,
        group_func,
        "Logical Error Rate Per Round",
        |rate, stat| shot_error_rate_to_piece_error_rate(rate, pieces_func(stat)),
        output,
    )
}

fn plot_error_rate_transformed<X, G, R>(
    stats: &[TaskStats],
    x_func: X,
    group_func: G,
    y_label: &str,
    rate_transform: R,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>>
where
    X: Fn(&TaskStats) -> f64,
    G: Fn(&TaskStats) -> String,
    R: Fn(f64, &TaskStats) -> f64,
{
    let mut groups: BTreeMap<String, Vec<(f64, f64, f64, f64)>> = BTreeMap::new();
    for stat in stats {
        let x = x_func(stat);
        let (low, best, high) = fit_stat_rates(stat, &rate_transform);
        groups.entry(group_func(stat)).or_default().push((x, low, best, high));
    }
    for pts in groups.values_mut() {
        pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }

    if groups.is_empty() {
        return Ok(());
    }

    // Compute axis ranges
    let all_x: Vec<f64> = groups.values().flat_map(|v| v.iter().map(|p| p.0)).collect();
    let all_y: Vec<f64> = groups.values().flat_map(|v| v.iter().flat_map(|p| [p.1, p.2, p.3])).collect();

    let x_min = all_x.iter().cloned().fold(f64::INFINITY, f64::min);
    let x_max = all_x.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let y_min_pos = all_y.iter().cloned().filter(|v| *v > 0.0).fold(f64::INFINITY, f64::min);
    let y_max = all_y.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    // Multiplicative padding so the range stays positive on a log-scale x-axis.
    let x_range = (x_min / 1.1)..(x_max * 1.1);
    let y_low  = (y_min_pos * 0.5).max(1e-10);
    let y_high = (y_max * 2.0).min(1.0);

    let ext = output.extension().and_then(|e| e.to_str()).unwrap_or("svg").to_lowercase();
    if ext == "png" {
        let root = BitMapBackend::new(output, (CANVAS_WIDTH, CANVAS_HEIGHT)).into_drawing_area();
        render(root, &groups, x_range, y_low..y_high, y_label)?;
    } else {
        let root = SVGBackend::new(output, (CANVAS_WIDTH, CANVAS_HEIGHT)).into_drawing_area();
        render(root, &groups, x_range, y_low..y_high, y_label)?;
    }
    Ok(())
}

fn fit_stat_rates<R>(stat: &TaskStats, rate_transform: &R) -> (f64, f64, f64)
where
    R: Fn(f64, &TaskStats) -> f64,
{
    let effective_shots = stat.shots - stat.discards;
    let fit = fit_binomial(effective_shots, stat.errors, MAX_LIKELIHOOD_FACTOR);
    let low = rate_transform(fit.low.unwrap_or(0.0), stat);
    let best = rate_transform(fit.best.unwrap_or(0.0), stat);
    let high = rate_transform(fit.high.unwrap_or(0.0), stat);
    (low, best, high)
}

fn confidence_band_polygon(points: &[(f64, f64, f64, f64)]) -> Vec<(f64, f64)> {
    let mut polygon = Vec::with_capacity(points.len() * 2);
    polygon.extend(points.iter().map(|(x, _low, _best, high)| (*x, high.max(1e-10))));
    polygon.extend(
        points
            .iter()
            .rev()
            .map(|(x, low, _best, _high)| (*x, low.max(1e-10))),
    );
    polygon
}

fn render<DB: DrawingBackend>(
    root: DrawingArea<DB, plotters::coord::Shift>,
    groups: &BTreeMap<String, Vec<(f64, f64, f64, f64)>>,
    x_range: std::ops::Range<f64>,
    y_range: std::ops::Range<f64>,
    y_label: &str,
) -> Result<(), Box<dyn std::error::Error>>
where
    DB::ErrorType: 'static,
{
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .margin(40)
        .x_label_area_size(50)
        .y_label_area_size(70)
        .build_cartesian_2d(
            (x_range.start..x_range.end).log_scale(),
            (y_range.start..y_range.end).log_scale(),
        )?;

    chart
        .configure_mesh()
        .x_desc("Physical Error Rate")
        .y_desc(y_label)
        .draw()?;

    for (i, (label, points)) in groups.iter().enumerate() {
        let color = Palette99::pick(i).mix(0.9);
        let legend_color = color.clone();

        if points.len() > 1 {
            chart.draw_series(std::iter::once(Polygon::new(
                confidence_band_polygon(points),
                ShapeStyle::from(&color.mix(0.2)).filled(),
            )))?;
        }

        // Line through best values
        chart.draw_series(LineSeries::new(
            points.iter().map(|(x, _, best, _)| (*x, best.max(1e-10))),
            ShapeStyle::from(&color).stroke_width(2),
        ))?.label(label).legend(move |(x, y)| {
            PathElement::new(vec![(x, y), (x + 20, y)], ShapeStyle::from(&legend_color).stroke_width(2))
        });

        chart.draw_series(points.iter().map(|(x, _low, best, _high)| {
            Circle::new(
                (*x, best.max(1e-10)),
                4,
                ShapeStyle::from(&color).filled(),
            )
        }))?;

        // Match sinter's behavior: multi-point series use a filled uncertainty
        // highlight, while a single isolated point falls back to an error bar.
        if points.len() == 1 {
            const CAP_FACTOR: f64 = 1.015;
            let (x, low, _best, high) = points[0];
            let low = low.max(1e-10);
            let x_lo = x / CAP_FACTOR;
            let x_hi = x * CAP_FACTOR;
            chart.draw_series([
                PathElement::new(vec![(x, low), (x, high)], ShapeStyle::from(&color).stroke_width(1)),
                PathElement::new(vec![(x_lo, low), (x_hi, low)], ShapeStyle::from(&color).stroke_width(1)),
                PathElement::new(vec![(x_lo, high), (x_hi, high)], ShapeStyle::from(&color).stroke_width(1)),
            ])?;
        }
    }

    chart.configure_series_labels()
        .position(SeriesLabelPosition::UpperLeft)
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .draw()?;

    root.present()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::shot_error_rate_to_piece_error_rate;
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn make_stat(p: f64, d: u64, r: u64, shots: u64, errors: u64) -> TaskStats {
        TaskStats {
            strong_id: String::new(),
            decoder: String::new(),
            metadata: serde_json::json!({"p": p, "d": d, "r": r}),
            shots,
            errors,
            discards: 0,
            seconds: 0.0,
            custom_counts: HashMap::new(),
        }
    }

    #[test]
    fn test_plot_svg_created() {
        let stats = vec![
            make_stat(0.001, 3, 9, 10000, 10),
            make_stat(0.005, 3, 9, 10000, 100),
            make_stat(0.01,  3, 9, 10000, 500),
            make_stat(0.001, 5, 15, 10000, 1),
            make_stat(0.005, 5, 15, 10000, 20),
            make_stat(0.01,  5, 15, 10000, 150),
        ];
        let dir = tempdir().unwrap();
        let out = dir.path().join("plot.svg");
        plot_error_rate(
            &stats,
            |s| s.metadata["p"].as_f64().unwrap(),
            |s| format!("d={}", s.metadata["d"].as_u64().unwrap()),
            &out,
        ).unwrap();
        assert!(out.exists());
        let content = std::fs::read_to_string(&out).unwrap();
        assert!(content.contains("<svg"), "output should be SVG");
    }

    #[test]
    fn test_plot_png_created() {
        let stats = vec![
            make_stat(0.001, 3, 9, 10000, 10),
            make_stat(0.01,  3, 9, 10000, 500),
        ];
        let dir = tempdir().unwrap();
        let out = dir.path().join("plot.png");
        plot_error_rate(
            &stats,
            |s| s.metadata["p"].as_f64().unwrap(),
            |s| format!("d={}", s.metadata["d"].as_u64().unwrap()),
            &out,
        ).unwrap();
        assert!(out.exists());
        // PNG magic bytes: 0x89 0x50 0x4E 0x47
        let bytes = std::fs::read(&out).unwrap();
        assert_eq!(&bytes[0..4], b"\x89PNG");
    }

    #[test]
    fn test_plot_per_piece_svg_created() {
        let stats = vec![
            make_stat(0.008, 3, 9, 10_000, 120),
            make_stat(0.010, 3, 9, 10_000, 220),
            make_stat(0.008, 5, 15, 10_000, 30),
            make_stat(0.010, 5, 15, 10_000, 70),
        ];
        let dir = tempdir().unwrap();
        let out = dir.path().join("plot_per_round.svg");
        plot_error_rate_per_piece(
            &stats,
            |s| s.metadata["p"].as_f64().unwrap(),
            |s| format!("d={}", s.metadata["d"].as_u64().unwrap()),
            |s| s.metadata["r"].as_u64().unwrap() as f64,
            &out,
        )
        .unwrap();
        assert!(out.exists());
        let content = std::fs::read_to_string(&out).unwrap();
        assert!(content.contains("<svg"), "output should be SVG");
    }

    #[test]
    fn test_plot_svg_uses_filled_uncertainty_bands_for_multi_point_series() {
        let stats = vec![
            make_stat(0.008, 3, 9, 10_000, 120),
            make_stat(0.009, 3, 9, 10_000, 160),
            make_stat(0.010, 3, 9, 10_000, 220),
            make_stat(0.008, 5, 15, 10_000, 30),
            make_stat(0.009, 5, 15, 10_000, 45),
            make_stat(0.010, 5, 15, 10_000, 70),
        ];
        let dir = tempdir().unwrap();
        let out = dir.path().join("plot_with_bands.svg");
        plot_error_rate_per_piece(
            &stats,
            |s| s.metadata["p"].as_f64().unwrap(),
            |s| format!("d={}", s.metadata["d"].as_u64().unwrap()),
            |s| s.metadata["r"].as_u64().unwrap() as f64,
            &out,
        )
        .unwrap();

        let content = std::fs::read_to_string(&out).unwrap();
        assert!(
            content.contains("fill=\"#E6194B\"") || content.contains("fill=\"#3CB44B\""),
            "expected a series-colored filled confidence band in the SVG, got:\n{content}"
        );
    }

    #[test]
    fn test_plot_svg_marks_sampled_points_on_curve() {
        let stats = vec![
            make_stat(0.008, 3, 9, 10_000, 120),
            make_stat(0.009, 3, 9, 10_000, 160),
            make_stat(0.010, 3, 9, 10_000, 220),
        ];
        let dir = tempdir().unwrap();
        let out = dir.path().join("plot_with_markers.svg");
        plot_error_rate_per_piece(
            &stats,
            |s| s.metadata["p"].as_f64().unwrap(),
            |s| format!("d={}", s.metadata["d"].as_u64().unwrap()),
            |s| s.metadata["r"].as_u64().unwrap() as f64,
            &out,
        )
        .unwrap();

        let content = std::fs::read_to_string(&out).unwrap();
        assert!(
            content.contains("<circle"),
            "expected explicit point markers in the SVG, got:\n{content}"
        );
    }

    #[test]
    fn test_per_piece_fit_is_lower_than_per_shot_fit_for_multiple_rounds() {
        let stat = make_stat(0.01, 3, 9, 10_000, 100);
        let fit = fit_binomial(stat.shots, stat.errors, MAX_LIKELIHOOD_FACTOR);
        let per_shot_best = fit.best.unwrap();
        let per_piece_best =
            shot_error_rate_to_piece_error_rate(per_shot_best, stat.metadata["r"].as_u64().unwrap() as f64);
        assert!(per_piece_best < per_shot_best);
    }

    #[test]
    fn test_confidence_band_polygon_traces_upper_then_lower_boundary() {
        let points = vec![
            (1.0, 0.1, 0.2, 0.3),
            (2.0, 0.2, 0.3, 0.4),
            (3.0, 0.3, 0.4, 0.5),
        ];
        assert_eq!(
            confidence_band_polygon(&points),
            vec![
                (1.0, 0.3),
                (2.0, 0.4),
                (3.0, 0.5),
                (3.0, 0.3),
                (2.0, 0.2),
                (1.0, 0.1),
            ]
        );
    }
}
