# Issue 89 BpLsd Solve Path Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the first supported deterministic LSD order-1 solve path behind `rbposd::BpLsdDecoder`.

**Architecture:** Keep `BpLsdDecoder` as the public entrypoint and move LSD-specific post-BP logic into a new internal `rbposd/src/lsd.rs` module. The decoder continues to run existing `BpCore`; nonzero BP residuals become LSD target syndromes, and the new LSD workspace produces residual corrections without using `OsdWorkspace`.

**Tech Stack:** Rust 2024 workspace, `rbposd` crate, `serde`/`serde_json` dev-dependencies already present for JSON fixtures, standard `cargo test`.

## Global Constraints

- Do not modify `rsinter`, DEM adapters, benchmark runner params, result rows, or benchmark specs.
- Do not add a fixture manifest or fixture catalog validation.
- Do not extend the Python parity harness or add upstream `ldpc` differential plumbing.
- Do not implement additional BP methods, schedules, `bits_per_step`, or `always_run_lsd`.
- Do not add new public LSD method variants.
- Do not change the public shape of `DecodeResult`.
- Keep `LsdMethod::LocalizedStatistics` as the only public LSD method variant.
- Support `lsd_order = 0` and `lsd_order = 1`.
- Reject `lsd_order > 1` deterministically.
- Use `DecodeError::NoLsdSolution` for LSD failures.

---

## File Structure

- Modify `rbposd/src/error.rs`: add `DecodeError::NoLsdSolution` and display text.
- Modify `rbposd/dev/parity_schema.rs`: add stable error code mapping for `NoLsdSolution`.
- Modify `rbposd/tests/smoke.rs`: cover `NoLsdSolution` display text.
- Modify `rbposd/tests/parity_dev.rs`: cover `NoLsdSolution` error code.
- Create `rbposd/src/lsd.rs`: internal LSD workspace, order-0 residual solve, order-1 component-local candidate search, deterministic ordering helpers, unit tests.
- Modify `rbposd/src/lib.rs`: register internal `lsd` module.
- Modify `rbposd/src/lsd_decoder.rs`: use `LsdWorkspace`, accept order 1, reject orders greater than 1, call the internal LSD solver.
- Create `rbposd/tests/fixtures/lsd/lsd_small_sparse_code.json`: positive order-1 fixture.
- Create `rbposd/tests/fixtures/lsd/lsd_order_one_improves_over_baseline.json`: positive order-1 fixture whose correction differs from order 0.
- Create `rbposd/tests/fixtures/lsd/lsd_unsatisfiable_case.json`: negative fixture.
- Modify `rbposd/tests/lsd.rs`: add fixture loader and #89 integration tests.
- Modify `rbposd/doc/ldpc_mvp_reference.md`: document order-1 support, `NoLsdSolution`, and the minimal fixture boundary.

---

### Task 1: Add LSD Failure Error Contract

**Files:**
- Modify: `rbposd/src/error.rs`
- Modify: `rbposd/dev/parity_schema.rs`
- Modify: `rbposd/tests/smoke.rs`
- Modify: `rbposd/tests/parity_dev.rs`

**Interfaces:**
- Consumes: existing `DecodeError` enum and parity error-code mapping.
- Produces: `DecodeError::NoLsdSolution`, display text `no LSD solution found`, and stable parity code `NoLsdSolution`.

- [ ] **Step 1: Write failing smoke and parity-dev tests**

In `rbposd/tests/smoke.rs`, add this assertion inside `correction_helpers_and_error_display_cover_remaining_contracts` after the `NoOsdSolution` assertion:

```rust
    assert_eq!(
        DecodeError::NoLsdSolution.to_string(),
        "no LSD solution found"
    );
```

In `rbposd/tests/parity_dev.rs`, add this tuple to the `stable_error_cases` array inside `parity_outcomes_use_stable_error_codes_and_partial_diagnostics_matching`:

```rust
        (DecodeError::NoLsdSolution, "NoLsdSolution"),
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
cargo test -p rbposd correction_helpers_and_error_display_cover_remaining_contracts
cargo test -p rbposd parity_outcomes_use_stable_error_codes_and_partial_diagnostics_matching
```

Expected: FAIL with `no variant or associated item named 'NoLsdSolution'`.

- [ ] **Step 3: Add the error variant and display text**

In `rbposd/src/error.rs`, add this variant after `NoOsdSolution`:

```rust
    NoLsdSolution,
```

Update the `Display` match by adding this arm after the `NoOsdSolution` arm:

```rust
            Self::NoLsdSolution => write!(f, "no LSD solution found"),
```

The relevant part of `DecodeError` should read:

```rust
    BpDidNotConverge,
    NoOsdSolution,
    NoLsdSolution,
    UnsupportedLsdOrder {
        order: usize,
    },
```

- [ ] **Step 4: Add the stable parity error code**

In `rbposd/dev/parity_schema.rs`, add this match arm to `error_code` after `NoOsdSolution`:

