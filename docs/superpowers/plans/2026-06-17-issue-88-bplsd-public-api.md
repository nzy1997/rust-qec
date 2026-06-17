# Issue 88 BpLsdDecoder Public API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a first-class `BpLsdDecoder` public API to `rbposd` with minimal LSD config, order-0 behavior, and focused API/documentation tests.

**Architecture:** Keep `BpOsdDecoder` source-compatible and add `BpLsdDecoder` as a separate public decoder family. Extract only the small shared BP/channel-prior support into an internal `decoder_core.rs`, then let the LSD decoder reuse existing BP machinery and a narrow GF(2) order-0 fallback without claiming full LSD algorithm support.

**Tech Stack:** Rust 2024 workspace; `rbposd` crate; standard library only; `cargo test`; `cargo fmt`.

## Global Constraints

- Do not implement full LSD post-BP search in issue #88.
- Do not modify `rsinter`, benchmark runner params, DEM adapters, or result rows.
- Do not add LSD fixture catalogs, borrowed upstream differential cases, or Python parity harness support.
- Keep `BpOsdDecoder`, `DecoderConfig`, and existing call sites source-compatible.
- Keep `LsdConfig` separate from `DecoderConfig`.
- The only #88 `LsdMethod` variant is `LocalizedStatistics`.
- `LsdConfig::default()` must set `method = LsdMethod::LocalizedStatistics` and `lsd_order = 0`.
- `BpLsdDecoder::new(...)` must reject `lsd_order > 0` with `DecodeError::UnsupportedLsdOrder { order }`.
- `BpLsdDecoder::decode(...)` must return `DecodeResult` without changing the `DecodeResult` struct shape.
- `BpLsdDecoder` must set `used_osd = false`.
- No new crate dependencies.

---

## File Structure

- Modify `rbposd/src/config.rs`: add `LsdMethod` and `LsdConfig`.
- Modify `rbposd/src/error.rs`: add `DecodeError::UnsupportedLsdOrder { order: usize }` and display text.
- Modify `rbposd/dev/parity_schema.rs`: add a stable error code for `UnsupportedLsdOrder`.
- Modify `rbposd/tests/smoke.rs`: add LSD config default and unsupported-order display coverage.
- Modify `rbposd/tests/parity_dev.rs`: include `UnsupportedLsdOrder` in stable parity error-code tests.
- Create `rbposd/src/decoder_core.rs`: internal shared BP/channel-prior support.
- Modify `rbposd/src/decoder.rs`: keep `BpOsdDecoder` API unchanged while using `BpCore`.
- Create `rbposd/src/lsd_decoder.rs`: implement `BpLsdDecoder`.
- Create `rbposd/tests/lsd.rs`: public API and validation tests for `BpLsdDecoder`.
- Modify `rbposd/src/lib.rs`: add internal modules and top-level exports.
- Modify `rbposd/tests/reference.rs`: require crate docs to expose both OSD and LSD examples.
- Modify `rbposd/doc/ldpc_mvp_reference.md`: document the issue #88 LSD public contract.

---

### Task 1: Add LSD Config And Unsupported-Order Error Contract

**Files:**
- Modify: `rbposd/src/config.rs`
- Modify: `rbposd/src/error.rs`
- Modify: `rbposd/src/lib.rs`
- Modify: `rbposd/dev/parity_schema.rs`
- Modify: `rbposd/tests/smoke.rs`
- Modify: `rbposd/tests/parity_dev.rs`

**Interfaces:**
- Consumes: existing `DecodeError`, `DecoderConfig`, and config re-export patterns.
- Produces: `LsdMethod`, `LsdConfig`, and `DecodeError::UnsupportedLsdOrder { order: usize }` for later tasks.

- [ ] **Step 1: Write failing config and error tests**

Update the import in `rbposd/tests/smoke.rs` to include the new public types:

```rust
use rbposd::{
    BpVariant, ChannelModel, Correction, DecodeError, DecoderConfig, LsdConfig, LsdMethod,
    OsdVariant, Schedule,
};
```

Add this test after `decoder_config_default_contract` in `rbposd/tests/smoke.rs`:

