# rstim DEM Parity Phase 1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Bring `rstim analyze_errors` Phase 1 behavior into default-semantic parity with the March 9, 2026 DEM parity design: reject invalid default inputs cleanly, fix correlated-block semantics, and preserve already-correct DEM generation paths.

**Architecture:** Keep the existing `ErrorAnalyzer::circuit_to_dem` entry point in [`/Users/nzy/rcode/rstim/rstim/src/error_analyzer.rs`](/Users/nzy/rcode/rstim/rstim/src/error_analyzer.rs) and extend it with narrow validation helpers instead of rewriting propagation. Add failing analyzer and CLI regression tests first, then implement guardrails in small passes: checked `rec[]` lookup, deterministic-collapse checks, probability guard checks, and correlated/disjoint channel handling. Preserve current reverse-walk sensitivity propagation except where default Stim parity requires stricter semantics.

**Tech Stack:** Rust, Cargo workspace tests, existing `rstim::error_analyzer::ErrorAnalyzer`, `clap` CLI, existing DEM IR in [`/Users/nzy/rcode/rstim/rstim/src/dem.rs`](/Users/nzy/rcode/rstim/rstim/src/dem.rs).

---

### Task 1: Convert invalid `rec[]` lookbacks from panic to structured error

**Files:**
- Modify: `rstim/src/error_analyzer.rs`
- Modify: `rstim/tests/stim_error_analyzer.rs`
- Modify: `rstim/tests/cli_analyze.rs`

**Step 1: Replace the panic-based tests with failing error assertions**

In [`/Users/nzy/rcode/rstim/rstim/tests/stim_error_analyzer.rs`](/Users/nzy/rcode/rstim/rstim/tests/stim_error_analyzer.rs), replace the two `#[should_panic]` tests near the bottom with explicit `Result` assertions:

```rust
#[test]
fn stim_measurement_before_beginning_detector() {
    let result = circuit_to_dem_err("DETECTOR rec[-1]");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("rec"));
}

#[test]
fn stim_measurement_before_beginning_observable() {
    let result = circuit_to_dem_err("OBSERVABLE_INCLUDE(0) rec[-1]");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("rec"));
}
```

Add one CLI regression in [`/Users/nzy/rcode/rstim/rstim/tests/cli_analyze.rs`](/Users/nzy/rcode/rstim/rstim/tests/cli_analyze.rs):

```rust
#[test]
fn analyze_errors_invalid_rec_fails_cleanly() {
    let output = run_with_stdin(&["analyze_errors"], "DETECTOR rec[-1]");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("rec"));
    assert!(!stderr.contains("panicked"));
}
```

**Step 2: Run the targeted tests to verify they fail**

```bash
cargo test -p rstim --test stim_error_analyzer stim_measurement_before_beginning_detector -- --exact
cargo test -p rstim --test stim_error_analyzer stim_measurement_before_beginning_observable -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_invalid_rec_fails_cleanly -- --exact
```

Expected: analyzer tests fail due to panic or unwrap, and the CLI test fails because `rstim analyze_errors` currently panics instead of returning a clean error.

**Step 3: Add a checked record-index helper and use it at every `Rec` lookup**

In [`/Users/nzy/rcode/rstim/rstim/src/error_analyzer.rs`](/Users/nzy/rcode/rstim/rstim/src/error_analyzer.rs), add:

```rust
fn checked_rec_index(num_measurements: usize, offset: i32) -> Result<usize, String> {
    let idx = num_measurements as i32 + offset;
    if idx < 0 || idx >= num_measurements as i32 {
        return Err(format!(
            "invalid rec[{offset}] reference with {num_measurements} measurements available"
        ));
    }
    Ok(idx as usize)
}
```

Then update the `DETECTOR` and `OBSERVABLE_INCLUDE` branches to use it:

```rust
if let StimTarget::Rec(offset) = t {
    let abs_idx = checked_rec_index(self.num_measurements, *offset)?;
    self.measurement_sens[abs_idx].xor_item(det_target.clone());
}
```

