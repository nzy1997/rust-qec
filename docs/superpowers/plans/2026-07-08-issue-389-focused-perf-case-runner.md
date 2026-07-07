# Issue 389 Focused Perf Case Runner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add focused `--case <label>` perf runs and selected-case failure records for reviewer-readable Stim-style simulator evidence.

**Architecture:** Keep full-suite perf behavior on the existing abort-on-failure path. Add a focused runner path that validates one case label, reuses the existing source and variant discovery logic, and writes explicit per-variant status records when a selected variant fails. Extend summary/report code to understand status-bearing raw records and to summarize only the selected case when `perf ci --case` is used.

**Tech Stack:** Rust 2024, `clap` CLI derives, `serde` JSON/JSONL records, existing `rstim::perf` runner/summary/report modules, integration tests under `rstim/tests/`.

## Global Constraints

- Add `--case <label>` support to `rstim perf run` and `rstim perf ci`.
- The public selected case label is exactly `stim-style-surface-sample-d11-r100-b1024`.
- Keep the default no-`--case` behavior unchanged and conservative.
- Validate unknown case labels before running any benchmarks.
- Unknown selected labels must exit nonzero and print text containing `unknown benchmark case`.
- Preserve warmup and measured-round behavior for selected cases.
- A selected failing tool variant must emit a raw JSONL record with `status: "tool_failed"` and a reviewer-readable failure reason.
- Raw status vocabulary includes `completed`, `tool_failed`, `timed_out`, and `missing_variant`.
- Completed records include `status: "completed"`.
- Selected-case CI writes `raw.jsonl`, `summary.json`, and `report.md` containing only the selected case.
- Do not optimize the selected benchmark.
- Do not make Stim-vs-`rstim` speed a hard gate.

---

## File Structure

- Modify `rstim/src/perf/record.rs`: add status/failure fields to raw records with backward-compatible deserialization.
- Modify `rstim/src/perf/runner.rs`: add focused case lookup, selected-case writer, and failure-as-record handling.
- Modify `rstim/src/perf/summary.rs`: add selected-case summary options and summarize failed variants without aborting.
- Modify `rstim/src/perf/report.rs`: render variant status and failure context.
- Modify `rstim/src/perf.rs`: re-export new status/options/focused runner APIs.
- Modify `rstim/src/cli.rs`: add `--case` to `perf run` and `perf ci`, validate before opening outputs, and make selected CI skip full-suite gating.
- Modify `rstim/tests/perf_harness.rs`: assert status serialization and compatibility.
- Modify `rstim/tests/perf_runner.rs`: test selected-case failing `stim-cli` raw records.
- Modify `rstim/tests/perf_summary.rs`: test selected-case summary/report behavior for failed records.
- Modify `rstim/tests/cli_perf.rs`: test CLI unknown-case and focused CI artifacts.

### Task 1: Add Raw Record Status Semantics

**Files:**
- Modify: `rstim/src/perf/record.rs`
- Modify: `rstim/src/perf/summary.rs`
- Modify: `rstim/src/perf/report.rs`
- Modify: `rstim/src/perf.rs`
- Modify: `rstim/tests/perf_harness.rs`
- Modify: `rstim/tests/perf_summary.rs`

**Interfaces:**
- Produces: `PerfRecordStatus::{Completed, ToolFailed, TimedOut, MissingVariant}` serialized as snake_case JSON strings.
- Produces: `PerfMeasurementRecord.status: PerfRecordStatus`.
- Produces: `PerfMeasurementRecord.failure_reason: Option<String>`.
- Produces: `PerfMeasurementRecord.stderr: Option<String>`.
- Produces: `PerfSummaryOptions { case_label: Option<String> }`.
- Produces: `summarize_jsonl_str_with_options(raw: &str, options: PerfSummaryOptions) -> Result<PerfSummary, String>`.

- [ ] **Step 1: Write failing raw status tests**

Add `PerfRecordStatus` to the `rstim/tests/perf_harness.rs` perf import and add this test:

