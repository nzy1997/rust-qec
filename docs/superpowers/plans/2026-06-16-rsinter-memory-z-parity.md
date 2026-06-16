# Rsinter Memory-Z Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add explicit `surface_rotated_memory_z` support to `rsinter` and add focused parity coverage for issue #65.

**Architecture:** Keep the existing `rsinter` benchmark pipeline intact. Extend only the surface input selector so memory-X remains the default and memory-Z dispatches to `rstim::codegen::surface_code::rotated_memory_z`; add regression tests that prove the selected workload, noise placement, DEM parity, and tiny `rmatching` smoke path.

**Tech Stack:** Rust workspace, `cargo test`, `rstim` surface-code codegen, `rsinter` benchmark registry and runners, Python Stim checks already used by `rstim/tests/cross_validate_dem.rs`.

---

## File Structure

- Modify `rsinter/src/bench/registry.rs`: accept `surface_rotated_memory_z` and preserve the concrete surface input type on `BenchCasePoint`.
- Modify `rsinter/src/bench/circuit_source.rs`: dispatch `surface_rotated_memory_x` to `rotated_memory_x` and `surface_rotated_memory_z` to `rotated_memory_z`; record the actual `input_type` in result params.
- Modify `rsinter/tests/bench_registry.rs`: add registry tests for memory-Z expansion and the existing memory-X default.
- Create `rsinter/tests/bench_circuit_source.rs`: test circuit-source dispatch and result metadata.
- Modify `rsinter/tests/bench_run.rs`: add a tiny memory-Z `rmatching` benchmark smoke test.
- Modify `rstim/tests/stim_codegen.rs`: add one-channel noise-placement regression tests for memory-Z.
- Modify `rstim/tests/cross_validate_dem.rs`: add issue-shaped memory-Z count and decomposed DEM parity checks against Stim.
- No production changes should be made to `rmatching` unless the new parity tests expose a decoder defect.

---

### Task 1: Registry Input Type Support

**Files:**
- Modify: `rsinter/tests/bench_registry.rs`
- Modify: `rsinter/src/bench/registry.rs`

- [ ] **Step 1: Write the failing registry test**

Add this test near `expand_runner_points_defaults_to_legacy_surface_input` in `rsinter/tests/bench_registry.rs`:

```rust
#[test]
fn expand_runner_points_accepts_rotated_memory_z_input_type() {
    let mut params = valid_runner_params();
    params.insert(
        "input_type".into(),
        toml::Value::String("surface_rotated_memory_z".into()),
    );

    let points = expand_runner_points(&params).unwrap();

    assert_eq!(points.len(), 1);
    assert_eq!(points[0].input_type, "surface_rotated_memory_z");
    assert_eq!(points[0].distance, Some(3));
    assert_eq!(points[0].rounds, 1);
    assert_eq!(points[0].p, 0.002);
    assert_eq!(points[0].basis, None);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p rsinter --test bench_registry expand_runner_points_accepts_rotated_memory_z_input_type
```

Expected: FAIL with an error containing `unknown input_type: surface_rotated_memory_z`.

- [ ] **Step 3: Implement registry support**

In `rsinter/src/bench/registry.rs`, change the surface input match and `expand_surface_points` signature.

Replace the current surface match with:

```rust
    match input_type.as_str() {
        "surface_rotated_memory_x" | "surface_rotated_memory_z" => expand_surface_points(
            &input_type,
            params,
            rounds,
            ps,
            max_shots,
            max_errors,
            max_wall_seconds,
            batch_size,
            decoder_params,
        ),
        "css" => expand_css_points(
            params,
            rounds,
            ps,
            max_shots,
            max_errors,
            max_wall_seconds,
            batch_size,
            decoder_params,
        ),
        other => Err(format!("unknown input_type: {other}")),
    }
```

Change the function signature from:

```rust
fn expand_surface_points(
    params: &BTreeMap<String, Value>,
```

to:

```rust
fn expand_surface_points(
    input_type: &str,
    params: &BTreeMap<String, Value>,
```

Inside the `BenchCasePoint` literal in `expand_surface_points`, replace:

```rust
                    input_type: "surface_rotated_memory_x".into(),
```

with:

```rust
                    input_type: input_type.to_string(),
```

- [ ] **Step 4: Run registry tests**

Run:

```bash
cargo test -p rsinter --test bench_registry
```

