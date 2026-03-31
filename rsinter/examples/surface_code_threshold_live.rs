/// Collect and plot a real rotated surface-code threshold sweep.
///
/// This runs circuit generation, DEM extraction, MWPM decoding, and sampling.
/// The plotted y-axis is the per-round logical error rate inferred from the
/// per-shot logical error rate and the rounds count in task metadata.
use std::path::Path;

use rsinter::plot::plot_error_rate_per_piece;
use rsinter::threshold::{
    STIM_SURFACE_CODE_THRESHOLD_MAX_SHOTS,
    collect_rotated_surface_code_threshold,
    stim_surface_code_threshold_collect_options,
};

fn main() {
    let num_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    // Mirrors Stim's notebook collection settings: max_shots=1_000_000 and
    // max_errors=5_000, so easier points stop well before the shot cap.
    let stats = collect_rotated_surface_code_threshold(
        &[3, 5, 7],
        &[0.008, 0.009, 0.010, 0.011, 0.012],
        STIM_SURFACE_CODE_THRESHOLD_MAX_SHOTS,
        &stim_surface_code_threshold_collect_options(num_workers),
    )
    .unwrap();

    let out = Path::new("rstim/doc/surface_code_threshold_live.svg");
    plot_error_rate_per_piece(
        &stats,
        |s| s.metadata["p"].as_f64().unwrap(),
        |s| format!("d={}", s.metadata["d"].as_u64().unwrap()),
        |s| s.metadata["r"].as_u64().unwrap() as f64,
        out,
    )
    .unwrap();

    println!("Written {}", out.display());
}