```rust
#[test]
fn perf_measurement_record_json_line_contains_status_and_failure_context() {
    let record = PerfMeasurementRecord {
        case_label: "loss-protection-sample".to_string(),
        tool_variant: PerfVariant::StimCli.label().to_string(),
        workload: PerfWorkload::Sample.as_str().to_string(),
        tier: PerfCaseTier::Gating.as_str().to_string(),
        measurement_index: 0,
        warmup: false,
        qubits: 1,
        measurements: 1,
        detectors: 1,
        observables: 0,
        repeat_depth: 1,
        repeat_count: 0,
        shots: Some(128),
        wall_time_ns: 0,
        peak_memory_bytes: None,
        status: PerfRecordStatus::ToolFailed,
        failure_reason: Some("stim failed: boom".to_string()),
        stderr: Some("boom\n".to_string()),
    };

    let line = record.to_json_line();
    let json: serde_json::Value = serde_json::from_str(line.trim_end()).unwrap();
    let parsed = PerfMeasurementRecord::from_json_line(line.trim_end()).unwrap();

    assert_eq!(json["status"], "tool_failed");
    assert_eq!(json["failure_reason"], "stim failed: boom");
    assert_eq!(json["stderr"], "boom\n");
    assert_eq!(parsed, record);
}

#[test]
fn perf_measurement_record_deserializes_legacy_rows_as_completed() {
    let parsed = PerfMeasurementRecord::from_json_line(
        "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":80,\"peak_memory_bytes\":128}"
    )
    .unwrap();

    assert_eq!(parsed.status, PerfRecordStatus::Completed);
    assert_eq!(parsed.failure_reason, None);
    assert_eq!(parsed.stderr, None);
}
```

Add this test to `rstim/tests/perf_summary.rs`:

```rust
#[test]
fn selected_summary_keeps_failed_variant_and_omits_unrelated_missing_cases() {
    let raw = concat!(
        "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":0,\"peak_memory_bytes\":null,\"status\":\"tool_failed\",\"failure_reason\":\"stim failed: boom\",\"stderr\":\"boom\\n\"}\n",
        "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":70,\"peak_memory_bytes\":256,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n"
    );

    let summary = rstim::perf::summarize_jsonl_str_with_options(
        raw,
        rstim::perf::PerfSummaryOptions {
            case_label: Some("loss-protection-sample".to_string()),
        },
    )
    .unwrap();

    assert_eq!(summary.cases.len(), 1);
    assert_eq!(summary.cases[0].case_label, "loss-protection-sample");
    assert!(!summary
        .issues
        .iter()
        .any(|issue| issue.message.contains("missing benchmark case data")));

    let stim = summary.cases[0]
        .variants
        .iter()
        .find(|variant| variant.tool_variant == "stim-cli")
        .unwrap();
    assert_eq!(stim.status, "tool_failed");
    assert_eq!(stim.sample_count, 0);
    assert_eq!(stim.failure_reason.as_deref(), Some("stim failed: boom"));

    let report = render_markdown_report(&summary, None);
    assert!(report.contains("stim-cli status: `tool_failed`"));
    assert!(report.contains("stim failed: boom"));
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```sh
cargo test -p rstim --test perf_harness perf_measurement_record_json_line_contains_status_and_failure_context
cargo test -p rstim --test perf_harness perf_measurement_record_deserializes_legacy_rows_as_completed
cargo test -p rstim --test perf_summary selected_summary_keeps_failed_variant_and_omits_unrelated_missing_cases
```

Expected: each command fails to compile because the new status/options fields and exports do not exist.

- [ ] **Step 3: Implement raw status fields**

In `rstim/src/perf/record.rs`, add this enum above `PerfMeasurementRecord`:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PerfRecordStatus {
    Completed,
    ToolFailed,
    TimedOut,
    MissingVariant,
}

impl PerfRecordStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PerfRecordStatus::Completed => "completed",
            PerfRecordStatus::ToolFailed => "tool_failed",
            PerfRecordStatus::TimedOut => "timed_out",
            PerfRecordStatus::MissingVariant => "missing_variant",
        }
    }
}

fn default_perf_record_status() -> PerfRecordStatus {
    PerfRecordStatus::Completed
}
```