```rust
#[test]
fn lsd_config_default_contract() {
    let cfg = LsdConfig::default();

    assert_eq!(cfg.method, LsdMethod::LocalizedStatistics);
    assert_eq!(cfg.lsd_order, 0);

    let method = LsdMethod::LocalizedStatistics;
    assert_eq!(method, cfg.method);
}
```

Add this assertion to `correction_helpers_and_error_display_cover_remaining_contracts` in `rbposd/tests/smoke.rs`:

```rust
    assert_eq!(
        DecodeError::UnsupportedLsdOrder { order: 1 }.to_string(),
        "unsupported LSD order 1; only order 0 is supported"
    );
```

Add this tuple to the `stable_error_cases` array in `rbposd/tests/parity_dev.rs`:

```rust
        (
            DecodeError::UnsupportedLsdOrder { order: 1 },
            "UnsupportedLsdOrder",
        ),
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p rbposd lsd_config_default_contract
cargo test -p rbposd parity_outcomes_use_stable_error_codes_and_partial_diagnostics_matching
```

Expected: FAIL with unresolved imports or missing variants for `LsdConfig`, `LsdMethod`, and `UnsupportedLsdOrder`.

- [ ] **Step 3: Add config types**

In `rbposd/src/config.rs`, add this enum after `OsdVariant`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LsdMethod {
    LocalizedStatistics,
}
```

Add this config struct after `DecoderConfig`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LsdConfig {
    pub method: LsdMethod,
    pub lsd_order: usize,
}
```

Add this default implementation after `impl Default for DecoderConfig`:

```rust
impl Default for LsdConfig {
    fn default() -> Self {
        Self {
            method: LsdMethod::LocalizedStatistics,
            lsd_order: 0,
        }
    }
}
```

Update the config re-export in `rbposd/src/lib.rs` to:

```rust
pub use config::{
    BpVariant, ChannelModel, DecoderConfig, LsdConfig, LsdMethod, OsdVariant, Schedule,
};
```

- [ ] **Step 4: Add unsupported-order error**

In `rbposd/src/error.rs`, add this enum variant after `NoOsdSolution`:

```rust
    UnsupportedLsdOrder { order: usize },
```

Add this display match arm after the `NoOsdSolution` arm:

```rust
            Self::UnsupportedLsdOrder { order } => {
                write!(f, "unsupported LSD order {order}; only order 0 is supported")
            }
```

In `rbposd/dev/parity_schema.rs`, add this match arm to `error_code`:

```rust
        DecodeError::UnsupportedLsdOrder { .. } => "UnsupportedLsdOrder",
```

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p rbposd lsd_config_default_contract
cargo test -p rbposd parity_outcomes_use_stable_error_codes_and_partial_diagnostics_matching
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add rbposd/src/config.rs rbposd/src/error.rs rbposd/src/lib.rs rbposd/dev/parity_schema.rs rbposd/tests/smoke.rs rbposd/tests/parity_dev.rs
git commit -m "feat: add rbposd lsd config contract"
```

---

### Task 2: Extract Shared BP Decoder Core

**Files:**
- Create: `rbposd/src/decoder_core.rs`
- Modify: `rbposd/src/lib.rs`
- Modify: `rbposd/src/decoder.rs`

**Interfaces:**
- Consumes: `ChannelModel`, `DecoderConfig`, `DecodeError`, `ParityCheckMatrix`, `Syndrome`, `Correction`, `CompiledGraph`, `BpWorkspace`, and `run_minimum_sum_compiled_in_place`.
- Produces: internal `BpCore` with:
  - `BpCore::new(pcm: &ParityCheckMatrix, channel: &ChannelModel) -> Result<BpCore, DecodeError>`
  - `BpCore::workspace(&self) -> BpWorkspace`
  - `BpCore::hard_decision_from_prior(&self) -> Correction`
  - `BpCore::run_minimum_sum_in_place(&self, syndrome: &Syndrome, config: &DecoderConfig, workspace: &mut BpWorkspace) -> BpRunInfo`

- [ ] **Step 1: Write failing internal tests**

Add `mod decoder_core;` to `rbposd/src/lib.rs` after `mod decoder;`.

Create `rbposd/src/decoder_core.rs` with only these tests:

```rust
#[cfg(test)]
mod tests {
    use crate::config::ChannelModel;
    use crate::error::DecodeError;
    use crate::matrix::ParityCheckMatrix;
    use crate::vector::Correction;

