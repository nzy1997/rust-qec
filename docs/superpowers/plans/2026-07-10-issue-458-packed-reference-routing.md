# Issue 458 Packed Reference Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route supported noiseless reference construction through `PackedInverseTableau`, preserve typed legacy fallback reasons, and expose typed direct-compile and `Auto` recovery decisions.

**Architecture:** The compiled routing layer owns the typed fallback reason enum so compiled sampler selection, packed reference selection, and top-level `Auto` recovery share one vocabulary. `rstim/src/data_path.rs` owns packed-reference execution and legacy fallback. `rstim/src/sampler.rs` keeps existing public wrappers while adding hidden decision-returning helpers for tests and telemetry.

**Tech Stack:** Rust 2024, `rstim` crate integration tests, `PackedInverseTableau`, existing legacy `Executor` and frame simulator sampling paths.

## Global Constraints

- Preserve existing public wrappers: `build_reference_sample`, `CompiledMeasurementSampler::compile(...)->Result<Self, String>`, and `sample_batch_with_options` must remain callable.
- Expose test-visible decisions equivalent to `PackedInverse`, `LegacyFallback(Loss)`, `LegacyFallback(MeasurementRecordFeedback)`, `LegacyFallback(SweepDependent)`, and `LegacyFallback(UnsupportedOperation(name))`.
- `CompiledMeasurementSampler::compile_with_decision` must reject non-fast-path circuits with the same typed reason and must not silently embed legacy reference construction.
- `SamplingBackend::Auto` must recover through interpreted legacy sampling and expose `InterpretedLegacy(reason)` through a hidden decision helper.
- Scan `REPEAT` bodies recursively for packed-reference support and fallback reasons.
- Metadata and noiselessly skipped noise, including `TICK`, coordinates, detector/observable annotations, `X_ERROR`, `Y_ERROR`, `Z_ERROR`, and depolarization, must not force fallback by themselves.
- The canonical fixture `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim` must select `PackedInverse` and return exactly 12,121 false bits.
- Do not remove legacy support, add unsupported packed gates, or add timing thresholds.
- Required focused verification command: `cargo test -p rstim --test packed_reference_routing -- --nocapture`.
- Required benchmark verification commands: release build, distribution case SHA check, and `python3 -m benchmarks.rstim_vs_stim_simulator.verify_distributions ...`.
- Required final Agent Desk verification command: `cargo test`.

---

## File Structure

- Create `rstim/tests/packed_reference_routing.rs`: issue acceptance tests for reference routing, direct compile rejection, `Auto` recovery, canonical fixture packed selection, and nested repeat packed selection.
- Modify `rstim/src/compiled/path.rs`: add typed fallback reasons and update sampler path selection to return the first typed reason.
- Modify `rstim/src/compiled/circuit.rs`: track sweep-dependent sampler operations recursively.
- Modify `rstim/src/data_path.rs`: add reference decision/result types, packed-reference execution, sweep-aware decision helper, and compatibility wrapper.
- Modify `rstim/src/executor.rs`: add legacy measurement-record and sweep controlled-pair execution so interpreted fallback preserves samples.
- Modify `rstim/src/sampler.rs`: add typed compiled-sampler constructor, top-level decision helpers, sweep-aware test helper, and executor fallback routing for feedback/sweep.
- Modify existing tests that assert `CompiledPathDecision::Fallback(&str)` to assert the typed fallback enum.
- Modify `rstim/src/compiled/mod.rs` only if a new routing type needs a crate/root re-export.

### Task 1: Add Failing Packed Reference Routing Tests

**Files:**
- Create: `rstim/tests/packed_reference_routing.rs`

**Interfaces:**
- Consumes planned but not-yet-existing APIs:
  - `rstim::compiled::SamplingFallbackReason`
  - `rstim::data_path::{build_reference_sample_with_decision, build_reference_sample_with_sweep_bits_and_decision, ReferenceSampleDecision}`
  - `rstim::sampler::{sample_batch_with_options_and_decision, sample_batch_with_options_sweep_bits_and_decision, SampleBatchDecision}`
  - `rstim::CompiledMeasurementSampler::compile_with_decision`