Use the same pattern for observable references.

**Step 4: Run the targeted tests to verify they pass**

```bash
cargo test -p rstim --test stim_error_analyzer stim_measurement_before_beginning_detector -- --exact
cargo test -p rstim --test stim_error_analyzer stim_measurement_before_beginning_observable -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_invalid_rec_fails_cleanly -- --exact
```

Expected: all three tests pass and stderr contains a normal `Error: ...` message instead of a panic backtrace.

**Step 5: Commit**

```bash
git add rstim/src/error_analyzer.rs rstim/tests/stim_error_analyzer.rs rstim/tests/cli_analyze.rs
git commit -m "fix: return structured errors for invalid rec lookbacks"
```

### Task 2: Reject gauge detectors and observables by default

**Files:**
- Modify: `rstim/src/error_analyzer.rs`
- Modify: `rstim/tests/stim_error_analyzer.rs`
- Modify: `rstim/tests/cli_analyze.rs`

**Step 1: Unignore the existing gauge tests and add one CLI check**

In [`/Users/nzy/rcode/rstim/rstim/tests/stim_error_analyzer.rs`](/Users/nzy/rcode/rstim/rstim/tests/stim_error_analyzer.rs), remove the `#[ignore = ...]` attributes from:

- `stim_detect_gauge_observable`
- `stim_detect_gauge_detector_r_h_m`
- `stim_detect_gauge_detector_m_h_m`
- `stim_detect_gauge_detector_mz_mx`
- `stim_detect_gauge_detector_my_mx`
- `stim_detect_gauge_detector_mx_mz`
- `stim_detect_gauge_detector_rx_mz`
- `stim_detect_gauge_detector_ry_mx`
- `stim_detect_gauge_detector_rz_mx`
- `stim_detect_gauge_detector_mx_no_reset`

Add one CLI regression:

```rust
#[test]
fn analyze_errors_rejects_gauge_detector() {
    let output = run_with_stdin(&["analyze_errors"], "R 0\nH 0\nM 0\nDETECTOR rec[-1]");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("non-deterministic"));
}
```

**Step 2: Run the gauge-focused tests to verify they fail**

```bash
cargo test -p rstim --test stim_error_analyzer stim_detect_gauge -- --nocapture
cargo test -p rstim --test cli_analyze analyze_errors_rejects_gauge_detector -- --exact
```

Expected: the analyzer tests fail because `circuit_to_dem` still accepts gauge detectors/observables, and the CLI test exits successfully instead of rejecting the circuit.

**Step 3: Add deterministic-collapse guard helpers**

In [`/Users/nzy/rcode/rstim/rstim/src/error_analyzer.rs`](/Users/nzy/rcode/rstim/rstim/src/error_analyzer.rs), introduce narrow helpers that check whether a collapse/reset is about to discard active sensitivity:

```rust
impl ErrorAnalyzer {
    fn ensure_measurement_is_deterministic(
        &self,
        q: usize,
        basis: PauliBasis,
        kind: &'static str,
    ) -> Result<(), String> {
        let bad = match basis {
            PauliBasis::X => !self.x_sens[q].is_empty(),
            PauliBasis::Y => !(self.x_sens[q].targets == self.z_sens[q].targets),
            PauliBasis::Z => !self.z_sens[q].is_empty(),
        };
        if bad {
            return Err(format!("non-deterministic {kind} encountered"));
        }
        Ok(())
    }

    fn ensure_reset_is_deterministic(
        &self,
        q: usize,
        basis: PauliBasis,
        kind: &'static str,
    ) -> Result<(), String> {
        self.ensure_measurement_is_deterministic(q, basis, kind)
    }

    fn ensure_no_pending_gauge(&self) -> Result<(), String> {
        for (x, z) in self.x_sens.iter().zip(self.z_sens.iter()) {
            if !x.is_empty() || !z.is_empty() {
                return Err("non-deterministic detector encountered".to_string());
            }
        }
        for meas in &self.measurement_sens {
            if !meas.is_empty() {
                return Err("non-deterministic detector encountered".to_string());
            }
        }
        Ok(())
    }
}
```

