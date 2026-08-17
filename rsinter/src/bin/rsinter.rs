use std::fs::File;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
#[cfg(feature = "rbposd-runner")]
use rbposd::OsdVariant;

#[cfg(feature = "rbposd-runner")]
use rsinter::bb_circuit_memory::{
    export_bravyi_model_audit_for_code, export_comparison_case_for_code,
    export_comparison_case_for_code_with_osd_variant, run_simulation_for_code,
    run_simulation_for_code_with_osd_variant, SimulationConfig,
};
#[cfg(feature = "plotting")]
use rsinter::bench::bb_compare_csv::read_bb_compare_csv;
use rsinter::bench::merge::merge_result_rows;
#[cfg(feature = "plotting")]
use rsinter::bench::plot::render_benchmark_plot;
use rsinter::bench::registry::build_default_rust_runner_registry;
use rsinter::bench::result::{read_results_jsonl, write_results_jsonl};
use rsinter::bench::run::{run_rust_benchmark_with_options, BenchRunOptions};
use rsinter::bench::spec::BenchmarkSpec;
#[cfg(feature = "plotting")]
use rsinter::bench::surface_compare_csv::read_surface_compare_csv;
use rsinter::replay::{ReplayOptions, run_replay};

#[derive(Parser)]
#[command(
    name = "rsinter",
    version,
    about = "Rust benchmark and sampling harness",
    after_help = "bench subcommands: run, merge, plot, plot-surface-compare-csv, plot-bb-compare-csv"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Decode a frozen b8 detector file against a detector error model.
    Replay {
        #[arg(long)]
        dem: PathBuf,
        #[arg(long)]
        dets: PathBuf,
        #[arg(long)]
        decoder: String,
        #[arg(long)]
        decoder_config: Option<PathBuf>,
        #[arg(long)]
        predictions_out: PathBuf,
        #[arg(long)]
        stats_out: PathBuf,
        #[arg(long, default_value_t = 65_536)]
        batch_size: usize,
        #[arg(
            long,
            help = "Validate the inferred shot count; required for zero-detector DEMs"
        )]
        shots: Option<usize>,
    },
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
        osd_method: Option<String>,
        #[arg(long)]
        json_model_audit: bool,
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
    PlotBbCompareCsv {
        #[arg(long)]
        spec: String,
        #[arg(long)]
        input: String,
        #[arg(long)]
        out: String,
    },
}

#[cfg_attr(not(feature = "rbposd-runner"), allow(dead_code))]
struct BbCircuitBposdMemoryArgs {
    code_id: String,
    physical_error_rate: f64,
    num_cycles: usize,
    num_trials: u64,
    seed: Option<u64>,
    max_bp_iterations: usize,
    osd_order: usize,
    osd_method: Option<String>,
    json_model_audit: bool,
    json_compare_case: bool,
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
        Commands::Replay {
            dem,
            dets,
            decoder,
            decoder_config,
            predictions_out,
            stats_out,
            batch_size,
            shots,
        } => {
            run_replay(&ReplayOptions {
                dem,
                dets,
                decoder,
                decoder_config,
                predictions_out,
                stats_out,
                batch_size,
                shots,
            })?;
        }
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
                require_plotting_feature()?;
                run_benchmark_plot_command(spec, input, out)?;
            }
            BenchCommands::PlotSurfaceCompareCsv { spec, input, out } => {
                require_plotting_feature()?;
                run_surface_compare_plot_command(spec, input, out)?;
            }
            BenchCommands::PlotBbCompareCsv { spec, input, out } => {
                require_plotting_feature()?;
                run_bb_compare_plot_command(spec, input, out)?;
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
            osd_method,
            json_model_audit,
            json_compare_case,
        } => {
            run_bb_circuit_bposd_memory(BbCircuitBposdMemoryArgs {
                code_id,
                physical_error_rate,
                num_cycles,
                num_trials,
                seed,
                max_bp_iterations,
                osd_order,
                osd_method,
                json_model_audit,
                json_compare_case,
            })?;
        }
    }
    Ok(())
}

#[cfg(not(feature = "plotting"))]
fn require_plotting_feature() -> Result<(), String> {
    Err("requires Cargo feature 'plotting'".into())
}

#[cfg(feature = "plotting")]
fn require_plotting_feature() -> Result<(), String> {
    Ok(())
}