```rust
        DecodeError::NoLsdSolution => "NoLsdSolution",
```

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p rbposd correction_helpers_and_error_display_cover_remaining_contracts
cargo test -p rbposd parity_outcomes_use_stable_error_codes_and_partial_diagnostics_matching
```

Expected: PASS.

- [ ] **Step 6: Commit Task 1**

Run:

```bash
git add rbposd/src/error.rs rbposd/dev/parity_schema.rs rbposd/tests/smoke.rs rbposd/tests/parity_dev.rs
git commit -m "feat: add rbposd lsd failure error"
```

---

### Task 2: Add Minimal LSD Fixtures And Failing Integration Tests

**Files:**
- Create: `rbposd/tests/fixtures/lsd/lsd_small_sparse_code.json`
- Create: `rbposd/tests/fixtures/lsd/lsd_order_one_improves_over_baseline.json`
- Create: `rbposd/tests/fixtures/lsd/lsd_unsatisfiable_case.json`
- Modify: `rbposd/tests/lsd.rs`

**Interfaces:**
- Consumes: public `BpLsdDecoder`, `LsdConfig`, `ChannelModel`, `ParityCheckMatrix`, `Syndrome`, `Correction`, `DecodeError`.
- Produces: integration tests `bplsd_order_one_recovers_the_borrowed_small_matrix_cases` and `bplsd_returns_a_decoder_error_for_an_unsatisfiable_case`.

- [ ] **Step 1: Add the positive small sparse fixture**

Create `rbposd/tests/fixtures/lsd/lsd_small_sparse_code.json` with this exact content:

```json
{
  "id": "lsd_small_sparse_code",
  "matrix": {
    "num_checks": 2,
    "num_bits": 3,
    "rows": [[1, 2], [0]]
  },
  "channel": {
    "kind": "bsc",
    "error_rate": 0.05
  },
  "syndrome": [true, false],
  "lsd_order": 1,
  "expected": {
    "status": "success"
  }
}
```

- [ ] **Step 2: Add the order-one improvement fixture**

Create `rbposd/tests/fixtures/lsd/lsd_order_one_improves_over_baseline.json` with this exact content:

```json
{
  "id": "lsd_order_one_improves_over_baseline",
  "matrix": {
    "num_checks": 2,
    "num_bits": 3,
    "rows": [[0], [1, 2]]
  },
  "channel": {
    "kind": "bsc",
    "error_rate": 0.3775406687981454
  },
  "syndrome": [false, true],
  "lsd_order": 1,
  "expected": {
    "status": "success",
    "order_0_correction": [false, true, false],
    "order_1_correction": [false, false, true]
  }
}
```

This fixture uses `error_rate = 1 / (1 + exp(0.5))`, so the channel LLR is `0.5`. With the current BP path, BP reaches the iteration budget without satisfying the syndrome; order 0 selects bit 1, and order 1 selects bit 2 by deterministic equal-cost lexicographic candidate tie-break.

- [ ] **Step 3: Add the unsatisfiable fixture**

Create `rbposd/tests/fixtures/lsd/lsd_unsatisfiable_case.json` with this exact content:

```json
{
  "id": "lsd_unsatisfiable_case",
  "matrix": {
    "num_checks": 2,
    "num_bits": 1,
    "rows": [[0], [0]]
  },
  "channel": {
    "kind": "bsc",
    "error_rate": 0.05
  },
  "syndrome": [true, false],
  "lsd_order": 1,
  "expected": {
    "status": "error",
    "error": "NoLsdSolution"
  }
}
```

- [ ] **Step 4: Add fixture loader imports and helper types**

At the top of `rbposd/tests/lsd.rs`, replace the current imports with:

```rust
use std::fs;
use std::path::Path;

use rbposd::{
    BpLsdDecoder, ChannelModel, Correction, DecodeError, LsdConfig, ParityCheckMatrix, Syndrome,
};
use serde::Deserialize;
```

Add these helper structs after the imports:

```rust
#[derive(Debug, Deserialize)]
struct LsdFixture {
    id: String,
    matrix: MatrixFixture,
    channel: ChannelFixture,
    syndrome: Vec<bool>,
    lsd_order: usize,
    expected: ExpectedFixture,
}

#[derive(Debug, Deserialize)]
struct MatrixFixture {
    num_checks: usize,
    num_bits: usize,
    rows: Vec<Vec<usize>>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ChannelFixture {
    Bsc { error_rate: f64 },
    BitFlipProbabilities { probabilities: Vec<f64> },
}

#[derive(Debug, Deserialize)]
struct ExpectedFixture {
    status: String,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    order_0_correction: Option<Vec<bool>>,
    #[serde(default)]
    order_1_correction: Option<Vec<bool>>,
}

impl LsdFixture {
    fn load(name: &str) -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("lsd")
            .join(name);
        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        serde_json::from_str(&contents)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
    }

    fn pcm(&self) -> ParityCheckMatrix {
        ParityCheckMatrix::from_sparse_rows(
            self.matrix.num_checks,
            self.matrix.num_bits,
            self.matrix.rows.clone(),
        )
        .unwrap_or_else(|error| panic!("invalid matrix in {}: {error}", self.id))
    }

    fn channel(&self) -> ChannelModel {
        match &self.channel {
            ChannelFixture::Bsc { error_rate } => ChannelModel::Bsc {
                error_rate: *error_rate,
            },
            ChannelFixture::BitFlipProbabilities { probabilities } => {
                ChannelModel::BitFlipProbabilities(probabilities.clone())
            }
        }
    }

    fn syndrome(&self) -> Syndrome {
        Syndrome::from(self.syndrome.clone())
    }