    use super::{BpCore, compute_prior_llrs, prior_hard_decision};

    #[test]
    fn computes_uniform_prior_llrs_from_bsc() {
        let pcm = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap();

        let llrs = compute_prior_llrs(&pcm, &ChannelModel::Bsc { error_rate: 0.2 }).unwrap();

        let expected = ((1.0_f64 - 0.2) / 0.2).ln();
        assert_eq!(llrs.len(), 3);
        assert!(llrs.iter().all(|value| (*value - expected).abs() < 1.0e-12));
    }

    #[test]
    fn rejects_probability_vector_length_mismatch() {
        let pcm = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap();

        let error = compute_prior_llrs(
            &pcm,
            &ChannelModel::BitFlipProbabilities(vec![0.1, 0.2]),
        )
        .unwrap_err();

        assert_eq!(
            error,
            DecodeError::DimensionMismatch {
                what: "channel probabilities",
                expected: 3,
                actual: 2,
            }
        );
    }

    #[test]
    fn prior_hard_decision_uses_negative_llrs() {
        let decision = prior_hard_decision(&[2.0, -1.0, 0.0, -0.5]);

        assert_eq!(decision, Correction::from(vec![false, true, false, true]));
    }

    #[test]
    fn bp_core_builds_workspace_for_its_compiled_graph() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
        let core = BpCore::new(&pcm, &ChannelModel::Bsc { error_rate: 0.05 }).unwrap();

        let workspace = core.workspace();

        assert_eq!(workspace.hard_decision_bits.len(), 3);
        assert_eq!(workspace.unsatisfied_checks.len(), 2);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p rbposd decoder_core
```

Expected: FAIL with unresolved `BpCore`, `compute_prior_llrs`, and `prior_hard_decision`.

- [ ] **Step 3: Implement shared decoder core**

Replace `rbposd/src/decoder_core.rs` with:

```rust
use crate::bp::{BpRunInfo, BpWorkspace, CompiledGraph, run_minimum_sum_compiled_in_place};
use crate::config::{ChannelModel, DecoderConfig};
use crate::error::DecodeError;
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

#[derive(Debug, Clone)]
pub(crate) struct BpCore {
    graph: CompiledGraph,
    prior_llrs: Vec<f64>,
}

impl BpCore {
    pub(crate) fn new(
        pcm: &ParityCheckMatrix,
        channel: &ChannelModel,
    ) -> Result<Self, DecodeError> {
        Ok(Self {
            graph: CompiledGraph::from_pcm(pcm),
            prior_llrs: compute_prior_llrs(pcm, channel)?,
        })
    }

    pub(crate) fn workspace(&self) -> BpWorkspace {
        BpWorkspace::new(&self.graph)
    }

    pub(crate) fn hard_decision_from_prior(&self) -> Correction {
        prior_hard_decision(&self.prior_llrs)
    }