Add these fields at the end of `PerfMeasurementRecord`:

```rust
    #[serde(default = "default_perf_record_status")]
    pub status: PerfRecordStatus,
    #[serde(default)]
    pub failure_reason: Option<String>,
    #[serde(default)]
    pub stderr: Option<String>,
```

Update every `PerfMeasurementRecord { ... }` literal in tests and runner code with:

```rust
        status: PerfRecordStatus::Completed,
        failure_reason: None,
        stderr: None,
```

- [ ] **Step 4: Implement selected summary and failed variant summaries**

In `rstim/src/perf/summary.rs`, add `PerfRecordStatus` to the import list. Add these public structs/fields:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PerfSummaryOptions {
    pub case_label: Option<String>,
}
```

Extend `PerfVariantSummary`:

```rust
    pub status: String,
    pub failure_reason: Option<String>,
    pub stderr: Option<String>,
```

Keep `summarize_jsonl_str(raw)` as:

```rust
pub fn summarize_jsonl_str(raw: &str) -> Result<PerfSummary, String> {
    summarize_jsonl_str_with_options(raw, PerfSummaryOptions::default())
}
```

Add `summarize_jsonl_str_with_options` by moving the current summary body into that function. Build the case list from all benchmark cases when `options.case_label` is `None`, or from the single matching case when it is `Some(label)`. Return `Err(format!("unknown benchmark case: {label}"))` when the selected label is absent.

When summarizing variant records, compute medians from measured records whose `status == PerfRecordStatus::Completed`. If no completed measured records exist but a measured failed record exists, create:

```rust
PerfVariantSummary {
    tool_variant: tool_variant.clone(),
    sample_count: 0,
    median_wall_time_ns: 0,
    median_peak_memory_bytes: None,
    status: failed.status.as_str().to_string(),
    failure_reason: failed.failure_reason.clone(),
    stderr: failed.stderr.clone(),
}
```

For completed variants, set `status` to `"completed"` and failure fields to `None`. For comparisons, require both summaries to have `status == "completed"` and `sample_count > 0`; otherwise push the existing `MissingComparisonVariants` issue kind with text that names the unavailable variant.

- [ ] **Step 5: Render status in reports and export APIs**

In `rstim/src/perf/report.rs`, inside `render_case_section`, after expected and present variants, add:

```rust
    for variant in &case.variants {
        if variant.status != "completed" {
            out.push_str(&format!(
                "- {} status: `{}`",
                variant.tool_variant, variant.status
            ));
            if let Some(reason) = &variant.failure_reason {
                out.push_str(&format!("; reason: `{}`", reason));
            }
            out.push('\n');
        }
    }
