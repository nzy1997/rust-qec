use std::process::Command;

use rstim::perf::summarize_jsonl_str;

const PUBLIC_SELECTED_CASE_LABEL: &str = "stim-style-surface-sample-d11-r100-b1024";

fn rstim_cmd() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rstim"));
    cmd.current_dir(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root"),
    );
    cmd
}

fn write_fake_stim(path: &std::path::Path, body: &str) {
    std::fs::write(path, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }
}

const RAW_JSONL: &str = concat!(
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

const REGRESSION_RAW_JSONL: &str = concat!(
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

#[test]
fn perf_help_lists_pipeline_subcommands() {
    let output = rstim_cmd().args(["perf", "--help"]).output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for name in ["run", "summarize", "gate", "report", "ci"] {
        assert!(text.contains(name), "missing {name} from perf help");
    }
}

#[test]
fn perf_summarize_and_report_work_from_temp_files() {
    let dir = tempfile::tempdir().unwrap();
    let raw_path = dir.path().join("raw.jsonl");
    let summary_path = dir.path().join("summary.json");
    let report_path = dir.path().join("report.md");

    std::fs::write(&raw_path, RAW_JSONL).unwrap();

    let summarize = rstim_cmd()
        .args([
            "perf",
            "summarize",
            "--in",
            raw_path.to_str().unwrap(),
            "--out",
            summary_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        summarize.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&summarize.stderr)
    );

    let gate = rstim_cmd()
        .args(["perf", "gate", "--in", summary_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        gate.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&gate.stderr)
    );

    let report = rstim_cmd()
        .args([
            "perf",
            "report",
            "--in",
            summary_path.to_str().unwrap(),
            "--out",
            report_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&report.stderr)
    );

    let report_text = std::fs::read_to_string(report_path).unwrap();
    assert!(report_text.contains("## Gating Cases"));
    assert!(report_text.contains("rep-sample-d13-r13"));
}

#[test]
fn perf_gate_returns_nonzero_for_regression_summary() {
    let dir = tempfile::tempdir().unwrap();
    let summary_path = dir.path().join("summary.json");
    let summary = summarize_jsonl_str(REGRESSION_RAW_JSONL).unwrap();
    std::fs::write(&summary_path, serde_json::to_vec_pretty(&summary).unwrap()).unwrap();

    let output = rstim_cmd()
        .args(["perf", "gate", "--in", summary_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("RegressionFailure") || stderr.contains("exceeds threshold"));
}

#[test]
fn perf_ci_writes_artifacts_before_returning_gate_failure() {
    let dir = tempfile::tempdir().unwrap();
    let raw_override_path = dir.path().join("override.jsonl");
    let out_dir = dir.path().join("perf-artifacts");
    std::fs::write(&raw_override_path, REGRESSION_RAW_JSONL).unwrap();

    let output = rstim_cmd()
        .env(
            "RSTIM_TEST_PERF_CI_RAW",
            raw_override_path.to_str().unwrap(),
        )
        .env("RSTIM_TEST_STIM", "/definitely/not-used/stim")
        .args(["perf", "ci", "--out-dir", out_dir.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.contains("InfrastructureFailure"));
    assert!(stderr.contains("RegressionFailure") || stderr.contains("exceeds threshold"));

    let raw_text = std::fs::read_to_string(out_dir.join("raw.jsonl")).unwrap();
    let summary_text = std::fs::read_to_string(out_dir.join("summary.json")).unwrap();
    let report_text = std::fs::read_to_string(out_dir.join("report.md")).unwrap();

    assert_eq!(raw_text, REGRESSION_RAW_JSONL);
    assert!(summary_text.contains("\"cases\""));
    assert!(report_text.contains("## Gate Verdict"));
    assert!(report_text.contains("RegressionFailure") || report_text.contains("exceeds threshold"));
}

#[test]
fn perf_ci_returns_infrastructure_failure_when_override_raw_path_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = dir.path().join("perf-artifacts");
    let missing_raw = dir.path().join("missing.jsonl");

    let output = rstim_cmd()
        .env("RSTIM_TEST_PERF_CI_RAW", missing_raw.to_str().unwrap())
        .args(["perf", "ci", "--out-dir", out_dir.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("InfrastructureFailure"));
    assert!(stderr.contains("failed to copy test perf raw artifact"));
}

#[test]
fn perf_run_unknown_case_fails_before_creating_output() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("unused.jsonl");

    let output = rstim_cmd()
        .args([
            "perf",
            "run",
            "--case",
            "no-such-case",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown benchmark case"));
    assert!(!out.exists());
}

#[test]
fn perf_run_case_records_tool_failure_without_aborting() {
    let dir = tempfile::tempdir().unwrap();
    let fake_stim = dir.path().join("fake-stim-fail");
    let out = dir.path().join("raw.jsonl");
    write_fake_stim(
        &fake_stim,
        "#!/bin/sh\ncat >/dev/null\necho 'stim exploded' >&2\nexit 1\n",
    );

    let output = rstim_cmd()
        .env("RSTIM_TEST_STIM", &fake_stim)
        .args([
            "perf",
            "run",
            "--case",
            "loss-protection-sample",
            "--warmup-rounds",
            "0",
            "--measure-rounds",
            "1",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let raw = std::fs::read_to_string(out).unwrap();
    assert!(raw.contains("\"tool_variant\":\"stim-cli\""));
    assert!(raw.contains("\"status\":\"tool_failed\""));
    assert!(raw.contains("stim failed: stim exploded"));
    assert!(raw.contains("\"tool_variant\":\"rstim-interpreted\""));
    assert!(raw.contains("\"status\":\"completed\""));
    assert!(
        raw.lines()
            .all(|line| line.contains("\"case_label\":\"loss-protection-sample\""))
    );
}

#[test]
fn perf_ci_case_writes_only_selected_artifacts_without_gate_failure() {
    let dir = tempfile::tempdir().unwrap();
    let fake_stim = dir.path().join("fake-stim-fail");
    let out_dir = dir.path().join("focused-ci");
    write_fake_stim(
        &fake_stim,
        "#!/bin/sh\ncat >/dev/null\necho 'stim exploded' >&2\nexit 1\n",
    );

    let output = rstim_cmd()
        .env("RSTIM_TEST_STIM", &fake_stim)
        .args([
            "perf",
            "ci",
            "--case",
            "loss-protection-sample",
            "--warmup-rounds",
            "0",
            "--measure-rounds",
            "1",
            "--out-dir",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let raw = std::fs::read_to_string(out_dir.join("raw.jsonl")).unwrap();
    let summary = std::fs::read_to_string(out_dir.join("summary.json")).unwrap();
    let report = std::fs::read_to_string(out_dir.join("report.md")).unwrap();

    assert!(
        raw.lines()
            .all(|line| line.contains("\"case_label\":\"loss-protection-sample\""))
    );
    assert!(summary.contains("\"case_label\": \"loss-protection-sample\""));
    assert!(!summary.contains("rep-sample-d13-r13"));
    assert!(report.contains("loss-protection-sample"));
    assert!(!report.contains("rep-sample-d13-r13"));
    assert!(report.contains("tool_failed"));
}

#[test]
fn perf_run_case_with_public_label_writes_only_selected_completed_records() {
    let dir = tempfile::tempdir().unwrap();
    let fake_stim = dir.path().join("fake-stim-success");
    let out = dir.path().join("raw.jsonl");
    write_fake_stim(&fake_stim, "#!/bin/sh\ncat >/dev/null\nexit 0\n");

    let output = rstim_cmd()
        .env("RSTIM_TEST_STIM", &fake_stim)
        .args([
            "perf",
            "run",
            "--case",
            PUBLIC_SELECTED_CASE_LABEL,
            "--warmup-rounds",
            "0",
            "--measure-rounds",
            "1",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let raw = std::fs::read_to_string(out).unwrap();
    assert!(!raw.is_empty());
    assert!(raw.contains("\"tool_variant\":\"stim-cli\""));
    assert!(raw.contains("\"tool_variant\":\"rstim-"));
    assert!(raw.contains("\"status\":\"completed\""));
    assert!(!raw.contains("loss-protection-sample"));
    assert!(
        raw.lines()
            .all(|line| line.contains(&format!("\"case_label\":\"{PUBLIC_SELECTED_CASE_LABEL}\"")))
    );
}

#[test]
fn perf_ci_case_with_public_label_writes_only_selected_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let fake_stim = dir.path().join("fake-stim-success");
    let out_dir = dir.path().join("focused-ci-public-case");
    write_fake_stim(&fake_stim, "#!/bin/sh\ncat >/dev/null\nexit 0\n");

    let output = rstim_cmd()
        .env("RSTIM_TEST_STIM", &fake_stim)
        .args([
            "perf",
            "ci",
            "--case",
            PUBLIC_SELECTED_CASE_LABEL,
            "--warmup-rounds",
            "0",
            "--measure-rounds",
            "1",
            "--out-dir",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let raw = std::fs::read_to_string(out_dir.join("raw.jsonl")).unwrap();
    let summary = std::fs::read_to_string(out_dir.join("summary.json")).unwrap();
    let report = std::fs::read_to_string(out_dir.join("report.md")).unwrap();

    assert!(!raw.is_empty());
    assert!(
        raw.lines()
            .all(|line| line.contains(&format!("\"case_label\":\"{PUBLIC_SELECTED_CASE_LABEL}\"")))
    );
    assert!(summary.contains(&format!("\"case_label\": \"{PUBLIC_SELECTED_CASE_LABEL}\"")));
    assert!(!summary.contains("rep-sample-d13-r13"));
    assert!(!summary.contains("loss-protection-sample"));
    assert!(report.contains(PUBLIC_SELECTED_CASE_LABEL));
    assert!(!report.contains("rep-sample-d13-r13"));
    assert!(!report.contains("loss-protection-sample"));
}

#[test]
fn perf_ci_case_filters_override_raw_to_selected_case() {
    let dir = tempfile::tempdir().unwrap();
    let raw_override_path = dir.path().join("override.jsonl");
    let out_dir = dir.path().join("filtered-override");
    let selected_line = format!(
        "{{\"case_label\":\"{PUBLIC_SELECTED_CASE_LABEL}\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"report_only\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":100,\"shots\":1024,\"wall_time_ns\":100,\"peak_memory_bytes\":null,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}}\n"
    );
    let mixed_raw = format!(
        "{}{}",
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":130,\"peak_memory_bytes\":1024,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n",
        selected_line
    );
    std::fs::write(&raw_override_path, mixed_raw).unwrap();

    let output = rstim_cmd()
        .env(
            "RSTIM_TEST_PERF_CI_RAW",
            raw_override_path.to_str().unwrap(),
        )
        .args([
            "perf",
            "ci",
            "--case",
            PUBLIC_SELECTED_CASE_LABEL,
            "--out-dir",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let raw = std::fs::read_to_string(out_dir.join("raw.jsonl")).unwrap();
    let summary = std::fs::read_to_string(out_dir.join("summary.json")).unwrap();
    let report = std::fs::read_to_string(out_dir.join("report.md")).unwrap();

    assert!(raw.contains(PUBLIC_SELECTED_CASE_LABEL));
    assert!(!raw.contains("rep-sample-d13-r13"));
    assert!(
        raw.lines()
            .all(|line| line.contains(&format!("\"case_label\":\"{PUBLIC_SELECTED_CASE_LABEL}\"")))
    );
    assert!(summary.contains(&format!("\"case_label\": \"{PUBLIC_SELECTED_CASE_LABEL}\"")));
    assert!(!summary.contains("rep-sample-d13-r13"));
    assert!(report.contains(PUBLIC_SELECTED_CASE_LABEL));
    assert!(!report.contains("rep-sample-d13-r13"));
}

#[test]
fn perf_summarize_and_report_public_fixture_show_rates_and_report_only_stim_context() {
    let dir = tempfile::tempdir().unwrap();
    let summary_path = dir.path().join("summary.json");
    let report_path = dir.path().join("report.md");
    let raw_path = std::path::Path::new("rstim/tests/fixtures/perf/stim_style_sample_raw.jsonl");

    let summarize = rstim_cmd()
        .args([
            "perf",
            "summarize",
            "--in",
            raw_path.to_str().unwrap(),
            "--out",
            summary_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        summarize.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&summarize.stderr)
    );

    let summary = std::fs::read_to_string(&summary_path).unwrap();
    assert!(summary.contains("\"median_shots_per_second\""));
    assert!(summary.contains("\"rstim_compiled_vs_stim_cli_ratio\""));

    let report = rstim_cmd()
        .args([
            "perf",
            "report",
            "--in",
            summary_path.to_str().unwrap(),
            "--out",
            report_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&report.stderr)
    );

    let report_text = std::fs::read_to_string(report_path).unwrap();
    assert!(report_text.contains(PUBLIC_SELECTED_CASE_LABEL));
    assert!(report_text.contains("shots/s"));
    assert!(report_text.contains("report-only Stim comparison"));
}