    fn lsd_config(&self) -> LsdConfig {
        LsdConfig {
            lsd_order: self.lsd_order,
            ..LsdConfig::default()
        }
    }
}
```

- [ ] **Step 5: Add the order-one positive fixture test**

Add this test near the existing LSD integration tests in `rbposd/tests/lsd.rs`:

```rust
#[test]
fn bplsd_order_one_recovers_the_borrowed_small_matrix_cases() {
    for fixture_name in [
        "lsd_small_sparse_code.json",
        "lsd_order_one_improves_over_baseline.json",
    ] {
        let fixture = LsdFixture::load(fixture_name);
        assert_eq!(fixture.expected.status, "success");

        let pcm = fixture.pcm();
        let syndrome = fixture.syndrome();
        let decoder = BpLsdDecoder::new(pcm.clone(), fixture.channel(), fixture.lsd_config())
            .unwrap_or_else(|error| panic!("failed to construct decoder for {}: {error}", fixture.id));
        let result = decoder
            .decode(&syndrome)
            .unwrap_or_else(|error| panic!("failed to decode {}: {error}", fixture.id));

        assert!(!result.used_osd, "fixture {} unexpectedly used OSD", fixture.id);
        assert_eq!(result.residual_syndrome_weight, 0, "fixture {}", fixture.id);
        assert_eq!(pcm.multiply(&result.correction), syndrome, "fixture {}", fixture.id);

        if let Some(expected_order_1) = fixture.expected.order_1_correction {
            let expected_order_1 = Correction::from(expected_order_1);
            assert_eq!(result.correction, expected_order_1, "fixture {}", fixture.id);
        }

        if let Some(expected_order_0) = fixture.expected.order_0_correction {
            let order_0_decoder = BpLsdDecoder::new(
                pcm.clone(),
                fixture.channel(),
                LsdConfig {
                    lsd_order: 0,
                    ..LsdConfig::default()
                },
            )
            .unwrap_or_else(|error| {
                panic!("failed to construct order-0 decoder for {}: {error}", fixture.id)
            });
            let order_0_result = order_0_decoder
                .decode(&syndrome)
                .unwrap_or_else(|error| panic!("failed order-0 decode for {}: {error}", fixture.id));
            let expected_order_0 = Correction::from(expected_order_0);
            assert_eq!(order_0_result.correction, expected_order_0, "fixture {}", fixture.id);
            assert_ne!(
                result.correction, order_0_result.correction,
                "fixture {} did not exercise a distinct order-1 correction",
                fixture.id
            );
        }
    }
}
```

- [ ] **Step 6: Add the negative fixture test**

Add this test after the positive fixture test:

```rust
#[test]
fn bplsd_returns_a_decoder_error_for_an_unsatisfiable_case() {
    let fixture = LsdFixture::load("lsd_unsatisfiable_case.json");
    assert_eq!(fixture.expected.status, "error");
    assert_eq!(fixture.expected.error.as_deref(), Some("NoLsdSolution"));

    let pcm = fixture.pcm();
    let syndrome = fixture.syndrome();
    let decoder = BpLsdDecoder::new(pcm, fixture.channel(), fixture.lsd_config())
        .unwrap_or_else(|error| panic!("failed to construct decoder for {}: {error}", fixture.id));

    let error = decoder.decode(&syndrome).unwrap_err();

    assert_eq!(error, DecodeError::NoLsdSolution);
}
```

- [ ] **Step 7: Update the unsupported-order integration test**

Rename `bplsddecoder_rejects_nonzero_lsd_order_until_algorithm_milestone` to:

```rust
fn bplsddecoder_rejects_lsd_order_above_first_supported_order()
```

Inside that test, change:

```rust
        lsd_order: 1,
```

to:

```rust
        lsd_order: 2,
```

Change the final assertion to:

```rust
    assert_eq!(err, DecodeError::UnsupportedLsdOrder { order: 2 });
```

- [ ] **Step 8: Run the new tests to verify they fail for the intended reasons**

Run:

```bash
cargo test -p rbposd bplsd_order_one_recovers_the_borrowed_small_matrix_cases
cargo test -p rbposd bplsd_returns_a_decoder_error_for_an_unsatisfiable_case
cargo test -p rbposd bplsddecoder_rejects_lsd_order_above_first_supported_order
```

Expected:

- `bplsd_order_one_recovers_the_borrowed_small_matrix_cases` FAILS because `lsd_order = 1` still returns `UnsupportedLsdOrder`.
- `bplsd_returns_a_decoder_error_for_an_unsatisfiable_case` FAILS because `lsd_order = 1` still returns `UnsupportedLsdOrder`.
- `bplsddecoder_rejects_lsd_order_above_first_supported_order` PASSES because order 2 remains unsupported.

- [ ] **Step 9: Commit Task 2**

Run:

```bash
git add rbposd/tests/fixtures/lsd/lsd_small_sparse_code.json rbposd/tests/fixtures/lsd/lsd_order_one_improves_over_baseline.json rbposd/tests/fixtures/lsd/lsd_unsatisfiable_case.json rbposd/tests/lsd.rs
git commit -m "test: add rbposd lsd order one fixtures"
```

---

### Task 3: Extract Existing Order-0 LSD Residual Solve Into `lsd.rs`

**Files:**
- Create: `rbposd/src/lsd.rs`
- Modify: `rbposd/src/lib.rs`
- Modify: `rbposd/src/lsd_decoder.rs`

**Interfaces:**
- Consumes: `PreparedLinearSystem`, `ParityCheckMatrix`, `Correction`, `Syndrome`, existing `BpLsdDecoder` BP handoff data.
- Produces:
  - `pub(crate) struct LsdWorkspace`
  - `LsdWorkspace::new(pcm: &ParityCheckMatrix) -> Self`
  - `pub(crate) fn decode_lsd_with_workspace(...) -> Result<Correction, DecodeError>`

- [ ] **Step 1: Register the new internal module**

In `rbposd/src/lib.rs`, add this line after `mod gf2;`:

```rust
mod lsd;
```

- [ ] **Step 2: Create `rbposd/src/lsd.rs` with order-0 behavior and unit tests**

Create `rbposd/src/lsd.rs` with this initial content:

```rust
use crate::error::DecodeError;
use crate::gf2::{DetailedSolution, PreparedLinearSystem};
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