Expected: PASS.

- [ ] **Step 5: Commit registry support**

Run:

```bash
git add rsinter/src/bench/registry.rs rsinter/tests/bench_registry.rs
git commit -m "feat: accept rsinter memory-z surface input"
```

---

### Task 2: Circuit Source Dispatch

**Files:**
- Create: `rsinter/tests/bench_circuit_source.rs`
- Modify: `rsinter/src/bench/circuit_source.rs`

- [ ] **Step 1: Write the failing dispatch test**

Create `rsinter/tests/bench_circuit_source.rs` with:

```rust
use std::collections::BTreeMap;
use std::path::Path;

use rsinter::bench::circuit_source::build_circuit_for_point;
use rsinter::bench::registry::BenchCasePoint;
use rstim::ir::StimInstr;

fn surface_point(input_type: &str) -> BenchCasePoint {
    BenchCasePoint {
        input_type: input_type.into(),
        code_id: None,
        distance: Some(3),
        rounds: 3,
        p: 0.002,
        basis: None,
        schedule: None,
        hx_path: None,
        hz_path: None,
        observables_path: None,
        max_shots: 0,
        max_errors: 2,
        max_wall_seconds: None,
        batch_size: 4,
        decoder_params: BTreeMap::new(),
    }
}

fn has_op(circuit: &[StimInstr], op_name: &str) -> bool {
    circuit
        .iter()
        .any(|instr| matches!(instr, StimInstr::Op { name, .. } if name == op_name))
}

#[test]
fn build_circuit_for_point_dispatches_rotated_memory_z() {
    let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let memory_x = build_circuit_for_point(&surface_point("surface_rotated_memory_x"), spec_dir)
        .unwrap();
    let memory_z = build_circuit_for_point(&surface_point("surface_rotated_memory_z"), spec_dir)
        .unwrap();

    assert_eq!(
        memory_x.params["input_type"],
        serde_json::json!("surface_rotated_memory_x")
    );
    assert_eq!(
        memory_z.params["input_type"],
        serde_json::json!("surface_rotated_memory_z")
    );

    assert!(has_op(&memory_x.circuit, "RX"));
    assert!(has_op(&memory_x.circuit, "MX"));
    assert!(has_op(&memory_z.circuit, "R"));
    assert!(has_op(&memory_z.circuit, "M"));
    assert!(!has_op(&memory_z.circuit, "MX"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p rsinter --test bench_circuit_source build_circuit_for_point_dispatches_rotated_memory_z
```

Expected: FAIL with an error containing `unknown input_type: surface_rotated_memory_z`.

- [ ] **Step 3: Implement circuit dispatch**

In `rsinter/src/bench/circuit_source.rs`, replace:

```rust
use rstim::codegen::surface_code::rotated_memory_x;
```

with:

```rust
use rstim::codegen::surface_code::{rotated_memory_x, rotated_memory_z};
```

In `build_circuit_for_point`, replace:

```rust
        "surface_rotated_memory_x" => build_surface(point),
```

with:

```rust
        "surface_rotated_memory_x" | "surface_rotated_memory_z" => build_surface(point),
```

In `build_surface`, replace:

```rust
    let circuit = rotated_memory_x(distance, point.rounds, point.p);
    let mut params = ParamMap::from_pairs([
        ("input_type", serde_json::json!("surface_rotated_memory_x")),
```

with:

```rust
    let circuit = match point.input_type.as_str() {
        "surface_rotated_memory_x" => rotated_memory_x(distance, point.rounds, point.p),
        "surface_rotated_memory_z" => rotated_memory_z(distance, point.rounds, point.p),
        other => return Err(format!("unknown input_type: {other}")),
    };
    let mut params = ParamMap::from_pairs([
        ("input_type", serde_json::json!(point.input_type.as_str())),
```

- [ ] **Step 4: Run circuit-source tests**

Run:

```bash
cargo test -p rsinter --test bench_circuit_source
```

Expected: PASS.

- [ ] **Step 5: Run the registry and circuit-source tests together**

Run:

```bash
cargo test -p rsinter --test bench_registry --test bench_circuit_source
```

Expected: PASS.

- [ ] **Step 6: Commit circuit dispatch**

Run:

```bash
git add rsinter/src/bench/circuit_source.rs rsinter/tests/bench_circuit_source.rs
git commit -m "feat: build rsinter memory-z surface circuits"
```