```

Leave the existing median line loop in place; it will print `0` measured rounds for failed variants, which is explicit and parseable.

In `rstim/src/perf.rs`, re-export:

```rust
pub use record::{PerfMeasurementRecord, PerfRecord, PerfRecordStatus};
pub use summary::{
    PerfCaseSummary, PerfComparisonSummary, PerfSummary, PerfSummaryIssue, PerfSummaryIssueKind,
    PerfSummaryOptions, PerfVariantSummary, summarize_jsonl_str, summarize_jsonl_str_with_options,
};
```

- [ ] **Step 6: Run tests and verify GREEN**

Run the three commands from Step 2 again.

Expected: all three pass.

### Task 2: Add Focused Runner And Failure-As-Record Path

**Files:**
- Modify: `rstim/src/perf/runner.rs`
- Modify: `rstim/src/perf.rs`
- Modify: `rstim/tests/perf_runner.rs`

**Interfaces:**
- Produces: `run_benchmark_case_to_writer(out: &mut dyn Write, case_label: &str, options: PerfRunOptions) -> Result<(), String>`.
- Produces: `benchmark_case_by_label(case_label: &str) -> Result<PerfBenchmarkCase, String>`.
- Full-suite `run_benchmark_suite_to_writer` keeps abort-on-failure behavior.

- [ ] **Step 1: Write failing selected runner test**

Add `serde_json::Value` to `rstim/tests/perf_runner.rs` imports and add this test:

```rust
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
```

- [ ] **Step 2: Run test and verify RED**

Run:

```sh
cargo test -p rstim --test perf_runner selected_case_writer_records_failing_stim_cli_as_tool_failed_jsonl
```

Expected: fails to compile because `run_benchmark_case_to_writer` does not exist.

- [ ] **Step 3: Implement focused runner helpers**

In `rstim/src/perf/runner.rs`, add `PerfRecordStatus` to the `use super::{ ... }` list. Add this failure struct near `PerfRunOptions`:

```rust
#[derive(Debug, Clone)]
struct PerfVariantFailure {
    status: PerfRecordStatus,
    failure_reason: String,
    stderr: Option<String>,
}

impl PerfVariantFailure {
    fn tool_failed(reason: impl Into<String>, stderr: Option<String>) -> Self {
        Self {
            status: PerfRecordStatus::ToolFailed,
            failure_reason: reason.into(),
            stderr,
        }
    }
}
```

Change `run_variant` and `run_stim_cli` to return `Result<u128, PerfVariantFailure>`. For existing `String` errors from parser/sampler/analyzer paths, map them with `PerfVariantFailure::tool_failed(error, None)`. For existing callers that should abort, convert the failure back to `failure.failure_reason`.

In `run_stim_cli`, preserve the legacy error text for nonzero exits:

```rust
let stderr = String::from_utf8_lossy(&output.stderr).to_string();
if !output.status.success() {
    return Err(PerfVariantFailure::tool_failed(
        format!("stim failed: {stderr}"),
        Some(stderr),
    ));
}
```

Add `benchmark_case_by_label`:

```rust
pub fn benchmark_case_by_label(case_label: &str) -> Result<PerfBenchmarkCase, String> {
    benchmark_cases()
        .into_iter()
        .find(|case| case.label == case_label)
        .ok_or_else(|| format!("unknown benchmark case: {case_label}"))
}
```

Add `run_selected_case_measurements` by copying the structure of `run_case_measurements`, but on per-round `run_variant` errors, push a `PerfMeasurementRecord` with `status`, `failure_reason`, `stderr`, `wall_time_ns: 0`, and otherwise identical metadata. Add `run_benchmark_case_to_writer`:

```rust
pub fn run_benchmark_case_to_writer(
    out: &mut dyn Write,
    case_label: &str,
    options: PerfRunOptions,
) -> Result<(), String> {
    let case = benchmark_case_by_label(case_label)?;
    let text = source_text(case.source)?;
    let instrs = parse_lines(&text)?;
    let variants = benchmark_case_variants(case, &instrs)?;
    let records = run_selected_case_measurements(case, &text, &variants, options)?;
    for record in records {
        out.write_all(record.to_json_line().as_bytes())
            .map_err(|e| format!("failed to write perf record: {e}"))?;
    }
    Ok(())
}
```

- [ ] **Step 4: Export focused runner APIs**

In `rstim/src/perf.rs`, export:

```rust
pub use runner::{
    PerfRunOptions, benchmark_case_by_label, run_benchmark_case_to_writer,
    run_benchmark_suite_to_writer, run_case_measurements,
};
```

- [ ] **Step 5: Run test and verify GREEN**

Run:

```sh
cargo test -p rstim --test perf_runner selected_case_writer_records_failing_stim_cli_as_tool_failed_jsonl
```

Expected: pass.

### Task 3: Add CLI `--case` For Run And CI

**Files:**
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/tests/cli_perf.rs`