- Produces the failing integration coverage required by issue #458.

- [ ] **Step 1: Write the failing integration test**

Create `rstim/tests/packed_reference_routing.rs` with this structure:

```rust
use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::compiled::SamplingFallbackReason;
use rstim::data_path::{
    ReferenceSampleDecision, build_reference_sample_with_decision,
    build_reference_sample_with_sweep_bits_and_decision,
};
use rstim::parser::parse_lines;
use rstim::sampler::{
    SampleBatchDecision, SampleOptions, SamplingBackend, sample_batch_with_options_and_decision,
    sample_batch_with_options_sweep_bits_and_decision,
};
use rstim::CompiledMeasurementSampler;
use rstim::sim::bit_table::BitTable;

const SURFACE_D11_R100: &str = include_str!(
    "../../benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
);

#[derive(Clone)]
struct FallbackCase {
    name: &'static str,
    circuit: &'static str,
    sweep_bits: Option<&'static [bool]>,
    reason: SamplingFallbackReason,
    legacy_bits: Vec<bool>,
}

fn fallback_cases() -> Vec<FallbackCase> {
    vec![
        FallbackCase {
            name: "loss",
            circuit: "LOSS(0) 0\nM 0\n",
            sweep_bits: None,
            reason: SamplingFallbackReason::Loss,
            legacy_bits: vec![false],
        },
        FallbackCase {
            name: "feedback",
            circuit: "X 0\nM 0\nCX rec[-1] 1\nM 1\n",
            sweep_bits: None,
            reason: SamplingFallbackReason::MeasurementRecordFeedback,
            legacy_bits: vec![true, true],
        },
        FallbackCase {
            name: "sweep",
            circuit: "X 1\nCX sweep[0] 1\nM 1\n",
            sweep_bits: Some(&[true]),
            reason: SamplingFallbackReason::SweepDependent,
            legacy_bits: vec![false],
        },
        FallbackCase {
            name: "unsupported",
            circuit: "H 1\nX 0\nCZ 0 1\nH 1\nM 0 1\n",
            sweep_bits: None,
            reason: SamplingFallbackReason::UnsupportedOperation("CZ".to_string()),
            legacy_bits: vec![true, true],
        },
    ]
}

fn measurement_rows(table: &BitTable) -> Vec<Vec<bool>> {
    (0..table.num_major())
        .map(|row| (0..table.num_minor()).map(|shot| table.get(row, shot)).collect())
        .collect()
}
```

Add tests:

```rust
#[test]
fn reference_construction_reports_packed_or_typed_legacy_fallback() {
    for case in fallback_cases() {
        let instrs = parse_lines(case.circuit).unwrap();
        let result = match case.sweep_bits {
            Some(bits) => build_reference_sample_with_sweep_bits_and_decision(&instrs, Some(bits)),
            None => build_reference_sample_with_decision(&instrs),
        }
        .unwrap();

        assert_eq!(result.bits, case.legacy_bits, "{}", case.name);
        assert_eq!(
            result.decision,
            ReferenceSampleDecision::LegacyFallback(case.reason.clone()),
            "{}",
            case.name
        );
    }
}

#[test]
fn direct_compiled_sampler_rejects_with_typed_reason() {
    for case in fallback_cases() {
        let instrs = parse_lines(case.circuit).unwrap();
        let err = CompiledMeasurementSampler::compile_with_decision(
            &instrs,
            rstim::data_path::ReferenceSampleMode::SimulateNoiseless,
        )
        .unwrap_err();
        assert_eq!(err, case.reason, "{}", case.name);
    }
}

#[test]
fn auto_backend_recovers_through_interpreted_legacy_with_reason() {
    for case in fallback_cases() {
        let instrs = parse_lines(case.circuit).unwrap();
        let mut rng = StdRng::seed_from_u64(458);
        let (out, decision) = match case.sweep_bits {
            Some(bits) => sample_batch_with_options_sweep_bits_and_decision(
                &instrs,
                1,
                &mut rng,
                SampleOptions {
                    backend: SamplingBackend::Auto,
                    output_mode: rstim::sampler::SampleOutputMode::MeasurementsOnly,
                    ..SampleOptions::default()
                },
                Some(bits),
            ),
            None => sample_batch_with_options_and_decision(
                &instrs,
                1,
                &mut rng,
                SampleOptions {
                    backend: SamplingBackend::Auto,
                    output_mode: rstim::sampler::SampleOutputMode::MeasurementsOnly,
                    ..SampleOptions::default()
                },
            ),
        }
        .unwrap();

        assert_eq!(
            decision,
            SampleBatchDecision::InterpretedLegacy(case.reason.clone()),
            "{}",
            case.name
        );
        assert_eq!(
            measurement_rows(&out.measurements)
                .into_iter()
                .map(|row| row[0])
                .collect::<Vec<_>>(),
            case.legacy_bits,
            "{}",
            case.name
        );
    }
}
```

