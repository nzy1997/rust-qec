# Issue 283 BB P-Point Runner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a BB p-point runner that builds BB setup once per point and exposes setup/sample/decode accounting that proves reuse across trials.

**Architecture:** Add an explicit `BbPPointConfig`/`BbPPointResult` surface in `rsinter::bb_circuit_memory`, then refactor the current simulation path through a private reusable setup object. The setup object owns the code, syndrome cycle, effective models, and Z/X decoders; the trial loop borrows that setup and only samples/decodes. Existing CLI and comparison-export functions keep their public behavior by delegating to the p-point runner.

**Tech Stack:** Rust 2024 Cargo workspace; `rsinter` library and integration tests; existing `rbposd::BpOsdDecoder`; `cargo test`.

## Global Constraints

- Existing CLI flags and stdout behavior for `rsinter bb-circuit-bposd-memory` remain compatible.
- Existing `SimulationConfig` callers remain compatible.
- Construction counters count setup phases once per successful p-point: code build, syndrome-cycle build, effective-model build, decoder-set build.
- `sample_count` is a trial-loop counter and must equal `num_trials` for successful p-point results.
- `decode_call_count == z_decode_call_count + x_decode_call_count`.
- The negative control must fail with an error containing `setup/model rebuild count mismatch`.
- Full 50k-trial sweeps, plot/report generation, and OSD semantic changes stay out of scope.

---

## File Structure

- Modify `rsinter/src/bb_circuit_memory.rs`: add p-point config/result types, additive profile counters, p-point validation, private reusable setup, and route existing runners through it.
- Modify `rsinter/tests/bb_circuit_memory.rs`: add the issue-named positive and negative integration tests, and update imports.
- Add `docs/superpowers/specs/2026-06-26-issue-283-bb-p-point-runner-design.md`: already committed brainstorming design.
- Add `docs/superpowers/plans/2026-06-26-issue-283-bb-p-point-runner.md`: this implementation plan.

### Task 1: Add Failing P-Point Integration Tests

**Files:**
- Modify: `rsinter/tests/bb_circuit_memory.rs`

**Interfaces:**
- Consumes: existing `SimulationConfig` and `run_simulation_for_code`.
- Produces: required tests that later tasks satisfy with `run_bb_p_point`, `BbPPointConfig`, and `validate_bb_p_point_result`.

- [ ] **Step 1: Update the test import block**

Change the `use rsinter::bb_circuit_memory` block at the top of `rsinter/tests/bb_circuit_memory.rs` to include the new names:

```rust
use rsinter::bb_circuit_memory::{
    BbPPointConfig, OperationKind, SimulationConfig, bb_circuit_bposd_result_row, build_code,
    build_effective_models, build_syndrome_cycle, build_upstream_code,
    export_comparison_case_for_code, run_bb_p_point, run_simulation, run_simulation_for_code,
    sample_seeded_trial, validate_bb_p_point_result, validate_bposd_profile_result_row,
};
```

- [ ] **Step 2: Add the positive issue test**

Insert this test after `effective_models_only_use_basis_specific_logical_rows`:

```rust
#[test]
fn bb_p_point_runner_reuses_setup_across_trials() {
    let result = run_bb_p_point(BbPPointConfig::from_simulation_config(
        "bb72",
        SimulationConfig {
            physical_error_rate: 0.0,
            num_cycles: 1,
            num_trials: 8,
            seed: Some(17),
            max_bp_iterations: 10,
            osd_order: 0,
        },
    ))
    .unwrap();

    assert_eq!(result.code_id, "bb72");
    assert_eq!(result.result.num_trials, 8);
    assert_eq!(result.result.num_failed_trials, 0);

    let profile = &result.result.profile;
    println!("{}", serde_json::to_string_pretty(profile).unwrap());
    assert_eq!(profile.code_build_count, 1);
    assert_eq!(profile.syndrome_cycle_build_count, 1);
    assert_eq!(profile.effective_model_build_count, 1);
    assert_eq!(profile.decoder_build_count, 1);
    assert_eq!(profile.sample_count, 8);
    assert_eq!(profile.z_decode_call_count, 8);
    assert_eq!(profile.x_decode_call_count, 8);
    assert_eq!(
        profile.decode_call_count,
        profile.z_decode_call_count + profile.x_decode_call_count
    );

    validate_bb_p_point_result(&result).unwrap();
}
```