#[derive(Debug)]
pub(crate) struct LsdWorkspace {
    prepared: PreparedLinearSystem,
    column_order: Vec<usize>,
    local_rows: Vec<Vec<usize>>,
    local_to_global_bits: Vec<usize>,
    local_to_global_checks: Vec<usize>,
    local_reliability: Vec<f64>,
    candidate_bits: Vec<bool>,
}

impl LsdWorkspace {
    pub(crate) fn new(pcm: &ParityCheckMatrix) -> Self {
        Self {
            prepared: PreparedLinearSystem::from_pcm(pcm),
            column_order: (0..pcm.num_bits()).collect(),
            local_rows: Vec::new(),
            local_to_global_bits: Vec::new(),
            local_to_global_checks: Vec::new(),
            local_reliability: Vec::new(),
            candidate_bits: vec![false; pcm.num_bits()],
        }
    }
}

pub(crate) fn decode_lsd_with_workspace(
    pcm: &ParityCheckMatrix,
    target_syndrome: &Syndrome,
    reliability: &[f64],
    lsd_order: usize,
    workspace: &mut LsdWorkspace,
) -> Result<Correction, DecodeError> {
    debug_assert_eq!(target_syndrome.len(), pcm.num_checks());
    debug_assert_eq!(reliability.len(), pcm.num_bits());

    match lsd_order {
        0 => solve_order_zero(pcm, target_syndrome, reliability, workspace),
        1 => solve_order_zero(pcm, target_syndrome, reliability, workspace),
        _ => Err(DecodeError::UnsupportedLsdOrder { order: lsd_order }),
    }
}

fn solve_order_zero(
    pcm: &ParityCheckMatrix,
    target_syndrome: &Syndrome,
    reliability: &[f64],
    workspace: &mut LsdWorkspace,
) -> Result<Correction, DecodeError> {
    sort_unreliable_columns(&mut workspace.column_order, reliability);
    workspace
        .prepared
        .solve_with_column_order(target_syndrome, &workspace.column_order)
        .map_err(|_| DecodeError::NoLsdSolution)
        .and_then(|correction| verify_residual(pcm, target_syndrome, correction))
}

fn sort_unreliable_columns(column_order: &mut Vec<usize>, reliability: &[f64]) {
    column_order.clear();
    column_order.extend(0..reliability.len());
    column_order.sort_by(|&a, &b| {
        reliability[a]
            .partial_cmp(&reliability[b])
            .unwrap()
            .then_with(|| a.cmp(&b))
    });
}

fn verify_residual(
    pcm: &ParityCheckMatrix,
    target_syndrome: &Syndrome,
    correction: Correction,
) -> Result<Correction, DecodeError> {
    if pcm.multiply(&correction) == *target_syndrome {
        Ok(correction)
    } else {
        Err(DecodeError::NoLsdSolution)
    }
}

fn correction_cost(bits: &[bool], reliability: &[f64]) -> f64 {
    bits.iter()
        .zip(reliability.iter())
        .filter_map(|(&bit, &cost)| bit.then_some(cost))
        .sum()
}