Add positive packed tests:

```rust
#[test]
fn canonical_surface_fixture_uses_packed_reference_and_all_false_bits() {
    let instrs = parse_lines(SURFACE_D11_R100).unwrap();
    let result = build_reference_sample_with_decision(&instrs).unwrap();

    assert_eq!(result.decision, ReferenceSampleDecision::PackedInverse);
    assert_eq!(result.bits.len(), 12_121);
    assert!(result.bits.iter().all(|bit| !*bit));
}

#[test]
fn nested_repeat_metadata_and_noiseless_noise_stay_packed() {
    let circuit = "\
REPEAT 2 {
  H 0
  M 0
  X_ERROR(0.0) 0
  TICK
  DETECTOR rec[-1]
  OBSERVABLE_INCLUDE(0) rec[-1]
  REPEAT 3 {
    H 0
    M 0
    X_ERROR(0.0) 0
    TICK
    DETECTOR rec[-1]
    OBSERVABLE_INCLUDE(0) rec[-1]
  }
}
";
    let instrs = parse_lines(circuit).unwrap();
    let result = build_reference_sample_with_decision(&instrs).unwrap();

    assert_eq!(result.decision, ReferenceSampleDecision::PackedInverse);
    assert_eq!(result.bits.len(), 8);
    println!("PASS packed reference routing");
}
```

- [ ] **Step 2: Run test to verify RED**

Run:

```bash
cargo test -p rstim --test packed_reference_routing -- --nocapture
```

Expected: FAIL with unresolved imports and missing methods, proving the new tests target absent behavior.

- [ ] **Step 3: Commit the red test**

Run:

```bash
git add rstim/tests/packed_reference_routing.rs
git commit -m "test: cover packed reference routing"
```

Expected: commit succeeds with only the failing acceptance test added.

---

### Task 2: Add Typed Routing Reasons And Packed Reference Builder

**Files:**
- Modify: `rstim/src/compiled/path.rs`
- Modify: `rstim/src/compiled/circuit.rs`
- Modify: `rstim/src/data_path.rs`
- Modify: tests that assert `CompiledPathDecision::Fallback(...)`

**Interfaces:**
- Produces:
  - `pub enum SamplingFallbackReason`
  - `CompiledPathDecision::Fallback(SamplingFallbackReason)`
  - `ReferenceSampleDecision`
  - `ReferenceSampleResult`
  - `build_reference_sample_with_decision`
  - `build_reference_sample_with_sweep_bits_and_decision`

- [ ] **Step 1: Add typed fallback reason**

In `rstim/src/compiled/path.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SamplingFallbackReason {
    Loss,
    MeasurementRecordFeedback,
    SweepDependent,
    UnsupportedOperation(String),
}

impl SamplingFallbackReason {
    pub fn message(&self) -> &'static str {
        match self {
            SamplingFallbackReason::Loss => "loss instructions require the interpreted path",
            SamplingFallbackReason::MeasurementRecordFeedback => {
                "feedback instructions require the interpreted path"
            }
            SamplingFallbackReason::SweepDependent => {
                "sweep-dependent instructions require the interpreted path"
            }
            SamplingFallbackReason::UnsupportedOperation(_) => {
                "unsupported sampler instructions require the interpreted path"
            }
        }
    }
}

impl std::fmt::Display for SamplingFallbackReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}
```

