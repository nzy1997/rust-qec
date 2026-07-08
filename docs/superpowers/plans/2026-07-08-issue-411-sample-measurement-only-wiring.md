# Issue 411 Sample Measurement-Only Wiring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire public sample workflows to `SampleOutputMode::MeasurementsOnly` while keeping detect workflows full-output and recording the selected mode in perf raw JSONL.

**Architecture:** CLI wiring stays in `rstim/src/cli.rs` by adding a small reusable sample-options helper and using it from `run_sample`. Perf wiring stays in `rstim/src/perf/runner.rs` by deriving sampler mode from `PerfWorkload`, and raw JSONL evidence is added through an optional `PerfMeasurementRecord.sample_output_mode` field in `rstim/src/perf/record.rs`.

**Tech Stack:** Rust 2024, Cargo integration tests, existing `rstim::sampler::SampleOutputMode`, existing perf runner and JSONL record types.

## Global Constraints

- `rstim sample` must keep the same user-visible measurement rows and stdout formats.
- `rstim detect` must keep detector and observable output behavior and user-visible format.
- `PerfWorkload::Sample` must request measurement-only sampler output.
- `PerfWorkload::Detect` must remain visibly full-output.
- Perf raw records for sample workloads must include a machine-readable mode marker with value `measurements_only`.
- Detect perf raw records must remain visibly full-output with mode marker value `full`.
- Existing checked benchmark artifacts must not be rewritten.
- Do not introduce performance pass/fail thresholds.
- Do not claim rstim matches Stim; this is an implementation-path change.
- Required focused verification command: `cargo test -p rstim --test cli_sample_only`.
- Required public-label verification command: `cargo test -p rstim --test cli_perf -- --exact perf_run_case_with_public_label_writes_only_selected_completed_records`.
- Required final verification command from Agent Desk: `cargo test`.

---

### Task 1: Wire CLI And Perf Sample Mode

**Files:**
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/src/perf/record.rs`
- Modify: `rstim/src/perf/runner.rs`
- Modify: `rstim/src/perf.rs`
- Modify: `rstim/tests/cli_perf.rs`
- Modify: `rstim/tests/perf_harness.rs`
- Create: `rstim/tests/cli_sample_only.rs`

**Interfaces:**
- Consumes: `SampleOptions { output_mode, .. }`, `SampleOutputMode::{Full, MeasurementsOnly}`, `run_sample`, `run_detect`, `run_case_measurements`, `run_benchmark_case_to_writer`, and `PerfMeasurementRecord::from_json_line`.
- Produces: `rstim::cli::sample_cli_options(skip_reference_sample: bool) -> SampleOptions`, `PerfSampleOutputMode::{Full, MeasurementsOnly}`, and optional `PerfMeasurementRecord.sample_output_mode`.

- [ ] **Step 1: Write the failing integration tests**

Create `rstim/tests/cli_sample_only.rs` with:

```rust
use rstim::cli::{run_detect, run_sample, sample_cli_options};
use rstim::data_path::ReferenceSampleMode;
use rstim::perf::{
    benchmark_case_by_label, run_case_measurements, PerfRunOptions, PerfSampleOutputMode,
    PerfVariant,
};
use rstim::sampler::SampleOutputMode;

#[test]
fn sample_cli_uses_measurement_only_mode_and_preserves_output() {
    let default_options = sample_cli_options(false);
    assert_eq!(default_options.output_mode, SampleOutputMode::MeasurementsOnly);
    assert_eq!(
        default_options.reference_sample_mode,
        ReferenceSampleMode::SimulateNoiseless
    );

    let skipped_options = sample_cli_options(true);
    assert_eq!(skipped_options.output_mode, SampleOutputMode::MeasurementsOnly);
    assert_eq!(
        skipped_options.reference_sample_mode,
        ReferenceSampleMode::AssumeAllZero
    );

    let mut out = Vec::new();
    run_sample(
        "R 0\nX 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        2,
        "01",
        Some(7),
        false,
        &mut out,
    )
    .expect("sample command");

    assert_eq!(String::from_utf8(out).unwrap(), "1\n1\n");
}

#[test]
fn perf_sample_workload_records_measurement_only_mode() {
    let case = benchmark_case_by_label("loss-protection-sample").unwrap();
    let records = run_case_measurements(
        case,
        "LOSS(1) 0\nMRL 0\nDETECTOR rec[-1]\n",
        &[PerfVariant::RstimInterpreted],
        PerfRunOptions {
            warmup_rounds: 0,
            measured_rounds: 1,
        },
    )
    .expect("sample perf records");

    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].sample_output_mode,
        Some(PerfSampleOutputMode::MeasurementsOnly)
    );
}

#[test]
fn sample_only_mode_does_not_change_detect_output() {
    let mut out = Vec::new();
    run_detect(
        "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        1,
        "dets",
        Some(7),
        false,
        &mut out,
    )
    .expect("detect command");

    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("D0"), "detect output missing detector: {text}");
    assert!(text.contains("L0"), "detect output missing observable: {text}");
}

#[test]
fn detect_perf_workload_records_full_output_mode() {
    let case = benchmark_case_by_label("surface-detect-d13-r13").unwrap();
    let records = run_case_measurements(
        case,
        "X_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        &[PerfVariant::RstimInterpreted],
        PerfRunOptions {
            warmup_rounds: 0,
            measured_rounds: 1,
        },
    )
    .expect("detect perf records");

    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].sample_output_mode,
        Some(PerfSampleOutputMode::Full)
    );
}
```

Update the import in `rstim/tests/cli_perf.rs` from:

```rust
use rstim::perf::summarize_jsonl_str;
```

to:

```rust
use rstim::perf::{summarize_jsonl_str, PerfMeasurementRecord, PerfSampleOutputMode};
```

In `perf_run_case_with_public_label_writes_only_selected_completed_records`, after the existing raw assertions, add:

```rust
let records = raw
    .lines()
    .map(|line| PerfMeasurementRecord::from_json_line(line).unwrap())
    .collect::<Vec<_>>();
