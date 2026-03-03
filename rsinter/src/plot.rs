use std::collections::BTreeMap;
use std::path::Path;

use plotters::prelude::*;

use crate::stats::fit_binomial;
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
    // Group stats and compute (x, low, best, high) per group
    let mut groups: BTreeMap<String, Vec<(f64, f64, f64, f64)>> = BTreeMap::new();
    for stat in stats {
        let x = x_func(stat);
        let effective_shots = stat.shots - stat.discards;
        let fit = fit_binomial(effective_shots, stat.errors, MAX_LIKELIHOOD_FACTOR);
        let best = fit.best.unwrap_or(0.0);
        let low  = fit.low.unwrap_or(0.0);
        let high = fit.high.unwrap_or(0.0);
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
        render(root, &groups, x_range, y_low..y_high)?;
    } else {
        let root = SVGBackend::new(output, (CANVAS_WIDTH, CANVAS_HEIGHT)).into_drawing_area();
        render(root, &groups, x_range, y_low..y_high)?;
    }
    Ok(())
}

fn render<DB: DrawingBackend>(
    root: DrawingArea<DB, plotters::coord::Shift>,
    groups: &BTreeMap<String, Vec<(f64, f64, f64, f64)>>,
    x_range: std::ops::Range<f64>,
    y_range: std::ops::Range<f64>,
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
        .y_desc("Logical Error Rate")
        .draw()?;

    // Multiplicative cap: each error bar whisker extends 1.5% of x on each side.
    // This stays visually consistent on a log-scale x-axis.
    const CAP_FACTOR: f64 = 1.015;

    for (i, (label, points)) in groups.iter().enumerate() {
        let color = Palette99::pick(i).mix(0.9);
        let legend_color = color.clone();

        // Line through best values
        chart.draw_series(LineSeries::new(
            points.iter().map(|(x, _, best, _)| (*x, best.max(1e-10))),
            ShapeStyle::from(&color).stroke_width(2),
        ))?.label(label).legend(move |(x, y)| {
            PathElement::new(vec![(x, y), (x + 20, y)], ShapeStyle::from(&legend_color).stroke_width(2))
        });

        // Error bars: vertical line from low to high, with horizontal caps
        chart.draw_series(points.iter().flat_map(|(x, low, _best, high)| {
            let low = low.max(1e-10);
            let x_lo = x / CAP_FACTOR;
            let x_hi = x * CAP_FACTOR;
            vec![
                PathElement::new(vec![(*x, low), (*x, *high)], ShapeStyle::from(&color).stroke_width(1)),
                PathElement::new(vec![(x_lo, low),  (x_hi, low)],   ShapeStyle::from(&color).stroke_width(1)),
                PathElement::new(vec![(x_lo, *high), (x_hi, *high)], ShapeStyle::from(&color).stroke_width(1)),
            ]
        }))?;
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
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn make_stat(p: f64, d: u64, shots: u64, errors: u64) -> TaskStats {
        TaskStats {
            strong_id: String::new(),
            decoder: String::new(),
            metadata: serde_json::json!({"p": p, "d": d}),
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
            make_stat(0.001, 3, 10000, 10),
            make_stat(0.005, 3, 10000, 100),
            make_stat(0.01,  3, 10000, 500),
            make_stat(0.001, 5, 10000, 1),
            make_stat(0.005, 5, 10000, 20),
            make_stat(0.01,  5, 10000, 150),
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
            make_stat(0.001, 3, 10000, 10),
            make_stat(0.01,  3, 10000, 500),
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
}