Change `CompiledPathDecision` to:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompiledPathDecision {
    FastPath,
    Fallback(SamplingFallbackReason),
}
```

Update `choose_sampler_path` so loss, feedback, sweep, and unsupported markers
return the typed enum in that order.

- [ ] **Step 2: Track sweep-dependent sampler operations and first unsupported op**

In `rstim/src/compiled/circuit.rs`, add `pub has_sweep_dependency: bool` to
`CompiledFeatureFlags`. Set it while compiling every `StimInstr::Op`:

```rust
flags.has_sweep_dependency |= is_sweep_dependent_sampler_operation(name, targets);
```

Add:

```rust
fn is_sweep_dependent_sampler_operation(name: &str, targets: &[StimTarget]) -> bool {
    targets.iter().any(|target| matches!(target, StimTarget::Sweep(_)))
        && !is_noiseless_reference_skip(name)
}

fn is_noiseless_reference_skip(name: &str) -> bool {
    matches!(
        name,
        "TICK"
            | "QUBIT_COORDS"
            | "SHIFT_COORDS"
            | "DETECTOR"
            | "OBSERVABLE_INCLUDE"
            | "X_ERROR"
            | "Y_ERROR"
            | "Z_ERROR"
            | "DEPOLARIZE1"
            | "DEPOLARIZE2"
            | "I_ERROR"
            | "II_ERROR"
    )
}
```

In `rstim/src/compiled/path.rs`, replace the boolean unsupported scan with:

```rust
fn first_unsupported_sampler_op(blocks: &[CompiledBlock]) -> Option<String> {
    blocks.iter().find_map(|block| match block {
        CompiledBlock::Ops(ops) => ops.iter().find_map(|op| match op {
            CompiledOp::UnsupportedSamplerOp { name } => Some(name.clone()),
            _ => None,
        }),
        CompiledBlock::Repeat(region) => first_unsupported_sampler_op(&region.body),
    })
}
```

- [ ] **Step 3: Implement packed reference routing**

Replace `rstim/src/data_path.rs` with the compatible wrapper plus new typed
helpers. Keep imports local to the module:

```rust
use crate::compiled::SamplingFallbackReason;
use crate::executor::{reference_sample, reference_sample_with_sweep_bits};
use crate::ir::{StimInstr, StimTarget};
use crate::sim::packed_inverse_tableau::PackedInverseTableau;
```

Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceSampleDecision {
    PackedInverse,
    LegacyFallback(SamplingFallbackReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceSampleResult {
    pub bits: Vec<bool>,
    pub decision: ReferenceSampleDecision,
}

pub fn build_reference_sample_with_decision(
    instrs: &[StimInstr],
) -> Result<ReferenceSampleResult, String> {
    build_reference_sample_with_sweep_bits_and_decision(instrs, None)
}

pub fn build_reference_sample_with_sweep_bits_and_decision(
    instrs: &[StimInstr],
    sweep_bits: Option<&[bool]>,
) -> Result<ReferenceSampleResult, String> {
    match packed_reference_sample(instrs) {
        Ok(bits) => Ok(ReferenceSampleResult {
            bits,
            decision: ReferenceSampleDecision::PackedInverse,
        }),
        Err(reason) => Ok(ReferenceSampleResult {
            bits: reference_sample_with_sweep_bits(instrs, sweep_bits)?,
            decision: ReferenceSampleDecision::LegacyFallback(reason),
        }),
    }
}
```

Keep:

```rust
pub fn build_reference_sample(
    instrs: &[StimInstr],
    mode: ReferenceSampleMode,
) -> Result<Vec<bool>, String> {
    match mode {
        ReferenceSampleMode::SimulateNoiseless => {
            Ok(build_reference_sample_with_decision(instrs)?.bits)
        }
        ReferenceSampleMode::AssumeAllZero => {
            Ok(vec![false; crate::stats::num_measurements(instrs)])
        }
    }
}
```

