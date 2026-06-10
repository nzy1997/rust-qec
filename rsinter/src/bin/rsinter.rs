use std::fs::File;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use rsinter::bench::merge::merge_result_rows;
use rsinter::bench::plot::render_benchmark_plot;
use rsinter::bench::registry::build_default_rust_runner_registry;
use rsinter::bench::result::{read_results_jsonl, write_results_jsonl};
use rsinter::bench::run::run_rust_benchmark;
use rsinter::bench::spec::BenchmarkSpec;

#[derive(Parser)]
#[command(
    name = "rsinter",
    version,
    about = "Rust benchmark and sampling harness",
    after_help = "bench subcommands: run, merge, plot"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Bench {
        #[command(subcommand)]
        command: BenchCommands,
    },
}

#[derive(Subcommand)]
enum BenchCommands {
    Run {
        #[arg(long)]
        spec: String,
        #[arg(long)]
        language: String,
        #[arg(long)]
        out: String,
    },
    Merge {
        #[arg(long)]
        spec: String,
        #[arg(long = "input")]
        input: Vec<String>,
        #[arg(long)]
        out: String,
    },
    Plot {
        #[arg(long)]
        spec: String,
        #[arg(long = "input")]
        input: Vec<String>,
        #[arg(long)]
        out: String,
    },
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Bench { command } => match command {
            BenchCommands::Run {
                spec,
                language,
                out,
            } => {
                let text = std::fs::read_to_string(&spec).map_err(|e| e.to_string())?;
                let bench_spec: BenchmarkSpec = toml::from_str(&text).map_err(|e| e.to_string())?;
                bench_spec.validate()?;
                let registry = build_default_rust_runner_registry();
                run_rust_benchmark(
                    &bench_spec,
                    &language,
                    PathBuf::from(out).as_path(),
                    &registry,
                )?;
            }
            BenchCommands::Merge {
                spec: _,
                input,
                out,
            } => {
                let mut row_sets = Vec::new();
                for path in input {
                    let data = std::fs::read(path).map_err(|e| e.to_string())?;
                    row_sets.push(read_results_jsonl(&data[..])?);
                }
                let merged = merge_result_rows(row_sets);
                ensure_parent_dir(PathBuf::from(&out).as_path())?;
                let mut file = File::create(out).map_err(|e| e.to_string())?;
                write_results_jsonl(&merged, &mut file)?;
            }
            BenchCommands::Plot { spec, input, out } => {
                let text = std::fs::read_to_string(&spec).map_err(|e| e.to_string())?;
                let bench_spec: BenchmarkSpec = toml::from_str(&text).map_err(|e| e.to_string())?;
                let mut rows = Vec::new();
                for path in input {
                    let data = std::fs::read(path).map_err(|e| e.to_string())?;
                    rows.extend(read_results_jsonl(&data[..])?);
                }
                ensure_parent_dir(PathBuf::from(&out).as_path())?;
                render_benchmark_plot(&bench_spec, &rows, PathBuf::from(out).as_path())?;
            }
        },
    }
    Ok(())
}

fn ensure_parent_dir(path: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Ok(())
}