Call these before clearing sensitivity in:

- `M` / `MZ`
- `MX`
- `MY`
- `MR` / `MRZ`
- `MRX`
- `MRY`
- `R` / `RZ`
- `RX`
- `RY`

At the end of `circuit_to_dem`, after `undo_circuit(instrs)?`, call:

```rust
analyzer.ensure_no_pending_gauge()?;
```

Use `"detector"` for detector-only paths and `"observable"` when the incompatible sensitivity includes `DemTarget::Observable(_)`. Keep the first version simple: detect the condition and return an error; do not attempt Stim-style long diagnostics.

**Step 4: Run the gauge-focused tests to verify they pass**

```bash
cargo test -p rstim --test stim_error_analyzer stim_detect_gauge -- --nocapture
cargo test -p rstim --test cli_analyze analyze_errors_rejects_gauge_detector -- --exact
```

Expected: all gauge tests pass and the CLI exits non-zero with a short `non-deterministic ... encountered` error.

**Step 5: Commit**

```bash
git add rstim/src/error_analyzer.rs rstim/tests/stim_error_analyzer.rs rstim/tests/cli_analyze.rs
git commit -m "fix: reject gauge detectors and observables by default"
```

### Task 3: Reject over-mixing `DEPOLARIZE1` and `DEPOLARIZE2`

**Files:**
- Modify: `rstim/src/error_analyzer.rs`
- Modify: `rstim/tests/stim_error_analyzer.rs`
- Modify: `rstim/tests/cli_analyze.rs`

**Step 1: Add failing tests for invalid depolarize probabilities**

In [`/Users/nzy/rcode/rstim/rstim/tests/stim_error_analyzer.rs`](/Users/nzy/rcode/rstim/rstim/tests/stim_error_analyzer.rs), add:

```rust
#[test]
fn stim_depolarize1_overmix_rejected() {
    let result = circuit_to_dem_err("DEPOLARIZE1(0.76) 0\nM 0\nDETECTOR rec[-1]");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("DEPOLARIZE1"));
}

#[test]
fn stim_depolarize2_overmix_rejected() {
    let result = circuit_to_dem_err("DEPOLARIZE2(0.94) 0 1\nM 0 1\nDETECTOR rec[-1]");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("DEPOLARIZE2"));
}
```

Add one CLI regression in [`/Users/nzy/rcode/rstim/rstim/tests/cli_analyze.rs`](/Users/nzy/rcode/rstim/rstim/tests/cli_analyze.rs):

```rust
#[test]
fn analyze_errors_rejects_overmixed_depolarize() {
    let output = run_with_stdin(
        &["analyze_errors"],
        "DEPOLARIZE1(0.76) 0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr).unwrap().contains("DEPOLARIZE1"));
}
```

**Step 2: Run the new tests to verify they fail**

```bash
cargo test -p rstim --test stim_error_analyzer stim_depolarize1_overmix_rejected -- --exact
cargo test -p rstim --test stim_error_analyzer stim_depolarize2_overmix_rejected -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_rejects_overmixed_depolarize -- --exact
```

Expected: all three tests fail because the analyzer still computes independent probabilities instead of rejecting invalid mixing.

**Step 3: Add probability guards before conversion**

In [`/Users/nzy/rcode/rstim/rstim/src/error_analyzer.rs`](/Users/nzy/rcode/rstim/rstim/src/error_analyzer.rs), add:

```rust
fn ensure_valid_depolarize1_probability(p: f64) -> Result<(), String> {
    if p > 0.75 {
        return Err(format!("DEPOLARIZE1({p}) exceeds exact-analysis limit of 3/4"));
    }
    Ok(())
}

fn ensure_valid_depolarize2_probability(p: f64) -> Result<(), String> {
    if p > 15.0 / 16.0 {
        return Err(format!("DEPOLARIZE2({p}) exceeds exact-analysis limit of 15/16"));
    }
    Ok(())
}
```