Implement `packed_reference_sample`, `apply_packed_reference_instrs`, and
`apply_packed_reference_op`. Use `crate::executor::max_qubit(instrs)?` for the
qubit count and `PackedInverseTableau::identity(num_qubits)`. The operation
match must:

```rust
match name {
    "I" | "I_ERROR" | "II_ERROR" => {}
    "H" => for q in qubits(targets)? { tableau.h(q); }
    "S" | "SQRT_Z" => for q in qubits(targets)? { tableau.s(q); }
    "S_DAG" | "SQRT_Z_DAG" => for q in qubits(targets)? { tableau.s_dag(q); }
    "X" => for q in qubits(targets)? { tableau.x_gate(q); }
    "Y" => for q in qubits(targets)? { tableau.y_gate(q); }
    "Z" => for q in qubits(targets)? { tableau.z_gate(q); }
    "CX" | "CNOT" | "ZCX" => for (c, t) in qubit_pairs(targets)? { tableau.cx(c, t); }
    "M" | "MZ" => for (q, inv) in qubits_with_inversion(targets)? {
        measurements.push(tableau.measure_z_biased(q, inv));
    }
    "MX" => for (q, inv) in qubits_with_inversion(targets)? {
        measurements.push(tableau.measure_x_biased(q, inv));
    }
    "MY" => for (q, inv) in qubits_with_inversion(targets)? {
        measurements.push(tableau.measure_y_biased(q, inv));
    }
    "MR" | "MRZ" => for (q, inv) in qubits_with_inversion(targets)? {
        measurements.push(tableau.measure_reset_z_biased(q, inv));
    }
    "MRX" => for (q, inv) in qubits_with_inversion(targets)? {
        measurements.push(tableau.measure_reset_x_biased(q, inv));
    }
    "MRY" => for (q, inv) in qubits_with_inversion(targets)? {
        measurements.push(tableau.measure_reset_y_biased(q, inv));
    }
    "R" | "RZ" => for q in qubits(targets)? { tableau.reset_z_biased(q); }
    "RX" => for q in qubits(targets)? { tableau.reset_x_biased(q); }
    "RY" => for q in qubits(targets)? { tableau.reset_y_biased(q); }
    "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" | "DETECTOR" | "OBSERVABLE_INCLUDE"
    | "X_ERROR" | "Y_ERROR" | "Z_ERROR" | "DEPOLARIZE1" | "DEPOLARIZE2" => {}
    other if is_loss_operation(other) => return Err(SamplingFallbackReason::Loss),
    other => return Err(SamplingFallbackReason::UnsupportedOperation(other.to_string())),
}
```

Before the match, return `MeasurementRecordFeedback` when a non-metadata op has
any `StimTarget::Rec(_)`, and return `SweepDependent` when a non-skip op has any
`StimTarget::Sweep(_)`.

Add target helpers in `data_path.rs` that accept only plain qubits for gates,
plain pairs for `CX`, and qubit or inverted qubit targets for measurement.

- [ ] **Step 4: Update typed routing assertions**

Update existing tests in:

- `rstim/tests/compiled_routing.rs`
- `rstim/tests/compiled_sampler.rs`
- `rstim/tests/compiled_sampler_ir.rs`
- `rstim/tests/sample_correctness_contract.rs`

Replace string fallback expectations with `SamplingFallbackReason` values.
Keep string-returning public wrapper tests where they explicitly call
`sample_batch_with_options` with `SamplingBackend::Compiled`.

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p rstim --test packed_reference_routing -- --nocapture
cargo test -p rstim --test compiled_routing
cargo test -p rstim --test compiled_sampler_ir
```

Expected: packed reference tests still fail for direct compile and `Auto`
decision helpers not yet implemented; typed routing tests pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add rstim/src/compiled/path.rs rstim/src/compiled/circuit.rs rstim/src/data_path.rs rstim/tests/compiled_routing.rs rstim/tests/compiled_sampler.rs rstim/tests/compiled_sampler_ir.rs rstim/tests/sample_correctness_contract.rs
git commit -m "feat: add packed reference routing decisions"
```

