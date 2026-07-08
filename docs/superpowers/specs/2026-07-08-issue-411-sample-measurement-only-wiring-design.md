# Issue 411 Sample Measurement-Only Wiring Design

## Problem

Issue 410 added an explicit `SampleOutputMode`, but the public sample workflows still call the sampler through its default full-output mode. `rstim sample` and sample perf workloads only need measurement rows, so they should request `SampleOutputMode::MeasurementsOnly`. `rstim detect` and detect perf workloads still need detector and observable rows and must stay on full output.

The perf runner also needs machine-readable evidence of the selected mode. Timing alone cannot prove which sampler path was used, and benchmark artifacts should remain auditable without reading implementation code.

## Selected Approach

Use narrow call-site wiring plus an optional raw-record marker:

- `rstim sample` builds `SampleOptions` with `output_mode: SampleOutputMode::MeasurementsOnly` while preserving its existing stdout formats.
- `rstim detect` and `rstim detect --obs_out` continue to use default `SampleOptions`, which remains full output.
- `PerfWorkload::Sample` uses `SampleOutputMode::MeasurementsOnly` for rstim sampler variants.
- `PerfWorkload::Detect` uses `SampleOutputMode::Full` for rstim sampler variants.
- `PerfMeasurementRecord` gains an optional serialized field, `sample_output_mode`, with values `measurements_only` or `full` for sample and detect workloads. Analyze workloads leave it absent.

This is preferable to adding a user-visible CLI mode flag because the issue asks to keep `rstim sample` output unchanged when possible. It is also preferable to changing `SampleOptions::default()` because issue 410 explicitly kept the default backward-compatible as full output.

## Architecture

`rstim/src/cli.rs` owns the public CLI command behavior. `run_sample` already constructs `SampleOptions` for reference-sample policy, so it will set `output_mode` there. The detect helpers keep using the sampler default or an explicit full-output option if tests need a clear assertion point.

`rstim/src/perf/runner.rs` owns perf workload execution and raw record construction. The runner will derive the sampler output mode from `PerfWorkload`, pass it into `sample_batch_with_options`, and include the same mode in every raw `PerfMeasurementRecord` for sample and detect workloads. Stim CLI records get the same workload-level marker because the raw file describes the benchmark workload contract, even though the external Stim command does not consume `SampleOptions`.

`rstim/src/perf/record.rs` owns raw JSONL serialization. Adding `sample_output_mode: Option<PerfSampleOutputMode>` with `#[serde(skip_serializing_if = "Option::is_none", default)]` keeps old JSONL parse-compatible and avoids adding irrelevant mode fields to `analyze_errors` records.

## Data Flow

1. CLI `sample` parses the circuit and calls `sample_batch_with_options` with `SampleOutputMode::MeasurementsOnly`.
2. CLI `detect` parses the circuit and calls the sampler in full-output mode, then writes detector and observable output exactly as before.
3. Perf runner maps `PerfWorkload::Sample` to `SampleOutputMode::MeasurementsOnly` and `PerfWorkload::Detect` to `SampleOutputMode::Full`.
4. Perf runner writes `sample_output_mode` on raw records for sample and detect workloads.
5. Perf summaries and gates continue to compute rates and comparisons from existing fields; the new marker is audit metadata, not a threshold input.

## Testing

Add `rstim/tests/cli_sample_only.rs` with the issue-required tests:

- `sample_cli_uses_measurement_only_mode_and_preserves_output`
- `perf_sample_workload_records_measurement_only_mode`
- `sample_only_mode_does_not_change_detect_output`
- `detect_perf_workload_records_full_output_mode`

The CLI sample test should call `rstim::cli::run_sample` so it can prove the internal sampler mode through existing `BatchOutput` materialization behavior without changing user-visible stdout. The detect test should compare existing output for a circuit with `DETECTOR` and `OBSERVABLE_INCLUDE` so a measurement-only regression would fail. The perf tests should parse raw `PerfMeasurementRecord` JSONL or direct runner records and assert `sample_output_mode`.

Update the existing public-label perf CLI test so it reads the raw JSONL records from `rstim perf run --case stim-style-surface-sample-d11-r100-b1024` and confirms sample records carry `sample_output_mode: "measurements_only"`. This proves the audit evidence exists in the same public workflow reviewers run.

## Out Of Scope

- Do not rewrite checked benchmark artifacts.
- Do not add performance pass/fail thresholds.
- Do not change `rstim detect` user-visible output.
- Do not claim rstim matches Stim; this only records the implementation path selected for sample workloads.

## Self Review

- No placeholders remain.
- The design preserves `SampleOptions::default()` as full output.
- The design keeps `rstim sample` stdout and `rstim detect` output formats stable.
- The raw-record marker is machine-readable and backward-compatible for old JSONL.
- The implementation scope is limited to CLI wiring, perf runner mode selection, perf record serialization, and focused tests.
