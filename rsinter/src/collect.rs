use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use rand::rngs::StdRng;
use rand::SeedableRng;
use rayon::prelude::*;

use rstim::output::write_shots_b8;
use rstim::sampler::sample_batch;

use crate::csv_io;
use crate::decode::Decoder;
use crate::failure::{classify_completed, classify_error, FailureKind};
use crate::task::Task;
use crate::task_stats::TaskStats;

pub struct CollectOptions {
    pub num_workers: usize,
    pub max_shots: Option<u64>,
    pub max_errors: Option<u64>,
    pub max_wall_seconds: Option<f64>,
    pub max_batch_size: Option<usize>,
    pub start_batch_size: usize,
    pub save_resume_filepath: Option<PathBuf>,
    pub print_progress: bool,
}

fn validate_max_wall_seconds(max_wall_seconds: Option<f64>) -> Result<(), String> {
    if let Some(seconds) = max_wall_seconds {
        if !seconds.is_finite() || seconds <= 0.0 {
            return Err("max_wall_seconds must be positive".into());
        }
    }
    Ok(())
}

fn under_wall_budget(total_seconds: f64, max_wall_seconds: Option<f64>) -> bool {
    match max_wall_seconds {
        Some(max_seconds) => total_seconds < max_seconds,
        None => true,
    }
}

fn should_continue_collecting(
    total_shots: u64,
    total_errors: u64,
    total_seconds: f64,
    max_shots: u64,
    max_errors: u64,
    max_wall_seconds: Option<f64>,
) -> bool {
    total_shots < max_shots
        && total_errors < max_errors
        && under_wall_budget(total_seconds, max_wall_seconds)
}

fn make_task_stats(
    task: &Task,
    strong_id: String,
    shots: u64,
    errors: u64,
    seconds: f64,
    failure_kind: FailureKind,
) -> TaskStats {
    TaskStats {
        strong_id,
        decoder: task.decoder.clone(),
        metadata: task.metadata.clone(),
        shots,
        errors,
        discards: 0,
        seconds,
        failure_kind,
        custom_counts: HashMap::new(),
    }
}

