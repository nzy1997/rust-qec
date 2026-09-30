//! Self-contained surface-code memory sweep for the Get started guide.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use plotters::prelude::*;
use serde::Serialize;

const CANVAS_WIDTH: u32 = 800;
const CANVAS_HEIGHT: u32 = 600;

#[derive(Debug, Clone)]
pub struct SurfaceCodeLerOptions {
    pub distances: Vec<u64>,
    pub rounds: Vec<u64>,
    pub physical_error_rates: Vec<f64>,
    pub shots: u64,
    pub seed: u64,
    pub out_dir: PathBuf,
}

#[derive(Serialize)]
struct ErrorRateRow {
    distance: u64,
    rounds: u64,
    physical_error_rate: f64,
    shots: u64,
    seed: u64,
    logical_failures: u64,
    logical_error_rate_per_shot: f64,
    logical_error_rate_per_round: f64,
}

fn run_cli(mut command: Command) -> Result<(), String> {
    let display = format!("{command:?}");
    let output = command
        .output()
        .map_err(|error| format!("{display}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = if output.stderr.is_empty() {
        String::from_utf8_lossy(&output.stdout)
    } else {
        String::from_utf8_lossy(&output.stderr)
    };
    Err(format!("{display} failed: {detail}"))
}

fn count_failures(actual: &Path, predicted: &Path, shots: usize) -> Result<u64, String> {
    let actual_text = fs::read_to_string(actual).map_err(|error| error.to_string())?;
    let predicted_text = fs::read_to_string(predicted).map_err(|error| error.to_string())?;
    let actual_rows: Vec<_> = actual_text.lines().collect();
    let predicted_rows: Vec<_> = predicted_text.lines().collect();
    if actual_rows.len() != shots || predicted_rows.len() != shots {
        return Err("sampler and decoder must each return exactly one row per shot".into());
    }
    if actual_rows
        .iter()
        .chain(predicted_rows.iter())
        .any(|row| *row != "0" && *row != "1")
    {
        return Err("surface-code-ler expects one logical bit per shot".into());
    }
    Ok(actual_rows
        .iter()
        .zip(predicted_rows.iter())
        .filter(|(actual, predicted)| actual != predicted)
        .count() as u64)
}

pub fn run(options: &SurfaceCodeLerOptions) -> Result<(), String> {
    if options.distances.is_empty() || options.distances.len() != options.rounds.len() {
        return Err(
            "--distances and --rounds must contain the same nonzero number of values".into(),
        );
    }
    if options.distances.iter().any(|distance| *distance < 2)
        || options.distances.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err("--distances must be at least 2 and strictly increasing".into());
    }
    if options.rounds.contains(&0) {
        return Err("--rounds values must be positive".into());
    }
    if options.physical_error_rates.is_empty()
        || options
            .physical_error_rates
            .iter()
            .any(|rate| !rate.is_finite() || *rate <= 0.0 || *rate >= 1.0)
        || options
            .physical_error_rates
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(
            "--physical-error-rates must be finite, between 0 and 1, and strictly increasing"
                .into(),
        );
    }
    if options.shots == 0 {
        return Err("--shots must be positive".into());
    }
    let shot_count = usize::try_from(options.shots)
        .map_err(|_| "--shots exceeds the platform's supported size".to_string())?;
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let work = tempfile::tempdir().map_err(|error| error.to_string())?;
    let circuit = work.path().join("surface.stim");
    let dem = work.path().join("surface.dem");
    let detectors = work.path().join("detectors.01");
    let actual = work.path().join("actual.01");
    let predicted = work.path().join("predicted.01");
    let mut rows = Vec::new();

    for (&distance, &rounds) in options.distances.iter().zip(&options.rounds) {
        for (index, &p) in options.physical_error_rates.iter().enumerate() {
            let seed = distance
                .checked_mul(100)
                .and_then(|offset| options.seed.checked_add(offset))
                .and_then(|value| value.checked_add(index as u64))
                .ok_or_else(|| "sampling seed overflow".to_string())?;
            let distance_text = distance.to_string();
            let rounds_text = rounds.to_string();
            let noise = p.to_string();
            let seed_text = seed.to_string();
            let shots_text = options.shots.to_string();

            let mut command = Command::new(&executable);
            command
                .args([
                    "circuit",
                    "gen",
                    "--code",
                    "surface_code",
                    "--task",
                    "rotated_memory_z",
                ])
                .args(["--distance", &distance_text, "--rounds", &rounds_text])
                .args(["--after-clifford-depolarization", &noise])
                .args(["--before-round-data-depolarization", &noise])
                .args(["--after-reset-flip-probability", &noise])
                .args(["--before-measure-flip-probability", &noise])
                .arg("--out")
                .arg(&circuit);
            run_cli(command)?;

            let mut command = Command::new(&executable);
            command
                .args(["circuit", "dem", "--in"])
                .arg(&circuit)
                .arg("--decompose-errors")
                .arg("--out")
                .arg(&dem);
            run_cli(command)?;

            let mut command = Command::new(&executable);
            command
                .args(["circuit", "detect", "--in"])
                .arg(&circuit)
                .args(["--shots", &shots_text, "--seed", &seed_text])
                .args(["--out-format", "01", "--out"])
                .arg(&detectors)
                .arg("--obs-out")
                .arg(&actual)
                .args(["--obs-out-format", "01"]);
            run_cli(command)?;

            let mut command = Command::new(&executable);
            command
                .args(["circuit", "decode", "--decoder", "rmatching", "--dem"])
                .arg(&dem)
                .arg("--in")
                .arg(&detectors)
                .arg("--out")
                .arg(&predicted);
            run_cli(command)?;

            let failures = count_failures(&actual, &predicted, shot_count)?;
            rows.push(ErrorRateRow {
                distance,
                rounds,
                physical_error_rate: p,
                shots: options.shots,
                seed,
                logical_failures: failures,
                logical_error_rate_per_shot: failures as f64 / options.shots as f64,
                logical_error_rate_per_round: per_round_error_rate(
                    failures as f64 / options.shots as f64,
                    rounds,
                ),
            });
            println!(
                "d={distance} p={p:.3}: {failures}/{} logical failures",
                options.shots
            );
        }
    }

    fs::create_dir_all(&options.out_dir).map_err(|error| error.to_string())?;
    let csv_path = options.out_dir.join("surface-code-ler.csv");
    let svg_path = options.out_dir.join("surface-code-ler.svg");
    let mut writer = csv::Writer::from_path(&csv_path).map_err(|error| error.to_string())?;
    for row in &rows {
        writer.serialize(row).map_err(|error| error.to_string())?;
    }
    writer.flush().map_err(|error| error.to_string())?;
    plot_rows(&rows, &svg_path)?;
    println!("Wrote {} and {}", csv_path.display(), svg_path.display());
    Ok(())
}

