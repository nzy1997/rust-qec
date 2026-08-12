use std::time::Instant;

use rstim::interactive_shot::{EditableShot, ExpansionLimits, NoiseOutcome};
use serde_json::json;

fn main() -> Result<(), String> {
    let mut rows = Vec::new();
    for repeat in [1_u64, 10, 50] {
        let source = format!(
            "R 0 1 2 3\nREPEAT {repeat} {{\n DEPOLARIZE1(0.1) 0 1\n DEPOLARIZE2(0.1) 2 3\n M 0 1 2 3\n DETECTOR rec[-4] rec[-3]\n DETECTOR rec[-2] rec[-1]\n R 0 1 2 3\n}}\n"
        );
        let open_start = Instant::now();
        let mut shot = EditableShot::open(&source, ExpansionLimits::default(), 7)?;
        let open_ms = open_start.elapsed().as_secs_f64() * 1_000.0;
        let snapshot_start = Instant::now();
        let snapshot = shot.view_snapshot()?;
        let snapshot_ms = snapshot_start.elapsed().as_secs_f64() * 1_000.0;
        let edit_id = shot.session().catalog().events()[0].id.clone();
        let edit_start = Instant::now();
        shot.set_noise(&edit_id, NoiseOutcome::X)?;
        let edited = shot.view_snapshot()?;
        let edit_snapshot_ms = edit_start.elapsed().as_secs_f64() * 1_000.0;
        rows.push(json!({
            "repeat": repeat,
            "expanded_operations": shot.session().expansion().operations,
            "noise_events": shot.session().expansion().noise_events,
            "measurements": shot.session().expansion().measurements,
            "open_ms": open_ms,
            "snapshot_ms": snapshot_ms,
            "edit_and_snapshot_ms": edit_snapshot_ms,
            "svg_bytes": snapshot.svg.len(),
            "edited_svg_bytes": edited.svg.len(),
        }));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "format_version": "rstim-interactive-profile-v1",
            "rows": rows,
        }))
        .map_err(|error| error.to_string())?
    );
    Ok(())
}
