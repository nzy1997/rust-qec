use std::io::{self, Read, Write};

use clap::{Parser, Subcommand};
use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::dem::DetectorErrorModel;
use crate::error_analyzer::ErrorAnalyzer;
use crate::executor::Executor;
use crate::m2d::{M2dOptions, measurements_to_detections_with_options};
use crate::output::{
    OutputFormat, write_shots_01, write_shots_b8, write_shots_dets, write_shots_hits,
    write_shots_ptb64, write_shots_r8,
};
use crate::parser::parse_lines;
use crate::sampler::{SampleOptions, sample_batch, sample_batch_with_options};
use crate::sim::bit_table::BitTable;

#[derive(Parser)]
#[command(name = "rstim", version, about = "Rust stabilizer circuit simulator")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Summarize circuit structure and counts
    Stats {
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
        #[arg(long)]
        json: bool,
    },
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
        #[arg(long = "skip_reference_sample")]
        skip_reference_sample: bool,
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
        #[arg(long = "obs_out")]
        obs_out: Option<String>,
        #[arg(long = "obs_out_format", default_value = "01")]
        obs_out_format: String,
    },
    /// Convert a circuit into a detector error model
    #[command(name = "analyze_errors")]
    AnalyzeErrors {
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
        #[arg(long = "approximate_disjoint_errors")]
        approximate_disjoint_errors: bool,
        #[arg(long = "allow_gauge_detectors")]
        allow_gauge_detectors: bool,
        #[arg(long = "decompose_errors")]
        decompose_errors: bool,
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
    /// Convert shot data between output formats
    #[command(name = "convert")]
    Convert {
        #[arg(long = "in_format", default_value = "01")]
        in_format: String,
        #[arg(long = "out_format", default_value = "01")]
        out_format: String,
        #[arg(long)]
        bits: Option<usize>,
        #[arg(long)]
        circuit: Option<String>,
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
        #[arg(long)]
        shots: Option<usize>,
    },
    /// Convert measurement results to detection events
    #[command(name = "m2d")]
    M2d {
        #[arg(long = "in_format", default_value = "01")]
        in_format: String,
        #[arg(long = "out_format", default_value = "dets")]
        out_format: String,
        #[arg(long)]
        circuit: Option<String>,
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
        #[arg(long = "append_observables")]
        append_observables: bool,
        #[arg(long = "skip_reference_sample")]
        skip_reference_sample: bool,
        #[arg(long = "sweep")]
        sweep: Option<String>,
        #[arg(long = "sweep_format", default_value = "01")]
        sweep_format: String,
        #[arg(long = "ran_without_feedback")]
        ran_without_feedback: bool,
        #[arg(long)]
        shots: Option<usize>,
    },
    /// Explain which errors could have caused observed detection events
    #[command(name = "explain_errors")]
    ExplainErrors {
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long = "in_format", default_value = "dets")]
        in_format: String,
        #[arg(long)]
        circuit: Option<String>,
        #[arg(long)]
        dem: Option<String>,
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
    /// Export a circuit as QP101 JSON
    #[command(name = "export_json")]
    ExportJson {
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
        #[arg(long, default_value = "pretty")]
        format: String,
        #[arg(long = "highlight_dem_error")]
        highlight_dem_error: Option<usize>,
        #[arg(long = "sample_shot")]
        sample_shot: bool,
        #[arg(long)]
        seed: Option<u64>,
    },
    /// Run performance evidence workflows
    Perf {
        #[command(subcommand)]
        command: PerfCommands,
    },
}

#[derive(Subcommand)]
pub enum PerfCommands {
    /// Emit raw benchmark JSONL
    Run {
        #[arg(long)]
        out: Option<String>,
        #[arg(long, default_value_t = 1)]
        warmup_rounds: usize,
        #[arg(long, default_value_t = 5)]
        measure_rounds: usize,
    },
    /// Aggregate raw JSONL into summary JSON
    Summarize {
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
    },
    /// Evaluate summary JSON and return non-zero on gate failure
    Gate {
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long, default_value_t = 1.10)]
        sampler_threshold: f64,
        #[arg(long, default_value_t = 1.10)]
        analyzer_threshold: f64,
    },
    /// Render summary JSON as Markdown
    Report {
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
    },
    /// Run the full perf pipeline and write raw / summary / report artifacts
    Ci {
        #[arg(long = "out-dir")]
        out_dir: String,
        #[arg(long, default_value_t = 1)]
        warmup_rounds: usize,
        #[arg(long, default_value_t = 5)]
        measure_rounds: usize,
    },
}

#[derive(Clone, Copy)]
enum JsonOutputFormat {
    Pretty,
    Compact,
}

enum PerfCiError {
    Infrastructure(String),
    Gate(String),
}

fn parse_json_output_format(format: &str) -> Result<JsonOutputFormat, String> {
    match format {
        "pretty" => Ok(JsonOutputFormat::Pretty),
        "compact" => Ok(JsonOutputFormat::Compact),
        other => Err(format!("unknown json format: {other}")),
    }
}

