/// Generate a surface code threshold plot for the getting_started.md documentation.
///
/// Uses synthetic data representative of a rotated surface code under circuit-level
/// noise. This example exists to generate a static documentation asset; it does
/// not sample circuits or run a decoder.
use std::collections::HashMap;
use std::path::Path;

use rsinter::failure::FailureKind;
use rsinter::plot::plot_error_rate;
use rsinter::task_stats::TaskStats;

fn make_stat(p: f64, d: u64, r: u64, shots: u64, errors: u64) -> TaskStats {
    TaskStats {
        strong_id: String::new(),
        decoder: "synthetic".into(),
        metadata: serde_json::json!({"p": p, "d": d, "r": r}),
        shots,
        errors,
        discards: 0,
        seconds: 0.0,
        failure_kind: FailureKind::Ok,
        custom_counts: HashMap::new(),
    }
}

fn main() {
    // Synthetic data representative of rotated surface code under circuit-level noise.
    // shots=100_000; errors chosen so errors/shots approximates an illustrative curve.
    let stats = vec![
        // d=3, rounds=9
        make_stat(0.008, 3, 9,  100_000,  1_050),
        make_stat(0.009, 3, 9,  100_000,  1_290),
        make_stat(0.010, 3, 9,  100_000,  1_530),
        make_stat(0.011, 3, 9,  100_000,  1_830),
        make_stat(0.012, 3, 9,  100_000,  2_130),
        // d=5, rounds=15
        make_stat(0.008, 5, 15, 100_000,    310),
        make_stat(0.009, 5, 15, 100_000,    490),
        make_stat(0.010, 5, 15, 100_000,    780),
        make_stat(0.011, 5, 15, 100_000,  1_180),
        make_stat(0.012, 5, 15, 100_000,  1_660),
        // d=7, rounds=21
        make_stat(0.008, 7, 21, 100_000,     72),
        make_stat(0.009, 7, 21, 100_000,    193),
        make_stat(0.010, 7, 21, 100_000,    510),
        make_stat(0.011, 7, 21, 100_000,  1_029),
        make_stat(0.012, 7, 21, 100_000,  1_684),
    ];

    let out = Path::new("rstim/doc/surface_code_threshold.svg");
    plot_error_rate(
        &stats,
        |s| s.metadata["p"].as_f64().unwrap(),
        |s| format!("d={}", s.metadata["d"].as_u64().unwrap()),
        out,
    ).unwrap();

    println!("Written {}", out.display());
}