---

### Task 3: Wire Direct Compile, Auto Recovery, And Legacy Feedback/Sweep Sampling

**Files:**
- Modify: `rstim/src/executor.rs`
- Modify: `rstim/src/sampler.rs`
- Modify: `rstim/tests/packed_reference_routing.rs` if the test needs small API-name adjustments from Task 2.

**Interfaces:**
- Produces:
  - `CompiledMeasurementSampler::compile_with_decision`
  - `SampleBatchDecision`
  - `sample_batch_with_options_and_decision`
  - `sample_batch_with_options_sweep_bits_and_decision`
  - `Executor::run_with_sweep_bits`

- [ ] **Step 1: Add legacy runtime support for rec/sweep controlled pairs**

In `rstim/src/executor.rs`, add:

```rust
pub fn run_with_sweep_bits(
    &mut self,
    rng: &mut impl Rng,
    sweep_bits: Option<&[bool]>,
) -> Result<ExecOutput, String> {
    let (out, _) = self.run_internal_with_sweep_bits(rng, false, sweep_bits)?;
    Ok(out)
}
```

Refactor `run_internal` to delegate to `run_internal_with_sweep_bits`. Pass
`sweep_bits` through `execute_instrs` and `execute_op`.

Replace the `CX`/`CY`/`CZ` branch in `execute_op` with:

```rust
apply_runtime_controlled_pairs(name, targets, exec, sweep_bits)?;
```

Add `apply_runtime_controlled_pairs`:

```rust
fn apply_runtime_controlled_pairs(
    name: &str,
    targets: &[StimTarget],
    exec: &mut ExecutionState,
    sweep_bits: Option<&[bool]>,
) -> Result<(), String> {
    if targets.len() % 2 != 0 {
        return Err("odd number of targets".to_string());
    }
    let mut it = targets.iter();
    while let (Some(a), Some(b)) = (it.next(), it.next()) {
        match (a, b) {
            (StimTarget::Qubit(c), StimTarget::Qubit(t)) => {
                if !exec.lost[*c as usize] && !exec.lost[*t as usize] {
                    apply_controlled_pair_effect(name, &mut exec.state, *c as usize, *t as usize)?;
                }
            }
            (StimTarget::Rec(offset), StimTarget::Qubit(q)) => {
                if exec.recorder.rec(*offset).ok_or("rec out of range")? && !exec.lost[*q as usize] {
                    apply_single_qubit_feedback_effect(name, &mut exec.state, *q as usize)?;
                }
            }
            (StimTarget::Sweep(k), StimTarget::Qubit(q)) => {
                if sweep_bits.and_then(|bits| bits.get(*k as usize)).copied().unwrap_or(false)
                    && !exec.lost[*q as usize]
                {
                    apply_single_qubit_feedback_effect(name, &mut exec.state, *q as usize)?;
                }
            }
            (StimTarget::Sweep(_), _) | (_, StimTarget::Sweep(_)) => {
                return Err("unsupported sweep target placement".to_string());
            }
            _ => return Err("expected qubit target in pair".to_string()),
        }
    }
    Ok(())
}
```

`apply_controlled_pair_effect` maps `CX`/`CNOT`/`ZCX` to `state.cx`,
`CY`/`ZCY` to `state.cy`, and `CZ`/`ZCZ` to `state.cz`.
`apply_single_qubit_feedback_effect` maps the same families to `x_gate`,
`y_gate`, and `z_gate` on the target.

Also update `apply_reference_controlled_pairs` to handle
`(StimTarget::Rec(offset), StimTarget::Qubit(q))` by reading from the existing
`measurements` vector. Change its signature to accept `measurements: &[bool]`
and pass it from `ref_sample_op`.

- [ ] **Step 2: Add direct compile typed API**

In `rstim/src/sampler.rs`, import `ReferenceSampleDecision`,
`build_reference_sample_with_decision`, and `SamplingFallbackReason`.

Add to `impl CompiledMeasurementSampler`:

```rust
#[doc(hidden)]
pub fn compile_with_decision(
    instrs: &[StimInstr],
    reference_mode: ReferenceSampleMode,
) -> Result<Self, SamplingFallbackReason> {
    let compiled = compile_circuit(instrs).map_err(|err| {
        SamplingFallbackReason::UnsupportedOperation(format!("compile error: {err}"))
    })?;
    match choose_sampler_path(&compiled) {
        CompiledPathDecision::FastPath => {}
        CompiledPathDecision::Fallback(reason) => return Err(reason),
    }

    let reference_sample = match reference_mode {
        ReferenceSampleMode::SimulateNoiseless => {
            let result = build_reference_sample_with_decision(instrs).map_err(|err| {
                SamplingFallbackReason::UnsupportedOperation(format!("reference error: {err}"))
            })?;
            match result.decision {
                ReferenceSampleDecision::PackedInverse => result.bits,
                ReferenceSampleDecision::LegacyFallback(reason) => return Err(reason),
            }
        }
        ReferenceSampleMode::AssumeAllZero => vec![false; crate::stats::num_measurements(instrs)],
    };

    Ok(Self {
        compiled,
        reference_sample,
        diagnostics: CompiledMeasurementSamplerDiagnostics {
            compiled_ir_builds: 1,
            reference_builds: 1,
            sample_calls: 0,
        },
    })
}
```

Change existing `compile` to:

```rust
pub fn compile(
    instrs: &[StimInstr],
    reference_mode: ReferenceSampleMode,
) -> Result<Self, String> {
    Self::compile_with_decision(instrs, reference_mode).map_err(|reason| reason.to_string())
}
```

- [ ] **Step 3: Add sample decision helpers**

In `rstim/src/sampler.rs`, add:

```rust
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SampleBatchDecision {
    PackedInverse,
    Interpreted,
    InterpretedLegacy(SamplingFallbackReason),
}
```

Add:

```rust
#[doc(hidden)]
pub fn sample_batch_with_options_and_decision(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
    options: SampleOptions,
) -> Result<(BatchOutput, SampleBatchDecision), String> {
    sample_batch_with_options_sweep_bits_and_decision(instrs, n_shots, rng, options, None)
}

#[doc(hidden)]
pub fn sample_batch_with_options_sweep_bits_and_decision(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
    options: SampleOptions,
    sweep_bits: Option<&[bool]>,
) -> Result<(BatchOutput, SampleBatchDecision), String> {
    match options.backend {
        SamplingBackend::Interpreted => {
            let out = sample_batch_interpreted_with_sweep_bits(instrs, n_shots, rng, options, sweep_bits)?;
            Ok((out, SampleBatchDecision::Interpreted))
        }
        SamplingBackend::Compiled => {
            let mut sampler = CompiledMeasurementSampler::compile_with_decision(
                instrs,
                options.reference_sample_mode,
            )
            .map_err(|reason| reason.to_string())?;
            let out = sampler.sample(n_shots, rng, options.output_mode)?;
            Ok((out, SampleBatchDecision::PackedInverse))
        }
        SamplingBackend::Auto => {
            match CompiledMeasurementSampler::compile_with_decision(instrs, options.reference_sample_mode) {
                Ok(mut sampler) => {
                    let out = sampler.sample(n_shots, rng, options.output_mode)?;
                    Ok((out, SampleBatchDecision::PackedInverse))
                }
                Err(reason) => {
                    let out = sample_batch_interpreted_with_sweep_bits(
                        instrs,
                        n_shots,
                        rng,
                        options,
                        sweep_bits,
                    )?;
                    Ok((out, SampleBatchDecision::InterpretedLegacy(reason)))
                }
            }
        }
    }
}
```

Change existing `sample_batch_with_options` to call the decision helper and
return only `.0`.

- [ ] **Step 4: Make interpreted fallback preserve feedback and sweep samples**

Refactor `sample_batch_interpreted` into:

```rust
fn sample_batch_interpreted_with_sweep_bits(
    instrs: &[StimInstr],
    n_shots: usize,
    rng: &mut impl Rng,
    options: SampleOptions,
    sweep_bits: Option<&[bool]>,
) -> Result<BatchOutput, String>
```