fn log_likelihood(rate: f64, failures: u64, shots: u64) -> f64 {
    if (rate == 0.0 && failures > 0) || (rate == 1.0 && failures < shots) {
        return f64::NEG_INFINITY;
    }
    let mut value = 0.0;
    if failures > 0 {
        value += failures as f64 * rate.ln();
    }
    if failures < shots {
        value += (shots - failures) as f64 * (-rate).ln_1p();
    }
    value
}

fn likelihood_bounds(failures: u64, shots: u64) -> (f64, f64) {
    let best = failures as f64 / shots as f64;
    let cutoff = log_likelihood(best, failures, shots) - 9.0_f64.ln();
    let (mut low, mut high) = (0.0, best);
    for _ in 0..60 {
        let middle = (low + high) / 2.0;
        if log_likelihood(middle, failures, shots) < cutoff {
            low = middle;
        } else {
            high = middle;
        }
    }
    let lower = high;
    let (mut low, mut high) = (best, 1.0);
    for _ in 0..60 {
        let middle = (low + high) / 2.0;
        if log_likelihood(middle, failures, shots) >= cutoff {
            low = middle;
        } else {
            high = middle;
        }
    }
    (lower, low)
}

fn per_round_error_rate(per_shot: f64, rounds: u64) -> f64 {
    // The inverse of p_shot = (1 - (1 - 2 p_round)^rounds) / 2.
    // Clamp finite-sample rates above 50%, where this parity model cannot invert them.
    let per_shot = per_shot.clamp(0.0, 0.5);
    -(((-2.0 * per_shot).ln_1p() / rounds as f64).exp_m1()) / 2.0
}

