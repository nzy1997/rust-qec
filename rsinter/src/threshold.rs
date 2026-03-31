use std::collections::HashMap;

use rstim::codegen::{NoiseParams, rotated_memory_z_with_params};
use rstim::error_analyzer::ErrorAnalyzer;

use crate::collect::{CollectOptions, collect};
use crate::decode::MwpmDecoder;
use crate::task::{CollectionOptions, Task};
use crate::task_stats::TaskStats;

// Match the surface-code sweep budget used in Stim's getting-started notebook:
// stop after 5,000 logical errors or 1,000,000 shots, whichever comes first.
pub const STIM_SURFACE_CODE_THRESHOLD_MAX_SHOTS: u64 = 1_000_000;
pub const STIM_SURFACE_CODE_THRESHOLD_MAX_ERRORS: u64 = 5_000;

fn rounds_for_distance(distance: usize) -> usize {
    distance * 3
}

pub fn stim_surface_code_threshold_collect_options(num_workers: usize) -> CollectOptions {
    CollectOptions {
        num_workers,
        max_shots: Some(STIM_SURFACE_CODE_THRESHOLD_MAX_SHOTS),
        max_errors: Some(STIM_SURFACE_CODE_THRESHOLD_MAX_ERRORS),
        max_batch_size: Some(16_384),
        start_batch_size: 256,
        save_resume_filepath: None,
        print_progress: true,
    }
}

pub fn make_rotated_surface_code_threshold_tasks(
    distances: &[usize],
    noises: &[f64],
    shots_per_task: u64,
) -> Result<Vec<Task>, String> {
    let mut tasks = Vec::new();

    for &distance in distances {
        let rounds = rounds_for_distance(distance);
        for &noise in noises {
            let circuit =
                rotated_memory_z_with_params(distance, rounds, NoiseParams::uniform(noise));
            let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit)?;
            tasks.push(Task {
                circuit,
                decoder: "mwpm".into(),
                dem,
                metadata: serde_json::json!({
                    "d": distance,
                    "p": noise,
                    "r": rounds,
                }),
                collection_options: CollectionOptions {
                    max_shots: Some(shots_per_task),
                    max_errors: None,
                },
            });
        }
    }

    Ok(tasks)
}

pub fn collect_rotated_surface_code_threshold(
    distances: &[usize],
    noises: &[f64],
    shots_per_task: u64,
    options: &CollectOptions,
) -> Result<Vec<TaskStats>, String> {
    let tasks = make_rotated_surface_code_threshold_tasks(distances, noises, shots_per_task)?;
    let mut decoders: HashMap<String, Box<dyn crate::decode::Decoder>> = HashMap::new();
    decoders.insert("mwpm".into(), Box::new(MwpmDecoder));
    collect(tasks, decoders, options)
}
