# Issue 452 Reusable Compiled Measurement Sampler Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reusable `CompiledMeasurementSampler` that compiles typed sampler IR and builds the exact reference sample once, then samples repeated shot counts without rebuilding either object.

**Architecture:** The public sampler type lives in `rstim/src/sampler.rs` and owns the existing `CompiledCircuit`, cached reference sample, reference mode, and diagnostics counters. `rstim/src/compiled/sampler.rs` gains a crate-private helper that accepts a cached reference slice while preserving the current `sample_compiled_batch` wrapper for one-shot callers. `rstim/src/lib.rs` re-exports the new public types at crate root.

**Tech Stack:** Rust 2024, existing `rstim` compiled sampler modules, `rand::Rng`, deterministic `StdRng` integration tests.

## Global Constraints

- Public paths must be exactly `rstim::CompiledMeasurementSampler` and `rstim::CompiledMeasurementSamplerDiagnostics`.
- `CompiledMeasurementSampler::compile(instrs: &[StimInstr], reference_mode: ReferenceSampleMode) -> Result<Self, String>` must build typed IR and the exact reference sample once.
- `sampler.sample(shots: usize, rng: &mut impl Rng, output_mode: SampleOutputMode) -> Result<BatchOutput, String>` must sample arbitrary shot counts repeatedly without rebuilding typed IR or reference.
- `CompiledMeasurementSamplerDiagnostics` and `sampler.diagnostics()` must be `#[doc(hidden)]`.
- Diagnostics must be `Copy` and expose public fields `compiled_ir_builds`, `reference_builds`, and `sample_calls`.
- Compilation must run the existing compiled-path gate and reject unsupported circuits before returning a sampler.
- Unsupported `S 0\nM 0\n` must return `unsupported sampler instructions require the interpreted path`.
- Do not replace the legacy reference tableau or change unsupported, loss, feedback, or top-level automatic fallback semantics.
- Required focused verification command: `cargo test -p rstim --test reusable_compiled_measurement_sampler -- --nocapture`.
- Required final verification command from Agent Desk: `cargo test`.

---

## File Structure

- Create `rstim/tests/reusable_compiled_measurement_sampler.rs`: required integration tests and acceptance print.
- Modify `rstim/src/compiled/sampler.rs`: add reference-aware helper and keep existing one-shot helper behavior.
- Modify `rstim/src/compiled/mod.rs`: re-export the reference-aware helper within the crate.
- Modify `rstim/src/sampler.rs`: add reusable sampler type, diagnostics type, compile method, sample method, and hidden diagnostics accessor.
- Modify `rstim/src/lib.rs`: re-export root public types.

### Task 1: Add Failing Reusable Sampler Tests

**Files:**
- Create: `rstim/tests/reusable_compiled_measurement_sampler.rs`

**Interfaces:**
- Consumes: `rstim::{CompiledMeasurementSampler, CompiledMeasurementSamplerDiagnostics}`, `rstim::data_path::ReferenceSampleMode`, `rstim::parser::parse_lines`, and `rstim::sampler::SampleOutputMode`.
- Produces: four integration tests named `compile_once_samples_many_batches`, `compiled_sampler_caches_nonzero_reference_bits`, `compiled_sampler_preserves_both_output_modes`, and `unsupported_circuit_is_rejected_at_compile_time`.

- [ ] **Step 1: Write the failing integration test**

Create `rstim/tests/reusable_compiled_measurement_sampler.rs` with tests that:

```rust
use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::data_path::ReferenceSampleMode;
use rstim::parser::parse_lines;
use rstim::sampler::SampleOutputMode;
use rstim::{CompiledMeasurementSampler, CompiledMeasurementSamplerDiagnostics};

fn assert_diagnostics(
    actual: CompiledMeasurementSamplerDiagnostics,
    compiled_ir_builds: usize,
    reference_builds: usize,
    sample_calls: usize,
) {
    assert_eq!(actual.compiled_ir_builds, compiled_ir_builds);
    assert_eq!(actual.reference_builds, reference_builds);
    assert_eq!(actual.sample_calls, sample_calls);
}
```