fn collect_one_task(
    task: &Task,
    decoders: &HashMap<String, Box<dyn Decoder>>,
    existing: &HashMap<String, TaskStats>,
    options: &CollectOptions,
) -> Result<TaskStats, String> {
    let strong_id = task.strong_id();

    let mut total_shots: u64 = 0;
    let mut total_errors: u64 = 0;
    let mut total_seconds: f64 = 0.0;

    if let Some(prev) = existing.get(&strong_id) {
        total_shots = prev.shots;
        total_errors = prev.errors;
        total_seconds = prev.seconds;
    }

    let decoder = decoders
        .get(&task.decoder)
        .ok_or_else(|| format!("decoder not found: {}", task.decoder))?;
    let compiled = match decoder.compile_for_dem(&task.dem) {
        Ok(compiled) => compiled,
        Err(error) => {
            let failure_kind = classify_error(&error, FailureKind::SolverFailure);
            return Ok(make_task_stats(
                task,
                strong_id,
                total_shots,
                total_errors,
                total_seconds,
                failure_kind,
            ));
        }
    };

    let num_dets = task.dem.effective_num_detectors();
    let num_obs = task.dem.num_observables();
    let obs_bytes_per_shot = (num_obs + 7) / 8;

    let max_shots = task
        .collection_options
        .max_shots
        .or(options.max_shots)
        .unwrap_or(u64::MAX);
    let max_errors = task
        .collection_options
        .max_errors
        .or(options.max_errors)
        .unwrap_or(u64::MAX);
    let max_wall_seconds = task
        .collection_options
        .max_wall_seconds
        .or(options.max_wall_seconds);

    let mut batch_size = options.start_batch_size;
    let mut rng = StdRng::from_entropy();

    while should_continue_collecting(
        total_shots,
        total_errors,
        total_seconds,
        max_shots,
        max_errors,
        max_wall_seconds,
    ) {
        let remaining = (max_shots - total_shots) as usize;
        let n = batch_size.min(remaining);
        if n == 0 {
            break;
        }

        let batch_started = Instant::now();
        let batch = match sample_batch(&task.circuit, n, &mut rng) {
            Ok(batch) => batch,
            Err(_) => {
                return Ok(make_task_stats(
                    task,
                    strong_id,
                    total_shots,
                    total_errors,
                    total_seconds + batch_started.elapsed().as_secs_f64(),
                    FailureKind::SamplerError,
                ));
            }
        };

        let mut det_buf = Vec::new();
        if write_shots_b8(&batch.detections, &mut det_buf).is_err() {
            return Ok(make_task_stats(
                task,
                strong_id,
                total_shots,
                total_errors,
                total_seconds + batch_started.elapsed().as_secs_f64(),
                FailureKind::SamplerError,
            ));
        }
        let mut obs_buf = Vec::new();
        if write_shots_b8(&batch.observable_flips, &mut obs_buf).is_err() {
            return Ok(make_task_stats(
                task,
                strong_id,
                total_shots,
                total_errors,
                total_seconds + batch_started.elapsed().as_secs_f64(),
                FailureKind::SamplerError,
            ));
        }

        let predictions = match compiled.decode_shots_bit_packed(&det_buf, n, num_dets, num_obs) {
            Ok(predictions) => predictions,
            Err(error) => {
                let failure_kind = classify_error(&error, FailureKind::SolverFailure);
                return Ok(make_task_stats(
                    task,
                    strong_id,
                    total_shots,
                    total_errors,
                    total_seconds + batch_started.elapsed().as_secs_f64(),
                    failure_kind,
                ));
            }
        };

        let expected_len = n * obs_bytes_per_shot;
        if predictions.len() != expected_len {
            return Ok(make_task_stats(
                task,
                strong_id,
                total_shots,
                total_errors,
                total_seconds + batch_started.elapsed().as_secs_f64(),
                FailureKind::SolverFailure,
            ));
        }
        if obs_buf.len() != expected_len {
            return Ok(make_task_stats(
                task,
                strong_id,
                total_shots,
                total_errors,
                total_seconds + batch_started.elapsed().as_secs_f64(),
                FailureKind::SamplerError,
            ));
        }

        let mut batch_errors = 0u64;
        for shot in 0..n {
            let offset = shot * obs_bytes_per_shot;
            let mut mismatch = false;
            for byte in 0..obs_bytes_per_shot {
                if predictions[offset + byte] != obs_buf[offset + byte] {
                    mismatch = true;
                    break;
                }
            }
            if mismatch {
                batch_errors += 1;
            }
        }

        total_shots += n as u64;
        total_errors += batch_errors;
        total_seconds += batch_started.elapsed().as_secs_f64();

        if let Some(max) = options.max_batch_size {
            batch_size = (batch_size * 2).min(max);
        } else {
            batch_size *= 2;
        }
    }

    let timed_out = max_wall_seconds.is_some_and(|max_seconds| {
        total_seconds >= max_seconds && total_shots < max_shots && total_errors < max_errors
    });
    let failure_kind = classify_completed(total_errors, timed_out);

    Ok(make_task_stats(
        task,
        strong_id,
        total_shots,
        total_errors,
        total_seconds,
        failure_kind,
    ))
}

pub fn collect(
    tasks: Vec<Task>,
    decoders: HashMap<String, Box<dyn Decoder>>,
    options: &CollectOptions,
) -> Result<Vec<TaskStats>, String> {
    validate_max_wall_seconds(options.max_wall_seconds)?;
    for task in &tasks {
        validate_max_wall_seconds(task.collection_options.max_wall_seconds)?;
    }

    // Load existing data if resume path exists
    let mut existing: HashMap<String, TaskStats> = HashMap::new();
    if let Some(ref path) = options.save_resume_filepath {
        if path.exists() {
            let data = std::fs::read(path).map_err(|e| e.to_string())?;
            for s in csv_io::read_csv(&data)? {
                existing
                    .entry(s.strong_id.clone())
                    .and_modify(|e| {
                        let merged = e.clone() + s.clone();
                        *e = merged;
                    })
                    .or_insert(s);
            }
        }
    }

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(options.num_workers)
        .build()
        .map_err(|e| e.to_string())?;

    let results: Result<Vec<TaskStats>, String> = pool.install(|| {
        tasks
            .par_iter()
            .map(|task| collect_one_task(task, &decoders, &existing, options))
            .collect()
    });
    let results = results?;

    // Save to CSV if path specified
    if let Some(ref path) = options.save_resume_filepath {
        let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        csv_io::write_csv(&results, &mut file)?;
    }

    if options.print_progress {
        for r in &results {
            eprintln!(
                "[rsinter] {} shots={} errors={} ({:.2}s)",
                &r.strong_id[..8],
                r.shots,
                r.errors,
                r.seconds
            );
        }
    }

    Ok(results)
}
