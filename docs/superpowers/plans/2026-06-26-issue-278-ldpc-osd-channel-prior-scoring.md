# Issue 278 ldpc OSD Channel-Prior Scoring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Score `ldpc`-compatible OSD-CS candidates with channel-prior objective weights while preserving legacy BP-reliability scoring.

**Architecture:** Keep BP posterior reliability as the OSD column-ordering input. Add a separate candidate objective-weight slice to OSD comparison, pass BP reliability for legacy planners, and pass channel-prior LLR magnitudes for `OsdVariant::LdpcCombinationSweep`.

**Tech Stack:** Rust 2024 workspace; `rbposd` crate; existing BP/OSD/GF(2) helpers; `cargo test`.

## Global Constraints

- `OsdVariant::LdpcCombinationSweep` must rank candidates using channel-prior objective weights.
- Legacy Rust OSD modes must keep BP-reliability candidate scoring.
- Column ordering must continue to use BP posterior reliability in both modes.
- Objective weights are `abs(prior_llr)` values derived from the existing `ChannelModel` prior LLRs.
- All objective weights must be finite and dimension-matched before OSD candidate comparison.
- Do not change BP update rules.
- Do not change candidate enumeration shape outside `ldpc` mode.
- Do not run BB campaign sweeps.

---

## File Structure

- Modify `rbposd/src/decoder_core.rs`: expose finite channel-prior objective weights from `BpCore`.
- Modify `rbposd/src/osd.rs`: separate OSD ordering reliability from candidate objective weights.
- Modify `rbposd/src/decoder.rs`: choose BP reliability or channel-prior objective weights based on the effective OSD planner.
- Modify `rbposd/tests/osd.rs`: add the required positive and negative behavior controls.

### Task 1: Add Failing Candidate-Scoring Tests

**Files:**
- Modify: `rbposd/tests/osd.rs`

**Interfaces:**
- Consumes: `BpOsdDecoder::new`, `BpOsdDecoder::decode`, `ChannelModel`, `DecoderConfig`, `OsdVariant`, `ParityCheckMatrix`, `Syndrome`.
- Produces: failing tests named `ldpc_osd_cs_uses_channel_prior_candidate_weight` and `legacy_osd_candidate_scoring_keeps_existing_reliability_behavior`.

- [ ] **Step 1: Add the shared fixture helper**

Append this helper near the top of `rbposd/tests/osd.rs`, after the imports:

```rust
fn channel_prior_scoring_fixture() -> (ParityCheckMatrix, ChannelModel, Syndrome) {
    (
        ParityCheckMatrix::from_sparse_rows(2, 4, vec![vec![0, 1, 3], vec![1, 2, 3]]).unwrap(),
        ChannelModel::BitFlipProbabilities(vec![0.05, 0.15, 0.12, 0.08]),
        Syndrome::from(vec![true, true]),
    )
}
```

- [ ] **Step 2: Add the `ldpc` positive control**

Add this test after `ldpc_osd_cs_pair_candidate_can_improve_over_singles`:

```rust
#[test]
fn ldpc_osd_cs_uses_channel_prior_candidate_weight() {
    let (pcm, channel, syndrome) = channel_prior_scoring_fixture();
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        channel,
        DecoderConfig {
            max_bp_iterations: 1,
            osd_variant: OsdVariant::LdpcCombinationSweep,
            osd_order: 2,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let result = decoder.decode(&syndrome).unwrap();

    assert!(result.used_osd);
    assert_eq!(
        result.correction,
        Correction::from(vec![false, false, false, true])
    );
    assert_eq!(pcm.multiply(&result.correction), syndrome);
    assert_eq!(result.stats.osd_candidate_count, 3);
}
```

- [ ] **Step 3: Add the legacy negative control and invalid-probability check**

Add this test after `ldpc_osd_cs_uses_channel_prior_candidate_weight`:

```rust
#[test]
fn legacy_osd_candidate_scoring_keeps_existing_reliability_behavior() {
    let (pcm, channel, syndrome) = channel_prior_scoring_fixture();
    let legacy = BpOsdDecoder::new(
        pcm.clone(),
        channel,
        DecoderConfig {
            max_bp_iterations: 1,
            osd_variant: OsdVariant::LegacyCombinationSweep,
            osd_order: 2,
            ..DecoderConfig::default()
        },
    )
    .unwrap();

    let result = legacy.decode(&syndrome).unwrap();

    assert!(result.used_osd);
    assert_eq!(
        result.correction,
        Correction::from(vec![false, true, false, false])
    );
    assert_eq!(pcm.multiply(&result.correction), syndrome);

    let invalid = BpOsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.05, f64::NAN, 0.12, 0.08]),
        DecoderConfig {
            osd_variant: OsdVariant::LdpcCombinationSweep,
            osd_order: 2,
            ..DecoderConfig::default()
        },
    )
    .unwrap_err();
    assert_eq!(invalid, DecodeError::InvalidProbability);
}
```

- [ ] **Step 4: Verify the `ldpc` test fails before implementation**

Run:

```bash
cargo test -p rbposd ldpc_osd_cs_uses_channel_prior_candidate_weight -- --nocapture
```

Expected: FAIL because the current `ldpc` mode still uses BP reliability and returns `Correction::from(vec![false, true, false, false])`.

- [ ] **Step 5: Verify the legacy control passes before implementation**

Run:

```bash
cargo test -p rbposd legacy_osd_candidate_scoring_keeps_existing_reliability_behavior -q
```

Expected: PASS, confirming the fixture captures the existing BP-reliability behavior and invalid channel probabilities are already rejected.

### Task 2: Route Candidate Objective Weights

**Files:**
- Modify: `rbposd/src/decoder_core.rs`
- Modify: `rbposd/src/osd.rs`
- Modify: `rbposd/src/decoder.rs`

**Interfaces:**
- Consumes: failing tests from Task 1.
- Produces: channel-prior objective weights for `ldpc` OSD and preserved BP-reliability objective weights for legacy OSD.

- [ ] **Step 1: Expose finite prior objective weights**

In `rbposd/src/decoder_core.rs`, add this method to `impl BpCore` after `hard_decision_from_prior`:

```rust
    pub(crate) fn channel_prior_objective_weights(&self) -> &[f64] {
        &self.prior_llrs
    }
```

Then replace `compute_prior_llrs`'s `Ok(Self { ... })` construction in `BpCore::new` with validation that stores finite prior LLRs:

```rust
        let prior_llrs = compute_prior_llrs(pcm, channel)?;
        validate_objective_weights(&prior_llrs)?;
        Ok(Self {
            graph: CompiledGraph::from_pcm(pcm),
            prior_llrs,
        })
```

Add this helper below `compute_prior_llrs`:

```rust
fn validate_objective_weights(weights: &[f64]) -> Result<(), DecodeError> {
    if weights.iter().all(|weight| weight.is_finite()) {
        Ok(())
    } else {
        Err(DecodeError::InvalidProbability)
    }
}
```

The stored prior LLRs can be used directly because OSD scoring sums only true-bit
weights and `residual_cost` will take absolute values after Task 2 Step 2.

- [ ] **Step 2: Separate OSD ordering scores from objective weights**

In `rbposd/src/osd.rs`, rename the `reliability` argument of
`decode_osd_with_workspace` to `ordering_reliability` and add
`objective_weights: &[f64]` immediately after it:

```rust
pub(crate) fn decode_osd_with_workspace(
    pcm: &ParityCheckMatrix,
    syndrome: &Syndrome,
    base_correction_bits: &[bool],
    ordering_reliability: &[f64],
    objective_weights: &[f64],
    workspace: &mut OsdWorkspace,
    planner: OsdVariant,
    osd_order: usize,
) -> Result<OsdDecodeOutcome, DecodeError> {
```

Inside the function, validate dimensions and finite objective values before
sorting columns:

```rust
    debug_assert_eq!(ordering_reliability.len(), pcm.num_bits());
    validate_objective_weights(objective_weights, pcm.num_bits())?;
    let target_syndrome = xor_syndromes(&multiply_bits(pcm, base_correction_bits), syndrome);
    workspace.sort_unreliable_columns(ordering_reliability);
```

