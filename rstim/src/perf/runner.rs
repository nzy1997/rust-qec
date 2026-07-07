use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::cli::generate_common_circuit_text;
use crate::error_analyzer::{AnalyzeBackend, AnalyzeOptions, ErrorAnalyzer};
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

impl PerfRunOptions {
    fn validate(self) -> Result<Self, String> {
        if self.measured_rounds == 0 {
            return Err("PerfRunOptions.measured_rounds must be greater than 0".to_string());
        }
        Ok(self)
    }
}

fn source_text(source: super::PerfCircuitSource) -> Result<String, String> {
    match source {
        super::PerfCircuitSource::Inline { text } => Ok(text.to_string()),
        super::PerfCircuitSource::Fixture {
            canonical_input_path,
            ..
        } => fixture_text(canonical_input_path),
        super::PerfCircuitSource::Generator {
            code,
            task,
            distance,
            rounds,
            noise,
        } => generate_common_circuit_text(code, task, distance, rounds, noise),
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .to_path_buf()
}

fn fixture_text(canonical_input_path: &str) -> Result<String, String> {
    let path = workspace_root().join(canonical_input_path);
    std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read perf fixture {}: {e}", path.display()))
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
    let options = options.validate()?;
    let instrs = parse_lines(text)?;
    let summary = summarize(&instrs);
    let mut records = Vec::new();
    let total_rounds = options.warmup_rounds + options.measured_rounds;
    let repeat_count = effective_repeat_count(&instrs);

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
                repeat_count,
                shots: case.shots,
                wall_time_ns,
                peak_memory_bytes: current_peak_memory_bytes(),
            });
        }
    }

    Ok(records)
}

pub(crate) fn write_case_measurements_to_writer(
    out: &mut dyn Write,
    case: PerfBenchmarkCase,
    text: &str,
    variants: &[PerfVariant],
    options: PerfRunOptions,
) -> Result<(), String> {
    let records = run_case_measurements(case, text, variants, options)?;
    for record in records {
        out.write_all(record.to_json_line().as_bytes())
            .map_err(|e| format!("failed to write perf record: {e}"))?;
    }
    Ok(())
}

pub fn run_benchmark_suite_to_writer(
    out: &mut dyn Write,
    options: PerfRunOptions,
) -> Result<(), String> {
    for case in benchmark_cases() {
        let text = source_text(case.source)?;
        let instrs = parse_lines(&text)?;
        let variants = benchmark_case_variants(case, &instrs)?;
        write_case_measurements_to_writer(out, case, &text, &variants, options)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perf::{PerfCaseTier, PerfCircuitSource, PerfNoiseMetadata};

    #[test]
    fn source_text_returns_error_for_unknown_generator_pair() {
        let result = source_text(PerfCircuitSource::Generator {
            code: "surface_code",
            task: "unknown_task",
            distance: 3,
            rounds: 3,
            noise: 0.001,
        });

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown code/task"));
    }

    #[test]
    fn source_text_loads_checked_fixture_text() {
        let text = source_text(PerfCircuitSource::Fixture {
            case_id: "stim_surface_d11_r100",
            canonical_input_path:
                "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
            noise: PerfNoiseMetadata {
                before_round_data_depolarization: 0.0,
                after_clifford_depolarization: 0.001,
                before_measure_flip_probability: 0.001,
                after_reset_flip_probability: 0.001,
            },
        })
        .expect("checked fixture text");

        assert!(text.starts_with("# Generated surface_code circuit."));
        assert!(text.contains("QUBIT_COORDS(1, 1) 1"));
    }

    #[test]
    fn writer_helper_emits_jsonl_records_with_newlines_and_variant_order() {
        let case = PerfBenchmarkCase {
            label: "inline-sample",
            workload: PerfWorkload::Sample,
            source: PerfCircuitSource::Inline {
                text: "X_ERROR(0.001) 0\nM 0\n",
            },
            shots: Some(32),
            tier: PerfCaseTier::Gating,
            requires_compiled: true,
            requires_fallback: false,
            comparisons: &[],
        };
        let variants = [PerfVariant::RstimInterpreted, PerfVariant::RstimCompiled];
        let mut out = Vec::new();

        write_case_measurements_to_writer(
            &mut out,
            case,
            "X_ERROR(0.001) 0\nM 0\n",
            &variants,
            PerfRunOptions {
                warmup_rounds: 1,
                measured_rounds: 2,
            },
        )
        .expect("write case measurements");

        let text = String::from_utf8(out).expect("utf8 jsonl");
        let lines: Vec<&str> = text.lines().collect();
        let records: Vec<PerfMeasurementRecord> = lines
            .iter()
            .map(|line| PerfMeasurementRecord::from_json_line(line).expect("jsonl record"))
            .collect();

        assert_eq!(lines.len(), 6);
        assert!(text.ends_with('\n'));
        assert_eq!(text.matches('\n').count(), lines.len());
        assert_eq!(
            records
                .iter()
                .map(|record| record.tool_variant.as_str())
                .collect::<Vec<_>>(),
            vec![
                "rstim-interpreted",
                "rstim-interpreted",
                "rstim-interpreted",
                "rstim-compiled",
                "rstim-compiled",
                "rstim-compiled",
            ]
        );
        assert_eq!(
            records
                .iter()
                .map(|record| (record.measurement_index, record.warmup))
                .collect::<Vec<_>>(),
            vec![
                (0, true),
                (1, false),
                (2, false),
                (0, true),
                (1, false),
                (2, false),
            ]
        );
        assert!(
            records
                .iter()
                .all(|record| record.case_label == "inline-sample")
        );
    }
}