fn is_better_candidate(candidate: &[bool], best: &[bool], reliability: &[f64]) -> bool {
    let candidate_cost = correction_cost(candidate, reliability);
    let best_cost = correction_cost(best, reliability);
    if candidate_cost < best_cost - f64::EPSILON {
        return true;
    }
    if (candidate_cost - best_cost).abs() <= f64::EPSILON {
        return candidate < best;
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::error::DecodeError;
    use crate::matrix::ParityCheckMatrix;
    use crate::vector::{Correction, Syndrome};

    use super::{LsdWorkspace, decode_lsd_with_workspace, is_better_candidate};

    #[test]
    fn order_zero_matches_existing_reliability_ordered_residual_solve() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![1, 2], vec![0]]).unwrap();
        let target_syndrome = Syndrome::from(vec![true, false]);
        let reliability = vec![1.0, 0.2, 0.4];
        let mut workspace = LsdWorkspace::new(&pcm);

        let correction =
            decode_lsd_with_workspace(&pcm, &target_syndrome, &reliability, 0, &mut workspace)
                .unwrap();

        assert_eq!(pcm.multiply(&correction), target_syndrome);
    }

    #[test]
    fn order_zero_maps_unsatisfiable_system_to_lsd_failure() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 1, vec![vec![0], vec![0]]).unwrap();
        let target_syndrome = Syndrome::from(vec![true, false]);
        let reliability = vec![1.0];
        let mut workspace = LsdWorkspace::new(&pcm);

        let error =
            decode_lsd_with_workspace(&pcm, &target_syndrome, &reliability, 0, &mut workspace)
                .unwrap_err();

        assert_eq!(error, DecodeError::NoLsdSolution);
    }

    #[test]
    fn candidate_tie_break_prefers_lexicographically_smaller_bits() {
        let candidate = vec![false, false, true];
        let best = vec![false, true, false];
        let reliability = vec![1.0, 0.0, 0.0];

        assert!(is_better_candidate(&candidate, &best, &reliability));
    }

    #[test]
    fn candidate_cost_break_prefers_lower_reliability_weight() {
        let candidate = vec![false, false, true];
        let best = vec![false, true, false];
        let reliability = vec![1.0, 0.5, 0.1];

        assert!(is_better_candidate(&candidate, &best, &reliability));
    }

    #[test]
    fn candidate_cost_break_rejects_higher_reliability_weight() {
        let candidate = vec![false, false, true];
        let best = vec![false, true, false];
        let reliability = vec![1.0, 0.1, 0.5];

        assert!(!is_better_candidate(&candidate, &best, &reliability));
    }

    #[test]
    fn correction_cost_uses_only_set_bits() {
        let bits = Correction::from(vec![true, false, true]);
        let reliability = vec![0.25, 100.0, 0.75];

        assert_eq!(super::correction_cost(bits.as_slice(), &reliability), 1.0);
    }
}
```

This file imports `DetailedSolution` before it is used by Task 4. If `cargo test -p rbposd lsd` reports an unused import warning only under strict linting, remove the import in Task 3 and re-add it in Task 4.

- [ ] **Step 3: Change `BpLsdDecoder` to use `LsdWorkspace` for order 0**

In `rbposd/src/lsd_decoder.rs`, replace the imports:

```rust
use crate::gf2::PreparedLinearSystem;
use crate::matrix::ParityCheckMatrix;
```

with:

```rust
use crate::lsd::{LsdWorkspace, decode_lsd_with_workspace};
use crate::matrix::ParityCheckMatrix;
```

Change the struct field:

```rust
    fallback_workspace: Mutex<LsdFallbackWorkspace>,
```

to:

```rust
    lsd_workspace: Mutex<LsdWorkspace>,
```

In `Clone::clone`, replace:

```rust
            fallback_workspace: Mutex::new(LsdFallbackWorkspace::new(&self.pcm)),
```

with:

```rust
            lsd_workspace: Mutex::new(LsdWorkspace::new(&self.pcm)),
```

In `new(...)`, replace:

```rust
        let fallback_workspace = Mutex::new(LsdFallbackWorkspace::new(&pcm));
```

with:

```rust
        let lsd_workspace = Mutex::new(LsdWorkspace::new(&pcm));
```

In the returned `Self`, replace:

```rust
            fallback_workspace,
```

with:

```rust
            lsd_workspace,
```

In `decode(...)`, replace the current fallback block:

```rust
        let correction = {
            let mut fallback_workspace = self.fallback_workspace.lock().unwrap();
            fallback_workspace.solve_order_zero(
                &self.pcm,
                syndrome,
                &bp_workspace.hard_decision_bits,
                &bp_workspace.reliability,
            )?
        };
```

with:

```rust
        let correction = {
            let target_syndrome = xor_syndromes(&multiply_bits(&self.pcm, &bp_workspace.hard_decision_bits), syndrome);
            let residual = {
                let mut lsd_workspace = self.lsd_workspace.lock().unwrap();
                decode_lsd_with_workspace(
                    &self.pcm,
                    &target_syndrome,
                    &bp_workspace.reliability,
                    self.config.lsd_order,
                    &mut lsd_workspace,
                )?
            };
            xor_correction_bits(&bp_workspace.hard_decision_bits, &residual)
        };