Update the legacy and `ldpc` candidate calls:

```rust
                best_legacy_osd_candidate(objective_weights, &reduced, base, osd_order, &mut stats)?
```

and:

```rust
            best_ldpc_osd_candidate(objective_weights, &reduced, base, osd_order, &mut stats)?
```

Change `decode_osd0_with_workspace` to pass `reliability` for both ordering and
objective weights:

```rust
        reliability,
        reliability,
```

Add this helper near `residual_cost`:

```rust
fn validate_objective_weights(weights: &[f64], expected: usize) -> Result<(), DecodeError> {
    if weights.len() != expected {
        return Err(DecodeError::DimensionMismatch {
            what: "OSD objective weights",
            expected,
            actual: weights.len(),
        });
    }
    if !weights.iter().all(|weight| weight.is_finite()) {
        return Err(DecodeError::InvalidProbability);
    }
    Ok(())
}
```

Change `residual_cost` to sum absolute objective weights:

```rust
fn residual_cost(bits: &[bool], weights: &[f64]) -> f64 {
    bits.iter()
        .zip(weights.iter())
        .filter_map(|(&bit, &weight)| bit.then_some(weight.abs()))
        .sum()
}
```

Do not change `diagnose_osd_candidate_search_with_workspace` or
`profile_osd_with_workspace`, because they do not compare candidates and only
need BP reliability for ordering.

- [ ] **Step 3: Pass the selected objective weights from the decoder**

In `rbposd/src/decoder.rs`, update the `decode_osd_with_workspace` call:

```rust
            let objective_weights = match effective_planner {
                crate::config::OsdVariant::LdpcCombinationSweep => {
                    self.core.channel_prior_objective_weights()
                }
                crate::config::OsdVariant::Osd0
                | crate::config::OsdVariant::LegacyCombinationSweep => &bp_workspace.reliability,
            };
            decode_osd_with_workspace(
                &self.pcm,
                syndrome,
                &bp_workspace.hard_decision_bits,
                &bp_workspace.reliability,
                objective_weights,
                &mut osd_workspace,
                effective_planner,
                self.config.osd_order,
            )?
```

This is the only public behavior switch: `LdpcCombinationSweep` uses channel
prior weights, and all legacy paths keep BP reliability.

- [ ] **Step 4: Verify the focused tests pass**

Run:

```bash
cargo test -p rbposd ldpc_osd_cs_uses_channel_prior_candidate_weight -- --nocapture
cargo test -p rbposd legacy_osd_candidate_scoring_keeps_existing_reliability_behavior -q
```

Expected: both PASS.

### Task 3: Regression Sweep And Cleanup

**Files:**
- Modify only files touched in Tasks 1 and 2 if cleanup is required.

**Interfaces:**
- Consumes: completed Task 2 implementation.
- Produces: formatted, verified branch ready for PR.

- [ ] **Step 1: Run formatting check on touched Rust files**

Run:

```bash
rustfmt --edition 2024 rbposd/src/decoder_core.rs rbposd/src/osd.rs rbposd/src/decoder.rs rbposd/tests/osd.rs --check
```

Expected: PASS. If it fails, run the same command without `--check`, review the diff, and re-run with `--check`.

- [ ] **Step 2: Run issue-required verification**

Run:

```bash
cargo test -p rbposd ldpc_osd_cs_uses_channel_prior_candidate_weight -- --nocapture
cargo test -p rbposd legacy_osd_candidate_scoring_keeps_existing_reliability_behavior -q
```

Expected: both PASS.

- [ ] **Step 3: Run broader verification**

Run:

```bash
cargo test -p rbposd
cargo test
```

Expected: both PASS. If the non-offline command attempts a network fetch in this sandbox, run the same command with `CARGO_NET_OFFLINE=true` and record the network limitation in the report.

- [ ] **Step 4: Review the diff**

Run:

```bash
git diff --check
git diff --stat
```

Expected: no whitespace errors, and changes limited to the spec/plan plus `rbposd` scoring implementation and tests.
