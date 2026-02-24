use std::io::{self, Read, Write};

use clap::{Parser, Subcommand};
use rand::SeedableRng;
use rand::rngs::StdRng;

use rstim::output::{OutputFormat, write_shots_01, write_shots_b8, write_shots_r8, write_shots_hits};
use rstim::parser::parse_lines;
use rstim::sampler::sample_batch;

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
        None => {
            println!("rstim {}", rstim::version());
            Ok(())
        }
    }
}

fn read_circuit(path: Option<&str>) -> Result<String, String> {
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

fn cmd_sample(
    shots: u64,
    out_format: &str,
    in_path: Option<&str>,
    out_path: Option<&str>,
    seed: Option<u64>,
) -> Result<(), String> {
    let fmt = OutputFormat::from_str(out_format)?;
    let circuit_text = read_circuit(in_path)?;
    let instrs = parse_lines(&circuit_text)?;
    let mut rng = make_rng(seed);
    let result = sample_batch(&instrs, shots as usize, &mut rng)?;
    let mut out = open_output(out_path)?;
    match fmt {
        OutputFormat::Format01 => write_shots_01(&result.measurements, &mut out),
        OutputFormat::B8 => write_shots_b8(&result.measurements, &mut out),
        OutputFormat::R8 => write_shots_r8(&result.measurements, &mut out),
        OutputFormat::Hits => write_shots_hits(&result.measurements, &mut out),
        OutputFormat::Dets => return Err("dets format not applicable to sample command; use detect".to_string()),
    }.map_err(|e| format!("write error: {e}"))
}
