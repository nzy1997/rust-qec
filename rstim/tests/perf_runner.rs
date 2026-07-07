use rstim::codegen::{repetition_code_memory, rotated_memory_x};
use rstim::ir::circuit_to_string;
use rstim::perf::{benchmark_cases, run_case_measurements, PerfRunOptions, PerfVariant};
use serde_json::Value;
use std::fs;
use std::sync::{Mutex, MutexGuard, OnceLock};

const LOSS_PROTECTION_CIRCUIT: &str = "LOSS(1) 0\nMRL 0\nDETECTOR rec[-1]\n";

fn loss_protection_case() -> rstim::perf::PerfBenchmarkCase {
    benchmark_cases()
        .into_iter()
        .find(|case| case.label == "loss-protection-sample")
        .expect("loss protection case")
}

fn stim_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn lock_stim_env() -> MutexGuard<'static, ()> {
    stim_env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn runner_emits_one_warmup_and_five_measured_records_by_default() {
    let records = run_case_measurements(
        loss_protection_case(),
        LOSS_PROTECTION_CIRCUIT,
        &[PerfVariant::RstimInterpreted],
        PerfRunOptions::default(),
    )
    .expect("runner records");

    let warmup_count = records.iter().filter(|record| record.warmup).count();
    let measured_count = records.iter().filter(|record| !record.warmup).count();

    assert_eq!(records.len(), 6);
    assert_eq!(warmup_count, 1);
    assert_eq!(measured_count, 5);
    assert!(records[0].warmup);
    assert_eq!(records[0].measurement_index, 0);
    assert!(!records[1].warmup);
    assert_eq!(records[5].measurement_index, 5);
    assert!(records
        .iter()
        .all(|record| record.case_label == "loss-protection-sample"));
}

#[test]
fn runner_rejects_zero_measured_rounds() {
    let result = run_case_measurements(
        loss_protection_case(),
        LOSS_PROTECTION_CIRCUIT,
        &[PerfVariant::RstimInterpreted],
        PerfRunOptions {
            warmup_rounds: 1,
            measured_rounds: 0,
        },
    );

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("measured_rounds"));
}

#[test]
fn runner_records_generator_cases_and_suite_writer_emit_expected_labels() {
    let case = benchmark_cases()
        .into_iter()
        .find(|case| case.label == "rep-sample-d13-r13")
        .expect("generator sample case");
    let text = circuit_to_string(&repetition_code_memory(13, 13, 0.001));
    let records = run_case_measurements(
        case,
        &text,
        &[PerfVariant::RstimInterpreted],
        PerfRunOptions {
            warmup_rounds: 0,
            measured_rounds: 1,
        },
    )
    .expect("generator records");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].case_label, "rep-sample-d13-r13");
    assert_eq!(records[0].measurement_index, 0);
    assert!(!records[0].warmup);
    assert!(records[0].qubits > 0);
}

