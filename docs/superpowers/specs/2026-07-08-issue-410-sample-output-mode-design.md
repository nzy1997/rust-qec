# Issue 410 Sample Output Mode Design

## Problem

`sample_batch` always returns measurements, detections, and observable flips. The `sample` workload only needs measurements, so it pays for detector and observable materialization that only `detect` and related callers need.

## Selected Approach

Add an explicit sampler output mode:

- `SampleOutputMode::Full` keeps current behavior.
- `SampleOutputMode::MeasurementsOnly` returns measurement bits and skips detector/observable materialization.
- `SampleOptions::default()` remains full output.
- `BatchOutput` records the requested mode and materialization counts so tests and later call sites can tell full output from measurement-only output.

This keeps existing `BatchOutput` table access stable for current callers while adding an explicit, testable boundary for future `sample` call-site changes. A new result enum would make the type boundary stronger, but it would force broader call-site churn that this issue explicitly defers. Computing full output and discarding detector/observable tables is rejected because it preserves the performance problem.

## Architecture

`rstim/src/sampler.rs` owns the public API. `SampleOptions` gains an `output_mode` field, and `BatchOutput` gains an `output_mode` marker plus detector/observable materialization counters. Helper constructors keep result construction consistent.

`rstim/src/sim/frame.rs` owns frame-simulator materialization. It gains a detector/observable materialization switch that defaults to enabled for compatibility. In measurement-only mode, `DETECTOR` and `OBSERVABLE_INCLUDE` instructions are treated as annotation no-ops after measurements have been recorded, so measurement bits and RNG sequencing stay unchanged while detector/observable rows are not built or stored.

`rstim/src/compiled/sampler.rs` passes the requested mode into `FrameSimulator` for the compiled fast path. The interpreted fast path does the same. The executor fallback still produces measurement bits, but in measurement-only mode it skips the `measurements_to_detections_with_options` conversion and returns empty detector/observable tables with zero materialization counters.

## Data Flow

1. Callers pass `SampleOptions { output_mode, .. }`.
2. `sample_batch_with_options` routes to the selected backend without changing backend selection.
3. Frame-based paths configure the frame simulator before `run` or `run_compiled_blocks`.
4. The frame simulator always executes measurement-producing instructions and conditionally skips detector/observable materialization.
5. `BatchOutput` returns measurement bits in both modes. Full mode returns detector/observable tables. Measurement-only mode returns empty detector/observable tables, records `SampleOutputMode::MeasurementsOnly`, and records zero materialization counts.

## Testing

Add `rstim/tests/sample_output_mode.rs` with the issue-required tests:

- `measurement_only_mode_preserves_measurement_bits`
- `measurement_only_mode_skips_detector_and_observable_materialization`
- `full_mode_still_materializes_detector_and_observable_bits`
- `default_sample_options_remain_full_output`

The tests use a circuit containing `M`, `DETECTOR`, and `OBSERVABLE_INCLUDE`. They compare measurement rows between modes, assert measurement-only mode exposes empty detector/observable tables and zero materialization counters, assert full mode exposes non-empty detector/observable output and non-zero counters, and assert the default options still request full output.

## Out Of Scope

- Do not change CLI behavior.
- Do not change perf-runner workload mode selection.
- Do not change random sampling semantics.
- Do not add wall-clock performance thresholds.

## Self Review

- No placeholders remain.
- The selected API exposes the mode and a testable materialization marker.
- The default behavior remains full output.
- The implementation scope is limited to sampler API, frame-simulator materialization, compiled sampler wiring, and focused tests.