pub fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Some(Commands::Stats { r#in, out, json }) => {
            let text = read_input(r#in.as_deref())?;
            let mut w = open_output(out.as_deref())?;
            run_stats(&text, json, &mut w)
        }
        Some(Commands::Sample {
            shots,
            out_format,
            r#in,
            out,
            seed,
            skip_reference_sample,
        }) => {
            let text = read_input(r#in.as_deref())?;
            let mut w = open_output(out.as_deref())?;
            run_sample(
                &text,
                shots.unwrap_or(1) as usize,
                &out_format,
                seed,
                skip_reference_sample,
                &mut w,
            )
        }
        Some(Commands::Detect {
            shots,
            out_format,
            r#in,
            out,
            seed,
            append_observables,
            obs_out,
            obs_out_format,
        }) => {
            let text = read_input(r#in.as_deref())?;
            let mut w = open_output(out.as_deref())?;
            if let Some(obs_path) = obs_out.as_deref() {
                let mut obs_w = open_output(Some(obs_path))?;
                run_detect_with_obs(
                    &text,
                    shots.unwrap_or(1) as usize,
                    &out_format,
                    seed,
                    append_observables,
                    &mut w,
                    &mut obs_w,
                    &obs_out_format,
                )
            } else {
                run_detect(
                    &text,
                    shots.unwrap_or(1) as usize,
                    &out_format,
                    seed,
                    append_observables,
                    &mut w,
                )
            }
        }
        Some(Commands::AnalyzeErrors {
            r#in,
            out,
            approximate_disjoint_errors,
            allow_gauge_detectors,
            decompose_errors,
        }) => {
            let text = read_input(r#in.as_deref())?;
            let mut w = open_output(out.as_deref())?;
            run_analyze_errors_with_flags(
                &text,
                approximate_disjoint_errors,
                allow_gauge_detectors,
                decompose_errors,
                &mut w,
            )
        }
        Some(Commands::Gen {
            code,
            task,
            distance,
            rounds,
            noise,
            out,
        }) => {
            let mut w = open_output(out.as_deref())?;
            run_gen(&code, &task, distance, rounds, noise, &mut w)
        }
        Some(Commands::Convert {
            in_format,
            out_format,
            bits,
            circuit,
            r#in,
            out,
            shots,
        }) => {
            let data = read_input_bytes(r#in.as_deref())?;
            let mut w = open_output(out.as_deref())?;
            run_convert(
                &data,
                &in_format,
                &out_format,
                bits,
                circuit.as_deref(),
                shots,
                &mut w,
            )
        }
        Some(Commands::M2d {
            in_format,
            out_format,
            circuit,
            r#in,
            out,
            append_observables,
            skip_reference_sample,
            sweep,
            sweep_format,
            ran_without_feedback,
            shots,
        }) => {
            let circ_text = read_input(circuit.as_deref())?;
            let data = read_input_bytes(r#in.as_deref())?;
            let sweep_data = sweep
                .as_deref()
                .map(|path| read_input_bytes(Some(path)))
                .transpose()?;
            let mut w = open_output(out.as_deref())?;
            let options = M2dOptions {
                reference_sample_mode: if skip_reference_sample {
                    crate::data_path::ReferenceSampleMode::AssumeAllZero
                } else {
                    crate::data_path::ReferenceSampleMode::SimulateNoiseless
                },
                ran_without_feedback,
            };
            run_m2d_impl(
                &circ_text,
                &data,
                &in_format,
                &out_format,
                shots,
                sweep_data
                    .as_deref()
                    .map(|data| (data, sweep_format.as_str())),
                options,
                append_observables,
                &mut w,
            )
        }
        Some(Commands::ExplainErrors {
            r#in,
            in_format,
            circuit,
            dem,
            out,
        }) => {
            let det_data = read_input_bytes(r#in.as_deref())?;
            let circuit_text = circuit
                .as_deref()
                .map(|p| std::fs::read_to_string(p).map_err(|e| e.to_string()))
                .transpose()?;
            let dem_text = dem
                .as_deref()
                .map(|p| std::fs::read_to_string(p).map_err(|e| e.to_string()))
                .transpose()?;
            let mut w = open_output(out.as_deref())?;
            run_explain_errors(
                circuit_text.as_deref().unwrap_or(""),
                dem_text.as_deref(),
                &det_data,
                &in_format,
                &mut w,
            )
        }
        Some(Commands::SampleDem {
            shots,
            out_format,
            r#in,
            out,
            seed,
            obs_out,
            obs_out_format,
        }) => {
            let text = read_input(r#in.as_deref())?;
            let mut w = open_output(out.as_deref())?;
            if let Some(obs_path) = obs_out.as_deref() {
                let mut obs_w = open_output(Some(obs_path))?;
                run_sample_dem_with_obs(
                    &text,
                    shots.unwrap_or(1) as usize,
                    &out_format,
                    seed,
                    &mut w,
                    &mut obs_w,
                    &obs_out_format,
                )
            } else {
                run_sample_dem(
                    &text,
                    shots.unwrap_or(1) as usize,
                    &out_format,
                    seed,
                    &mut w,
                )
            }
        }
        Some(Commands::ExportJson {
            r#in,
            out,
            format,
            highlight_dem_error,
            sample_shot,
            seed,
        }) => {
            let text = read_input(r#in.as_deref())?;
            let format = parse_json_output_format(&format)?;
            let mut w = open_output(out.as_deref())?;
            run_export_json(&text, format, highlight_dem_error, sample_shot, seed, &mut w)
        }
        Some(Commands::Perf { command }) => match command {
            PerfCommands::Run {
                out,
                warmup_rounds,
                measure_rounds,
            } => {
                let mut w = open_output(out.as_deref())?;
                crate::perf::run_benchmark_suite_to_writer(
                    &mut w,
                    crate::perf::PerfRunOptions {
                        warmup_rounds,
                        measured_rounds: measure_rounds,
                    },
                )
            }
            PerfCommands::Summarize { r#in, out } => {
                let raw = read_input(r#in.as_deref())?;
                let summary = crate::perf::summarize_jsonl_str(&raw)?;
                let mut w = open_output(out.as_deref())?;
                serde_json::to_writer_pretty(&mut *w, &summary)
                    .map_err(|e| format!("failed to write perf summary: {e}"))?;
                w.write_all(b"\n")
                    .map_err(|e| format!("failed to write perf summary newline: {e}"))
            }
            PerfCommands::Gate {
                r#in,
                sampler_threshold,
                analyzer_threshold,
            } => {
                let text = read_input(r#in.as_deref())?;
                let summary: crate::perf::PerfSummary = serde_json::from_str(&text)
                    .map_err(|e| format!("failed to parse perf summary: {e}"))?;
                let verdict = crate::perf::evaluate_summary(
                    &summary,
                    crate::perf::PerfGateConfig {
                        sampler_ratio_threshold: sampler_threshold,
                        analyzer_ratio_threshold: analyzer_threshold,
                    },
                );
                if verdict.status == crate::perf::PerfGateStatus::Pass {
                    Ok(())
                } else {
                    Err(verdict.summary_markdown())
                }
            }
            PerfCommands::Report { r#in, out } => {
                let text = read_input(r#in.as_deref())?;
                let summary: crate::perf::PerfSummary = serde_json::from_str(&text)
                    .map_err(|e| format!("failed to parse perf summary: {e}"))?;
                let report = crate::perf::render_markdown_report(&summary, None);
                let mut w = open_output(out.as_deref())?;
                w.write_all(report.as_bytes())
                    .map_err(|e| format!("failed to write perf report: {e}"))
            }
            PerfCommands::Ci {
                out_dir,
                warmup_rounds,
                measure_rounds,
            } => run_perf_ci(&out_dir, warmup_rounds, measure_rounds).map_err(|e| match e {
                PerfCiError::Infrastructure(message) => {
                    format!("InfrastructureFailure\n- {message}")
                }
                PerfCiError::Gate(message) => message,
            }),
        },
        None => {
            println!("rstim {}", crate::version());
            Ok(())
        }
    }
}

fn run_perf_ci(
    out_dir: &str,
    warmup_rounds: usize,
    measure_rounds: usize,
) -> Result<(), PerfCiError> {
    std::fs::create_dir_all(out_dir)
        .map_err(|e| PerfCiError::Infrastructure(format!("failed to create perf out dir {out_dir}: {e}")))?;

    let raw_path = std::path::Path::new(out_dir).join("raw.jsonl");
    let summary_path = std::path::Path::new(out_dir).join("summary.json");
    let report_path = std::path::Path::new(out_dir).join("report.md");

    write_perf_ci_raw_artifact(&raw_path, warmup_rounds, measure_rounds)?;

    let raw_text = std::fs::read_to_string(&raw_path)
        .map_err(|e| PerfCiError::Infrastructure(format!(
            "failed to read raw perf artifact {}: {e}",
            raw_path.display()
        )))?;
    finalize_perf_ci_artifacts(
        &raw_text,
        &summary_path,
        &report_path,
        crate::perf::PerfGateConfig::default(),
    )
}

fn write_perf_ci_raw_artifact(
    raw_path: &std::path::Path,
    warmup_rounds: usize,
    measure_rounds: usize,
) -> Result<(), PerfCiError> {
    if let Ok(source_path) = std::env::var("RSTIM_TEST_PERF_CI_RAW") {
        std::fs::copy(&source_path, raw_path).map_err(|e| {
            PerfCiError::Infrastructure(format!(
                "failed to copy test perf raw artifact from {source_path} to {}: {e}",
                raw_path.display()
            ))
        })?;
        return Ok(());
    }

    let mut raw = open_output(raw_path.to_str()).map_err(PerfCiError::Infrastructure)?;
    crate::perf::run_benchmark_suite_to_writer(
        &mut raw,
        crate::perf::PerfRunOptions {
            warmup_rounds,
            measured_rounds: measure_rounds,
        },
    )
    .map_err(PerfCiError::Infrastructure)
}

fn finalize_perf_ci_artifacts(
    raw_text: &str,
    summary_path: &std::path::Path,
    report_path: &std::path::Path,
    config: crate::perf::PerfGateConfig,
) -> Result<(), PerfCiError> {
    let summary = crate::perf::summarize_jsonl_str(raw_text).map_err(PerfCiError::Infrastructure)?;
    let verdict = crate::perf::evaluate_summary(&summary, config);

    std::fs::write(
        summary_path,
        serde_json::to_string_pretty(&summary)
            .map_err(|e| PerfCiError::Infrastructure(format!("failed to serialize perf summary: {e}")))?,
    )
    .map_err(|e| PerfCiError::Infrastructure(format!(
        "failed to write perf summary {}: {e}",
        summary_path.display()
    )))?;

    let report = crate::perf::render_markdown_report(&summary, Some(&verdict.summary_markdown()));
    std::fs::write(report_path, report).map_err(|e| {
        PerfCiError::Infrastructure(format!(
            "failed to write perf report {}: {e}",
            report_path.display()
        ))
    })?;

    if verdict.status == crate::perf::PerfGateStatus::Pass {
        Ok(())
    } else {
        Err(PerfCiError::Gate(verdict.summary_markdown()))
    }
}

pub fn read_input(path: Option<&str>) -> Result<String, String> {
    match path {
        Some(p) => std::fs::read_to_string(p).map_err(|e| format!("failed to read {p}: {e}")),
        None => {
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| format!("failed to read stdin: {e}"))?;
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

pub fn write_format(
    fmt: OutputFormat,
    table: &BitTable,
    out: &mut dyn Write,
) -> Result<(), String> {
    match fmt {
        OutputFormat::Format01 => write_shots_01(table, out),
        OutputFormat::B8 => write_shots_b8(table, out),
        OutputFormat::R8 => write_shots_r8(table, out),
        OutputFormat::Hits => write_shots_hits(table, out),
        OutputFormat::Dets => return Err("use write_shots_dets for dets format".to_string()),
        OutputFormat::Ptb64 => write_shots_ptb64(table, out),
    }
    .map_err(|e| format!("write error: {e}"))
}

pub fn merge_detections_observables(dets: &BitTable, obs: &BitTable) -> BitTable {
    let n_dets = dets.num_major();
    let n_obs = obs.num_major();
    let n_shots = dets.num_minor();
    let mut merged = BitTable::new(n_dets + n_obs, n_shots);
    for row in 0..n_dets {
        for shot in 0..n_shots {
            if dets.get(row, shot) {
                merged.set(row, shot, true);
            }
        }
    }
    for row in 0..n_obs {
        for shot in 0..n_shots {
            if obs.get(row, shot) {
                merged.set(n_dets + row, shot, true);
            }
        }
    }
    merged
}

pub fn run_stats(text: &str, json: bool, out: &mut dyn Write) -> Result<(), String> {
    let instrs = parse_lines(text)?;
    let summary = crate::stats::summarize(&instrs);
    if json {
        serde_json::to_writer_pretty(&mut *out, &summary)
            .map_err(|e| format!("write error: {e}"))?;
        out.write_all(b"\n")
            .map_err(|e| format!("write error: {e}"))?;
        return Ok(());
    }

    writeln!(out, "instruction_count: {}", summary.instruction_count).map_err(|e| e.to_string())?;
    writeln!(out, "repeat_blocks: {}", summary.repeat_blocks).map_err(|e| e.to_string())?;
    writeln!(out, "max_repeat_depth: {}", summary.max_repeat_depth).map_err(|e| e.to_string())?;
    writeln!(out, "num_qubits: {}", summary.num_qubits).map_err(|e| e.to_string())?;
    writeln!(out, "num_measurements: {}", summary.num_measurements).map_err(|e| e.to_string())?;
    writeln!(out, "num_detectors: {}", summary.num_detectors).map_err(|e| e.to_string())?;
    writeln!(out, "num_observables: {}", summary.num_observables).map_err(|e| e.to_string())?;
    writeln!(out, "num_ticks: {}", summary.num_ticks).map_err(|e| e.to_string())?;
    writeln!(out, "num_sweep_bits: {}", summary.num_sweep_bits).map_err(|e| e.to_string())?;
    Ok(())
}

fn write_detection_outputs(
    detections: &BitTable,
    observable_flips: &BitTable,
    fmt: OutputFormat,
    append_observables: bool,
    out: &mut dyn Write,
    obs_out: Option<(&mut dyn Write, OutputFormat)>,
) -> Result<(), String> {
    match fmt {
        OutputFormat::Dets => {
            write_shots_dets(detections, observable_flips, out)
                .map_err(|e| format!("write error: {e}"))?;
        }
        _ => {
            if append_observables {
                let merged = merge_detections_observables(detections, observable_flips);
                write_format(fmt, &merged, out)?;
            } else {
                write_format(fmt, detections, out)?;
            }
        }
    }
    if let Some((obs_writer, obs_fmt)) = obs_out {
        write_format(obs_fmt, observable_flips, obs_writer)?;
    }
    Ok(())
}

pub fn run_gen(
    code: &str,
    task: &str,
    distance: usize,
    rounds: usize,
    noise: f64,
    out: &mut dyn Write,
) -> Result<(), String> {
    let circuit_text = generate_common_circuit_text(code, task, distance, rounds, noise)?;
    out.write_all(circuit_text.as_bytes())
        .map_err(|e| format!("write error: {e}"))
}

pub(crate) fn generate_common_circuit_text(
    code: &str,
    task: &str,
    distance: usize,
    rounds: usize,
    noise: f64,
) -> Result<String, String> {
    let instrs = match (code, task) {
        ("repetition_code", "memory") => {
            crate::codegen::repetition_code_memory(distance, rounds, noise)
        }
        ("surface_code", "rotated_memory_x") => {
            crate::codegen::surface_code::rotated_memory_x(distance, rounds, noise)
        }
        ("surface_code", "rotated_memory_z") => {
            crate::codegen::surface_code::rotated_memory_z(distance, rounds, noise)
        }
        ("surface_code", "unrotated_memory_x") => {
            crate::codegen::surface_code::unrotated_memory_x(distance, rounds, noise)
        }
        ("surface_code", "unrotated_memory_z") => {
            crate::codegen::surface_code::unrotated_memory_z(distance, rounds, noise)
        }
        ("color_code", "memory_xyz") => {
            crate::codegen::color_code::memory_xyz(distance, rounds, noise)
        }
        _ => return Err(format!("unknown code/task: {code}/{task}")),
    };
    Ok(crate::ir::circuit_to_string(&instrs))
}

pub fn run_sample(
    circuit_text: &str,
    shots: usize,
    out_format: &str,
    seed: Option<u64>,
    skip_reference_sample: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    let fmt = OutputFormat::from_str(out_format)?;
    let instrs = parse_lines(circuit_text)?;
    let mut rng = make_rng(seed);
    let options = SampleOptions {
        reference_sample_mode: if skip_reference_sample {
            crate::data_path::ReferenceSampleMode::AssumeAllZero
        } else {
            crate::data_path::ReferenceSampleMode::SimulateNoiseless
        },
        ..SampleOptions::default()
    };
    let result = sample_batch_with_options(&instrs, shots, &mut rng, options)?;
    match fmt {
        OutputFormat::Dets => {
            Err("dets format not applicable to sample command; use detect".to_string())
        }
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
    write_detection_outputs(
        &result.detections,
        &result.observable_flips,
        fmt,
        append_observables,
        out,
        None,
    )
}

pub fn run_detect_with_obs(
    circuit_text: &str,
    shots: usize,
    out_format: &str,
    seed: Option<u64>,
    append_observables: bool,
    out: &mut dyn Write,
    obs_out: &mut dyn Write,
    obs_out_format: &str,
) -> Result<(), String> {
    let fmt = OutputFormat::from_str(out_format)?;
    let obs_fmt = OutputFormat::from_str(obs_out_format)?;
    let instrs = parse_lines(circuit_text)?;
    let mut rng = make_rng(seed);
    let result = sample_batch(&instrs, shots, &mut rng)?;
    write_detection_outputs(
        &result.detections,
        &result.observable_flips,
        fmt,
        append_observables,
        out,
        Some((obs_out, obs_fmt)),
    )
}

pub fn run_analyze_errors(circuit_text: &str, out: &mut dyn Write) -> Result<(), String> {
    run_analyze_errors_with_flags(circuit_text, false, false, false, out)
}

pub fn run_analyze_errors_with_options(
    circuit_text: &str,
    approximate_disjoint_errors: bool,
    allow_gauge_detectors: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    run_analyze_errors_with_flags(
        circuit_text,
        approximate_disjoint_errors,
        allow_gauge_detectors,
        false,
        out,
    )
}

pub fn run_analyze_errors_with_flags(
    circuit_text: &str,
    approximate_disjoint_errors: bool,
    allow_gauge_detectors: bool,
    decompose_errors: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    let instrs = parse_lines(circuit_text)?;
    let options = crate::error_analyzer::AnalyzeOptions {
        backend: crate::error_analyzer::AnalyzeBackend::Auto,
        approximate_disjoint_errors,
        allow_gauge_detectors,
    };
    let dem = if decompose_errors {
        ErrorAnalyzer::circuit_to_dem_with_options_decomposed(&instrs, options)?
    } else {
        ErrorAnalyzer::circuit_to_dem_with_options(&instrs, options)?
    };
    let dem_str = dem.to_string();
    out.write_all(dem_str.as_bytes())
        .map_err(|e| format!("write error: {e}"))
}

fn run_export_json(
    text: &str,
    format: JsonOutputFormat,
    highlight_dem_error: Option<usize>,
    sample_shot: bool,
    seed: Option<u64>,
    w: &mut dyn Write,
) -> Result<(), String> {
    let instrs = parse_lines(text)?;
    if seed.is_some() && !sample_shot {
        return Err("--seed is only supported with --sample_shot".to_string());
    }
    if sample_shot && highlight_dem_error.is_some() {
        return Err("--sample_shot cannot be combined with --highlight_dem_error".to_string());
    }
    let doc = match highlight_dem_error {
        Some(index) => {
            let tracked = ErrorAnalyzer::circuit_to_tracked_dem(&instrs).map_err(|err| {
                if err.starts_with("tracked DEM does not yet support instruction ") {
                    format!(
                        "--highlight_dem_error currently supports a subset of noise instructions: {err}"
                    )
                } else {
                    err
                }
            })?;
            crate::qp101::export_qp101_with_highlighted_dem_error(&instrs, &tracked, index)
                .map_err(|err| {
                    if err.starts_with("DEM error index ") && err.contains(" out of range ") {
                        format!("DEM error index out of range: {err}")
                    } else {
                        err
                    }
                })?
        }
        None if sample_shot => {
            let mut ex = Executor::from_instrs(instrs.clone())?;
            let mut rng = make_rng(seed);
            let (_out, trace) = ex.run_with_trace(&mut rng)?;
            crate::qp101::export_qp101_with_sample_trace(&instrs, &trace).map_err(|err| {
                if err.starts_with("sample trace visualization does not yet support instruction ")
                {
                    format!(
                        "--sample_shot currently supports a subset of sample visualization instructions: {err}"
                    )
                } else {
                    err
                }
            })?
        }
        None => crate::qp101::export_qp101(&instrs)?,
    };
    match format {
        JsonOutputFormat::Pretty => {
            serde_json::to_writer_pretty(&mut *w, &doc).map_err(|e| format!("write error: {e}"))?
        }
        JsonOutputFormat::Compact => {
            serde_json::to_writer(&mut *w, &doc).map_err(|e| format!("write error: {e}"))?
        }
    }
    w.write_all(b"\n")
        .map_err(|e| format!("write error: {e}"))?;
    Ok(())
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
    write_detection_outputs(
        &result.detections,
        &result.observable_flips,
        fmt,
        false,
        out,
        None,
    )
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
    write_detection_outputs(
        &result.detections,
        &result.observable_flips,
        fmt,
        false,
        out,
        Some((obs_out, obs_fmt)),
    )
}

pub fn read_input_bytes(path: Option<&str>) -> Result<Vec<u8>, String> {
    use std::io::Read;
    match path {
        Some(p) => std::fs::read(p).map_err(|e| e.to_string()),
        None => {
            let mut buf = Vec::new();
            std::io::stdin()
                .read_to_end(&mut buf)
                .map_err(|e| e.to_string())?;
            Ok(buf)
        }
    }
}

fn read_table_from_format(
    data: &[u8],
    format: &str,
    bits: usize,
    shots: Option<usize>,
) -> Result<BitTable, String> {
    use crate::output::*;

    match format {
        "01" => read_shots_01(data, bits),
        "b8" => read_shots_b8(data, bits),
        "r8" => read_shots_r8(data, bits),
        "hits" => read_shots_hits(data, bits),
        "ptb64" => {
            let n = shots.ok_or("--shots required for ptb64 input")?;
            read_shots_ptb64(data, bits, n)
        }
        _ => Err(format!("unknown in_format: {format}")),
    }
}

pub fn run_convert(
    data: &[u8],
    in_format: &str,
    out_format: &str,
    bits: Option<usize>,
    circuit: Option<&str>,
    shots: Option<usize>,
    out: &mut dyn Write,
) -> Result<(), String> {
    use crate::output::*;
    let n_bits = if let Some(b) = bits {
        b
    } else if let Some(circ_text) = circuit {
        let instrs = crate::parser::parse_lines(circ_text)?;
        crate::stats::num_measurements(&instrs)
    } else {
        return Err("--bits or --circuit required for convert".to_string());
    };

    let table = read_table_from_format(data, in_format, n_bits, shots)?;

    match out_format {
        "01" => write_shots_01(&table, out),
        "b8" => write_shots_b8(&table, out),
        "r8" => write_shots_r8(&table, out),
        "hits" => write_shots_hits(&table, out),
        "ptb64" => write_shots_ptb64(&table, out),
        _ => return Err(format!("unknown out_format: {out_format}")),
    }
    .map_err(|e| e.to_string())
}

pub fn run_m2d(
    circuit_text: &str,
    data: &[u8],
    in_format: &str,
    out_format: &str,
    shots: Option<usize>,
    append_observables: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    run_m2d_impl(
        circuit_text,
        data,
        in_format,
        out_format,
        shots,
        None,
        M2dOptions::default(),
        append_observables,
        out,
    )
}

pub fn run_m2d_with_options(
    circuit_text: &str,
    data: &[u8],
    in_format: &str,
    out_format: &str,
    shots: Option<usize>,
    options: M2dOptions,
    append_observables: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    run_m2d_impl(
        circuit_text,
        data,
        in_format,
        out_format,
        shots,
        None,
        options,
        append_observables,
        out,
    )
}

fn run_m2d_impl(
    circuit_text: &str,
    data: &[u8],
    in_format: &str,
    out_format: &str,
    shots: Option<usize>,
    sweep_data: Option<(&[u8], &str)>,
    options: M2dOptions,
    append_observables: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    let instrs = parse_lines(circuit_text)?;
    let n_meas = crate::stats::num_measurements(&instrs);
    let meas_table = read_table_from_format(data, in_format, n_meas, shots)?;
    let sweep_table = if let Some((sweep_bytes, sweep_format)) = sweep_data {
        let n_sweep_bits = crate::stats::num_sweep_bits(&instrs);
        Some(read_table_from_format(
            sweep_bytes,
            sweep_format,
            n_sweep_bits,
            shots,
        )?)
    } else {
        None
    };
    let result = measurements_to_detections_with_options(
        &instrs,
        &meas_table,
        sweep_table.as_ref(),
        options,
    )?;
    let fmt = OutputFormat::from_str(out_format)?;

    match fmt {
        OutputFormat::Dets => write_shots_dets(&result.detections, &result.observable_flips, out)
            .map_err(|e| format!("write error: {e}")),
        _ => {
            if append_observables {
                let merged =
                    merge_detections_observables(&result.detections, &result.observable_flips);
                write_format(fmt, &merged, out)
            } else {
                write_format(fmt, &result.detections, out)
            }
        }
    }
}

pub fn run_explain_errors(
    circuit_text: &str,
    dem_text: Option<&str>,
    det_data: &[u8],
    in_format: &str,
    out: &mut dyn Write,
) -> Result<(), String> {
    let dem = if let Some(dt) = dem_text {
        crate::dem::DetectorErrorModel::parse(dt)?
    } else {
        let instrs = crate::parser::parse_lines(circuit_text)?;
        crate::error_analyzer::ErrorAnalyzer::circuit_to_dem(&instrs)?
    };

    let fired_per_shot = parse_fired_detectors(det_data, in_format, dem.num_detectors())?;

    for (shot_idx, fired) in fired_per_shot.iter().enumerate() {
        let explanations = crate::explain_errors::explain(&dem, fired);
        if explanations.is_empty() {
            writeln!(out, "shot {shot_idx}: no errors needed").map_err(|e| e.to_string())?;
        } else {
            writeln!(out, "shot {shot_idx}:").map_err(|e| e.to_string())?;
            for e in &explanations {
                let det_str: Vec<String> = e.detectors.iter().map(|d| format!("D{d}")).collect();
                let obs_str: Vec<String> = e.observables.iter().map(|o| format!("L{o}")).collect();
                let targets: Vec<String> = det_str.into_iter().chain(obs_str).collect();
                writeln!(out, "  error({:.4}) {}", e.probability, targets.join(" "))
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

fn parse_fired_detectors(
    data: &[u8],
    format: &str,
    n_dets: usize,
) -> Result<Vec<Vec<usize>>, String> {
    match format {
        "dets" => {
            let text = std::str::from_utf8(data).map_err(|e| e.to_string())?;
            let mut shots = Vec::new();
            for line in text.lines() {
                let line = line.trim();
                if !line.starts_with("shot") {
                    continue;
                }
                let mut fired = Vec::new();
                for token in line.split_whitespace().skip(1) {
                    if let Some(rest) = token.strip_prefix('D') {
                        let d: usize = rest.parse().map_err(|_| format!("bad detector {token}"))?;
                        fired.push(d);
                    }
                }
                shots.push(fired);
            }
            Ok(shots)
        }
        "01" => {
            use crate::output::read_shots_01;
            let table = read_shots_01(data, n_dets)?;
            let n_shots = table.num_minor();
            let mut shots = Vec::new();
            for shot in 0..n_shots {
                let fired: Vec<usize> = (0..n_dets).filter(|&d| table.get(d, shot)).collect();
                shots.push(fired);
            }
            Ok(shots)
        }
        _ => Err(format!(
            "unsupported in_format for explain_errors: {format}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_export_json_sample_shot_without_seed_exports_annotations_in_process() {
        let mut out = Vec::new();
        run_export_json(
            "LOSS(1) 0\nM 0\nDETECTOR rec[-1]\n",
            JsonOutputFormat::Pretty,
            None,
            true,
            None,
            &mut out,
        )
        .unwrap();

        let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(value["standard"], "QP101-ZY");
        assert_eq!(value["operations"][0]["annotations"][0]["label"], "L");
        assert_eq!(value["operations"][1]["annotations"][0]["label"], "1[L]");
        assert_eq!(value["operations"][2]["annotations"][0]["label"], "D0");
    }

    #[test]
    fn run_export_json_sample_shot_preserves_non_support_export_errors_in_process() {
        let err = run_export_json(
            "SHIFT_COORDS(1) 0\nLOSS(1) 0\nM 0\n",
            JsonOutputFormat::Pretty,
            None,
            true,
            Some(7),
            &mut Vec::new(),
        )
        .unwrap_err();

        assert!(err.contains("SHIFT_COORDS"));
        assert!(!err.contains("subset of sample visualization instructions"));
    }

    #[test]
    fn run_dispatches_export_json_sample_shot_command_in_process() {
        let input = tempfile::NamedTempFile::new().unwrap();
        let output = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(input.path(), "LOSS(1) 0\nM 0\nDETECTOR rec[-1]\n").unwrap();

        run(Cli {
            command: Some(Commands::ExportJson {
                r#in: Some(input.path().display().to_string()),
                out: Some(output.path().display().to_string()),
                format: "pretty".to_string(),
                highlight_dem_error: None,
                sample_shot: true,
                seed: Some(7),
            }),
        })
        .unwrap();

        let text = std::fs::read_to_string(output.path()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["operations"][0]["annotations"][0]["label"], "L");
        assert_eq!(value["operations"][1]["annotations"][0]["label"], "1[L]");
    }

    #[test]
    fn perf_ci_preserves_gate_failures_without_infrastructure_prefix() {
        let verdict = crate::perf::PerfGateVerdict {
            status: crate::perf::PerfGateStatus::RegressionFailure,
            messages: vec!["regression".to_string()],
        };
        let err = Err::<(), PerfCiError>(PerfCiError::Gate(verdict.summary_markdown())).map_err(
            |e| match e {
                PerfCiError::Infrastructure(message) => {
                    format!("InfrastructureFailure\n- {message}")
                }
                PerfCiError::Gate(message) => message,
            },
        );

        let err = err.unwrap_err();
        assert!(!err.starts_with("InfrastructureFailure"));
    }

    #[test]
    fn finalize_perf_ci_artifacts_writes_outputs_before_returning_gate_failure() {
        let dir = tempfile::tempdir().unwrap();
        let summary_path = dir.path().join("summary.json");
        let report_path = dir.path().join("report.md");
        let raw = concat!(
            "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":130,\"peak_memory_bytes\":1024}\n",
            "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":100,\"peak_memory_bytes\":4096}\n",
            "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-compiled\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":111,\"peak_memory_bytes\":2048}\n",
            "{\"case_label\":\"surface-detect-d13-r13\",\"tool_variant\":\"stim-cli\",\"workload\":\"detect\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":169,\"measurements\":312,\"detectors\":144,\"observables\":1,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":10000,\"wall_time_ns\":240,\"peak_memory_bytes\":4096}\n",
            "{\"case_label\":\"surface-detect-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"detect\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":169,\"measurements\":312,\"detectors\":144,\"observables\":1,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":10000,\"wall_time_ns\":210,\"peak_memory_bytes\":8192}\n",
            "{\"case_label\":\"surface-detect-d13-r13\",\"tool_variant\":\"rstim-compiled\",\"workload\":\"detect\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":169,\"measurements\":312,\"detectors\":144,\"observables\":1,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":10000,\"wall_time_ns\":170,\"peak_memory_bytes\":6144}\n",
            "{\"case_label\":\"repeat-analyze-large\",\"tool_variant\":\"stim-cli\",\"workload\":\"analyze_errors\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":4096,\"shots\":null,\"wall_time_ns\":700,\"peak_memory_bytes\":512}\n",
            "{\"case_label\":\"repeat-analyze-large\",\"tool_variant\":\"rstim-analyzer-flattened\",\"workload\":\"analyze_errors\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":4096,\"shots\":null,\"wall_time_ns\":600,\"peak_memory_bytes\":1024}\n",
            "{\"case_label\":\"repeat-analyze-large\",\"tool_variant\":\"rstim-analyzer-compiled\",\"workload\":\"analyze_errors\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":4096,\"shots\":null,\"wall_time_ns\":500,\"peak_memory_bytes\":768}\n",
            "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":80,\"peak_memory_bytes\":128}\n",
            "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":70,\"peak_memory_bytes\":256}\n"
        );

        let err = finalize_perf_ci_artifacts(
            raw,
            &summary_path,
            &report_path,
            crate::perf::PerfGateConfig {
                sampler_ratio_threshold: 1.10,
                analyzer_ratio_threshold: 1.10,
            },
        )
        .unwrap_err();

        match err {
            PerfCiError::Gate(message) => {
                assert!(message.contains("RegressionFailure") || message.contains("exceeds threshold"));
            }
            PerfCiError::Infrastructure(message) => {
                panic!("unexpected infrastructure failure: {message}");
            }
        }

        let summary_text = std::fs::read_to_string(&summary_path).unwrap();
        let report_text = std::fs::read_to_string(&report_path).unwrap();
        assert!(summary_text.contains("\"cases\""));
        assert!(report_text.contains("## Gate Verdict"));
        assert!(report_text.contains("RegressionFailure") || report_text.contains("exceeds threshold"));
    }
}
