use std::io::Write;

use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::dem::DetectorErrorModel;
use crate::error_analyzer::ErrorAnalyzer;
use crate::output::{
    OutputFormat, write_shots_01, write_shots_b8, write_shots_r8, write_shots_hits, write_shots_dets,
};
use crate::parser::parse_lines;
use crate::sampler::sample_batch;
use crate::sim::bit_table::BitTable;

pub fn make_rng(seed: Option<u64>) -> StdRng {
    match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    }
}

pub fn write_format(fmt: OutputFormat, table: &BitTable, out: &mut dyn Write) -> Result<(), String> {
    match fmt {
        OutputFormat::Format01 => write_shots_01(table, out),
        OutputFormat::B8 => write_shots_b8(table, out),
        OutputFormat::R8 => write_shots_r8(table, out),
        OutputFormat::Hits => write_shots_hits(table, out),
        OutputFormat::Dets => return Err("use write_shots_dets for dets format".to_string()),
    }.map_err(|e| format!("write error: {e}"))
}

pub fn merge_detections_observables(dets: &BitTable, obs: &BitTable) -> BitTable {
    let n_dets = dets.num_major();
    let n_obs = obs.num_major();
    let n_shots = dets.num_minor();
    let mut merged = BitTable::new(n_dets + n_obs, n_shots);
    for row in 0..n_dets {
        for shot in 0..n_shots {
            if dets.get(row, shot) { merged.set(row, shot, true); }
        }
    }
    for row in 0..n_obs {
        for shot in 0..n_shots {
            if obs.get(row, shot) { merged.set(n_dets + row, shot, true); }
        }
    }
    merged
}

pub fn run_sample(
    circuit_text: &str,
    shots: usize,
    out_format: &str,
    seed: Option<u64>,
    out: &mut dyn Write,
) -> Result<(), String> {
    let fmt = OutputFormat::from_str(out_format)?;
    let instrs = parse_lines(circuit_text)?;
    let mut rng = make_rng(seed);
    let result = sample_batch(&instrs, shots, &mut rng)?;
    match fmt {
        OutputFormat::Dets => Err("dets format not applicable to sample command; use detect".to_string()),
        _ => write_format(fmt, &result.measurements, out),
    }
}

pub fn run_detect(
    circuit_text: &str,
    shots: usize,
    out_format: &str,
    seed: Option<u64>,
    append_observables: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    let fmt = OutputFormat::from_str(out_format)?;
    let instrs = parse_lines(circuit_text)?;
    let mut rng = make_rng(seed);
    let result = sample_batch(&instrs, shots, &mut rng)?;
    match fmt {
        OutputFormat::Dets => {
            write_shots_dets(&result.detections, &result.observable_flips, out)
                .map_err(|e| format!("write error: {e}"))
        }
        _ => {
            if append_observables {
                let merged = merge_detections_observables(&result.detections, &result.observable_flips);
                write_format(fmt, &merged, out)
            } else {
                write_format(fmt, &result.detections, out)
            }
        }
    }
}

pub fn run_analyze_errors(
    circuit_text: &str,
    out: &mut dyn Write,
) -> Result<(), String> {
    let instrs = parse_lines(circuit_text)?;
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs)?;
    let dem_str = dem.to_string();
    out.write_all(dem_str.as_bytes()).map_err(|e| format!("write error: {e}"))
}

pub fn run_sample_dem(
    dem_text: &str,
    shots: usize,
    out_format: &str,
    seed: Option<u64>,
    out: &mut dyn Write,
) -> Result<(), String> {
    let fmt = OutputFormat::from_str(out_format)?;
    let dem = DetectorErrorModel::parse(dem_text)?;
    let mut rng = make_rng(seed);
    let result = dem.sample_batch(shots, &mut rng);
    match fmt {
        OutputFormat::Dets => {
            write_shots_dets(&result.detections, &result.observable_flips, out)
                .map_err(|e| format!("write error: {e}"))
        }
        _ => write_format(fmt, &result.detections, out),
    }
}

pub fn run_sample_dem_with_obs(
    dem_text: &str,
    shots: usize,
    out_format: &str,
    seed: Option<u64>,
    out: &mut dyn Write,
    obs_out: &mut dyn Write,
    obs_out_format: &str,
) -> Result<(), String> {
    let fmt = OutputFormat::from_str(out_format)?;
    let obs_fmt = OutputFormat::from_str(obs_out_format)?;
    let dem = DetectorErrorModel::parse(dem_text)?;
    let mut rng = make_rng(seed);
    let result = dem.sample_batch(shots, &mut rng);
    match fmt {
        OutputFormat::Dets => {
            write_shots_dets(&result.detections, &result.observable_flips, out)
                .map_err(|e| format!("write error: {e}"))?;
        }
        _ => {
            write_format(fmt, &result.detections, out)?;
        }
    }
    write_format(obs_fmt, &result.observable_flips, obs_out)
}