    pub(crate) fn run_minimum_sum_in_place(
        &self,
        syndrome: &Syndrome,
        config: &DecoderConfig,
        workspace: &mut BpWorkspace,
    ) -> BpRunInfo {
        run_minimum_sum_compiled_in_place(
            &self.graph,
            syndrome,
            &self.prior_llrs,
            config,
            workspace,
        )
    }
}

pub(crate) fn compute_prior_llrs(
    pcm: &ParityCheckMatrix,
    channel: &ChannelModel,
) -> Result<Vec<f64>, DecodeError> {
    match channel {
        ChannelModel::Bsc { error_rate } => {
            let probability = validate_probability(*error_rate)?;
            let llr = probability_to_llr(probability);
            Ok(vec![llr; pcm.num_bits()])
        }
        ChannelModel::BitFlipProbabilities(probabilities) => {
            if probabilities.len() != pcm.num_bits() {
                return Err(DecodeError::DimensionMismatch {
                    what: "channel probabilities",
                    expected: pcm.num_bits(),
                    actual: probabilities.len(),
                });
            }

            probabilities
                .iter()
                .map(|&probability| validate_probability(probability).map(probability_to_llr))
                .collect()
        }
    }
}

fn validate_probability(probability: f64) -> Result<f64, DecodeError> {
    if !probability.is_finite() || probability <= 0.0 || probability >= 1.0 {
        return Err(DecodeError::InvalidProbability);
    }
    Ok(probability)
}

fn probability_to_llr(probability: f64) -> f64 {
    ((1.0 - probability) / probability).ln()
}

pub(crate) fn prior_hard_decision(prior_llrs: &[f64]) -> Correction {
    Correction::from(prior_llrs.iter().map(|&llr| llr < 0.0).collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use crate::config::ChannelModel;
    use crate::error::DecodeError;
    use crate::matrix::ParityCheckMatrix;
    use crate::vector::Correction;

    use super::{BpCore, compute_prior_llrs, prior_hard_decision};

    #[test]
    fn computes_uniform_prior_llrs_from_bsc() {
        let pcm = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap();

        let llrs = compute_prior_llrs(&pcm, &ChannelModel::Bsc { error_rate: 0.2 }).unwrap();

        let expected = ((1.0_f64 - 0.2) / 0.2).ln();
        assert_eq!(llrs.len(), 3);
        assert!(llrs.iter().all(|value| (*value - expected).abs() < 1.0e-12));
    }

    #[test]
    fn rejects_probability_vector_length_mismatch() {
        let pcm = ParityCheckMatrix::from_sparse_rows(1, 3, vec![vec![0, 1, 2]]).unwrap();

        let error = compute_prior_llrs(
            &pcm,
            &ChannelModel::BitFlipProbabilities(vec![0.1, 0.2]),
        )
        .unwrap_err();

        assert_eq!(
            error,
            DecodeError::DimensionMismatch {
                what: "channel probabilities",
                expected: 3,
                actual: 2,
            }
        );
    }

    #[test]
    fn prior_hard_decision_uses_negative_llrs() {
        let decision = prior_hard_decision(&[2.0, -1.0, 0.0, -0.5]);

        assert_eq!(decision, Correction::from(vec![false, true, false, true]));
    }

    #[test]
    fn bp_core_builds_workspace_for_its_compiled_graph() {
        let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
        let core = BpCore::new(&pcm, &ChannelModel::Bsc { error_rate: 0.05 }).unwrap();

        let workspace = core.workspace();

        assert_eq!(workspace.hard_decision_bits.len(), 3);
        assert_eq!(workspace.unsatisfied_checks.len(), 2);
    }
}
```

- [ ] **Step 4: Refactor `BpOsdDecoder` to use `BpCore`**

Replace `rbposd/src/decoder.rs` with:

```rust
use std::sync::Mutex;

use crate::bp::BpWorkspace;
use crate::config::{ChannelModel, DecoderConfig};
use crate::decoder_core::BpCore;
use crate::error::DecodeError;
use crate::matrix::ParityCheckMatrix;
use crate::osd::{OsdWorkspace, decode_osd_with_workspace};
use crate::vector::{Correction, Syndrome};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeResult {
    pub correction: Correction,
    pub converged: bool,
    pub bp_iterations: usize,
    pub used_osd: bool,
    pub residual_syndrome_weight: usize,
}

#[derive(Debug)]
pub struct BpOsdDecoder {
    pcm: ParityCheckMatrix,
    core: BpCore,
    config: DecoderConfig,
    bp_workspace: Mutex<BpWorkspace>,
    osd_workspace: Mutex<OsdWorkspace>,
}

impl Clone for BpOsdDecoder {
    fn clone(&self) -> Self {
        Self {
            pcm: self.pcm.clone(),
            core: self.core.clone(),
            config: self.config,
            bp_workspace: Mutex::new(self.core.workspace()),
            osd_workspace: Mutex::new(OsdWorkspace::new(&self.pcm)),
        }
    }
}

impl BpOsdDecoder {
    pub fn new(
        pcm: ParityCheckMatrix,
        channel: ChannelModel,
        config: DecoderConfig,
    ) -> Result<Self, DecodeError> {
        let core = BpCore::new(&pcm, &channel)?;
        let bp_workspace = Mutex::new(core.workspace());
        let osd_workspace = Mutex::new(OsdWorkspace::new(&pcm));
        Ok(Self {
            pcm,
            core,
            config,
            bp_workspace,
            osd_workspace,
        })
    }

    pub fn decode(&self, syndrome: &Syndrome) -> Result<DecodeResult, DecodeError> {
        if syndrome.len() != self.pcm.num_checks() {
            return Err(DecodeError::DimensionMismatch {
                what: "syndrome",
                expected: self.pcm.num_checks(),
                actual: syndrome.len(),
            });
        }

        if syndrome.weight() == 0 {
            let prior_correction = self.core.hard_decision_from_prior();
            if self.pcm.multiply(&prior_correction) == *syndrome {
                return Ok(DecodeResult {
                    correction: prior_correction,
                    converged: true,
                    bp_iterations: 0,
                    used_osd: false,
                    residual_syndrome_weight: 0,
                });
            }
        }

        let mut bp_workspace = self.bp_workspace.lock().unwrap();
        let bp_info =
            self.core
                .run_minimum_sum_in_place(syndrome, &self.config, &mut bp_workspace);
        if bp_info.residual_weight == 0 {
            return Ok(DecodeResult {
                correction: Correction::from(bp_workspace.hard_decision_bits.clone()),
                converged: bp_info.converged,
                bp_iterations: bp_info.iterations,
                used_osd: false,
                residual_syndrome_weight: bp_info.residual_weight,
            });
        }

        let correction = {
            let mut osd_workspace = self.osd_workspace.lock().unwrap();
            decode_osd_with_workspace(
                &self.pcm,
                syndrome,
                &bp_workspace.hard_decision_bits,
                &bp_workspace.reliability,
                &mut osd_workspace,
                self.config.osd_order,
            )?
        };
        drop(bp_workspace);

        Ok(DecodeResult {
            correction,
            converged: bp_info.converged,
            bp_iterations: bp_info.iterations,
            used_osd: true,
            residual_syndrome_weight: 0,
        })
    }
}
```

- [ ] **Step 5: Run focused and compatibility tests**

Run:

```bash
cargo test -p rbposd decoder_core
cargo test -p rbposd --test bp --test osd --test reuse
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add rbposd/src/lib.rs rbposd/src/decoder_core.rs rbposd/src/decoder.rs
git commit -m "refactor: share rbposd bp decoder core"
```

---

### Task 3: Add `BpLsdDecoder` Public API And Order-0 Fallback

**Files:**
- Create: `rbposd/tests/lsd.rs`
- Create: `rbposd/src/lsd_decoder.rs`
- Modify: `rbposd/src/lib.rs`

**Interfaces:**
- Consumes: `LsdConfig`, `LsdMethod`, `DecodeError::UnsupportedLsdOrder`, `BpCore`, `PreparedLinearSystem`, and `DecodeResult`.
- Produces:
  - `pub struct BpLsdDecoder`
  - `BpLsdDecoder::new(pcm: ParityCheckMatrix, channel: ChannelModel, config: LsdConfig) -> Result<Self, DecodeError>`
  - `BpLsdDecoder::decode(&self, syndrome: &Syndrome) -> Result<DecodeResult, DecodeError>`

- [ ] **Step 1: Write failing public API tests**

Create `rbposd/tests/lsd.rs`:

```rust
use rbposd::{
    BpLsdDecoder, ChannelModel, DecodeError, LsdConfig, ParityCheckMatrix, Syndrome,
};

#[test]
fn bplsddecoder_public_api_matches_reference_contract() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let decoder = BpLsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        LsdConfig::default(),
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(!result.used_osd);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}

#[test]
fn bplsddecoder_rejects_channel_length_mismatch() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();

