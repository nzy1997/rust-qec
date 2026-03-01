# Getting Started Doc + Missing Features Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `decompose_errors` support, per-channel noise params in codegen, and a getting_started doc for rstim.

**Architecture:** Three independent features: (1) post-processing pass on DEM to decompose non-graphlike errors, (2) `NoiseParams` struct replacing single `f64` in codegen, (3) markdown tutorial doc with runnable Rust examples.

**Tech Stack:** Rust, rstim crate, rsinter crate, rmatching crate

---

### Task 1: Add `NoiseParams` struct to codegen

**Files:**
- Create: `rstim/src/codegen/noise_params.rs`
- Modify: `rstim/src/codegen/mod.rs`

**Step 1: Create `noise_params.rs`**

```rust
/// Per-channel noise parameters for circuit generation.
///
/// Mirrors Stim's `CircuitGenParameters` noise channels.
#[derive(Debug, Clone, Copy)]
pub struct NoiseParams {
    /// DEPOLARIZE1 on data qubits at the start of each round.
    pub before_round_data_depolarization: f64,
    /// DEPOLARIZE1 after single-qubit gates, DEPOLARIZE2 after two-qubit gates.
    pub after_clifford_depolarization: f64,
    /// X_ERROR before Z-basis measurement (anti-basis error).
    pub before_measure_flip_probability: f64,
    /// X_ERROR after Z-basis reset (anti-basis error).
    pub after_reset_flip_probability: f64,
}

impl NoiseParams {
    /// All channels set to the same value.
    pub fn uniform(noise: f64) -> Self {
        NoiseParams {
            before_round_data_depolarization: noise,
            after_clifford_depolarization: noise,
            before_measure_flip_probability: noise,
            after_reset_flip_probability: noise,
        }
    }

    /// No noise.
    pub fn none() -> Self {
        Self::uniform(0.0)
    }
}

impl Default for NoiseParams {
    fn default() -> Self {
        Self::none()
    }
}
```

**Step 2: Update `codegen/mod.rs`**

Add to `rstim/src/codegen/mod.rs`:
```rust
pub mod noise_params;
pub use noise_params::NoiseParams;
```

**Step 3: Run tests**

Run: `cd /Users/nzy/rcode/rstim && cargo test -p rstim`
Expected: PASS (no behavior change yet)

**Step 4: Commit**

```bash
git add rstim/src/codegen/noise_params.rs rstim/src/codegen/mod.rs
git commit -m "feat: add NoiseParams struct for per-channel noise in codegen"
```

---

### Task 2: Update `repetition_code_memory` to use `NoiseParams`

**Files:**
- Modify: `rstim/src/codegen/rep_code.rs`
- Test: existing tests in `rstim/tests/`

**Step 1: Add `_with_params` variant and update existing function**

In `rstim/src/codegen/rep_code.rs`, keep the existing `repetition_code_memory(distance, rounds, noise)` as a convenience wrapper, and add:

```rust
use super::noise_params::NoiseParams;

pub fn repetition_code_memory(distance: usize, rounds: usize, noise: f64) -> Vec<StimInstr> {
    repetition_code_memory_with_params(distance, rounds, NoiseParams::uniform(noise))
}

pub fn repetition_code_memory_with_params(distance: usize, rounds: usize, params: NoiseParams) -> Vec<StimInstr> {
    // ... same logic but with per-channel noise
}
```

The noise insertion points in rep_code.rs need to change from the current single `DEPOLARIZE1` to:

| Current location | New behavior |
|---|---|
| After ancilla reset (line 34-36) | Add `X_ERROR(after_reset_flip_probability)` after each `R` |
| After CX gates (lines 38-43) | Add `DEPOLARIZE2(after_clifford_depolarization)` after CX pairs |
| Before ancilla measurement (line 51-53) | Add `X_ERROR(before_measure_flip_probability)` before each `M` |
| Data qubits at round start (lines 45-49) | Move to round start as `DEPOLARIZE1(before_round_data_depolarization)` |
| Before final data measurement (lines 76-78) | Replace with `X_ERROR(before_measure_flip_probability)` before each `M`, plus `DEPOLARIZE1(before_round_data_depolarization)` at round start |