---

### Task 3: Rsinter End-To-End Memory-Z Smoke

**Files:**
- Modify: `rsinter/tests/bench_run.rs`

- [ ] **Step 1: Add the smoke test**

Add this test after `rust_benchmark_run_writes_manifest_and_results_jsonl` in `rsinter/tests/bench_run.rs`:

```rust
#[test]
fn rust_benchmark_run_supports_surface_rotated_memory_z() {
    let spec_text = r#"
name = "surface_decoder_memory_z"
version = 1
mode = "independent"

[[runner]]
name = "rmatching_memory_z"
language = "rust"
impl_key = "rmatching"

[runner.params]
input_type = "surface_rotated_memory_z"
distance = [3]
rounds = [9]
p = [0.008]
max_shots = 8
max_errors = 8
batch_size = 4

[plot]
title = "Surface Decoder Memory-Z"

[plot.x]
field = "params.p"
scale = "log"
label = "Physical Error Rate"

[plot.series]
group_by = ["runner"]
label_template = "{runner}"

[[plot.panel]]
metric = "metrics.logical_error_rate"
scale = "log"
label = "Logical Error Rate"
"#;

    let spec: BenchmarkSpec = toml::from_str(spec_text).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = build_default_rust_runner_registry();

    let artifact_root = run_rust_benchmark(
        &spec,
        "rust",
        dir.path(),
        &registry,
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap();
    let data = fs::read(
        artifact_root
            .join("rmatching_memory_z")
            .join("test-run")
            .join("results.jsonl"),
    )
    .unwrap();
    let rows = read_results_jsonl(&data[..]).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].params["input_type"],
        serde_json::json!("surface_rotated_memory_z")
    );
    assert_eq!(rows[0].params["distance"], serde_json::json!(3));
    assert_eq!(rows[0].params["rounds"], serde_json::json!(9));
    assert_eq!(rows[0].params["p"], serde_json::json!(0.008));
    assert_eq!(rows[0].case_summary["num_dets"], serde_json::json!(72));
    assert_eq!(rows[0].case_summary["num_obs"], serde_json::json!(1));
    assert_eq!(rows[0].status, "ok");
    assert_eq!(rows[0].error, None);
    assert_ne!(rows[0].failure_kind, FailureKind::SolverFailure);
    assert_ne!(rows[0].failure_kind, FailureKind::SamplerError);
}
```

- [ ] **Step 2: Run the smoke test**

Run:

```bash
cargo test -p rsinter --test bench_run rust_benchmark_run_supports_surface_rotated_memory_z
```

Expected: PASS after Tasks 1 and 2. If it fails with `num_dets` not equal to `72`, inspect whether `effective_num_detectors()` or the memory-Z generator changed before updating the assertion.

- [ ] **Step 3: Run rsinter benchmark tests**

Run:

```bash
cargo test -p rsinter --test bench_registry --test bench_circuit_source --test bench_run
```

Expected: PASS.

- [ ] **Step 4: Commit smoke coverage**

Run:

```bash
git add rsinter/tests/bench_run.rs
git commit -m "test: add rsinter memory-z benchmark smoke"
```

---

### Task 4: Memory-Z Noise Placement Coverage

**Files:**
- Modify: `rstim/tests/stim_codegen.rs`

- [ ] **Step 1: Add focused one-channel tests**

Add these tests after `surface_code_before_round_data_depolarization_does_not_extend_into_tail_measurement_step` in `rstim/tests/stim_codegen.rs`:

```rust
#[test]
fn surface_code_before_measure_flip_covers_ancilla_and_final_data_measurements() {
    let rounds = 3;
    let params = NoiseParams {
        before_round_data_depolarization: 0.0,
        after_clifford_depolarization: 0.0,
        before_measure_flip_probability: 0.001,
        after_reset_flip_probability: 0.0,
    };
    let circuit = rotated_memory_z_with_params(3, rounds, params);

    let x_error_targets = count_qubit_targets_named(&circuit, "X_ERROR");
    let ancilla_measure_targets = count_qubit_targets_named(&circuit, "MR");
    let final_data_measure_targets =
        count_qubit_targets_named(tail_after_last_tick(&circuit), "M");

    assert_eq!(
        x_error_targets,
        ancilla_measure_targets + final_data_measure_targets,
        "before_measure_flip_probability should apply before every ancilla MR and before final data M"
    );
}

#[test]
fn surface_code_after_reset_flip_covers_initial_resets_and_ancilla_mr_resets() {
    let rounds = 3;
    let params = NoiseParams {
        before_round_data_depolarization: 0.0,
        after_clifford_depolarization: 0.0,
        before_measure_flip_probability: 0.0,
        after_reset_flip_probability: 0.001,
    };
    let circuit = rotated_memory_z_with_params(3, rounds, params);

    let x_error_targets = count_qubit_targets_named(&circuit, "X_ERROR");
    let ancilla_mr_targets = count_qubit_targets_named(&circuit, "MR");
    let final_data_measure_targets =
        count_qubit_targets_named(tail_after_last_tick(&circuit), "M");
    let ancilla_count = ancilla_mr_targets / rounds;

    assert_eq!(
        x_error_targets,
        final_data_measure_targets + ancilla_count + ancilla_mr_targets,
        "after_reset_flip_probability should apply after initial data reset, initial ancilla reset, and each ancilla MR reset"
    );
}

#[test]
fn issue_memory_z_uniform_noise_contains_all_four_noise_channels() {
    let circuit = rotated_memory_z_with_params(3, 9, NoiseParams::uniform(0.008));
    let text = circuit_to_string(&circuit);

    assert!(text.contains("DEPOLARIZE1(0.008)"), "missing before-round or after-H depolarization: {text}");
    assert!(text.contains("DEPOLARIZE2(0.008)"), "missing after-CX depolarization: {text}");
    assert!(text.contains("X_ERROR(0.008)"), "missing reset or measurement flip channel: {text}");
    assert!(text.contains("MR"), "missing ancilla measurement/reset operations: {text}");
    assert!(text.contains("OBSERVABLE_INCLUDE(0)"), "missing logical observable: {text}");
}
```

- [ ] **Step 2: Run the noise tests**

Run:

```bash
cargo test -p rstim --test stim_codegen surface_code_before_measure_flip_covers_ancilla_and_final_data_measurements
cargo test -p rstim --test stim_codegen surface_code_after_reset_flip_covers_initial_resets_and_ancilla_mr_resets
cargo test -p rstim --test stim_codegen issue_memory_z_uniform_noise_contains_all_four_noise_channels
```

Expected: PASS. These tests pin existing generator semantics; failure means issue #65 has a codegen/noise-placement root cause and the implementation should fix `rstim/src/codegen/surface_code.rs` before continuing.

- [ ] **Step 3: Run the full codegen test file**

Run:

```bash
cargo test -p rstim --test stim_codegen
```

Expected: PASS.

- [ ] **Step 4: Commit noise coverage**

Run:

```bash
git add rstim/tests/stim_codegen.rs
git commit -m "test: pin memory-z surface noise placement"
```

---

### Task 5: Stim Count And DEM Parity

**Files:**
- Modify: `rstim/tests/cross_validate_dem.rs`

- [ ] **Step 1: Update imports**

In `rstim/tests/cross_validate_dem.rs`, replace the current `rstim::codegen` import with:

```rust
use rstim::codegen::{
    color_code::memory_xyz,
    repetition_code_memory,
    surface_code::{rotated_memory_x, rotated_memory_z},
};
```

- [ ] **Step 2: Add the Python Stim count helper**

Add this helper after `stim_python_generated_surface_code_circuit_text`:

```rust
fn stim_python_generated_memory_z_counts(distance: usize, rounds: usize, noise: f64) -> (usize, usize) {
    let script = format!(
        r#"
import stim

circuit = stim.Circuit.generated(
    'surface_code:rotated_memory_z',
    rounds={rounds},
    distance={distance},
    after_clifford_depolarization={noise},
    after_reset_flip_probability={noise},
    before_measure_flip_probability={noise},
    before_round_data_depolarization={noise},
)
print(circuit.num_detectors)
print(circuit.num_observables)
"#
    );
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .output()
        .expect("failed to run python3");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "python3 stim count generation failed: {stderr}"
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut lines = stdout.lines();
    let num_detectors = lines
        .next()
        .expect("missing detector count")
        .parse::<usize>()
        .expect("detector count should be numeric");
    let num_observables = lines
        .next()
        .expect("missing observable count")
        .parse::<usize>()
        .expect("observable count should be numeric");
    (num_detectors, num_observables)
}
```