Use executor fallback when `sweep_bits.is_some()` or
`uses_executor_sampling_fallback(instrs)` is true. Extend
`uses_executor_sampling_fallback` so it returns true for loss, loss-visible
measurement/reset, `MPP`, measurement-record feedback, and sweep-dependent
operations. Leave ordinary `CZ` unsupported for packed but handled by the
existing frame simulator interpreted path.

Refactor `sample_batch_with_executor` into a sweep-aware helper that:

- builds `ref_sample` with `build_reference_sample_with_sweep_bits_and_decision`
  for `SimulateNoiseless` and repeated false bits for `AssumeAllZero`;
- calls `Executor::run_with_sweep_bits(rng, sweep_bits)` for each shot;
- for full output mode, passes a repeated static sweep `BitTable` to
  `measurements_to_detections_with_options` when `sweep_bits` is present.

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p rstim --test packed_reference_routing -- --nocapture
cargo test -p rstim --test reusable_compiled_measurement_sampler -- --nocapture
cargo test -p rstim --test compiled_sampler -- --nocapture
```

Expected: all pass, and packed reference test output includes
`PASS packed reference routing`.

- [ ] **Step 6: Commit**

Run:

```bash
git add rstim/src/executor.rs rstim/src/sampler.rs rstim/tests/packed_reference_routing.rs
git commit -m "feat: route references through packed inverse"
```

---

### Task 4: Final Verification And Cleanup

**Files:**
- All files modified by Tasks 1-3.

**Interfaces:**
- Consumes completed routing implementation.
- Produces a verified branch ready to push and open as a PR.

- [ ] **Step 1: Run focused issue verification**

Run:

```bash
cargo test -p rstim --test packed_reference_routing -- --nocapture
```

Expected: PASS and output contains `PASS packed reference routing`.

- [ ] **Step 2: Build release CLI**

Run:

```bash
cargo build --release -p rstim --bin rstim
```

Expected: PASS with exit code 0.

- [ ] **Step 3: Verify distribution case catalog hash**

Run:

```bash
test "$(shasum -a 256 benchmarks/rstim_vs_stim_simulator/distribution_cases.toml | awk '{print $1}')" = \
  6f28ad3cd13f4464c59548eef5cc135ad68c439ba01292c7132562f748970432
```

Expected: PASS with exit code 0.

- [ ] **Step 4: Run distribution verifier**

Run:

```bash
python3 -m benchmarks.rstim_vs_stim_simulator.verify_distributions \
  --cases benchmarks/rstim_vs_stim_simulator/distribution_cases.toml \
  --rstim target/release/rstim --shots 10000 --seeds 7 \
  --out /tmp/rstim-packed-reference-distributions.json
```

Expected: PASS and stdout contains `PASS distribution correctness`.

- [ ] **Step 5: Run final cargo suite**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 6: Review diff hygiene**

Run:

```bash
git diff --check master..HEAD
git status --short
```

Expected: no whitespace errors and no uncommitted changes.

- [ ] **Step 7: Commit verification-only cleanup if needed**

If verification required code or test cleanup, commit it with:

```bash
git add rstim/src/compiled/path.rs rstim/src/compiled/circuit.rs rstim/src/data_path.rs rstim/src/executor.rs rstim/src/sampler.rs rstim/tests/packed_reference_routing.rs rstim/tests/compiled_routing.rs rstim/tests/compiled_sampler.rs rstim/tests/compiled_sampler_ir.rs rstim/tests/sample_correctness_contract.rs
git commit -m "fix: complete packed reference routing"
```

Expected: no commit is created if no cleanup was needed.

## Self-Review

- Every issue-required fallback reason is covered at reference, direct compile, and top-level `Auto` layers.
- Canonical fixture and nested repeat positive packed cases are covered.
- The plan keeps compatibility wrappers and adds typed hidden helpers instead of removing existing APIs.
- Verification includes the issue command, release build, hash guard, distribution verifier, and `cargo test`.
- No placeholders remain.