```

Delete the entire `LsdFallbackWorkspace` struct and its `impl` from `rbposd/src/lsd_decoder.rs`. Keep the helper functions `multiply_bits`, `xor_syndromes`, and `xor_correction_bits`.

- [ ] **Step 4: Run order-0 regression tests**

Run:

```bash
cargo test -p rbposd --test lsd bplsddecoder_public_api_matches_reference_contract
cargo test -p rbposd --test lsd bplsddecoder_order_zero_fallback_repairs_bp_residual_without_osd
cargo test -p rbposd lsd::tests
```

Expected: PASS for order-0 behavior and internal LSD unit tests. The order-1 fixture tests from Task 2 still fail because the constructor still rejects `lsd_order = 1`.

- [ ] **Step 5: Commit Task 3**

Run:

```bash
git add rbposd/src/lib.rs rbposd/src/lsd.rs rbposd/src/lsd_decoder.rs
git commit -m "refactor: move rbposd lsd residual solve"
```

---

### Task 4: Implement Deterministic LSD Order-1 Candidate Search

**Files:**
- Modify: `rbposd/src/lsd.rs`
- Modify: `rbposd/src/lsd_decoder.rs`
- Modify: `rbposd/src/error.rs`
- Modify: `rbposd/tests/smoke.rs`

**Interfaces:**
- Consumes: `decode_lsd_with_workspace(...)` and `LsdWorkspace` from Task 3.
- Produces: `lsd_order = 1` support in `BpLsdDecoder`, deterministic order-1 residual correction, `UnsupportedLsdOrder` display text for orders above 1.

- [ ] **Step 1: Add internal order-1 unit tests**

In `rbposd/src/lsd.rs`, add these tests to the existing `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn order_one_prefers_forced_free_column_on_component_tie() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0], vec![1, 2]]).unwrap();
        let target_syndrome = Syndrome::from(vec![false, true]);
        let reliability = vec![1_000_000_000.5, 0.0, 0.0];
        let mut workspace = LsdWorkspace::new(&pcm);

        let correction =
            decode_lsd_with_workspace(&pcm, &target_syndrome, &reliability, 1, &mut workspace)
                .unwrap();

        assert_eq!(correction, Correction::from(vec![false, false, true]));
        assert_eq!(pcm.multiply(&correction), target_syndrome);
    }

    #[test]
    fn order_one_reports_lsd_failure_for_inconsistent_component() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 1, vec![vec![0], vec![0]]).unwrap();
        let target_syndrome = Syndrome::from(vec![true, false]);
        let reliability = vec![1_000_000_000.0];
        let mut workspace = LsdWorkspace::new(&pcm);

        let error =
            decode_lsd_with_workspace(&pcm, &target_syndrome, &reliability, 1, &mut workspace)
                .unwrap_err();

        assert_eq!(error, DecodeError::NoLsdSolution);
    }
```

- [ ] **Step 2: Run the order-1 unit tests to verify they fail**

Run:

```bash
cargo test -p rbposd order_one_prefers_forced_free_column_on_component_tie
cargo test -p rbposd order_one_reports_lsd_failure_for_inconsistent_component
```

Expected:

- The first test FAILS because order 1 still delegates to order 0 and returns `Correction::from(vec![false, true, false])`.
- The second test PASSes if order 0 already maps the inconsistent system to `NoLsdSolution`.

- [ ] **Step 3: Add cluster data structures and order-1 dispatch**

In `rbposd/src/lsd.rs`, add this struct after `LsdWorkspace`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
struct LsdCluster {
    checks: Vec<usize>,
    bits: Vec<usize>,
}
```

In `decode_lsd_with_workspace`, replace the order-1 branch:

```rust
        1 => solve_order_zero(pcm, target_syndrome, reliability, workspace),
```

with:

```rust
        1 => solve_order_one(pcm, target_syndrome, reliability, workspace),
```

- [ ] **Step 4: Add component-cluster construction helpers**

In `rbposd/src/lsd.rs`, add these helpers after `sort_unreliable_columns`:

```rust
fn build_unsatisfied_clusters(
    pcm: &ParityCheckMatrix,
    target_syndrome: &Syndrome,
    reliability: &[f64],
) -> Vec<LsdCluster> {
    let mut visited_checks = vec![false; pcm.num_checks()];
    let mut clusters = Vec::new();
    for check in 0..pcm.num_checks() {
        if !target_syndrome.as_slice()[check] || visited_checks[check] {
            continue;
        }
        clusters.push(build_component_cluster(pcm, check, reliability, &mut visited_checks));
    }
    clusters
}

fn build_component_cluster(
    pcm: &ParityCheckMatrix,
    start_check: usize,
    reliability: &[f64],
    visited_checks: &mut [bool],
) -> LsdCluster {
    let mut checks = vec![start_check];
    let mut bits = Vec::new();
    visited_checks[start_check] = true;

    let mut cursor = 0usize;
    while cursor < checks.len() {
        let check = checks[cursor];
        for &bit in pcm.row_neighbors(check) {
            insert_sorted_by_reliability(&mut bits, bit, reliability);
            for neighbor_check in 0..pcm.num_checks() {
                if !visited_checks[neighbor_check]
                    && pcm.row_neighbors(neighbor_check).contains(&bit)
                {
                    visited_checks[neighbor_check] = true;
                    checks.push(neighbor_check);
                }
            }
        }
        cursor += 1;
    }

    checks.sort_unstable();
    bits.dedup();
    bits.sort_by(|&a, &b| {
        reliability[a]
            .partial_cmp(&reliability[b])
            .unwrap()
            .then_with(|| a.cmp(&b))
    });
    LsdCluster { checks, bits }
}

fn insert_sorted_by_reliability(bits: &mut Vec<usize>, bit: usize, reliability: &[f64]) {
    if bits.contains(&bit) {
        return;
    }
    bits.push(bit);
    bits.sort_by(|&a, &b| {
        reliability[a]
            .partial_cmp(&reliability[b])
            .unwrap()
            .then_with(|| a.cmp(&b))
    });
}
```

This helper deliberately closes over connected checks touched by cluster bits. That keeps the first landing deterministic and localized by connected component while avoiding a public `bits_per_step` parameter.

- [ ] **Step 5: Add local problem construction and candidate search helpers**

In `rbposd/src/lsd.rs`, add these helpers before `verify_residual`:

```rust
fn solve_order_one(
    pcm: &ParityCheckMatrix,
    target_syndrome: &Syndrome,
    reliability: &[f64],
    workspace: &mut LsdWorkspace,
) -> Result<Correction, DecodeError> {
    let clusters = build_unsatisfied_clusters(pcm, target_syndrome, reliability);
    workspace.candidate_bits.clear();
    workspace.candidate_bits.resize(pcm.num_bits(), false);

    for cluster in clusters {
        let local = solve_cluster_order_one(pcm, target_syndrome, reliability, &cluster, workspace)?;
        for (global_bit, bit) in cluster.bits.iter().copied().zip(local.as_slice().iter().copied()) {
            if bit {
                workspace.candidate_bits[global_bit] ^= true;
            }
        }
    }

    verify_residual(
        pcm,
        target_syndrome,
        Correction::from(workspace.candidate_bits.clone()),
    )
}

fn solve_cluster_order_one(
    pcm: &ParityCheckMatrix,
    target_syndrome: &Syndrome,
    reliability: &[f64],
    cluster: &LsdCluster,
    workspace: &mut LsdWorkspace,
) -> Result<Correction, DecodeError> {
    build_local_problem(pcm, target_syndrome, reliability, cluster, workspace)?;
    let local_pcm = ParityCheckMatrix::from_sparse_rows(
        workspace.local_to_global_checks.len(),
        workspace.local_to_global_bits.len(),
        workspace.local_rows.clone(),
    )
    .map_err(|_| DecodeError::NoLsdSolution)?;
    let local_syndrome = Syndrome::from(
        workspace
            .local_to_global_checks
            .iter()
            .map(|&check| target_syndrome.as_slice()[check])
            .collect::<Vec<_>>(),
    );
    let mut local_prepared = PreparedLinearSystem::from_pcm(&local_pcm);
    let local_order = (0..workspace.local_to_global_bits.len()).collect::<Vec<_>>();

    let base = local_prepared
        .solve_with_column_order_detailed(&local_syndrome, &local_order, &[])
        .map_err(|_| DecodeError::NoLsdSolution)?;
    let best = best_order_one_candidate(
        &mut local_prepared,
        &local_syndrome,
        &local_order,
        &workspace.local_reliability,
        base,
    );
    Ok(best.correction)
}

fn build_local_problem(
    pcm: &ParityCheckMatrix,
    _target_syndrome: &Syndrome,
    reliability: &[f64],
    cluster: &LsdCluster,
    workspace: &mut LsdWorkspace,
) -> Result<(), DecodeError> {
    workspace.local_to_global_checks.clear();
    workspace.local_to_global_checks.extend(cluster.checks.iter().copied());
    workspace.local_to_global_bits.clear();
    workspace.local_to_global_bits.extend(cluster.bits.iter().copied());
    workspace.local_to_global_bits.sort_by(|&a, &b| {
        reliability[a]
            .partial_cmp(&reliability[b])
            .unwrap()
            .then_with(|| a.cmp(&b))
    });
    workspace.local_reliability.clear();
    workspace
        .local_reliability
        .extend(workspace.local_to_global_bits.iter().map(|&bit| reliability[bit]));

    workspace.local_rows.clear();
    for &global_check in &workspace.local_to_global_checks {
        let mut local_row = Vec::new();
        for (local_bit, &global_bit) in workspace.local_to_global_bits.iter().enumerate() {
            if pcm.row_neighbors(global_check).contains(&global_bit) {
                local_row.push(local_bit);
            }
        }
        workspace.local_rows.push(local_row);
    }
    Ok(())
}

fn best_order_one_candidate(
    prepared: &mut PreparedLinearSystem,
    syndrome: &Syndrome,
    column_order: &[usize],
    reliability: &[f64],
    base: DetailedSolution,
) -> DetailedSolution {
    let mut best = base;
    let free_columns = best.free_columns.clone();
    for column in free_columns {
        if let Ok(candidate) =
            prepared.solve_with_column_order_detailed(syndrome, column_order, &[column])
        {
            if is_better_candidate(
                candidate.correction.as_slice(),
                best.correction.as_slice(),
                reliability,
            ) {
                best = candidate;
            }
        }
    }
    best
}
```

- [ ] **Step 6: Update `BpLsdDecoder` order acceptance**

In `rbposd/src/lsd_decoder.rs`, replace:

```rust
        if config.lsd_order != 0 {
            return Err(DecodeError::UnsupportedLsdOrder {
                order: config.lsd_order,
            });
        }
```

with:

```rust
        if config.lsd_order > 1 {
            return Err(DecodeError::UnsupportedLsdOrder {
                order: config.lsd_order,
            });
        }
```

- [ ] **Step 7: Update unsupported-order display text**

In `rbposd/src/error.rs`, replace the `UnsupportedLsdOrder` display arm with:

```rust
            Self::UnsupportedLsdOrder { order } => {
                write!(
                    f,
                    "unsupported LSD order {order}; only orders 0 and 1 are supported"
                )
            }
```

In `rbposd/tests/smoke.rs`, update the existing unsupported-order assertion to:

```rust
    assert_eq!(
        DecodeError::UnsupportedLsdOrder { order: 2 }.to_string(),
        "unsupported LSD order 2; only orders 0 and 1 are supported"
    );
```

- [ ] **Step 8: Run internal and integration LSD tests**

Run:

```bash
cargo test -p rbposd order_one_prefers_forced_free_column_on_component_tie
cargo test -p rbposd order_one_reports_lsd_failure_for_inconsistent_component
cargo test -p rbposd bplsd_order_one_recovers_the_borrowed_small_matrix_cases
cargo test -p rbposd bplsd_returns_a_decoder_error_for_an_unsatisfiable_case
cargo test -p rbposd bplsddecoder_rejects_lsd_order_above_first_supported_order
cargo test -p rbposd correction_helpers_and_error_display_cover_remaining_contracts
```

Expected: PASS.

- [ ] **Step 9: Run the full LSD integration test file**

Run:

```bash
cargo test -p rbposd --test lsd
```

Expected: PASS.

- [ ] **Step 10: Commit Task 4**

Run:

```bash
git add rbposd/src/lsd.rs rbposd/src/lsd_decoder.rs rbposd/src/error.rs rbposd/tests/smoke.rs
git commit -m "feat: implement rbposd lsd order one path"
```

---

### Task 5: Document Issue #89 Contract And Run Full Verification

**Files:**
- Modify: `rbposd/doc/ldpc_mvp_reference.md`
- Modify: `rbposd/tests/reference.rs`

**Interfaces:**
- Consumes: completed order-1 LSD behavior and `DecodeError::NoLsdSolution`.
- Produces: reference documentation that states the new order support and keeps #90/#98 boundaries explicit.

- [ ] **Step 1: Write failing reference-doc assertion**

In `rbposd/tests/reference.rs`, update the `required` array inside `task_6_documentation_surfaces_exist` so it includes:

```rust
        "NoLsdSolution",
        "lsd_order=1",
        "lsd_small_sparse_code.json",
        "#90/#98",
```

The full array should read:

```rust
    for required in [
        "BpLsdDecoder",
        "LsdConfig",
        "LsdMethod",
        "UnsupportedLsdOrder",
        "NoLsdSolution",
        "lsd_order=1",
        "lsd_small_sparse_code.json",
        "#90/#98",
    ] {
```

- [ ] **Step 2: Run the reference-doc test to verify it fails**

Run:

```bash
cargo test -p rbposd task_6_documentation_surfaces_exist
```

Expected: FAIL because `rbposd/doc/ldpc_mvp_reference.md` does not yet mention all new strings.

- [ ] **Step 3: Update `rbposd/doc/ldpc_mvp_reference.md` error list**

In the `DecodeError` variants list, replace:

```markdown
  `BpDidNotConverge`, `NoOsdSolution`,
  `UnsupportedLsdOrder { order: usize }`
```

with:

```markdown
  `BpDidNotConverge`, `NoOsdSolution`, `NoLsdSolution`,
  `UnsupportedLsdOrder { order: usize }`
```

- [ ] **Step 4: Update the LSD public API contract section**

Replace the bullet list under `The issue #88 behavior is intentionally narrow:` with:

```markdown
The issue #89 behavior remains narrow but now includes the first real supported
LSD solve path:

- `LsdMethod::LocalizedStatistics` is the only public LSD method variant.
- `lsd_order=0` is the order-0 residual solve baseline.
- `lsd_order=1` runs the first deterministic localized LSD solve path.
- `lsd_order>1` returns `DecodeError::UnsupportedLsdOrder`.
- LSD failures return `DecodeError::NoLsdSolution`.
- successful decodes return `DecodeResult` and keep `used_osd=false`.
```

Then replace the final follow-up paragraph with:

```markdown
Issue #89 checks in a minimal Rust-side fixture set under
`rbposd/tests/fixtures/lsd/`, including `lsd_small_sparse_code.json`,
`lsd_order_one_improves_over_baseline.json`, and
`lsd_unsatisfiable_case.json`.

Fixture manifests, Python `ldpc` differential harness coverage, and broader
fixture catalog validation are owned by #90/#98.
```

- [ ] **Step 5: Run reference-doc test**

Run:

```bash
cargo test -p rbposd task_6_documentation_surfaces_exist
```

Expected: PASS.

- [ ] **Step 6: Run required issue #89 verification commands**

Run:

```bash
cargo test -p rbposd bplsd_order_one_recovers_the_borrowed_small_matrix_cases
cargo test -p rbposd bplsd_returns_a_decoder_error_for_an_unsatisfiable_case
cargo test -p rbposd --test lsd
cargo test -p rbposd
```

Expected: PASS.

- [ ] **Step 7: Run formatting and diff hygiene**

Run:

```bash
cargo fmt --check --package rbposd
git diff --check
```

Expected: PASS.

If `cargo fmt --check --package rbposd` fails, run:

```bash
cargo fmt --package rbposd
cargo fmt --check --package rbposd
```

Expected after formatting: PASS.

- [ ] **Step 8: Commit Task 5**

Run:

```bash
git add rbposd/doc/ldpc_mvp_reference.md rbposd/tests/reference.rs
git commit -m "docs: document rbposd lsd order one contract"
```

---

## Final Verification

After all tasks are complete, run:

```bash
cargo test -p rbposd bplsd_order_one_recovers_the_borrowed_small_matrix_cases
cargo test -p rbposd bplsd_returns_a_decoder_error_for_an_unsatisfiable_case
cargo test -p rbposd --test lsd
cargo test -p rbposd
cargo fmt --check --package rbposd
git diff --check
```

Expected: every command passes.

Then check the final branch state:

```bash
git status --short
git log --oneline -5
```

Expected:

- `git status --short` prints nothing.
- The recent commits include the five task commits from this plan.
