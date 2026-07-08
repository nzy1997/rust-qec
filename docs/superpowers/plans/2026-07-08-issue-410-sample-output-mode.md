# Issue 410 Sample Output Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an explicit measurement-only sampler output mode that skips detector and observable materialization while preserving the current full-output default.

**Architecture:** `SampleOptions` carries a new `SampleOutputMode` enum. Frame-based sampler paths configure `FrameSimulator` before execution, and `FrameSimulator` treats `DETECTOR` and `OBSERVABLE_INCLUDE` as no-ops in measurement-only mode. `BatchOutput` records the requested mode and materialization counters so tests can prove the cheaper path was used.

**Tech Stack:** Rust 2024, Cargo integration tests, existing `BitTable`, `FrameSimulator`, and compiled/interpreted sampler paths.

## Global Constraints

- `SampleOptions::default()` must continue to behave like full output.
- Measurement-only mode must return measurement bits without materializing detections or observable flips.
- Full mode must keep existing `BatchOutput` semantics for detections and observable flips.
- The API must make the requested mode visible and testable.
- Do not change CLI or perf call sites in this issue.
- Do not change random sampling semantics.
- Do not add wall-clock performance thresholds.
- Required focused verification command: `cargo test -p rstim --test sample_output_mode`.
- Required final verification command from Agent Desk: `cargo test`.

---

### Task 1: Add Sample Output Mode

**Files:**
- Modify: `rstim/src/sampler.rs`
- Modify: `rstim/src/compiled/sampler.rs`
- Modify: `rstim/src/sim/frame.rs`
- Create: `rstim/tests/sample_output_mode.rs`

**Interfaces:**
- Consumes: `sample_batch_with_options(instrs, n_shots, rng, SampleOptions)` and existing `BatchOutput` table fields.
- Produces: `SampleOutputMode::{Full, MeasurementsOnly}`, `SampleOptions { output_mode, .. }`, `BatchOutput.output_mode`, `BatchOutput.detector_materializations`, and `BatchOutput.observable_materializations`.

- [ ] **Step 1: Write the failing integration test**

Create `rstim/tests/sample_output_mode.rs` with:

```rust
use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::parser::parse_lines;
use rstim::sampler::{
    sample_batch_with_options, SampleOptions, SampleOutputMode, SamplingBackend,
};
use rstim::sim::bit_table::BitTable;

fn circuit_with_measurements_detectors_and_observables() -> Vec<rstim::ir::StimInstr> {
    parse_lines(
        "X 0\n\
         M 0\n\
         DETECTOR rec[-1]\n\
         OBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap()
}

fn table_rows(table: &BitTable) -> Vec<Vec<bool>> {
    (0..table.num_major())
        .map(|major| {
            (0..table.num_minor())
                .map(|minor| table.get(major, minor))
                .collect()
        })
        .collect()
}

fn sample_with_mode(mode: SampleOutputMode) -> rstim::sampler::BatchOutput {
    let instrs = circuit_with_measurements_detectors_and_observables();
    let mut rng = StdRng::seed_from_u64(123);
    sample_batch_with_options(
        &instrs,
        8,
        &mut rng,
        SampleOptions {
            backend: SamplingBackend::Compiled,
            output_mode: mode,
            ..SampleOptions::default()
        },
    )
    .unwrap()
}

#[test]
fn measurement_only_mode_preserves_measurement_bits() {
    let full = sample_with_mode(SampleOutputMode::Full);
    let measurements_only = sample_with_mode(SampleOutputMode::MeasurementsOnly);

    assert_eq!(
        table_rows(&measurements_only.measurements),
        table_rows(&full.measurements)
    );
    assert_eq!(measurements_only.measurements.num_major(), 1);
    assert_eq!(measurements_only.measurements.num_minor(), 8);
}

#[test]
fn measurement_only_mode_skips_detector_and_observable_materialization() {
    let out = sample_with_mode(SampleOutputMode::MeasurementsOnly);

    assert_eq!(out.output_mode, SampleOutputMode::MeasurementsOnly);
    assert_eq!(out.detections.num_major(), 0);
    assert_eq!(out.detections.num_minor(), 8);
    assert_eq!(out.observable_flips.num_major(), 0);
    assert_eq!(out.observable_flips.num_minor(), 8);
    assert_eq!(out.detector_materializations, 0);
    assert_eq!(out.observable_materializations, 0);
}

#[test]
fn full_mode_still_materializes_detector_and_observable_bits() {
    let out = sample_with_mode(SampleOutputMode::Full);

    assert_eq!(out.output_mode, SampleOutputMode::Full);
    assert_eq!(out.detections.num_major(), 1);
    assert_eq!(out.observable_flips.num_major(), 1);
    assert_eq!(out.detector_materializations, 1);
    assert_eq!(out.observable_materializations, 1);
    for shot in 0..8 {
        assert!(out.measurements.get(0, shot), "shot {shot}");
        assert!(!out.detections.get(0, shot), "shot {shot}");
        assert!(!out.observable_flips.get(0, shot), "shot {shot}");
    }
}

#[test]
fn default_sample_options_remain_full_output() {
    assert_eq!(SampleOptions::default().output_mode, SampleOutputMode::Full);

    let instrs = circuit_with_measurements_detectors_and_observables();
    let mut rng = StdRng::seed_from_u64(123);
    let out = sample_batch_with_options(&instrs, 8, &mut rng, SampleOptions::default()).unwrap();

    assert_eq!(out.output_mode, SampleOutputMode::Full);
    assert_eq!(out.detections.num_major(), 1);
    assert_eq!(out.observable_flips.num_major(), 1);
    assert_eq!(out.detector_materializations, 1);
    assert_eq!(out.observable_materializations, 1);
}
```

- [ ] **Step 2: Run the focused test to verify RED**

Run: `cargo test -p rstim --test sample_output_mode`

Expected: FAIL because `SampleOutputMode`, `SampleOptions::output_mode`, and the new `BatchOutput` fields do not exist.

- [ ] **Step 3: Add sampler API and output constructors**

In `rstim/src/sampler.rs`, add the enum before `SamplingBackend`:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SampleOutputMode {
    #[default]
    Full,
    MeasurementsOnly,
}
```

Change `BatchOutput` and `SampleOptions`:

```rust
pub struct BatchOutput {
    pub measurements: BitTable,
    pub detections: BitTable,
    pub observable_flips: BitTable,
    pub output_mode: SampleOutputMode,
    pub detector_materializations: usize,
    pub observable_materializations: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SampleOptions {
    pub reference_sample_mode: crate::data_path::ReferenceSampleMode,
    pub backend: SamplingBackend,
    pub output_mode: SampleOutputMode,
}
```

Add helper constructors near the struct definitions:

```rust
impl BatchOutput {
    fn full(
        measurements: BitTable,
        detections: BitTable,
        observable_flips: BitTable,
        detector_materializations: usize,
        observable_materializations: usize,
    ) -> Self {
        Self {
            measurements,
            detections,
            observable_flips,
            output_mode: SampleOutputMode::Full,
            detector_materializations,
            observable_materializations,
        }
    }