assert!(!records.is_empty());
assert!(records.iter().all(|record| {
    record.sample_output_mode == Some(PerfSampleOutputMode::MeasurementsOnly)
}));
```

- [ ] **Step 2: Run the focused tests to verify RED**

Run: `cargo test -p rstim --test cli_sample_only`

Expected: FAIL because `sample_cli_options`, `PerfSampleOutputMode`, and `PerfMeasurementRecord.sample_output_mode` do not exist.

Run: `cargo test -p rstim --test cli_perf -- --exact perf_run_case_with_public_label_writes_only_selected_completed_records`

Expected: FAIL because `sample_output_mode` is not recorded in parsed raw records.

- [ ] **Step 3: Add the CLI sample options helper and wire `run_sample`**

In `rstim/src/cli.rs`, change the sampler import to include `SampleOutputMode`:

```rust
use crate::sampler::{SampleOptions, SampleOutputMode, sample_batch, sample_batch_with_options};
```

Add this helper above `run_sample`:

```rust
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
```

In `run_sample`, replace the inline `SampleOptions` literal with:

```rust
let options = sample_cli_options(skip_reference_sample);
```

Leave `run_detect` and `run_detect_with_obs` on `sample_batch`, which uses `SampleOptions::default()` and therefore full output.

- [ ] **Step 4: Add the perf raw mode enum and optional record field**

In `rstim/src/perf/record.rs`, add the enum after `PerfMeasurementRecord`:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PerfSampleOutputMode {
    Full,
    MeasurementsOnly,
}
```

Add this field to `PerfMeasurementRecord` after `peak_memory_bytes`:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub sample_output_mode: Option<PerfSampleOutputMode>,
```

In `rstim/src/perf.rs`, export it with the existing record types:

```rust
pub use record::{PerfMeasurementRecord, PerfRecord, PerfRecordStatus, PerfSampleOutputMode};
```

In `rstim/tests/perf_harness.rs`, add `sample_output_mode: None,` to each direct `PerfMeasurementRecord` literal so those tests keep describing non-workload-specific serialization behavior.

- [ ] **Step 5: Wire perf runner mode selection and record evidence**

In `rstim/src/perf/runner.rs`, update imports:

```rust
use crate::sampler::{SampleOptions, SampleOutputMode, SamplingBackend, sample_batch_with_options};
```

and:

```rust
PerfBenchmarkCase, PerfMeasurementRecord, PerfRecordStatus, PerfSampleOutputMode, PerfVariant,
PerfWorkload,
```

Add helper functions near `run_variant`:

```rust
fn sampler_output_mode_for_workload(workload: PerfWorkload) -> Option<SampleOutputMode> {
    match workload {
        PerfWorkload::Sample => Some(SampleOutputMode::MeasurementsOnly),
        PerfWorkload::Detect => Some(SampleOutputMode::Full),
        PerfWorkload::AnalyzeErrors => None,
    }
}

fn perf_sample_output_mode_for_workload(workload: PerfWorkload) -> Option<PerfSampleOutputMode> {
    match workload {
        PerfWorkload::Sample => Some(PerfSampleOutputMode::MeasurementsOnly),
        PerfWorkload::Detect => Some(PerfSampleOutputMode::Full),
        PerfWorkload::AnalyzeErrors => None,
    }
}
```

In the `PerfWorkload::Sample | PerfWorkload::Detect` arm of `run_variant`, change the `SampleOptions` literal to:

```rust
SampleOptions {
    backend,
    output_mode: sampler_output_mode_for_workload(case.workload)
        .expect("sample and detect workloads have sampler output modes"),
    ..SampleOptions::default()
}
```

In each `PerfMeasurementRecord` construction in `run_case_measurements` and `run_selected_case_measurements`, add:

```rust
sample_output_mode: perf_sample_output_mode_for_workload(case.workload),
```

This includes completed records and missing-variant records so selected-case raw files remain auditable even when a variant is unavailable.

- [ ] **Step 6: Run focused tests to verify GREEN**

Run: `cargo test -p rstim --test cli_sample_only`

Expected: PASS with the four issue-required tests.

Run: `cargo test -p rstim --test cli_perf -- --exact perf_run_case_with_public_label_writes_only_selected_completed_records`

Expected: PASS and parsed raw records all carry `sample_output_mode: measurements_only`.

- [ ] **Step 7: Run broader checks and commit**

Run: `cargo test -p rstim --test perf_runner`

Expected: PASS; confirms existing perf runner behavior still works with the new record field.

Run: `cargo test`

Expected: PASS for the workspace.

Commit:

```bash
git add rstim/src/cli.rs rstim/src/perf.rs rstim/src/perf/record.rs rstim/src/perf/runner.rs rstim/tests/cli_sample_only.rs rstim/tests/cli_perf.rs rstim/tests/perf_harness.rs
git commit -m "feat: wire sample workflows to measurement-only mode"
```

## Self Review

- Spec coverage: CLI sample wiring, detect full-output preservation, perf sample/detect markers, and public-label raw JSONL evidence are all covered by Task 1.
- Placeholder scan: no placeholders, TODOs, or deferred implementation steps remain.
- Type consistency: `SampleOutputMode` is the sampler API mode, `PerfSampleOutputMode` is the serialized raw-record mode, and `sample_output_mode` is optional for backward-compatible JSONL parsing.