#[test]
fn runner_uses_stim_override_for_sample_detect_and_analyze_variants() {
    let _guard = lock_stim_env();
    let dir = tempfile::tempdir().unwrap();
    let fake_stim = dir.path().join("fake-stim");
    fs::write(
        &fake_stim,
        "#!/bin/sh\ncmd=\"$1\"\nshift\ncase \"$cmd\" in\nsample|detect|analyze_errors) cat >/dev/null ;;\n*) echo bad cmd >&2; exit 1 ;;\nesac\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&fake_stim).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake_stim, perms).unwrap();
    }

    unsafe {
        std::env::set_var("RSTIM_TEST_STIM", &fake_stim);
    }

    let sample_records = run_case_measurements(
        benchmark_cases()
            .into_iter()
            .find(|case| case.label == "loss-protection-sample")
            .expect("sample fallback case"),
        LOSS_PROTECTION_CIRCUIT,
        &[PerfVariant::StimCli],
        PerfRunOptions {
            warmup_rounds: 0,
            measured_rounds: 1,
        },
    )
    .expect("sample stim-cli records");
    assert_eq!(sample_records.len(), 1);

    let detect_case = benchmark_cases()
        .into_iter()
        .find(|case| case.label == "surface-detect-d13-r13")
        .expect("detect case");
    let detect_text = circuit_to_string(&rotated_memory_x(13, 13, 0.001));
    let detect_records = run_case_measurements(
        detect_case,
        &detect_text,
        &[PerfVariant::StimCli],
        PerfRunOptions {
            warmup_rounds: 0,
            measured_rounds: 1,
        },
    )
    .expect("detect stim-cli records");
    assert_eq!(detect_records.len(), 1);

    let analyze_case = benchmark_cases()
        .into_iter()
        .find(|case| case.label == "repeat-analyze-large")
        .expect("analyze case");
    let analyze_text = "REPEAT 4096 {\n    X_ERROR(0.001) 0\n    MR 0\n    DETECTOR rec[-1]\n}\n";
    let analyze_records = run_case_measurements(
        analyze_case,
        analyze_text,
        &[PerfVariant::StimCli],
        PerfRunOptions {
            warmup_rounds: 0,
            measured_rounds: 1,
        },
    )
    .expect("analyze stim-cli records");
    assert_eq!(analyze_records.len(), 1);

    unsafe {
        std::env::remove_var("RSTIM_TEST_STIM");
    }
}

#[test]
fn runner_propagates_stim_failure_output() {
    let _guard = lock_stim_env();
    let dir = tempfile::tempdir().unwrap();
    let fake_stim = dir.path().join("fake-stim-fail");
    fs::write(
        &fake_stim,
        "#!/bin/sh\ncat >/dev/null\necho 'stim exploded' >&2\nexit 1\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&fake_stim).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake_stim, perms).unwrap();
    }

    unsafe {
        std::env::set_var("RSTIM_TEST_STIM", &fake_stim);
    }
    let err = run_case_measurements(
        loss_protection_case(),
        LOSS_PROTECTION_CIRCUIT,
        &[PerfVariant::StimCli],
        PerfRunOptions {
            warmup_rounds: 0,
            measured_rounds: 1,
        },
    )
    .unwrap_err();
    unsafe {
        std::env::remove_var("RSTIM_TEST_STIM");
    }

    assert!(err.contains("stim failed: stim exploded"));
}

#[test]
fn selected_case_writer_records_failing_stim_cli_as_tool_failed_jsonl() {
    let _guard = lock_stim_env();
    let dir = tempfile::tempdir().unwrap();
    let fake_stim = dir.path().join("fake-stim-fail");
    fs::write(
        &fake_stim,
        "#!/bin/sh\ncat >/dev/null\necho 'stim exploded' >&2\nexit 1\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&fake_stim).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake_stim, perms).unwrap();
    }

    unsafe {
        std::env::set_var("RSTIM_TEST_STIM", &fake_stim);
    }

    let mut raw = Vec::new();
    rstim::perf::run_benchmark_case_to_writer(
        &mut raw,
        "loss-protection-sample",
        PerfRunOptions {
            warmup_rounds: 0,
            measured_rounds: 1,
        },
    )
    .expect("selected case writes raw records despite stim failure");

    unsafe {
        std::env::remove_var("RSTIM_TEST_STIM");
    }

    let text = String::from_utf8(raw).unwrap();
    let lines = text.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert!(lines
        .iter()
        .all(|line| line.contains("\"case_label\":\"loss-protection-sample\"")));

    let stim: Value = serde_json::from_str(
        lines
            .iter()
            .find(|line| line.contains("\"tool_variant\":\"stim-cli\""))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(stim["status"], "tool_failed");
    assert!(stim["failure_reason"]
        .as_str()
        .unwrap()
        .contains("stim failed: stim exploded"));
    assert_eq!(stim["stderr"], "stim exploded\n");

    let rstim: Value = serde_json::from_str(
        lines
            .iter()
            .find(|line| line.contains("\"tool_variant\":\"rstim-interpreted\""))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(rstim["status"], "completed");
}
