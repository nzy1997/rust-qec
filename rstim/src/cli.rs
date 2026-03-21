use std::io::{self, Read, Write};

use clap::{Parser, Subcommand};
use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::dem::DetectorErrorModel;
use crate::error_analyzer::ErrorAnalyzer;
use crate::m2d::{M2dOptions, measurements_to_detections_with_options};
use crate::output::{
    OutputFormat, write_shots_01, write_shots_b8, write_shots_r8, write_shots_hits, write_shots_dets, write_shots_ptb64,
};
use crate::parser::parse_lines;
use crate::sampler::{sample_batch, sample_batch_with_options, SampleOptions};
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
    /// Export a circuit as QSTD101 JSON
    #[command(name = "export_json")]
    ExportJson {
        #[arg(long = "in")]
        r#in: Option<String>,
        #[arg(long)]
        out: Option<String>,
        #[arg(long, default_value = "pretty")]
        format: String,
    },
}

#[derive(Clone, Copy)]
enum JsonOutputFormat {
    Pretty,
    Compact,
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
        Some(Commands::Sample { shots, out_format, r#in, out, seed, skip_reference_sample }) => {
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
                run_detect(&text, shots.unwrap_or(1) as usize, &out_format, seed, append_observables, &mut w)
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
        Some(Commands::Gen { code, task, distance, rounds, noise, out }) => {
            let mut w = open_output(out.as_deref())?;
            run_gen(&code, &task, distance, rounds, noise, &mut w)
        }
        Some(Commands::Convert { in_format, out_format, bits, circuit, r#in, out, shots }) => {
            let data = read_input_bytes(r#in.as_deref())?;
            let mut w = open_output(out.as_deref())?;
            run_convert(&data, &in_format, &out_format, bits, circuit.as_deref(), shots, &mut w)
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
                sweep_data.as_deref().map(|data| (data, sweep_format.as_str())),
                options,
                append_observables,
                &mut w,
            )
        }
        Some(Commands::ExplainErrors { r#in, in_format, circuit, dem, out }) => {
            let det_data = read_input_bytes(r#in.as_deref())?;
            let circuit_text = circuit.as_deref()
                .map(|p| std::fs::read_to_string(p).map_err(|e| e.to_string()))
                .transpose()?;
            let dem_text = dem.as_deref()
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
        Some(Commands::ExportJson { r#in, out, format }) => {
            let text = read_input(r#in.as_deref())?;
            let format = parse_json_output_format(&format)?;
            let mut w = open_output(out.as_deref())?;
            run_export_json(&text, format, &mut w)
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
            write_shots_dets(detections, observable_flips, out).map_err(|e| format!("write error: {e}"))?;
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
    let instrs = match (code, task) {
        ("repetition_code", "memory") => crate::codegen::repetition_code_memory(distance, rounds, noise),
        ("surface_code", "rotated_memory_x") => crate::codegen::surface_code::rotated_memory_x(distance, rounds, noise),
        ("surface_code", "rotated_memory_z") => crate::codegen::surface_code::rotated_memory_z(distance, rounds, noise),
        ("surface_code", "unrotated_memory_x") => crate::codegen::surface_code::unrotated_memory_x(distance, rounds, noise),
        ("surface_code", "unrotated_memory_z") => crate::codegen::surface_code::unrotated_memory_z(distance, rounds, noise),
        ("color_code", "memory_xyz") => crate::codegen::color_code::memory_xyz(distance, rounds, noise),
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
    };
    let result = sample_batch_with_options(&instrs, shots, &mut rng, options)?;
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

pub fn run_analyze_errors(
    circuit_text: &str,
    out: &mut dyn Write,
) -> Result<(), String> {
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
        approximate_disjoint_errors,
        allow_gauge_detectors,
    };
    let dem = if decompose_errors {
        ErrorAnalyzer::circuit_to_dem_with_options_decomposed(&instrs, options)?
    } else {
        ErrorAnalyzer::circuit_to_dem_with_options(&instrs, options)?
    };
    let dem_str = dem.to_string();
    out.write_all(dem_str.as_bytes()).map_err(|e| format!("write error: {e}"))
}

fn run_export_json(text: &str, format: JsonOutputFormat, w: &mut dyn Write) -> Result<(), String> {
    let instrs = parse_lines(text)?;
    let doc = crate::qstd101::export_qstd101(&instrs)?;
    match format {
        JsonOutputFormat::Pretty => {
            serde_json::to_writer_pretty(&mut *w, &doc).map_err(|e| format!("write error: {e}"))?
        }
        JsonOutputFormat::Compact => {
            serde_json::to_writer(&mut *w, &doc).map_err(|e| format!("write error: {e}"))?
        }
    }
    w.write_all(b"\n").map_err(|e| format!("write error: {e}"))?;
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
            std::io::stdin().read_to_end(&mut buf).map_err(|e| e.to_string())?;
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
    }.map_err(|e| e.to_string())
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
        Some(read_table_from_format(sweep_bytes, sweep_format, n_sweep_bits, shots)?)
    } else {
        None
    };
    let result = measurements_to_detections_with_options(&instrs, &meas_table, sweep_table.as_ref(), options)?;
    let fmt = OutputFormat::from_str(out_format)?;

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
                if !line.starts_with("shot") { continue; }
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
        _ => Err(format!("unsupported in_format for explain_errors: {format}")),
    }
}