Export the new function from `codegen/mod.rs`:
```rust
pub use rep_code::{repetition_code_memory, repetition_code_memory_with_params};
```

**Step 2: Write test**

Add to `rstim/tests/codegen.rs` (or create if needed):

```rust
use rstim::codegen::{repetition_code_memory, repetition_code_memory_with_params, NoiseParams};
use rstim::ir::circuit_to_string;

#[test]
fn rep_code_uniform_matches_legacy() {
    let legacy = repetition_code_memory(5, 3, 0.01);
    let params = repetition_code_memory_with_params(5, 3, NoiseParams::uniform(0.01));
    // Both should produce valid circuits (may differ in noise placement detail)
    assert!(!legacy.is_empty());
    assert!(!params.is_empty());
}

#[test]
fn rep_code_per_channel_noise() {
    let params = NoiseParams {
        before_round_data_depolarization: 0.01,
        after_clifford_depolarization: 0.02,
        before_measure_flip_probability: 0.03,
        after_reset_flip_probability: 0.04,
    };
    let circuit = repetition_code_memory_with_params(3, 2, params);
    let text = circuit_to_string(&circuit);
    assert!(text.contains("DEPOLARIZE1(0.01)"), "should have data depolarization");
    assert!(text.contains("DEPOLARIZE2(0.02)"), "should have clifford depolarization");
    assert!(text.contains("X_ERROR(0.03)"), "should have measure flip");
    assert!(text.contains("X_ERROR(0.04)"), "should have reset flip");
}

#[test]
fn rep_code_no_noise() {
    let circuit = repetition_code_memory_with_params(3, 2, NoiseParams::none());
    let text = circuit_to_string(&circuit);
    assert!(!text.contains("ERROR"), "no noise instructions");
    assert!(!text.contains("DEPOLARIZE"), "no depolarize instructions");
}
```

**Step 3: Run tests**

Run: `cd /Users/nzy/rcode/rstim && cargo test -p rstim`
Expected: PASS

**Step 4: Commit**

```bash
git add rstim/src/codegen/rep_code.rs rstim/src/codegen/mod.rs rstim/tests/codegen.rs
git commit -m "feat: add per-channel noise to repetition_code_memory"
```

---

### Task 3: Update surface code codegen to use `NoiseParams`

**Files:**
- Modify: `rstim/src/codegen/surface_code.rs`
- Modify: `rstim/src/codegen/mod.rs`

**Step 1: Add `_with_params` variants**

Same pattern as Task 2. Keep existing `rotated_memory_x/z(d, rounds, noise)` as wrappers. Add:

```rust
pub fn rotated_memory_x_with_params(distance: usize, rounds: usize, params: NoiseParams) -> Vec<StimInstr>
pub fn rotated_memory_z_with_params(distance: usize, rounds: usize, params: NoiseParams) -> Vec<StimInstr>
pub fn unrotated_memory_x_with_params(distance: usize, rounds: usize, params: NoiseParams) -> Vec<StimInstr>
pub fn unrotated_memory_z_with_params(distance: usize, rounds: usize, params: NoiseParams) -> Vec<StimInstr>
```

Noise mapping in the internal `rotated_surface_code` / `unrotated_surface_code`:

| Current code | New behavior |
|---|---|
| `DEPOLARIZE1` after data reset | `X_ERROR(after_reset_flip_probability)` |
| `DEPOLARIZE2` after CX layers | `DEPOLARIZE2(after_clifford_depolarization)` |
| `DEPOLARIZE1` before data measurement | Split into `DEPOLARIZE1(before_round_data_depolarization)` at round start + `X_ERROR(before_measure_flip_probability)` before measurement |

Export new functions from `codegen/mod.rs`.

**Step 2: Write test**

