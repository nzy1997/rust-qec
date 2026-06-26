use std::fs::File;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use rsinter::bb_circuit_memory::{
    SimulationConfig, export_comparison_case_for_code, run_simulation_for_code,
};
use rsinter::bench::merge::merge_result_rows;
use rsinter::bench::plot::render_benchmark_plot;
use rsinter::bench::registry::build_default_rust_runner_registry;
use rsinter::bench::result::{read_results_jsonl, write_results_jsonl};
use rsinter::bench::run::{BenchRunOptions, run_rust_benchmark_with_options};
use rsinter::bench::spec::BenchmarkSpec;
use rsinter::bench::surface_compare_csv::read_surface_compare_csv;

#[derive(Parser)]
#[command(
    name = "rsinter",
    version,
    about = "Rust benchmark and sampling harness",
    after_help = "bench subcommands: run, merge, plot, plot-surface-compare-csv"
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
    BbCircuitBposdMemory {
        #[arg(long, default_value = "bb144")]
        code_id: String,
        #[arg(long, default_value_t = 0.003, allow_hyphen_values = true)]
        physical_error_rate: f64,
        #[arg(long, default_value_t = 12)]
        num_cycles: usize,
        #[arg(long, default_value_t = 50_000)]
        num_trials: u64,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long, default_value_t = 10_000)]
        max_bp_iterations: usize,
        #[arg(long, default_value_t = 7)]
        osd_order: usize,
        #[arg(long)]
        json_compare_case: bool,
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
        #[arg(
            long,
            help = "Resume from existing per-runner test-run/results.jsonl rows under --out"
        )]
        resume: bool,
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
    PlotSurfaceCompareCsv {
        #[arg(long)]
        spec: String,
        #[arg(long)]
        input: String,
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
                resume,
            } => {
                let spec_path = PathBuf::from(&spec);
                let spec_dir = spec_path
                    .parent()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."));
                let text = std::fs::read_to_string(&spec_path).map_err(|e| e.to_string())?;
                let bench_spec: BenchmarkSpec = toml::from_str(&text).map_err(|e| e.to_string())?;
                bench_spec.validate()?;
                let registry = build_default_rust_runner_registry();
                run_rust_benchmark_with_options(
                    &bench_spec,
                    &language,
                    PathBuf::from(out).as_path(),
                    &registry,
                    &spec_dir,
                    BenchRunOptions { resume },
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
                let merged = merge_result_rows(row_sets)?;
                ensure_parent_dir(PathBuf::from(&out).as_path())?;
                let mut file = File::create(out).map_err(|e| e.to_string())?;
                write_results_jsonl(&merged, &mut file)?;
            }
            BenchCommands::Plot { spec, input, out } => {
                let text = std::fs::read_to_string(&spec).map_err(|e| e.to_string())?;
                let bench_spec: BenchmarkSpec = toml::from_str(&text).map_err(|e| e.to_string())?;
                bench_spec.validate()?;
                let mut rows = Vec::new();
                for path in input {
                    let data = std::fs::read(path).map_err(|e| e.to_string())?;
                    rows.extend(read_results_jsonl(&data[..])?);
                }
                ensure_parent_dir(PathBuf::from(&out).as_path())?;
                render_benchmark_plot(&bench_spec, &rows, PathBuf::from(out).as_path())?;
            }
            BenchCommands::PlotSurfaceCompareCsv { spec, input, out } => {
                let text = std::fs::read_to_string(&spec).map_err(|e| e.to_string())?;
                let bench_spec: BenchmarkSpec = toml::from_str(&text).map_err(|e| e.to_string())?;
                bench_spec.validate()?;
                let rows =
                    read_surface_compare_csv(PathBuf::from(input).as_path(), &bench_spec.name)?;
                ensure_parent_dir(PathBuf::from(&out).as_path())?;
                render_benchmark_plot(&bench_spec, &rows, PathBuf::from(out).as_path())?;
            }
        },
        Commands::BbCircuitBposdMemory {
            code_id,
            physical_error_rate,
            num_cycles,
            num_trials,
            seed,
            max_bp_iterations,
            osd_order,
            json_compare_case,
        } => {
            let num_trials = usize::try_from(num_trials)
                .map_err(|_| "num_trials exceeds supported platform usize".to_string())?;
            let config = SimulationConfig {
                physical_error_rate,
                num_cycles,
                num_trials,
                seed,
                max_bp_iterations,
                osd_order,
            };
            if json_compare_case {
                let export = export_comparison_case_for_code(&code_id, config)?;
                serde_json::to_writer_pretty(std::io::stdout(), &export)
                    .map_err(|e| e.to_string())?;
                println!();
            } else {
                let result = run_simulation_for_code(&code_id, config)?;
                println!(
                    "{}\t{}\t{}\t{}",
                    result.physical_error_rate,
                    result.num_cycles,
                    result.num_trials,
                    result.num_failed_trials
                );
            }
        }
    }
    Ok(())
}

fn ensure_parent_dir(path: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Ok(())
}