    let err = BpLsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.2]),
        LsdConfig::default(),
    )
    .unwrap_err();

    assert_eq!(
        err,
        DecodeError::DimensionMismatch {
            what: "channel probabilities",
            expected: 3,
            actual: 2,
        }
    );
}

#[test]
fn bplsddecoder_rejects_nonzero_lsd_order_until_algorithm_milestone() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![0, 1]]).unwrap();
    let config = LsdConfig {
        lsd_order: 1,
        ..LsdConfig::default()
    };

    let err = BpLsdDecoder::new(pcm, ChannelModel::Bsc { error_rate: 0.05 }, config).unwrap_err();

    assert_eq!(err, DecodeError::UnsupportedLsdOrder { order: 1 });
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p rbposd --test lsd
```

Expected: FAIL with unresolved import `rbposd::BpLsdDecoder`.

- [ ] **Step 3: Implement `BpLsdDecoder`**

Create `rbposd/src/lsd_decoder.rs`:

```rust
use std::sync::Mutex;

use crate::bp::BpWorkspace;
use crate::config::{ChannelModel, DecoderConfig, LsdConfig, LsdMethod};
use crate::decoder::DecodeResult;
use crate::decoder_core::BpCore;
use crate::error::DecodeError;
use crate::gf2::PreparedLinearSystem;
use crate::matrix::ParityCheckMatrix;
use crate::vector::{Correction, Syndrome};