Then call them at the start of the `DEPOLARIZE1` and `DEPOLARIZE2` branches:

```rust
let p = args.first().copied().unwrap_or(0.0);
ensure_valid_depolarize1_probability(p)?;
```

and:

```rust
let p = args.first().copied().unwrap_or(0.0);
ensure_valid_depolarize2_probability(p)?;
```

**Step 4: Run the new tests to verify they pass**

```bash
cargo test -p rstim --test stim_error_analyzer stim_depolarize1_overmix_rejected -- --exact
cargo test -p rstim --test stim_error_analyzer stim_depolarize2_overmix_rejected -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_rejects_overmixed_depolarize -- --exact
```

Expected: all three tests pass.

**Step 5: Commit**

```bash
git add rstim/src/error_analyzer.rs rstim/tests/stim_error_analyzer.rs rstim/tests/cli_analyze.rs
git commit -m "fix: reject overmixed depolarize channels in analyze_errors"
```

### Task 4: Enforce default rejection for disjoint `PAULI_CHANNEL_2`

**Files:**
- Modify: `rstim/src/error_analyzer.rs`
- Modify: `rstim/tests/stim_error_analyzer.rs`
- Modify: `rstim/tests/cli_analyze.rs`

**Step 1: Add failing tests for default `PAULI_CHANNEL_2` rejection**

In [`/Users/nzy/rcode/rstim/rstim/tests/stim_error_analyzer.rs`](/Users/nzy/rcode/rstim/rstim/tests/stim_error_analyzer.rs), add:

```rust
#[test]
fn stim_pauli_channel_2_rejected_by_default() {
    let result = circuit_to_dem_err(
        "PAULI_CHANNEL_2(0.01,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\nDETECTOR rec[-1]",
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("PAULI_CHANNEL_2"));
}
```

In [`/Users/nzy/rcode/rstim/rstim/tests/cli_analyze.rs`](/Users/nzy/rcode/rstim/rstim/tests/cli_analyze.rs), add:

```rust
#[test]
fn analyze_errors_rejects_pauli_channel_2() {
    let output = run_with_stdin(
        &["analyze_errors"],
        "PAULI_CHANNEL_2(0.01,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\nDETECTOR rec[-1]",
    );
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr).unwrap().contains("PAULI_CHANNEL_2"));
}
```

**Step 2: Run the tests to verify they fail**

```bash
cargo test -p rstim --test stim_error_analyzer stim_pauli_channel_2_rejected_by_default -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_rejects_pauli_channel_2 -- --exact
```

Expected: both tests fail because `rstim` currently approximates `PAULI_CHANNEL_2` instead of rejecting it.

**Step 3: Reject `PAULI_CHANNEL_2` at the analyzer boundary**

In [`/Users/nzy/rcode/rstim/rstim/src/error_analyzer.rs`](/Users/nzy/rcode/rstim/rstim/src/error_analyzer.rs), replace the `PAULI_CHANNEL_2` branch body with an explicit default error:

```rust
"PAULI_CHANNEL_2" => {
    if args.iter().any(|p| *p > 0.0) {
        return Err(
            "PAULI_CHANNEL_2 requires an approximation mode that rstim does not yet expose"
                .to_string(),
        );
    }
}
```

Keep the zero-probability no-op behavior.

**Step 4: Run the tests to verify they pass**

```bash
cargo test -p rstim --test stim_error_analyzer stim_pauli_channel_2_rejected_by_default -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_rejects_pauli_channel_2 -- --exact
```

Expected: both tests pass.

**Step 5: Commit**

```bash
git add rstim/src/error_analyzer.rs rstim/tests/stim_error_analyzer.rs rstim/tests/cli_analyze.rs
git commit -m "fix: reject pauli_channel_2 without approximation mode"
```

