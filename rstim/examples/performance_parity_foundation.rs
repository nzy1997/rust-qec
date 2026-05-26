use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::error_analyzer::{AnalyzeBackend, AnalyzeOptions, ErrorAnalyzer};
use rstim::parser::parse_lines;
use rstim::perf::{
    PerfBenchmarkCase, PerfCircuitSource, PerfRecord, PerfVariant, PerfWorkload,
    benchmark_case_variants, benchmark_cases, effective_repeat_count,
};
use rstim::sampler::{SampleOptions, SamplingBackend, sample_batch_with_options};
use rstim::stats::summarize;

fn rstim_bin() -> PathBuf {
    let exe = std::env::current_exe().expect("current exe");
    let profile_dir = exe
        .parent()
        .and_then(|parent| parent.parent())
        .expect("example binary should live under target/<profile>/examples");
    let profile_name = profile_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("debug");
    let binary = profile_dir.join(format!("rstim{}", std::env::consts::EXE_SUFFIX));
    assert!(
        binary.exists(),
        "rstim binary not found at {}. Build it first with `cargo build -p rstim --{} --bin rstim`.",
        binary.display(),
        profile_name
    );
    binary
}

fn source_text(source: PerfCircuitSource) -> String {
    match source {
        PerfCircuitSource::Inline { text } => text.to_string(),
        PerfCircuitSource::Generator {
            code,
            task,
            distance,
            rounds,
            noise,
        } => {
            let output = Command::new(rstim_bin())
                .args([
                    "gen",
                    "--code",
                    code,
                    "--task",
                    task,
                    "--distance",
                    &distance.to_string(),
                    "--rounds",
                    &rounds.to_string(),
                    "--after_clifford_depolarization",
                    &noise.to_string(),
                ])
                .output()
                .expect("run rstim gen");
            assert!(
                output.status.success(),
                "rstim gen failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).expect("utf8 circuit")
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
            return Some(usage.ru_maxrss as u64);
        }
        #[cfg(not(target_os = "macos"))]
        {
            return Some((usage.ru_maxrss as u64).saturating_mul(1024));
        }
    }
    #[cfg(not(unix))]
    {
        None
    }
}

fn run_stim_cli(case: PerfBenchmarkCase, text: &str, shots: Option<usize>) -> u128 {
    let stim_cmd = std::env::var("RSTIM_TEST_STIM").unwrap_or_else(|_| "stim".to_string());
    let args = match case.workload {
        PerfWorkload::Sample => vec![
            "sample".to_string(),
            "--shots".to_string(),
            shots.unwrap_or(1).to_string(),
        ],
        PerfWorkload::Detect => vec![
            "detect".to_string(),
            "--shots".to_string(),
            shots.unwrap_or(1).to_string(),
        ],
        PerfWorkload::AnalyzeErrors => vec!["analyze_errors".to_string()],
    };

    let mut child = Command::new(stim_cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn stim");

    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(text.as_bytes())
        .expect("write circuit");

    let start = Instant::now();
    let output = child.wait_with_output().expect("wait");
    assert!(
        output.status.success(),
        "stim failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    start.elapsed().as_nanos()
}

fn run_rstim_variant(case: PerfBenchmarkCase, text: &str, variant: PerfVariant) -> u128 {
    let instrs = parse_lines(text).expect("parse benchmark circuit");
    let start = Instant::now();

    match case.workload {
        PerfWorkload::Sample | PerfWorkload::Detect => {
            let backend = match variant {
                PerfVariant::RstimInterpreted => SamplingBackend::Interpreted,
                PerfVariant::RstimCompiled => SamplingBackend::Compiled,
                _ => SamplingBackend::Auto,
            };
            let mut rng = StdRng::seed_from_u64(1234);
            let _ = sample_batch_with_options(
                &instrs,
                case.shots.unwrap_or(1),
                &mut rng,
                SampleOptions {
                    backend,
                    ..SampleOptions::default()
                },
            )
            .expect("sample benchmark");
        }
        PerfWorkload::AnalyzeErrors => {
            let backend = match variant {
                PerfVariant::RstimAnalyzerFlattened => AnalyzeBackend::Flattened,
                PerfVariant::RstimAnalyzerCompiled => AnalyzeBackend::Compiled,
                _ => AnalyzeBackend::Auto,
            };
            let _ = ErrorAnalyzer::circuit_to_dem_with_options(
                &instrs,
                AnalyzeOptions {
                    backend,
                    ..AnalyzeOptions::default()
                },
            )
            .expect("analyze benchmark");
        }
    }

    start.elapsed().as_nanos()
}

fn main() {
    for case in benchmark_cases() {
        let text = source_text(case.source);
        let instrs = parse_lines(&text).expect("parse benchmark circuit");
        let summary = summarize(&instrs);

        for variant in benchmark_case_variants(case, &instrs).expect("benchmark case variants") {
            let wall_time_ns = match variant {
                PerfVariant::StimCli => run_stim_cli(case, &text, case.shots),
                _ => run_rstim_variant(case, &text, variant),
            };

            let record = PerfRecord {
                case_label: case.label.to_string(),
                tool_variant: variant.label().to_string(),
                workload: case.workload.as_str().to_string(),
                qubits: summary.num_qubits,
                measurements: summary.num_measurements,
                detectors: summary.num_detectors,
                observables: summary.num_observables,
                repeat_depth: summary.max_repeat_depth,
                repeat_count: effective_repeat_count(&instrs),
                shots: case.shots,
                wall_time_ns,
                peak_memory_bytes: current_peak_memory_bytes(),
            };
            print!("{}", record.to_json_line());
        }
    }
}