    fn measurements_only(measurements: BitTable, n_shots: usize) -> Self {
        Self {
            measurements,
            detections: BitTable::new(0, n_shots),
            observable_flips: BitTable::new(0, n_shots),
            output_mode: SampleOutputMode::MeasurementsOnly,
            detector_materializations: 0,
            observable_materializations: 0,
        }
    }
}
```

- [ ] **Step 4: Add frame materialization control**

In `rstim/src/sim/frame.rs`, add these fields to `FrameSimulator`:

```rust
materialize_detector_observable_outputs: bool,
detector_materializations: usize,
observable_materializations: usize,
```

Initialize them in `FrameSimulator::new`:

```rust
materialize_detector_observable_outputs: true,
detector_materializations: 0,
observable_materializations: 0,
```

Add public(crate) methods in the `impl FrameSimulator` block:

```rust
pub(crate) fn set_materialize_detector_observable_outputs(&mut self, enabled: bool) {
    self.materialize_detector_observable_outputs = enabled;
}

pub(crate) fn detector_materializations(&self) -> usize {
    self.detector_materializations
}

pub(crate) fn observable_materializations(&self) -> usize {
    self.observable_materializations
}
```

At the top of the `"DETECTOR"` arm in `exec_op`, add:

```rust
if !self.materialize_detector_observable_outputs {
    return Ok(());
}
self.detector_materializations += 1;
```

At the top of the `"OBSERVABLE_INCLUDE"` arm in `exec_op`, add:

```rust
if !self.materialize_detector_observable_outputs {
    return Ok(());
}
self.observable_materializations += 1;
```

- [ ] **Step 5: Wire output mode through interpreted and executor sampler paths**

In `sample_batch_interpreted`, after `FrameSimulator::new`, configure the frame:

```rust
frame.set_materialize_detector_observable_outputs(
    options.output_mode == SampleOutputMode::Full,
);
```

Replace the return construction with:

```rust
match options.output_mode {
    SampleOutputMode::Full => Ok(BatchOutput::full(
        measurements,
        frame.detections(),
        frame.observable_flips(),
        frame.detector_materializations(),
        frame.observable_materializations(),
    )),
    SampleOutputMode::MeasurementsOnly => Ok(BatchOutput::measurements_only(
        measurements,
        n_shots,
    )),
}
```

In `sample_batch_with_executor`, after measurement collection and before `measurements_to_detections_with_options`, add:

```rust
if options.output_mode == SampleOutputMode::MeasurementsOnly {
    return Ok(BatchOutput::measurements_only(measurements, n_shots));
}
```

Replace the final executor return with:

```rust
Ok(BatchOutput::full(
    measurements,
    m2d.detections,
    m2d.observable_flips,
    0,
    0,
))
```

- [ ] **Step 6: Wire output mode through compiled sampler path**

In `rstim/src/compiled/sampler.rs`, import `SampleOutputMode`, configure the frame after `FrameSimulator::new`, and return via the same `BatchOutput` constructors:

```rust
use crate::sampler::{BatchOutput, SampleOptions, SampleOutputMode};
```

```rust
frame.set_materialize_detector_observable_outputs(
    options.output_mode == SampleOutputMode::Full,
);
```

```rust
match options.output_mode {
    SampleOutputMode::Full => Ok(BatchOutput::full(
        measurements,
        frame.detections(),
        frame.observable_flips(),
        frame.detector_materializations(),
        frame.observable_materializations(),
    )),
    SampleOutputMode::MeasurementsOnly => Ok(BatchOutput::measurements_only(
        measurements,
        n_shots,
    )),
}
```

- [ ] **Step 7: Make constructors visible within the crate**

If `rstim/src/compiled/sampler.rs` cannot call the `BatchOutput` constructors, change both constructor signatures from `fn` to `pub(crate) fn` in `rstim/src/sampler.rs`.

- [ ] **Step 8: Run the focused test to verify GREEN**

Run: `cargo test -p rstim --test sample_output_mode`

Expected: PASS with 4 tests.

- [ ] **Step 9: Run affected existing sampler tests**

Run: `cargo test -p rstim --test compiled_sampler`

Expected: PASS with 5 tests.

- [ ] **Step 10: Commit implementation**

Run:

```bash
git add rstim/src/sampler.rs rstim/src/compiled/sampler.rs rstim/src/sim/frame.rs rstim/tests/sample_output_mode.rs docs/superpowers/plans/2026-07-08-issue-410-sample-output-mode.md
git commit -m "feat: add sampler measurement-only output mode"
```
