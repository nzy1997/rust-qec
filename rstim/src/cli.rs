use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};

use clap::{Parser, Subcommand};
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::codegen::css::{
    css_memory, parse_css_matrix_json, parse_css_observable_json, CssCheckMatrices,
    CssMemoryConfig, CssObservableSource, CssSchedule, MemoryBasis,
};
use crate::codegen::NoiseParams;
use crate::dem::DetectorErrorModel;
use crate::error_analyzer::ErrorAnalyzer;
use crate::executor::Executor;
use crate::m2d::{measurements_to_detections_with_options, M2dOptions};
use crate::measurement_transform::{
    DecodedSampleBlock, MeasurementTransform, MeasurementTransformError,
};
use crate::output::{
    write_shots_01, write_shots_b8, write_shots_dets, write_shots_hits, write_shots_ptb64,
    write_shots_r8, OutputFormat,
};
use crate::parser::parse_lines;
use crate::sample_archive::{
    format::SampleArchiveErrorCode, ArchiveLimits, SampleArchiveOptions, SampleArchiveReader,
    SampleArchiveWriter,
};
use crate::sampler::{sample_batch, sample_batch_with_options, SampleOptions, SampleOutputMode};
use crate::sim::bit_table::BitTable;

#[derive(Parser)]
#[command(name = "rstim", version, about = "Rust stabilizer circuit simulator")]
pub struct Cli {
    #[arg(long = "benchmark-telemetry-json", global = true)]
    pub benchmark_telemetry_json: Option<String>,
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
        distance: Option<usize>,
        #[arg(long)]
        rounds: usize,
        #[arg(long = "after_clifford_depolarization", default_value = "0")]
        noise: f64,
        #[arg(long = "after_clifford_loss_probability", default_value = "0")]
        after_clifford_loss_probability: f64,
        #[arg(long)]
        hx: Option<String>,
        #[arg(long)]
        hz: Option<String>,
        #[arg(long)]
        basis: Option<String>,
        #[arg(long, default_value = "greedy")]
        schedule: String,
        #[arg(long)]
        observables: Option<String>,
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
    /// Render a circuit as SVG through QP101
    #[command(name = "render_svg")]
    RenderSvg {
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
        #[arg(long = "highlight_dem_error")]
        highlight_dem_error: Option<usize>,
        #[arg(long = "sample_shot")]
        sample_shot: bool,
        #[arg(long)]
        seed: Option<u64>,
    },
    /// Pack b8 measurement samples into an RSMP archive
    #[command(name = "pack_samples")]
    PackSamples {
        #[arg(long = "circuit")]
        circuit: String,
        #[arg(long = "shots")]
        shots: u64,
        #[arg(long = "in")]
        r#in: String,
        #[arg(long = "in_format", default_value = "b8")]
        in_format: String,
        #[arg(long = "out")]
        out: String,
    },
    /// Unpack an RSMP archive into b8 sample streams
    #[command(name = "unpack_samples")]
    UnpackSamples {
        #[arg(long = "circuit")]
        circuit: String,
        #[arg(long = "in")]
        r#in: String,
        #[arg(long = "measurements_out")]
        measurements_out: Option<String>,
        #[arg(long = "measurements_out_format", default_value = "b8")]
        measurements_out_format: String,
        #[arg(long = "detectors_out")]
        detectors_out: Option<String>,
        #[arg(long = "detectors_out_format", default_value = "b8")]
        detectors_out_format: String,
        #[arg(long = "obs_out")]
        obs_out: Option<String>,
        #[arg(long = "obs_out_format", default_value = "b8")]
        obs_out_format: String,
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
        #[arg(long = "case")]
        case: Option<String>,
        #[arg(long, default_value_t = 1)]
        warmup_rounds: usize,
        #[arg(long, default_value_t = 5)]
        measure_rounds: usize,
    },
    /// Aggregate raw JSONL into summary JSON
    Summarize {
        #[arg(long = "case")]
        case: Option<String>,
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
        #[arg(long = "case")]
        case: Option<String>,
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

#[derive(Clone, Copy)]
struct Qp101BuildOptions {
    highlight_dem_error: Option<usize>,
    sample_shot: bool,
    seed: Option<u64>,
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
    let Cli {
        benchmark_telemetry_json,
        command,
    } = cli;
    if benchmark_telemetry_json.is_some() && !cfg!(feature = "benchmark-telemetry") {
        return Err(
            "--benchmark-telemetry-json requires building rstim with --features benchmark-telemetry"
                .to_string(),
        );
    }
    #[cfg(feature = "benchmark-telemetry")]
    if benchmark_telemetry_json.is_some() {
        crate::sim::frame::reset_frame_noise_telemetry();
    }

    let result = run_command(command);
    finish_benchmark_telemetry(benchmark_telemetry_json.as_deref(), result)
}

fn run_command(command: Option<Commands>) -> Result<(), String> {
    match command {
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
            after_clifford_loss_probability,
            hx,
            hz,
            basis,
            schedule,
            observables,
            out,
        }) => {
            if code == "css" {
                let mut buffer = Vec::new();
                run_css_gen(
                    &task,
                    hx.as_deref(),
                    hz.as_deref(),
                    basis.as_deref(),
                    rounds,
                    noise,
                    &schedule,
                    observables.as_deref(),
                    &mut buffer,
                )?;
                let mut w = open_output(out.as_deref())?;
                w.write_all(&buffer)
                    .map_err(|error| format!("write error: {error}"))
            } else {
                let distance = distance
                    .ok_or_else(|| "distance is required for common generators".to_string())?;
                let mut w = open_output(out.as_deref())?;
                let mut params = NoiseParams::uniform(noise);
                params.after_clifford_loss_probability = after_clifford_loss_probability;
                run_gen_with_params(&code, &task, distance, rounds, params, &mut w)
            }
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
            run_export_json(
                &text,
                format,
                highlight_dem_error,
                sample_shot,
                seed,
                &mut w,
            )
        }
        Some(Commands::RenderSvg {
            r#in,
            out,
            highlight_dem_error,
            sample_shot,
            seed,
        }) => {
            let text = read_input(r#in.as_deref())?;
            let svg = run_render_svg_to_string(
                &text,
                Qp101BuildOptions {
                    highlight_dem_error,
                    sample_shot,
                    seed,
                },
            )?;
            let mut w = open_output(out.as_deref())?;
            w.write_all(svg.as_bytes())
                .map_err(|e| format!("write error: {e}"))
        }
        Some(Commands::PackSamples {
            circuit,
            shots,
            r#in,
            in_format,
            out,
        }) => run_pack_samples(&circuit, shots, &r#in, &in_format, &out),
        Some(Commands::UnpackSamples {
            circuit,
            r#in,
            measurements_out,
            measurements_out_format,
            detectors_out,
            detectors_out_format,
            obs_out,
            obs_out_format,
        }) => run_unpack_samples(
            &circuit,
            &r#in,
            measurements_out.as_deref(),
            &measurements_out_format,
            detectors_out.as_deref(),
            &detectors_out_format,
            obs_out.as_deref(),
            &obs_out_format,
        ),
        Some(Commands::Perf { command }) => {
            match command {
                PerfCommands::Run {
                    out,
                    case,
                    warmup_rounds,
                    measure_rounds,
                } => {
                    if let Some(case_label) = case.as_deref() {
                        crate::perf::benchmark_case_by_label(case_label)?;
                    }
                    let mut w = open_output(out.as_deref())?;
                    let options = crate::perf::PerfRunOptions {
                        warmup_rounds,
                        measured_rounds: measure_rounds,
                    };
                    if let Some(case_label) = case.as_deref() {
                        crate::perf::run_benchmark_case_to_writer(&mut w, case_label, options)
                    } else {
                        crate::perf::run_benchmark_suite_to_writer(&mut w, options)
                    }
                }
                PerfCommands::Summarize { case, r#in, out } => {
                    let raw = read_input(r#in.as_deref())?;
                    let summary = if let Some(label) = case.as_deref() {
                        let options = crate::perf::PerfSummaryOptions {
                            case_label: Some(label.to_string()),
                        };
                        crate::perf::summarize_jsonl_str_with_options(&raw, options)?
                    } else {
                        crate::perf::summarize_jsonl_str(&raw)?
                    };
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
                    case,
                    warmup_rounds,
                    measure_rounds,
                } => run_perf_ci(&out_dir, warmup_rounds, measure_rounds, case.as_deref()).map_err(
                    |e| match e {
                        PerfCiError::Infrastructure(message) => {
                            format!("InfrastructureFailure\n- {message}")
                        }
                        PerfCiError::Gate(message) => message,
                    },
                ),
            }
        }
        None => {
            println!("rstim {}", crate::version());
            Ok(())
        }
    }
}

fn finish_benchmark_telemetry(
    path: Option<&str>,
    result: Result<(), String>,
) -> Result<(), String> {
    if result.is_err() || path.is_none() {
        return result;
    }

    #[cfg(feature = "benchmark-telemetry")]
    {
        write_benchmark_telemetry_json(path.unwrap())?;
    }
    #[cfg(not(feature = "benchmark-telemetry"))]
    {
        let _ = path;
    }
    result
}

#[cfg(feature = "benchmark-telemetry")]
#[derive(serde::Serialize)]
struct BenchmarkTelemetryJson {
    operations: Vec<crate::sim::frame::FrameNoiseTelemetryRecord>,
}

#[cfg(feature = "benchmark-telemetry")]
fn write_benchmark_telemetry_json(path: &str) -> Result<(), String> {
    let file =
        std::fs::File::create(path).map_err(|error| format!("failed to create {path}: {error}"))?;
    let mut writer = io::BufWriter::new(file);
    let telemetry = BenchmarkTelemetryJson {
        operations: crate::sim::frame::take_frame_noise_telemetry(),
    };
    serde_json::to_writer_pretty(&mut writer, &telemetry)
        .map_err(|error| format!("failed to write benchmark telemetry JSON: {error}"))?;
    writer
        .write_all(b"\n")
        .map_err(|error| format!("failed to write benchmark telemetry JSON newline: {error}"))
}

fn run_perf_ci(
    out_dir: &str,
    warmup_rounds: usize,
    measure_rounds: usize,
    case_label: Option<&str>,
) -> Result<(), PerfCiError> {
    if let Some(label) = case_label {
        crate::perf::benchmark_case_by_label(label).map_err(PerfCiError::Infrastructure)?;
    }
    std::fs::create_dir_all(out_dir).map_err(|e| {
        PerfCiError::Infrastructure(format!("failed to create perf out dir {out_dir}: {e}"))
    })?;

    let raw_path = std::path::Path::new(out_dir).join("raw.jsonl");
    let summary_path = std::path::Path::new(out_dir).join("summary.json");
    let report_path = std::path::Path::new(out_dir).join("report.md");

    write_perf_ci_raw_artifact(&raw_path, warmup_rounds, measure_rounds, case_label)?;

    let raw_text = std::fs::read_to_string(&raw_path).map_err(|e| {
        PerfCiError::Infrastructure(format!(
            "failed to read raw perf artifact {}: {e}",
            raw_path.display()
        ))
    })?;
    finalize_perf_ci_artifacts(
        &raw_text,
        &summary_path,
        &report_path,
        crate::perf::PerfGateConfig::default(),
        case_label,
    )
}

fn write_perf_ci_raw_artifact(
    raw_path: &std::path::Path,
    warmup_rounds: usize,
    measure_rounds: usize,
    case_label: Option<&str>,
) -> Result<(), PerfCiError> {
    if let Ok(source_path) = std::env::var("RSTIM_TEST_PERF_CI_RAW") {
        write_test_perf_ci_raw_override(&source_path, raw_path, case_label)?;
        return Ok(());
    }

    let mut raw = open_output(raw_path.to_str()).map_err(PerfCiError::Infrastructure)?;
    let options = crate::perf::PerfRunOptions {
        warmup_rounds,
        measured_rounds: measure_rounds,
    };
    if let Some(label) = case_label {
        crate::perf::run_benchmark_case_to_writer(&mut raw, label, options)
    } else {
        crate::perf::run_benchmark_suite_to_writer(&mut raw, options)
    }
    .map_err(PerfCiError::Infrastructure)
}

fn write_test_perf_ci_raw_override(
    source_path: &str,
    raw_path: &std::path::Path,
    case_label: Option<&str>,
) -> Result<(), PerfCiError> {
    let Some(label) = case_label else {
        std::fs::copy(source_path, raw_path).map_err(|e| {
            PerfCiError::Infrastructure(format!(
                "failed to copy test perf raw artifact from {source_path} to {}: {e}",
                raw_path.display()
            ))
        })?;
        return Ok(());
    };

    let raw_text = std::fs::read_to_string(source_path).map_err(|e| {
        PerfCiError::Infrastructure(format!(
            "failed to read test perf raw artifact from {source_path}: {e}"
        ))
    })?;
    let mut filtered = String::new();
    for line in raw_text.lines().filter(|line| !line.trim().is_empty()) {
        let value = serde_json::from_str::<serde_json::Value>(line).map_err(|e| {
            PerfCiError::Infrastructure(format!(
                "failed to parse test perf raw artifact from {source_path}: {e}"
            ))
        })?;
        if value.get("case_label").and_then(serde_json::Value::as_str) == Some(label) {
            filtered.push_str(line);
            filtered.push('\n');
        }
    }

    std::fs::write(raw_path, filtered).map_err(|e| {
        PerfCiError::Infrastructure(format!(
            "failed to write filtered test perf raw artifact {}: {e}",
            raw_path.display()
        ))
    })
}

fn finalize_perf_ci_artifacts(
    raw_text: &str,
    summary_path: &std::path::Path,
    report_path: &std::path::Path,
    config: crate::perf::PerfGateConfig,
    case_label: Option<&str>,
) -> Result<(), PerfCiError> {
    let summary = if let Some(label) = case_label {
        crate::perf::summarize_jsonl_str_with_options(
            raw_text,
            crate::perf::PerfSummaryOptions {
                case_label: Some(label.to_string()),
            },
        )
        .map_err(PerfCiError::Infrastructure)?
    } else {
        crate::perf::summarize_jsonl_str(raw_text).map_err(PerfCiError::Infrastructure)?
    };
    let verdict = crate::perf::evaluate_summary(&summary, config);

    std::fs::write(
        summary_path,
        serde_json::to_string_pretty(&summary).map_err(|e| {
            PerfCiError::Infrastructure(format!("failed to serialize perf summary: {e}"))
        })?,
    )
    .map_err(|e| {
        PerfCiError::Infrastructure(format!(
            "failed to write perf summary {}: {e}",
            summary_path.display()
        ))
    })?;

    let report = if case_label.is_some() {
        crate::perf::render_markdown_report(&summary, None)
    } else {
        crate::perf::render_markdown_report(&summary, Some(&verdict.summary_markdown()))
    };
    std::fs::write(report_path, report).map_err(|e| {
        PerfCiError::Infrastructure(format!(
            "failed to write perf report {}: {e}",
            report_path.display()
        ))
    })?;

    if case_label.is_some() || verdict.status == crate::perf::PerfGateStatus::Pass {
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

struct PendingOutput {
    final_path: PathBuf,
    temp_path: PathBuf,
    file: BufWriter<File>,
    published: bool,
}

impl PendingOutput {
    fn create(final_path: &str, reserved_final_paths: &BTreeSet<PathBuf>) -> Result<Self, String> {
        let final_path = PathBuf::from(final_path);
        let parent = final_path.parent().unwrap_or_else(|| Path::new("."));
        let name = final_path
            .file_name()
            .ok_or_else(|| "output path must name a file".to_string())?
            .to_string_lossy();
        let process_id = std::process::id();

        for retry in 0..1024 {
            let temp_path = parent.join(format!(".{name}.rstim-{process_id}-{retry}.tmp"));
            if reserved_final_paths.contains(&lexical_absolute_path_from_path(&temp_path)?) {
                continue;
            }
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)
            {
                Ok(file) => {
                    return Ok(Self {
                        final_path,
                        temp_path,
                        file: BufWriter::new(file),
                        published: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!(
                        "failed to create staged output {}: {error}",
                        temp_path.display()
                    ));
                }
            }
        }

        Err(format!(
            "failed to allocate a staged output beside {}",
            final_path.display()
        ))
    }

    fn publish(&mut self) -> Result<(), String> {
        self.file
            .flush()
            .map_err(|error| format!("failed to flush {}: {error}", self.temp_path.display()))?;
        std::fs::rename(&self.temp_path, &self.final_path).map_err(|error| {
            format!(
                "failed to publish {} to {}: {error}",
                self.temp_path.display(),
                self.final_path.display()
            )
        })?;
        self.published = true;
        Ok(())
    }
}

impl Drop for PendingOutput {
    fn drop(&mut self) {
        if !self.published {
            let _ = std::fs::remove_file(&self.temp_path);
        }
    }
}

fn run_pack_samples(
    circuit_path: &str,
    shots: u64,
    input_path: &str,
    input_format: &str,
    output_path: &str,
) -> Result<(), String> {
    let reserved_output_paths =
        preflight_pack_samples(circuit_path, shots, input_path, input_format, output_path)?;

    let circuit_text = read_rsmp_text(circuit_path)?;
    let circuit = parse_lines(&circuit_text)?;
    let transform = MeasurementTransform::from_circuit(&circuit)
        .map_err(format_transform_error_for_rsmp_cli)?;
    let data = read_rsmp_bytes(input_path)?;
    let measurements = read_exact_b8_measurements(&data, transform.num_measurements(), shots)?;
    let limits = ArchiveLimits::default();

    if output_path == "-" {
        let stdout = io::stdout();
        let output = BufWriter::new(stdout.lock());
        let mut writer = SampleArchiveWriter::new(
            output,
            transform,
            shots,
            SampleArchiveOptions::default(),
            limits,
        )
        .map_err(|error| error.to_string())?;
        if shots > 0 {
            writer
                .write_measurements(&measurements)
                .map_err(|error| error.to_string())?;
        }
        writer.finish().map_err(|error| error.to_string())?;
    } else {
        let mut output = PendingOutput::create(output_path, &reserved_output_paths)?;
        let mut writer = SampleArchiveWriter::new(
            &mut output.file,
            transform,
            shots,
            SampleArchiveOptions::default(),
            limits,
        )
        .map_err(|error| error.to_string())?;
        if shots > 0 {
            writer
                .write_measurements(&measurements)
                .map_err(|error| error.to_string())?;
        }
        writer.finish().map_err(|error| error.to_string())?;
        output.publish()?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_unpack_samples(
    circuit_path: &str,
    input_path: &str,
    measurements_out: Option<&str>,
    measurements_out_format: &str,
    detectors_out: Option<&str>,
    detectors_out_format: &str,
    obs_out: Option<&str>,
    obs_out_format: &str,
) -> Result<(), String> {
    let reserved_output_paths = preflight_unpack_samples(
        circuit_path,
        input_path,
        measurements_out,
        measurements_out_format,
        detectors_out,
        detectors_out_format,
        obs_out,
        obs_out_format,
    )?;

    let circuit_text = read_rsmp_text(circuit_path)?;
    let circuit = parse_lines(&circuit_text)?;
    let decoded = if input_path == "-" {
        let stdin = io::stdin();
        read_sample_archive(stdin.lock(), &circuit)?
    } else {
        let input = File::open(input_path)
            .map_err(|error| format!("failed to read {input_path}: {error}"))?;
        read_sample_archive(BufReader::new(input), &circuit)?
    };

    write_unpacked_b8_outputs(
        &decoded,
        measurements_out,
        detectors_out,
        obs_out,
        &reserved_output_paths,
    )
}

fn preflight_pack_samples(
    circuit_path: &str,
    shots: u64,
    input_path: &str,
    input_format: &str,
    output_path: &str,
) -> Result<BTreeSet<PathBuf>, String> {
    if input_format != "b8" {
        return Err("pack_samples only supports --in_format b8".to_string());
    }
    if [circuit_path, input_path]
        .iter()
        .filter(|path| **path == "-")
        .count()
        > 1
    {
        return Err("pack_samples accepts at most one stdin input".to_string());
    }
    if [output_path].iter().filter(|path| **path == "-").count() > 1 {
        return Err("pack_samples accepts at most one stdout output".to_string());
    }
    if shots > ArchiveLimits::default().transform.max_shots_per_block {
        return Err("pack_samples shot count exceeds archive block limit".to_string());
    }
    if output_path.is_empty() {
        return Err("pack_samples requires --out".to_string());
    }
    let mut final_paths = BTreeSet::new();
    if output_path != "-" {
        final_paths.insert(lexical_absolute_path(output_path)?);
    }
    Ok(final_paths)
}

#[allow(clippy::too_many_arguments)]
fn preflight_unpack_samples(
    circuit_path: &str,
    input_path: &str,
    measurements_out: Option<&str>,
    measurements_out_format: &str,
    detectors_out: Option<&str>,
    detectors_out_format: &str,
    obs_out: Option<&str>,
    obs_out_format: &str,
) -> Result<BTreeSet<PathBuf>, String> {
    let outputs = [
        (
            measurements_out,
            measurements_out_format,
            "--measurements_out_format",
        ),
        (
            detectors_out,
            detectors_out_format,
            "--detectors_out_format",
        ),
        (obs_out, obs_out_format, "--obs_out_format"),
    ];
    if outputs.iter().all(|(path, _, _)| path.is_none()) {
        return Err("unpack_samples requires at least one output".to_string());
    }
    for (_, format, flag) in outputs {
        if format != "b8" {
            return Err(format!("unpack_samples only supports {flag} b8"));
        }
    }
    if [circuit_path, input_path]
        .iter()
        .filter(|path| **path == "-")
        .count()
        > 1
    {
        return Err("unpack_samples accepts at most one stdin input".to_string());
    }
    if outputs
        .iter()
        .filter(|(path, _, _)| *path == Some("-"))
        .count()
        > 1
    {
        return Err("unpack_samples accepts at most one stdout output".to_string());
    }

    let mut final_paths = BTreeSet::new();
    for (path, _, _) in outputs {
        if let Some(path) = path.filter(|path| *path != "-") {
            let normalized = lexical_absolute_path(path)?;
            if !final_paths.insert(normalized) {
                return Err("unpack_samples output paths must be distinct".to_string());
            }
        }
    }
    Ok(final_paths)
}

fn lexical_absolute_path(path: &str) -> Result<PathBuf, String> {
    lexical_absolute_path_from_path(Path::new(path))
}

fn lexical_absolute_path_from_path(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("failed to resolve output path: {error}"))?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(name) => normalized.push(name),
        }
    }
    Ok(normalized)
}

fn format_transform_error_for_rsmp_cli(error: MeasurementTransformError) -> String {
    match error {
        MeasurementTransformError::UnsupportedSweep => format!(
            "{}: {}",
            SampleArchiveErrorCode::UnsupportedSweep.as_str(),
            error
        ),
        _ => error.to_string(),
    }
}

fn read_rsmp_text(path: &str) -> Result<String, String> {
    if path == "-" {
        let mut text = String::new();
        io::stdin()
            .read_to_string(&mut text)
            .map_err(|error| format!("failed to read stdin: {error}"))?;
        Ok(text)
    } else {
        std::fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))
    }
}

fn read_rsmp_bytes(path: &str) -> Result<Vec<u8>, String> {
    if path == "-" {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|error| format!("failed to read stdin: {error}"))?;
        Ok(bytes)
    } else {
        std::fs::read(path).map_err(|error| format!("failed to read {path}: {error}"))
    }
}

fn read_exact_b8_measurements(data: &[u8], bits: usize, shots: u64) -> Result<BitTable, String> {
    let shots =
        usize::try_from(shots).map_err(|_| "pack_samples shot count exceeds usize".to_string())?;
    let bytes_per_shot = bits
        .checked_add(7)
        .ok_or_else(|| "b8 measurement width overflows".to_string())?
        / 8;
    let expected_len = bytes_per_shot
        .checked_mul(shots)
        .ok_or_else(|| "b8 input length overflows".to_string())?;
    if data.len() != expected_len {
        return Err(format!(
            "b8 input has {} bytes; expected {expected_len}",
            data.len()
        ));
    }
    let partial_bits = bits % 8;
    if partial_bits != 0 {
        let unused_mask = !((1u8 << partial_bits) - 1);
        for shot in 0..shots {
            let last_byte = data[shot * bytes_per_shot + bytes_per_shot - 1];
            if last_byte & unused_mask != 0 {
                return Err("b8 input has nonzero unused high bits".to_string());
            }
        }
    }

    let mut measurements = BitTable::try_new(bits, shots)
        .map_err(|error| format!("BitTable allocation failed: {error:?}"))?;
    for shot in 0..shots {
        for bit in 0..bits {
            let byte = data[shot * bytes_per_shot + bit / 8];
            if byte & (1 << (bit % 8)) != 0 {
                measurements.set(bit, shot, true);
            }
        }
    }
    Ok(measurements)
}

fn read_sample_archive<R: Read>(
    input: R,
    circuit: &[crate::ir::StimInstr],
) -> Result<DecodedSampleBlock, String> {
    let mut reader = SampleArchiveReader::open(input, circuit, ArchiveLimits::default())
        .map_err(|error| error.to_string())?;
    let block = reader.next_block().map_err(|error| error.to_string())?;
    reader.finish().map_err(|error| error.to_string())?;
    if let Some(block) = block {
        return Ok(block);
    }

    let transform =
        MeasurementTransform::from_circuit(circuit).map_err(|error| error.to_string())?;
    Ok(DecodedSampleBlock {
        measurements: empty_bit_table(transform.num_measurements())?,
        detections: empty_bit_table(transform.num_detectors())?,
        observable_flips: empty_bit_table(transform.num_observables())?,
    })
}

fn empty_bit_table(rows: usize) -> Result<BitTable, String> {
    BitTable::try_new(rows, 0).map_err(|error| format!("BitTable allocation failed: {error:?}"))
}

fn write_unpacked_b8_outputs(
    decoded: &DecodedSampleBlock,
    measurements_out: Option<&str>,
    detectors_out: Option<&str>,
    obs_out: Option<&str>,
    reserved_final_paths: &BTreeSet<PathBuf>,
) -> Result<(), String> {
    let outputs = [
        (measurements_out, &decoded.measurements),
        (detectors_out, &decoded.detections),
        (obs_out, &decoded.observable_flips),
    ];

    for (path, table) in outputs {
        if path == Some("-") {
            let stdout = io::stdout();
            let mut output = BufWriter::new(stdout.lock());
            write_shots_b8(table, &mut output).map_err(|error| format!("write error: {error}"))?;
            output
                .flush()
                .map_err(|error| format!("write error: {error}"))?;
        }
    }

    let mut pending_outputs = Vec::new();
    for (path, table) in outputs {
        if let Some(path) = path.filter(|path| *path != "-") {
            let mut output = PendingOutput::create(path, reserved_final_paths)?;
            write_shots_b8(table, &mut output.file)
                .map_err(|error| format!("write error: {error}"))?;
            pending_outputs.push(output);
        }
    }
    for output in &mut pending_outputs {
        output.publish()?;
    }
    Ok(())
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
    try_merge_detections_observables(dets, obs)
        .expect("trusted detector/observable dimensions allocate")
}

pub fn try_merge_detections_observables(
    dets: &BitTable,
    obs: &BitTable,
) -> Result<BitTable, String> {
    let n_dets = dets.num_major();
    let n_obs = obs.num_major();
    let n_shots = dets.num_minor();
    if obs.num_minor() != n_shots {
        return Err(format!(
            "observable shot count {} does not match detection shot count {}",
            obs.num_minor(),
            n_shots
        ));
    }
    let merged_rows = n_dets
        .checked_add(n_obs)
        .ok_or_else(|| "detector and observable row count overflows".to_string())?;
    let mut merged = BitTable::try_new(merged_rows, n_shots)
        .map_err(|err| format!("BitTable allocation failed: {err:?}"))?;
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
    Ok(merged)
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
                let merged = try_merge_detections_observables(detections, observable_flips)?;
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

pub fn run_gen_with_params(
    code: &str,
    task: &str,
    distance: usize,
    rounds: usize,
    params: NoiseParams,
    out: &mut dyn Write,
) -> Result<(), String> {
    let circuit_text =
        generate_common_circuit_text_with_params(code, task, distance, rounds, params)?;
    out.write_all(circuit_text.as_bytes())
        .map_err(|e| format!("write error: {e}"))
}

pub fn run_css_gen(
    task: &str,
    hx_path: Option<&str>,
    hz_path: Option<&str>,
    basis: Option<&str>,
    rounds: usize,
    noise: f64,
    schedule: &str,
    observables_path: Option<&str>,
    out: &mut dyn Write,
) -> Result<(), String> {
    if task != "memory" {
        return Err(format!("unknown css task: {task}"));
    }
    let hx_path =
        hx_path.ok_or_else(|| "--hx is required for css memory generation".to_string())?;
    let hz_path =
        hz_path.ok_or_else(|| "--hz is required for css memory generation".to_string())?;
    let hx_text = read_input(Some(hx_path))?;
    let hz_text = read_input(Some(hz_path))?;
    let hx = parse_css_matrix_json(&hx_text).map_err(|error| error.to_string())?;
    let hz = parse_css_matrix_json(&hz_text).map_err(|error| error.to_string())?;
    if hx.num_cols != hz.num_cols {
        return Err(format!(
            "hx and hz widths differ: {} != {}",
            hx.num_cols, hz.num_cols
        ));
    }
    let observables = if let Some(path) = observables_path {
        let text = read_input(Some(path))?;
        let parsed = parse_css_observable_json(&text).map_err(|error| error.to_string())?;
        if parsed.num_cols != hx.num_cols {
            return Err(format!(
                "observable width differs from CSS width: {} != {}",
                parsed.num_cols, hx.num_cols
            ));
        }
        CssObservableSource::Explicit(parsed.rows)
    } else {
        CssObservableSource::CanonicalFallback
    };
    let basis = parse_memory_basis(
        basis.ok_or_else(|| "--basis is required for css memory generation".to_string())?,
    )?;
    let schedule = parse_css_schedule(schedule)?;
    let circuit = css_memory(CssMemoryConfig {
        checks: CssCheckMatrices {
            hx: hx.rows,
            hz: hz.rows,
            num_data_qubits: hx.num_cols,
        },
        rounds,
        noise: NoiseParams::uniform(noise),
        basis,
        schedule,
        observables,
    })
    .map_err(|error| error.to_string())?;
    out.write_all(crate::ir::circuit_to_string(&circuit).as_bytes())
        .map_err(|error| format!("write error: {error}"))
}

fn parse_memory_basis(value: &str) -> Result<MemoryBasis, String> {
    match value {
        "x" | "X" => Ok(MemoryBasis::X),
        "z" | "Z" => Ok(MemoryBasis::Z),
        other => Err(format!("unknown CSS memory basis: {other}")),
    }
}

fn parse_css_schedule(value: &str) -> Result<CssSchedule, String> {
    match value {
        "sequential" => Ok(CssSchedule::Sequential),
        "greedy" => Ok(CssSchedule::Greedy),
        other => Err(format!("unknown CSS schedule: {other}")),
    }
}

pub(crate) fn generate_common_circuit_text(
    code: &str,
    task: &str,
    distance: usize,
    rounds: usize,
    noise: f64,
) -> Result<String, String> {
    generate_common_circuit_text_with_params(
        code,
        task,
        distance,
        rounds,
        NoiseParams::uniform(noise),
    )
}

pub(crate) fn generate_common_circuit_text_with_params(
    code: &str,
    task: &str,
    distance: usize,
    rounds: usize,
    params: NoiseParams,
) -> Result<String, String> {
    let instrs = match (code, task) {
        ("repetition_code", "memory") => {
            crate::codegen::repetition_code_memory_with_params(distance, rounds, params)
        }
        ("surface_code", "rotated_memory_x") => {
            crate::codegen::surface_code::rotated_memory_x_with_params(distance, rounds, params)
        }
        ("surface_code", "rotated_memory_z") => {
            crate::codegen::surface_code::rotated_memory_z_with_params(distance, rounds, params)
        }
        ("surface_code", "unrotated_memory_x") => {
            crate::codegen::surface_code::unrotated_memory_x_with_params(distance, rounds, params)
        }
        ("surface_code", "unrotated_memory_z") => {
            crate::codegen::surface_code::unrotated_memory_z_with_params(distance, rounds, params)
        }
        ("color_code", "memory_xyz") => {
            crate::codegen::color_code::memory_xyz_with_params(distance, rounds, params)
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
    let options = sample_cli_options(skip_reference_sample);
    let result = sample_batch_with_options(&instrs, shots, &mut rng, options)?;
    match fmt {
        OutputFormat::Dets => {
            Err("dets format not applicable to sample command; use detect".to_string())
        }
        _ => write_format(fmt, &result.measurements, out),
    }
}

pub fn sample_cli_options(skip_reference_sample: bool) -> SampleOptions {
    SampleOptions {
        reference_sample_mode: if skip_reference_sample {
            crate::data_path::ReferenceSampleMode::AssumeAllZero
        } else {
            crate::data_path::ReferenceSampleMode::SimulateNoiseless
        },
        output_mode: SampleOutputMode::MeasurementsOnly,
        ..SampleOptions::default()
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
    let doc = build_qp101_document(
        &instrs,
        Qp101BuildOptions {
            highlight_dem_error,
            sample_shot,
            seed,
        },
    )?;
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

fn build_qp101_document(
    instrs: &[crate::ir::StimInstr],
    options: Qp101BuildOptions,
) -> Result<crate::qp101::Qp101Document, String> {
    if options.seed.is_some() && !options.sample_shot {
        return Err("--seed is only supported with --sample_shot".to_string());
    }
    if options.sample_shot && options.highlight_dem_error.is_some() {
        return Err("--sample_shot cannot be combined with --highlight_dem_error".to_string());
    }

    match options.highlight_dem_error {
        Some(index) => {
            let tracked = ErrorAnalyzer::circuit_to_tracked_dem(instrs).map_err(|err| {
                if err.starts_with("tracked DEM does not yet support instruction ") {
                    format!(
                        "--highlight_dem_error currently supports a subset of noise instructions: {err}"
                    )
                } else {
                    err
                }
            })?;
            crate::qp101::export_qp101_with_highlighted_dem_error(instrs, &tracked, index).map_err(
                |err| {
                    if err.starts_with("DEM error index ") && err.contains(" out of range ") {
                        format!("DEM error index out of range: {err}")
                    } else {
                        err
                    }
                },
            )
        }
        None if options.sample_shot => {
            let mut ex = Executor::from_instrs(instrs.to_vec())?;
            let mut rng = make_rng(options.seed);
            let (_out, trace) = ex.run_with_trace(&mut rng)?;
            crate::qp101::export_qp101_with_sample_trace(instrs, &trace).map_err(|err| {
                if err.starts_with("sample trace visualization does not yet support instruction ")
                {
                    format!(
                        "--sample_shot currently supports a subset of sample visualization instructions: {err}"
                    )
                } else {
                    err
                }
            })
        }
        None => build_plain_qp101_document(instrs),
    }
}

fn build_plain_qp101_document(
    instrs: &[crate::ir::StimInstr],
) -> Result<crate::qp101::Qp101Document, String> {
    crate::qp101::export_qp101(instrs)
}

fn run_render_svg_to_string(text: &str, options: Qp101BuildOptions) -> Result<String, String> {
    let instrs = parse_lines(text)?;
    let doc = build_qp101_document(&instrs, options)?;
    crate::qp101_svg::render_svg(&doc)
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
    let result = dem.try_sample_batch(shots, &mut rng)?;
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
    let result = dem.try_sample_batch(shots, &mut rng)?;
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
                    try_merge_detections_observables(&result.detections, &result.observable_flips)?;
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
    use std::sync::{Mutex, MutexGuard, OnceLock};

    const PERF_PASS_RAW: &str = concat!(
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":130,\"peak_memory_bytes\":1024}\n",
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":100,\"peak_memory_bytes\":4096}\n",
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-compiled\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":80,\"peak_memory_bytes\":2048}\n",
        "{\"case_label\":\"surface-detect-d13-r13\",\"tool_variant\":\"stim-cli\",\"workload\":\"detect\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":169,\"measurements\":312,\"detectors\":144,\"observables\":1,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":10000,\"wall_time_ns\":240,\"peak_memory_bytes\":4096}\n",
        "{\"case_label\":\"surface-detect-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"detect\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":169,\"measurements\":312,\"detectors\":144,\"observables\":1,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":10000,\"wall_time_ns\":210,\"peak_memory_bytes\":8192}\n",
        "{\"case_label\":\"surface-detect-d13-r13\",\"tool_variant\":\"rstim-compiled\",\"workload\":\"detect\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":169,\"measurements\":312,\"detectors\":144,\"observables\":1,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":10000,\"wall_time_ns\":170,\"peak_memory_bytes\":6144}\n",
        "{\"case_label\":\"repeat-analyze-large\",\"tool_variant\":\"stim-cli\",\"workload\":\"analyze_errors\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":4096,\"shots\":null,\"wall_time_ns\":700,\"peak_memory_bytes\":512}\n",
        "{\"case_label\":\"repeat-analyze-large\",\"tool_variant\":\"rstim-analyzer-flattened\",\"workload\":\"analyze_errors\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":4096,\"shots\":null,\"wall_time_ns\":600,\"peak_memory_bytes\":1024}\n",
        "{\"case_label\":\"repeat-analyze-large\",\"tool_variant\":\"rstim-analyzer-compiled\",\"workload\":\"analyze_errors\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":4096,\"shots\":null,\"wall_time_ns\":500,\"peak_memory_bytes\":768}\n",
        "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":80,\"peak_memory_bytes\":128}\n",
        "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":70,\"peak_memory_bytes\":256}\n"
    );

    fn cli_test_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn lock_cli_test_env() -> MutexGuard<'static, ()> {
        cli_test_env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    #[cfg(not(feature = "benchmark-telemetry"))]
    fn benchmark_telemetry_flag_requires_feature() {
        let dir = tempfile::tempdir().unwrap();
        let telemetry_path = dir.path().join("telemetry.json");

        let err = run(Cli {
            benchmark_telemetry_json: Some(telemetry_path.display().to_string()),
            command: Some(Commands::Stats {
                r#in: None,
                out: None,
                json: false,
            }),
        })
        .unwrap_err();

        assert!(err.contains("--benchmark-telemetry-json"));
        assert!(err.contains("benchmark-telemetry"));
        assert!(!telemetry_path.exists());
    }

    #[test]
    fn merge_detections_observables_rejects_shot_count_mismatch() {
        let dets = BitTable::try_new(1, 2).unwrap();
        let obs = BitTable::try_new(1, 1).unwrap();

        let err = try_merge_detections_observables(&dets, &obs).unwrap_err();

        assert!(err.contains("observable shot count 1 does not match detection shot count 2"));
    }

    #[test]
    #[cfg(feature = "benchmark-telemetry")]
    fn benchmark_telemetry_json_records_sample_noise_operations() {
        let dir = tempfile::tempdir().unwrap();
        let input_path = dir.path().join("input.stim");
        let output_path = dir.path().join("shots.01");
        let telemetry_path = dir.path().join("telemetry.json");
        std::fs::write(
            &input_path,
            "X_ERROR(0.001) 0 1 2\nDEPOLARIZE1(0.3) 0 1 2\nM 0 1 2\n",
        )
        .unwrap();

        run(Cli {
            benchmark_telemetry_json: Some(telemetry_path.display().to_string()),
            command: Some(Commands::Sample {
                shots: Some(17),
                out_format: "01".to_string(),
                r#in: Some(input_path.display().to_string()),
                out: Some(output_path.display().to_string()),
                seed: Some(463),
                skip_reference_sample: false,
            }),
        })
        .unwrap();

        let telemetry: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(telemetry_path).unwrap()).unwrap();
        let operations = telemetry["operations"].as_array().unwrap();
        assert_eq!(operations.len(), 2);
        assert_eq!(operations[0]["operation"], "X_ERROR");
        assert_eq!(operations[0]["sampling_path"], "sparse");
        assert_eq!(operations[0]["targets"], 3);
        assert!(operations[0].get("pairs").is_none());
        assert_eq!(operations[0]["iterator_builds"], 1);
        assert_eq!(operations[0]["attempt_count"], 51);
        assert_eq!(operations[1]["operation"], "DEPOLARIZE1");
        assert_eq!(operations[1]["sampling_path"], "dense");
        assert_eq!(operations[1]["targets"], 3);
        assert!(operations[1].get("pairs").is_none());
        assert_eq!(operations[1]["iterator_builds"], 0);
        assert_eq!(operations[1]["attempt_count"], 51);
    }

    fn write_fake_executable(path: &std::path::Path, body: &str) {
        std::fs::write(path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(path, perms).unwrap();
        }
    }

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
            benchmark_telemetry_json: None,
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
    fn run_without_subcommand_reports_version() {
        run(Cli {
            benchmark_telemetry_json: None,
            command: None,
        })
        .unwrap();
    }

    #[test]
    fn run_dispatches_perf_ci_error_paths_in_process() {
        let _guard = lock_cli_test_env();
        let dir = tempfile::tempdir().unwrap();
        let regression_raw_path = dir.path().join("regression.jsonl");
        let gate_out_dir = dir.path().join("gate");
        let missing_raw_path = dir.path().join("missing.jsonl");
        let infra_out_dir = dir.path().join("infra");
        let regression_raw = PERF_PASS_RAW.replacen(
            "\"shots\":20000,\"wall_time_ns\":80,\"peak_memory_bytes\":2048",
            "\"shots\":20000,\"wall_time_ns\":111,\"peak_memory_bytes\":2048",
            1,
        );
        std::fs::write(&regression_raw_path, regression_raw).unwrap();

        unsafe {
            std::env::set_var("RSTIM_TEST_PERF_CI_RAW", &regression_raw_path);
        }
        let gate_err = run(Cli {
            benchmark_telemetry_json: None,
            command: Some(Commands::Perf {
                command: PerfCommands::Ci {
                    out_dir: gate_out_dir.display().to_string(),
                    case: None,
                    warmup_rounds: 1,
                    measure_rounds: 5,
                },
            }),
        })
        .unwrap_err();
        unsafe {
            std::env::remove_var("RSTIM_TEST_PERF_CI_RAW");
        }

        assert!(!gate_err.starts_with("InfrastructureFailure"));
        assert!(gate_err.contains("RegressionFailure") || gate_err.contains("exceeds threshold"));
        assert!(std::fs::read_to_string(gate_out_dir.join("summary.json"))
            .unwrap()
            .contains("\"cases\""));

        unsafe {
            std::env::set_var("RSTIM_TEST_PERF_CI_RAW", &missing_raw_path);
        }
        let infra_err = run(Cli {
            benchmark_telemetry_json: None,
            command: Some(Commands::Perf {
                command: PerfCommands::Ci {
                    out_dir: infra_out_dir.display().to_string(),
                    case: None,
                    warmup_rounds: 1,
                    measure_rounds: 5,
                },
            }),
        })
        .unwrap_err();
        unsafe {
            std::env::remove_var("RSTIM_TEST_PERF_CI_RAW");
        }

        assert!(infra_err.starts_with("InfrastructureFailure"));
        assert!(infra_err.contains("failed to copy test perf raw artifact"));
    }

    #[test]
    fn run_dispatches_perf_run_case_and_suite_paths_in_process() {
        let _guard = lock_cli_test_env();
        let dir = tempfile::tempdir().unwrap();
        let fake_stim = dir.path().join("fake-stim-fail");
        let selected_out = dir.path().join("selected.jsonl");
        let suite_out = dir.path().join("suite.jsonl");
        let ci_raw_path = dir.path().join("ci-raw.jsonl");
        write_fake_executable(
            &fake_stim,
            "#!/bin/sh\ncat >/dev/null\necho 'stim exploded' >&2\nexit 1\n",
        );

        unsafe {
            std::env::set_var("RSTIM_TEST_STIM", &fake_stim);
        }
        run(Cli {
            benchmark_telemetry_json: None,
            command: Some(Commands::Perf {
                command: PerfCommands::Run {
                    out: Some(selected_out.display().to_string()),
                    case: Some("loss-protection-sample".to_string()),
                    warmup_rounds: 0,
                    measure_rounds: 1,
                },
            }),
        })
        .unwrap();
        let suite_err = run(Cli {
            benchmark_telemetry_json: None,
            command: Some(Commands::Perf {
                command: PerfCommands::Run {
                    out: Some(suite_out.display().to_string()),
                    case: None,
                    warmup_rounds: 0,
                    measure_rounds: 1,
                },
            }),
        })
        .unwrap_err();
        let ci_err = write_perf_ci_raw_artifact(&ci_raw_path, 0, 1, None).unwrap_err();
        unsafe {
            std::env::remove_var("RSTIM_TEST_STIM");
        }

        let selected_raw = std::fs::read_to_string(selected_out).unwrap();
        assert!(selected_raw.contains("\"case_label\":\"loss-protection-sample\""));
        assert!(selected_raw.contains("\"status\":\"tool_failed\""));
        assert!(suite_err.contains("stim failed: stim exploded"));
        assert!(matches!(ci_err, PerfCiError::Infrastructure(_)));
        let ci_message = match ci_err {
            PerfCiError::Infrastructure(message) | PerfCiError::Gate(message) => message,
        };
        assert!(ci_message.contains("stim failed: stim exploded"));
    }

    #[test]
    fn selected_perf_ci_override_reports_read_parse_and_write_errors() {
        let _guard = lock_cli_test_env();
        let dir = tempfile::tempdir().unwrap();
        let missing_raw_path = dir.path().join("missing.jsonl");
        let invalid_raw_path = dir.path().join("invalid.jsonl");
        let valid_raw_path = dir.path().join("valid.jsonl");
        let raw_path = dir.path().join("raw.jsonl");
        let raw_dir_path = dir.path().join("raw-dir");
        std::fs::write(&invalid_raw_path, "not json\n").unwrap();
        std::fs::write(
            &valid_raw_path,
            "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":80,\"peak_memory_bytes\":128}\n",
        )
        .unwrap();
        std::fs::create_dir(&raw_dir_path).unwrap();

        let read_err = write_test_perf_ci_raw_override(
            missing_raw_path.to_str().unwrap(),
            &raw_path,
            Some("loss-protection-sample"),
        )
        .unwrap_err();
        let parse_err = write_test_perf_ci_raw_override(
            invalid_raw_path.to_str().unwrap(),
            &raw_path,
            Some("loss-protection-sample"),
        )
        .unwrap_err();
        let write_err = write_test_perf_ci_raw_override(
            valid_raw_path.to_str().unwrap(),
            &raw_dir_path,
            Some("loss-protection-sample"),
        )
        .unwrap_err();

        assert!(matches!(read_err, PerfCiError::Infrastructure(_)));
        let read_message = match read_err {
            PerfCiError::Infrastructure(message) | PerfCiError::Gate(message) => message,
        };
        assert!(read_message.contains("failed to read test perf raw artifact"));
        assert!(matches!(parse_err, PerfCiError::Infrastructure(_)));
        let parse_message = match parse_err {
            PerfCiError::Infrastructure(message) | PerfCiError::Gate(message) => message,
        };
        assert!(parse_message.contains("failed to parse test perf raw artifact"));
        assert!(matches!(write_err, PerfCiError::Infrastructure(_)));
        let write_message = match write_err {
            PerfCiError::Infrastructure(message) | PerfCiError::Gate(message) => message,
        };
        assert!(write_message.contains("failed to write filtered test perf raw artifact"));
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
            None,
        )
        .unwrap_err();

        match err {
            PerfCiError::Gate(message) => {
                assert!(
                    message.contains("RegressionFailure") || message.contains("exceeds threshold")
                );
            }
            PerfCiError::Infrastructure(message) => {
                panic!("unexpected infrastructure failure: {message}");
            }
        }

        let summary_text = std::fs::read_to_string(&summary_path).unwrap();
        let report_text = std::fs::read_to_string(&report_path).unwrap();
        assert!(summary_text.contains("\"cases\""));
        assert!(report_text.contains("## Gate Verdict"));
        assert!(
            report_text.contains("RegressionFailure") || report_text.contains("exceeds threshold")
        );
    }

    #[test]
    fn finalize_perf_ci_artifacts_returns_ok_for_passing_summary() {
        let dir = tempfile::tempdir().unwrap();
        let summary_path = dir.path().join("summary.json");
        let report_path = dir.path().join("report.md");

        finalize_perf_ci_artifacts(
            PERF_PASS_RAW,
            &summary_path,
            &report_path,
            crate::perf::PerfGateConfig::default(),
            None,
        )
        .unwrap_or_else(|_| panic!("passing perf ci artifacts"));

        let report_text = std::fs::read_to_string(&report_path).unwrap();
        assert!(report_text.contains("## Gate Verdict"));
        assert!(report_text.contains("PASS"));
    }

    #[test]
    fn run_gen_and_generator_helpers_cover_supported_perf_sources() {
        let mut out = Vec::new();
        run_gen("surface_code", "rotated_memory_z", 3, 1, 0.0, &mut out).unwrap();
        assert!(!out.is_empty());
        for (code, task, rounds) in [
            ("repetition_code", "memory", 1),
            ("surface_code", "rotated_memory_x", 1),
            ("surface_code", "unrotated_memory_x", 1),
            ("surface_code", "unrotated_memory_z", 1),
            ("color_code", "memory_xyz", 2),
        ] {
            assert!(generate_common_circuit_text(code, task, 3, rounds, 0.0)
                .unwrap()
                .contains("QUBIT_COORDS"));
        }
        assert!(
            generate_common_circuit_text("surface_code", "unknown", 3, 1, 0.0)
                .unwrap_err()
                .contains("unknown code/task")
        );
    }

    fn write_css_json(dir: &tempfile::TempDir, name: &str, text: &str) -> String {
        let path = dir.path().join(name);
        std::fs::write(&path, text).unwrap();
        path.display().to_string()
    }

    #[test]
    fn run_dispatches_css_gen_command_in_process() {
        let dir = tempfile::tempdir().unwrap();
        let hx = write_css_json(
            &dir,
            "hx.json",
            r#"{"format":"sparse_rows","num_cols":2,"rows":[]}"#,
        );
        let hz = write_css_json(
            &dir,
            "hz.json",
            r#"{"format":"sparse_rows","num_cols":2,"rows":[[0,1]]}"#,
        );
        let observables = write_css_json(
            &dir,
            "obs.json",
            r#"{"format":"sparse_rows","num_cols":2,"rows":[[0]]}"#,
        );
        let out = dir.path().join("memory.stim");

        run(Cli {
            benchmark_telemetry_json: None,
            command: Some(Commands::Gen {
                code: "css".to_string(),
                task: "memory".to_string(),
                distance: None,
                rounds: 2,
                noise: 0.0,
                after_clifford_loss_probability: 0.0,
                hx: Some(hx),
                hz: Some(hz),
                basis: Some("Z".to_string()),
                schedule: "sequential".to_string(),
                observables: Some(observables),
                out: Some(out.display().to_string()),
            }),
        })
        .unwrap();

        let text = std::fs::read_to_string(out).unwrap();
        assert!(text.contains("R 0"));
        assert!(text.contains("M 0"));
        assert!(text.contains("MR 2"));
        assert!(text.contains("OBSERVABLE_INCLUDE"));
    }

    #[test]
    fn run_css_gen_accepts_canonical_fallback_without_observable_file() {
        let dir = tempfile::tempdir().unwrap();
        let h = r#"{"format":"sparse_rows","num_cols":7,"rows":[[0,3,5,6],[1,3,4,6],[2,4,5,6]]}"#;
        let hx = write_css_json(&dir, "steane_hx.json", h);
        let hz = write_css_json(&dir, "steane_hz.json", h);
        let mut out = Vec::new();

        run_css_gen(
            "memory",
            Some(&hx),
            Some(&hz),
            Some("x"),
            1,
            0.0,
            "greedy",
            None,
            &mut out,
        )
        .unwrap();

        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("OBSERVABLE_INCLUDE"));
        assert_eq!(parse_memory_basis("X").unwrap(), MemoryBasis::X);
        assert_eq!(parse_memory_basis("z").unwrap(), MemoryBasis::Z);
        assert_eq!(parse_css_schedule("greedy").unwrap(), CssSchedule::Greedy);
        assert_eq!(
            parse_css_schedule("sequential").unwrap(),
            CssSchedule::Sequential
        );
    }

    #[test]
    fn run_css_gen_reports_input_errors_in_process() {
        let dir = tempfile::tempdir().unwrap();
        let hx = write_css_json(
            &dir,
            "hx.json",
            r#"{"format":"sparse_rows","num_cols":2,"rows":[[0,1]]}"#,
        );
        let hz = write_css_json(
            &dir,
            "hz.json",
            r#"{"format":"sparse_rows","num_cols":2,"rows":[]}"#,
        );
        let hz_wide = write_css_json(
            &dir,
            "hz_wide.json",
            r#"{"format":"sparse_rows","num_cols":3,"rows":[]}"#,
        );
        let obs_wide = write_css_json(
            &dir,
            "obs_wide.json",
            r#"{"format":"sparse_rows","num_cols":3,"rows":[[0,1,2]]}"#,
        );

        let err = run_css_gen(
            "stability",
            None,
            None,
            None,
            1,
            0.0,
            "greedy",
            None,
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(err.contains("unknown css task"), "error was: {err}");

        let err = run_css_gen(
            "memory",
            None,
            Some(&hz),
            Some("x"),
            1,
            0.0,
            "greedy",
            None,
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(err.contains("--hx is required"), "error was: {err}");

        let err = run_css_gen(
            "memory",
            Some(&hx),
            None,
            Some("x"),
            1,
            0.0,
            "greedy",
            None,
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(err.contains("--hz is required"), "error was: {err}");

        let err = run_css_gen(
            "memory",
            Some(&hx),
            Some(&hz_wide),
            Some("x"),
            1,
            0.0,
            "greedy",
            None,
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(err.contains("hx and hz widths differ"), "error was: {err}");

        let err = run_css_gen(
            "memory",
            Some(&hx),
            Some(&hz),
            Some("x"),
            1,
            0.0,
            "greedy",
            Some(&obs_wide),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(err.contains("observable width differs"), "error was: {err}");

        let err = run_css_gen(
            "memory",
            Some(&hx),
            Some(&hz),
            None,
            1,
            0.0,
            "greedy",
            None,
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(err.contains("--basis is required"), "error was: {err}");

        let err = run_css_gen(
            "memory",
            Some(&hx),
            Some(&hz),
            Some("y"),
            1,
            0.0,
            "greedy",
            None,
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(err.contains("unknown CSS memory basis"), "error was: {err}");

        let err = run_css_gen(
            "memory",
            Some(&hx),
            Some(&hz),
            Some("x"),
            1,
            0.0,
            "layered",
            None,
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(err.contains("unknown CSS schedule"), "error was: {err}");
    }

    #[test]
    fn run_common_gen_missing_distance_is_in_process_error_without_touching_out() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.stim");
        std::fs::write(&out, "keep me").unwrap();

        let err = run(Cli {
            benchmark_telemetry_json: None,
            command: Some(Commands::Gen {
                code: "repetition_code".to_string(),
                task: "memory".to_string(),
                distance: None,
                rounds: 1,
                noise: 0.0,
                after_clifford_loss_probability: 0.0,
                hx: None,
                hz: None,
                basis: None,
                schedule: "greedy".to_string(),
                observables: None,
                out: Some(out.display().to_string()),
            }),
        })
        .unwrap_err();

        assert!(
            err.contains("distance is required for common generators"),
            "error was: {err}"
        );
        assert_eq!(std::fs::read_to_string(out).unwrap(), "keep me");
    }

    #[test]
    fn run_dispatches_perf_summarize_gate_and_report_commands_in_process() {
        let dir = tempfile::tempdir().unwrap();
        let raw_path = dir.path().join("raw.jsonl");
        let summary_path = dir.path().join("summary.json");
        let report_path = dir.path().join("report.md");
        std::fs::write(&raw_path, PERF_PASS_RAW).unwrap();

        run(Cli {
            benchmark_telemetry_json: None,
            command: Some(Commands::Perf {
                command: PerfCommands::Summarize {
                    case: None,
                    r#in: Some(raw_path.display().to_string()),
                    out: Some(summary_path.display().to_string()),
                },
            }),
        })
        .unwrap();

        run(Cli {
            benchmark_telemetry_json: None,
            command: Some(Commands::Perf {
                command: PerfCommands::Gate {
                    r#in: Some(summary_path.display().to_string()),
                    sampler_threshold: 1.10,
                    analyzer_threshold: 1.10,
                },
            }),
        })
        .unwrap();

        run(Cli {
            benchmark_telemetry_json: None,
            command: Some(Commands::Perf {
                command: PerfCommands::Report {
                    r#in: Some(summary_path.display().to_string()),
                    out: Some(report_path.display().to_string()),
                },
            }),
        })
        .unwrap();

        let report_text = std::fs::read_to_string(&report_path).unwrap();
        assert!(report_text.contains("## Gating Cases"));
        assert!(report_text.contains("rep-sample-d13-r13"));
    }

    #[test]
    fn run_dispatches_perf_summarize_selected_case_in_process() {
        let dir = tempfile::tempdir().unwrap();
        let raw_path = dir.path().join("raw.jsonl");
        let summary_path = dir.path().join("summary.json");
        std::fs::write(&raw_path, PERF_PASS_RAW).unwrap();

        run(Cli {
            benchmark_telemetry_json: None,
            command: Some(Commands::Perf {
                command: PerfCommands::Summarize {
                    case: Some("rep-sample-d13-r13".to_string()),
                    r#in: Some(raw_path.display().to_string()),
                    out: Some(summary_path.display().to_string()),
                },
            }),
        })
        .unwrap();

        let summary: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(summary_path).unwrap()).unwrap();
        let cases = summary["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0]["case_label"], "rep-sample-d13-r13");
        assert!(summary["issues"].as_array().unwrap().is_empty());
    }

    #[test]
    fn run_dispatches_perf_gate_failure_in_process() {
        let dir = tempfile::tempdir().unwrap();
        let summary_path = dir.path().join("summary.json");
        let summary = crate::perf::summarize_jsonl_str(concat!(
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
        ))
        .unwrap();
        std::fs::write(&summary_path, serde_json::to_vec_pretty(&summary).unwrap()).unwrap();

        let err = run(Cli {
            benchmark_telemetry_json: None,
            command: Some(Commands::Perf {
                command: PerfCommands::Gate {
                    r#in: Some(summary_path.display().to_string()),
                    sampler_threshold: 1.10,
                    analyzer_threshold: 1.10,
                },
            }),
        })
        .unwrap_err();

        assert!(err.contains("RegressionFailure") || err.contains("exceeds threshold"));
    }
}
