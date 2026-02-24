use std::io::{self, Read, Write};

use clap::{Parser, Subcommand};
use rand::SeedableRng;
use rand::rngs::StdRng;

use rstim::dem::DetectorErrorModel;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::output::{
    OutputFormat, write_shots_01, write_shots_b8, write_shots_r8, write_shots_hits, write_shots_dets,
};
use rstim::parser::parse_lines;
use rstim::sampler::sample_batch;
use rstim::sim::bit_table::BitTable;

#[derive(Parser)]
#[command(name = "rstim", version, about = "Rust stabilizer circuit simulator")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
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

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Some(Commands::Sample { shots, out_format, r#in, out, seed }) => {
            cmd_sample(shots.unwrap_or(1), &out_format, r#in.as_deref(), out.as_deref(), seed)
        }
        Some(Commands::Detect { shots, out_format, r#in, out, seed, append_observables }) => {
            cmd_detect(shots.unwrap_or(1), &out_format, r#in.as_deref(), out.as_deref(), seed, append_observables)
        }
        Some(Commands::AnalyzeErrors { r#in, out }) => {
            cmd_analyze_errors(r#in.as_deref(), out.as_deref())
        }
        Some(Commands::SampleDem { shots, out_format, r#in, out, seed, obs_out, obs_out_format }) => {
            cmd_sample_dem(
                shots.unwrap_or(1), &out_format, r#in.as_deref(), out.as_deref(),
                seed, obs_out.as_deref(), &obs_out_format,
            )
        }
        None => {
            println!("rstim {}", rstim::version());
            Ok(())
        }
    }
}

fn read_input(path: Option<&str>) -> Result<String, String> {
    match path {
        Some(p) => std::fs::read_to_string(p).map_err(|e| format!("failed to read {p}: {e}")),
        None => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf).map_err(|e| format!("failed to read stdin: {e}"))?;
            Ok(buf)
        }
    }
}

fn make_rng(seed: Option<u64>) -> StdRng {
    match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    }
}

fn open_output(path: Option<&str>) -> Result<Box<dyn Write>, String> {
    match path {
        Some(p) => {
            let f = std::fs::File::create(p).map_err(|e| format!("failed to create {p}: {e}"))?;
            Ok(Box::new(io::BufWriter::new(f)))
        }
        None => Ok(Box::new(io::BufWriter::new(io::stdout().lock()))),
    }
}

fn write_format(fmt: OutputFormat, table: &BitTable, out: &mut impl Write) -> Result<(), String> {
    match fmt {
        OutputFormat::Format01 => write_shots_01(table, out),
        OutputFormat::B8 => write_shots_b8(table, out),
        OutputFormat::R8 => write_shots_r8(table, out),
        OutputFormat::Hits => write_shots_hits(table, out),
        OutputFormat::Dets => return Err("use write_shots_dets for dets format".to_string()),
    }.map_err(|e| format!("write error: {e}"))
}

fn merge_detections_observables(dets: &BitTable, obs: &BitTable) -> BitTable {
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

fn cmd_sample(
    shots: u64,
    out_format: &str,
    in_path: Option<&str>,
    out_path: Option<&str>,
    seed: Option<u64>,
) -> Result<(), String> {
    let fmt = OutputFormat::from_str(out_format)?;
    let circuit_text = read_input(in_path)?;
    let instrs = parse_lines(&circuit_text)?;
    let mut rng = make_rng(seed);
    let result = sample_batch(&instrs, shots as usize, &mut rng)?;
    let mut out = open_output(out_path)?;
    match fmt {
        OutputFormat::Dets => return Err("dets format not applicable to sample command; use detect".to_string()),
        _ => write_format(fmt, &result.measurements, &mut out),
    }
}

fn cmd_detect(
    shots: u64,
    out_format: &str,
    in_path: Option<&str>,
    out_path: Option<&str>,
    seed: Option<u64>,
    append_observables: bool,
) -> Result<(), String> {
    let fmt = OutputFormat::from_str(out_format)?;
    let circuit_text = read_input(in_path)?;
    let instrs = parse_lines(&circuit_text)?;
    let mut rng = make_rng(seed);
    let result = sample_batch(&instrs, shots as usize, &mut rng)?;
    let mut out = open_output(out_path)?;
    match fmt {
        OutputFormat::Dets => {
            write_shots_dets(&result.detections, &result.observable_flips, &mut out)
                .map_err(|e| format!("write error: {e}"))
        }
        _ => {
            if append_observables {
                let merged = merge_detections_observables(&result.detections, &result.observable_flips);
                write_format(fmt, &merged, &mut out)
            } else {
                write_format(fmt, &result.detections, &mut out)
            }
        }
    }
}

fn cmd_analyze_errors(
    in_path: Option<&str>,
    out_path: Option<&str>,
) -> Result<(), String> {
    let circuit_text = read_input(in_path)?;
    let instrs = parse_lines(&circuit_text)?;
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs)?;
    let dem_str = dem.to_string();
    let mut out = open_output(out_path)?;
    out.write_all(dem_str.as_bytes()).map_err(|e| format!("write error: {e}"))
}

fn cmd_sample_dem(
    shots: u64,
    out_format: &str,
    in_path: Option<&str>,
    out_path: Option<&str>,
    seed: Option<u64>,
    obs_out: Option<&str>,
    obs_out_format: &str,
) -> Result<(), String> {
    let fmt = OutputFormat::from_str(out_format)?;
    let dem_text = read_input(in_path)?;
    let dem = DetectorErrorModel::parse(&dem_text)?;
    let mut rng = make_rng(seed);
    let result = dem.sample_batch(shots as usize, &mut rng);
    let mut out = open_output(out_path)?;
    match fmt {
        OutputFormat::Dets => {
            write_shots_dets(&result.detections, &result.observable_flips, &mut out)
                .map_err(|e| format!("write error: {e}"))?;
        }
        _ => {
            write_format(fmt, &result.detections, &mut out)?;
        }
    }
    if let Some(obs_path) = obs_out {
        let obs_fmt = OutputFormat::from_str(obs_out_format)?;
        let mut obs_writer = open_output(Some(obs_path))?;
        write_format(obs_fmt, &result.observable_flips, &mut obs_writer)?;
    }
    Ok(())
}
