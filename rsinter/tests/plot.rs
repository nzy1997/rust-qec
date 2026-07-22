#![cfg(feature = "plotting")]

use std::collections::HashMap;

use rsinter::failure::FailureKind;
use rsinter::plot::{plot_error_rate, plot_error_rate_per_piece};
use rsinter::task_stats::TaskStats;
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
        failure_kind: FailureKind::Ok,
        custom_counts: HashMap::new(),
    }
}

#[test]
fn plot_error_rate_empty_input_does_not_write_output() {
    let stats: Vec<TaskStats> = Vec::new();
    let dir = tempdir().unwrap();
    let out = dir.path().join("empty.svg");

    plot_error_rate(&stats, |_| 0.0, |_| "unused".to_string(), &out).unwrap();

    assert!(!out.exists());
}

#[test]
fn plot_error_rate_writes_svg_and_png_outputs() {
    let stats = vec![
        make_stat(0.001, 3, 9, 10_000, 10),
        make_stat(0.005, 3, 9, 10_000, 100),
        make_stat(0.010, 3, 9, 10_000, 500),
        make_stat(0.001, 5, 15, 10_000, 1),
        make_stat(0.005, 5, 15, 10_000, 20),
        make_stat(0.010, 5, 15, 10_000, 150),
    ];
    let dir = tempdir().unwrap();
    let svg_out = dir.path().join("plot.svg");
    let png_out = dir.path().join("plot.png");

    for out in [&svg_out, &png_out] {
        plot_error_rate(
            &stats,
            |s| s.metadata["p"].as_f64().unwrap(),
            |s| format!("d={}", s.metadata["d"].as_u64().unwrap()),
            out,
        )
        .unwrap();
    }

    assert!(std::fs::read_to_string(&svg_out).unwrap().contains("<svg"));
    assert_eq!(&std::fs::read(&png_out).unwrap()[0..4], b"\x89PNG");
}

#[test]
fn plot_error_rate_per_piece_covers_multi_point_bands_and_single_point_error_bars() {
    let multi_stats = vec![
        make_stat(0.008, 3, 9, 10_000, 120),
        make_stat(0.009, 3, 9, 10_000, 160),
        make_stat(0.010, 3, 9, 10_000, 220),
        make_stat(0.008, 5, 15, 10_000, 30),
        make_stat(0.009, 5, 15, 10_000, 45),
        make_stat(0.010, 5, 15, 10_000, 70),
    ];
    let single_stats = vec![make_stat(0.008, 3, 9, 10_000, 120)];
    let dir = tempdir().unwrap();
    let multi_out = dir.path().join("plot_per_piece_multi.svg");
    let single_out = dir.path().join("plot_per_piece_single.svg");

    plot_error_rate_per_piece(
        &multi_stats,
        |s| s.metadata["p"].as_f64().unwrap(),
        |s| format!("d={}", s.metadata["d"].as_u64().unwrap()),
        |s| s.metadata["r"].as_u64().unwrap() as f64,
        &multi_out,
    )
    .unwrap();
    plot_error_rate_per_piece(
        &single_stats,
        |s| s.metadata["p"].as_f64().unwrap(),
        |s| format!("d={}", s.metadata["d"].as_u64().unwrap()),
        |s| s.metadata["r"].as_u64().unwrap() as f64,
        &single_out,
    )
    .unwrap();

    let multi_svg = std::fs::read_to_string(&multi_out).unwrap();
    let single_svg = std::fs::read_to_string(&single_out).unwrap();
    assert!(multi_svg.contains("<svg"));
    assert!(single_svg.contains("<svg"));
}