The test bodies must assert the exact diagnostics lifecycle, nonzero reference
measurement bit, both output modes, unsupported compile-time error, and print
`PASS reusable compiled measurement sampler` from the lifecycle test.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rstim --test reusable_compiled_measurement_sampler -- --nocapture`

Expected: FAIL because the test target or public API does not exist yet.

### Task 2: Add Cached Reference Sampling Path

**Files:**
- Modify: `rstim/src/compiled/sampler.rs`
- Modify: `rstim/src/compiled/mod.rs`

**Interfaces:**
- Consumes: existing `sample_compiled_batch(compiled, n_shots, rng, options)`.
- Produces: `pub(crate) fn sample_compiled_batch_with_reference(compiled: &CompiledCircuit, reference_sample: &[bool], n_shots: usize, rng: &mut impl Rng, options: SampleOptions) -> Result<BatchOutput, String>`.

- [ ] **Step 1: Add the helper**

Move the existing frame execution body from `sample_compiled_batch` into
`sample_compiled_batch_with_reference`, replacing the internal
`build_reference_sample` call with the provided `reference_sample` slice.

- [ ] **Step 2: Preserve one-shot behavior**

Keep `sample_compiled_batch` public within `rstim::compiled`; it should build
the reference sample exactly as before and then call
`sample_compiled_batch_with_reference`.

- [ ] **Step 3: Re-export for crate use**

Update `rstim/src/compiled/mod.rs` to re-export
`sample_compiled_batch_with_reference`.

### Task 3: Add Public Reusable Sampler API

**Files:**
- Modify: `rstim/src/sampler.rs`
- Modify: `rstim/src/lib.rs`

**Interfaces:**
- Consumes: `compile_circuit`, `choose_sampler_path`, `build_reference_sample`, `sample_compiled_batch_with_reference`.
- Produces: `CompiledMeasurementSampler`, `CompiledMeasurementSamplerDiagnostics`, root re-exports, and hidden diagnostics accessor.

- [ ] **Step 1: Add diagnostics type**

Add a hidden `CompiledMeasurementSamplerDiagnostics` with required public fields
and derives `Debug, Clone, Copy, Default, PartialEq, Eq`.

- [ ] **Step 2: Add sampler struct and compile method**

`compile` must build the compiled circuit once, run `choose_sampler_path`, build
the reference sample once, and return a sampler with diagnostics set to
`1, 1, 0`.

- [ ] **Step 3: Add sample method and diagnostics accessor**

`sample` must increment only `sample_calls`, preserve cached setup counters, and
call `sample_compiled_batch_with_reference` with per-call `SampleOutputMode`.
`diagnostics()` must return the hidden copy snapshot.

- [ ] **Step 4: Add root re-export**

Update `rstim/src/lib.rs` with:

```rust
pub use sampler::{CompiledMeasurementSampler, CompiledMeasurementSamplerDiagnostics};
```

### Task 4: Verify And Commit

**Files:**
- All modified files from Tasks 1-3.

**Interfaces:**
- Consumes: completed implementation.
- Produces: verified commit ready for PR.

- [ ] **Step 1: Run focused verification**

Run: `cargo test -p rstim --test reusable_compiled_measurement_sampler -- --nocapture`

Expected: PASS and output includes `PASS reusable compiled measurement sampler`.

- [ ] **Step 2: Run full verification**

Run: `cargo test`

Expected: PASS.

- [ ] **Step 3: Review diff**

Run: `git diff --stat` and `git diff --check`.

Expected: no whitespace errors and only issue-scoped files changed.

- [ ] **Step 4: Commit**

Run:

```bash
git add docs/superpowers/specs/2026-07-10-issue-452-reusable-compiled-measurement-sampler-design.md docs/superpowers/plans/2026-07-10-issue-452-reusable-compiled-measurement-sampler.md rstim/src/compiled/sampler.rs rstim/src/compiled/mod.rs rstim/src/sampler.rs rstim/src/lib.rs rstim/tests/reusable_compiled_measurement_sampler.rs
git commit -m "feat: add reusable compiled measurement sampler"
```

## Self-Review

- The plan covers every issue-required API, diagnostic field, test name, output string, and error string.
- The plan keeps legacy one-shot compiled sampling behavior intact.
- The plan does not change unsupported, loss, feedback, or automatic fallback semantics.
- No placeholders remain.