### Task 5: Fix `E` / `ELSE_CORRELATED_ERROR` semantics and reject unsupported multi-branch blocks

**Files:**
- Modify: `rstim/src/error_analyzer.rs`
- Modify: `rstim/tests/stim_error_analyzer.rs`
- Modify: `rstim/tests/cli_analyze.rs`

**Step 1: Add failing tests for correlated-block semantics**

In [`/Users/nzy/rcode/rstim/rstim/tests/stim_error_analyzer.rs`](/Users/nzy/rcode/rstim/rstim/tests/stim_error_analyzer.rs), add:

```rust
#[test]
fn stim_else_correlated_error_without_leader_is_rejected() {
    let result = circuit_to_dem_err("ELSE_CORRELATED_ERROR(0.25) X0\nM 0\nDETECTOR rec[-1]");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("ELSE_CORRELATED_ERROR"));
}

#[test]
fn stim_correlated_error_two_branch_block_is_mutually_exclusive() {
    let dem = circuit_to_dem(
        "E(0.25) X0\nELSE_CORRELATED_ERROR(0.5) X0\nM 0\nDETECTOR rec[-1]",
    );
    assert_eq!(error_count(&dem), 1);
    assert_has_error(&dem, 0.625, &[DemTarget::Detector(0)]);
}

#[test]
fn stim_correlated_error_three_branch_block_rejected_by_default() {
    let result = circuit_to_dem_err(
        "E(0.1) X0\nELSE_CORRELATED_ERROR(0.2) Z0\nELSE_CORRELATED_ERROR(0.3) Y0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("approximation"));
}
```

In [`/Users/nzy/rcode/rstim/rstim/tests/cli_analyze.rs`](/Users/nzy/rcode/rstim/rstim/tests/cli_analyze.rs), add:

```rust
#[test]
fn analyze_errors_rejects_unsupported_correlated_block() {
    let output = run_with_stdin(
        &["analyze_errors"],
        "E(0.1) X0\nELSE_CORRELATED_ERROR(0.2) Z0\nELSE_CORRELATED_ERROR(0.3) Y0\nM 0\nDETECTOR rec[-1]",
    );
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr).unwrap().contains("approximation"));
}
```

**Step 2: Run the tests to verify they fail**

```bash
cargo test -p rstim --test stim_error_analyzer stim_else_correlated_error_without_leader_is_rejected -- --exact
cargo test -p rstim --test stim_error_analyzer stim_correlated_error_two_branch_block_is_mutually_exclusive -- --exact
cargo test -p rstim --test stim_error_analyzer stim_correlated_error_three_branch_block_rejected_by_default -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_rejects_unsupported_correlated_block -- --exact
```

Expected: `rstim` currently treats `E` and `ELSE_CORRELATED_ERROR` as independent error insertions, so the semantic test fails and the rejection tests fail.

**Step 3: Add a lightweight correlated-block collector**

In [`/Users/nzy/rcode/rstim/rstim/src/error_analyzer.rs`](/Users/nzy/rcode/rstim/rstim/src/error_analyzer.rs), refactor `undo_circuit` to iterate by index so it can consume contiguous correlated blocks:

```rust
fn undo_circuit(&mut self, instrs: &[StimInstr]) -> Result<(), String> {
    let mut k = instrs.len();
    while k > 0 {
        k -= 1;
        match &instrs[k] {
            StimInstr::Op { name, args, targets, .. }
                if name == "ELSE_CORRELATED_ERROR" =>
            {
                return Err("ELSE_CORRELATED_ERROR without preceding E block".to_string());
            }
            StimInstr::Op { name, .. } if name == "CORRELATED_ERROR" || name == "E" => {
                let start = k;
                while k > 0 {
                    if let StimInstr::Op { name, .. } = &instrs[k - 1] {
                        if name == "ELSE_CORRELATED_ERROR" {
                            k -= 1;
                            continue;
                        }
                    }
                    break;
                }
                self.undo_correlated_block(&instrs[k..=start])?;
            }
            StimInstr::Op { name, args, targets, .. } => {
                self.undo_op(name.as_str(), args, targets)?;
            }
            StimInstr::Repeat { count, body } => {
                for _ in 0..*count {
                    self.undo_circuit(body)?;
                }
            }
        }
    }
    Ok(())
}
```