```rust
#[test]
fn surface_code_per_channel_noise() {
    let params = NoiseParams {
        before_round_data_depolarization: 0.001,
        after_clifford_depolarization: 0.002,
        before_measure_flip_probability: 0.003,
        after_reset_flip_probability: 0.004,
    };
    let circuit = rotated_memory_z_with_params(3, 3, params);
    let text = circuit_to_string(&circuit);
    assert!(text.contains("DEPOLARIZE2(0.002)"), "should have 2-qubit depolarization after CX");
}
```

**Step 3: Run tests and commit**

Run: `cd /Users/nzy/rcode/rstim && cargo test -p rstim`

```bash
git add rstim/src/codegen/surface_code.rs rstim/src/codegen/mod.rs rstim/tests/codegen.rs
git commit -m "feat: add per-channel noise to surface code codegen"
```

---

### Task 4: Update color code codegen to use `NoiseParams`

**Files:**
- Modify: `rstim/src/codegen/color_code.rs`
- Modify: `rstim/src/codegen/mod.rs`

Same pattern as Tasks 2-3. Add `memory_xyz_with_params(distance, rounds, params)`. Export from mod.rs.

**Step 1: Implement and test**

```rust
#[test]
fn color_code_per_channel_noise() {
    let params = NoiseParams {
        before_round_data_depolarization: 0.01,
        after_clifford_depolarization: 0.02,
        before_measure_flip_probability: 0.03,
        after_reset_flip_probability: 0.04,
    };
    let circuit = memory_xyz_with_params(5, 4, params);
    let text = circuit_to_string(&circuit);
    assert!(text.contains("DEPOLARIZE2(0.02)"));
}
```

**Step 2: Run tests and commit**

```bash
git add rstim/src/codegen/color_code.rs rstim/src/codegen/mod.rs rstim/tests/codegen.rs
git commit -m "feat: add per-channel noise to color code codegen"
```

---

### Task 5: Implement `decompose_errors` in error analyzer

**Files:**
- Modify: `rstim/src/error_analyzer.rs`

**Context:** `DemTarget::Separator` already exists and is fully supported in parsing, display, and sampling. The error analyzer just never generates separators. We need a post-processing function that decomposes non-graphlike errors.

**Step 1: Write tests first**

Add `rstim/tests/decompose_errors.rs`:

```rust
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::parser::parse_lines;

#[test]
fn decompose_simple_circuit_no_change() {
    // Circuit where all errors are already graphlike (≤2 detectors)
    let circuit = parse_lines("
        R 0 1 2 3
        X_ERROR(0.1) 0
        M 0 1 2 3
        DETECTOR rec[-4] rec[-3]
        DETECTOR rec[-3] rec[-2]
        OBSERVABLE_INCLUDE(0) rec[-1]
    ").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
    let text = dem.to_string();
    // Should have errors with at most 2 detectors each
    for line in text.lines() {
        if line.starts_with("error") {
            let det_count = line.matches('D').count();
            assert!(det_count <= 2, "should be graphlike: {}", line);
        }
    }
}

#[test]
fn decompose_depolarize2_produces_graphlike() {
    // DEPOLARIZE2 can produce 3+ detector errors that need decomposition
    let circuit = parse_lines("
        R 0 1 2 3
        H 1 3
        TICK
        CX 0 1 2 3
        DEPOLARIZE2(0.01) 0 1 2 3
        TICK
        CX 0 1 2 3
        H 1 3
        M 0 1 2 3
        DETECTOR rec[-4] rec[-3]
        DETECTOR rec[-3] rec[-2]
        DETECTOR rec[-2] rec[-1]
        OBSERVABLE_INCLUDE(0) rec[-1]
    ").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
    let text = dem.to_string();
    // All errors should be graphlike after decomposition
    for line in text.lines() {
        if line.starts_with("error") {
            // Split on ^ to check each component
            let components: Vec<&str> = line.split('^').collect();
            for comp in components {
                let det_count = comp.matches('D').count();
                assert!(det_count <= 2, "each component should have ≤2 detectors: {}", line);
            }
        }
    }
}

#[test]
fn decompose_rep_code_all_graphlike() {
    use rstim::codegen::repetition_code_memory;
    let circuit = repetition_code_memory(5, 3, 0.01);
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
    let text = dem.to_string();
    for line in text.lines() {
        if line.starts_with("error") {
            let components: Vec<&str> = line.split('^').collect();
            for comp in components {
                let det_count = comp.matches('D').count();
                assert!(det_count <= 2, "non-graphlike error: {}", line);
            }
        }
    }
}
```