#[cfg(feature = "rbposd-runner")]
fn run_bb_circuit_bposd_memory(args: BbCircuitBposdMemoryArgs) -> Result<(), String> {
    let num_trials = usize::try_from(args.num_trials)
        .map_err(|_| "num_trials exceeds supported platform usize".to_string())?;
    let config = SimulationConfig {
        physical_error_rate: args.physical_error_rate,
        num_cycles: args.num_cycles,
        num_trials,
        seed: args.seed,
        max_bp_iterations: args.max_bp_iterations,
        osd_order: args.osd_order,
    };
    let osd_variant = match args.osd_method.as_deref() {
        Some(method) => Some(OsdVariant::from_method_name(method).map_err(|e| e.to_string())?),
        None => None,
    };
    if args.json_model_audit {
        let export = export_bravyi_model_audit_for_code(&args.code_id, config)?;
        serde_json::to_writer_pretty(std::io::stdout(), &export).map_err(|e| e.to_string())?;
        println!();
    } else if args.json_compare_case {
        let export = match osd_variant {
            Some(osd_variant) => export_comparison_case_for_code_with_osd_variant(
                &args.code_id,
                config,
                osd_variant,
            )?,
            None => export_comparison_case_for_code(&args.code_id, config)?,
        };
        serde_json::to_writer_pretty(std::io::stdout(), &export).map_err(|e| e.to_string())?;
        println!();
    } else {
        let result = match osd_variant {
            Some(osd_variant) => {
                run_simulation_for_code_with_osd_variant(&args.code_id, config, osd_variant)?
            }
            None => run_simulation_for_code(&args.code_id, config)?,
        };
        println!(
            "{}\t{}\t{}\t{}",
            result.physical_error_rate,
            result.num_cycles,
            result.num_trials,
            result.num_failed_trials
        );
    }
    Ok(())
}

#[cfg(not(feature = "rbposd-runner"))]
fn run_bb_circuit_bposd_memory(_args: BbCircuitBposdMemoryArgs) -> Result<(), String> {
    Err("requires Cargo feature 'rbposd-runner'".into())
}

#[cfg(feature = "plotting")]
fn run_benchmark_plot_command(spec: String, input: Vec<String>, out: String) -> Result<(), String> {
    let text = std::fs::read_to_string(&spec).map_err(|e| e.to_string())?;
    let bench_spec: BenchmarkSpec = toml::from_str(&text).map_err(|e| e.to_string())?;
    bench_spec.validate()?;
    let mut rows = Vec::new();
    for path in input {
        let data = std::fs::read(path).map_err(|e| e.to_string())?;
        rows.extend(read_results_jsonl(&data[..])?);
    }
    ensure_parent_dir(PathBuf::from(&out).as_path())?;
    render_benchmark_plot(&bench_spec, &rows, PathBuf::from(out).as_path())
}

#[cfg(not(feature = "plotting"))]
fn run_benchmark_plot_command(
    _spec: String,
    _input: Vec<String>,
    _out: String,
) -> Result<(), String> {
    Err("bench plot requires Cargo feature 'plotting'".into())
}

#[cfg(feature = "plotting")]
fn run_surface_compare_plot_command(
    spec: String,
    input: String,
    out: String,
) -> Result<(), String> {
    let text = std::fs::read_to_string(&spec).map_err(|e| e.to_string())?;
    let bench_spec: BenchmarkSpec = toml::from_str(&text).map_err(|e| e.to_string())?;
    bench_spec.validate()?;
    let rows = read_surface_compare_csv(PathBuf::from(input).as_path(), &bench_spec.name)?;
    ensure_parent_dir(PathBuf::from(&out).as_path())?;
    render_benchmark_plot(&bench_spec, &rows, PathBuf::from(out).as_path())
}

#[cfg(not(feature = "plotting"))]
fn run_surface_compare_plot_command(
    _spec: String,
    _input: String,
    _out: String,
) -> Result<(), String> {
    Err("bench plot-surface-compare-csv requires Cargo feature 'plotting'".into())
}

#[cfg(feature = "plotting")]
fn run_bb_compare_plot_command(spec: String, input: String, out: String) -> Result<(), String> {
    let text = std::fs::read_to_string(&spec).map_err(|e| e.to_string())?;
    let bench_spec: BenchmarkSpec = toml::from_str(&text).map_err(|e| e.to_string())?;
    bench_spec.validate()?;
    let rows = read_bb_compare_csv(PathBuf::from(input).as_path(), &bench_spec.name)?;
    ensure_parent_dir(PathBuf::from(&out).as_path())?;
    render_benchmark_plot(&bench_spec, &rows, PathBuf::from(out).as_path())
}

#[cfg(not(feature = "plotting"))]
fn run_bb_compare_plot_command(_spec: String, _input: String, _out: String) -> Result<(), String> {
    Err("bench plot-bb-compare-csv requires Cargo feature 'plotting'".into())
}

fn ensure_parent_dir(path: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Ok(())
}