Add helpers:

```rust
struct CorrelatedBranch {
    probability: f64,
    targets: Vec<DemTarget>,
}

fn branch_targets(&self, targets: &[StimTarget]) -> Vec<DemTarget> { /* same sensitivity logic as current E branch */ }

fn undo_correlated_block(&mut self, block: &[StimInstr]) -> Result<(), String> {
    if block.len() > 2 {
        return Err("correlated error block requires approximation mode for >2 branches".to_string());
    }

    let mut grouped: BTreeMap<Vec<DemTarget>, f64> = BTreeMap::new();
    let mut remaining = 1.0;
    for instr in block.iter().rev() {
        let (probability, targets) = correlated_branch_from_instr(self, instr)?;
        let effective = probability * remaining;
        remaining *= 1.0 - probability;
        if effective > 0.0 && !targets.is_empty() {
            *grouped.entry(targets).or_default() += effective;
        }
    }
    for (targets, probability) in grouped {
        self.errors.push((probability, targets));
    }
    Ok(())
}
```

Implementation notes:

- Require a leading `E` or `CORRELATED_ERROR`; reject a standalone `ELSE_CORRELATED_ERROR`.
- Treat contiguous `ELSE_CORRELATED_ERROR` instructions as part of the same mutually-exclusive block.
- Preserve current sensitivity extraction logic by factoring the old branch code into a reusable helper instead of duplicating it.
- Keep the default Phase 1 policy narrow: allow the exact 2-branch case, reject larger blocks until approximation options exist.

**Step 4: Run the correlated-block tests to verify they pass**

```bash
cargo test -p rstim --test stim_error_analyzer stim_else_correlated_error_without_leader_is_rejected -- --exact
cargo test -p rstim --test stim_error_analyzer stim_correlated_error_two_branch_block_is_mutually_exclusive -- --exact
cargo test -p rstim --test stim_error_analyzer stim_correlated_error_three_branch_block_rejected_by_default -- --exact
cargo test -p rstim --test cli_analyze analyze_errors_rejects_unsupported_correlated_block -- --exact
```

Expected: all four tests pass.

**Step 5: Commit**

```bash
git add rstim/src/error_analyzer.rs rstim/tests/stim_error_analyzer.rs rstim/tests/cli_analyze.rs
git commit -m "fix: handle correlated error blocks with default-safe semantics"
```

### Task 6: Run the preserved parity regressions and close the Phase 1 loop

**Files:**
- No code changes required unless regressions appear

**Step 1: Run the existing analyzer parity coverage that should stay green**

```bash
cargo test -p rstim --test stim_error_analyzer
cargo test -p rstim --test cli_analyze
cargo test -p rstim --test cross_validate_dem
```

Expected: all tests pass. If `cross_validate_dem` has Stim CLI/environment assumptions, document any skipped cases in the commit message or session notes instead of weakening assertions.

**Step 2: Run the broader package tests that exercise DEM paths**

```bash
cargo test -p rstim --test stim_dem
cargo test -p rstim --test dem_integration
cargo test -p rstim --test error_analyzer
```

Expected: pass. These confirm the new Phase 1 guardrails did not break normal DEM generation or downstream consumers.

**Step 3: If any regression appears, fix it before claiming completion**

Likely hot spots:

- detector/observable target ordering in merged errors
- false-positive gauge rejection at measurement/reset boundaries
- repeat-block traversal after the correlated-block refactor
- CLI stderr text assumptions that are too strict

**Step 4: Commit the final verification checkpoint**

```bash
git add -A
git commit -m "test: verify dem parity phase 1 regressions stay green"
```