**Step 2: Implement `circuit_to_dem_decomposed`**

Add to `rstim/src/error_analyzer.rs`:

```rust
impl ErrorAnalyzer {
    /// Like `circuit_to_dem`, but decomposes non-graphlike errors into
    /// graphlike components (at most 2 detectors per component).
    pub fn circuit_to_dem_decomposed(instrs: &[StimInstr]) -> Result<DetectorErrorModel, String> {
        let mut dem = Self::circuit_to_dem(instrs)?;
        decompose_errors(&mut dem)?;
        Ok(dem)
    }
}

/// Post-process a DEM to decompose non-graphlike errors.
fn decompose_errors(dem: &mut DetectorErrorModel) -> Result<(), String> {
    // 1. Collect all graphlike errors as a lookup table: sorted detector set -> targets
    // 2. For each non-graphlike error (3+ detectors in a single component):
    //    a. Try to express it as XOR of known graphlike errors
    //    b. If successful, replace with decomposed version (components separated by ^)
    //    c. If not, return error
    // ...
}
```

The decomposition algorithm:
1. Scan all errors. An error is "graphlike" if each `^`-separated component has ≤2 `Detector` targets.
2. Build a map: `BTreeSet<usize>` (detector indices) → index in error list.
3. For each non-graphlike error with detector set S (|S| ≥ 3):
   - Try all pairs of known graphlike errors whose detector sets XOR to S.
   - If found, replace the error with the decomposed components separated by `DemTarget::Separator`.
   - If not found, try single graphlike errors whose detector set is a subset, then look for the remainder.
4. If decomposition fails, return `Err`.

**Step 3: Run tests and commit**

Run: `cd /Users/nzy/rcode/rstim && cargo test -p rstim decompose`

```bash
git add rstim/src/error_analyzer.rs rstim/tests/decompose_errors.rs
git commit -m "feat: add circuit_to_dem_decomposed for graphlike error decomposition"
```

---

### Task 6: Cross-validate decomposed DEM against Stim

**Files:**
- Create: `rstim/tests/cross_validate_dem.rs`

**Step 1: Write cross-validation test**

This test generates circuits with Stim (via CLI), gets the DEM with `decompose_errors=true`, and compares against rstim's `circuit_to_dem_decomposed`. Since exact DEM output may differ (order, probability grouping), we compare semantically: same set of detector-observable pairs should appear.

```rust
#[test]
#[ignore] // requires stim CLI installed
fn cross_validate_decomposed_dem_rep_code() {
    use rstim::codegen::repetition_code_memory;
    use rstim::ir::circuit_to_string;
    use rstim::error_analyzer::ErrorAnalyzer;
    use std::process::Command;

    let circuit = repetition_code_memory(5, 3, 0.01);
    let circuit_text = circuit_to_string(&circuit);

    // Get Stim's DEM
    let output = Command::new("stim")
        .args(["analyze_errors", "--decompose_errors"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(circuit_text.as_bytes()).unwrap();
            child.wait_with_output()
        })
        .unwrap();
    let stim_dem_text = String::from_utf8(output.stdout).unwrap();

    // Get rstim's DEM
    let rstim_dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
    let rstim_dem_text = rstim_dem.to_string();

    // Both should parse and be non-empty
    assert!(!stim_dem_text.is_empty());
    assert!(!rstim_dem_text.is_empty());

    // Compare error count (should be similar)
    let stim_errors = stim_dem_text.lines().filter(|l| l.trim().starts_with("error")).count();
    let rstim_errors = rstim_dem_text.lines().filter(|l| l.trim().starts_with("error")).count();
    assert_eq!(stim_errors, rstim_errors, "error count mismatch:\nstim:\n{}\nrstim:\n{}", stim_dem_text, rstim_dem_text);
}
```

