use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use rstim::dem::DetectorErrorModel;
use rstim::parser::parse_lines;
use rstim::showcase::{
    dem_semantic_summary, median_duration_ns, render_markdown_table, showcase_cases,
    strip_comment_preamble, structural_circuit_summary,
};

fn rstim_bin() -> PathBuf {
    let exe = std::env::current_exe().expect("current exe");
    let debug_dir = exe
        .parent()
        .and_then(|parent| parent.parent())
        .expect("example binary should live under target/<profile>/examples");
    let binary = debug_dir.join(format!("rstim{}", std::env::consts::EXE_SUFFIX));
    assert!(
        binary.exists(),
        "rstim binary not found at {}",
        binary.display()
    );
    binary
}

fn timed_capture(cmd: &str, args: &[String], stdin_data: Option<&str>) -> (String, Duration) {
    let start = Instant::now();
    let output = if let Some(stdin_data) = stdin_data {
        let mut child = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin_data.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    } else {
        Command::new(cmd).args(args).output().unwrap()
    };
    assert!(
        output.status.success(),
        "command failed: {cmd} {args:?}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (String::from_utf8(output.stdout).unwrap(), start.elapsed())
}

fn format_ms(nanos: u128) -> String {
    format!("{:.3}", nanos as f64 / 1_000_000.0)
}

fn format_ratio(numerator: u128, denominator: u128) -> String {
    if denominator == 0 {
        "inf".to_string()
    } else {
        format!("{:.2}x", numerator as f64 / denominator as f64)
    }
}

fn main() {
    let stim_cmd = std::env::var("RSTIM_TEST_STIM").unwrap_or_else(|_| "stim".to_string());
    let rstim_cmd = rstim_bin().to_string_lossy().into_owned();
    let mut rows = Vec::new();

    for case in showcase_cases() {
        let gen_args = vec![
            "gen".to_string(),
            "--code".to_string(),
            case.code.to_string(),
            "--task".to_string(),
            case.task.to_string(),
            "--distance".to_string(),
            case.distance.to_string(),
            "--rounds".to_string(),
            case.rounds.to_string(),
        ];
        let noisy_gen_args = vec![
            "gen".to_string(),
            "--code".to_string(),
            case.code.to_string(),
            "--task".to_string(),
            case.task.to_string(),
            "--distance".to_string(),
            case.distance.to_string(),
            "--rounds".to_string(),
            case.rounds.to_string(),
            "--after_clifford_depolarization".to_string(),
            "0.001".to_string(),
        ];
        let analyze_args = vec!["analyze_errors".to_string()];

        // Warmup each path once to reduce one-time noise.
        let (_, _) = timed_capture(&stim_cmd, &gen_args, None);
        let (_, _) = timed_capture(&rstim_cmd, &gen_args, None);
        let (warm_noisy_circuit, _) = timed_capture(&stim_cmd, &noisy_gen_args, None);
        let (_, _) = timed_capture(&stim_cmd, &analyze_args, Some(&warm_noisy_circuit));
        let (_, _) = timed_capture(&rstim_cmd, &analyze_args, Some(&warm_noisy_circuit));

        let mut stim_gen_times = Vec::new();
        let mut rstim_gen_times = Vec::new();
        let mut stim_dem_times = Vec::new();
        let mut rstim_dem_times = Vec::new();

        let mut last_stim_text = String::new();
        let mut last_rstim_text = String::new();
        let mut last_stim_dem = String::new();
        let mut last_rstim_dem = String::new();

        for _ in 0..5 {
            let (stim_text, stim_gen_time) = timed_capture(&stim_cmd, &gen_args, None);
            let (rstim_text, rstim_gen_time) = timed_capture(&rstim_cmd, &gen_args, None);
            let (noisy_circuit, _) = timed_capture(&stim_cmd, &noisy_gen_args, None);
            let (stim_dem, stim_dem_time) =
                timed_capture(&stim_cmd, &analyze_args, Some(&noisy_circuit));
            let (rstim_dem, rstim_dem_time) =
                timed_capture(&rstim_cmd, &analyze_args, Some(&noisy_circuit));

            stim_gen_times.push(stim_gen_time);
            rstim_gen_times.push(rstim_gen_time);
            stim_dem_times.push(stim_dem_time);
            rstim_dem_times.push(rstim_dem_time);
            last_stim_text = stim_text;
            last_rstim_text = rstim_text;
            last_stim_dem = stim_dem;
            last_rstim_dem = rstim_dem;
        }

        let gen_status = if strip_comment_preamble(&last_stim_text) == last_rstim_text {
            "exact".to_string()
        } else {
            let stim_instrs = parse_lines(strip_comment_preamble(&last_stim_text)).unwrap();
            let rstim_instrs = parse_lines(&last_rstim_text).unwrap();
            if structural_circuit_summary(&stim_instrs) == structural_circuit_summary(&rstim_instrs)
            {
                "normalized".to_string()
            } else {
                "mismatch".to_string()
            }
        };

        let stim_summary =
            dem_semantic_summary(&DetectorErrorModel::parse(&last_stim_dem).unwrap());
        let rstim_summary =
            dem_semantic_summary(&DetectorErrorModel::parse(&last_rstim_dem).unwrap());

        let mut max_rel = 0.0f64;
        let mut dem_status = "match".to_string();
        if stim_summary.annotation_lines != rstim_summary.annotation_lines
            || stim_summary.error_probabilities.len() != rstim_summary.error_probabilities.len()
        {
            dem_status = "mismatch".to_string();
        } else {
            for (targets, stim_p) in &stim_summary.error_probabilities {
                let Some(rstim_p) = rstim_summary.error_probabilities.get(targets) else {
                    dem_status = "mismatch".to_string();
                    max_rel = f64::INFINITY;
                    break;
                };
                let rel = (stim_p - rstim_p).abs() / stim_p.max(1e-20);
                max_rel = max_rel.max(rel);
                if rel > 1e-12 {
                    dem_status = "mismatch".to_string();
                }
            }
        }

        let stim_gen_ns = median_duration_ns(&stim_gen_times);
        let rstim_gen_ns = median_duration_ns(&rstim_gen_times);
        let stim_dem_ns = median_duration_ns(&stim_dem_times);
        let rstim_dem_ns = median_duration_ns(&rstim_dem_times);

        rows.push(vec![
            case.label(),
            gen_status,
            dem_status,
            format!("{max_rel:.3e}"),
            format_ms(stim_gen_ns),
            format_ms(rstim_gen_ns),
            format_ms(stim_dem_ns),
            format_ms(rstim_dem_ns),
            format_ratio(rstim_gen_ns, stim_gen_ns),
            format_ratio(rstim_dem_ns, stim_dem_ns),
        ]);
    }

    print!("{}", render_markdown_table(&rows));
}