- [ ] **Step 3: Add issue-shaped memory-Z parity tests**

Add these tests before `cross_validate_decomposed_color_code_failure_mode`:

```rust
#[test]
fn issue_memory_z_counts_match_python_stim() {
    let _guard = lock_stim_env();
    let circuit = rotated_memory_z(3, 9, 0.008);
    let (stim_num_detectors, stim_num_observables) =
        stim_python_generated_memory_z_counts(3, 9, 0.008);

    assert_eq!(rstim::stats::num_detectors(&circuit), stim_num_detectors);
    assert_eq!(rstim::stats::num_observables(&circuit), stim_num_observables);
}

#[test]
fn cross_validate_decomposed_memory_z_issue_dem() {
    let _guard = lock_stim_env();
    let circuit = rotated_memory_z(3, 9, 0.008);
    let circuit_text = circuit_to_string(&circuit);
    let stim_dem_text = stim_analyze_errors_flags(&circuit_text, &["--decompose_errors"]);
    let rstim_dem_text = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit)
        .unwrap()
        .to_string();

    assert_all_graphlike_dem_text(&stim_dem_text);
    assert_all_graphlike_dem_text(&rstim_dem_text);
    assert_semantic_dem_parity(&stim_dem_text, &rstim_dem_text);
}
```

- [ ] **Step 4: Run the parity tests**

Run:

```bash
cargo test -p rstim --test cross_validate_dem issue_memory_z_counts_match_python_stim
cargo test -p rstim --test cross_validate_dem cross_validate_decomposed_memory_z_issue_dem
```

Expected: PASS when Python Stim and the Stim CLI are available. If the command fails because `stim` is missing, rerun the same command in the environment used by the existing `cross_validate_dem` tests before changing code.

- [ ] **Step 5: Run the full cross-validation file**

Run:

```bash
cargo test -p rstim --test cross_validate_dem
```

Expected: PASS.

- [ ] **Step 6: Commit parity coverage**

Run:

```bash
git add rstim/tests/cross_validate_dem.rs
git commit -m "test: cross-check memory-z issue circuit against stim"
```

---

### Task 6: Final Verification

**Files:**
- No new source files unless a previous task exposed a defect that required one.

- [ ] **Step 1: Run focused test suite**

Run:

```bash
cargo test -p rsinter --test bench_registry --test bench_circuit_source --test bench_run
```

Expected: PASS.

- [ ] **Step 2: Run rstim parity and codegen tests**

Run:

```bash
cargo test -p rstim --test stim_codegen --test cross_validate_dem
```

Expected: PASS.

- [ ] **Step 3: Run workspace check**

Run:

```bash
cargo test -p rsinter -p rstim
```

Expected: PASS.

- [ ] **Step 4: Inspect git status**

Run:

```bash
git status --short
```

Expected: only intentional source and test files are modified since the last task commit.

- [ ] **Step 5: Commit any verification-only adjustments**

If Step 4 shows tracked edits that came from fixing test failures during Task 6, commit them:

```bash
git add rsinter/src/bench/registry.rs rsinter/src/bench/circuit_source.rs rsinter/tests/bench_registry.rs rsinter/tests/bench_circuit_source.rs rsinter/tests/bench_run.rs rstim/tests/stim_codegen.rs rstim/tests/cross_validate_dem.rs
git commit -m "test: verify rsinter memory-z parity path"
```

Expected: either a small verification commit is created, or there are no remaining changes to commit.

---

## Self-Review Checklist

- Spec coverage:
  - Explicit `surface_rotated_memory_z` support is covered by Tasks 1 and 2.
  - Backwards-compatible memory-X default remains covered by the existing registry default test and Task 1.
  - Result metadata is covered by Tasks 2 and 3.
  - Four-channel noise placement is covered by Task 4.
  - Stim count and DEM parity are covered by Task 5.
  - Tiny `rmatching` end-to-end smoke is covered by Task 3.
- Placeholder scan: this plan contains no deferred implementation slots.
- Type consistency:
  - `BenchCasePoint.input_type` remains a `String`.
  - `build_circuit_for_point` continues returning `Result<BuiltCircuit, String>`.
  - Result params and case summary use `serde_json::Value` assertions.
  - The test commands target existing package and test names.