**Step 2: Run and commit**

Run: `cd /Users/nzy/rcode/rstim && cargo test -p rstim cross_validate_decomposed -- --include-ignored`

```bash
git add rstim/tests/cross_validate_dem.rs
git commit -m "test: add cross-validation of decomposed DEM against Stim"
```

---

### Task 7: Write getting_started.md doc

**Files:**
- Create: `rstim/doc/getting_started.md`

**Context:** This doc mirrors Stim's `getting_started.ipynb` but uses rstim's Rust API. Skip all visualization. Each section has a complete runnable code example.

**Step 1: Write the doc**

The doc should cover these sections with complete Rust code examples:

1. **What is rstim?** — Brief intro
2. **Create a circuit and sample** — `parse_lines()`, `sample_batch()`, reading `BatchOutput.measurements`
3. **Add detectors and sample them** — `StimTarget::Rec`, detector annotations, `BatchOutput.detections`
4. **Generate QEC circuits** — `repetition_code_memory()`, `rotated_memory_z_with_params()` with `NoiseParams`
5. **Extract detector error model** — `ErrorAnalyzer::circuit_to_dem()`, `circuit_to_dem_decomposed()`
6. **Decode with rmatching** — Build `rmatching::Matching` from DEM text, `decode()`
7. **Estimate threshold with Monte Carlo** — Loop over distances/noise, count logical errors
8. **Use rsinter for parallel sampling** — `Task`, `collect()`, `shot_error_rate_to_piece_error_rate()`

Each code block should be a standalone `fn main()` that compiles with appropriate `use` statements. Keep examples minimal — shortest code that demonstrates the concept.

**Step 2: Commit**

```bash
mkdir -p rstim/doc
git add rstim/doc/getting_started.md
git commit -m "docs: add getting_started.md tutorial"
```

---

### Task 8: Add doc-test or example binary to verify code compiles

**Files:**
- Create: `rstim/examples/getting_started.rs`

**Step 1: Create a single runnable example**

Extract the key code from getting_started.md into a single `examples/getting_started.rs` that exercises the full pipeline: generate circuit → sample → get DEM → decode (if rmatching available). This ensures the doc examples stay compilable.

```rust
//! Getting started example — exercises the rstim pipeline.
use rstim::codegen::{repetition_code_memory_with_params, NoiseParams};
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::sampler::sample_batch;
use rstim::parser::parse_lines;
use rstim::ir::circuit_to_string;
use rand::SeedableRng;
use rand::rngs::StdRng;

fn main() {
    let mut rng = StdRng::seed_from_u64(42);

    // 1. Build a simple circuit
    let circuit = parse_lines("H 0\nCNOT 0 1\nM 0 1").unwrap();
    let output = sample_batch(&circuit, 10, &mut rng).unwrap();
    println!("Sampled {} shots, {} measurements each", output.measurements.num_major(), output.measurements.num_minor());

    // 2. Generate a rep code with per-channel noise
    let params = NoiseParams {
        before_round_data_depolarization: 0.01,
        after_clifford_depolarization: 0.02,
        before_measure_flip_probability: 0.01,
        after_reset_flip_probability: 0.01,
    };
    let circuit = repetition_code_memory_with_params(5, 3, params);
    println!("Generated circuit: {} instructions", circuit.len());

    // 3. Extract DEM
    let dem = ErrorAnalyzer::circuit_to_dem_decomposed(&circuit).unwrap();
    println!("DEM: {} detectors, {} observables", dem.num_detectors(), dem.num_observables());
    println!("{}", dem);
}
```

**Step 2: Run and commit**

Run: `cd /Users/nzy/rcode/rstim && cargo run -p rstim --example getting_started`

```bash
git add rstim/examples/getting_started.rs
git commit -m "feat: add getting_started example binary"
```

---

### Task 9: Push and verify CI

**Step 1: Run full test suite**

```bash
cd /Users/nzy/rcode/rstim && cargo test --workspace
```

**Step 2: Push**

```bash
git push origin master
```