**Interfaces:**
- `rstim perf run --case <label> --out <path>` writes only selected-case raw JSONL.
- `rstim perf ci --case <label> --out-dir <dir>` writes selected-case `raw.jsonl`, `summary.json`, and `report.md` without running the full-suite gate.

- [ ] **Step 1: Write failing CLI tests**

Add this helper to `rstim/tests/cli_perf.rs`:

```rust
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
```

Add these tests:

```rust
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
    assert!(raw
        .lines()
        .all(|line| line.contains("\"case_label\":\"loss-protection-sample\"")));
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

    assert!(raw
        .lines()
        .all(|line| line.contains("\"case_label\":\"loss-protection-sample\"")));
    assert!(summary.contains("\"case_label\": \"loss-protection-sample\""));
    assert!(!summary.contains("rep-sample-d13-r13"));
    assert!(report.contains("loss-protection-sample"));
    assert!(!report.contains("rep-sample-d13-r13"));
    assert!(report.contains("tool_failed"));
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```sh
cargo test -p rstim --test cli_perf perf_run_unknown_case_fails_before_creating_output
cargo test -p rstim --test cli_perf perf_run_case_records_tool_failure_without_aborting
cargo test -p rstim --test cli_perf perf_ci_case_writes_only_selected_artifacts_without_gate_failure
```

Expected: fail because `--case` is not recognized.

- [ ] **Step 3: Implement CLI flags and selected run routing**

In `rstim/src/cli.rs`, add `case: Option<String>` to `PerfCommands::Run` and `PerfCommands::Ci`:

```rust
        #[arg(long = "case")]
        case: Option<String>,
```

In the `PerfCommands::Run` match arm, destructure `case`. If `case` is `Some`, call `crate::perf::benchmark_case_by_label(label)?` before `open_output`, then open the output and call `run_benchmark_case_to_writer`. If `case` is `None`, keep the existing `run_benchmark_suite_to_writer` path.

In `PerfCommands::Ci`, destructure `case` and pass `case.as_deref()` to `run_perf_ci`.

- [ ] **Step 4: Implement selected CI artifacts**

Change `run_perf_ci` to:

```rust
fn run_perf_ci(
    out_dir: &str,
    warmup_rounds: usize,
    measure_rounds: usize,
    case_label: Option<&str>,
) -> Result<(), PerfCiError>
```

Validate `case_label` with `crate::perf::benchmark_case_by_label(label)` before `create_dir_all`. Change `write_perf_ci_raw_artifact` to accept `case_label: Option<&str>` and call `run_benchmark_case_to_writer` when it is `Some`. Preserve the `RSTIM_TEST_PERF_CI_RAW` override copy behavior.

Change `finalize_perf_ci_artifacts` to accept `case_label: Option<&str>`. When `case_label` is `Some(label)`, call:

```rust
let summary = crate::perf::summarize_jsonl_str_with_options(
    raw_text,
    crate::perf::PerfSummaryOptions {
        case_label: Some(label.to_string()),
    },
)
.map_err(PerfCiError::Infrastructure)?;
let report = crate::perf::render_markdown_report(&summary, None);
```

Write `summary.json` and `report.md`, then return `Ok(())` without evaluating the full-suite gate. When `case_label` is `None`, keep the existing summary, gate, report, and gate-failure behavior.

- [ ] **Step 5: Run tests and verify GREEN**

Run the three commands from Step 2 again.

Expected: all three pass.

### Task 4: Final Verification And PR Preparation

**Files:**
- Modify as needed based on test failures in previous tasks.

**Interfaces:**
- Produces a branch ready for PR with docs, tests, and implementation committed.

- [ ] **Step 1: Run focused perf tests**

Run:

```sh
cargo test -p rstim --test perf_harness
cargo test -p rstim --test perf_runner
cargo test -p rstim --test perf_summary
cargo test -p rstim --test cli_perf
```

Expected: all pass.

- [ ] **Step 2: Run issue verification command for `perf run --case`**

Run:

```sh
cargo run -p rstim --bin rstim -- perf run \
  --case stim-style-surface-sample-d11-r100-b1024 \
  --warmup-rounds 0 \
  --measure-rounds 1 \
  --out /tmp/rstim-vs-stim-speed.jsonl