- [ ] **Step 3: Add the negative control**

Insert this test immediately after the positive issue test:

```rust
#[test]
fn bb_p_point_runner_rejects_per_trial_setup_rebuild() {
    let mut result = run_bb_p_point(BbPPointConfig::from_simulation_config(
        "bb72",
        SimulationConfig {
            physical_error_rate: 0.0,
            num_cycles: 1,
            num_trials: 8,
            seed: Some(17),
            max_bp_iterations: 10,
            osd_order: 0,
        },
    ))
    .unwrap();

    result.result.profile.code_build_count = 8;
    result.result.profile.effective_model_build_count = 8;
    result.result.profile.decoder_build_count = 8;

    let error = validate_bb_p_point_result(&result).unwrap_err();
    assert!(
        error.contains("setup/model rebuild count mismatch"),
        "{error}"
    );
}
```

- [ ] **Step 4: Run the positive test and verify it fails**

Run:

```bash
cargo test -p rsinter bb_p_point_runner_reuses_setup_across_trials -- --nocapture
```

Expected: FAIL because `BbPPointConfig`, `run_bb_p_point`, and `validate_bb_p_point_result` are not yet defined.

- [ ] **Step 5: Run the negative test and verify it fails**

Run:

```bash
cargo test -p rsinter bb_p_point_runner_rejects_per_trial_setup_rebuild -q
```

Expected: FAIL for the same missing p-point API names.

### Task 2: Add P-Point Types, Profile Counters, and Validation

**Files:**
- Modify: `rsinter/src/bb_circuit_memory.rs`

**Interfaces:**
- Consumes: `SimulationConfig`, `BbCircuitBposdProfile`, `SimulationResult`, `rbposd::OsdVariant`.
- Produces:
  - `pub struct BbPPointConfig`
  - `impl BbPPointConfig::from_simulation_config(code_id: impl Into<String>, simulation: SimulationConfig) -> Self`
  - `pub struct BbPPointResult`
  - additive `BbCircuitBposdProfile` fields
  - `pub fn validate_bb_p_point_result(result: &BbPPointResult) -> Result<(), String>`

- [ ] **Step 1: Add public p-point config and result structs**

Insert after the `SimulationConfig` `impl Default` block:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct BbPPointConfig {
    pub code_id: String,
    pub physical_error_rate: f64,
    pub num_cycles: usize,
    pub num_trials: usize,
    pub seed: Option<u64>,
    pub max_bp_iterations: usize,
    pub osd_order: usize,
    pub osd_variant: OsdVariant,
}

impl BbPPointConfig {
    pub fn from_simulation_config(
        code_id: impl Into<String>,
        config: SimulationConfig,
    ) -> Self {
        Self::from_simulation_config_with_osd_variant(code_id, config, OsdVariant::Osd0)
    }

    pub fn from_simulation_config_with_osd_variant(
        code_id: impl Into<String>,
        config: SimulationConfig,
        osd_variant: OsdVariant,
    ) -> Self {
        Self {
            code_id: code_id.into(),
            physical_error_rate: config.physical_error_rate,
            num_cycles: config.num_cycles,
            num_trials: config.num_trials,
            seed: config.seed,
            max_bp_iterations: config.max_bp_iterations,
            osd_order: config.osd_order,
            osd_variant,
        }
    }

