use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Instant;

use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::codegen;
use crate::error_analyzer::{AnalyzeBackend, AnalyzeOptions, ErrorAnalyzer};
use crate::ir::circuit_to_string;
use crate::parser::parse_lines;
use crate::sampler::{SampleOptions, SamplingBackend, sample_batch_with_options};
use crate::stats::summarize;

use super::{
    PerfBenchmarkCase, PerfMeasurementRecord, PerfVariant, PerfWorkload, benchmark_case_variants,
    benchmark_cases, effective_repeat_count,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerfRunOptions {
    pub warmup_rounds: usize,
    pub measured_rounds: usize,
}

impl Default for PerfRunOptions {
    fn default() -> Self {
        Self {
            warmup_rounds: 1,
            measured_rounds: 5,
        }
    }
}

fn source_text(source: super::PerfCircuitSource) -> String {
    match source {
        super::PerfCircuitSource::Inline { text } => text.to_string(),
        super::PerfCircuitSource::Generator {
            code,
            task,
            distance,
            rounds,
            noise,
        } => {
            let instrs = match (code, task) {
                ("repetition_code", "memory") => {
                    codegen::repetition_code_memory(distance, rounds, noise)
                }
                ("surface_code", "rotated_memory_x") => {
                    codegen::surface_code::rotated_memory_x(distance, rounds, noise)
                }
                ("surface_code", "rotated_memory_z") => {
                    codegen::surface_code::rotated_memory_z(distance, rounds, noise)
                }
                ("surface_code", "unrotated_memory_x") => {
                    codegen::surface_code::unrotated_memory_x(distance, rounds, noise)
                }
                ("surface_code", "unrotated_memory_z") => {
                    codegen::surface_code::unrotated_memory_z(distance, rounds, noise)
                }
                ("color_code", "memory_xyz") => {
                    codegen::color_code::memory_xyz(distance, rounds, noise)
                }
                _ => panic!("unknown benchmark code/task: {code}/{task}"),
            };
            circuit_to_string(&instrs)
        }
    }
}

fn current_peak_memory_bytes() -> Option<u64> {
    #[cfg(unix)]
    {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
        // SAFETY: getrusage initializes the provided struct on success.
        let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
        if rc != 0 {
            return None;
        }
        // SAFETY: guarded by the successful getrusage call above.
        let usage = unsafe { usage.assume_init() };
        #[cfg(target_os = "macos")]
        {
            Some(usage.ru_maxrss as u64)
        }
        #[cfg(not(target_os = "macos"))]
        {
            Some((usage.ru_maxrss as u64).saturating_mul(1024))
        }
    }
    #[cfg(not(unix))]
    {
        None
    }
}

fn run_variant(case: PerfBenchmarkCase, text: &str, variant: PerfVariant) -> Result<u128, String> {
    let instrs = parse_lines(text)?;
    let start = Instant::now();

    match case.workload {
        PerfWorkload::Sample | PerfWorkload::Detect => {
            let backend = match variant {
                PerfVariant::RstimInterpreted => SamplingBackend::Interpreted,
                PerfVariant::RstimCompiled => SamplingBackend::Compiled,
                PerfVariant::StimCli => return Ok(run_stim_cli(case, text)?),
                _ => SamplingBackend::Auto,
            };
            let mut rng = StdRng::seed_from_u64(1234);
            sample_batch_with_options(
                &instrs,
                case.shots.unwrap_or(1),
                &mut rng,
                SampleOptions {
                    backend,
                    ..SampleOptions::default()
                },
            )?;
        }
        PerfWorkload::AnalyzeErrors => {
            let backend = match variant {
                PerfVariant::RstimAnalyzerFlattened => AnalyzeBackend::Flattened,
                PerfVariant::RstimAnalyzerCompiled => AnalyzeBackend::Compiled,
                PerfVariant::StimCli => return Ok(run_stim_cli(case, text)?),
                _ => AnalyzeBackend::Auto,
            };
            ErrorAnalyzer::circuit_to_dem_with_options(
                &instrs,
                AnalyzeOptions {
                    backend,
                    ..AnalyzeOptions::default()
                },
            )?;
        }
    }

    Ok(start.elapsed().as_nanos())
}

fn run_stim_cli(case: PerfBenchmarkCase, text: &str) -> Result<u128, String> {
    let stim_cmd = std::env::var("RSTIM_TEST_STIM").unwrap_or_else(|_| "stim".to_string());
    let args = match case.workload {
        PerfWorkload::Sample => vec![
            "sample".to_string(),
            "--shots".to_string(),
            case.shots.unwrap_or(1).to_string(),
        ],
        PerfWorkload::Detect => vec![
            "detect".to_string(),
            "--shots".to_string(),
            case.shots.unwrap_or(1).to_string(),
        ],
        PerfWorkload::AnalyzeErrors => vec!["analyze_errors".to_string()],
    };

    let mut child = Command::new(stim_cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn stim: {e}"))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "missing stim stdin".to_string())?
        .write_all(text.as_bytes())
        .map_err(|e| format!("failed to write stim stdin: {e}"))?;
    let start = Instant::now();
    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for stim: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "stim failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(start.elapsed().as_nanos())
}

pub fn run_case_measurements(
    case: PerfBenchmarkCase,
    text: &str,
    variants: &[PerfVariant],
    options: PerfRunOptions,
) -> Result<Vec<PerfMeasurementRecord>, String> {
    let instrs = parse_lines(text)?;
    let summary = summarize(&instrs);
    let mut records = Vec::new();
    let total_rounds = options.warmup_rounds + options.measured_rounds;

    for variant in variants {
        for measurement_index in 0..total_rounds {
            let warmup = measurement_index < options.warmup_rounds;
            let wall_time_ns = run_variant(case, text, *variant)?;
            records.push(PerfMeasurementRecord {
                case_label: case.label.to_string(),
                tool_variant: variant.label().to_string(),
                workload: case.workload.as_str().to_string(),
                tier: case.tier.as_str().to_string(),
                measurement_index,
                warmup,
                qubits: summary.num_qubits,
                measurements: summary.num_measurements,
                detectors: summary.num_detectors,
                observables: summary.num_observables,
                repeat_depth: summary.max_repeat_depth,
                repeat_count: effective_repeat_count(&instrs),
                shots: case.shots,
                wall_time_ns,
                peak_memory_bytes: current_peak_memory_bytes(),
            });
        }
    }

    Ok(records)
}

pub fn run_benchmark_suite_to_writer(
    out: &mut dyn Write,
    options: PerfRunOptions,
) -> Result<(), String> {
    for case in benchmark_cases() {
        let text = source_text(case.source);
        let instrs = parse_lines(&text)?;
        let variants = benchmark_case_variants(case, &instrs)?;
        let records = run_case_measurements(case, &text, &variants, options)?;
        for record in records {
            out.write_all(record.to_json_line().as_bytes())
                .map_err(|e| format!("failed to write perf record: {e}"))?;
        }
    }
    Ok(())
}