```

Expected: exits zero. `/tmp/rstim-vs-stim-speed.jsonl` contains only `stim-style-surface-sample-d11-r100-b1024`, includes `stim-cli`, includes at least one `rstim` variant, and completed records include `"status":"completed"`.

- [ ] **Step 3: Run issue verification command for `perf ci --case`**

Run:

```sh
cargo run -p rstim --bin rstim -- perf ci \
  --case stim-style-surface-sample-d11-r100-b1024 \
  --warmup-rounds 0 \
  --measure-rounds 1 \
  --out-dir /tmp/rstim-vs-stim-perf-ci
```

Expected: exits zero. `/tmp/rstim-vs-stim-perf-ci/raw.jsonl`, `summary.json`, and `report.md` contain only `stim-style-surface-sample-d11-r100-b1024`.

- [ ] **Step 4: Run unknown-case negative control**

Run:

```sh
cargo run -p rstim --bin rstim -- perf run --case no-such-case --out /tmp/unused.jsonl
```

Expected: exits nonzero and stderr contains `unknown benchmark case`.

- [ ] **Step 5: Run repository gate**

Run:

```sh
cargo test
```

Expected: pass. Existing rmatching warning output about unused `saw_same_tree` variables may appear.

- [ ] **Step 6: Review, commit, push, and open PR**

Run:

```sh
git status --short
git diff --stat master...HEAD
git add docs/superpowers/plans/2026-07-08-issue-389-focused-perf-case-runner.md rstim/src/perf/record.rs rstim/src/perf/runner.rs rstim/src/perf/summary.rs rstim/src/perf/report.rs rstim/src/perf.rs rstim/src/cli.rs rstim/tests/perf_harness.rs rstim/tests/perf_runner.rs rstim/tests/perf_summary.rs rstim/tests/cli_perf.rs
git commit -m "perf: add focused benchmark case runner"
git push -u origin agent/issue-389-add-a-focused-perf-runner-mode-for-one-benchmark-run-1
gh pr create --base master --head agent/issue-389-add-a-focused-perf-runner-mode-for-one-benchmark-run-1 --title "Add focused perf case runner" --body "## Summary

- add \`--case\` to \`rstim perf run\` and \`rstim perf ci\`
- record selected-case tool failures as raw JSONL status records
- keep focused CI artifacts scoped to the selected case

## Verification

- cargo test -p rstim --test perf_harness
- cargo test -p rstim --test perf_runner
- cargo test -p rstim --test perf_summary
- cargo test -p rstim --test cli_perf
- cargo run -p rstim --bin rstim -- perf run --case stim-style-surface-sample-d11-r100-b1024 --warmup-rounds 0 --measure-rounds 1 --out /tmp/rstim-vs-stim-speed.jsonl
- cargo run -p rstim --bin rstim -- perf ci --case stim-style-surface-sample-d11-r100-b1024 --warmup-rounds 0 --measure-rounds 1 --out-dir /tmp/rstim-vs-stim-perf-ci
- cargo run -p rstim --bin rstim -- perf run --case no-such-case --out /tmp/unused.jsonl
- cargo test

Closes #389"
```

Expected: PR URL is produced. Stop after PR creation.

## Self-Review

- Spec coverage: Task 1 covers raw status semantics, summary compatibility, and report rendering. Task 2 covers selected runner failure records. Task 3 covers `--case` CLI behavior and focused CI artifacts. Task 4 covers issue verification, full `cargo test`, commit, push, and PR creation.
- Placeholder scan: no unresolved placeholders remain in commands, file paths, labels, or expected outcomes.
- Type consistency: `PerfRecordStatus`, `PerfSummaryOptions`, `summarize_jsonl_str_with_options`, `benchmark_case_by_label`, and `run_benchmark_case_to_writer` are named consistently across tasks.