fn plot_rows(rows: &[ErrorRateRow], output: &Path) -> Result<(), String> {
    let mut groups: BTreeMap<(u64, u64), Vec<(f64, f64, f64, f64)>> = BTreeMap::new();
    for row in rows {
        let (low, high) = likelihood_bounds(row.logical_failures, row.shots);
        groups.entry((row.distance, row.rounds)).or_default().push((
            row.physical_error_rate,
            per_round_error_rate(low, row.rounds),
            row.logical_error_rate_per_round,
            per_round_error_rate(high, row.rounds),
        ));
    }
    let x_min = rows
        .iter()
        .map(|row| row.physical_error_rate)
        .fold(f64::INFINITY, f64::min);
    let x_max = rows
        .iter()
        .map(|row| row.physical_error_rate)
        .fold(f64::NEG_INFINITY, f64::max);
    let y_min = groups
        .values()
        .flat_map(|points| points.iter().map(|point| point.1))
        .filter(|value| *value > 0.0)
        .fold(f64::INFINITY, f64::min);
    let y_max = groups
        .values()
        .flat_map(|points| points.iter().map(|point| point.3))
        .fold(f64::NEG_INFINITY, f64::max);
    let y_low = if y_min.is_finite() {
        (y_min * 0.75).max(1e-10)
    } else {
        (y_max / 10.0).max(1e-10)
    };
    let x_padding = ((x_max - x_min) * 0.1).max(x_min * 0.05);
    let root = SVGBackend::new(output, (CANVAS_WIDTH, CANVAS_HEIGHT)).into_drawing_area();
    render(
        root,
        &groups,
        (x_min - x_padding).max(0.0)..(x_max + x_padding).min(1.0),
        y_low..(y_max * 1.3).min(1.0),
    )
    .map_err(|error| error.to_string())
}

fn render<DB: DrawingBackend>(
    root: DrawingArea<DB, plotters::coord::Shift>,
    groups: &BTreeMap<(u64, u64), Vec<(f64, f64, f64, f64)>>,
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
        .build_cartesian_2d(x_range, y_range.log_scale())?;
    chart
        .configure_mesh()
        .x_desc("Physical Error Rate")
        .x_labels(7)
        .x_label_formatter(&|value| format!("{value:.3}"))
        .y_desc("Logical Error Rate per Round")
        .light_line_style(WHITE)
        .draw()?;

    for (index, ((distance, rounds), points)) in groups.iter().enumerate() {
        let color = match index {
            0 => RGBColor(191, 55, 79).mix(0.9),
            1 => RGBColor(24, 130, 98).mix(0.9),
            2 => RGBColor(48, 82, 180).mix(0.9),
            _ => Palette99::pick(index).mix(0.9),
        };
        let legend_color = color;
        let mut band = points
            .iter()
            .map(|(x, _low, _best, high)| (*x, high.max(1e-10)))
            .collect::<Vec<_>>();
        band.extend(
            points
                .iter()
                .rev()
                .map(|(x, low, _best, _high)| (*x, low.max(1e-10))),
        );
        chart.draw_series(std::iter::once(Polygon::new(
            band,
            ShapeStyle::from(&color.mix(0.2)).filled(),
        )))?;
        chart
            .draw_series(LineSeries::new(
                points
                    .iter()
                    .map(|(x, _low, best, _high)| (*x, best.max(1e-10))),
                ShapeStyle::from(&color).stroke_width(2),
            ))?
            .label(format!("distance {distance}, rounds {rounds}"))
            .legend(move |(x, y)| {
                PathElement::new(
                    vec![(x, y), (x + 20, y)],
                    ShapeStyle::from(&legend_color).stroke_width(2),
                )
            });
        chart.draw_series(points.iter().map(|(x, _low, best, _high)| {
            Circle::new((*x, best.max(1e-10)), 4, ShapeStyle::from(&color).filled())
        }))?;
    }
    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::UpperLeft)
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .draw()?;
    root.present()?;
    Ok(())
}