    fn simulation_config(&self) -> SimulationConfig {
        SimulationConfig {
            physical_error_rate: self.physical_error_rate,
            num_cycles: self.num_cycles,
            num_trials: self.num_trials,
            seed: self.seed,
            max_bp_iterations: self.max_bp_iterations,
            osd_order: self.osd_order,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BbPPointResult {
    pub code_id: String,
    pub result: SimulationResult,
}
```

- [ ] **Step 2: Add the profile counter fields**

Extend `BbCircuitBposdProfile` with these fields after `decode_seconds`:

```rust
    pub code_build_count: usize,
    pub syndrome_cycle_build_count: usize,
    pub effective_model_build_count: usize,
    pub decoder_build_count: usize,
    pub sample_count: usize,
```

- [ ] **Step 3: Add p-point validation**

Insert after `impl BbCircuitBposdProfile`:

```rust
pub fn validate_bb_p_point_result(result: &BbPPointResult) -> Result<(), String> {
    let profile = &result.result.profile;
    if profile.code_build_count != 1
        || profile.syndrome_cycle_build_count != 1
        || profile.effective_model_build_count != 1
        || profile.decoder_build_count != 1
    {
        return Err(format!(
            "setup/model rebuild count mismatch: code_build_count={}, syndrome_cycle_build_count={}, effective_model_build_count={}, decoder_build_count={}, expected all counters to be 1 for one p-point",
            profile.code_build_count,
            profile.syndrome_cycle_build_count,
            profile.effective_model_build_count,
            profile.decoder_build_count
        ));
    }

    if profile.sample_count != result.result.num_trials {
        return Err(format!(
            "sample_count mismatch: sample_count={} num_trials={}",
            profile.sample_count, result.result.num_trials
        ));
    }

    if profile.decode_call_count != profile.z_decode_call_count + profile.x_decode_call_count {
        return Err(
            "decode_call_count must equal z_decode_call_count + x_decode_call_count".into(),
        );
    }

    Ok(())
}
```

- [ ] **Step 4: Add row metrics for the new counters**

In `bb_circuit_bposd_result_row`, add these metrics after `decode_seconds`:

```rust
            (
                "code_build_count",
                result.profile.code_build_count as f64,
            ),
            (
                "syndrome_cycle_build_count",
                result.profile.syndrome_cycle_build_count as f64,
            ),
            (
                "effective_model_build_count",
                result.profile.effective_model_build_count as f64,
            ),
            (
                "decoder_build_count",
                result.profile.decoder_build_count as f64,
            ),
            ("sample_count", result.profile.sample_count as f64),
```

In `validate_bposd_profile_result_row`, add the same metric names to both
`required_metric_keys` and `counter_metric_keys`.

- [ ] **Step 5: Run the focused tests**

Run:

```bash
cargo test -p rsinter bb_p_point_runner_reuses_setup_across_trials -- --nocapture
```

Expected: FAIL because `run_bb_p_point` is still missing or returns zero setup counters until Task 3.

### Task 3: Refactor the Runner Through One Reusable P-Point Setup

**Files:**
- Modify: `rsinter/src/bb_circuit_memory.rs`

**Interfaces:**
- Consumes: Task 2 p-point types and profile counters.
- Produces:
  - `pub fn run_bb_p_point(config: BbPPointConfig) -> Result<BbPPointResult, String>`
  - existing `run_simulation_for_code*` and export functions delegate to the p-point path.
  - `SimulationCaseRun` still returns `SimulationResult`, `EffectiveModels`, and optional export trials.

- [ ] **Step 1: Add a private reusable setup struct**

Insert after `SimulationCaseRun`:

```rust
struct BbPPointSetup {
    code: BbCode,
    cycle: SyndromeCycle,
    models: EffectiveModels,
    z_decoder: BpOsdDecoder,
    x_decoder: BpOsdDecoder,
    setup_profile: BbCircuitBposdProfile,
}
```

- [ ] **Step 2: Add the setup builder**

Insert before `run_simulation_case_for_code_with_osd_variant`:

```rust
fn build_bb_p_point_setup(config: &BbPPointConfig) -> Result<BbPPointSetup, String> {
    let simulation_config = config.simulation_config();
    validate_simulation_config(&simulation_config)?;

    let setup_started = Instant::now();
    let mut setup_profile = BbCircuitBposdProfile::default();

    let code = build_code(&config.code_id)?;
    setup_profile.code_build_count = 1;

    let cycle = build_syndrome_cycle(&code);
    setup_profile.syndrome_cycle_build_count = 1;

    let models = build_effective_models(&code, &cycle, &simulation_config)?;
    setup_profile.effective_model_build_count = 1;

    if models.z_faults.channel_probs.is_empty() || models.x_faults.channel_probs.is_empty() {
        return Err("effective decoder models must contain at least one probability column".into());
    }

    let decoder_config = DecoderConfig {
        max_bp_iterations: config.max_bp_iterations,
        osd_variant: config.osd_variant,
        osd_order: config.osd_order,
        ..DecoderConfig::default()
    };

    let z_decoder = BpOsdDecoder::new(
        models.z_faults.decoder.clone(),
        ChannelModel::BitFlipProbabilities(models.z_faults.channel_probs.clone()),
        decoder_config,
    )
    .map_err(|error| format!("failed to compile Z-fault rbposd decoder: {error}"))?;

    let x_decoder = BpOsdDecoder::new(
        models.x_faults.decoder.clone(),
        ChannelModel::BitFlipProbabilities(models.x_faults.channel_probs.clone()),
        decoder_config,
    )
    .map_err(|error| format!("failed to compile X-fault rbposd decoder: {error}"))?;

    setup_profile.decoder_build_count = 1;
    setup_profile.setup_seconds = setup_started.elapsed().as_secs_f64();

    Ok(BbPPointSetup {
        code,
        cycle,
        models,
        z_decoder,
        x_decoder,
        setup_profile,
    })
}
```

- [ ] **Step 3: Add the public p-point runner**

Insert before `run_simulation`:

```rust
pub fn run_bb_p_point(config: BbPPointConfig) -> Result<BbPPointResult, String> {
    let code_id = config.code_id.clone();
    let run = run_bb_p_point_case(config, false)?;
    Ok(BbPPointResult {
        code_id,
        result: run.result,
    })
}
```

- [ ] **Step 4: Replace the old case runner body**

Add a helper with the existing trial loop shape:

```rust
fn run_bb_p_point_case(
    config: BbPPointConfig,
    collect_trials: bool,
) -> Result<SimulationCaseRun, String> {
    let setup = build_bb_p_point_setup(&config)?;
    let mut rng = match config.seed {
        Some(seed) => StdRng::seed_from_u64(seed),
        None => StdRng::from_entropy(),
    };

    let mut profile = setup.setup_profile.clone();
    let mut num_failed_trials = 0usize;
    let mut trials = collect_trials.then(|| Vec::with_capacity(config.num_trials));
    for _ in 0..config.num_trials {
        let sample_started = Instant::now();
        let sample = simulate_trial(
            &setup.code,
            &setup.cycle,
            config.num_cycles,
            config.physical_error_rate,
            &mut rng,
        );
        profile.sample_seconds += sample_started.elapsed().as_secs_f64();
        profile.sample_count += 1;
        let mut trial_export = collect_trials.then(|| comparison_trial_export(&sample));

        let decode_started = Instant::now();
        let z_result = decode_logicals(&setup.z_decoder, &sample.z_syndrome)
            .map_err(|error| format!("failed to decode Z faults: {error}"))?;
        let z_decode_seconds = decode_started.elapsed().as_secs_f64();
        profile.decode_seconds += z_decode_seconds;
        profile.add_z_stats(&z_result.stats);
        let predicted_z =
            correction_to_logicals(&z_result.correction, &setup.models.z_faults, setup.code.k());
        if let Some(trial) = trial_export.as_mut() {
            trial.z_logical_prediction = Some(predicted_z.clone());
            trial.z_profile = Some(profile_from_decode_stats(
                ProfileReplayBasis::Z,
                z_decode_seconds,
                &z_result.stats,
            ));
        }
        if predicted_z != sample.z_logical {
            if let (Some(trials), Some(trial)) = (trials.as_mut(), trial_export.take()) {
                trials.push(trial);
            }
            num_failed_trials += 1;
            continue;
        }

        let decode_started = Instant::now();
        let x_result = decode_logicals(&setup.x_decoder, &sample.x_syndrome)
            .map_err(|error| format!("failed to decode X faults: {error}"))?;
        let x_decode_seconds = decode_started.elapsed().as_secs_f64();
        profile.decode_seconds += x_decode_seconds;
        profile.add_x_stats(&x_result.stats);
        let predicted_x =
            correction_to_logicals(&x_result.correction, &setup.models.x_faults, setup.code.k());
        if let Some(trial) = trial_export.as_mut() {
            trial.x_logical_prediction = Some(predicted_x.clone());
            trial.x_profile = Some(profile_from_decode_stats(
                ProfileReplayBasis::X,
                x_decode_seconds,
                &x_result.stats,
            ));
        }
        if predicted_x != sample.x_logical {
            num_failed_trials += 1;
        }
        if let (Some(trials), Some(trial)) = (trials.as_mut(), trial_export.take()) {
            trials.push(trial);
        }
    }

    Ok(SimulationCaseRun {
        result: SimulationResult {
            physical_error_rate: config.physical_error_rate,
            num_cycles: config.num_cycles,
            num_trials: config.num_trials,
            num_failed_trials,
            profile,
        },
        models: setup.models,
        trials,
    })
}
```

Then make `run_simulation_case_for_code_with_osd_variant` only convert to
`BbPPointConfig` and delegate:

```rust
fn run_simulation_case_for_code_with_osd_variant(
    code_id: &str,
    config: SimulationConfig,
    osd_variant: OsdVariant,
    collect_trials: bool,
) -> Result<SimulationCaseRun, String> {
    run_bb_p_point_case(
        BbPPointConfig::from_simulation_config_with_osd_variant(code_id, config, osd_variant),
        collect_trials,
    )
}
```

- [ ] **Step 5: Run the focused tests**

Run:

```bash
cargo test -p rsinter bb_p_point_runner_reuses_setup_across_trials -- --nocapture
cargo test -p rsinter bb_p_point_runner_rejects_per_trial_setup_rebuild -q
```

Expected: both tests PASS.

### Task 4: Update Existing Counter Tests and Run Verification

**Files:**
- Modify: `rsinter/tests/bb_circuit_memory.rs`
- Modify: `rsinter/src/bb_circuit_memory.rs` if compile/test feedback reveals missing metric list updates.

**Interfaces:**
- Consumes: Task 3 implementation.
- Produces: all existing BB profile validation tests passing with the new additive counters.

- [ ] **Step 1: Update existing profile counter assertions**

In `bb_circuit_bposd_timing_counters_partition_decode_work`, add assertions after the finite time checks:

```rust
    assert_eq!(profile.code_build_count, 1);
    assert_eq!(profile.syndrome_cycle_build_count, 1);
    assert_eq!(profile.effective_model_build_count, 1);
    assert_eq!(profile.decoder_build_count, 1);
    assert_eq!(profile.sample_count, 1);
```

Add the new metric names to the loop that checks `row.metrics.contains_key`:

```rust
        "code_build_count",
        "syndrome_cycle_build_count",
        "effective_model_build_count",
        "decoder_build_count",
        "sample_count",
```

- [ ] **Step 2: Extend the incomplete-row negative test**

In `bb_circuit_bposd_timing_counters_reject_incomplete_rows`, after the existing
missing `decode_call_count` assertion, add:

```rust
    let mut missing_setup = bb_circuit_bposd_result_row("bb90", &result);
    missing_setup.metrics.remove("effective_model_build_count");
    assert!(validate_bposd_profile_result_row(&missing_setup).is_err());
```

- [ ] **Step 3: Run issue verification**

Run:

```bash
cargo test -p rsinter bb_p_point_runner_reuses_setup_across_trials -- --nocapture
cargo test -p rsinter bb_p_point_runner_rejects_per_trial_setup_rebuild -q
```

Expected: both commands PASS.

- [ ] **Step 4: Run wider verification**

Run:

```bash
cargo test
```

Expected: PASS for the workspace.

- [ ] **Step 5: Commit implementation**

Run:

```bash
git add rsinter/src/bb_circuit_memory.rs rsinter/tests/bb_circuit_memory.rs docs/superpowers/plans/2026-06-26-issue-283-bb-p-point-runner.md
git commit -m "feat: add bb p-point runner"
```