#[derive(Debug)]
pub struct BpLsdDecoder {
    pcm: ParityCheckMatrix,
    core: BpCore,
    config: LsdConfig,
    bp_config: DecoderConfig,
    bp_workspace: Mutex<BpWorkspace>,
    fallback_workspace: Mutex<LsdFallbackWorkspace>,
}

impl Clone for BpLsdDecoder {
    fn clone(&self) -> Self {
        Self {
            pcm: self.pcm.clone(),
            core: self.core.clone(),
            config: self.config,
            bp_config: self.bp_config,
            bp_workspace: Mutex::new(self.core.workspace()),
            fallback_workspace: Mutex::new(LsdFallbackWorkspace::new(&self.pcm)),
        }
    }
}

impl BpLsdDecoder {
    pub fn new(
        pcm: ParityCheckMatrix,
        channel: ChannelModel,
        config: LsdConfig,
    ) -> Result<Self, DecodeError> {
        match config.method {
            LsdMethod::LocalizedStatistics => {}
        }
        if config.lsd_order != 0 {
            return Err(DecodeError::UnsupportedLsdOrder {
                order: config.lsd_order,
            });
        }

        let core = BpCore::new(&pcm, &channel)?;
        let bp_workspace = Mutex::new(core.workspace());
        let fallback_workspace = Mutex::new(LsdFallbackWorkspace::new(&pcm));

        Ok(Self {
            pcm,
            core,
            config,
            bp_config: DecoderConfig::default(),
            bp_workspace,
            fallback_workspace,
        })
    }

    pub fn decode(&self, syndrome: &Syndrome) -> Result<DecodeResult, DecodeError> {
        if syndrome.len() != self.pcm.num_checks() {
            return Err(DecodeError::DimensionMismatch {
                what: "syndrome",
                expected: self.pcm.num_checks(),
                actual: syndrome.len(),
            });
        }

        if syndrome.weight() == 0 {
            let prior_correction = self.core.hard_decision_from_prior();
            if self.pcm.multiply(&prior_correction) == *syndrome {
                return Ok(DecodeResult {
                    correction: prior_correction,
                    converged: true,
                    bp_iterations: 0,
                    used_osd: false,
                    residual_syndrome_weight: 0,
                });
            }
        }

        let mut bp_workspace = self.bp_workspace.lock().unwrap();
        let bp_info =
            self.core
                .run_minimum_sum_in_place(syndrome, &self.bp_config, &mut bp_workspace);
        if bp_info.residual_weight == 0 {
            return Ok(DecodeResult {
                correction: Correction::from(bp_workspace.hard_decision_bits.clone()),
                converged: bp_info.converged,
                bp_iterations: bp_info.iterations,
                used_osd: false,
                residual_syndrome_weight: 0,
            });
        }

        let correction = {
            let mut fallback_workspace = self.fallback_workspace.lock().unwrap();
            fallback_workspace.solve_order_zero(
                &self.pcm,
                syndrome,
                &bp_workspace.hard_decision_bits,
                &bp_workspace.reliability,
            )?
        };
        drop(bp_workspace);

        Ok(DecodeResult {
            correction,
            converged: bp_info.converged,
            bp_iterations: bp_info.iterations,
            used_osd: false,
            residual_syndrome_weight: 0,
        })
    }
}

#[derive(Debug)]
struct LsdFallbackWorkspace {
    prepared: PreparedLinearSystem,
    column_order: Vec<usize>,
}

impl LsdFallbackWorkspace {
    fn new(pcm: &ParityCheckMatrix) -> Self {
        Self {
            prepared: PreparedLinearSystem::from_pcm(pcm),
            column_order: (0..pcm.num_bits()).collect(),
        }
    }

