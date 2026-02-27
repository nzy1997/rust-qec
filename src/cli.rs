use std::io::{self, Read, Write};

use clap::{Parser, Subcommand};
use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::dem::DetectorErrorModel;
use crate::error_analyzer::ErrorAnalyzer;
use crate::output::{
    OutputFormat, write_shots_01, write_shots_b8, write_shots_r8, write_shots_hits, write_shots_dets, write_shots_ptb64,
};
use crate::parser::parse_lines;
use crate::sampler::sample_batch;
use crate::sim::bit_table::BitTable;

#[derive(Parser)]
#[command(name = "rstim", version, about = "Rust stabilizer circuit simulator")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Sample measurement results from a circuit
    Sample {
        #[arg(long)]
        shots: Option<u64>,
        #[arg(long = "out_format", default_value = "01")]
        out_format: String,
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
        #[arg(long)]
        seed: Option<u64>,
    },
    /// Sample detection events and observable flips from a circuit
    Detect {
        #[arg(long)]
        shots: Option<u64>,
        #[arg(long = "out_format", default_value = "01")]
        out_format: String,
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long = "append_observables")]
        append_observables: bool,
    },
    /// Convert a circuit into a detector error model
    #[command(name = "analyze_errors")]
    AnalyzeErrors {
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
    },
    /// Generate a common QEC circuit
    #[command(name = "gen")]
    Gen {
        #[arg(long)]
        code: String,
        #[arg(long)]
        task: String,
        #[arg(long)]
        distance: usize,
        #[arg(long)]
        rounds: usize,
        #[arg(long = "after_clifford_depolarization", default_value = "0")]
        noise: f64,
        #[arg(long)]
        out: Option<String>,
    },
    /// Sample detection events from a detector error model
    #[command(name = "sample_dem")]
    SampleDem {
        #[arg(long)]
        shots: Option<u64>,
        #[arg(long = "out_format", default_value = "01")]
        out_format: String,
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long = "obs_out")]
        obs_out: Option<String>,
        #[arg(long = "obs_out_format", default_value = "01")]
        obs_out_format: String,
    },
}

pub fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Some(Commands::Sample { shots, out_format, r#in, out, seed }) => {
            let text = read_input(r#in.as_deref())?;
            let mut w = open_output(out.as_deref())?;
            run_sample(&text, shots.unwrap_or(1) as usize, &out_format, seed, &mut w)
        }
        Some(Commands::Detect { shots, out_format, r#in, out, seed, append_observables }) => {
            let text = read_input(r#in.as_deref())?;
            let mut w = open_output(out.as_deref())?;
            run_detect(&text, shots.unwrap_or(1) as usize, &out_format, seed, append_observables, &mut w)
        }
        Some(Commands::AnalyzeErrors { r#in, out }) => {
            let text = read_input(r#in.as_deref())?;
            let mut w = open_output(out.as_deref())?;
            run_analyze_errors(&text, &mut w)
        }
        Some(Commands::Gen { code, task, distance, rounds, noise, out }) => {
            let mut w = open_output(out.as_deref())?;
            run_gen(&code, &task, distance, rounds, noise, &mut w)
        }
        Some(Commands::SampleDem { shots, out_format, r#in, out, seed, obs_out, obs_out_format }) => {
            let text = read_input(r#in.as_deref())?;
            let mut w = open_output(out.as_deref())?;
            if let Some(obs_path) = obs_out.as_deref() {
                let mut obs_w = open_output(Some(obs_path))?;
                run_sample_dem_with_obs(
                    &text, shots.unwrap_or(1) as usize, &out_format, seed,
                    &mut w, &mut obs_w, &obs_out_format,
                )
            } else {
                run_sample_dem(&text, shots.unwrap_or(1) as usize, &out_format, seed, &mut w)
            }
        }
        None => {
            println!("rstim {}", crate::version());
            Ok(())
        }
    }
}

pub fn read_input(path: Option<&str>) -> Result<String, String> {
    match path {
        Some(p) => std::fs::read_to_string(p).map_err(|e| format!("failed to read {p}: {e}")),
        None => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf).map_err(|e| format!("failed to read stdin: {e}"))?;
            Ok(buf)
        }
    }
}

pub fn open_output(path: Option<&str>) -> Result<Box<dyn Write>, String> {
    match path {
        Some(p) => {
            let f = std::fs::File::create(p).map_err(|e| format!("failed to create {p}: {e}"))?;
            Ok(Box::new(io::BufWriter::new(f)))
        }
        None => Ok(Box::new(io::BufWriter::new(io::stdout().lock()))),
    }
}

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
        OutputFormat::Ptb64 => write_shots_ptb64(table, out),
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

pub fn run_gen(
    code: &str,
    task: &str,
    distance: usize,
    rounds: usize,
    noise: f64,
    out: &mut dyn Write,
) -> Result<(), String> {
    let instrs = match (code, task) {
        ("repetition_code", "memory") => crate::codegen::repetition_code_memory(distance, rounds, noise),
        _ => return Err(format!("unknown code/task: {code}/{task}")),
    };
    let circuit_text = crate::ir::circuit_to_string(&instrs);
    out.write_all(circuit_text.as_bytes()).map_err(|e| format!("write error: {e}"))
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