    fn solve_order_zero(
        &mut self,
        pcm: &ParityCheckMatrix,
        syndrome: &Syndrome,
        base_correction_bits: &[bool],
        reliability: &[f64],
    ) -> Result<Correction, DecodeError> {
        let target_syndrome = xor_syndromes(&multiply_bits(pcm, base_correction_bits), syndrome);
        self.sort_unreliable_columns(reliability);
        let residual = self
            .prepared
            .solve_with_column_order(&target_syndrome, &self.column_order)?;
        Ok(xor_correction_bits(base_correction_bits, &residual))
    }

    fn sort_unreliable_columns(&mut self, reliability: &[f64]) {
        self.column_order.clear();
        self.column_order.extend(0..reliability.len());
        self.column_order.sort_by(|&a, &b| {
            reliability[a]
                .partial_cmp(&reliability[b])
                .unwrap()
                .then_with(|| a.cmp(&b))
        });
    }
}

fn multiply_bits(pcm: &ParityCheckMatrix, bits: &[bool]) -> Syndrome {
    let mut syndrome = vec![false; pcm.num_checks()];
    for (check, value) in syndrome.iter_mut().enumerate() {
        let mut parity = false;
        for &bit in pcm.row_neighbors(check) {
            parity ^= bits[bit];
        }
        *value = parity;
    }
    Syndrome::from(syndrome)
}

fn xor_syndromes(lhs: &Syndrome, rhs: &Syndrome) -> Syndrome {
    Syndrome::from(
        lhs.as_slice()
            .iter()
            .zip(rhs.as_slice().iter())
            .map(|(a, b)| *a ^ *b)
            .collect::<Vec<_>>(),
    )
}

fn xor_correction_bits(lhs: &[bool], rhs: &Correction) -> Correction {
    Correction::from(
        lhs.iter()
            .zip(rhs.as_slice().iter())
            .map(|(a, b)| *a ^ *b)
            .collect::<Vec<_>>(),
    )
}
```

Update `rbposd/src/lib.rs` by adding this module declaration after `mod gf2;`:

```rust
mod lsd_decoder;
```

Add this top-level re-export after the decoder re-export:

```rust
pub use lsd_decoder::BpLsdDecoder;
```

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test -p rbposd --test lsd
```

Expected: PASS.

- [ ] **Step 5: Run compatibility tests**

Run:

```bash
cargo test -p rbposd --test bp --test osd --test reuse
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add rbposd/src/lsd_decoder.rs rbposd/src/lib.rs rbposd/tests/lsd.rs
git commit -m "feat: add rbposd bplsd decoder api"
```

---

### Task 4: Document The Public LSD Surface

**Files:**
- Modify: `rbposd/tests/reference.rs`
- Modify: `rbposd/src/lib.rs`
- Modify: `rbposd/doc/ldpc_mvp_reference.md`

**Interfaces:**
- Consumes: public exports `BpLsdDecoder`, `LsdConfig`, and `LsdMethod`.
- Produces: crate-level and contract-document references that make LSD API discoverable.

- [ ] **Step 1: Write failing documentation-surface checks**

In `rbposd/tests/reference.rs`, add this assertion after the existing crate-level usage example assertion:

```rust
    assert!(
        lib_contents.contains("use rbposd::{BpLsdDecoder, ChannelModel, LsdConfig, ParityCheckMatrix, Syndrome};"),
        "missing BpLsdDecoder crate-level usage example in {}",
        lib_rs_display
    );
```

Still in `task_6_documentation_surfaces_exist`, add this code after the crate-level assertions:

```rust
    let reference_doc = crate_root.join("doc/ldpc_mvp_reference.md");
    let reference_contents = fs::read_to_string(&reference_doc).unwrap();
    let reference_doc_display = reference_doc.display().to_string();
    for required in ["BpLsdDecoder", "LsdConfig", "LsdMethod", "UnsupportedLsdOrder"] {
        assert!(
            reference_contents.contains(required),
            "missing {required} in {}",
            reference_doc_display
        );
    }
```

- [ ] **Step 2: Run documentation test to verify it fails**

Run:

```bash
cargo test -p rbposd task_6_documentation_surfaces_exist
```

Expected: FAIL because `rbposd/src/lib.rs` and `rbposd/doc/ldpc_mvp_reference.md` do not yet mention the LSD public API.

- [ ] **Step 3: Add crate-level LSD example**

Add this second example block to the crate docs in `rbposd/src/lib.rs` after the existing `BpOsdDecoder` example:

```rust
//! ```rust
//! use rbposd::{BpLsdDecoder, ChannelModel, LsdConfig, ParityCheckMatrix, Syndrome};
//!
//! let pcm = ParityCheckMatrix::from_sparse_rows(
//!     2,
//!     3,
//!     vec![vec![0, 1], vec![1, 2]],
//! )
//! .unwrap();
//! let decoder = BpLsdDecoder::new(
//!     pcm.clone(),
//!     ChannelModel::Bsc { error_rate: 0.05 },
//!     LsdConfig::default(),
//! )
//! .unwrap();
//! let syndrome = Syndrome::from(vec![true, false]);
//! let result = decoder.decode(&syndrome).unwrap();
//! assert_eq!(pcm.multiply(&result.correction), syndrome);
//! ```
//!
```

- [ ] **Step 4: Update `ldpc_mvp_reference.md`**

In `rbposd/doc/ldpc_mvp_reference.md`, add this bullet group to the `Included:` list after the existing `DecoderConfig` bullet:

```markdown
- `LsdConfig` and its default contract:
  `method=LocalizedStatistics`, `lsd_order=0`
- `LsdMethod` with the first supported variant:
  `LocalizedStatistics`
```

Update the `DecodeError` variants list to include:

```markdown
  `BpDidNotConverge`, `NoOsdSolution`,
  `UnsupportedLsdOrder { order: usize }`
```

Update the crate exports bullet to include the LSD exports:

```markdown
  `BpVariant, ChannelModel, DecoderConfig, LsdConfig, LsdMethod,
  OsdVariant, Schedule, DecodeError`
```

Add this section after the `zero_iter_semantics_mismatch` paragraph:

````markdown
## LSD Public API Contract

Issue #88 adds `BpLsdDecoder` as a first-class public decoder family parallel
to `BpOsdDecoder`.

The supported construction path is:

```rust
let decoder = BpLsdDecoder::new(pcm, channel, LsdConfig::default())?;
let result = decoder.decode(&syndrome)?;
```

The issue #88 behavior is intentionally narrow:

- `LsdMethod::LocalizedStatistics` is the only public LSD method variant.
- `lsd_order=0` is the only supported order.
- `lsd_order>0` returns `DecodeError::UnsupportedLsdOrder`.
- successful decodes return `DecodeResult` and keep `used_osd=false`.
- the order-0 fallback is an API validity bridge, not the full LSD algorithm.

Full LSD post-BP search, nonzero-order behavior, borrowed LSD fixtures, and
Python `ldpc` differential harness coverage are owned by follow-on issues.
````

- [ ] **Step 5: Run documentation tests**

Run:

```bash
cargo test -p rbposd task_6_documentation_surfaces_exist
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add rbposd/src/lib.rs rbposd/tests/reference.rs rbposd/doc/ldpc_mvp_reference.md
git commit -m "docs: document rbposd bplsd public api"
```

---

### Task 5: Final Verification

**Files:**
- Verify: all files touched by Tasks 1-4

**Interfaces:**
- Consumes: completed `BpLsdDecoder` public API and docs.
- Produces: final confidence that issue #88 is complete and OSD behavior stayed compatible.

- [ ] **Step 1: Run formatting check**

Run:

```bash
cargo fmt --check --package rbposd
```

Expected: PASS.

If formatting fails, run:

```bash
cargo fmt --package rbposd
cargo fmt --check --package rbposd
git add rbposd
git commit -m "style: format rbposd bplsd api changes"
```

Expected after formatting: PASS.

- [ ] **Step 2: Run issue verification commands**

Run:

```bash
cargo test -p rbposd bplsddecoder_public_api_matches_reference_contract
cargo test -p rbposd bplsddecoder_rejects_channel_length_mismatch
cargo test -p rbposd
```

Expected: PASS for all three commands.

- [ ] **Step 3: Run diff hygiene check**

Run:

```bash
git diff --check
```

Expected: no output.

- [ ] **Step 4: Inspect final changed files**

Run:

```bash
git status --short
```

Expected: clean working tree after all task commits.
